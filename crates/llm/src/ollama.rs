//! Ollama provider for local LLM inference
//!
//! Uses Ollama's OpenAI-compatible API for tool-calling capable models.

use crate::{
    ChatOptions, ChatResponse, ChatStream, ContentPart, LlmError, LlmProvider, LlmResult, Message,
    ModelCapabilities, ModelInfo, ProviderConfig, Role, StopReason, StreamChunk, Usage,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "codellama:34b";
const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes for local models

/// Ollama provider for local inference
pub struct OllamaProvider {
    client: Client,
    config: ProviderConfig,
}

impl OllamaProvider {
    /// Create a new Ollama provider
    pub fn new(config: ProviderConfig) -> LlmResult<Self> {
        let timeout = if config.timeout_secs > 0 {
            config.timeout_secs
        } else {
            DEFAULT_TIMEOUT_SECS
        };

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout))
            .build()
            .map_err(|e| LlmError::ConfigurationError(e.to_string()))?;

        Ok(Self { client, config })
    }

    /// Create from endpoint URL (convenience constructor)
    pub fn from_url(url: impl Into<String>) -> LlmResult<Self> {
        let mut config = ProviderConfig::new("");
        config.base_url = Some(url.into());
        config.timeout_secs = DEFAULT_TIMEOUT_SECS;
        Self::new(config)
    }

    /// Get the base URL
    fn base_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(DEFAULT_OLLAMA_URL)
    }

    /// Convert messages to Ollama format (OpenAI-compatible)
    fn convert_messages(&self, messages: &[Message]) -> Vec<OllamaMessage> {
        messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "tool",
                };

                let content = msg.text();

                OllamaMessage {
                    role: role.to_string(),
                    content: Some(content),
                    tool_calls: None,
                    tool_call_id: None,
                    name: msg.name.clone(),
                }
            })
            .collect()
    }

    /// Convert tools to Ollama format
    fn convert_tools(&self, tools: &[crate::ToolDefinition]) -> Option<Vec<OllamaTool>> {
        if tools.is_empty() {
            return None;
        }

        Some(
            tools
                .iter()
                .map(|t| OllamaTool {
                    tool_type: "function".to_string(),
                    function: OllamaToolFunction {
                        name: t.name.clone(),
                        description: Some(t.description.clone()),
                        parameters: Some(t.input_schema.clone()),
                    },
                })
                .collect(),
        )
    }

    /// Parse response into ChatResponse
    fn parse_response(&self, response: OllamaResponse) -> LlmResult<ChatResponse> {
        let choice = response
            .choices
            .first()
            .ok_or_else(|| LlmError::InvalidRequest("No choices in response".to_string()))?;

        let mut content_parts = Vec::new();

        if let Some(text) = &choice.message.content {
            if !text.is_empty() {
                content_parts.push(ContentPart::Text { text: text.clone() });
            }
        }

        // Handle tool calls
        if let Some(tool_calls) = &choice.message.tool_calls {
            for tc in tool_calls {
                content_parts.push(ContentPart::ToolUse {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    input: serde_json::from_str(&tc.function.arguments).unwrap_or_default(),
                });
            }
        }

        let stop_reason = choice.finish_reason.as_deref().map(|r| match r {
            "stop" => StopReason::EndTurn,
            "length" => StopReason::MaxTokens,
            "tool_calls" => StopReason::ToolUse,
            _ => StopReason::Unknown,
        });

        let usage = response
            .usage
            .map(|u| Usage {
                input_tokens: u.prompt_tokens.unwrap_or(0),
                output_tokens: u.completion_tokens.unwrap_or(0),
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            })
            .unwrap_or_default();

        Ok(ChatResponse {
            id: response.id,
            model: response.model,
            content: content_parts,
            stop_reason,
            usage,
        })
    }

    /// Parse error response
    fn parse_error(&self, status: reqwest::StatusCode, body: &str) -> LlmError {
        match status.as_u16() {
            404 => LlmError::ModelNotFound(format!("Model not found: {}", body)),
            503 => LlmError::ProviderError {
                provider: "ollama".to_string(),
                message: "Ollama server unavailable".to_string(),
            },
            _ => LlmError::ProviderError {
                provider: "ollama".to_string(),
                message: body.to_string(),
            },
        }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn default_model(&self) -> &str {
        self.config
            .default_model
            .as_deref()
            .unwrap_or(DEFAULT_MODEL)
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", self.base_url());

        let response = self.client.get(&url).send().await.map_err(|e| {
            LlmError::ProviderError {
                provider: "ollama".to_string(),
                message: format!("Failed to connect to Ollama: {}", e),
            }
        })?;

        if !response.status().is_success() {
            return Err(LlmError::ProviderError {
                provider: "ollama".to_string(),
                message: "Failed to list models".to_string(),
            });
        }

        let tags: OllamaTagsResponse = response.json().await?;

        Ok(tags
            .models
            .unwrap_or_default()
            .into_iter()
            .map(|m| ModelInfo {
                id: m.name.clone(),
                name: m.name,
                context_length: 4096, // Default, actual varies by model
                capabilities: ModelCapabilities {
                    tool_use: true, // Most modern models support tools
                    vision: false,  // Would need to check per-model
                    streaming: true,
                    thinking: false,
                    input_cost_per_mtok: None, // Local, no cost
                    output_cost_per_mtok: None,
                },
            })
            .collect())
    }

    async fn model_info(&self, model: &str) -> LlmResult<ModelInfo> {
        let models = self.list_models().await?;
        models
            .into_iter()
            .find(|m| m.id == model || m.id.starts_with(&format!("{}:", model.split(':').next().unwrap_or(model))))
            .ok_or_else(|| LlmError::ModelNotFound(model.to_string()))
    }

    async fn chat(
        &self,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> LlmResult<ChatResponse> {
        let options = options.unwrap_or_default();
        let model = options
            .model
            .clone()
            .unwrap_or_else(|| self.default_model().to_string());

        let mut ollama_messages = self.convert_messages(messages);

        // Prepend system message if provided
        if let Some(system) = &options.system {
            ollama_messages.insert(
                0,
                OllamaMessage {
                    role: "system".to_string(),
                    content: Some(system.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
            );
        }

        let request = OllamaRequest {
            model: model.clone(),
            messages: ollama_messages,
            stream: false,
            tools: self.convert_tools(&options.tools),
            tool_choice: options.tool_choice.as_ref().map(|tc| match tc {
                crate::ToolChoice::Auto => "auto".to_string(),
                crate::ToolChoice::None => "none".to_string(),
                crate::ToolChoice::Any => "required".to_string(),
                crate::ToolChoice::Tool { name } => name.clone(),
            }),
            temperature: options.temperature,
            max_tokens: options.max_tokens,
        };

        let url = format!("{}/v1/chat/completions", self.base_url());

        let response = self.client.post(&url).json(&request).send().await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(self.parse_error(status, &body));
        }

        let ollama_response: OllamaResponse = serde_json::from_str(&body)?;
        self.parse_response(ollama_response)
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> LlmResult<ChatStream> {
        let options = options.unwrap_or_default();
        let model = options
            .model
            .clone()
            .unwrap_or_else(|| self.default_model().to_string());

        let mut ollama_messages = self.convert_messages(messages);

        if let Some(system) = &options.system {
            ollama_messages.insert(
                0,
                OllamaMessage {
                    role: "system".to_string(),
                    content: Some(system.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
            );
        }

        let request = OllamaRequest {
            model: model.clone(),
            messages: ollama_messages,
            stream: true,
            tools: self.convert_tools(&options.tools),
            tool_choice: None,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
        };

        let url = format!("{}/v1/chat/completions", self.base_url());

        let response = self.client.post(&url).json(&request).send().await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            return Err(self.parse_error(status, &body));
        }

        let stream = response.bytes_stream();
        Ok(Box::pin(parse_sse_stream(stream, model)))
    }

    async fn health_check(&self) -> LlmResult<bool> {
        let url = format!("{}/api/tags", self.base_url());
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

/// Check if Ollama is running and list available models
pub async fn check_ollama_health(
    endpoint: Option<&str>,
) -> LlmResult<(bool, Vec<String>)> {
    let url = format!("{}/api/tags", endpoint.unwrap_or(DEFAULT_OLLAMA_URL));

    let client = Client::new();
    let response = client.get(&url).send().await.map_err(|e| {
        LlmError::ProviderError {
            provider: "ollama".to_string(),
            message: format!("Failed to connect: {}", e),
        }
    })?;

    if !response.status().is_success() {
        return Ok((false, vec![]));
    }

    let tags: OllamaTagsResponse = response.json().await?;
    let models = tags
        .models
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.name)
        .collect();

    Ok((true, models))
}

/// Parse SSE stream from Ollama
fn parse_sse_stream(
    stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = LlmResult<StreamChunk>> + Send {
    let started = false;

    futures::stream::unfold(
        (stream.boxed(), String::new(), started, model),
        |(mut stream, mut buffer, mut started, model)| async move {
            loop {
                // Look for complete SSE data
                if let Some(data_start) = buffer.find("data: ") {
                    let offset = data_start + 6;
                    if let Some(newline) = buffer[offset..].find('\n') {
                        let data = buffer[offset..offset + newline].trim().to_string();
                        buffer = buffer[offset + newline + 1..].to_string();

                        if data == "[DONE]" {
                            return Some((
                                Ok(StreamChunk::Done {
                                    stop_reason: Some(StopReason::EndTurn),
                                    usage: Usage::default(),
                                }),
                                (stream, buffer, started, model),
                            ));
                        }

                        if let Ok(response) = serde_json::from_str::<OllamaStreamResponse>(&data) {
                            // Emit start chunk once
                            if !started {
                                started = true;
                                return Some((
                                    Ok(StreamChunk::Start {
                                        id: response.id.clone(),
                                        model: model.clone(),
                                    }),
                                    (stream, buffer, started, model),
                                ));
                            }

                            // Extract delta content
                            if let Some(choice) = response.choices.first() {
                                if let Some(delta) = &choice.delta {
                                    if let Some(content) = &delta.content {
                                        if !content.is_empty() {
                                            return Some((
                                                Ok(StreamChunk::Text(content.clone())),
                                                (stream, buffer, started, model),
                                            ));
                                        }
                                    }
                                }

                                // Check for done
                                if choice.finish_reason.is_some() {
                                    return Some((
                                        Ok(StreamChunk::Done {
                                            stop_reason: Some(StopReason::EndTurn),
                                            usage: Usage::default(),
                                        }),
                                        (stream, buffer, started, model),
                                    ));
                                }
                            }
                        }
                        continue;
                    }
                }

                // Need more data
                match stream.next().await {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    Some(Err(e)) => {
                        return Some((Err(LlmError::from(e)), (stream, buffer, started, model)));
                    }
                    None => {
                        return None;
                    }
                }
            }
        },
    )
}

// Ollama API types (OpenAI-compatible)

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OllamaToolFunction,
}

#[derive(Debug, Serialize)]
struct OllamaToolFunction {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OllamaToolCallFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    id: String,
    model: String,
    choices: Vec<OllamaChoice>,
    #[serde(default)]
    usage: Option<OllamaUsage>,
}

#[derive(Debug, Deserialize)]
struct OllamaChoice {
    message: OllamaMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamResponse {
    id: String,
    choices: Vec<OllamaStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamChoice {
    #[serde(default)]
    delta: Option<OllamaStreamDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Option<Vec<OllamaModel>>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_messages() {
        let config = ProviderConfig::new("");
        let provider = OllamaProvider::new(config).unwrap();

        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        let converted = provider.convert_messages(&messages);
        assert_eq!(converted.len(), 3);
        assert_eq!(converted[0].role, "system");
        assert_eq!(converted[1].role, "user");
        assert_eq!(converted[2].role, "assistant");
    }
}
