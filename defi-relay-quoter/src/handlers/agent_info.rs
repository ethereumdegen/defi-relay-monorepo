use crate::config::Config;
use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::json;
use tracing::debug;

/// EIP-8004 agent registration endpoint
/// Returns agent metadata for discovery
pub async fn agent_info_handler(req: HttpRequest, config: web::Data<Config>) -> HttpResponse {
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

    let agent_info = json!({
        "type": "https://eips.ethereum.org/EIPS/eip-8004#registration-v1",
        "name": "defi-relay-quoter",
        "description": "Pay-per-use 0x swap quotes via x402 - get permit2 swap quotes for any supported chain",
        "services": [
            {
                "name": "0x-swap-quote",
                "description": format!("Get 0x swap permit2 quotes ({} raw USDC/request)", config.cost_per_request),
                "endpoint": format!("{}/swap/permit2/quote", base_url),
                "version": "v2",
                "methods": ["GET"]
            }
        ],
        "x402Support": true,
        "active": true,
        "supportedTrust": ["reputation"],
        "pricing": {
            "quote": {
                "amount": config.cost_per_request.to_string(),
                "asset": "USDC",
                "network": "Base",
                "description": "Per quote request"
            }
        }
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(agent_info)
}
