use crate::error::AppError;
use crate::models::{PaymentPayload, VerifyPaymentRequirements, VerifyRequest, VerifyResponse, X402_VERSION};
use reqwest::Client;
use tracing::{debug, error, info};

#[derive(Clone)]
pub struct FacilitatorClient {
    client: Client,
    base_url: String,
}

impl FacilitatorClient {
    pub fn new(base_url: &str) -> Self {
        FacilitatorClient {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Verify a payment payload with the facilitator service
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

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to connect to facilitator: {}", e);
                AppError::Facilitator(format!("Connection failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Facilitator returned error: {} - {}", status, body);
            return Err(AppError::Facilitator(format!(
                "Facilitator returned {}: {}",
                status, body
            )));
        }

        let verify_response: VerifyResponse = response.json().await.map_err(|e| {
            error!("Failed to parse facilitator response: {}", e);
            AppError::Facilitator(format!("Invalid response: {}", e))
        })?;

        if verify_response.is_valid {
            info!(
                "Payment verified for payer: {:?}",
                verify_response.payer
            );
        } else {
            info!(
                "Payment rejected: {:?}",
                verify_response.invalid_reason
            );
        }

        Ok(verify_response)
    }
}
