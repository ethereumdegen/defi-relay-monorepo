//! x402 Payment Middleware for keystore-server
//!
//! Provides payment verification and settlement using the x402 protocol.
//! Uses pay2.defirelay.com as the facilitator.

use actix_web::http::{header::HeaderMap, StatusCode};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::config::X402Config;
use crate::error::AppError;
use crate::models::x402::{
    PaymentRequiredResponse, PaymentRequirements, SettleResponse, VerifyRequest, VerifyResponse,
};

/// Build payment requirements from config
fn build_payment_requirements(
    config: &X402Config,
    resource: &str,
    description: &str,
) -> PaymentRequirements {
    PaymentRequirements {
        scheme: "permit".to_string(),
        network: config.payment_network.clone(),
        max_amount_required: config.cost_per_backup.clone(),
        resource: resource.to_string(),
        description: description.to_string(),
        mime_type: "application/json".to_string(),
        pay_to: config.wallet_address.clone(),
        max_timeout_seconds: 300, // 5 minutes
        asset: config.payment_token_address.clone(),
        extra: Some(serde_json::json!({
            "token": config.payment_token_symbol,
            "address": config.payment_token_address,
            "decimals": config.payment_token_decimals,
            "name": config.payment_token_name,
            "version": config.payment_token_version,
            "facilitatorSigner": config.facilitator_signer,
            "minimum_amount": true
        })),
    }
}

/// Generate a 402 Payment Required response
pub fn payment_required_response(
    config: &X402Config,
    resource: &str,
    description: &str,
) -> AppError {
    let requirements = build_payment_requirements(config, resource, description);

    let response = PaymentRequiredResponse {
        x402_version: 1,
        accepts: vec![requirements],
        error: None,
    };

    let body = serde_json::to_string(&response).unwrap_or_default();

    AppError::PaymentRequired { body }
}

/// Decode payment header and build verify request
fn build_verify_request(
    payment_header: &str,
    payment_requirements: PaymentRequirements,
) -> Result<VerifyRequest, String> {
    let payload_bytes = BASE64
        .decode(payment_header)
        .map_err(|e| format!("Invalid payment header encoding: {}", e))?;

    let payment_payload: serde_json::Value = serde_json::from_slice(&payload_bytes)
        .map_err(|e| format!("Invalid payment payload JSON: {}", e))?;

    Ok(VerifyRequest {
        x402_version: 1,
        payment_payload,
        payment_requirements,
    })
}

/// Verify payment with facilitator
async fn verify_payment(
    http_client: &reqwest::Client,
    facilitator_url: &str,
    verify_request: &VerifyRequest,
) -> Result<VerifyResponse, String> {
    let verify_url = format!("{}/verify", facilitator_url);

    let response = http_client
        .post(&verify_url)
        .json(verify_request)
        .send()
        .await
        .map_err(|e| format!("Failed to contact facilitator: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Facilitator returned error: {} - {}", status, body));
    }

    response
        .json::<VerifyResponse>()
        .await
        .map_err(|e| format!("Failed to parse verify response: {}", e))
}

/// Settle payment with facilitator
async fn settle_payment(
    http_client: &reqwest::Client,
    facilitator_url: &str,
    settle_request: &VerifyRequest,
) -> Result<SettleResponse, String> {
    let settle_url = format!("{}/settle", facilitator_url);

    let response = http_client
        .post(&settle_url)
        .json(settle_request)
        .send()
        .await
        .map_err(|e| format!("Failed to contact facilitator for settlement: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Facilitator settlement error: {} - {}", status, body));
    }

    response
        .json::<SettleResponse>()
        .await
        .map_err(|e| format!("Failed to parse settle response: {}", e))
}

/// Error response for payment failures
fn payment_error_response(status: StatusCode, message: &str) -> AppError {
    match status {
        StatusCode::BAD_REQUEST => AppError::BadRequest(message.to_string()),
        StatusCode::PAYMENT_REQUIRED => AppError::PaymentError(message.to_string()),
        StatusCode::BAD_GATEWAY => AppError::BadGateway(message.to_string()),
        _ => AppError::Internal(message.to_string()),
    }
}

/// Require x402 payment - checks header, verifies, and settles synchronously
/// Returns Ok(Option<tx_hash>) on success, Err(AppError) on failure
pub async fn require_x402_payment(
    http_client: &reqwest::Client,
    x402_config: &X402Config,
    headers: &HeaderMap,
    resource: &str,
    description: &str,
) -> Result<Option<String>, AppError> {
    let payment_header = headers.get("X-PAYMENT").and_then(|v| v.to_str().ok());

    match payment_header {
        None => {
            // No payment header, return 402
            Err(payment_required_response(x402_config, resource, description))
        }
        Some(payment) => {
            // Build payment requirements (must match what we return in 402)
            let payment_requirements =
                build_payment_requirements(x402_config, resource, description);

            // Build verify request
            let verify_request = build_verify_request(payment, payment_requirements).map_err(|e| {
                tracing::error!("Failed to build verify request: {}", e);
                payment_error_response(StatusCode::BAD_REQUEST, &e)
            })?;

            // Verify payment
            match verify_payment(http_client, &x402_config.facilitator_url, &verify_request).await {
                Ok(verify_response) => {
                    if verify_response.is_valid {
                        // Settle synchronously
                        match settle_payment(
                            http_client,
                            &x402_config.facilitator_url,
                            &verify_request,
                        )
                        .await
                        {
                            Ok(settle_response) => {
                                if settle_response.success {
                                    tracing::info!(
                                        "Payment settled: {:?} from {:?}",
                                        settle_response.transaction,
                                        settle_response.payer
                                    );
                                    Ok(settle_response.transaction)
                                } else {
                                    tracing::error!(
                                        "Settlement failed: {:?}",
                                        settle_response.error_reason
                                    );
                                    Err(payment_error_response(
                                        StatusCode::PAYMENT_REQUIRED,
                                        &format!(
                                            "Payment settlement failed: {}",
                                            settle_response.error_reason.unwrap_or_default()
                                        ),
                                    ))
                                }
                            }
                            Err(e) => {
                                tracing::error!("Settlement error: {}", e);
                                Err(payment_error_response(
                                    StatusCode::BAD_GATEWAY,
                                    &format!("Settlement error: {}", e),
                                ))
                            }
                        }
                    } else {
                        tracing::warn!(
                            "Payment verification failed: {:?}",
                            verify_response.invalid_reason
                        );
                        Err(payment_error_response(
                            StatusCode::PAYMENT_REQUIRED,
                            &format!(
                                "Payment verification failed: {}",
                                verify_response.invalid_reason.unwrap_or_default()
                            ),
                        ))
                    }
                }
                Err(e) => {
                    tracing::error!("Payment verification error: {}", e);
                    Err(payment_error_response(
                        StatusCode::BAD_GATEWAY,
                        &format!("Payment verification error: {}", e),
                    ))
                }
            }
        }
    }
}
