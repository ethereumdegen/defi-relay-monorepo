use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

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
    headers: &HeaderMap,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    success: false,
                    error: "Missing or invalid Authorization header".to_string(),
                }),
            ))
        }
    };

    SessionService::get_wallet_id(pool, token)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    error: "Database error".to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    success: false,
                    error: "Invalid or expired token".to_string(),
                }),
            )
        })
}

/// Extract wallet_id from session token OR ERC-8128 signature.
async fn validate_session(
    pool: &sqlx::PgPool,
    headers: &HeaderMap,
    method: &str,
    authority: &str,
    path: &str,
    query: Option<&str>,
    body: &[u8],
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    // Try Bearer session first
    if let Ok(wallet_id) = validate_session_bearer(pool, headers).await {
        return Ok(wallet_id);
    }

    // Fall back to ERC-8128
    if erc8128_verify::has_erc8128_headers(headers) {
        let identity = erc8128_verify::verify_erc8128(method, authority, path, query, body, headers)
            .map_err(|e| {
                tracing::warn!("ERC-8128 verification failed: {}", e);
                (
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        success: false,
                        error: format!("ERC-8128 verification failed: {}", e),
                    }),
                )
            })?;
        return Ok(identity.wallet_address.to_lowercase());
    }

    Err((
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            success: false,
            error: "Missing or invalid Authorization header".to_string(),
        }),
    ))
}

/// Minimum encrypted data size (hex chars) - prevents spam with tiny payloads
/// 100 bytes = 200 hex chars (ECIES overhead alone is ~113 bytes)
const MIN_ENCRYPTED_DATA_HEX_LEN: usize = 200;

/// POST /api/store_keys - Store encrypted backup
/// Requires x402 payment if configured (X402_WALLET_ADDRESS set)
pub async fn store_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
    body_bytes: axum::body::Bytes,
) -> Result<Json<StoreKeysResponse>, axum::response::Response> {
    // Validate session first
    let authority = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let wallet_id = validate_session(
        &state.pool, &headers, "POST", authority, "/api/store_keys", None, &body_bytes,
    )
    .await
    .map_err(|e| e.into_response())?;

    let payload: StoreKeysRequest = serde_json::from_slice(&body_bytes).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: format!("Invalid request body: {}", e),
            }),
        )
            .into_response()
    })?;

    // Check for x402 payment if configured (capture tx_hash for audit)
    let payment_tx: Option<String> = if let Some(ref x402_config) = state.config.x402 {
        require_x402_payment(
            &state.http_client,
            x402_config,
            &headers,
            "/api/store_keys",
            "Store encrypted backup to keystore",
        )
        .await?
    } else {
        None
    };

    // Validate encrypted_data is not empty (use DELETE endpoint to remove backup)
    if payload.encrypted_data.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: "Cannot store empty data. Use DELETE /api/delete_keys to remove backup.".to_string(),
            }),
        ).into_response());
    }

    // Validate minimum size (prevents spam with tiny payloads)
    if payload.encrypted_data.len() < MIN_ENCRYPTED_DATA_HEX_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: format!(
                    "encrypted_data too small (minimum {} bytes)",
                    MIN_ENCRYPTED_DATA_HEX_LEN / 2
                ),
            }),
        ).into_response());
    }

    // Validate encrypted_data format (should be hex)
    if !payload
        .encrypted_data
        .chars()
        .all(|c| c.is_ascii_hexdigit())
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: "Invalid encrypted_data format (expected hex string)".to_string(),
            }),
        ).into_response());
    }

    // Check size limit
    if payload.encrypted_data.len() > state.config.max_encrypted_data_size * 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: format!(
                    "encrypted_data exceeds maximum size of {} bytes",
                    state.config.max_encrypted_data_size
                ),
            }),
        ).into_response());
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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                error: "Database error".to_string(),
            }),
        ).into_response()
    })?;

    Ok(Json(StoreKeysResponse {
        success: true,
        message: "Backup stored".to_string(),
        key_count: backup.key_count,
        updated_at: backup.updated_at.to_rfc3339(),
    }))
}

/// POST /api/get_keys - Retrieve encrypted backup
pub async fn get_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<GetKeysResponse>, (StatusCode, Json<ErrorResponse>)> {
    let authority = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let wallet_id = validate_session(
        &state.pool, &headers, "POST", authority, "/api/get_keys", None, &[],
    )
    .await?;

    let backup = BackupService::get_by_wallet(&state.pool, &wallet_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    error: "Database error".to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    success: false,
                    error: "No backup found for this wallet".to_string(),
                }),
            )
        })?;

    Ok(Json(GetKeysResponse {
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
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeleteKeysResponse>, (StatusCode, Json<ErrorResponse>)> {
    let authority = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let wallet_id = validate_session(
        &state.pool, &headers, "POST", authority, "/api/delete_keys", None, &[],
    )
    .await?;

    let deleted = BackupService::delete(&state.pool, &wallet_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete backup: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    error: "Database error".to_string(),
                }),
            )
        })?;

    if deleted {
        Ok(Json(DeleteKeysResponse {
            success: true,
            message: "Backup deleted".to_string(),
        }))
    } else {
        Ok(Json(DeleteKeysResponse {
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
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LogoutResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Extract token from header
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    success: false,
                    error: "Missing or invalid Authorization header".to_string(),
                }),
            ))
        }
    };

    let deleted = SessionService::delete(&state.pool, token)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete session: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    error: "Database error".to_string(),
                }),
            )
        })?;

    if deleted {
        Ok(Json(LogoutResponse {
            success: true,
            message: "Logged out successfully".to_string(),
        }))
    } else {
        // Token wasn't found, but that's okay - user is effectively logged out
        Ok(Json(LogoutResponse {
            success: true,
            message: "Session already expired or invalid".to_string(),
        }))
    }
}
