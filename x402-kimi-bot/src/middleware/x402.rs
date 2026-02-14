use crate::config::Config;
use crate::models::{
    PaymentPayload, PaymentRequired, PaymentResponse, VerifyPaymentRequirements,
    usdc_address, BASE_NETWORK,
};
use crate::services::{FacilitatorClient, NonceTracker, PendingSettlement, RateLimiter, SettlementQueue, VerificationCache};
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
    settlement_queue: Arc<SettlementQueue>,
    rate_limiter: Arc<RateLimiter>,
    verification_cache: Arc<VerificationCache>,
}

impl X402Middleware {
    pub fn new(
        config: Config,
        facilitator: FacilitatorClient,
        nonce_tracker: Arc<NonceTracker>,
        settlement_queue: Arc<SettlementQueue>,
        rate_limiter: Arc<RateLimiter>,
        verification_cache: Arc<VerificationCache>,
    ) -> Self {
        X402Middleware {
            config,
            facilitator,
            nonce_tracker,
            settlement_queue,
            rate_limiter,
            verification_cache,
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
            settlement_queue: self.settlement_queue.clone(),
            rate_limiter: self.rate_limiter.clone(),
            verification_cache: self.verification_cache.clone(),
        }))
    }
}

pub struct X402MiddlewareService<S> {
    service: Arc<S>,
    config: Config,
    facilitator: FacilitatorClient,
    nonce_tracker: Arc<NonceTracker>,
    settlement_queue: Arc<SettlementQueue>,
    rate_limiter: Arc<RateLimiter>,
    verification_cache: Arc<VerificationCache>,
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
        let settlement_queue = self.settlement_queue.clone();
        let rate_limiter = self.rate_limiter.clone();
        let verification_cache = self.verification_cache.clone();

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

                    // Check rate limit based on payer address
                    let payer_address = payment_payload.payload.authorization.from.to_hex();
                    if !rate_limiter.check_rate_limit(&payer_address) {
                        warn!("Rate limit exceeded for address: {}", payer_address);
                        let response = HttpResponse::TooManyRequests()
                            .insert_header(("Retry-After", "1"))
                            .body("Rate limit exceeded: maximum 5 requests per second per address");

                        return Ok(req.into_response(response).map_into_right_body());
                    }

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

                    // Create payment requirements for verification / settlement
                    let payment_requirements = VerifyPaymentRequirements {
                        scheme: "exact".to_string(),
                        network: BASE_NETWORK.to_string(),
                        amount: config.cost_per_request.to_string(),
                        pay_to: config.bot_wallet_address,
                        asset: usdc_address(),
                        max_timeout_seconds: 60,
                        extra: None,
                    };

                    // ── Path A: Cache hit — trusted payer, skip verify ──
                    if verification_cache.is_verified(&payer_address) {
                        debug!("Cache hit for payer {}, skipping verify", payer_address);

                        let pending_settlement = PendingSettlement::new(
                            payment_payload,
                            payment_requirements,
                            nonce_hex.clone(),
                        );

                        if let Err(_rejected) = settlement_queue.push(pending_settlement).await {
                            error!(
                                "Settlement queue full, rejecting request. Queue size: {}",
                                settlement_queue.len()
                            );
                            let response = HttpResponse::ServiceUnavailable()
                                .body("Service temporarily unavailable: settlement queue full. Please retry later.");
                            return Ok(req.into_response(response).map_into_right_body());
                        }

                        let res = service.call(req).await?;

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

                        return Ok(ServiceResponse::new(req, response));
                    }

                    // Pre-check queue capacity before doing any expensive work
                    if settlement_queue.is_full() {
                        error!(
                            "Settlement queue full, rejecting request early. Queue size: {}",
                            settlement_queue.len()
                        );
                        let response = HttpResponse::ServiceUnavailable()
                            .body("Service temporarily unavailable: settlement queue full. Please retry later.");
                        return Ok(req.into_response(response).map_into_right_body());
                    }

