use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::services::{backup::BackupService, session::SessionService};
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

/// Extract wallet_id from session token
async fn validate_session(
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

/// POST /api/store_keys - Store encrypted backup
pub async fn store_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<StoreKeysRequest>,
) -> Result<Json<StoreKeysResponse>, (StatusCode, Json<ErrorResponse>)> {
    let wallet_id = validate_session(&state.pool, &headers).await?;

    // Validate encrypted_data format (should be hex)
    if payload.encrypted_data.is_empty()
        || !payload
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
        ));
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
        ));
    }

    let key_count = payload.key_count.unwrap_or(0);

    let backup = BackupService::upsert(&state.pool, &wallet_id, &payload.encrypted_data, key_count)
        .await
        .map_err(|e| {
            tracing::error!("Failed to store backup: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    error: "Database error".to_string(),
                }),
            )
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
    let wallet_id = validate_session(&state.pool, &headers).await?;

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
