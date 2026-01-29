use crate::config::Config;
use crate::models::{
    PaymentPayload, PaymentRequired, PaymentResponse, VerifyPaymentRequirements,
    usdc_address, BASE_NETWORK,
};
use crate::services::{FacilitatorClient, NonceTracker};
use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error, HttpResponse,
};
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

const X_PAYMENT_HEADER: &str = "X-PAYMENT";
const PAYMENT_REQUIRED_HEADER: &str = "PAYMENT-REQUIRED";

pub struct X402Middleware {
    config: Config,
    facilitator: FacilitatorClient,
    nonce_tracker: Arc<NonceTracker>,
}

impl X402Middleware {
    pub fn new(config: Config, facilitator: FacilitatorClient, nonce_tracker: Arc<NonceTracker>) -> Self {
        X402Middleware {
            config,
            facilitator,
            nonce_tracker,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for X402Middleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = X402MiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(X402MiddlewareService {
            service: Arc::new(service),
            config: self.config.clone(),
            facilitator: self.facilitator.clone(),
            nonce_tracker: self.nonce_tracker.clone(),
        }))
    }
}

pub struct X402MiddlewareService<S> {
    service: Arc<S>,
    config: Config,
    facilitator: FacilitatorClient,
    nonce_tracker: Arc<NonceTracker>,
}

