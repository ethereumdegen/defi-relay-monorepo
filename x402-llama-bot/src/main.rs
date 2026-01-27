mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod services;

use actix_web::{web, App, HttpResponse, HttpServer};
use config::Config;
use handlers::{agent_info_handler, chat_handler};
use middleware::X402Middleware;
use services::{FacilitatorClient, LlamaClient};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// Health check endpoint
async fn health_handler() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "x402-llama-bot"
    }))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Load configuration
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            error!("Configuration error: {}", e);
            std::process::exit(1);
        }
    };

    let port = config.port;
    info!("Starting x402-llama-bot on port {}", port);
    info!("Bot wallet: {}", config.bot_wallet_address);
    info!("Facilitator URL: {}", config.facilitator_url);
    info!("Cost per request: {} raw USDC", config.cost_per_request);

    // Create service clients
    let facilitator_client = FacilitatorClient::new(&config.facilitator_url);
    let llama_client = LlamaClient::new(&config.do_agent_endpoint, &config.do_agent_secret);

    // Store config for middleware
    let config_for_middleware = config.clone();
    let facilitator_for_middleware = facilitator_client.clone();

    HttpServer::new(move || {
        App::new()
            // Share Llama client across handlers
            .app_data(web::Data::new(llama_client.clone()))
            // Public endpoints (no payment required)
            .route("/health", web::get().to(health_handler))
            .route("/agent.json", web::get().to(agent_info_handler))
            // Protected endpoint with x402 middleware
            .service(
                web::scope("/chat")
                    .wrap(X402Middleware::new(
                        config_for_middleware.clone(),
                        facilitator_for_middleware.clone(),
                    ))
                    .route("", web::post().to(chat_handler)),
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
