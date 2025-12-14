//! OpenRouter provider implementation.
//!
//! This module provides the OpenRouter provider which offers access to multiple
//! LLM providers through a unified OpenAI-compatible API.
//!
//! OpenRouter uses the OpenAI chat completions format:
//! - Base URL: https://openrouter.ai/api/v1
//! - Auth: Bearer token (OPENROUTER_API_KEY)
//! - Model format: provider/model-name (e.g., "anthropic/claude-3.5-sonnet")

use crate::message::{
    CompletionRequest, ContentBlock, Message, Role, Tool, ToolChoice, ToolResultContent,
};
use crate::model::{ModelCapabilities, ModelInfo, ModelLimits, ModelPricing};
use crate::provider::{LlmProvider, ProviderCapabilities, ProviderError};
use crate::stream::{CompletionStream, FinishReason, SseStream, StreamEvent, Usage};
use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use serde::Deserialize;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Default API base URL.
const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// OpenRouter provider.
pub struct OpenRouterProvider {
    client: Client,
    api_key: String,
    base_url: String,
    /// Optional site URL for OpenRouter attribution.
    site_url: Option<String>,
    /// Optional site name for OpenRouter attribution.
    site_name: Option<String>,
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider.
    ///
    /// Reads the API key from `OPENROUTER_API_KEY` environment variable.
    pub fn new() -> Result<Self, ProviderError> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| ProviderError::MissingCredentials("OPENROUTER_API_KEY".into()))?;

        Ok(Self {
            client: Client::new(),
            api_key,
            base_url: std::env::var("OPENROUTER_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_BASE_URL.into()),
            site_url: std::env::var("OPENROUTER_SITE_URL").ok(),
            site_name: std::env::var("OPENROUTER_SITE_NAME").ok(),
        })
    }

    /// Create with a custom API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.into(),
            site_url: None,
            site_name: None,
        }
    }

    /// Set the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set site URL for attribution.
    pub fn site_url(mut self, url: impl Into<String>) -> Self {
        self.site_url = Some(url.into());
        self
    }

    /// Set site name for attribution.
    pub fn site_name(mut self, name: impl Into<String>) -> Self {
        self.site_name = Some(name.into());
        self
    }

    /// Build the request body for the OpenRouter API (OpenAI format).
    fn build_request(
        &self,
        request: &CompletionRequest,
    ) -> Result<serde_json::Value, ProviderError> {
        let messages = transform_messages(&request.messages, request.system.as_deref())?;

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "stream": true,
        });

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }

        if !request.stop.is_empty() {
            body["stop"] = serde_json::json!(request.stop);
        }

        if !request.tools.is_empty() {
            body["tools"] = transform_tools(&request.tools)?;
        }

        if let Some(tool_choice) = &request.tool_choice {
            body["tool_choice"] = transform_tool_choice(tool_choice)?;
        }

        Ok(body)
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    fn id(&self) -> &'static str {
        "openrouter"
    }

    fn display_name(&self) -> &'static str {
        "OpenRouter"
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(openrouter_models())
    }

    async fn stream(&self, request: CompletionRequest) -> Result<CompletionStream, ProviderError> {
        let body = self.build_request(&request)?;
        let model = request.model.clone();

        let mut req = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        // Add optional OpenRouter headers
        if let Some(site_url) = &self.site_url {
            req = req.header("HTTP-Referer", site_url);
        }
        if let Some(site_name) = &self.site_name {
            req = req.header("X-Title", site_name);
        }

        let response = req
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError {
                status,
                message: error_body,
            });
        }

        // Create SSE stream and wrap in adapter
        let sse_stream = SseStream::new(response.bytes_stream());
        let adapter = OpenRouterStreamAdapter::new(sse_stream, model);

        Ok(Box::pin(adapter))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            extended_thinking: false, // Depends on underlying model
            prompt_caching: false,    // Not directly supported
            interleaved_thinking: false,
            fine_grained_tool_streaming: true,
        }
    }
}

// ============================================================================
// Stream Adapter
// ============================================================================

