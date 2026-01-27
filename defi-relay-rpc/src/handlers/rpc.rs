use crate::config::NetworkRegistry;
use crate::error::AppError;
use actix_web::{web, HttpResponse};
use serde_json::Value;
use tracing::{debug, error};

/// Handle JSON-RPC requests for a specific network
pub async fn rpc_handler(
    path: web::Path<String>,
    body: web::Json<Value>,
    registry: web::Data<NetworkRegistry>,
) -> Result<HttpResponse, AppError> {
    let network = path.into_inner();
    debug!("RPC request for network: {}", network);

    let rpc_client = registry.get(&network).ok_or_else(|| {
        error!("Network not found: {}", network);
        AppError::NetworkNotFound(network.clone())
    })?;

    let response = rpc_client.forward(body.into_inner()).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(response))
}
