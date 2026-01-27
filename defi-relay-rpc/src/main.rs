mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod services;

use actix_cors::Cors;
use actix_web::{http::header, web, App, HttpResponse, HttpServer};
use config::{load_networks_from_env, Config, NetworkRegistry, HEAVY_METHODS};
use handlers::{agent_info_handler, heavy_rpc_handler, light_rpc_handler, x402_discovery_handler};
use middleware::X402Middleware;
use services::{FacilitatorClient, NonceTracker};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// Root endpoint with usage instructions
async fn root_handler(
    registry: web::Data<NetworkRegistry>,
    config: web::Data<Config>,
) -> HttpResponse {
    let networks: Vec<String> = registry
        .list_networks()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let networks_list = if networks.is_empty() {
        "  (no networks configured)".to_string()
    } else {
        networks
            .iter()
            .map(|n| format!("  - /rpc/light/{0}\n  - /rpc/heavy/{0}", n))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let heavy_methods_list = HEAVY_METHODS
        .iter()
        .map(|m| format!("  - {}", m))
        .collect::<Vec<_>>()
        .join("\n");

    HttpResponse::Ok().content_type("text/plain").body(format!(
        r#"DeFi Relay RPC - Pay-per-request EVM RPC Access

This service provides access to EVM RPC nodes using the x402 payment protocol.

PRICING:
  Standard requests: {} raw USDC per request
  Heavy requests:    {} raw USDC per request

AVAILABLE NETWORKS:
{}

ENDPOINTS:
  GET  /                        - This help page
  GET  /health                  - Health check
  GET  /networks                - List available networks (JSON)
  GET  /agent.json              - EIP-8004 agent metadata
  GET  /.well-known/x402        - x402 discovery document
  POST /rpc/light/{{network}}     - Light JSON-RPC endpoint (non-heavy methods only)
  POST /rpc/heavy/{{network}}     - Heavy JSON-RPC endpoint (ALL methods supported)

HEAVY METHODS (require /rpc/heavy/{{network}}):
{}

USAGE:
  1. Choose your endpoint:
     - /rpc/light/{{network}} for standard methods (lower cost)
     - /rpc/heavy/{{network}} for heavy methods OR if you want a single endpoint for all methods
  2. Send a POST request to your chosen endpoint
  3. If no payment header is provided, you'll receive a 402 response
     with payment requirements in the "payment-required" header
  4. Create a payment using the x402 protocol and include it in
     the "X-PAYMENT" header
  5. The service will verify payment and forward your request to the RPC node

EXAMPLE REQUESTS:
  # Light request (standard cost)
  POST /rpc/light/mainnet
  Content-Type: application/json
  X-PAYMENT: <base64-encoded-payment>

  {{
    "jsonrpc": "2.0",
    "method": "eth_blockNumber",
    "params": [],
    "id": 1
  }}

  # Heavy request (higher cost)
  POST /rpc/heavy/mainnet
  Content-Type: application/json
  X-PAYMENT: <base64-encoded-payment>

  {{
    "jsonrpc": "2.0",
    "method": "eth_getLogs",
    "params": [{{"fromBlock": "0x0", "toBlock": "latest"}}],
    "id": 1
  }}

For more info on x402: https://www.x402.org
"#,
        config.cost_per_request,
        config.cost_per_heavy_request,
        networks_list,
        heavy_methods_list
    ))
}

/// Health check endpoint
async fn health_handler() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "defi-relay-rpc"
    }))
}

/// List available networks
async fn networks_handler(registry: web::Data<NetworkRegistry>) -> HttpResponse {
    let networks: Vec<String> = registry.list_networks().iter().map(|s| s.to_string()).collect();
    HttpResponse::Ok().json(serde_json::json!({
        "networks": networks
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

    // Load network registry
    let registry = load_networks_from_env();
    let network_count = registry.list_networks().len();

    if network_count == 0 {
        error!("No networks configured! Set environment variables like MAINNET_RPC_URL, BASE_RPC_URL, etc.");
        std::process::exit(1);
    }

    let port = config.port;
    info!("Starting defi-relay-rpc on port {}", port);
    info!("Wallet address: {}", config.wallet_address);
    info!("Facilitator URL: {}", config.facilitator_url);
    info!("Cost per request: {} raw USDC", config.cost_per_request);
    info!(
        "Cost per heavy request: {} raw USDC",
        config.cost_per_heavy_request
    );
    info!("Configured networks: {}", network_count);

    for network in registry.list_networks() {
        info!("  - /{}", network);
    }

    // Create service clients
    let facilitator_client = FacilitatorClient::new(&config.facilitator_url);
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

        // Standard cost middleware
        let standard_middleware = X402Middleware::with_cost(
            &config_for_app,
            config_for_app.cost_per_request,
            "EVM RPC access",
            facilitator_client.clone(),
            nonce_tracker.clone(),
        );

        // Heavy cost middleware
        let heavy_middleware = X402Middleware::with_cost(
            &config_for_app,
            config_for_app.cost_per_heavy_request,
            "Heavy EVM RPC access (getLogs, traces, etc.)",
            facilitator_client.clone(),
            nonce_tracker.clone(),
        );

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(registry.clone()))
            .app_data(web::Data::new(config_for_app.clone()))
            // Public endpoints
            .route("/", web::get().to(root_handler))
            .route("/health", web::get().to(health_handler))
            .route("/networks", web::get().to(networks_handler))
            .route("/agent.json", web::get().to(agent_info_handler))
            .route("/.well-known/x402", web::get().to(x402_discovery_handler))
            // RPC endpoints under /rpc scope
            .service(
                web::scope("/rpc")
                    // Light RPC endpoint (standard cost, non-heavy methods only)
                    .service(
                        web::scope("/light/{network}")
                            .wrap(standard_middleware)
                            .route("", web::post().to(light_rpc_handler)),
                    )
                    // Heavy RPC endpoint (higher cost, supports ALL methods)
                    .service(
                        web::scope("/heavy/{network}")
                            .wrap(heavy_middleware)
                            .route("", web::post().to(heavy_rpc_handler)),
                    ),
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
