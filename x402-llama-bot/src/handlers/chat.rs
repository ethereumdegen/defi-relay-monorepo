use crate::error::AppError;
use crate::models::ChatRequest;
use crate::services::LlamaClient;
use actix_web::{web, HttpResponse};
use tracing::{debug, info};

/// Handle chat requests - proxy to DigitalOcean Llama agent
/// Payment verification is handled by the x402 middleware
pub async fn chat_handler(
    llama_client: web::Data<LlamaClient>,
    request: web::Json<ChatRequest>,
) -> Result<HttpResponse, AppError> {
    info!("Processing chat request");
    debug!("Chat request: {:?}", request);

    let response = llama_client.chat(&request).await?;

    debug!("Chat response: {:?}", response);
    info!("Chat request completed successfully");

    Ok(HttpResponse::Ok().json(response))
}
