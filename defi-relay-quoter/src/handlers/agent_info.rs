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
        "description": "Pay-per-use 0x swap API via x402 - get swap prices and quotes for any supported chain",
        "services": [
            {
                "name": "0x-permit2-price",
                "description": format!("Get 0x permit2 indicative price ({} raw USDC/request)", config.cost_per_price),
                "endpoint": format!("{}/swap/permit2/price", base_url),
                "version": "v2",
                "methods": ["GET"]
            },
            {
                "name": "0x-permit2-quote",
                "description": format!("Get 0x permit2 full quote ({} raw USDC/request)", config.cost_per_quote),
                "endpoint": format!("{}/swap/permit2/quote", base_url),
                "version": "v2",
                "methods": ["GET"]
            },
            {
                "name": "0x-allowance-holder-price",
                "description": format!("Get 0x allowance-holder indicative price ({} raw USDC/request) - recommended", config.cost_per_price),
                "endpoint": format!("{}/swap/allowance-holder/price", base_url),
                "version": "v2",
                "methods": ["GET"]
            },
            {
                "name": "0x-allowance-holder-quote",
                "description": format!("Get 0x allowance-holder full quote ({} raw USDC/request) - recommended", config.cost_per_quote),
                "endpoint": format!("{}/swap/allowance-holder/quote", base_url),
                "version": "v2",
                "methods": ["GET"]
            }
        ],
        "x402Support": true,
        "active": true,
        "supportedTrust": ["reputation"],
        "pricing": {
            "price": {
                "amount": config.cost_per_price.to_string(),
                "asset": "USDC",
                "network": "Base",
                "description": "Indicative price (lightweight)"
            },
            "quote": {
                "amount": config.cost_per_quote.to_string(),
                "asset": "USDC",
                "network": "Base",
                "description": "Full quote with transaction data"
            }
        }
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(agent_info)
}
