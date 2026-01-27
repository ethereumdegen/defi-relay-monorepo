use crate::config::{Config, NetworkRegistry, HEAVY_METHODS};
use crate::models::{BASE_NETWORK, USDC_BASE_ADDRESS};
use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::json;
use tracing::debug;

/// x402 discovery document endpoint
/// Returns payment requirements and resource information
pub async fn x402_discovery_handler(
    req: HttpRequest,
    registry: web::Data<NetworkRegistry>,
    config: web::Data<Config>,
) -> HttpResponse {
    debug!("Serving .well-known/x402");

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

    // Build resources list with all endpoints
    let mut resources = Vec::new();

    for network in registry.list_networks() {
        // Light endpoint resource
        resources.push(json!({
            "url": format!("{}/rpc/light/{}", base_url, network),
            "description": format!("Light JSON-RPC endpoint for {} - standard methods like eth_blockNumber, eth_call, eth_getBalance", network),
            "mimeType": "application/json",
            "pricing": {
                "amount": config.cost_per_request.to_string(),
                "asset": USDC_BASE_ADDRESS,
                "network": BASE_NETWORK
            }
        }));

        // Heavy endpoint resource
        resources.push(json!({
            "url": format!("{}/rpc/heavy/{}", base_url, network),
            "description": format!("Heavy JSON-RPC endpoint for {} - eth_getLogs, debug_trace*, trace_*", network),
            "mimeType": "application/json",
            "pricing": {
                "amount": config.cost_per_heavy_request.to_string(),
                "asset": USDC_BASE_ADDRESS,
                "network": BASE_NETWORK
            }
        }));
    }

    let discovery = json!({
        "x402Version": 2,
        "payTo": config.wallet_address.to_hex(),
        "accepts": [
            {
                "scheme": "exact",
                "network": BASE_NETWORK,
                "asset": USDC_BASE_ADDRESS,
                "maxTimeoutSeconds": 60,
                "extra": {}
            }
        ],
        "resources": resources,
        "tiers": {
            "light": {
                "amount": config.cost_per_request.to_string(),
                "description": "Standard RPC methods",
                "pathPattern": "/rpc/light/{network}",
                "exampleMethods": ["eth_blockNumber", "eth_getBalance", "eth_call", "eth_estimateGas", "eth_sendRawTransaction"]
            },
            "heavy": {
                "amount": config.cost_per_heavy_request.to_string(),
                "description": "Heavy RPC methods requiring more compute",
                "pathPattern": "/rpc/heavy/{network}",
                "methods": HEAVY_METHODS
            }
        },
        "extensions": {
            "jsonrpc": {
                "info": {
                    "input": {
                        "jsonrpc": "2.0",
                        "method": "eth_blockNumber",
                        "params": [],
                        "id": 1
                    },
                    "output": {
                        "jsonrpc": "2.0",
                        "id": 1,
                        "result": "0x1234567"
                    }
                },
                "schema": {
                    "type": "object",
                    "properties": {
                        "jsonrpc": {
                            "type": "string",
                            "const": "2.0"
                        },
                        "method": {
                            "type": "string",
                            "description": "The RPC method to call"
                        },
                        "params": {
                            "type": "array",
                            "description": "Method parameters"
                        },
                        "id": {
                            "type": ["integer", "string"],
                            "description": "Request identifier"
                        }
                    },
                    "required": ["jsonrpc", "method", "id"]
                }
            }
        }
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(discovery)
}
