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
use services::{FacilitatorClient, KimiClient, NonceTracker, RateLimiter, SettlementQueue, SettlementWorker};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

/// Root endpoint with usage instructions
async fn root_handler() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body(
r#"x402 Kimi Bot - Pay-per-request AI Chat

This bot provides access to Moonshot's Kimi AI using the x402 payment protocol.

ENDPOINTS:
  GET  /                       - This help page
  GET  /health                 - Health check
  GET  /metrics                - Settlement stats and queue info
  GET  /.well-known/x402       - x402 discovery document
  GET  /agent.json             - Agent metadata (EIP-8004)
  POST /chat                   - Chat with Kimi (payment required)
  POST /api/v1/chat/completions - OpenAI-compatible endpoint (payment required)

USAGE:
  1. Send a POST request to /chat with an OpenAI-compatible chat payload
  2. If no payment header is provided, you'll receive a 402 response
     with payment requirements in the "payment-required" header
  3. Create a payment using the x402 protocol and include it in
     the "X-PAYMENT" header
  4. The bot will verify payment and forward your request to Kimi

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
async fn health_handler(
    settlement_queue: Option<web::Data<Arc<SettlementQueue>>>,
) -> HttpResponse {
    let queue_depth = settlement_queue
        .as_ref()
        .map(|q| q.len())
        .unwrap_or(0);

    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "x402-kimi-bot",
        "settlement_queue_depth": queue_depth
    }))
}

/// Metrics endpoint for observability
async fn metrics_handler(
    settlement_queue: web::Data<Arc<SettlementQueue>>,
    worker_metrics: web::Data<Arc<services::SettlementMetrics>>,
) -> HttpResponse {
    let (total, success, failure, retries) = worker_metrics.get_stats();

    HttpResponse::Ok().json(serde_json::json!({
        "settlement_queue": {
            "depth": settlement_queue.len(),
            "max_size": settlement_queue.max_size(),
            "is_full": settlement_queue.is_full()
        },
        "settlement_worker": {
            "total_processed": total,
            "success_count": success,
            "failure_count": failure,
            "retry_count": retries
        }
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
    info!("Starting x402-kimi-bot on port {}", port);
    info!("Bot wallet: {}", config.bot_wallet_address);
    info!("Facilitator URL: {}", config.facilitator_url);
    info!("Moonshot endpoint: {}", config.moonshot_endpoint);
    info!("Cost per request: {} raw USDC", config.cost_per_request);
    info!("Max input tokens: {}", config.max_input_tokens);
    info!("Max output tokens: {}", config.max_output_tokens);
    info!("Default model: {}", config.default_model);
    if config.system_prompt.is_some() {
        info!("System prompt loaded from SYSTEM_PROMPT.md");
    }

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
    let kimi_client = KimiClient::new(
        &config.moonshot_endpoint,
        &config.moonshot_api_key,
        &config.default_model,
    );

    // Create nonce tracker for replay protection (10 minute TTL)
    let nonce_tracker = Arc::new(NonceTracker::with_default_ttl());
    info!("Nonce tracker initialized for replay protection");

    // Create rate limiter (5 requests per second per address)
    let rate_limiter = Arc::new(RateLimiter::new(5));
    info!("Rate limiter initialized: 5 requests/second per address");

    // Create settlement queue with configurable max size
    let max_queue_size = std::env::var("SETTLEMENT_QUEUE_MAX_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(services::DEFAULT_MAX_QUEUE_SIZE);
    let settlement_queue = Arc::new(SettlementQueue::with_max_size(max_queue_size));
    info!(
        "Settlement queue initialized with max size: {}",
        max_queue_size
    );

    // Create shutdown channel for graceful shutdown
    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);

    // Spawn background settlement worker
    let worker = SettlementWorker::new(settlement_queue.clone(), facilitator_client.clone());
    let worker_metrics = worker.metrics();
    let shutdown_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        worker.run(shutdown_rx).await;
    });
    info!("Background settlement worker started");

    // Store references for middleware and handlers
    let config_for_middleware = config.clone();
    let facilitator_for_middleware = facilitator_client.clone();
    let nonce_tracker_for_middleware = nonce_tracker.clone();
    let settlement_queue_for_middleware = settlement_queue.clone();
    let rate_limiter_for_middleware = rate_limiter.clone();
    let settlement_queue_for_app = settlement_queue.clone();
    let worker_metrics_for_app = worker_metrics.clone();

    // Clone shutdown_tx for the shutdown handler
    let shutdown_tx_clone = shutdown_tx.clone();

    let server = HttpServer::new(move || {
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
            // Share config and Kimi client across handlers
            .app_data(web::Data::new(config_for_middleware.clone()))
            .app_data(web::Data::new(kimi_client.clone()))
            // Share settlement queue and metrics for observability
            .app_data(web::Data::new(settlement_queue_for_app.clone()))
            .app_data(web::Data::new(worker_metrics_for_app.clone()))
            // Public endpoints (no payment required)
            .route("/", web::get().to(root_handler))
            .route("/health", web::get().to(health_handler))
            .route("/metrics", web::get().to(metrics_handler))
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
                        settlement_queue_for_middleware.clone(),
                        rate_limiter_for_middleware.clone(),
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
                        settlement_queue_for_middleware.clone(),
                        rate_limiter_for_middleware.clone(),
                    ))
                    .route("/completions", web::post().to(chat_handler)),
            )
    })
    .bind(("0.0.0.0", port))?
    .run();

    // Run server and handle graceful shutdown
    let result = server.await;

    // Signal worker to shut down
    info!("Server stopping, signaling settlement worker to shut down...");
    let _ = shutdown_tx_clone.send(());

    // Give the worker a moment to finish any in-flight settlement
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Log final queue state
    let remaining = settlement_queue.len();
    if remaining > 0 {
        warn!(
            "Shutting down with {} pending settlements in queue (will be lost)",
            remaining
        );
    }

    let (total, success, failure, retries) = worker_metrics.get_stats();
    info!(
        "Final settlement stats: total={}, success={}, failure={}, retries={}",
        total, success, failure, retries
    );

    result
}
