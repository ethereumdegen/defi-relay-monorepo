//! Unified Paygate implementation supporting both V1 and V2 x402 protocols.
//!
//! This module provides a trait-based abstraction that allows sharing the core
//! payment gate logic between protocol versions while allowing version-specific
//! behavior through the [`PaygateProtocol`] trait.
//!
//! ## Overview
//!
//! The paygate handles:
//! - Extracting payment headers from requests
//! - Verifying payments with the facilitator
//! - Settling payments on-chain
//! - Returning appropriate 402 responses when payment is required

use actix_web::HttpResponse;
use http::{HeaderValue, StatusCode, Uri};
use serde_json::json;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use url::Url;
use x402_rs::facilitator::Facilitator;
use x402_rs::proto;
use x402_rs::proto::{SupportedResponse, v1, v2};
use x402_rs::util::Base64Bytes;

#[cfg(feature = "telemetry")]
use tracing::instrument;

// ============================================================================
// Common Types
// ============================================================================

/// Builder for resource information that can be used with both V1 and V2 protocols.
#[derive(Debug, Clone)]
pub struct ResourceInfoBuilder {
    /// Description of the protected resource
    pub description: String,
    /// MIME type of the protected resource
    pub mime_type: String,
    /// Optional explicit URL of the protected resource
    pub url: Option<String>,
}

impl Default for ResourceInfoBuilder {
    fn default() -> Self {
        Self {
            description: "".to_string(),
            mime_type: "application/json".to_string(),
            url: None,
        }
    }
}