/// Adapter that converts OpenAI-format SSE events to StreamEvents.
struct OpenRouterStreamAdapter<S> {
    inner: SseStream<S>,
    model: String,
    started: bool,
    current_content_index: usize,
    current_tool_calls: Vec<OpenAIToolCall>,
    usage: Usage,
}

impl<S> OpenRouterStreamAdapter<S> {
    fn new(inner: SseStream<S>, model: String) -> Self {
        Self {
            inner,
            model,
            started: false,
            current_content_index: 0,
            current_tool_calls: Vec::new(),
            usage: Usage::default(),
        }
    }
}

impl<S> Stream for OpenRouterStreamAdapter<S>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<StreamEvent, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(sse_event))) => {
                    // Handle done
                    if sse_event.is_done() {
                        // Emit finish event
                        return Poll::Ready(Some(Ok(StreamEvent::Finish {
                            finish_reason: FinishReason::Stop,
                            usage: self.usage.clone(),
                            provider_metadata: None,
                        })));
                    }

                    // Parse and convert event
                    match self.convert_event(&sse_event.data) {
                        Ok(events) => {
                            // Return first event, queue others if needed
                            if let Some(event) = events.into_iter().next() {
                                return Poll::Ready(Some(Ok(event)));
                            }
                            continue;
                        }
                        Err(e) => return Poll::Ready(Some(Err(e))),
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(ProviderError::Stream(e.to_string()))));
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<S> OpenRouterStreamAdapter<S> {
    fn convert_event(&mut self, data: &str) -> Result<Vec<StreamEvent>, ProviderError> {
        let chunk: OpenAIStreamChunk =
            serde_json::from_str(data).map_err(|e| ProviderError::Stream(e.to_string()))?;

        let mut events = Vec::new();

        // Emit start event on first chunk
        if !self.started {
            self.started = true;
            events.push(StreamEvent::Start {
                model: chunk.model.clone().unwrap_or_else(|| self.model.clone()),
                provider: "openrouter".to_string(),
            });
        }

        // Update usage if present
        if let Some(usage) = chunk.usage {
            self.usage.input_tokens = usage.prompt_tokens.unwrap_or(0);
            self.usage.output_tokens = usage.completion_tokens.unwrap_or(0);
        }

        // Process choices
        for choice in chunk.choices {
            // Check finish reason
            if let Some(reason) = choice.finish_reason {
                let finish_reason = match reason.as_str() {
                    "stop" => FinishReason::Stop,
                    "length" => FinishReason::Length,
                    "tool_calls" => FinishReason::ToolCalls,
                    "content_filter" => FinishReason::ContentFilter,
                    _ => FinishReason::Unknown,
                };

                events.push(StreamEvent::FinishStep {
                    finish_reason,
                    usage: self.usage.clone(),
                    provider_metadata: None,
                });
                continue;
            }

            // Process delta
            if let Some(delta) = choice.delta {
                // Handle content delta
                if let Some(content) = delta.content {
                    if !content.is_empty() {
                        let id = format!("content_{}", self.current_content_index);

                        // First content chunk starts a text block
                        if self.current_content_index == 0 {
                            events.push(StreamEvent::TextStart { id: id.clone() });
                            self.current_content_index = 1;
                        }

                        events.push(StreamEvent::TextDelta {
                            id: id.clone(),
                            delta: content,
                        });
                    }
                }

                // Handle tool calls delta
                if let Some(tool_calls) = delta.tool_calls {
                    for tc in tool_calls {
                        let index = tc.index.unwrap_or(0);

                        // Ensure we have enough entries
                        while self.current_tool_calls.len() <= index {
                            self.current_tool_calls.push(OpenAIToolCall {
                                id: String::new(),
                                name: String::new(),
                                arguments: String::new(),
                                started: false,
                            });
                        }

                        let current = &mut self.current_tool_calls[index];

                        // Update ID if present
                        if let Some(id) = tc.id {
                            current.id = id;
                        }

                        // Update function name and emit start
                        if let Some(function) = &tc.function {
                            if let Some(name) = &function.name {
                                if !current.started && !name.is_empty() {
                                    current.name = name.clone();
                                    current.started = true;
                                    events.push(StreamEvent::ToolInputStart {
                                        id: current.id.clone(),
                                        tool_name: name.clone(),
                                    });
                                }
                            }

                            // Stream argument deltas
                            if let Some(args) = &function.arguments {
                                if !args.is_empty() {
                                    current.arguments.push_str(args);
                                    events.push(StreamEvent::ToolInputDelta {
                                        id: current.id.clone(),
                                        delta: args.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(events)
    }
}

/// Temporary storage for accumulating tool call data.
struct OpenAIToolCall {
    id: String,
    name: String,
    arguments: String,
    started: bool,
}

// ============================================================================
// OpenAI API Types (used by OpenRouter)
// ============================================================================

#[derive(Debug, Deserialize)]
struct OpenAIStreamChunk {
    id: Option<String>,
    model: Option<String>,
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    index: Option<usize>,
    delta: Option<OpenAIDelta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIDelta {
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIToolCallDelta {
    index: Option<usize>,
    id: Option<String>,
    #[serde(rename = "type")]
    call_type: Option<String>,
    function: Option<OpenAIFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAIFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OpenAIError {
    error: OpenAIErrorInner,
}

#[derive(Debug, Deserialize)]
struct OpenAIErrorInner {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
    code: Option<String>,
}

// ============================================================================
// Transform functions
// ============================================================================

/// Transform messages to OpenAI format.
fn transform_messages(
    messages: &[Message],
    system: Option<&str>,
) -> Result<serde_json::Value, ProviderError> {
    let mut result = Vec::new();

    // Add system message first if provided
    if let Some(sys) = system {
        result.push(serde_json::json!({
            "role": "system",
            "content": sys,
        }));
    }

    for msg in messages {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Tool => "tool",
        };

        // Handle different message structures
        if msg.role == Role::Tool {
            // Tool results need special handling in OpenAI format
            for block in &msg.content {
                if let ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error: _,
                } = block
                {
                    let content_str = match content {
                        ToolResultContent::Text(t) => t.clone(),
                        ToolResultContent::Blocks(blocks) => {
                            // Concatenate text blocks
                            blocks
                                .iter()
                                .filter_map(|b| match b {
                                    ContentBlock::Text { text } => Some(text.as_str()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    };
                    result.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id,
                        "content": content_str,
                    }));
                }
            }
        } else if msg.role == Role::Assistant {
            // Assistant messages may have tool calls
            let mut tool_calls: Vec<serde_json::Value> = Vec::new();
            let mut text_content = String::new();

            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        text_content.push_str(text);
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        tool_calls.push(serde_json::json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": input.to_string(),
                            }
                        }));
                    }
                    _ => {}
                }
            }

            let mut msg_obj = serde_json::json!({
                "role": "assistant",
            });

            if !text_content.is_empty() {
                msg_obj["content"] = serde_json::json!(text_content);
            }

            if !tool_calls.is_empty() {
                msg_obj["tool_calls"] = serde_json::json!(tool_calls);
            }

            result.push(msg_obj);
        } else {
            // User/system messages - handle content blocks
            let content: Vec<serde_json::Value> = msg
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(serde_json::json!({
                        "type": "text",
                        "text": text,
                    })),
                    ContentBlock::Image { source, .. } => {
                        match source {
                            crate::message::ImageSource::Base64 { data, media_type } => {
                                Some(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": {
                                        "url": format!("data:{};base64,{}", media_type, data),
                                    }
                                }))
                            }
                            crate::message::ImageSource::Url { url } => {
                                Some(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": {
                                        "url": url,
                                    }
                                }))
                            }
                        }
                    }
                    _ => None,
                })
                .collect();

            // If only one text block, use simple string content
            if content.len() == 1 {
                if let Some(text) = content[0].get("text") {
                    result.push(serde_json::json!({
                        "role": role,
                        "content": text,
                    }));
                    continue;
                }
            }

            result.push(serde_json::json!({
                "role": role,
                "content": content,
            }));
        }
    }

    Ok(serde_json::json!(result))
}

