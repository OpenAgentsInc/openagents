//! OpenRouter provider implementation
//!
//! OpenRouter is an API proxy that provides access to multiple LLM providers
//! through a unified OpenAI-compatible API.

use crate::{
    ChatOptions, ChatResponse, ChatStream, ContentPart, LlmError, LlmProvider, LlmResult, Message,
    ModelCapabilities, ModelInfo, ProviderConfig, Role, StopReason, StreamChunk, Usage,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1";
const DEFAULT_MODEL: &str = "openai/gpt-4o";
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// OpenRouter provider
pub struct OpenRouterProvider {
    client: Client,
    config: ProviderConfig,
    /// Optional HTTP referer for attribution
    referer: Option<String>,
    /// Optional site name for attribution
    site_name: Option<String>,
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider
    pub fn new(config: ProviderConfig) -> LlmResult<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| LlmError::ConfigurationError(e.to_string()))?;

        Ok(Self {
            client,
            config,
            referer: None,
            site_name: None,
        })
    }

    /// Set the HTTP referer for attribution
    pub fn with_referer(mut self, referer: impl Into<String>) -> Self {
        self.referer = Some(referer.into());
        self
    }

    /// Set the site name for attribution
    pub fn with_site_name(mut self, name: impl Into<String>) -> Self {
        self.site_name = Some(name.into());
        self
    }

    /// Get the base URL
    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or(OPENROUTER_API_URL)
    }

    /// Build request headers
    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();

        if let Ok(auth) = format!("Bearer {}", self.config.api_key).parse() {
            headers.insert(reqwest::header::AUTHORIZATION, auth);
        }
        if let Ok(content_type) = "application/json".parse() {
            headers.insert(reqwest::header::CONTENT_TYPE, content_type);
        }
        if let Some(ref referer) = self.referer {
            if let Ok(value) = referer.parse() {
                headers.insert("HTTP-Referer", value);
            }
        }
        if let Some(ref site_name) = self.site_name {
            if let Ok(value) = site_name.parse() {
                headers.insert("X-Title", value);
            }
        }

        headers
    }

    /// Convert messages to OpenRouter format (OpenAI-compatible)
    fn convert_messages(&self, messages: &[Message]) -> Vec<OpenRouterMessage> {
        messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "tool",
                };

                let content = match &msg.content {
                    crate::Content::Text(text) => OpenRouterContent::Text(text.clone()),
                    crate::Content::Parts(parts) => {
                        let converted: Vec<OpenRouterContentPart> = parts
                            .iter()
                            .filter_map(|p| match p {
                                ContentPart::Text { text } => {
                                    Some(OpenRouterContentPart::Text { text: text.clone() })
                                }
                                ContentPart::Image { source } => {
                                    if let crate::ImageSource::Url { url } = source {
                                        Some(OpenRouterContentPart::ImageUrl {
                                            image_url: OpenRouterImageUrl { url: url.clone() },
                                        })
                                    } else if let crate::ImageSource::Base64 { media_type, data } = source {
                                        Some(OpenRouterContentPart::ImageUrl {
                                            image_url: OpenRouterImageUrl {
                                                url: format!("data:{};base64,{}", media_type, data),
                                            },
                                        })
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            })
                            .collect();
                        OpenRouterContent::Parts(converted)
                    }
                };

                OpenRouterMessage {
                    role: role.to_string(),
                    content,
                    tool_calls: None,
                    tool_call_id: None,
                    name: msg.name.clone(),
                }
            })
            .collect()
    }

    /// Convert tools to OpenRouter format
    fn convert_tools(&self, tools: &[crate::ToolDefinition]) -> Option<Vec<OpenRouterTool>> {
        if tools.is_empty() {
            return None;
        }

        Some(
            tools
                .iter()
                .map(|t| OpenRouterTool {
                    tool_type: "function".to_string(),
                    function: OpenRouterToolFunction {
                        name: t.name.clone(),
                        description: Some(t.description.clone()),
                        parameters: Some(t.input_schema.clone()),
                    },
                })
                .collect(),
        )
    }

    /// Convert tool choice
    fn convert_tool_choice(&self, choice: &crate::ToolChoice) -> OpenRouterToolChoice {
        match choice {
            crate::ToolChoice::Auto => OpenRouterToolChoice::String("auto".to_string()),
            crate::ToolChoice::None => OpenRouterToolChoice::String("none".to_string()),
            crate::ToolChoice::Any => OpenRouterToolChoice::String("required".to_string()),
            crate::ToolChoice::Tool { name } => OpenRouterToolChoice::Object {
                tool_type: "function".to_string(),
                function: OpenRouterToolChoiceFunction { name: name.clone() },
            },
        }
    }

    /// Parse response into ChatResponse
    fn parse_response(&self, response: OpenRouterResponse) -> LlmResult<ChatResponse> {
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
            "content_filter" => StopReason::ContentFilter,
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
            model: response.model.unwrap_or_default(),
            content: content_parts,
            stop_reason,
            usage,
        })
    }

    /// Parse error response
    fn parse_error(&self, status: reqwest::StatusCode, body: &str) -> LlmError {
        if let Ok(error_response) = serde_json::from_str::<OpenRouterErrorResponse>(body) {
            let msg = error_response.error.message;

            match status.as_u16() {
                401 => LlmError::AuthenticationError(msg),
                429 => LlmError::RateLimitError(msg),
                400 => LlmError::InvalidRequest(msg),
                404 => LlmError::ModelNotFound(msg),
                _ => LlmError::ProviderError {
                    provider: "openrouter".to_string(),
                    message: msg,
                },
            }
        } else {
            LlmError::ProviderError {
                provider: "openrouter".to_string(),
                message: body.to_string(),
            }
        }
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    fn name(&self) -> &'static str {
        "openrouter"
    }

    fn default_model(&self) -> &str {
        self.config
            .default_model
            .as_deref()
            .unwrap_or(DEFAULT_MODEL)
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        let url = format!("{}/models", self.base_url());

        let response = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(LlmError::ProviderError {
                provider: "openrouter".to_string(),
                message: "Failed to list models".to_string(),
            });
        }

        let models_response: OpenRouterModelsResponse = response.json().await?;

        Ok(models_response
            .data
            .into_iter()
            .map(|m| ModelInfo {
                id: m.id.clone(),
                name: m.name.unwrap_or(m.id),
                context_length: m.context_length.unwrap_or(4096),
                capabilities: ModelCapabilities {
                    tool_use: true,  // Most OpenRouter models support tools
                    vision: false,   // Would need per-model checking
                    streaming: true,
                    thinking: false,
                    input_cost_per_mtok: m.pricing.as_ref().and_then(|p| {
                        p.prompt.as_ref().and_then(|s| s.parse::<f64>().ok().map(|v| v * 1_000_000.0))
                    }),
                    output_cost_per_mtok: m.pricing.as_ref().and_then(|p| {
                        p.completion.as_ref().and_then(|s| s.parse::<f64>().ok().map(|v| v * 1_000_000.0))
                    }),
                },
            })
            .collect())
    }

    async fn model_info(&self, model: &str) -> LlmResult<ModelInfo> {
        let models = self.list_models().await?;
        models
            .into_iter()
            .find(|m| m.id == model)
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

        let mut openrouter_messages = self.convert_messages(messages);

        // Prepend system message if provided
        if let Some(system) = &options.system {
            openrouter_messages.insert(
                0,
                OpenRouterMessage {
                    role: "system".to_string(),
                    content: OpenRouterContent::Text(system.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
            );
        }

        let request = OpenRouterRequest {
            model,
            messages: openrouter_messages,
            stream: false,
            tools: self.convert_tools(&options.tools),
            tool_choice: options.tool_choice.as_ref().map(|tc| self.convert_tool_choice(tc)),
            temperature: options.temperature,
            max_tokens: options.max_tokens.or(Some(DEFAULT_MAX_TOKENS)),
            top_p: options.top_p,
            stop: if options.stop_sequences.is_empty() {
                None
            } else {
                Some(options.stop_sequences.clone())
            },
        };

        let url = format!("{}/chat/completions", self.base_url());

        let response = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(self.parse_error(status, &body));
        }

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&body)?;
        self.parse_response(openrouter_response)
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

        let mut openrouter_messages = self.convert_messages(messages);

        if let Some(system) = &options.system {
            openrouter_messages.insert(
                0,
                OpenRouterMessage {
                    role: "system".to_string(),
                    content: OpenRouterContent::Text(system.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
            );
        }

        let request = OpenRouterRequest {
            model: model.clone(),
            messages: openrouter_messages,
            stream: true,
            tools: self.convert_tools(&options.tools),
            tool_choice: options.tool_choice.as_ref().map(|tc| self.convert_tool_choice(tc)),
            temperature: options.temperature,
            max_tokens: options.max_tokens.or(Some(DEFAULT_MAX_TOKENS)),
            top_p: options.top_p,
            stop: if options.stop_sequences.is_empty() {
                None
            } else {
                Some(options.stop_sequences.clone())
            },
        };

        let url = format!("{}/chat/completions", self.base_url());

        let response = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            return Err(self.parse_error(status, &body));
        }

        let stream = response.bytes_stream();
        Ok(Box::pin(parse_sse_stream(stream, model)))
    }

    async fn health_check(&self) -> LlmResult<bool> {
        // Check if we can list models
        match self.list_models().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// Parse SSE stream from OpenRouter
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

                        if let Ok(response) = serde_json::from_str::<OpenRouterStreamResponse>(&data) {
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

// OpenRouter API types

#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenRouterTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<OpenRouterToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct OpenRouterMessage {
    role: String,
    content: OpenRouterContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenRouterToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenRouterContent {
    Text(String),
    Parts(Vec<OpenRouterContentPart>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenRouterContentPart {
    Text { text: String },
    ImageUrl { image_url: OpenRouterImageUrl },
}

#[derive(Debug, Serialize)]
struct OpenRouterImageUrl {
    url: String,
}

#[derive(Debug, Serialize)]
struct OpenRouterTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenRouterToolFunction,
}

#[derive(Debug, Serialize)]
struct OpenRouterToolFunction {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenRouterToolChoice {
    String(String),
    Object {
        #[serde(rename = "type")]
        tool_type: String,
        function: OpenRouterToolChoiceFunction,
    },
}

#[derive(Debug, Serialize)]
struct OpenRouterToolChoiceFunction {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenRouterToolCallFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    id: String,
    #[serde(default)]
    model: Option<String>,
    choices: Vec<OpenRouterChoice>,
    #[serde(default)]
    usage: Option<OpenRouterUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterResponseMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenRouterToolCall>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenRouterUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterStreamResponse {
    id: String,
    choices: Vec<OpenRouterStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterStreamChoice {
    #[serde(default)]
    delta: Option<OpenRouterStreamDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterStreamDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    context_length: Option<u32>,
    #[serde(default)]
    pricing: Option<OpenRouterPricing>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterPricing {
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    completion: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterErrorResponse {
    error: OpenRouterError,
}

#[derive(Debug, Deserialize)]
struct OpenRouterError {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_messages() {
        let config = ProviderConfig::new("test-key");
        let provider = OpenRouterProvider::new(config).unwrap();

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