impl<S, B> Service<ServiceRequest> for X402MiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Arc::clone(&self.service);
        let config = self.config.clone();
        let facilitator = self.facilitator.clone();
        let nonce_tracker = self.nonce_tracker.clone();

        Box::pin(async move {
            // Check for X-PAYMENT header
            let payment_header = req
                .headers()
                .get(X_PAYMENT_HEADER)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            match payment_header {
                None => {
                    // No payment header - return 402 Payment Required
                    info!("No payment header, returning 402");
                    let payment_required = PaymentRequired::new(
                        config.bot_wallet_address,
                        config.cost_per_request,
                        req.path(),
                    );

                    let encoded = match payment_required.to_base64() {
                        Ok(e) => e,
                        Err(e) => {
                            error!("Failed to encode payment required: {}", e);
                            let response = HttpResponse::InternalServerError()
                                .body("Failed to generate payment requirements");
                            return Ok(req
                                .into_response(response)
                                .map_into_right_body());
                        }
                    };

                    let response = HttpResponse::PaymentRequired()
                        .insert_header((
                            HeaderName::from_static("payment-required"),
                            HeaderValue::from_str(&encoded).unwrap_or_else(|_| {
                                HeaderValue::from_static("")
                            }),
                        ))
                        .body(format!(
                            "Payment required. See {} header for details.",
                            PAYMENT_REQUIRED_HEADER
                        ));

                    Ok(req.into_response(response).map_into_right_body())
                }
                Some(payment_header_value) => {
                    debug!("Payment header present, verifying...");

                    // Decode and parse payment payload
                    let payment_payload = match PaymentPayload::from_base64(&payment_header_value) {
                        Ok(p) => p,
                        Err(e) => {
                            warn!("Invalid payment payload: {}", e);
                            let payment_response = PaymentResponse::failure(&e.to_string());
                            let encoded = payment_response.to_base64().unwrap_or_default();

                            let response = HttpResponse::PaymentRequired()
                                .insert_header((
                                    HeaderName::from_static("payment-response"),
                                    HeaderValue::from_str(&encoded)
                                        .unwrap_or_else(|_| HeaderValue::from_static("")),
                                ))
                                .body(format!("Invalid payment: {}", e));

                            return Ok(req.into_response(response).map_into_right_body());
                        }
                    };

                    // Extract nonce and check for replay attack
                    let nonce_hex = payment_payload.payload.authorization.nonce.to_hex();
                    if !nonce_tracker.try_use_nonce(&nonce_hex) {
                        warn!("Replay attack detected! Nonce already used: {}", nonce_hex);
                        let payment_response = PaymentResponse::failure("Payment nonce already used (replay attack prevented)");
                        let encoded = payment_response.to_base64().unwrap_or_default();

                        let response = HttpResponse::PaymentRequired()
                            .insert_header((
                                HeaderName::from_static("payment-response"),
                                HeaderValue::from_str(&encoded)
                                    .unwrap_or_else(|_| HeaderValue::from_static("")),
                            ))
                            .body("Payment rejected: nonce already used");

                        return Ok(req.into_response(response).map_into_right_body());
                    }

                    // Create payment requirements for verification (x402 v2 format)
                    let payment_requirements = VerifyPaymentRequirements {
                        scheme: "exact".to_string(),
                        network: BASE_NETWORK.to_string(),
                        amount: config.cost_per_request.to_string(),
                        pay_to: config.bot_wallet_address,
                        asset: usdc_address(),
                        max_timeout_seconds: 60,
                        extra: None,
                    };

                    // Clone payment data for use in both verify and settle
                    let payment_payload_for_settle = payment_payload.clone();
                    let payment_requirements_for_settle = payment_requirements.clone();

                    // Step 1: Verify with facilitator
                    let verify_result = facilitator
                        .verify(payment_payload, payment_requirements)
                        .await;

                    match verify_result {
                        Ok(verify_response) if verify_response.is_valid => {
                            info!("Payment verified, attempting settlement before processing request");

                            // Step 2: Settle BEFORE processing the request
                            // This ensures we collect payment before delivering the service
                            let settle_result = facilitator
                                .settle(payment_payload_for_settle, payment_requirements_for_settle)
                                .await;

                            match settle_result {
                                Ok(settle_response) if settle_response.success => {
                                    info!(
                                        "Payment settled successfully. Tx: {:?}. Proceeding with request.",
                                        settle_response.transaction
                                    );

                                    // Step 3: Payment collected - now process the request
                                    let res = service.call(req).await?;

                                    // Add payment response header to successful response
                                    let payment_response = PaymentResponse::success();
                                    let encoded = payment_response.to_base64().unwrap_or_default();

                                    let (req, response) = res.into_parts();
                                    let mut response = response.map_into_left_body();

                                    if let Ok(header_value) = HeaderValue::from_str(&encoded) {
                                        response.headers_mut().insert(
                                            HeaderName::from_static("payment-response"),
                                            header_value,
                                        );
                                    }

                                    Ok(ServiceResponse::new(req, response))
                                }
                                Ok(settle_response) => {
                                    // Settlement failed - do NOT process the request
                                    let error_msg = settle_response
                                        .error_reason
                                        .unwrap_or_else(|| "Settlement failed".to_string());
                                    error!("Payment settlement failed: {}. Request rejected.", error_msg);

                                    let payment_response = PaymentResponse::failure(&format!(
                                        "Payment settlement failed: {}",
                                        error_msg
                                    ));
                                    let encoded = payment_response.to_base64().unwrap_or_default();

                                    let response = HttpResponse::PaymentRequired()
                                        .insert_header((
                                            HeaderName::from_static("payment-response"),
                                            HeaderValue::from_str(&encoded)
                                                .unwrap_or_else(|_| HeaderValue::from_static("")),
                                        ))
                                        .body(format!("Payment settlement failed: {}", error_msg));

                                    Ok(req.into_response(response).map_into_right_body())
                                }
                                Err(e) => {
                                    // Settlement error - do NOT process the request
                                    error!("Settlement error: {}. Request rejected.", e);

                                    let payment_response = PaymentResponse::failure(&format!(
                                        "Settlement error: {}",
                                        e
                                    ));
                                    let encoded = payment_response.to_base64().unwrap_or_default();

                                    let response = HttpResponse::PaymentRequired()
                                        .insert_header((
                                            HeaderName::from_static("payment-response"),
                                            HeaderValue::from_str(&encoded)
                                                .unwrap_or_else(|_| HeaderValue::from_static("")),
                                        ))
                                        .body(format!("Settlement error: {}", e));

                                    Ok(req.into_response(response).map_into_right_body())
                                }
                            }
                        }
                        Ok(verify_response) => {
                            // Payment invalid
                            let error_msg = verify_response
                                .invalid_reason
                                .unwrap_or_else(|| "Payment verification failed".to_string());
                            warn!("Payment verification failed: {}", error_msg);

                            let payment_response = PaymentResponse::failure(&error_msg);
                            let encoded = payment_response.to_base64().unwrap_or_default();

                            let response = HttpResponse::PaymentRequired()
                                .insert_header((
                                    HeaderName::from_static("payment-response"),
                                    HeaderValue::from_str(&encoded)
                                        .unwrap_or_else(|_| HeaderValue::from_static("")),
                                ))
                                .body(format!("Payment verification failed: {}", error_msg));

                            Ok(req.into_response(response).map_into_right_body())
                        }
                        Err(e) => {
                            error!("Facilitator error: {}", e);
                            let payment_response =
                                PaymentResponse::failure("Facilitator unavailable");
                            let encoded = payment_response.to_base64().unwrap_or_default();

                            let response = HttpResponse::BadGateway()
                                .insert_header((
                                    HeaderName::from_static("payment-response"),
                                    HeaderValue::from_str(&encoded)
                                        .unwrap_or_else(|_| HeaderValue::from_static("")),
                                ))
                                .body(format!("Facilitator error: {}", e));

                            Ok(req.into_response(response).map_into_right_body())
                        }
                    }
                }
            }
        })
    }
}