                    // ── Path B: Recent failure — sequential verify-then-serve ──
                    // Addresses that previously failed verification are not given
                    // the parallel path; verify must pass before Kimi is called.
                    if verification_cache.has_recent_failure(&payer_address) {
                        debug!("Downgrading payer {} to sequential path (recent failure)", payer_address);

                        let payload_for_settlement = payment_payload.clone();
                        let requirements_for_settlement = payment_requirements.clone();

                        let verify_result = facilitator
                            .verify(payment_payload, payment_requirements)
                            .await;

                        match verify_result {
                            Ok(verify_response) if verify_response.is_valid => {
                                info!("Payment verified (sequential) for payer: {:?}", verify_response.payer);
                                verification_cache.mark_verified(&payer_address);

                                let pending_settlement = PendingSettlement::new(
                                    payload_for_settlement,
                                    requirements_for_settlement,
                                    nonce_hex.clone(),
                                );

                                if let Err(_rejected) = settlement_queue.push(pending_settlement).await {
                                    warn!("Settlement not queued for nonce {} due to full queue", nonce_hex);
                                }

                                let res = service.call(req).await?;

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

                                return Ok(ServiceResponse::new(req, response));
                            }
                            Ok(verify_response) => {
                                let error_msg = verify_response
                                    .invalid_reason
                                    .unwrap_or_else(|| "Payment verification failed".to_string());
                                warn!("Payment verification failed (sequential): {}", error_msg);
                                verification_cache.record_failure(&payer_address);

                                let payment_response = PaymentResponse::failure(&error_msg);
                                let encoded = payment_response.to_base64().unwrap_or_default();

                                let response = HttpResponse::PaymentRequired()
                                    .insert_header((
                                        HeaderName::from_static("payment-response"),
                                        HeaderValue::from_str(&encoded)
                                            .unwrap_or_else(|_| HeaderValue::from_static("")),
                                    ))
                                    .body(format!("Payment verification failed: {}", error_msg));

                                return Ok(req.into_response(response).map_into_right_body());
                            }
                            Err(e) => {
                                error!("Facilitator error (sequential): {}", e);
                                // Don't record failure for facilitator errors —
                                // it's not the payer's fault.
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

                                return Ok(req.into_response(response).map_into_right_body());
                            }
                        }
                    }

                    // ── Path C: Clean first-timer — parallel verify + Kimi ──
                    debug!("Cache miss for payer {}, verifying + calling service in parallel", payer_address);

                    let payload_for_settlement = payment_payload.clone();
                    let requirements_for_settlement = payment_requirements.clone();

                    let verify_fut = facilitator.verify(payment_payload, payment_requirements);
                    let service_fut = service.call(req);

                    let (verify_result, service_result) = tokio::join!(verify_fut, service_fut);

                    match verify_result {
                        Ok(verify_response) if verify_response.is_valid => {
                            info!("Payment verified (parallel) for payer: {:?}", verify_response.payer);
                            verification_cache.mark_verified(&payer_address);

                            let pending_settlement = PendingSettlement::new(
                                payload_for_settlement,
                                requirements_for_settlement,
                                nonce_hex.clone(),
                            );

                            if let Err(_rejected) = settlement_queue.push(pending_settlement).await {
                                warn!("Settlement not queued for nonce {} due to full queue", nonce_hex);
                            }

                            let res = service_result?;
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
                        Ok(verify_response) => {
                            let error_msg = verify_response
                                .invalid_reason
                                .unwrap_or_else(|| "Payment verification failed".to_string());
                            warn!("Payment verification failed (parallel): {}", error_msg);
                            verification_cache.record_failure(&payer_address);

                            let payment_response = PaymentResponse::failure(&error_msg);
                            let encoded = payment_response.to_base64().unwrap_or_default();

                            let response = HttpResponse::PaymentRequired()
                                .insert_header((
                                    HeaderName::from_static("payment-response"),
                                    HeaderValue::from_str(&encoded)
                                        .unwrap_or_else(|_| HeaderValue::from_static("")),
                                ))
                                .body(format!("Payment verification failed: {}", error_msg));

                            match service_result {
                                Ok(res) => {
                                    let (req, _) = res.into_parts();
                                    Ok(ServiceResponse::new(req, response.map_into_right_body()))
                                }
                                Err(_) => {
                                    Err(actix_web::error::ErrorPaymentRequired(error_msg))
                                }
                            }
                        }
                        Err(e) => {
                            error!("Facilitator error (parallel): {}", e);
                            // Don't record failure for facilitator errors
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

                            match service_result {
                                Ok(res) => {
                                    let (req, _) = res.into_parts();
                                    Ok(ServiceResponse::new(req, response.map_into_right_body()))
                                }
                                Err(_) => {
                                    Err(actix_web::error::ErrorBadGateway(format!("Facilitator error: {}", e)))
                                }
                            }
                        }
                    }
                }
            }
        })
    }
}