impl ResourceInfoBuilder {
    /// Determines the resource URL (static or dynamic).
    ///
    /// If `url` is set, returns it directly. Otherwise, constructs a URL by combining
    /// the base URL with the request URI's path and query.
    pub fn as_resource_info(
        &self,
        base_url: Option<&Url>,
        headers: &actix_web::http::header::HeaderMap,
        uri: &Uri,
    ) -> v2::ResourceInfo {
        let url = self.url.clone().unwrap_or_else(|| {
            let mut url = base_url.cloned().unwrap_or_else(|| {
                let host = headers
                    .get("host")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("localhost");
                let origin = format!("http://{}", host);
                let url = Url::parse(&origin)
                    .unwrap_or_else(|_| Url::parse("http://localhost").unwrap());
                #[cfg(feature = "telemetry")]
                tracing::warn!(
                    "X402Middleware base_url is not configured; using {url} as origin for resource resolution"
                );
                url
            });
            url.set_path(uri.path());
            url.set_query(uri.query());
            url.to_string()
        });
        v2::ResourceInfo {
            description: self.description.clone(),
            mime_type: self.mime_type.clone(),
            url,
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Common verification errors shared between protocol versions.
#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("{0} header is required")]
    PaymentHeaderRequired(&'static str),
    #[error("Invalid or malformed payment header")]
    InvalidPaymentHeader,
    #[error("Unable to find matching payment requirements")]
    NoPaymentMatching,
    #[error("Verification failed: {0}")]
    VerificationFailed(String),
}

/// Paygate error type that wraps verification and settlement errors.
#[derive(Debug, thiserror::Error)]
pub enum PaygateError {
    #[error(transparent)]
    Verification(#[from] VerificationError),
    #[error("Settlement failed: {0}")]
    Settlement(String),
}

// ============================================================================
// PaygateProtocol Trait
// ============================================================================

/// Trait defining version-specific behavior for the x402 payment gate.
///
/// This trait is implemented directly on the price tag types (`V1PriceTag` and
/// `V2PriceTag`/`v2::PaymentRequirements`), allowing the core payment gate logic
/// to be shared while version-specific behavior is implemented separately.
pub trait PaygateProtocol: Clone + Send + Sync + 'static {
    /// The payment payload type extracted from the request header.
    type PaymentPayload: serde::de::DeserializeOwned + Send;

    /// The HTTP header name for the payment payload.
    const PAYMENT_HEADER_NAME: &'static str;

    /// Constructs a verify request from the payment payload and accepted requirements.
    fn make_verify_request(
        payload: Self::PaymentPayload,
        accepts: &[Self],
        resource: &v2::ResourceInfo,
    ) -> Result<proto::VerifyRequest, VerificationError>;

    /// Converts an error into an HTTP response with appropriate format.
    fn error_into_response(
        err: PaygateError,
        accepts: &[Self],
        resource: &v2::ResourceInfo,
    ) -> HttpResponse;

    /// Converts the verify response to the protocol-specific format and validates it.
    fn validate_verify_response(
        verify_response: proto::VerifyResponse,
    ) -> Result<(), VerificationError>;

    /// Enriches a price tag with facilitator capabilities.
    fn enrich_with_capabilities(&mut self, capabilities: &SupportedResponse);
}

// ============================================================================
// V1 Protocol Implementation (on v1::PriceTag)
// ============================================================================

impl PaygateProtocol for v1::PriceTag {
    type PaymentPayload = v1::PaymentPayload;

    const PAYMENT_HEADER_NAME: &'static str = "X-PAYMENT";

    fn make_verify_request(
        payment_payload: Self::PaymentPayload,
        accepts: &[Self],
        resource: &v2::ResourceInfo,
    ) -> Result<proto::VerifyRequest, VerificationError> {
        let selected = accepts
            .iter()
            .find(|requirement| {
                requirement.scheme == payment_payload.scheme
                    && requirement.network == payment_payload.network
            })
            .ok_or(VerificationError::NoPaymentMatching)?;

        let verify_request = v1::VerifyRequest {
            x402_version: v1::X402Version1,
            payment_payload,
            payment_requirements: price_tag_to_v1_requirements_with_resource(selected, resource),
        };

        verify_request
            .try_into()
            .map_err(|e| VerificationError::VerificationFailed(format!("{e}")))
    }

    fn error_into_response(
        err: PaygateError,
        accepts: &[Self],
        resource: &v2::ResourceInfo,
    ) -> HttpResponse {
        match err {
            PaygateError::Verification(err) => {
                let payment_required_response = v1::PaymentRequired {
                    error: Some(err.to_string()),
                    accepts: accepts
                        .iter()
                        .map(|pt| price_tag_to_v1_requirements_with_resource(pt, resource))
                        .collect(),
                    x402_version: v1::X402Version1,
                };
                let payment_required_response_bytes =
                    serde_json::to_vec(&payment_required_response).expect("serialization failed");
                HttpResponse::build(StatusCode::PAYMENT_REQUIRED)
                    .content_type("application/json")
                    .body(payment_required_response_bytes)
            }
            PaygateError::Settlement(err) => {
                let body = json!({
                    "error": "Settlement failed",
                    "details": err.to_string()
                })
                .to_string();
                HttpResponse::build(StatusCode::PAYMENT_REQUIRED)
                    .content_type("application/json")
                    .body(body)
            }
        }
    }

    fn validate_verify_response(
        verify_response: proto::VerifyResponse,
    ) -> Result<(), VerificationError> {
        let verify_response_v1: v1::VerifyResponse = verify_response
            .try_into()
            .map_err(|e| VerificationError::VerificationFailed(format!("{e}")))?;

        match verify_response_v1 {
            v1::VerifyResponse::Valid { .. } => Ok(()),
            v1::VerifyResponse::Invalid { reason, .. } => {
                Err(VerificationError::VerificationFailed(reason))
            }
        }
    }

    fn enrich_with_capabilities(&mut self, capabilities: &SupportedResponse) {
        self.enrich(capabilities);
    }
}

/// Helper function to convert V1PriceTag to v1::PaymentRequirements with resource info.
fn price_tag_to_v1_requirements_with_resource(
    price_tag: &v1::PriceTag,
    resource: &v2::ResourceInfo,
) -> v1::PaymentRequirements {
    v1::PaymentRequirements {
        scheme: price_tag.scheme.clone(),
        network: price_tag.network.clone(),
        max_amount_required: price_tag.amount.clone(),
        resource: resource.url.clone(),
        description: resource.description.clone(),
        mime_type: resource.mime_type.clone(),
        output_schema: None,
        pay_to: price_tag.pay_to.clone(),
        max_timeout_seconds: price_tag.max_timeout_seconds,
        asset: price_tag.asset.clone(),
        extra: price_tag.extra.clone(),
    }
}

// ============================================================================
// V2 Protocol Implementation (on v2::PriceTag)
// ============================================================================

impl PaygateProtocol for v2::PriceTag {
    type PaymentPayload = v2::PaymentPayload<v2::PaymentRequirements, serde_json::Value>;

    const PAYMENT_HEADER_NAME: &'static str = "Payment-Signature";

    fn make_verify_request(
        payment_payload: Self::PaymentPayload,
        accepts: &[Self],
        _resource: &v2::ResourceInfo,
    ) -> Result<proto::VerifyRequest, VerificationError> {
        let accepted = &payment_payload.accepted;

        let selected = accepts
            .iter()
            .find(|price_tag| **price_tag == *accepted)
            .ok_or(VerificationError::NoPaymentMatching)?;

        let verify_request = v2::VerifyRequest {
            x402_version: v2::X402Version2,
            payment_payload,
            payment_requirements: selected.requirements.clone(),
        };

        let json = serde_json::to_value(&verify_request)
            .map_err(|e| VerificationError::VerificationFailed(format!("{e}")))?;

        Ok(proto::VerifyRequest::from(json))
    }

    fn error_into_response(
        err: PaygateError,
        accepts: &[Self],
        resource: &v2::ResourceInfo,
    ) -> HttpResponse {
        match err {
            PaygateError::Verification(err) => {
                let payment_required_response = v2::PaymentRequired {
                    error: Some(err.to_string()),
                    accepts: accepts.iter().map(|pt| pt.requirements.clone()).collect(),
                    x402_version: v2::X402Version2,
                    resource: resource.clone(),
                };
                let payment_required_bytes =
                    serde_json::to_vec(&payment_required_response).expect("serialization failed");
                let payment_required_header = Base64Bytes::encode(&payment_required_bytes);
                let header_value = HeaderValue::from_bytes(payment_required_header.as_ref())
                    .expect("Failed to create header value");

                HttpResponse::build(StatusCode::PAYMENT_REQUIRED)
                    .append_header(("Payment-Required", header_value.to_str().unwrap_or("")))
                    .finish()
            }
            PaygateError::Settlement(err) => {
                let body = json!({
                    "error": "Settlement failed",
                    "details": err.to_string()
                })
                .to_string();
                HttpResponse::build(StatusCode::PAYMENT_REQUIRED)
                    .content_type("application/json")
                    .body(body)
            }
        }
    }

    fn validate_verify_response(
        verify_response: proto::VerifyResponse,
    ) -> Result<(), VerificationError> {
        let verify_response_v2: v2::VerifyResponse = verify_response
            .try_into()
            .map_err(|e| VerificationError::VerificationFailed(format!("{e}")))?;

        match verify_response_v2 {
            v2::VerifyResponse::Valid { .. } => Ok(()),
            v2::VerifyResponse::Invalid { reason, .. } => {
                Err(VerificationError::VerificationFailed(reason))
            }
        }
    }

    fn enrich_with_capabilities(&mut self, capabilities: &SupportedResponse) {
        self.enrich(capabilities);
    }
}

// ============================================================================
// Pre-process Result
// ============================================================================

/// Result of pre-processing a request for payment.
///
/// Contains the data needed to complete settlement after the inner service runs.
pub struct PreProcessResult {
    /// The verify/settle request built from the payment header
    pub verify_request: proto::VerifyRequest,
    /// If settle_before_execution, contains the settlement header value
    pub settlement_header: Option<HeaderValue>,
}

// ============================================================================
// Unified Paygate Implementation
// ============================================================================

/// Unified payment gate that works with both V1 and V2 protocols.
///
/// For actix-web, the paygate exposes `pre_process` and `post_process` methods
/// that the middleware calls around the inner service invocation.
pub struct Paygate<TPriceTag, TFacilitator> {
    /// The facilitator for verifying and settling payments
    pub facilitator: TFacilitator,
    /// Whether to settle before or after request execution
    pub settle_before_execution: bool,
    /// Accepted payment requirements
    pub accepts: Arc<Vec<TPriceTag>>,
    /// Resource information for the protected endpoint
    pub resource: v2::ResourceInfo,
}

impl<TPriceTag, TFacilitator> Paygate<TPriceTag, TFacilitator>
where
    TPriceTag: PaygateProtocol,
    TFacilitator: Facilitator,
{
    /// Pre-process: extract payment header, verify, and optionally settle.
    ///
    /// Returns `Ok(PreProcessResult)` on success, `Err(PaygateError)` on failure.
    /// The middleware should call this before invoking the inner service.
    #[cfg_attr(
        feature = "telemetry",
        instrument(name = "x402.pre_process", skip_all)
    )]
    pub async fn pre_process(
        &self,
        headers: &actix_web::http::header::HeaderMap,
    ) -> Result<PreProcessResult, PaygateError> {
        // Extract payment payload from headers
        let header = extract_payment_header(headers, TPriceTag::PAYMENT_HEADER_NAME)
            .ok_or(VerificationError::PaymentHeaderRequired(
                TPriceTag::PAYMENT_HEADER_NAME,
            ))?;
        let payment_payload = extract_payment_payload::<TPriceTag::PaymentPayload>(header)
            .ok_or(VerificationError::InvalidPaymentHeader)?;

        let verify_request =
            TPriceTag::make_verify_request(payment_payload, &self.accepts, &self.resource)?;

        if self.settle_before_execution {
            #[cfg(feature = "telemetry")]
            tracing::debug!("Settling payment before request execution");

            let settlement = self.settle_payment(&verify_request).await?;
            let header_value = settlement_to_header(settlement)?;

            Ok(PreProcessResult {
                verify_request,
                settlement_header: Some(header_value),
            })
        } else {
            #[cfg(feature = "telemetry")]
            tracing::debug!("Verifying payment before request execution");

            let verify_response = self.verify_payment(&verify_request).await?;
            TPriceTag::validate_verify_response(verify_response)?;

            Ok(PreProcessResult {
                verify_request,
                settlement_header: None,
            })
        }
    }

    /// Post-process: settle payment after the inner service has responded.
    ///
    /// Only needed when `settle_before_execution` is false.
    /// Returns the settlement header value to attach to the response.
    pub async fn post_process(
        &self,
        pre: &PreProcessResult,
    ) -> Result<HeaderValue, PaygateError> {
        if let Some(ref header) = pre.settlement_header {
            // Already settled before execution
            return Ok(header.clone());
        }

        // Settle after execution
        let settlement = self.settle_payment(&pre.verify_request).await?;
        settlement_to_header(settlement)
    }

    /// Builds an error response for a PaygateError.
    pub async fn error_response(&self, err: PaygateError) -> HttpResponse {
        let enriched_accepts = self.get_enriched_accepts().await;
        TPriceTag::error_into_response(err, &enriched_accepts, &self.resource)
    }

    /// Gets enriched price tags with facilitator capabilities.
    async fn get_enriched_accepts(&self) -> Vec<TPriceTag> {
        let capabilities = self.facilitator.supported().await.unwrap_or_default();

        self.accepts
            .iter()
            .map(|pt| {
                let mut pt_clone = pt.clone();
                pt_clone.enrich_with_capabilities(&capabilities);
                pt_clone
            })
            .collect()
    }

    /// Verifies a payment with the facilitator.
    pub async fn verify_payment(
        &self,
        verify_request: &proto::VerifyRequest,
    ) -> Result<proto::VerifyResponse, VerificationError> {
        let verify_response = self
            .facilitator
            .verify(verify_request)
            .await
            .map_err(|e| VerificationError::VerificationFailed(format!("{e}")))?;
        Ok(verify_response)
    }

    /// Settles a payment with the facilitator.
    pub async fn settle_payment(
        &self,
        settle_request: &proto::SettleRequest,
    ) -> Result<proto::SettleResponse, PaygateError> {
        let settle_response = self
            .facilitator
            .settle(settle_request)
            .await
            .map_err(|e| PaygateError::Settlement(format!("{e}")))?;
        Ok(settle_response)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extracts the payment header value from the actix header map.
fn extract_payment_header<'a>(
    header_map: &'a actix_web::http::header::HeaderMap,
    header_name: &'a str,
) -> Option<&'a [u8]> {
    header_map.get(header_name).map(|h| h.as_bytes())
}

/// Extracts and deserializes the payment payload from base64-encoded header bytes.
fn extract_payment_payload<T>(header_bytes: &[u8]) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let base64 = Base64Bytes::from(header_bytes).decode().ok()?;
    let value = serde_json::from_slice(base64.as_ref()).ok()?;
    Some(value)
}

