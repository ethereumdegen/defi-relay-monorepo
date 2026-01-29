use crate::error::AppError;
use crate::models::{
    PaymentPayload, SettleRequest, SettleResponse, VerifyPaymentRequirements, VerifyRequest,
    VerifyResponse, X402_VERSION,
};
use backoff::{backoff::Backoff, ExponentialBackoff};
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Maximum number of retry attempts for transient failures
const MAX_RETRIES: u32 = 3;

/// Configuration for the facilitator client
#[derive(Clone)]
pub struct FacilitatorConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial retry interval
    pub initial_interval: Duration,
    /// Maximum retry interval
    pub max_interval: Duration,
    /// Request timeout
    pub request_timeout: Duration,
}

impl Default for FacilitatorConfig {
    fn default() -> Self {
        Self {
            max_retries: MAX_RETRIES,
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(2),
            request_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Clone)]
pub struct FacilitatorClient {
    client: Client,
    base_url: String,
    config: FacilitatorConfig,
}

impl FacilitatorClient {
    pub fn new(base_url: &str) -> Self {
        Self::with_config(base_url, FacilitatorConfig::default())
    }

    pub fn with_config(base_url: &str, config: FacilitatorConfig) -> Self {
        let client = Client::builder()
            .timeout(config.request_timeout)
            .build()
            .expect("Failed to build HTTP client");

        FacilitatorClient {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            config,
        }
    }

    fn create_backoff(&self) -> ExponentialBackoff {
        ExponentialBackoff {
            initial_interval: self.config.initial_interval,
            max_interval: self.config.max_interval,
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        }
    }

    /// Check if an error is transient and worth retrying
    fn is_transient_error(err: &reqwest::Error) -> bool {
        err.is_timeout() || err.is_connect() || err.is_request()
    }

    /// Check if an HTTP status code is transient and worth retrying
    fn is_transient_status(status: reqwest::StatusCode) -> bool {
        status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    }

    /// Verify a payment payload with the facilitator service (with retries)
    pub async fn verify(
        &self,
        payment_payload: PaymentPayload,
        payment_requirements: VerifyPaymentRequirements,
    ) -> Result<VerifyResponse, AppError> {
        let url = format!("{}/verify", self.base_url);

        let request = VerifyRequest {
            x402_version: X402_VERSION,
            payment_payload,
            payment_requirements,
        };

        debug!("Sending verify request to facilitator: {}", url);

        let mut backoff = self.create_backoff();
        let mut attempts = 0;
        let mut last_error: Option<AppError> = None;

        loop {
            attempts += 1;

            let result = self
                .client
                .post(&url)
                .json(&request)
                .send()
                .await;

            match result {
                Ok(response) => {
                    if response.status().is_success() {
                        let verify_response: VerifyResponse =
                            response.json().await.map_err(|e| {
                                error!("Failed to parse facilitator response: {}", e);
                                AppError::Facilitator(format!("Invalid response: {}", e))
                            })?;

                        if verify_response.is_valid {
                            info!("Payment verified for payer: {:?}", verify_response.payer);
                        } else {
                            info!(
                                "Payment rejected: {:?}",
                                verify_response.invalid_reason
                            );
                        }

                        return Ok(verify_response);
                    }

                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();

                    // Check if this is a transient error worth retrying
                    if Self::is_transient_status(status) && attempts <= self.config.max_retries {
                        warn!(
                            "Facilitator returned transient error: {} - {} (attempt {}/{})",
                            status, body, attempts, self.config.max_retries
                        );
                        last_error = Some(AppError::Facilitator(format!(
                            "Facilitator returned {}: {}",
                            status, body
                        )));

                        if let Some(duration) = backoff.next_backoff() {
                            tokio::time::sleep(duration).await;
                            continue;
                        }
                    }

                    // Non-transient error or max retries exceeded
                    error!("Facilitator returned error: {} - {}", status, body);
                    return Err(AppError::Facilitator(format!(
                        "Facilitator returned {}: {}",
                        status, body
                    )));
                }
                Err(e) => {
                    if Self::is_transient_error(&e) && attempts <= self.config.max_retries {
                        warn!(
                            "Transient error connecting to facilitator: {} (attempt {}/{})",
                            e, attempts, self.config.max_retries
                        );
                        last_error = Some(AppError::Facilitator(format!("Connection failed: {}", e)));

                        if let Some(duration) = backoff.next_backoff() {
                            tokio::time::sleep(duration).await;
                            continue;
                        }
                    }

                    error!("Failed to connect to facilitator: {}", e);
                    return Err(last_error.unwrap_or_else(|| {
                        AppError::Facilitator(format!("Connection failed: {}", e))
                    }));
                }
            }
        }
    }

    /// Settle a verified payment with the facilitator service (with retries)
    pub async fn settle(
        &self,
        payment_payload: PaymentPayload,
        payment_requirements: VerifyPaymentRequirements,
    ) -> Result<SettleResponse, AppError> {
        let url = format!("{}/settle", self.base_url);

        let request = SettleRequest {
            x402_version: X402_VERSION,
            payment_payload,
            payment_requirements,
        };

        debug!("Sending settle request to facilitator: {}", url);

        let mut backoff = self.create_backoff();
        let mut attempts = 0;
        let mut last_error: Option<AppError> = None;

        loop {
            attempts += 1;

            let result = self
                .client
                .post(&url)
                .json(&request)
                .send()
                .await;

            match result {
                Ok(response) => {
                    if response.status().is_success() {
                        let settle_response: SettleResponse =
                            response.json().await.map_err(|e| {
                                error!("Failed to parse facilitator settlement response: {}", e);
                                AppError::Facilitator(format!("Invalid settlement response: {}", e))
                            })?;

                        if settle_response.success {
                            info!(
                                "Payment settled successfully. Tx: {:?}, Payer: {:?}",
                                settle_response.transaction, settle_response.payer
                            );
                        } else {
                            warn!(
                                "Payment settlement failed: {:?}",
                                settle_response.error_reason
                            );
                        }

                        return Ok(settle_response);
                    }

                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();

                    // Check if this is a transient error worth retrying
                    if Self::is_transient_status(status) && attempts <= self.config.max_retries {
                        warn!(
                            "Facilitator settlement returned transient error: {} - {} (attempt {}/{})",
                            status, body, attempts, self.config.max_retries
                        );
                        last_error = Some(AppError::Facilitator(format!(
                            "Settlement failed with {}: {}",
                            status, body
                        )));

                        if let Some(duration) = backoff.next_backoff() {
                            tokio::time::sleep(duration).await;
                            continue;
                        }
                    }

                    // Non-transient error or max retries exceeded
                    error!("Facilitator settlement returned error: {} - {}", status, body);
                    return Err(AppError::Facilitator(format!(
                        "Settlement failed with {}: {}",
                        status, body
                    )));
                }
                Err(e) => {
                    if Self::is_transient_error(&e) && attempts <= self.config.max_retries {
                        warn!(
                            "Transient error connecting to facilitator for settlement: {} (attempt {}/{})",
                            e, attempts, self.config.max_retries
                        );
                        last_error = Some(AppError::Facilitator(format!(
                            "Settlement connection failed: {}",
                            e
                        )));

                        if let Some(duration) = backoff.next_backoff() {
                            tokio::time::sleep(duration).await;
                            continue;
                        }
                    }

                    error!("Failed to connect to facilitator for settlement: {}", e);
                    return Err(last_error.unwrap_or_else(|| {
                        AppError::Facilitator(format!("Settlement connection failed: {}", e))
                    }));
                }
            }
        }
    }
}
