use actix_web::{http::header, web, HttpRequest, HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::middleware::require_x402_payment;
use crate::services::{erc8128_verify, identity::IdentityService, session::SessionService};
use crate::AppState;

use super::auth::ErrorResponse;

/// Maximum identity JSON size: 256KB
const MAX_IDENTITY_JSON_SIZE: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
pub struct StoreIdentityRequest {
    pub identity_json: String,
}

#[derive(Debug, Serialize)]
pub struct StoreIdentityResponse {
    pub success: bool,
    pub message: String,
    pub content_hash: String,
    pub url: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct GetIdentityResponse {
    pub success: bool,
    pub identity_json: serde_json::Value,
    pub content_hash: String,
    pub wallet_id: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteIdentityResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub success: bool,
    pub message: String,
}

/// Extract wallet_id from Bearer session token
async fn validate_session_bearer(
    pool: &sqlx::PgPool,
    headers: &actix_web::http::header::HeaderMap,
) -> Result<String, AppError> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return Err(AppError::Unauthorized(
                "Missing or invalid Authorization header".to_string(),
            ))
        }
    };

    SessionService::get_wallet_id(pool, token)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            AppError::Internal("Database error".to_string())
        })?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired token".to_string()))
}

/// Extract wallet_id from session token OR ERC-8128 signature.
async fn validate_session(
    pool: &sqlx::PgPool,
    headers: &actix_web::http::header::HeaderMap,
    method: &str,
    authority: &str,
    path: &str,
    query: Option<&str>,
    body: &[u8],
) -> Result<String, AppError> {
    // Try Bearer session first
    if let Ok(wallet_id) = validate_session_bearer(pool, headers).await {
        return Ok(wallet_id);
    }

    // Fall back to ERC-8128
    if erc8128_verify::has_erc8128_headers(headers) {
        let identity =
            erc8128_verify::verify_erc8128(method, authority, path, query, body, headers).map_err(
                |e| {
                    tracing::warn!("ERC-8128 verification failed: {}", e);
                    AppError::Unauthorized(format!("ERC-8128 verification failed: {}", e))
                },
            )?;
        return Ok(identity.wallet_address.to_lowercase());
    }

    Err(AppError::Unauthorized(
        "Missing or invalid Authorization header".to_string(),
    ))
}

/// POST /api/store_identity - Store identity JSON (authenticated, x402 if configured)
pub async fn store_identity(
    state: web::Data<AppState>,
    req: HttpRequest,
    body_bytes: web::Bytes,
) -> HttpResponse {
    match store_identity_inner(state, req, body_bytes).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(resp) => resp,
    }
}

async fn store_identity_inner(
    state: web::Data<AppState>,
    req: HttpRequest,
    body_bytes: web::Bytes,
) -> Result<StoreIdentityResponse, HttpResponse> {
    let headers = req.headers();

    let authority = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let wallet_id = validate_session(
        &state.pool,
        headers,
        "POST",
        authority,
        "/api/store_identity",
        None,
        &body_bytes,
    )
    .await
    .map_err(|e| e.error_response())?;

    let payload: StoreIdentityRequest = serde_json::from_slice(&body_bytes).map_err(|e| {
        HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error: format!("Invalid request body: {}", e),
        })
    })?;

    // Check for x402 payment if configured
    let payment_tx: Option<String> = if let Some(ref x402_config) = state.config.x402 {
        require_x402_payment(
            &state.http_client,
            x402_config,
            headers,
            "/api/store_identity",
            "Store agent identity to identity registry",
        )
        .await
        .map_err(|e| e.error_response())?
    } else {
        None
    };

    // Validate identity_json is not empty
    if payload.identity_json.trim().is_empty() {
        return Err(HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error: "identity_json cannot be empty".to_string(),
        }));
    }

    // Validate it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&payload.identity_json).map_err(|e| {
        HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error: format!("Invalid JSON: {}", e),
        })
    })?;

    // Re-serialize to canonical form (no extra whitespace, consistent key ordering)
    let canonical_json =
        serde_json::to_string(&parsed).unwrap_or_else(|_| payload.identity_json.clone());

    // Check size limit
    if canonical_json.len() > MAX_IDENTITY_JSON_SIZE {
        return Err(HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error: format!(
                "identity_json exceeds maximum size of {} bytes",
                MAX_IDENTITY_JSON_SIZE
            ),
        }));
    }

    let identity = IdentityService::upsert(
        &state.pool,
        &wallet_id,
        &canonical_json,
        payment_tx.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to store identity: {}", e);
        HttpResponse::InternalServerError().json(ErrorResponse {
            success: false,
            error: "Database error".to_string(),
        })
    })?;

    let base_url = &state.config.public_url;
    let url = format!("{}/api/identity/{}/raw", base_url, identity.content_hash);

    Ok(StoreIdentityResponse {
        success: true,
        message: "Identity stored".to_string(),
        content_hash: identity.content_hash,
        url,
        updated_at: identity.updated_at.to_rfc3339(),
    })
}