/// Converts a [`proto::SettleResponse`] into an HTTP header value.
fn settlement_to_header(settlement: proto::SettleResponse) -> Result<HeaderValue, PaygateError> {
    let json =
        serde_json::to_vec(&settlement).map_err(|err| PaygateError::Settlement(err.to_string()))?;
    let payment_header = Base64Bytes::encode(json);
    HeaderValue::from_bytes(payment_header.as_ref())
        .map_err(|err| PaygateError::Settlement(err.to_string()))
}

// ============================================================================
// PriceTagSource Trait and Implementations
// ============================================================================

/// Trait for types that can provide price tags for a request.
///
/// This trait abstracts over static and dynamic pricing strategies.
/// Uses actix-web's HeaderMap type for request headers.
pub trait PriceTagSource {
    /// The concrete price tag type produced by this source.
    type PriceTag: PaygateProtocol;

    /// Resolves price tags for the given request context.
    fn resolve(
        &self,
        headers: &actix_web::http::header::HeaderMap,
        uri: &Uri,
        base_url: Option<&Url>,
    ) -> impl Future<Output = Vec<Self::PriceTag>> + Send;
}

// ============================================================================
// StaticPriceTags Implementation
// ============================================================================

/// Static price tag source - returns the same price tags for every request.
#[derive(Clone, Debug)]
pub struct StaticPriceTags<TPriceTag> {
    tags: Arc<Vec<TPriceTag>>,
}

