use crate::config::Config;
use crate::error::AppError;
use crate::models::ChatRequest;
use crate::services::KimiClient;
use actix_web::{web, HttpResponse};
use tracing::{debug, info, warn};

/// Estimate token count from text using a simple heuristic.
/// Uses chars / 4 which is a common approximation for LLM tokenizers.
fn estimate_tokens(text: &str) -> u32 {
    (text.chars().count() / 4) as u32
}

/// Calculate estimated input tokens for a chat request
fn estimate_request_tokens(request: &ChatRequest) -> u32 {
    request
        .messages
        .iter()
        .map(|msg| estimate_tokens(&msg.role) + estimate_tokens(&msg.content))
        .sum()
}

/// Handle chat requests - proxy to Moonshot Kimi API
/// Payment verification is handled by the x402 middleware
pub async fn chat_handler(
    kimi_client: web::Data<KimiClient>,
    config: web::Data<Config>,
    request: web::Json<ChatRequest>,
) -> Result<HttpResponse, AppError> {
    info!("Processing chat request");
    debug!("Chat request: {:?}", request);

    // Estimate and validate input token count
    let estimated_tokens = estimate_request_tokens(&request);
    info!("Estimated input tokens: {}", estimated_tokens);

    if estimated_tokens > config.max_input_tokens {
        warn!(
            "Request rejected: estimated {} tokens exceeds limit of {}",
            estimated_tokens, config.max_input_tokens
        );
        return Err(AppError::InputTooLarge(format!(
            "Estimated {} input tokens exceeds maximum of {}",
            estimated_tokens, config.max_input_tokens
        )));
    }

    let response = kimi_client.chat(&request).await?;

    debug!("Chat response: {:?}", response);
    info!("Chat request completed successfully");

    Ok(HttpResponse::Ok().json(response))
}
