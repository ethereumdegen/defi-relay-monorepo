use crate::config::Config;
use crate::models::{BASE_NETWORK, USDC_BASE_ADDRESS};
use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::json;
use tracing::debug;

/// x402 discovery document endpoint
/// Returns payment requirements and resource information
pub async fn x402_discovery_handler(req: HttpRequest, config: web::Data<Config>) -> HttpResponse {
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
        "resources": [
            {
                "url": format!("{}/swap/permit2/quote", base_url),
                "description": "0x swap permit2 quote endpoint - returns swap quotes for any supported chain",
                "mimeType": "application/json",
                "pricing": {
                    "amount": config.cost_per_request.to_string(),
                    "asset": USDC_BASE_ADDRESS,
                    "network": BASE_NETWORK
                }
            }
        ],
        "extensions": {
            "0x-swap": {
                "info": {
                    "input": {
                        "method": "GET",
                        "path": "/swap/permit2/quote",
                        "queryParams": {
                            "chainId": "1",
                            "sellToken": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                            "buyToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                            "sellAmount": "1000000000000000000",
                            "taker": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
                        }
                    },
                    "output": "0x swap quote response with permit2 transaction data"
                },
                "requiredParams": ["chainId", "sellToken", "buyToken", "sellAmount", "taker"],
                "supportedChains": [1, 8453, 42161, 10, 137, 43114, 56]
            }
        }
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(discovery)
}
