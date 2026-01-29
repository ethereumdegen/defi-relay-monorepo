use crate::error::AppError;
use crate::models::{ChatRequest, ChatResponse};
use reqwest::Client;
use tracing::{error, info};

#[derive(Clone)]
pub struct KimiClient {
    client: Client,
    endpoint: String,
    api_key: String,
    default_model: String,
}

impl KimiClient {
    pub fn new(endpoint: &str, api_key: &str, default_model: &str) -> Self {
        KimiClient {
            client: Client::new(),
            endpoint: endpoint.to_string(),
            api_key: api_key.to_string(),
            default_model: default_model.to_string(),
        }
    }

    /// Send a chat request to the Moonshot Kimi API
    pub async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AppError> {
        info!("Sending chat request to Kimi API at: {}", self.endpoint);

        // Use default model if not specified
        let model = request.model.as_deref().unwrap_or(&self.default_model);
        info!(
            "Request model: {}, messages: {}",
            model,
            request.messages.len()
        );

        // Build request with default model if not specified
        let mut request_body = serde_json::to_value(request).map_err(|e| {
            error!("Failed to serialize request: {}", e);
            AppError::KimiAgent(format!("Serialization failed: {}", e))
        })?;

        if request.model.is_none() {
            request_body["model"] = serde_json::Value::String(self.default_model.clone());
        }

        // Log the full request being sent to Kimi API
        info!(
            ">>> KIMI API REQUEST:\n{}",
            serde_json::to_string_pretty(&request_body).unwrap_or_else(|_| format!("{:?}", request_body))
        );

        let response = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to connect to Kimi API: {}", e);
                AppError::KimiAgent(format!("Connection failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Kimi API returned error: {} - {}", status, body);
            return Err(AppError::KimiAgent(format!(
                "API returned {}: {}",
                status, body
            )));
        }

        let chat_response: ChatResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Kimi API response: {}", e);
            AppError::KimiAgent(format!("Invalid response: {}", e))
        })?;

        // Log the full response from Kimi API
        info!(
            "<<< KIMI API RESPONSE:\n{}",
            serde_json::to_string_pretty(&chat_response).unwrap_or_else(|_| format!("{:?}", chat_response))
        );

        info!(
            "Kimi response received - model: {}, choices: {}",
            chat_response.model,
            chat_response.choices.len()
        );
        if let Some(usage) = &chat_response.usage {
            info!(
                "Token usage - prompt: {}, completion: {}, total: {}",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
        }

        Ok(chat_response)
    }
}
