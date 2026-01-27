use actix_web::{get, post, web, HttpResponse};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app_state::AppState;
use crate::config::{usdc_address, BASE_NETWORK, PAYMENT_SCHEME, X402_VERSION};
use crate::controllers::web_controller::WebController;
use crate::error::FacilitatorError;
use crate::types::{
    DomainEthAddress, PaymentPayload, PaymentRequirements, SettleRequest, SettleResponse,
    SupportedKind, SupportedResponse, VerifyRequest, VerifyResponse,
};

pub struct FacilitatorController;

impl WebController for FacilitatorController {
    fn config(cfg: &mut web::ServiceConfig) {
        cfg.service(supported)
            .service(verify)
            .service(settle);
    }
}

#[get("/supported")]
async fn supported() -> HttpResponse {
    let response = SupportedResponse {
        kinds: vec![SupportedKind {
            x402_version: X402_VERSION,
            scheme: PAYMENT_SCHEME.to_string(),
            network: BASE_NETWORK.to_string(),
        }],
    };
    HttpResponse::Ok().json(response)
}

#[post("/verify")]
async fn verify(
    state: web::Data<AppState>,
    body: web::Json<VerifyRequest>,
) -> HttpResponse {
    match validate_and_verify(&state, &body.payment_payload, &body.payment_requirements).await {
        Ok(payer) => HttpResponse::Ok().json(VerifyResponse {
            is_valid: true,
            invalid_reason: None,
            payer: Some(DomainEthAddress::from(payer)),
        }),
        Err(e) => HttpResponse::Ok().json(VerifyResponse {
            is_valid: false,
            invalid_reason: Some(e.to_string()),
            payer: None,
        }),
    }
}

#[post("/settle")]
async fn settle(
    state: web::Data<AppState>,
    body: web::Json<SettleRequest>,
) -> HttpResponse {
    // First validate everything
    if let Err(e) = validate_and_verify(&state, &body.payment_payload, &body.payment_requirements).await {
        return HttpResponse::BadRequest().json(SettleResponse {
            success: false,
            transaction: None,
            network: None,
            error: Some(e.to_string()),
        });
    }

    // Check nonce hasn't been used (on-chain check)
    let nonce_check = state
        .settlement_service
        .is_nonce_used(
            body.payment_payload.payload.from.inner(),
            body.payment_payload.payload.nonce.inner(),
        )
        .await;

    match nonce_check {
        Ok(true) => {
            return HttpResponse::BadRequest().json(SettleResponse {
                success: false,
                transaction: None,
                network: None,
                error: Some(FacilitatorError::NonceAlreadyUsed.to_string()),
            });
        }
        Ok(false) => {}
        Err(e) => {
            return HttpResponse::InternalServerError().json(SettleResponse {
                success: false,
                transaction: None,
                network: None,
                error: Some(e.to_string()),
            });
        }
    }

    // Submit settlement transaction
    match state
        .settlement_service
        .settle(&body.payment_payload.payload, &body.payment_payload.signature)
        .await
    {
        Ok(tx_hash) => HttpResponse::Ok().json(SettleResponse {
            success: true,
            transaction: Some(tx_hash),
            network: Some(BASE_NETWORK.to_string()),
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(SettleResponse {
            success: false,
            transaction: None,
            network: None,
            error: Some(e.to_string()),
        }),
    }
}

/// Validate payment payload and requirements, verify signature
async fn validate_and_verify(
    state: &AppState,
    payload: &PaymentPayload,
    requirements: &PaymentRequirements,
) -> Result<ethers::types::Address, FacilitatorError> {
    // Validate x402 version
    if payload.x402_version != X402_VERSION {
        return Err(FacilitatorError::InvalidVersion {
            expected: X402_VERSION,
            got: payload.x402_version,
        });
    }

    // Validate scheme
    if payload.scheme != PAYMENT_SCHEME {
        return Err(FacilitatorError::InvalidScheme {
            expected: PAYMENT_SCHEME.to_string(),
            got: payload.scheme.clone(),
        });
    }

    // Validate network
    if payload.network != BASE_NETWORK {
        return Err(FacilitatorError::InvalidNetwork {
            expected: BASE_NETWORK.to_string(),
            got: payload.network.clone(),
        });
    }

    // Validate token (asset must be USDC)
    let usdc = usdc_address();
    if requirements.asset.inner() != usdc {
        return Err(FacilitatorError::InvalidToken {
            expected: format!("{:?}", usdc),
            got: format!("{:?}", requirements.asset.inner()),
        });
    }

    // Validate 'to' address matches facilitator (front-running protection)
    let facilitator_addr = state.config.facilitator_address;
    if payload.payload.to.inner() != facilitator_addr {
        return Err(FacilitatorError::InvalidToAddress {
            expected: format!("{:?}", facilitator_addr),
            got: format!("{:?}", payload.payload.to.inner()),
        });
    }

    // Validate amount meets requirements
    if payload.payload.value.inner() < requirements.max_amount_required.inner() {
        return Err(FacilitatorError::InsufficientAmount {
            required: requirements.max_amount_required.inner().to_string(),
            got: payload.payload.value.inner().to_string(),
        });
    }

    // Validate time bounds
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let valid_after = payload.payload.valid_after.inner().as_u64();
    let valid_before = payload.payload.valid_before.inner().as_u64();

    if now < valid_after {
        return Err(FacilitatorError::PaymentNotYetValid { valid_after });
    }

    if now >= valid_before {
        return Err(FacilitatorError::PaymentExpired { valid_before });
    }

    // Verify EIP-712 signature
    let payer = state
        .eip712_verifier
        .verify_signature(&payload.payload, &payload.signature)?;

    Ok(payer)
}
