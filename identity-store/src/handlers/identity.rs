use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::middleware::require_x402_payment;
use crate::services::{identity::IdentityService, session::SessionService};
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

/// POST /api/store_identity - Store identity JSON (authenticated, x402 if configured)
pub async fn store_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<StoreIdentityRequest>,
) -> Result<Json<StoreIdentityResponse>, axum::response::Response> {
    let wallet_id = validate_session(&state.pool, &headers).await.map_err(|e| e.into_response())?;

    // Check for x402 payment if configured
    let payment_tx: Option<String> = if let Some(ref x402_config) = state.config.x402 {
        require_x402_payment(
            &state.http_client,
            x402_config,
            &headers,
            "/api/store_identity",
            "Store agent identity to identity registry",
        )
        .await?
    } else {
        None
    };

    // Validate identity_json is not empty
    if payload.identity_json.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: "identity_json cannot be empty".to_string(),
            }),
        ).into_response());
    }

    // Validate it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&payload.identity_json).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: format!("Invalid JSON: {}", e),
            }),
        ).into_response()
    })?;

    // Re-serialize to canonical form (no extra whitespace, consistent key ordering)
    let canonical_json = serde_json::to_string(&parsed).unwrap_or_else(|_| payload.identity_json.clone());

    // Check size limit
    if canonical_json.len() > MAX_IDENTITY_JSON_SIZE {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: format!("identity_json exceeds maximum size of {} bytes", MAX_IDENTITY_JSON_SIZE),
            }),
        ).into_response());
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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                error: "Database error".to_string(),
            }),
        ).into_response()
    })?;

    let base_url = &state.config.public_url;
    let url = format!("{}/api/identity/{}/raw", base_url, identity.content_hash);

    Ok(Json(StoreIdentityResponse {
        success: true,
        message: "Identity stored".to_string(),
        content_hash: identity.content_hash,
        url,
        updated_at: identity.updated_at.to_rfc3339(),
    }))
}

/// GET /api/identity/:hash - Public: get identity by content hash
pub async fn get_identity_by_hash(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<GetIdentityResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate hash format (64 hex chars)
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: "Invalid content hash format".to_string(),
            }),
        ));
    }

    let identity = IdentityService::get_by_hash(&state.pool, &hash)
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
                    error: "Identity not found".to_string(),
                }),
            )
        })?;

    let identity_value: serde_json::Value = serde_json::from_str(&identity.identity_json)
        .unwrap_or(serde_json::Value::String(identity.identity_json));

    Ok(Json(GetIdentityResponse {
        success: true,
        identity_json: identity_value,
        content_hash: identity.content_hash,
        wallet_id: identity.wallet_id,
        updated_at: identity.updated_at.to_rfc3339(),
    }))
}

/// GET /api/identity/:hash/raw - Public: get raw identity JSON (EIP-8004 compliant)
///
/// Returns just the identity_json content directly, not wrapped in a response envelope.
/// This is the endpoint that agentURIs stored on-chain should point to, so any
/// EIP-8004 client can fetch and parse the RegistrationFile directly.
pub async fn get_identity_raw(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    use axum::response::IntoResponse;

    // Validate hash format (64 hex chars)
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: "Invalid content hash format".to_string(),
            }),
        ));
    }

    let identity = IdentityService::get_by_hash(&state.pool, &hash)
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
                    error: "Identity not found".to_string(),
                }),
            )
        })?;

    // Return the raw identity JSON directly with application/json content type
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        identity.identity_json,
    ).into_response())
}

/// POST /api/get_identity - Get your own identity (authenticated)
pub async fn get_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<GetIdentityResponse>, (StatusCode, Json<ErrorResponse>)> {
    let wallet_id = validate_session(&state.pool, &headers).await?;

    let identity = IdentityService::get_by_wallet(&state.pool, &wallet_id)
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
                    error: "No identity found for this wallet".to_string(),
                }),
            )
        })?;

    let identity_value: serde_json::Value = serde_json::from_str(&identity.identity_json)
        .unwrap_or(serde_json::Value::String(identity.identity_json));

    Ok(Json(GetIdentityResponse {
        success: true,
        identity_json: identity_value,
        content_hash: identity.content_hash,
        wallet_id: identity.wallet_id,
        updated_at: identity.updated_at.to_rfc3339(),
    }))
}

/// POST /api/delete_identity - Delete your identity (authenticated)
pub async fn delete_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeleteIdentityResponse>, (StatusCode, Json<ErrorResponse>)> {
    let wallet_id = validate_session(&state.pool, &headers).await?;

    let deleted = IdentityService::delete(&state.pool, &wallet_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete identity: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    error: "Database error".to_string(),
                }),
            )
        })?;

    if deleted {
        Ok(Json(DeleteIdentityResponse {
            success: true,
            message: "Identity deleted".to_string(),
        }))
    } else {
        Ok(Json(DeleteIdentityResponse {
            success: true,
            message: "No identity found to delete".to_string(),
        }))
    }
}

/// POST /api/logout - Invalidate current session
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LogoutResponse>, (StatusCode, Json<ErrorResponse>)> {
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
        Ok(Json(LogoutResponse {
            success: true,
            message: "Session already expired or invalid".to_string(),
        }))
    }
}
