use crate::error::AppError;
use crate::models::{ChatRequest, ChatResponse};
use reqwest::Client;
use tracing::{debug, error, info};

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
            endpoint: endpoint.to_string(),
            secret: secret.to_string(),
        }
    }

    /// Send a chat request to the DigitalOcean Llama agent
    pub async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AppError> {
        info!("Sending chat request to Llama agent at: {}", self.endpoint);
        info!(
            "Request model: {}, messages: {}",
            request.model.as_deref().unwrap_or("default"),
            request.messages.len()
        );
        debug!("Full request: {:?}", request);

        let response = self
            .client
            .post(&self.endpoint)
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

        info!(
            "Llama response received - model: {}, choices: {}",
            chat_response.model,
            chat_response.choices.len()
        );
        if let Some(usage) = &chat_response.usage {
            info!(
                "Token usage - prompt: {}, completion: {}, total: {}",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
        }
        debug!("Full response: {:?}", chat_response);

        Ok(chat_response)
    }
}