impl<TPriceTag> StaticPriceTags<TPriceTag> {
    /// Creates a new static price tag source from a vector of price tags.
    pub fn new(tags: Vec<TPriceTag>) -> Self {
        Self {
            tags: Arc::new(tags),
        }
    }

    /// Returns a reference to the stored price tags.
    pub fn tags(&self) -> &[TPriceTag] {
        &self.tags
    }
}

impl<TPriceTag> StaticPriceTags<TPriceTag>
where
    TPriceTag: Clone,
{
    /// Adds a price tag to the source.
    pub fn with_price_tag(mut self, tag: TPriceTag) -> Self {
        let mut tags = (*self.tags).clone();
        tags.push(tag);
        self.tags = Arc::new(tags);
        self
    }
}

impl<TPriceTag> PriceTagSource for StaticPriceTags<TPriceTag>
where
    TPriceTag: PaygateProtocol,
{
    type PriceTag = TPriceTag;

    async fn resolve(
        &self,
        _headers: &actix_web::http::header::HeaderMap,
        _uri: &Uri,
        _base_url: Option<&Url>,
    ) -> Vec<Self::PriceTag> {
        (*self.tags).clone()
    }
}

// ============================================================================
// DynamicPriceTags Implementation
// ============================================================================

/// Internal type alias for the boxed dynamic pricing callback.
type BoxedDynamicPriceCallback<TPriceTag> = dyn for<'a> Fn(
        &'a actix_web::http::header::HeaderMap,
        &'a Uri,
        Option<&'a Url>,
    ) -> Pin<Box<dyn Future<Output = Vec<TPriceTag>> + Send + 'a>>
    + Send
    + Sync;

