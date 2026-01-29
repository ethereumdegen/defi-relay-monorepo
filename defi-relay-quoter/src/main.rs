mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod services;

use actix_cors::Cors;
use actix_web::{http::header, web, App, HttpResponse, HttpServer};
use config::Config;
use handlers::{agent_info_handler, quote_handler, x402_discovery_handler};
use middleware::X402Middleware;
use services::{FacilitatorClient, NonceTracker, ZeroXClient};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// Root endpoint with usage instructions
async fn root_handler(config: web::Data<Config>) -> HttpResponse {
    HttpResponse::Ok().content_type("text/plain").body(format!(
        r#"DeFi Relay Quoter - Pay-per-use 0x Swap Quotes

This service provides access to 0x swap API quotes using the x402 payment protocol.

PRICING:
  {} raw USDC per quote request

ENDPOINT:
  GET /swap/permit2/quote - Get a swap quote from 0x

REQUIRED QUERY PARAMETERS:
  chainId     - Chain ID (e.g., 1 for Ethereum mainnet, 8453 for Base)
  sellToken   - Address of token to sell (use 0xeee...eee for native ETH)
  buyToken    - Address of token to buy
  sellAmount  - Amount of sellToken in smallest unit (wei)
  taker       - Address that will execute the swap

EXAMPLE REQUEST:
  GET /swap/permit2/quote?chainId=1&sellToken=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee&buyToken=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&sellAmount=1000000000000000000&taker=0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045
  X-PAYMENT: <base64-encoded-payment>

OTHER ENDPOINTS:
  GET  /                   - This help page
  GET  /health             - Health check
  GET  /agent.json         - EIP-8004 agent metadata
  GET  /.well-known/x402   - x402 discovery document

USAGE:
  1. Send a GET request to /swap/permit2/quote with required query parameters
  2. If no payment header is provided, you'll receive a 402 response
     with payment requirements in the "payment-required" header
  3. Create a payment using the x402 protocol and include it in
     the "X-PAYMENT" header
  4. The service will verify payment and forward your request to 0x

SUPPORTED CHAINS:
  - Ethereum (1)
  - Base (8453)
  - Arbitrum (42161)
  - Optimism (10)
  - Polygon (137)
  - Avalanche (43114)
  - BSC (56)

For more info on x402: https://www.x402.org
For 0x API docs: https://0x.org/docs/api
"#,
        config.cost_per_request
    ))
}

/// Health check endpoint
async fn health_handler() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "defi-relay-quoter"
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
    info!("Starting defi-relay-quoter on port {}", port);
    info!("Wallet address: {}", config.wallet_address);
    info!("Facilitator URL: {}", config.facilitator_url);
    info!("Cost per request: {} raw USDC", config.cost_per_request);
    info!("0x API base URL: {}", config.zerox_base_url);

    // Create service clients
    let facilitator_client = FacilitatorClient::new(&config.facilitator_url);
    let zerox_client = ZeroXClient::new(&config.zerox_base_url, &config.zerox_api_key);
    let nonce_tracker = Arc::new(NonceTracker::with_default_ttl());

    let config_for_app = config.clone();

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST", "OPTIONS"])
            .allowed_headers(vec![
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                header::HeaderName::from_static("x-payment"),
            ])
            .expose_headers(vec![header::HeaderName::from_static("payment-required")])
            .max_age(3600);

        // x402 middleware for the quote endpoint
        let quote_middleware = X402Middleware::with_cost(
            &config_for_app,
            config_for_app.cost_per_request,
            "0x swap quote",
            facilitator_client.clone(),
            nonce_tracker.clone(),
        );

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(config_for_app.clone()))
            .app_data(web::Data::new(zerox_client.clone()))
            // Public endpoints
            .route("/", web::get().to(root_handler))
            .route("/health", web::get().to(health_handler))
            .route("/agent.json", web::get().to(agent_info_handler))
            .route("/.well-known/x402", web::get().to(x402_discovery_handler))
            // Protected quote endpoint
            .service(
                web::scope("/swap/permit2")
                    .wrap(quote_middleware)
                    .route("/quote", web::get().to(quote_handler)),
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
