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
    /// "kimi" or "openai" — controls protocol differences
    archetype: String,
}

impl KimiClient {
    pub fn new(endpoint: &str, api_key: &str, default_model: &str, archetype: &str) -> Self {
        KimiClient {
            client: Client::new(),
            endpoint: endpoint.to_string(),
            api_key: api_key.to_string(),
            default_model: default_model.to_string(),
            archetype: archetype.to_string(),
        }
    }

    /// Send a chat request to the Moonshot Kimi API
    pub async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AppError> {
        info!("Sending chat request to Kimi API at: {}", self.endpoint);

        // Enforce model: reject requests that specify a different model than configured
        if let Some(ref requested_model) = request.model {
            if !requested_model.is_empty() && requested_model != &self.default_model {
                error!(
                    "Rejected request with model '{}' — relay is configured for '{}'",
                    requested_model, self.default_model
                );
                return Err(AppError::KimiAgent(format!(
                    "Model '{}' not available on this relay. Omit the model field to use the default.",
                    requested_model
                )));
            }
        }

        info!(
            "Using model: {}, messages: {}",
            self.default_model,
            request.messages.len()
        );

        // Always use the relay's configured model
        let mut request_body = serde_json::to_value(request).map_err(|e| {
            error!("Failed to serialize request: {}", e);
            AppError::KimiAgent(format!("Serialization failed: {}", e))
        })?;

        request_body["model"] = serde_json::Value::String(self.default_model.clone());

        // Protocol adjustments based on archetype
        if self.archetype == "openai" {
            // OpenAI uses max_completion_tokens instead of max_tokens
            if let Some(max_tokens) = request_body.get("max_tokens").and_then(|v| v.as_u64()) {
                request_body["max_completion_tokens"] = serde_json::Value::Number(max_tokens.into());
                request_body.as_object_mut().unwrap().remove("max_tokens");
            }
        } else if self.archetype == "kimi" {
            // Kimi K2.5 has thinking enabled by default, which is incompatible with
            // tool_choice: "required". Disable thinking so tool calling works reliably.
            request_body["thinking"] = serde_json::json!({"type": "disabled"});
        }

        // Log the full request being sent to API
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
