use crate::error::AppError;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, error};

/// Client for forwarding JSON-RPC requests to upstream EVM nodes
#[derive(Clone)]
pub struct RpcClient {
    client: Client,
    endpoint: String,
}

impl RpcClient {
    pub fn new(endpoint: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to build HTTP client");

        RpcClient {
            client,
            endpoint: endpoint.to_string(),
        }
    }

    /// Forward a JSON-RPC request to the upstream node
    pub async fn forward(&self, request: Value) -> Result<Value, AppError> {
        debug!("Forwarding RPC request to {}", self.endpoint);

        let response = self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send RPC request: {}", e);
                AppError::Rpc(format!("Failed to connect to RPC node: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("RPC node returned error: {} - {}", status, body);
            return Err(AppError::Rpc(format!("RPC node returned {}: {}", status, body)));
        }

        let result: Value = response.json().await.map_err(|e| {
            error!("Failed to parse RPC response: {}", e);
            AppError::Rpc(format!("Invalid RPC response: {}", e))
        })?;

        Ok(result)
    }
}
