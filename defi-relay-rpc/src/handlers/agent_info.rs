use crate::config::{Config, NetworkRegistry, HEAVY_METHODS};
use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::json;
use tracing::debug;

/// EIP-8004 agent registration endpoint
/// Returns agent metadata for discovery
pub async fn agent_info_handler(
    req: HttpRequest,
    registry: web::Data<NetworkRegistry>,
    config: web::Data<Config>,
) -> HttpResponse {
    debug!("Serving agent.json");

    let host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");

    let scheme = if host.contains("localhost") || host.starts_with("127.") {
        "http"
    } else {
        "https"
    };

    let base_url = format!("{}://{}", scheme, host);

    // Build services list with all network endpoints
    let mut services = Vec::new();

    for network in registry.list_networks() {
        // Light endpoint
        services.push(json!({
            "name": format!("EVM-RPC-Light-{}", network),
            "description": format!("Light JSON-RPC for {} ({} raw USDC/request)", network, config.cost_per_request),
            "endpoint": format!("{}/rpc/light/{}", base_url, network),
            "version": "1.0.0",
            "methods": ["eth_blockNumber", "eth_getBalance", "eth_call", "eth_estimateGas", "eth_sendRawTransaction", "eth_getTransactionReceipt", "..."]
        }));

        // Heavy endpoint
        services.push(json!({
            "name": format!("EVM-RPC-Heavy-{}", network),
            "description": format!("Heavy JSON-RPC for {} ({} raw USDC/request)", network, config.cost_per_heavy_request),
            "endpoint": format!("{}/rpc/heavy/{}", base_url, network),
            "version": "1.0.0",
            "methods": HEAVY_METHODS
        }));
    }

    let agent_info = json!({
        "type": "https://eips.ethereum.org/EIPS/eip-8004#registration-v1",
        "name": "defi-relay-rpc",
        "description": "Pay-per-use EVM RPC access via x402 - supports multiple networks with tiered pricing",
        "services": services,
        "x402Support": true,
        "active": true,
        "supportedTrust": ["reputation"],
        "pricing": {
            "light": {
                "amount": config.cost_per_request.to_string(),
                "asset": "USDC",
                "network": "Base",
                "description": "Standard RPC methods"
            },
            "heavy": {
                "amount": config.cost_per_heavy_request.to_string(),
                "asset": "USDC",
                "network": "Base",
                "description": "Heavy methods (eth_getLogs, traces, etc.)"
            }
        }
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(agent_info)
}
