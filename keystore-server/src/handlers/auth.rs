use actix_web::web;
use chrono::{Duration, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use siwe::{Message, VerificationOpts};
use time::OffsetDateTime;

use crate::error::AppError;
use crate::services::{challenge::ChallengeService, session::SessionService};
use crate::AppState;

// Request/Response types
#[derive(Debug, Deserialize)]
pub struct AuthorizeRequest {
    pub address: String,
}

#[derive(Debug, Serialize)]
pub struct AuthorizeResponse {
    pub success: bool,
    pub message: String,
    pub nonce: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub address: String,
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    pub success: bool,
    pub token: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

fn is_valid_address(address: &str) -> bool {
    address.len() == 42
        && address.starts_with("0x")
        && address[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_address(address: &str) -> Result<[u8; 20], &'static str> {
    if !is_valid_address(address) {
        return Err("Invalid address format");
    }
    let bytes = hex::decode(&address[2..]).map_err(|_| "Invalid hex")?;
    bytes.try_into().map_err(|_| "Invalid length")
}

fn generate_nonce() -> String {
    let bytes: [u8; 16] = rand::thread_rng().gen();
    hex::encode(bytes)
}

/// Convert chrono DateTime to time OffsetDateTime
fn chrono_to_time(dt: chrono::DateTime<Utc>) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(dt.timestamp()).unwrap()
}

/// POST /api/authorize - Request SIWE challenge
pub async fn authorize(
    state: web::Data<AppState>,
    payload: web::Json<AuthorizeRequest>,
) -> Result<web::Json<AuthorizeResponse>, AppError> {
    // Parse and validate address
    let address = parse_address(&payload.address)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let nonce = generate_nonce();
    let now = Utc::now();
    let expires_at = now + Duration::seconds(state.config.challenge_ttl_secs);

    // Create SIWE message using time crate types
    let domain = state.config.domain.clone().try_into().map_err(|e| {
        tracing::error!("Invalid domain config '{}': {:?}", state.config.domain, e);
        AppError::Internal("Server configuration error".to_string())
    })?;

    let uri = format!("https://{}", state.config.domain)
        .parse()
        .map_err(|e| {
            tracing::error!("Invalid URI from domain '{}': {:?}", state.config.domain, e);
            AppError::Internal("Server configuration error".to_string())
        })?;

    let message = Message {
        domain,
        address,
        statement: Some("Sign in to Keystore API".to_string()),
        uri,
        version: siwe::Version::V1,
        chain_id: 1,
        nonce: nonce.clone(),
        issued_at: chrono_to_time(now).into(),
        expiration_time: Some(chrono_to_time(expires_at).into()),
        not_before: None,
        request_id: None,
        resources: vec![],
    };

    let message_string = message.to_string();

    // Store challenge
    ChallengeService::create(
        &state.pool,
        &payload.address.to_lowercase(),
        &nonce,
        &message_string,
        expires_at,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to store challenge: {}", e);
        AppError::Internal("Database error".to_string())
    })?;

    Ok(web::Json(AuthorizeResponse {
        success: true,
        message: message_string,
        nonce,
    }))
}

/// POST /api/authorize/verify - Verify signature and issue token
pub async fn verify(
    state: web::Data<AppState>,
    payload: web::Json<VerifyRequest>,
) -> Result<web::Json<VerifyResponse>, AppError> {
    // Validate address format
    if !is_valid_address(&payload.address) {
        return Err(AppError::BadRequest("Invalid address format".to_string()));
    }

    let wallet_id = payload.address.to_lowercase();

    // Get pending challenge
    let challenge = ChallengeService::get_pending(&state.pool, &wallet_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            AppError::Internal("Database error".to_string())
        })?
        .ok_or_else(|| {
            AppError::Unauthorized("No pending challenge or expired".to_string())
        })?;

    // Parse and verify SIWE message
    let message: Message = challenge.message.parse().map_err(|e| {
        tracing::error!("Failed to parse SIWE message: {}", e);
        AppError::Internal("Invalid stored message".to_string())
    })?;

    // Decode signature
    let signature = hex::decode(payload.signature.trim_start_matches("0x")).map_err(|_| {
        AppError::BadRequest("Invalid signature format".to_string())
    })?;

    // Verify signature with default options
    let opts = VerificationOpts::default();
    message
        .verify(&signature, &opts)
        .await
        .map_err(|e| {
            tracing::warn!("Signature verification failed: {:?}", e);
            AppError::Unauthorized("Invalid signature".to_string())
        })?;

    // Delete used challenge
    ChallengeService::delete_for_wallet(&state.pool, &wallet_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete challenge: {}", e);
            AppError::Internal("Database error".to_string())
        })?;

    // Create session token
    let token = SessionService::generate_token();
    let expires_at = Utc::now() + Duration::seconds(state.config.session_ttl_secs);

    SessionService::create(&state.pool, &token, &wallet_id, expires_at)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create session: {}", e);
            AppError::Internal("Database error".to_string())
        })?;

    Ok(web::Json(VerifyResponse {
        success: true,
        token,
        expires_at: expires_at.to_rfc3339(),
    }))
}
