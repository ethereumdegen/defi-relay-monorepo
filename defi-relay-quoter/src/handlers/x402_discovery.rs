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
                "url": format!("{}/swap/permit2/price", base_url),
                "description": "0x permit2 indicative price - lightweight price check",
                "mimeType": "application/json",
                "pricing": {
                    "amount": config.cost_per_price.to_string(),
                    "asset": USDC_BASE_ADDRESS,
                    "network": BASE_NETWORK
                }
            },
            {
                "url": format!("{}/swap/permit2/quote", base_url),
                "description": "0x permit2 full quote - includes transaction data for execution",
                "mimeType": "application/json",
                "pricing": {
                    "amount": config.cost_per_quote.to_string(),
                    "asset": USDC_BASE_ADDRESS,
                    "network": BASE_NETWORK
                }
            },
            {
                "url": format!("{}/swap/allowance-holder/price", base_url),
                "description": "0x allowance-holder indicative price - lightweight price check (recommended)",
                "mimeType": "application/json",
                "pricing": {
                    "amount": config.cost_per_price.to_string(),
                    "asset": USDC_BASE_ADDRESS,
                    "network": BASE_NETWORK
                }
            },
            {
                "url": format!("{}/swap/allowance-holder/quote", base_url),
                "description": "0x allowance-holder full quote - single signature, better UX (recommended)",
                "mimeType": "application/json",
                "pricing": {
                    "amount": config.cost_per_quote.to_string(),
                    "asset": USDC_BASE_ADDRESS,
                    "network": BASE_NETWORK
                }
            }
        ],
        "tiers": {
            "price": {
                "amount": config.cost_per_price.to_string(),
                "description": "Indicative price - lightweight, read-only",
                "endpoints": ["/swap/permit2/price", "/swap/allowance-holder/price"]
            },
            "quote": {
                "amount": config.cost_per_quote.to_string(),
                "description": "Full quote with transaction data for execution",
                "endpoints": ["/swap/permit2/quote", "/swap/allowance-holder/quote"]
            }
        },
        "extensions": {
            "0x-swap": {
                "info": {
                    "input": {
                        "method": "GET",
                        "path": "/swap/allowance-holder/quote",
                        "queryParams": {
                            "chainId": "1",
                            "sellToken": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                            "buyToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                            "sellAmount": "1000000000000000000",
                            "taker": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
                        }
                    },
                    "output": "0x swap quote response with transaction data"
                },
                "requiredParams": ["chainId", "sellToken", "buyToken", "taker"],
                "amountParams": "One of: sellAmount OR buyAmount",
                "optionalParams": ["slippageBps", "excludedSources", "includedSources"],
                "supportedChains": [1, 8453, 42161, 10, 137, 43114, 56],
                "methods": {
                    "permit2": {
                        "description": "Requires Permit2 approval, universal standard with shared approvals",
                        "price": "/swap/permit2/price",
                        "quote": "/swap/permit2/quote"
                    },
                    "allowanceHolder": {
                        "description": "Single signature, better UX, lower gas - recommended for most use cases",
                        "price": "/swap/allowance-holder/price",
                        "quote": "/swap/allowance-holder/quote"
                    }
                }
            }
        }
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(discovery)
}
