mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod services;

use actix_cors::Cors;
use actix_web::{http::header, web, App, HttpResponse, HttpServer};
use config::Config;
use handlers::{
    agent_info_handler, allowance_holder_price_handler, allowance_holder_quote_handler,
    permit2_price_handler, permit2_quote_handler, x402_discovery_handler,
};
use middleware::X402Middleware;
use services::{FacilitatorClient, NonceTracker, ZeroXClient};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// Root endpoint with usage instructions
async fn root_handler(config: web::Data<Config>) -> HttpResponse {
    HttpResponse::Ok().content_type("text/plain").body(format!(
        r#"DeFi Relay Quoter - Pay-per-use 0x Swap API

This service provides access to 0x swap API using the x402 payment protocol.

PRICING:
  Price endpoints (indicative):  {} raw USDC per request
  Quote endpoints (full quote):  {} raw USDC per request

ENDPOINTS:

  Permit2 (requires Permit2 approval):
    GET /swap/permit2/price  - Get indicative price
    GET /swap/permit2/quote  - Get full quote with transaction data

  AllowanceHolder (single signature, recommended):
    GET /swap/allowance-holder/price  - Get indicative price
    GET /swap/allowance-holder/quote  - Get full quote with transaction data

REQUIRED QUERY PARAMETERS:
  chainId     - Chain ID (e.g., 1 for Ethereum, 8453 for Base)
  sellToken   - Address of token to sell (use 0xeee...eee for native ETH)
  buyToken    - Address of token to buy
  sellAmount  - Amount to sell in smallest unit (wei) - OR use buyAmount
  taker       - Address that will execute the swap

OPTIONAL PARAMETERS:
  buyAmount        - Amount to buy (alternative to sellAmount)
  slippageBps      - Slippage tolerance in basis points (e.g., 100 = 1%)
  excludedSources  - Liquidity sources to exclude
  includedSources  - Liquidity sources to include

EXAMPLE REQUEST:
  GET /swap/permit2/quote?chainId=1&sellToken=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee&buyToken=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&sellAmount=1000000000000000000&taker=0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045
  Header: X-PAYMENT: <base64-encoded-x402-payment>

OTHER ENDPOINTS:
  GET  /                   - This help page
  GET  /health             - Health check
  GET  /agent.json         - EIP-8004 agent metadata
  GET  /.well-known/x402   - x402 discovery document

PERMIT2 vs ALLOWANCE-HOLDER:
  - AllowanceHolder: Single signature, better UX, lower gas. Recommended for most use cases.
  - Permit2: Universal standard, shared approvals across apps, requires two signatures.

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
        config.cost_per_price, config.cost_per_quote
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
    info!("Cost per price: {} raw USDC", config.cost_per_price);
    info!("Cost per quote: {} raw USDC", config.cost_per_quote);
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
            .expose_headers(vec![
                header::HeaderName::from_static("payment-required"),
                header::HeaderName::from_static("payment-response"),
            ])
            .max_age(3600);

        // Price middleware (lower cost for indicative prices)
        let price_middleware = X402Middleware::with_cost(
            &config_for_app,
            config_for_app.cost_per_price,
            "0x swap price (indicative)",
            facilitator_client.clone(),
            nonce_tracker.clone(),
        );

        // Quote middleware (higher cost for full quotes)
        let quote_middleware = X402Middleware::with_cost(
            &config_for_app,
            config_for_app.cost_per_quote,
            "0x swap quote (full)",
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
            // Permit2 endpoints
            .service(
                web::scope("/swap/permit2")
                    .service(
                        web::resource("/price")
                            .wrap(price_middleware.clone())
                            .route(web::get().to(permit2_price_handler)),
                    )
                    .service(
                        web::resource("/quote")
                            .wrap(quote_middleware.clone())
                            .route(web::get().to(permit2_quote_handler)),
                    ),
            )
            // AllowanceHolder endpoints
            .service(
                web::scope("/swap/allowance-holder")
                    .service(
                        web::resource("/price")
                            .wrap(price_middleware)
                            .route(web::get().to(allowance_holder_price_handler)),
                    )
                    .service(
                        web::resource("/quote")
                            .wrap(quote_middleware)
                            .route(web::get().to(allowance_holder_quote_handler)),
                    ),
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
