use actix_web::{HttpRequest, HttpResponse};
use serde_json::json;
use tracing::debug;

/// EIP-8004 agent registration endpoint
/// Returns agent metadata for discovery
pub async fn agent_info_handler(req: HttpRequest) -> HttpResponse {
    debug!("Serving agent.json");

    // Get the host from the request to build the endpoint URL
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

    let endpoint = format!("{}://{}/chat", scheme, host);

    let agent_info = json!({
        "type": "https://eips.ethereum.org/EIPS/eip-8004#registration-v1",
        "name": "x402-kimi-bot",
        "description": "Pay-per-use Kimi AI (Moonshot) via x402",
        "services": [{
            "name": "A2A",
            "endpoint": endpoint,
            "version": "1.0.0"
        }],
        "x402Support": true,
        "active": true,
        "supportedTrust": ["reputation"]
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(agent_info)
}
