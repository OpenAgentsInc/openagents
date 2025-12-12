//! Anthropic Claude provider implementation

use crate::{
    ApiErrorResponse, ChatOptions, ChatResponse, ChatStream, ContentPart, LlmError, LlmProvider,
    LlmResult, Message, ModelCapabilities, ModelInfo, ProviderConfig, Role, StopReason,
    StreamChunk, ToolChoice, Usage,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001"; // Use ai::Model::default()
const DEFAULT_MAX_TOKENS: u32 = 8192;

/// Anthropic Claude provider
pub struct AnthropicProvider {
    client: Client,
    config: ProviderConfig,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(config: ProviderConfig) -> LlmResult<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| LlmError::ConfigurationError(e.to_string()))?;

        Ok(Self { client, config })
    }

    /// Get the base URL
    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or(ANTHROPIC_API_URL)
    }

    /// Build request headers
    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(key) = self.config.api_key.parse() {
            headers.insert("x-api-key", key);
        }
        if let Ok(version) = ANTHROPIC_VERSION.parse() {
            headers.insert("anthropic-version", version);
        }
        if let Ok(content_type) = "application/json".parse() {
            headers.insert(reqwest::header::CONTENT_TYPE, content_type);
        }
        headers
    }

    /// Convert messages to Anthropic format
    fn convert_messages(&self, messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    // Combine system messages
                    let text = msg.text();
                    system = Some(match system {
                        Some(existing) => format!("{}\n\n{}", existing, text),
                        None => text,
                    });
                }
                Role::User | Role::Assistant | Role::Tool => {
                    let role = match msg.role {
                        Role::User | Role::Tool => "user",
                        Role::Assistant => "assistant",
                        _ => continue,
                    };

                    let content = match &msg.content {
                        crate::Content::Text(text) => AnthropicContent::Text(text.clone()),
                        crate::Content::Parts(parts) => {
                            AnthropicContent::Parts(parts.iter().map(Self::convert_part).collect())
                        }
                    };

                    anthropic_messages.push(AnthropicMessage {
                        role: role.to_string(),
                        content,
                    });
                }
            }
        }

        (system, anthropic_messages)
    }

    /// Convert a content part to Anthropic format
    fn convert_part(part: &ContentPart) -> AnthropicContentPart {
        match part {
            ContentPart::Text { text } => AnthropicContentPart::Text { text: text.clone() },
            ContentPart::Image { source } => {
                let (media_type, data, source_type) = match source {
                    crate::ImageSource::Base64 { media_type, data } => {
                        (media_type.clone(), data.clone(), "base64")
                    }
                    crate::ImageSource::Url { url } => {
                        // Anthropic requires base64, but we'll pass URL for now
                        ("image/jpeg".to_string(), url.clone(), "url")
                    }
                };
                AnthropicContentPart::Image {
                    source: AnthropicImageSource {
                        source_type: source_type.to_string(),
                        media_type,
                        data,
                    },
                }
            }
            ContentPart::ToolUse { id, name, input } => AnthropicContentPart::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            },
            ContentPart::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => AnthropicContentPart::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: content.clone(),
                is_error: *is_error,
            },
        }
    }

    /// Convert tools to Anthropic format
    fn convert_tools(
        &self,
        tools: &[crate::ToolDefinition],
    ) -> Vec<AnthropicTool> {
        tools
            .iter()
            .map(|t| AnthropicTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect()
    }

    /// Convert tool choice to Anthropic format
    fn convert_tool_choice(&self, choice: &ToolChoice) -> AnthropicToolChoice {
        match choice {
            ToolChoice::Auto => AnthropicToolChoice {
                tool_type: "auto".to_string(),
                name: None,
            },
            ToolChoice::None => AnthropicToolChoice {
                tool_type: "none".to_string(),
                name: None,
            },
            ToolChoice::Any => AnthropicToolChoice {
                tool_type: "any".to_string(),
                name: None,
            },
            ToolChoice::Tool { name } => AnthropicToolChoice {
                tool_type: "tool".to_string(),
                name: Some(name.clone()),
            },
        }
    }

    /// Parse an error response
    fn parse_error(&self, status: reqwest::StatusCode, body: &str) -> LlmError {
        if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(body) {
            let msg = error_response.error.message;
            let error_type = error_response.error.error_type;

            match status.as_u16() {
                401 => LlmError::AuthenticationError(msg),
                429 => LlmError::RateLimitError(msg),
                400 if error_type.contains("invalid") => LlmError::InvalidRequest(msg),
                400 if msg.contains("context") || msg.contains("token") => {
                    LlmError::ContextLengthExceeded(msg)
                }
                404 => LlmError::ModelNotFound(msg),
                _ => LlmError::ProviderError {
                    provider: "anthropic".to_string(),
                    message: msg,
                },
            }
        } else {
            LlmError::ProviderError {
                provider: "anthropic".to_string(),
                message: body.to_string(),
            }
        }
    }

    /// Parse a response into ChatResponse
    fn parse_response(&self, response: AnthropicResponse) -> ChatResponse {
        let content = match response.content {
            AnthropicContent::Text(text) => vec![ContentPart::Text { text }],
            AnthropicContent::Parts(parts) => {
                parts.into_iter().map(Self::parse_content_part).collect()
            }
        };

        ChatResponse {
            id: response.id,
            model: response.model,
            content,
            stop_reason: response.stop_reason.map(|s| match s.as_str() {
                "end_turn" => StopReason::EndTurn,
                "max_tokens" => StopReason::MaxTokens,
                "stop_sequence" => StopReason::StopSequence,
                "tool_use" => StopReason::ToolUse,
                _ => StopReason::Unknown,
            }),
            usage: Usage {
                input_tokens: response.usage.input_tokens,
                output_tokens: response.usage.output_tokens,
                cache_creation_input_tokens: response
                    .usage
                    .cache_creation_input_tokens
                    .unwrap_or(0),
                cache_read_input_tokens: response.usage.cache_read_input_tokens.unwrap_or(0),
            },
        }
    }

    /// Parse a content part from Anthropic format
    fn parse_content_part(part: AnthropicContentPart) -> ContentPart {
        match part {
            AnthropicContentPart::Text { text } => ContentPart::Text { text },
            AnthropicContentPart::Image { source } => ContentPart::Image {
                source: if source.source_type == "base64" {
                    crate::ImageSource::Base64 {
                        media_type: source.media_type,
                        data: source.data,
                    }
                } else {
                    crate::ImageSource::Url { url: source.data }
                },
            },
            AnthropicContentPart::ToolUse { id, name, input } => {
                ContentPart::ToolUse { id, name, input }
            }
            AnthropicContentPart::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => ContentPart::ToolResult {
                tool_use_id,
                content,
                is_error,
            },
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn default_model(&self) -> &str {
        self.config
            .default_model
            .as_deref()
            .unwrap_or(DEFAULT_MODEL)
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        // Anthropic doesn't have a list models endpoint, return known models
        Ok(vec![
            ModelInfo {
                id: "claude-opus-4-20250514".to_string(),
                name: "Claude Opus 4".to_string(),
                context_length: 200000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: true,
                    streaming: true,
                    thinking: true,
                    input_cost_per_mtok: Some(15.0),
                    output_cost_per_mtok: Some(75.0),
                },
            },
            ModelInfo {
                id: "claude-sonnet-4-20250514".to_string(),
                name: "Claude Sonnet 4".to_string(),
                context_length: 200000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: true,
                    streaming: true,
                    thinking: true,
                    input_cost_per_mtok: Some(3.0),
                    output_cost_per_mtok: Some(15.0),
                },
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".to_string(),
                name: "Claude 3.5 Haiku".to_string(),
                context_length: 200000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: true,
                    streaming: true,
                    thinking: false,
                    input_cost_per_mtok: Some(0.80),
                    output_cost_per_mtok: Some(4.0),
                },
            },
        ])
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
        let (system, anthropic_messages) = self.convert_messages(messages);

        let mut request = AnthropicRequest {
            model: options
                .model
                .clone()
                .unwrap_or_else(|| self.default_model().to_string()),
            messages: anthropic_messages,
            system: options.system.or(system),
            max_tokens: options
                .max_tokens
                .or(self.config.default_max_tokens)
                .unwrap_or(DEFAULT_MAX_TOKENS),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: if options.stop_sequences.is_empty() {
                None
            } else {
                Some(options.stop_sequences.clone())
            },
            stream: false,
            tools: None,
            tool_choice: None,
            metadata: options.metadata.clone(),
        };

        if !options.tools.is_empty() {
            request.tools = Some(self.convert_tools(&options.tools));
            if let Some(ref choice) = options.tool_choice {
                request.tool_choice = Some(self.convert_tool_choice(choice));
            }
        }

        let url = format!("{}/messages", self.base_url());
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

        let anthropic_response: AnthropicResponse = serde_json::from_str(&body)?;
        Ok(self.parse_response(anthropic_response))
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> LlmResult<ChatStream> {
        let options = options.unwrap_or_default();
        let (system, anthropic_messages) = self.convert_messages(messages);

        let mut request = AnthropicRequest {
            model: options
                .model
                .clone()
                .unwrap_or_else(|| self.default_model().to_string()),
            messages: anthropic_messages,
            system: options.system.or(system),
            max_tokens: options
                .max_tokens
                .or(self.config.default_max_tokens)
                .unwrap_or(DEFAULT_MAX_TOKENS),
            temperature: options.temperature,
            top_p: options.top_p,
            stop_sequences: if options.stop_sequences.is_empty() {
                None
            } else {
                Some(options.stop_sequences.clone())
            },
            stream: true,
            tools: None,
            tool_choice: None,
            metadata: options.metadata.clone(),
        };

        if !options.tools.is_empty() {
            request.tools = Some(self.convert_tools(&options.tools));
            if let Some(ref choice) = options.tool_choice {
                request.tool_choice = Some(self.convert_tool_choice(choice));
            }
        }

        let url = format!("{}/messages", self.base_url());
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
        Ok(Box::pin(parse_sse_stream(stream)))
    }

    async fn health_check(&self) -> LlmResult<bool> {
        // Simple health check by making a minimal request
        let messages = vec![Message::user("Hi")];
        let options = ChatOptions::default().max_tokens(1);
        match self.chat(&messages, Some(options)).await {
            Ok(_) => Ok(true),
            Err(LlmError::AuthenticationError(_)) => Ok(false),
            Err(_) => Ok(false),
        }
    }
}