/// Transform tools to OpenAI format.
fn transform_tools(tools: &[Tool]) -> Result<serde_json::Value, ProviderError> {
    let result: Vec<serde_json::Value> = tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                }
            })
        })
        .collect();

    Ok(serde_json::json!(result))
}

/// Transform tool choice to OpenAI format.
fn transform_tool_choice(choice: &ToolChoice) -> Result<serde_json::Value, ProviderError> {
    Ok(match choice {
        ToolChoice::Auto => serde_json::json!("auto"),
        ToolChoice::None => serde_json::json!("none"),
        ToolChoice::Required => serde_json::json!("required"),
        ToolChoice::Tool { name } => serde_json::json!({
            "type": "function",
            "function": {
                "name": name,
            }
        }),
    })
}

// ============================================================================
// OpenRouter Model Definitions
// ============================================================================

/// Get available OpenRouter models.
///
/// OpenRouter provides access to many models. This returns commonly used ones.
/// Users can still use any model available on OpenRouter by specifying the full ID.
fn openrouter_models() -> Vec<ModelInfo> {
    vec![
        // Anthropic via OpenRouter
        ModelInfo::builder("anthropic/claude-3.5-sonnet", "openrouter")
            .name("Claude 3.5 Sonnet (OpenRouter)")
            .family("claude-3.5")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: true,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: false,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(3.0, 15.0))
            .limits(ModelLimits::new(200_000, 8_192))
            .build(),
        // OpenAI via OpenRouter
        ModelInfo::builder("openai/gpt-4o", "openrouter")
            .name("GPT-4o (OpenRouter)")
            .family("gpt-4o")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: true,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: false,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(2.50, 10.0))
            .limits(ModelLimits::new(128_000, 16_384))
            .build(),
        // Google via OpenRouter
        ModelInfo::builder("google/gemini-2.0-flash-exp:free", "openrouter")
            .name("Gemini 2.0 Flash (Free)")
            .family("gemini-2.0")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: true,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: false,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(0.0, 0.0)) // Free tier
            .limits(ModelLimits::new(1_000_000, 8_192))
            .build(),
        // DeepSeek via OpenRouter
        ModelInfo::builder("deepseek/deepseek-chat", "openrouter")
            .name("DeepSeek Chat")
            .family("deepseek")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: false,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: false,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(0.14, 0.28))
            .limits(ModelLimits::new(64_000, 8_192))
            .build(),
        // Meta Llama via OpenRouter
        ModelInfo::builder("meta-llama/llama-3.3-70b-instruct", "openrouter")
            .name("Llama 3.3 70B")
            .family("llama-3.3")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: false,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: false,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(0.35, 0.40))
            .limits(ModelLimits::new(128_000, 8_192))
            .build(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_messages() {
        let messages = vec![Message::user("Hello"), Message::assistant("Hi there!")];

        let result = transform_messages(&messages, Some("You are helpful")).unwrap();
        let arr = result.as_array().unwrap();

        // Should have system + user + assistant
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["role"], "system");
        assert_eq!(arr[1]["role"], "user");
        assert_eq!(arr[2]["role"], "assistant");
    }

    #[test]
    fn test_transform_tools() {
        let tools = vec![Tool::new(
            "test_tool",
            "A test tool",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "arg": { "type": "string" }
                }
            }),
        )];

        let result = transform_tools(&tools).unwrap();
        let arr = result.as_array().unwrap();

        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["function"]["name"], "test_tool");
    }

    #[test]
    fn test_transform_tool_choice() {
        assert_eq!(transform_tool_choice(&ToolChoice::Auto).unwrap(), "auto");
        assert_eq!(transform_tool_choice(&ToolChoice::None).unwrap(), "none");
        assert_eq!(
            transform_tool_choice(&ToolChoice::Required).unwrap(),
            "required"
        );
    }
}
