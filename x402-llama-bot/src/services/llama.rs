use crate::error::AppError;
use crate::models::{ChatRequest, ChatResponse};
use reqwest::Client;
use tracing::{debug, error};

#[derive(Clone)]
pub struct LlamaClient {
    client: Client,
    endpoint: String,
    secret: String,
}

impl LlamaClient {
    pub fn new(endpoint: &str, secret: &str) -> Self {
        LlamaClient {
            client: Client::new(),
            endpoint: endpoint.trim_end_matches('/').to_string(),
            secret: secret.to_string(),
        }
    }

    /// Send a chat request to the DigitalOcean Llama agent
    pub async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AppError> {
        let url = format!("{}/api/v1/chat/completions", self.endpoint);

        debug!("Sending chat request to DO agent: {}", url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.secret))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to connect to Llama agent: {}", e);
                AppError::LlamaAgent(format!("Connection failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Llama agent returned error: {} - {}", status, body);
            return Err(AppError::LlamaAgent(format!(
                "Agent returned {}: {}",
                status, body
            )));
        }

        let chat_response: ChatResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Llama agent response: {}", e);
            AppError::LlamaAgent(format!("Invalid response: {}", e))
        })?;

        debug!("Received response from Llama agent");

        Ok(chat_response)
    }
}