/// Parse SSE stream into StreamChunk
fn parse_sse_stream(
    stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = LlmResult<StreamChunk>> + Send {
    futures::stream::unfold(
        (stream.boxed(), String::new()),
        |(mut stream, mut buffer)| async move {
            loop {
                // First, try to parse any complete events from the buffer
                if let Some(event_end) = buffer.find("\n\n") {
                    let event_str = buffer[..event_end].to_string();
                    buffer = buffer[event_end + 2..].to_string();

                    if let Some(data) = event_str.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            continue;
                        }

                        match serde_json::from_str::<StreamEvent>(data) {
                            Ok(event) => {
                                if let Some(chunk) = parse_stream_event(event) {
                                    return Some((Ok(chunk), (stream, buffer)));
                                }
                            }
                            Err(e) => {
                                return Some((
                                    Err(LlmError::StreamError(e.to_string())),
                                    (stream, buffer),
                                ));
                            }
                        }
                    }
                    continue;
                }

                // Need more data from the stream
                match stream.next().await {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    Some(Err(e)) => {
                        return Some((Err(LlmError::from(e)), (stream, buffer)));
                    }
                    None => {
                        // Stream ended
                        return None;
                    }
                }
            }
        },
    )
}

/// Parse a stream event into StreamChunk
fn parse_stream_event(event: StreamEvent) -> Option<StreamChunk> {
    match event.event_type.as_str() {
        "message_start" => event.message.map(|m| StreamChunk::Start {
            id: m.id,
            model: m.model,
        }),
        "content_block_start" => event.content_block.and_then(|cb| {
            if cb.block_type == "tool_use" {
                Some(StreamChunk::ToolUseStart {
                    id: cb.id.unwrap_or_default(),
                    name: cb.name.unwrap_or_default(),
                })
            } else {
                None
            }
        }),
        "content_block_delta" => event.delta.and_then(|d| {
            if d.delta_type == "text_delta" {
                d.text.map(StreamChunk::Text)
            } else if d.delta_type == "input_json_delta" {
                d.partial_json.map(StreamChunk::ToolInputDelta)
            } else {
                None
            }
        }),
        "content_block_stop" => Some(StreamChunk::ToolUseEnd),
        "message_delta" => event.delta.and_then(|d| {
            d.stop_reason.map(|sr| StreamChunk::Done {
                stop_reason: Some(match sr.as_str() {
                    "end_turn" => StopReason::EndTurn,
                    "max_tokens" => StopReason::MaxTokens,
                    "stop_sequence" => StopReason::StopSequence,
                    "tool_use" => StopReason::ToolUse,
                    _ => StopReason::Unknown,
                }),
                usage: event.usage.map(|u| Usage {
                    input_tokens: u.input_tokens,
                    output_tokens: u.output_tokens,
                    cache_creation_input_tokens: u.cache_creation_input_tokens.unwrap_or(0),
                    cache_read_input_tokens: u.cache_read_input_tokens.unwrap_or(0),
                }).unwrap_or_default(),
            })
        }),
        "message_stop" => None, // Handled by message_delta
        "error" => event.error.map(|e| StreamChunk::Error(e.message)),
        _ => None,
    }
}

