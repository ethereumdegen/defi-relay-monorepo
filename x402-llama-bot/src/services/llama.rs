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

        // Log the full request
        info!(
            ">>> LLAMA API REQUEST:\n{}",
            serde_json::to_string_pretty(&request).unwrap_or_else(|_| format!("{:?}", request))
        );

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

        // Log the full response
        info!(
            "<<< LLAMA API RESPONSE:\n{}",
            serde_json::to_string_pretty(&chat_response).unwrap_or_else(|_| format!("{:?}", chat_response))
        );

        info!(
            "Llama response received - model: {}, choices: {}",
            chat_response.model,
            chat_response.choices.len()
        );

        // Log tool calls prominently
        for choice in &chat_response.choices {
            if let Some(ref tool_calls) = choice.message.tool_calls {
                for tc in tool_calls {
                    info!(
                        ">>> TOOL CALL: {} | id: {} | args: {}",
                        tc.function.name,
                        tc.id,
                        tc.function.arguments
                    );
                }
            }
        }

        if let Some(usage) = &chat_response.usage {
            info!(
                "Token usage - prompt: {}, completion: {}, total: {}",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
        }

        Ok(chat_response)
    }
}
