mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod services;

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{http::header, web, App, HttpResponse, HttpServer};
use std::fs;
use std::sync::Arc;
use config::Config;
use handlers::{agent_info_handler, chat_handler};
use middleware::X402Middleware;
use services::{FacilitatorClient, LlamaClient, NonceTracker};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// Root endpoint with usage instructions
async fn root_handler() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body(
r#"x402 Llama Bot - Pay-per-request AI Chat

This bot provides access to a Llama AI agent using the x402 payment protocol.

ENDPOINTS:
  GET  /                       - This help page
  GET  /health                 - Health check
  GET  /.well-known/x402       - x402 discovery document
  GET  /agent.json             - Agent metadata (EIP-8004)
  POST /chat                   - Chat with the Llama agent (payment required)
  POST /api/v1/chat/completions - OpenAI-compatible endpoint (payment required)

USAGE:
  1. Send a POST request to /chat with an OpenAI-compatible chat payload
  2. If no payment header is provided, you'll receive a 402 response
     with payment requirements in the "payment-required" header
  3. Create a payment using the x402 protocol and include it in
     the "X-PAYMENT" header
  4. The bot will verify payment and forward your request to Llama

EXAMPLE REQUEST:
  POST /chat
  Content-Type: application/json
  X-PAYMENT: <base64-encoded-payment>

  {
    "messages": [
      {"role": "user", "content": "Hello!"}
    ]
  }

For more info on x402: https://www.x402.org
"#)
}

/// Health check endpoint
async fn health_handler() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "x402-llama-bot"
    }))
}

/// Serve x402 discovery document (handles extensionless file)
async fn x402_discovery_handler() -> HttpResponse {
    match fs::read_to_string("public/.well-known/x402") {
        Ok(content) => HttpResponse::Ok()
            .content_type("application/json")
            .body(content),
        Err(_) => HttpResponse::NotFound().finish(),
    }
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
    info!("Max input tokens: {}", config.max_input_tokens);

    // Generate x402 discovery document if BASE_URL is configured
    if let Some(ref base_url) = config.base_url {
        let discovery = serde_json::json!({
            "version": 1,
            "resources": [
                format!("{}/chat", base_url),
                format!("{}/api/v1/chat/completions", base_url)
            ],
            "ownershipProofs": [
                config.bot_wallet_address.to_hex()
            ]
        });

        let discovery_path = "public/.well-known/x402";
        if let Err(e) = fs::write(discovery_path, serde_json::to_string_pretty(&discovery).unwrap()) {
            error!("Failed to write x402 discovery file: {}", e);
        } else {
            info!("Generated x402 discovery document at {}", discovery_path);
        }
    } else {
        info!("BASE_URL not set, skipping x402 discovery document generation");
    }

    // Create service clients
    let facilitator_client = FacilitatorClient::new(&config.facilitator_url);
    let llama_client = LlamaClient::new(&config.do_agent_endpoint, &config.do_agent_secret);

    // Create nonce tracker for replay protection (10 minute TTL)
    let nonce_tracker = Arc::new(NonceTracker::with_default_ttl());
    info!("Nonce tracker initialized for replay protection");

    // Store config for middleware
    let config_for_middleware = config.clone();
    let facilitator_for_middleware = facilitator_client.clone();
    let nonce_tracker_for_middleware = nonce_tracker.clone();

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST", "OPTIONS"])
            .allowed_headers(vec![
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                header::HeaderName::from_static("x-payment"),
            ])
            .expose_headers(vec![
                header::HeaderName::from_static("payment-required"),
            ])
            .max_age(3600);

        App::new()
            .wrap(cors)
            // Share config and Llama client across handlers
            .app_data(web::Data::new(config_for_middleware.clone()))
            .app_data(web::Data::new(llama_client.clone()))
            // Public endpoints (no payment required)
            .route("/", web::get().to(root_handler))
            .route("/health", web::get().to(health_handler))
            .route("/agent.json", web::get().to(agent_info_handler))
            // Serve x402 discovery document explicitly (actix-files has issues with extensionless files)
            .route("/.well-known/x402", web::get().to(x402_discovery_handler))
            // Serve .well-known directory for other files
            .service(Files::new("/.well-known", "public/.well-known"))
            // Protected endpoints with x402 middleware
            .service(
                web::scope("/chat")
                    .wrap(X402Middleware::new(
                        config_for_middleware.clone(),
                        facilitator_for_middleware.clone(),
                        nonce_tracker_for_middleware.clone(),
                    ))
                    .route("", web::post().to(chat_handler)),
            )
            // OpenAI-compatible endpoint
            .service(
                web::scope("/api/v1/chat")
                    .wrap(X402Middleware::new(
                        config_for_middleware.clone(),
                        facilitator_for_middleware.clone(),
                        nonce_tracker_for_middleware.clone(),
                    ))
                    .route("/completions", web::post().to(chat_handler)),
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
