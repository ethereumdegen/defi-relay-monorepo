//! Actix-web middleware for enforcing [x402](https://www.x402.org) payments on protected routes.
//!
//! This middleware validates incoming payment headers using a configured x402 facilitator,
//! and settles valid payments either before or after request execution (configurable).
//!
//! Returns a `402 Payment Required` response if the request lacks a valid payment.
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use alloy_primitives::address;
//! use actix_web::{web, App, HttpServer, HttpResponse};
//! use x402_actix::X402Middleware;
//! use x402_rs::networks::{KnownNetworkEip155, USDC};
//! use x402_rs::scheme::v1_eip155_exact::V1Eip155Exact;
//!
//! let x402 = X402Middleware::new("https://facilitator.x402.rs");
//!
//! App::new().service(
//!     web::resource("/protected")
//!         .wrap(x402.with_price_tag(V1Eip155Exact::price_tag(
//!             address!("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
//!             USDC::base_sepolia().parse("0.01").unwrap(),
//!         )))
//!         .route(web::get().to(|| async { HttpResponse::Ok().body("VIP content") }))
//! );
//! ```

use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use http::Uri;
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use url::Url;
use x402_rs::facilitator::Facilitator;

use crate::facilitator_client::FacilitatorClient;
use crate::paygate::{
    DynamicPriceTags, Paygate, PaygateProtocol, PriceTagSource, ResourceInfoBuilder,
    StaticPriceTags,
};

/// The main X402 middleware instance for enforcing x402 payments on routes.
///
/// Create a single instance per application and use it to build payment wrappers
/// for protected routes.
#[derive(Clone, Debug)]
pub struct X402Middleware<F> {
    facilitator: F,
    base_url: Option<Url>,
    settle_before_execution: bool,
}

impl<F> X402Middleware<F> {
    pub fn facilitator(&self) -> &F {
        &self.facilitator
    }
}

impl X402Middleware<Arc<FacilitatorClient>> {
    /// Creates a new middleware instance with a default facilitator URL.
    ///
    /// # Panics
    ///
    /// Panics if the facilitator URL is invalid.
    pub fn new(url: &str) -> Self {
        let facilitator = FacilitatorClient::try_from(url).expect("Invalid facilitator URL");
        Self {
            facilitator: Arc::new(facilitator),
            base_url: None,
            settle_before_execution: false,
        }
    }

    /// Creates a new middleware instance with a facilitator URL.
    pub fn try_new(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let facilitator = FacilitatorClient::try_from(url)?;
        Ok(Self {
            facilitator: Arc::new(facilitator),
            base_url: None,
            settle_before_execution: false,
        })
    }

    /// Returns the configured facilitator URL.
    pub fn facilitator_url(&self) -> &Url {
        self.facilitator.base_url()
    }

    /// Sets the TTL for caching the facilitator's supported response.
    pub fn with_supported_cache_ttl(&self, ttl: Duration) -> Self {
        let facilitator = Arc::new(self.facilitator.with_supported_cache_ttl(ttl));
        Self {
            facilitator,
            base_url: self.base_url.clone(),
            settle_before_execution: self.settle_before_execution,
        }
    }
}

impl TryFrom<&str> for X402Middleware<Arc<FacilitatorClient>> {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<String> for X402Middleware<Arc<FacilitatorClient>> {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(&value)
    }
}

impl<F> X402Middleware<F>
where
    F: Clone,
{
    /// Sets the base URL used to construct resource URLs dynamically.
    pub fn with_base_url(&self, base_url: Url) -> X402Middleware<F> {
        let mut this = self.clone();
        this.base_url = Some(base_url);
        this
    }

    /// Enables settlement prior to request execution.
    pub fn settle_before_execution(&self) -> X402Middleware<F> {
        let mut this = self.clone();
        this.settle_before_execution = true;
        this
    }

    /// Disables settlement prior to request execution (default behavior).
    pub fn settle_after_execution(&self) -> Self {
        let mut this = self.clone();
        this.settle_before_execution = false;
        this
    }
}