/// GET /api/identity/{hash} - Public: get identity by content hash
pub async fn get_identity_by_hash(
    state: web::Data<AppState>,
    hash: web::Path<String>,
) -> Result<web::Json<GetIdentityResponse>, AppError> {
    // Validate hash format (64 hex chars)
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::BadRequest(
            "Invalid content hash format".to_string(),
        ));
    }

    let identity = IdentityService::get_by_hash(&state.pool, &hash)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            AppError::Internal("Database error".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("Identity not found".to_string()))?;

    let identity_value: serde_json::Value = serde_json::from_str(&identity.identity_json)
        .unwrap_or(serde_json::Value::String(identity.identity_json));

    Ok(web::Json(GetIdentityResponse {
        success: true,
        identity_json: identity_value,
        content_hash: identity.content_hash,
        wallet_id: identity.wallet_id,
        updated_at: identity.updated_at.to_rfc3339(),
    }))
}

/// GET /api/identity/{hash}/raw - Public: get raw identity JSON (EIP-8004 compliant)
///
/// Returns just the identity_json content directly, not wrapped in a response envelope.
/// This is the endpoint that agentURIs stored on-chain should point to, so any
/// EIP-8004 client can fetch and parse the RegistrationFile directly.
pub async fn get_identity_raw(
    state: web::Data<AppState>,
    hash: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    // Validate hash format (64 hex chars)
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::BadRequest(
            "Invalid content hash format".to_string(),
        ));
    }

    let identity = IdentityService::get_by_hash(&state.pool, &hash)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            AppError::Internal("Database error".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("Identity not found".to_string()))?;

    // Return the raw identity JSON directly with application/json content type
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(identity.identity_json))
}

/// POST /api/get_identity - Get your own identity (authenticated)
pub async fn get_identity(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<web::Json<GetIdentityResponse>, AppError> {
    let headers = req.headers();
    let authority = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let wallet_id = validate_session(
        &state.pool,
        headers,
        "POST",
        authority,
        "/api/get_identity",
        None,
        &[],
    )
    .await?;

    let identity = IdentityService::get_by_wallet(&state.pool, &wallet_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            AppError::Internal("Database error".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("No identity found for this wallet".to_string()))?;

    let identity_value: serde_json::Value = serde_json::from_str(&identity.identity_json)
        .unwrap_or(serde_json::Value::String(identity.identity_json));

    Ok(web::Json(GetIdentityResponse {
        success: true,
        identity_json: identity_value,
        content_hash: identity.content_hash,
        wallet_id: identity.wallet_id,
        updated_at: identity.updated_at.to_rfc3339(),
    }))
}

/// POST /api/delete_identity - Delete your identity (authenticated)
pub async fn delete_identity(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<web::Json<DeleteIdentityResponse>, AppError> {
    let headers = req.headers();
    let authority = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let wallet_id = validate_session(
        &state.pool,
        headers,
        "POST",
        authority,
        "/api/delete_identity",
        None,
        &[],
    )
    .await?;

    let deleted = IdentityService::delete(&state.pool, &wallet_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete identity: {}", e);
            AppError::Internal("Database error".to_string())
        })?;

    if deleted {
        Ok(web::Json(DeleteIdentityResponse {
            success: true,
            message: "Identity deleted".to_string(),
        }))
    } else {
        Ok(web::Json(DeleteIdentityResponse {
            success: true,
            message: "No identity found to delete".to_string(),
        }))
    }
}

/// POST /api/logout - Invalidate current session
pub async fn logout(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<web::Json<LogoutResponse>, AppError> {
    let headers = req.headers();
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return Err(AppError::Unauthorized(
                "Missing or invalid Authorization header".to_string(),
            ))
        }
    };

    let deleted = SessionService::delete(&state.pool, token)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete session: {}", e);
            AppError::Internal("Database error".to_string())
        })?;

    if deleted {
        Ok(web::Json(LogoutResponse {
            success: true,
            message: "Logged out successfully".to_string(),
        }))
    } else {
        Ok(web::Json(LogoutResponse {
            success: true,
            message: "Session already expired or invalid".to_string(),
        }))
    }
}
