use crate::config::{NetworkRegistry, HEAVY_METHODS};
use crate::error::AppError;
use actix_web::{web, HttpResponse};
use serde_json::Value;
use tracing::{debug, error, warn};

/// Extract the RPC method from a JSON-RPC request body
fn extract_method(body: &Value) -> Option<&str> {
    body.get("method").and_then(|m| m.as_str())
}

/// Check if a method is considered "heavy"
fn is_heavy_method(method: &str) -> bool {
    HEAVY_METHODS.contains(&method)
}

/// Handle JSON-RPC requests for light endpoint (rejects heavy methods)
pub async fn light_rpc_handler(
    path: web::Path<String>,
    body: web::Json<Value>,
    registry: web::Data<NetworkRegistry>,
) -> Result<HttpResponse, AppError> {
    let network = path.into_inner();
    debug!("Light RPC request for network: {}", network);

    // Check if the method is a heavy method
    if let Some(method) = extract_method(&body) {
        if is_heavy_method(method) {
            warn!(
                "Heavy method '{}' rejected on light endpoint for network: {}",
                method, network
            );
            return Err(AppError::HeavyMethodNotAllowed(method.to_string()));
        }
    }

    let rpc_client = registry.get(&network).ok_or_else(|| {
        error!("Network not found: {}", network);
        AppError::NetworkNotFound(network.clone())
    })?;

    let response = rpc_client.forward(body.into_inner()).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(response))
}

/// Handle JSON-RPC requests for heavy endpoint (allows ALL methods)
pub async fn heavy_rpc_handler(
    path: web::Path<String>,
    body: web::Json<Value>,
    registry: web::Data<NetworkRegistry>,
) -> Result<HttpResponse, AppError> {
    let network = path.into_inner();
    debug!("Heavy RPC request for network: {}", network);

    let rpc_client = registry.get(&network).ok_or_else(|| {
        error!("Network not found: {}", network);
        AppError::NetworkNotFound(network.clone())
    })?;

    let response = rpc_client.forward(body.into_inner()).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(response))
}