impl<TFacilitator> X402Middleware<TFacilitator>
where
    TFacilitator: Clone,
{
    /// Sets the price tag for the protected route.
    pub fn with_price_tag<TPriceTag>(
        &self,
        price_tag: TPriceTag,
    ) -> X402LayerBuilder<StaticPriceTags<TPriceTag>, TFacilitator> {
        X402LayerBuilder {
            facilitator: self.facilitator.clone(),
            price_source: StaticPriceTags::new(vec![price_tag]),
            base_url: self.base_url.clone().map(Arc::new),
            resource: Arc::new(ResourceInfoBuilder::default()),
            settle_before_execution: self.settle_before_execution,
        }
    }

    /// Sets a dynamic price source for the protected route.
    pub fn with_dynamic_price<F, Fut, TPriceTag>(
        &self,
        callback: F,
    ) -> X402LayerBuilder<DynamicPriceTags<TPriceTag>, TFacilitator>
    where
        F: Fn(&actix_web::http::header::HeaderMap, &Uri, Option<&Url>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Vec<TPriceTag>> + Send + 'static,
    {
        X402LayerBuilder {
            facilitator: self.facilitator.clone(),
            price_source: DynamicPriceTags::new(callback),
            base_url: self.base_url.clone().map(Arc::new),
            resource: Arc::new(ResourceInfoBuilder::default()),
            settle_before_execution: self.settle_before_execution,
        }
    }
}

/// Builder for configuring the X402 middleware layer.
///
/// Generic over `TSource` which implements [`PriceTagSource`] to support
/// both static and dynamic pricing strategies.
#[derive(Clone)]
pub struct X402LayerBuilder<TSource, TFacilitator> {
    facilitator: TFacilitator,
    settle_before_execution: bool,
    base_url: Option<Arc<Url>>,
    price_source: TSource,
    resource: Arc<ResourceInfoBuilder>,
}

impl<TPriceTag, TFacilitator> X402LayerBuilder<StaticPriceTags<TPriceTag>, TFacilitator>
where
    TPriceTag: Clone,
{
    /// Adds another payment option.
    pub fn with_price_tag(mut self, price_tag: TPriceTag) -> Self {
        self.price_source = self.price_source.with_price_tag(price_tag);
        self
    }
}

impl<TSource, TFacilitator> X402LayerBuilder<TSource, TFacilitator> {
    /// Sets a description of what the payment grants access to.
    pub fn with_description(mut self, description: String) -> Self {
        let mut new_resource = (*self.resource).clone();
        new_resource.description = description;
        self.resource = Arc::new(new_resource);
        self
    }

    /// Sets the MIME type of the protected resource.
    pub fn with_mime_type(mut self, mime: String) -> Self {
        let mut new_resource = (*self.resource).clone();
        new_resource.mime_type = mime;
        self.resource = Arc::new(new_resource);
        self
    }

    /// Sets the full URL of the protected resource.
    pub fn with_resource(mut self, resource: Url) -> Self {
        let mut new_resource = (*self.resource).clone();
        new_resource.url = Some(resource.to_string());
        self.resource = Arc::new(new_resource);
        self
    }
}

// ============================================================================
// Actix-web Transform implementation
// ============================================================================

impl<S, B, TSource, TFacilitator> Transform<S, ServiceRequest>
    for X402LayerBuilder<TSource, TFacilitator>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
    TFacilitator: Facilitator + Clone + 'static,
    TSource: PriceTagSource + Clone + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = X402MiddlewareService<S, TSource, TFacilitator>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(X402MiddlewareService {
            service: Arc::new(service),
            facilitator: self.facilitator.clone(),
            settle_before_execution: self.settle_before_execution,
            base_url: self.base_url.clone(),
            price_source: self.price_source.clone(),
            resource: self.resource.clone(),
        }))
    }
}

// ============================================================================
// Actix-web Service implementation
// ============================================================================

/// Actix-web service that enforces x402 payments on incoming requests.
pub struct X402MiddlewareService<S, TSource, TFacilitator> {
    service: Arc<S>,
    facilitator: TFacilitator,
    base_url: Option<Arc<Url>>,
    settle_before_execution: bool,
    price_source: TSource,
    resource: Arc<ResourceInfoBuilder>,
}

impl<S, B, TSource, TFacilitator> Service<ServiceRequest>
    for X402MiddlewareService<S, TSource, TFacilitator>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
    TSource: PriceTagSource + Clone + 'static,
    TSource::PriceTag: PaygateProtocol,
    TFacilitator: Facilitator + Clone + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Arc::clone(&self.service);
        let price_source = self.price_source.clone();
        let facilitator = self.facilitator.clone();
        let base_url = self.base_url.clone();
        let resource_builder = self.resource.clone();
        let settle_before_execution = self.settle_before_execution;

        Box::pin(async move {
            let headers = req.headers();

            // Parse URI from the request path + query
            let uri: Uri = req
                .uri()
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or(req.uri().path())
                .parse()
                .unwrap_or_else(|_| Uri::from_static("/"));

            // Resolve price tags from the source
            let accepts = price_source
                .resolve(headers, &uri, base_url.as_deref())
                .await;

            // If no price tags are configured, bypass payment enforcement
            if accepts.is_empty() {
                let res = service.call(req).await?;
                return Ok(res.map_into_left_body());
            }

            let resource = resource_builder.as_resource_info(base_url.as_deref(), headers, &uri);

            let gate = Paygate {
                facilitator,
                settle_before_execution,
                accepts: Arc::new(accepts),
                resource,
            };

            // Pre-process: verify/settle payment
            let pre = match gate.pre_process(headers).await {
                Ok(pre) => pre,
                Err(err) => {
                    let response = gate.error_response(err).await;
                    return Ok(req.into_response(response).map_into_right_body());
                }
            };

            // Call the inner service
            let res = service.call(req).await?;

            // If settle_before_execution was true, we already have the header
            if let Some(ref header_value) = pre.settlement_header {
                let (req, response) = res.into_parts();
                let response = response.map_into_left_body();
                let mut srv_res = ServiceResponse::new(req, response);
                srv_res.headers_mut().insert(
                    actix_web::http::header::HeaderName::from_static("x-payment-response"),
                    actix_web::http::header::HeaderValue::from_bytes(header_value.as_bytes())
                        .unwrap_or_else(|_| {
                            actix_web::http::header::HeaderValue::from_static("")
                        }),
                );
                return Ok(srv_res);
            }

            // Check if the inner service returned an error status
            let status = res.status();
            if status.is_client_error() || status.is_server_error() {
                return Ok(res.map_into_left_body());
            }

            // Post-process: settle after execution
            match gate.post_process(&pre).await {
                Ok(header_value) => {
                    let (req, response) = res.into_parts();
                    let response = response.map_into_left_body();
                    let mut srv_res = ServiceResponse::new(req, response);
                    srv_res.headers_mut().insert(
                        actix_web::http::header::HeaderName::from_static("x-payment-response"),
                        actix_web::http::header::HeaderValue::from_bytes(header_value.as_bytes())
                            .unwrap_or_else(|_| {
                                actix_web::http::header::HeaderValue::from_static("")
                            }),
                    );
                    Ok(srv_res)
                }
                Err(err) => {
                    // We need to get the original request from the response to build error response
                    let (req, _response) = res.into_parts();
                    let error_response = gate.error_response(err).await;
                    Ok(ServiceResponse::new(req, error_response).map_into_right_body())
                }
            }
        })
    }
}
