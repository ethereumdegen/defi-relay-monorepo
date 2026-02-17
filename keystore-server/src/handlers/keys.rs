use actix_web::{http::header, web, HttpRequest, HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::middleware::require_x402_payment;
use crate::services::{backup::BackupService, erc8128_verify, session::SessionService};
use crate::AppState;

use super::auth::ErrorResponse;

#[derive(Debug, Deserialize)]
pub struct StoreKeysRequest {
    pub encrypted_data: String,
    pub key_count: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct StoreKeysResponse {
    pub success: bool,
    pub message: String,
    pub key_count: i32,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct GetKeysResponse {
    pub success: bool,
    pub encrypted_data: String,
    pub key_count: i32,
    pub updated_at: String,
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

/// Minimum encrypted data size (hex chars) - prevents spam with tiny payloads
/// 100 bytes = 200 hex chars (ECIES overhead alone is ~113 bytes)
const MIN_ENCRYPTED_DATA_HEX_LEN: usize = 200;

/// POST /api/store_keys - Store encrypted backup
/// Requires x402 payment if configured (X402_WALLET_ADDRESS set)
pub async fn store_keys(
    state: web::Data<AppState>,
    req: HttpRequest,
    body_bytes: web::Bytes,
) -> HttpResponse {
    match store_keys_inner(state, req, body_bytes).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(resp) => resp,
    }
}

async fn store_keys_inner(
    state: web::Data<AppState>,
    req: HttpRequest,
    body_bytes: web::Bytes,
) -> Result<StoreKeysResponse, HttpResponse> {
    let headers = req.headers();

    // Validate session first
    let authority = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let wallet_id = validate_session(
        &state.pool,
        headers,
        "POST",
        authority,
        "/api/store_keys",
        None,
        &body_bytes,
    )
    .await
    .map_err(|e| e.error_response())?;

    let payload: StoreKeysRequest = serde_json::from_slice(&body_bytes).map_err(|e| {
        HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error: format!("Invalid request body: {}", e),
        })
    })?;

    // Check for x402 payment if configured (capture tx_hash for audit)
    let payment_tx: Option<String> = if let Some(ref x402_config) = state.config.x402 {
        require_x402_payment(
            &state.http_client,
            x402_config,
            headers,
            "/api/store_keys",
            "Store encrypted backup to keystore",
        )
        .await
        .map_err(|e| e.error_response())?
    } else {
        None
    };

    // Validate encrypted_data is not empty (use DELETE endpoint to remove backup)
    if payload.encrypted_data.is_empty() {
        return Err(HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error: "Cannot store empty data. Use DELETE /api/delete_keys to remove backup."
                .to_string(),
        }));
    }

    // Validate minimum size (prevents spam with tiny payloads)
    if payload.encrypted_data.len() < MIN_ENCRYPTED_DATA_HEX_LEN {
        return Err(HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error: format!(
                "encrypted_data too small (minimum {} bytes)",
                MIN_ENCRYPTED_DATA_HEX_LEN / 2
            ),
        }));
    }

    // Validate encrypted_data format (should be hex)
    if !payload
        .encrypted_data
        .chars()
        .all(|c| c.is_ascii_hexdigit())
    {
        return Err(HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error: "Invalid encrypted_data format (expected hex string)".to_string(),
        }));
    }

    // Check size limit
    if payload.encrypted_data.len() > state.config.max_encrypted_data_size * 2 {
        return Err(HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error: format!(
                "encrypted_data exceeds maximum size of {} bytes",
                state.config.max_encrypted_data_size
            ),
        }));
    }

    // Validate key_count (ensure non-negative)
    let key_count = payload.key_count.unwrap_or(0).max(0);

    let backup = BackupService::upsert(
        &state.pool,
        &wallet_id,
        &payload.encrypted_data,
        key_count,
        payment_tx.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to store backup: {}", e);
        HttpResponse::InternalServerError().json(ErrorResponse {
            success: false,
            error: "Database error".to_string(),
        })
    })?;

    Ok(StoreKeysResponse {
        success: true,
        message: "Backup stored".to_string(),
        key_count: backup.key_count,
        updated_at: backup.updated_at.to_rfc3339(),
    })
}

/// POST /api/get_keys - Retrieve encrypted backup
pub async fn get_keys(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<web::Json<GetKeysResponse>, AppError> {
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
        "/api/get_keys",
        None,
        &[],
    )
    .await?;

    let backup = BackupService::get_by_wallet(&state.pool, &wallet_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            AppError::Internal("Database error".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("No backup found for this wallet".to_string()))?;

    Ok(web::Json(GetKeysResponse {
        success: true,
        encrypted_data: backup.encrypted_data,
        key_count: backup.key_count,
        updated_at: backup.updated_at.to_rfc3339(),
    }))
}

#[derive(Debug, Serialize)]
pub struct DeleteKeysResponse {
    pub success: bool,
    pub message: String,
}

/// POST /api/delete_keys - Delete backup (authenticated, free - no x402 payment)
pub async fn delete_keys(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<web::Json<DeleteKeysResponse>, AppError> {
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
        "/api/delete_keys",
        None,
        &[],
    )
    .await?;

    let deleted = BackupService::delete(&state.pool, &wallet_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete backup: {}", e);
            AppError::Internal("Database error".to_string())
        })?;

    if deleted {
        Ok(web::Json(DeleteKeysResponse {
            success: true,
            message: "Backup deleted".to_string(),
        }))
    } else {
        Ok(web::Json(DeleteKeysResponse {
            success: true,
            message: "No backup found to delete".to_string(),
        }))
    }
}

#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub success: bool,
    pub message: String,
}

/// POST /api/logout - Invalidate current session
pub async fn logout(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<web::Json<LogoutResponse>, AppError> {
    let headers = req.headers();
    // Extract token from header
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
        // Token wasn't found, but that's okay - user is effectively logged out
        Ok(web::Json(LogoutResponse {
            success: true,
            message: "Session already expired or invalid".to_string(),
        }))
    }
}
