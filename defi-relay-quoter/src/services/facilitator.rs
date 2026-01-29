use crate::error::AppError;
use crate::models::{
    PaymentPayload, SettleRequest, SettleResponse, VerifyPaymentRequirements, VerifyRequest,
    VerifyResponse, X402_VERSION,
};
use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const MAX_RETRIES: u32 = 3;

#[derive(Clone)]
pub struct FacilitatorClient {
    client: Client,
    base_url: String,
    max_retries: u32,
    initial_interval: Duration,
    max_interval: Duration,
}

impl FacilitatorClient {
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        FacilitatorClient {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            max_retries: MAX_RETRIES,
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(2),
        }
    }

    fn create_backoff(&self) -> ExponentialBackoff {
        ExponentialBackoff {
            initial_interval: self.initial_interval,
            max_interval: self.max_interval,
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        }
    }

    fn is_transient_error(err: &reqwest::Error) -> bool {
        err.is_timeout() || err.is_connect() || err.is_request()
    }

    fn is_transient_status(status: reqwest::StatusCode) -> bool {
        status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    }

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

            let result = self.client.post(&url).json(&request).send().await;

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

                    if Self::is_transient_status(status) && attempts <= self.max_retries {
                        warn!(
                            "Facilitator returned transient error: {} - {} (attempt {}/{})",
                            status, body, attempts, self.max_retries
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

                    error!("Facilitator returned error: {} - {}", status, body);
                    return Err(AppError::Facilitator(format!(
                        "Facilitator returned {}: {}",
                        status, body
                    )));
                }
                Err(e) => {
                    if Self::is_transient_error(&e) && attempts <= self.max_retries {
                        warn!(
                            "Transient error connecting to facilitator: {} (attempt {}/{})",
                            e, attempts, self.max_retries
                        );
                        last_error =
                            Some(AppError::Facilitator(format!("Connection failed: {}", e)));

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

            let result = self.client.post(&url).json(&request).send().await;

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

                    if Self::is_transient_status(status) && attempts <= self.max_retries {
                        warn!(
                            "Facilitator settlement returned transient error: {} - {} (attempt {}/{})",
                            status, body, attempts, self.max_retries
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

                    error!(
                        "Facilitator settlement returned error: {} - {}",
                        status, body
                    );
                    return Err(AppError::Facilitator(format!(
                        "Settlement failed with {}: {}",
                        status, body
                    )));
                }
                Err(e) => {
                    if Self::is_transient_error(&e) && attempts <= self.max_retries {
                        warn!(
                            "Transient error connecting to facilitator for settlement: {} (attempt {}/{})",
                            e, attempts, self.max_retries
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
