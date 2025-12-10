//! OpenAI provider implementation

use crate::{
    ChatOptions, ChatResponse, ChatStream, ContentPart, LlmError, LlmProvider,
    LlmResult, Message, ModelCapabilities, ModelInfo, ProviderConfig, Role, StopReason,
    StreamChunk, ToolChoice, Usage,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OPENAI_API_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o";
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// OpenAI provider
pub struct OpenAIProvider {
    client: Client,
    config: ProviderConfig,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider
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
            .unwrap_or(OPENAI_API_URL)
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
        if let Some(ref org_id) = self.config.organization_id {
            if let Ok(org) = org_id.parse() {
                headers.insert("OpenAI-Organization", org);
            }
        }
        headers
    }

    /// Convert messages to OpenAI format
    fn convert_messages(&self, messages: &[Message], system: Option<&str>) -> Vec<OpenAIMessage> {
        let mut openai_messages = Vec::new();

        // Add system message if provided
        if let Some(sys) = system {
            openai_messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: Some(OpenAIContent::Text(sys.to_string())),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        for msg in messages {
            match msg.role {
                Role::System => {
                    openai_messages.push(OpenAIMessage {
                        role: "system".to_string(),
                        content: Some(OpenAIContent::Text(msg.text())),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    });
                }
                Role::User => {
                    let content = match &msg.content {
                        crate::Content::Text(text) => OpenAIContent::Text(text.clone()),
                        crate::Content::Parts(parts) => {
                            OpenAIContent::Parts(parts.iter().filter_map(Self::convert_part).collect())
                        }
                    };
                    openai_messages.push(OpenAIMessage {
                        role: "user".to_string(),
                        content: Some(content),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    });
                }
                Role::Assistant => {
                    let (content, tool_calls) = self.convert_assistant_message(msg);
                    openai_messages.push(OpenAIMessage {
                        role: "assistant".to_string(),
                        content,
                        tool_calls,
                        tool_call_id: None,
                        name: None,
                    });
                }
                Role::Tool => {
                    // Tool results in OpenAI format
                    if let crate::Content::Parts(parts) = &msg.content {
                        for part in parts {
                            if let ContentPart::ToolResult {
                                tool_use_id,
                                content,
                                ..
                            } = part
                            {
                                openai_messages.push(OpenAIMessage {
                                    role: "tool".to_string(),
                                    content: Some(OpenAIContent::Text(content.clone())),
                                    tool_calls: None,
                                    tool_call_id: Some(tool_use_id.clone()),
                                    name: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        openai_messages
    }

    /// Convert assistant message, extracting tool calls
    fn convert_assistant_message(
        &self,
        msg: &Message,
    ) -> (Option<OpenAIContent>, Option<Vec<OpenAIToolCall>>) {
        match &msg.content {
            crate::Content::Text(text) => (Some(OpenAIContent::Text(text.clone())), None),
            crate::Content::Parts(parts) => {
                let mut text_parts = Vec::new();
                let mut tool_calls = Vec::new();

                for part in parts {
                    match part {
                        ContentPart::Text { text } => {
                            text_parts.push(text.clone());
                        }
                        ContentPart::ToolUse { id, name, input } => {
                            tool_calls.push(OpenAIToolCall {
                                id: id.clone(),
                                call_type: "function".to_string(),
                                function: OpenAIFunctionCall {
                                    name: name.clone(),
                                    arguments: input.to_string(),
                                },
                            });
                        }
                        _ => {}
                    }
                }

                let content = if text_parts.is_empty() {
                    None
                } else {
                    Some(OpenAIContent::Text(text_parts.join("")))
                };

                let tools = if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                };

                (content, tools)
            }
        }
    }

    /// Convert a content part to OpenAI format
    fn convert_part(part: &ContentPart) -> Option<OpenAIContentPart> {
        match part {
            ContentPart::Text { text } => Some(OpenAIContentPart::Text { text: text.clone() }),
            ContentPart::Image { source } => {
                let url = match source {
                    crate::ImageSource::Base64 { media_type, data } => {
                        format!("data:{};base64,{}", media_type, data)
                    }
                    crate::ImageSource::Url { url } => url.clone(),
                };
                Some(OpenAIContentPart::ImageUrl {
                    image_url: OpenAIImageUrl { url },
                })
            }
            // Tool use/result handled separately
            _ => None,
        }
    }

    /// Convert tools to OpenAI format
    fn convert_tools(&self, tools: &[crate::ToolDefinition]) -> Vec<OpenAITool> {
        tools
            .iter()
            .map(|t| OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunction {
                    name: t.name.clone(),
                    description: Some(t.description.clone()),
                    parameters: Some(t.input_schema.clone()),
                    strict: Some(true),
                },
            })
            .collect()
    }

    /// Convert tool choice to OpenAI format
    fn convert_tool_choice(&self, choice: &ToolChoice) -> OpenAIToolChoice {
        match choice {
            ToolChoice::Auto => OpenAIToolChoice::String("auto".to_string()),
            ToolChoice::None => OpenAIToolChoice::String("none".to_string()),
            ToolChoice::Any => OpenAIToolChoice::String("required".to_string()),
            ToolChoice::Tool { name } => OpenAIToolChoice::Object {
                tool_type: "function".to_string(),
                function: OpenAIToolChoiceFunction { name: name.clone() },
            },
        }
    }

    /// Parse an error response
    fn parse_error(&self, status: reqwest::StatusCode, body: &str) -> LlmError {
        if let Ok(error_response) = serde_json::from_str::<OpenAIErrorResponse>(body) {
            let msg = error_response.error.message;
            let error_type = error_response.error.error_type.unwrap_or_default();

            match status.as_u16() {
                401 => LlmError::AuthenticationError(msg),
                429 => LlmError::RateLimitError(msg),
                400 if error_type.contains("invalid") => LlmError::InvalidRequest(msg),
                400 if msg.contains("context") || msg.contains("token") => {
                    LlmError::ContextLengthExceeded(msg)
                }
                404 => LlmError::ModelNotFound(msg),
                _ => LlmError::ProviderError {
                    provider: "openai".to_string(),
                    message: msg,
                },
            }
        } else {
            LlmError::ProviderError {
                provider: "openai".to_string(),
                message: body.to_string(),
            }
        }
    }

    /// Parse a response into ChatResponse
    fn parse_response(&self, response: OpenAIResponse) -> ChatResponse {
        // Extract stop_reason before consuming choices
        let stop_reason = response.choices.first().and_then(|c| {
            c.finish_reason.as_ref().map(|s| match s.as_str() {
                "stop" => StopReason::EndTurn,
                "length" => StopReason::MaxTokens,
                "tool_calls" => StopReason::ToolUse,
                "content_filter" => StopReason::ContentFilter,
                _ => StopReason::Unknown,
            })
        });

        let choice = response.choices.into_iter().next();

        let content = if let Some(choice) = choice {
            let mut parts = Vec::new();

            // Add text content
            if let Some(text) = choice.message.content {
                parts.push(ContentPart::Text { text });
            }

            // Add tool calls
            if let Some(tool_calls) = choice.message.tool_calls {
                for tc in tool_calls {
                    let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::Value::Null);
                    parts.push(ContentPart::ToolUse {
                        id: tc.id,
                        name: tc.function.name,
                        input,
                    });
                }
            }

            parts
        } else {
            Vec::new()
        };

        ChatResponse {
            id: response.id,
            model: response.model,
            content,
            stop_reason,
            usage: Usage {
                input_tokens: response.usage.prompt_tokens,
                output_tokens: response.usage.completion_tokens,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn default_model(&self) -> &str {
        self.config
            .default_model
            .as_deref()
            .unwrap_or(DEFAULT_MODEL)
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "gpt-4o".to_string(),
                name: "GPT-4o".to_string(),
                context_length: 128000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: true,
                    streaming: true,
                    thinking: false,
                    input_cost_per_mtok: Some(2.50),
                    output_cost_per_mtok: Some(10.0),
                },
            },
            ModelInfo {
                id: "gpt-4o-mini".to_string(),
                name: "GPT-4o Mini".to_string(),
                context_length: 128000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: true,
                    streaming: true,
                    thinking: false,
                    input_cost_per_mtok: Some(0.15),
                    output_cost_per_mtok: Some(0.60),
                },
            },
            ModelInfo {
                id: "o1".to_string(),
                name: "o1".to_string(),
                context_length: 200000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: true,
                    streaming: true,
                    thinking: true,
                    input_cost_per_mtok: Some(15.0),
                    output_cost_per_mtok: Some(60.0),
                },
            },
            ModelInfo {
                id: "o1-mini".to_string(),
                name: "o1 Mini".to_string(),
                context_length: 128000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: false,
                    streaming: true,
                    thinking: true,
                    input_cost_per_mtok: Some(3.0),
                    output_cost_per_mtok: Some(12.0),
                },
            },
            ModelInfo {
                id: "gpt-4-turbo".to_string(),
                name: "GPT-4 Turbo".to_string(),
                context_length: 128000,
                capabilities: ModelCapabilities {
                    tool_use: true,
                    vision: true,
                    streaming: true,
                    thinking: false,
                    input_cost_per_mtok: Some(10.0),
                    output_cost_per_mtok: Some(30.0),
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
        let openai_messages = self.convert_messages(messages, options.system.as_deref());

        let mut request = OpenAIRequest {
            model: options
                .model
                .clone()
                .unwrap_or_else(|| self.default_model().to_string()),
            messages: openai_messages,
            max_tokens: Some(options
                .max_tokens
                .or(self.config.default_max_tokens)
                .unwrap_or(DEFAULT_MAX_TOKENS)),
            temperature: options.temperature,
            top_p: options.top_p,
            stop: if options.stop_sequences.is_empty() {
                None
            } else {
                Some(options.stop_sequences.clone())
            },
            stream: false,
            tools: None,
            tool_choice: None,
        };

        if !options.tools.is_empty() {
            request.tools = Some(self.convert_tools(&options.tools));
            if let Some(ref choice) = options.tool_choice {
                request.tool_choice = Some(self.convert_tool_choice(choice));
            }
        }

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

        let openai_response: OpenAIResponse = serde_json::from_str(&body)?;
        Ok(self.parse_response(openai_response))
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> LlmResult<ChatStream> {
        let options = options.unwrap_or_default();
        let openai_messages = self.convert_messages(messages, options.system.as_deref());

        let mut request = OpenAIRequest {
            model: options
                .model
                .clone()
                .unwrap_or_else(|| self.default_model().to_string()),
            messages: openai_messages,
            max_tokens: Some(options
                .max_tokens
                .or(self.config.default_max_tokens)
                .unwrap_or(DEFAULT_MAX_TOKENS)),
            temperature: options.temperature,
            top_p: options.top_p,
            stop: if options.stop_sequences.is_empty() {
                None
            } else {
                Some(options.stop_sequences.clone())
            },
            stream: true,
            tools: None,
            tool_choice: None,
        };

        if !options.tools.is_empty() {
            request.tools = Some(self.convert_tools(&options.tools));
            if let Some(ref choice) = options.tool_choice {
                request.tool_choice = Some(self.convert_tool_choice(choice));
            }
        }

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
        Ok(Box::pin(parse_sse_stream(stream)))
    }

    async fn health_check(&self) -> LlmResult<bool> {
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
        (stream.boxed(), String::new(), StreamState::default()),
        |(mut stream, mut buffer, mut state)| async move {
            loop {
                // Try to parse complete events from buffer
                if let Some(event_end) = buffer.find("\n\n") {
                    let event_str = buffer[..event_end].to_string();
                    buffer = buffer[event_end + 2..].to_string();

                    if let Some(data) = event_str.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            // Return final Done chunk with accumulated usage
                            return Some((
                                Ok(StreamChunk::Done {
                                    stop_reason: state.stop_reason.take(),
                                    usage: state.usage.take().unwrap_or_default(),
                                }),
                                (stream, buffer, state),
                            ));
                        }

                        match serde_json::from_str::<OpenAIStreamEvent>(data) {
                            Ok(event) => {
                                if let Some(chunk) = parse_stream_event(event, &mut state) {
                                    return Some((Ok(chunk), (stream, buffer, state)));
                                }
                            }
                            Err(e) => {
                                return Some((
                                    Err(LlmError::StreamError(e.to_string())),
                                    (stream, buffer, state),
                                ));
                            }
                        }
                    }
                    continue;
                }

                // Need more data
                match stream.next().await {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    Some(Err(e)) => {
                        return Some((Err(LlmError::from(e)), (stream, buffer, state)));
                    }
                    None => {
                        return None;
                    }
                }
            }
        },
    )
}

#[derive(Default)]
struct StreamState {
    stop_reason: Option<StopReason>,
    usage: Option<Usage>,
    current_tool_id: Option<String>,
    current_tool_name: Option<String>,
}

/// Parse a stream event into StreamChunk
fn parse_stream_event(event: OpenAIStreamEvent, state: &mut StreamState) -> Option<StreamChunk> {
    let choice = event.choices.into_iter().next()?;

    // Update stop reason and usage
    if let Some(reason) = choice.finish_reason {
        state.stop_reason = Some(match reason.as_str() {
            "stop" => StopReason::EndTurn,
            "length" => StopReason::MaxTokens,
            "tool_calls" => StopReason::ToolUse,
            "content_filter" => StopReason::ContentFilter,
            _ => StopReason::Unknown,
        });
    }

    if let Some(usage) = event.usage {
        state.usage = Some(Usage {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });
    }

    let delta = choice.delta?;

    // Handle text content
    if let Some(content) = delta.content {
        return Some(StreamChunk::Text(content));
    }

    // Handle tool calls
    if let Some(tool_calls) = delta.tool_calls {
        for tc in tool_calls {
            if let Some(function) = tc.function {
                // New tool call starting
                if let Some(name) = function.name {
                    state.current_tool_id = tc.id.clone();
                    state.current_tool_name = Some(name.clone());
                    return Some(StreamChunk::ToolUseStart {
                        id: tc.id.unwrap_or_default(),
                        name,
                    });
                }
                // Tool arguments delta
                if let Some(args) = function.arguments {
                    return Some(StreamChunk::ToolInputDelta(args));
                }
            }
        }
    }

    None
}

// OpenAI API types

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<OpenAIToolChoice>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<OpenAIContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum OpenAIContent {
    Text(String),
    Parts(Vec<OpenAIContentPart>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenAIContentPart {
    Text { text: String },
    ImageUrl { image_url: OpenAIImageUrl },
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIImageUrl {
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAIFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunction,
}

#[derive(Debug, Serialize)]
struct OpenAIFunction {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenAIToolChoice {
    String(String),
    Object {
        #[serde(rename = "type")]
        tool_type: String,
        function: OpenAIToolChoiceFunction,
    },
}

#[derive(Debug, Serialize)]
struct OpenAIToolChoiceFunction {
    name: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: OpenAIUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAIErrorResponse {
    error: OpenAIError,
}

#[derive(Debug, Deserialize)]
struct OpenAIError {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

// Stream event types

#[derive(Debug, Deserialize)]
struct OpenAIStreamEvent {
    choices: Vec<OpenAIStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChoice {
    delta: Option<OpenAIStreamDelta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamDelta {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIStreamToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamToolCall {
    #[serde(default)]
    id: Option<String>,
    function: Option<OpenAIStreamFunction>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_messages() {
        let config = ProviderConfig::new("test-key");
        let provider = OpenAIProvider::new(config).unwrap();

        let messages = vec![
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        let converted = provider.convert_messages(&messages, Some("You are helpful"));
        assert_eq!(converted.len(), 3); // system + user + assistant
        assert_eq!(converted[0].role, "system");
        assert_eq!(converted[1].role, "user");
        assert_eq!(converted[2].role, "assistant");
    }

    #[test]
    fn test_tool_definition_conversion() {
        let config = ProviderConfig::new("test-key");
        let provider = OpenAIProvider::new(config).unwrap();

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
        assert_eq!(converted[0].function.name, "search");
        assert_eq!(converted[0].function.strict, Some(true));
    }

    #[test]
    fn test_tool_choice_conversion() {
        let config = ProviderConfig::new("test-key");
        let provider = OpenAIProvider::new(config).unwrap();

        // Auto
        let choice = provider.convert_tool_choice(&ToolChoice::Auto);
        match choice {
            OpenAIToolChoice::String(s) => assert_eq!(s, "auto"),
            _ => panic!("Expected string"),
        }

        // Specific tool
        let choice = provider.convert_tool_choice(&ToolChoice::Tool {
            name: "search".to_string(),
        });
        match choice {
            OpenAIToolChoice::Object { function, .. } => {
                assert_eq!(function.name, "search");
            }
            _ => panic!("Expected object"),
        }
    }
}