/// Dynamic price tag source - computes price tags per-request via callback.
pub struct DynamicPriceTags<TPriceTag> {
    callback: Arc<BoxedDynamicPriceCallback<TPriceTag>>,
}

impl<TPriceTag> Clone for DynamicPriceTags<TPriceTag> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
        }
    }
}

impl<TPriceTag> std::fmt::Debug for DynamicPriceTags<TPriceTag> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicPriceTags")
            .field("callback", &"<callback>")
            .finish()
    }
}

impl<TPriceTag> DynamicPriceTags<TPriceTag> {
    /// Creates a new dynamic price source from an async closure.
    pub fn new<F, Fut>(callback: F) -> Self
    where
        F: Fn(&actix_web::http::header::HeaderMap, &Uri, Option<&Url>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Vec<TPriceTag>> + Send + 'static,
    {
        Self {
            callback: Arc::new(move |headers, uri, base_url| {
                Box::pin(callback(headers, uri, base_url))
            }),
        }
    }
}

impl<TPriceTag> PriceTagSource for DynamicPriceTags<TPriceTag>
where
    TPriceTag: PaygateProtocol,
{
    type PriceTag = TPriceTag;

    async fn resolve(
        &self,
        headers: &actix_web::http::header::HeaderMap,
        uri: &Uri,
        base_url: Option<&Url>,
    ) -> Vec<Self::PriceTag> {
        (self.callback)(headers, uri, base_url).await
    }
}
