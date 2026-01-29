use serde::{Deserialize, Serialize};
use serde_json::Value;

/// OpenAI-compatible chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: &str, content: &str) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
        }
    }
}

/// OpenAI-compatible tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// OpenAI-compatible tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// OpenAI-compatible chat request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
}

/// OpenAI-compatible chat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ResponseMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Response message can have tool_calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ===========================================
    // Tests for tool definitions in requests (TO Kimi)
    // ===========================================

    #[test]
    fn test_tool_definitions_preserved_in_request() {
        let request = ChatRequest {
            messages: vec![ChatMessage::user("What's the weather?")],
            model: Some("moonshot-v1-8k".to_string()),
            temperature: None,
            max_tokens: None,
            stream: None,
            tools: Some(vec![Tool {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_weather".to_string(),
                    description: "Get the current weather for a location".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "The city and state, e.g. San Francisco, CA"
                            }
                        },
                        "required": ["location"]
                    }),
                },
            }]),
            tool_choice: Some(json!("auto")),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        // Verify tools array exists and has correct structure
        assert!(parsed["tools"].is_array(), "tools should be an array");
        assert_eq!(parsed["tools"].as_array().unwrap().len(), 1);

        let tool = &parsed["tools"][0];
        assert_eq!(tool["type"], "function");
        assert_eq!(tool["function"]["name"], "get_weather");
        assert_eq!(
            tool["function"]["description"],
            "Get the current weather for a location"
        );
        assert!(tool["function"]["parameters"]["properties"]["location"].is_object());

        // Verify tool_choice is preserved
        assert_eq!(parsed["tool_choice"], "auto");
    }

    #[test]
    fn test_multiple_tools_preserved_in_request() {
        let request = ChatRequest {
            messages: vec![ChatMessage::user("Help me with tasks")],
            model: None,
            temperature: None,
            max_tokens: None,
            stream: None,
            tools: Some(vec![
                Tool {
                    tool_type: "function".to_string(),
                    function: FunctionDefinition {
                        name: "search_web".to_string(),
                        description: "Search the web".to_string(),
                        parameters: json!({"type": "object", "properties": {}}),
                    },
                },
                Tool {
                    tool_type: "function".to_string(),
                    function: FunctionDefinition {
                        name: "run_code".to_string(),
                        description: "Execute code".to_string(),
                        parameters: json!({"type": "object", "properties": {}}),
                    },
                },
            ]),
            tool_choice: None,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(parsed["tools"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["tools"][0]["function"]["name"], "search_web");
        assert_eq!(parsed["tools"][1]["function"]["name"], "run_code");
    }

    // ===========================================
    // Tests for tool calls in messages (TO Kimi - conversation history)
    // ===========================================

    #[test]
    fn test_assistant_message_with_tool_calls_preserved() {
        // When sending conversation history, assistant messages may contain tool_calls
        let message = ChatMessage {
            role: "assistant".to_string(),
            content: None, // Often null when assistant makes a tool call
            tool_calls: Some(vec![ToolCall {
                id: "call_abc123".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "get_weather".to_string(),
                    arguments: r#"{"location": "Boston, MA"}"#.to_string(),
                },
            }]),
            tool_call_id: None,
        };

        let serialized = serde_json::to_string(&message).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        // Verify tool_calls array is preserved
        assert!(parsed["tool_calls"].is_array());
        let tool_calls = parsed["tool_calls"].as_array().unwrap();
        assert_eq!(tool_calls.len(), 1);

        let tool_call = &tool_calls[0];
        assert_eq!(tool_call["id"], "call_abc123");
        assert_eq!(tool_call["type"], "function");
        assert_eq!(tool_call["function"]["name"], "get_weather");
        assert_eq!(tool_call["function"]["arguments"], r#"{"location": "Boston, MA"}"#);

        // Content should not be present (skip_serializing_if = None)
        assert!(parsed.get("content").is_none() || parsed["content"].is_null());
    }

    #[test]
    fn test_tool_result_message_preserved() {
        // Tool result messages have role "tool" and include tool_call_id
        let message = ChatMessage::tool_result(
            "call_abc123",
            r#"{"temperature": 72, "unit": "fahrenheit", "condition": "sunny"}"#,
        );

        let serialized = serde_json::to_string(&message).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(parsed["role"], "tool");
        assert_eq!(parsed["tool_call_id"], "call_abc123");
        assert!(parsed["content"].is_string());
    }

    #[test]
    fn test_full_tool_conversation_history_preserved() {
        // Simulate a full conversation with tool usage
        let messages = vec![
            ChatMessage::user("What's the weather in Boston?"),
            ChatMessage {
                role: "assistant".to_string(),
                content: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_weather_1".to_string(),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: "get_weather".to_string(),
                        arguments: r#"{"location": "Boston, MA"}"#.to_string(),
                    },
                }]),
                tool_call_id: None,
            },
            ChatMessage::tool_result("call_weather_1", r#"{"temp": 65, "condition": "cloudy"}"#),
            ChatMessage::assistant("The weather in Boston is 65°F and cloudy."),
            ChatMessage::user("And in New York?"),
        ];

        let request = ChatRequest {
            messages,
            model: None,
            temperature: None,
            max_tokens: None,
            stream: None,
            tools: Some(vec![Tool {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_weather".to_string(),
                    description: "Get weather".to_string(),
                    parameters: json!({}),
                },
            }]),
            tool_choice: None,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        let msgs = parsed["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 5);

        // Message 0: user
        assert_eq!(msgs[0]["role"], "user");

        // Message 1: assistant with tool_calls
        assert_eq!(msgs[1]["role"], "assistant");
        assert!(msgs[1]["tool_calls"].is_array());
        assert_eq!(msgs[1]["tool_calls"][0]["id"], "call_weather_1");

        // Message 2: tool result
        assert_eq!(msgs[2]["role"], "tool");
        assert_eq!(msgs[2]["tool_call_id"], "call_weather_1");

        // Message 3: assistant with content
        assert_eq!(msgs[3]["role"], "assistant");
        assert!(msgs[3]["content"].is_string());

        // Message 4: user follow-up
        assert_eq!(msgs[4]["role"], "user");
    }

    // ===========================================
    // Tests for tool calls in responses (FROM Kimi)
    // ===========================================

    #[test]
    fn test_response_with_tool_calls_deserialized() {
        // Simulate a Kimi API response with tool calls
        let response_json = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1699000000,
            "model": "moonshot-v1-8k",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_xyz789",
                        "type": "function",
                        "function": {
                            "name": "get_stock_price",
                            "arguments": "{\"symbol\": \"AAPL\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 20,
                "total_tokens": 70
            }
        });

        let response: ChatResponse = serde_json::from_value(response_json).unwrap();

        assert_eq!(response.choices.len(), 1);
        let message = &response.choices[0].message;

        assert_eq!(message.role, "assistant");
        assert!(message.content.is_none());

        let tool_calls = message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_xyz789");
        assert_eq!(tool_calls[0].call_type, "function");
        assert_eq!(tool_calls[0].function.name, "get_stock_price");
        assert_eq!(tool_calls[0].function.arguments, r#"{"symbol": "AAPL"}"#);

        assert_eq!(response.choices[0].finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn test_response_with_multiple_tool_calls_deserialized() {
        // Kimi may return multiple tool calls in a single response
        let response_json = json!({
            "id": "chatcmpl-456",
            "object": "chat.completion",
            "created": 1699000001,
            "model": "moonshot-v1-32k",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "get_weather",
                                "arguments": "{\"location\": \"NYC\"}"
                            }
                        },
                        {
                            "id": "call_2",
                            "type": "function",
                            "function": {
                                "name": "get_weather",
                                "arguments": "{\"location\": \"LA\"}"
                            }
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 40,
                "total_tokens": 140
            }
        });

        let response: ChatResponse = serde_json::from_value(response_json).unwrap();

        let tool_calls = response.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 2);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[1].id, "call_2");
        assert_eq!(tool_calls[0].function.arguments, r#"{"location": "NYC"}"#);
        assert_eq!(tool_calls[1].function.arguments, r#"{"location": "LA"}"#);
    }

    #[test]
    fn test_response_with_content_only_no_tool_calls() {
        // Normal response without tool calls should also work
        let response_json = json!({
            "id": "chatcmpl-789",
            "object": "chat.completion",
            "created": 1699000002,
            "model": "moonshot-v1-8k",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you today?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        });

        let response: ChatResponse = serde_json::from_value(response_json).unwrap();

        let message = &response.choices[0].message;
        assert_eq!(message.content.as_deref(), Some("Hello! How can I help you today?"));
        assert!(message.tool_calls.is_none());
    }

    // ===========================================
    // Round-trip tests (serialize then deserialize)
    // ===========================================

    #[test]
    fn test_request_roundtrip_with_tools() {
        let original_request = ChatRequest {
            messages: vec![
                ChatMessage::system("You are a helpful assistant."),
                ChatMessage::user("What time is it in Tokyo?"),
            ],
            model: Some("moonshot-v1-128k".to_string()),
            temperature: Some(0.7),
            max_tokens: Some(1000),
            stream: Some(false),
            tools: Some(vec![Tool {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_time".to_string(),
                    description: "Get current time in a timezone".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "timezone": {"type": "string"}
                        },
                        "required": ["timezone"]
                    }),
                },
            }]),
            tool_choice: Some(json!({"type": "function", "function": {"name": "get_time"}})),
        };

        let serialized = serde_json::to_string(&original_request).unwrap();
        let deserialized: ChatRequest = serde_json::from_str(&serialized).unwrap();

        // Verify all fields survived the round-trip
        assert_eq!(deserialized.messages.len(), 2);
        assert_eq!(deserialized.model, Some("moonshot-v1-128k".to_string()));
        assert_eq!(deserialized.temperature, Some(0.7));
        assert_eq!(deserialized.max_tokens, Some(1000));
        assert_eq!(deserialized.stream, Some(false));

        let tools = deserialized.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].tool_type, "function");
        assert_eq!(tools[0].function.name, "get_time");

        assert!(deserialized.tool_choice.is_some());
    }

    #[test]
    fn test_response_roundtrip_with_tool_calls() {
        let original_response = ChatResponse {
            id: "test-id".to_string(),
            object: "chat.completion".to_string(),
            created: 1699999999,
            model: "moonshot-v1-8k".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ResponseMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![ToolCall {
                        id: "call_roundtrip".to_string(),
                        call_type: "function".to_string(),
                        function: FunctionCall {
                            name: "calculate".to_string(),
                            arguments: r#"{"expression": "2+2"}"#.to_string(),
                        },
                    }]),
                },
                finish_reason: Some("tool_calls".to_string()),
            }],
            usage: Some(ChatUsage {
                prompt_tokens: 25,
                completion_tokens: 15,
                total_tokens: 40,
            }),
        };

        let serialized = serde_json::to_string(&original_response).unwrap();
        let deserialized: ChatResponse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.id, "test-id");
        assert_eq!(deserialized.choices.len(), 1);

        let tool_calls = deserialized.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_roundtrip");
        assert_eq!(tool_calls[0].function.name, "calculate");
        assert_eq!(tool_calls[0].function.arguments, r#"{"expression": "2+2"}"#);
    }

    // ===========================================
    // Tests for skip_serializing_if behavior
    // ===========================================

    #[test]
    fn test_none_fields_not_serialized() {
        // When tools are None, they should not appear in the JSON at all
        let request = ChatRequest {
            messages: vec![ChatMessage::user("Hello")],
            model: None,
            temperature: None,
            max_tokens: None,
            stream: None,
            tools: None,
            tool_choice: None,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        // These fields should not exist in the JSON (not even as null)
        assert!(!parsed.as_object().unwrap().contains_key("tools"));
        assert!(!parsed.as_object().unwrap().contains_key("tool_choice"));
        assert!(!parsed.as_object().unwrap().contains_key("model"));
        assert!(!parsed.as_object().unwrap().contains_key("temperature"));
    }

    #[test]
    fn test_message_without_tool_calls_clean_json() {
        let message = ChatMessage::user("Just a regular message");

        let serialized = serde_json::to_string(&message).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        // tool_calls and tool_call_id should not appear
        assert!(!parsed.as_object().unwrap().contains_key("tool_calls"));
        assert!(!parsed.as_object().unwrap().contains_key("tool_call_id"));

        // Only role and content should be present
        assert!(parsed.as_object().unwrap().contains_key("role"));
        assert!(parsed.as_object().unwrap().contains_key("content"));
    }
}
