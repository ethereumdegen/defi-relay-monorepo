use crate::error::AppError;
use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::{error, info, warn};

const MAX_RETRIES: u32 = 3;

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

    fn create_backoff() -> ExponentialBackoff {
        ExponentialBackoff {
            initial_interval: Duration::from_millis(500),
            max_interval: Duration::from_secs(4),
            max_elapsed_time: Some(Duration::from_secs(15)),
            ..ExponentialBackoff::default()
        }
    }

    fn is_transient_error(err: &reqwest::Error) -> bool {
        err.is_timeout() || err.is_connect() || err.is_request()
    }

    fn is_transient_status(status: reqwest::StatusCode) -> bool {
        status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    }

    /// Forward a request to a 0x swap API endpoint with retries
    async fn forward_request(&self, path: &str, query_string: &str) -> Result<Value, AppError> {
        let url = format!("{}/{}?{}", self.base_url, path, query_string);
        info!("0x API request: GET {}", url);

        let mut backoff = Self::create_backoff();
        let mut attempts: u32 = 0;

        loop {
            attempts += 1;

            let result = self
                .client
                .get(&url)
                .header("0x-api-key", &self.api_key)
                .header("0x-version", "v2")
                .send()
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        let body = response.text().await.map_err(|e| {
                            error!("Failed to read 0x response body: {}", e);
                            AppError::ZeroX(format!("Failed to read 0x response: {}", e))
                        })?;

                        info!("0x API response: status={}, body={}", status, body);

                        let result: Value = serde_json::from_str(&body).map_err(|e| {
                            error!("Failed to parse 0x response: {}", e);
                            AppError::ZeroX(format!("Invalid 0x response: {}", e))
                        })?;

                        return Ok(result);
                    }

                    let body = response.text().await.unwrap_or_default();
                    info!("0x API response: status={}, body={}", status, body);

                    if Self::is_transient_status(status) && attempts <= MAX_RETRIES {
                        warn!(
                            "0x API returned transient error: {} - {} (attempt {}/{})",
                            status, body, attempts, MAX_RETRIES
                        );
                        if let Some(duration) = backoff.next_backoff() {
                            tokio::time::sleep(duration).await;
                            continue;
                        }
                    }

                    error!("0x API returned error: {} - {}", status, body);
                    return Err(AppError::ZeroX(format!(
                        "0x API returned {}: {}",
                        status, body
                    )));
                }
                Err(e) => {
                    if Self::is_transient_error(&e) && attempts <= MAX_RETRIES {
                        warn!(
                            "Transient error connecting to 0x API: {} (attempt {}/{})",
                            e, attempts, MAX_RETRIES
                        );
                        if let Some(duration) = backoff.next_backoff() {
                            tokio::time::sleep(duration).await;
                            continue;
                        }
                    }

                    error!("Failed to send request to 0x: {}", e);
                    return Err(AppError::ZeroX(format!(
                        "Failed to connect to 0x API: {}",
                        e
                    )));
                }
            }
        }
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