// Anthropic API types

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<AnthropicToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Parts(Vec<AnthropicContentPart>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentPart {
    Text {
        text: String,
    },
    Image {
        source: AnthropicImageSource,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct AnthropicToolChoice {
    #[serde(rename = "type")]
    tool_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    model: String,
    content: AnthropicContent,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
    cache_creation_input_tokens: Option<u32>,
    cache_read_input_tokens: Option<u32>,
}

// Stream event types

#[derive(Debug, Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    message: Option<StreamMessage>,
    #[serde(default)]
    content_block: Option<StreamContentBlock>,
    #[serde(default)]
    delta: Option<StreamDelta>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
    #[serde(default)]
    error: Option<StreamError>,
}

#[derive(Debug, Deserialize)]
struct StreamMessage {
    id: String,
    model: String,
}

#[derive(Debug, Deserialize)]
struct StreamContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    #[serde(rename = "type")]
    delta_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    partial_json: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamError {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_messages() {
        let config = ProviderConfig::new("test-key");
        let provider = AnthropicProvider::new(config).unwrap();

        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        let (system, converted) = provider.convert_messages(&messages);
        assert_eq!(system, Some("You are helpful".to_string()));
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "user");
        assert_eq!(converted[1].role, "assistant");
    }

    #[test]
    fn test_tool_definition_conversion() {
        let config = ProviderConfig::new("test-key");
        let provider = AnthropicProvider::new(config).unwrap();

        let tools = vec![crate::ToolDefinition::new(
            "search",
            "Search for something",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }),
        )];

        let converted = provider.convert_tools(&tools);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].name, "search");
    }
}
