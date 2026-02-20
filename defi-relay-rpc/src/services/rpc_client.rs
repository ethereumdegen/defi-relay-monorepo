use crate::error::AppError;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::{error, info, warn};

/// Client for forwarding JSON-RPC requests to upstream EVM nodes
/// Supports multiple endpoints with automatic fallback
#[derive(Clone)]
pub struct RpcClient {
    client: Client,
    endpoints: Vec<String>,
}

impl RpcClient {
    pub fn new(endpoint: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to build HTTP client");

        RpcClient {
            client,
            endpoints: vec![endpoint.to_string()],
        }
    }

    pub fn with_fallbacks(endpoints: Vec<String>) -> Self {
        assert!(!endpoints.is_empty(), "At least one RPC endpoint is required");
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to build HTTP client");

        RpcClient { client, endpoints }
    }

    /// Forward a JSON-RPC request to the upstream node, trying fallbacks on failure
    pub async fn forward(&self, request: Value) -> Result<Value, AppError> {
        let mut last_error = None;

        for (i, endpoint) in self.endpoints.iter().enumerate() {
            info!("RPC request to {}: {}", endpoint, request);

            let result = self
                .client
                .post(endpoint)
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
                .await;

            let response = match result {
                Ok(r) => r,
                Err(e) => {
                    warn!("RPC endpoint {} failed to connect: {}", endpoint, e);
                    last_error = Some(format!("Failed to connect to {}: {}", endpoint, e));
                    continue;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                warn!("RPC endpoint {} returned error: {} - {}", endpoint, status, body);
                last_error = Some(format!("{} returned {}: {}", endpoint, status, body));
                if i + 1 < self.endpoints.len() {
                    warn!("Falling back to next RPC endpoint");
                }
                continue;
            }

            let result: Value = response.json().await.map_err(|e| {
                error!("Failed to parse RPC response from {}: {}", endpoint, e);
                AppError::Rpc(format!("Invalid RPC response: {}", e))
            })?;

            info!("RPC response from {}: {}", endpoint, result);
            return Ok(result);
        }

        let err_msg = last_error.unwrap_or_else(|| "No RPC endpoints configured".to_string());
        error!("All RPC endpoints failed. Last error: {}", err_msg);
        Err(AppError::Rpc(format!("All RPC endpoints failed: {}", err_msg)))
    }
}
