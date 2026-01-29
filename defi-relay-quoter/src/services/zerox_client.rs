use crate::error::AppError;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, error, info};

/// Client for forwarding requests to 0x swap API
#[derive(Clone)]
pub struct ZeroXClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl ZeroXClient {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to build HTTP client");

        ZeroXClient {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    /// Forward a request to a 0x swap API endpoint
    async fn forward_request(&self, path: &str, query_string: &str) -> Result<Value, AppError> {
        let url = format!("{}/{}?{}", self.base_url, path, query_string);
        debug!("Forwarding request to 0x: {}", url);

        let response = self
            .client
            .get(&url)
            .header("0x-api-key", &self.api_key)
            .header("0x-version", "v2")
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send request to 0x: {}", e);
                AppError::ZeroX(format!("Failed to connect to 0x API: {}", e))
            })?;

        let status = response.status();
        info!("0x API response status: {}", status);

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            error!("0x API returned error: {} - {}", status, body);
            return Err(AppError::ZeroX(format!(
                "0x API returned {}: {}",
                status, body
            )));
        }

        let result: Value = response.json().await.map_err(|e| {
            error!("Failed to parse 0x response: {}", e);
            AppError::ZeroX(format!("Invalid 0x response: {}", e))
        })?;

        debug!("0x response received successfully");
        Ok(result)
    }

    /// Get permit2 price (indicative, read-only)
    pub async fn get_permit2_price(&self, query_string: &str) -> Result<Value, AppError> {
        self.forward_request("swap/permit2/price", query_string).await
    }

    /// Get permit2 quote (full quote with transaction data)
    pub async fn get_permit2_quote(&self, query_string: &str) -> Result<Value, AppError> {
        self.forward_request("swap/permit2/quote", query_string).await
    }

    /// Get allowance-holder price (indicative, read-only)
    pub async fn get_allowance_holder_price(&self, query_string: &str) -> Result<Value, AppError> {
        self.forward_request("swap/allowance-holder/price", query_string).await
    }

    /// Get allowance-holder quote (full quote with transaction data)
    pub async fn get_allowance_holder_quote(&self, query_string: &str) -> Result<Value, AppError> {
        self.forward_request("swap/allowance-holder/quote", query_string).await
    }
}
