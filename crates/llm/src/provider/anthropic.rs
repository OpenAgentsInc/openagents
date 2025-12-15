//! Anthropic Claude provider implementation.
//!
//! This module provides the Anthropic provider for Claude models with support for:
//! - Streaming responses via SSE
//! - Extended thinking
//! - Tool calling
//! - Prompt caching

use crate::message::{
    CompletionRequest, ContentBlock, Message, Role, Tool, ToolChoice, ToolResultContent,
};
use crate::model::{self, ModelInfo};
use crate::provider::{LlmProvider, ProviderCapabilities, ProviderError};
use crate::stream::{CompletionStream, FinishReason, SseStream, StreamError, StreamEvent, Usage};
use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use serde::Deserialize;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Default API base URL.
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";

/// Default API version.
const API_VERSION: &str = "2023-06-01";

/// Default beta features to enable.
const DEFAULT_BETAS: &[&str] = &[
    "claude-code-20250219",
    "interleaved-thinking-2025-05-14",
    "fine-grained-tool-streaming-2025-05-14",
];

/// Anthropic Claude provider.
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
    betas: Vec<String>,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    ///
    /// Reads the API key from `ANTHROPIC_API_KEY` environment variable.
    pub fn new() -> Result<Self, ProviderError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| ProviderError::MissingCredentials("ANTHROPIC_API_KEY".into()))?;

        Ok(Self {
            client: Client::new(),
            api_key,
            base_url: std::env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_BASE_URL.into()),
            betas: DEFAULT_BETAS.iter().map(|s| s.to_string()).collect(),
        })
    }

    /// Create with a custom API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.into(),
            betas: DEFAULT_BETAS.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Set the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set beta features.
    pub fn betas(mut self, betas: Vec<String>) -> Self {
        self.betas = betas;
        self
    }

    /// Build the request body for the Anthropic API.
    fn build_request(
        &self,
        request: &CompletionRequest,
    ) -> Result<serde_json::Value, ProviderError> {
        let messages = transform_messages(&request.messages)?;

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(8192),
            "stream": true,
        });

        if let Some(system) = &request.system {
            body["system"] = serde_json::json!(system);
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }

        if !request.stop.is_empty() {
            body["stop_sequences"] = serde_json::json!(request.stop);
        }

        if !request.tools.is_empty() {
            body["tools"] = transform_tools(&request.tools)?;
        }

        if let Some(tool_choice) = &request.tool_choice {
            body["tool_choice"] = transform_tool_choice(tool_choice)?;
        }

        // Apply Anthropic-specific options
        if let Some(opts) = &request.provider_options.anthropic {
            if let Some(thinking) = &opts.thinking {
                body["thinking"] = serde_json::json!(thinking);
            }
        }

        Ok(body)
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    fn display_name(&self) -> &'static str {
        "Anthropic"
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(model::anthropic::all())
    }

    async fn stream(&self, request: CompletionRequest) -> Result<CompletionStream, ProviderError> {
        let body = self.build_request(&request)?;
        let model = request.model.clone();

        let response = self
            .client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("anthropic-beta", self.betas.join(","))
            .header("content-type", "application/json")
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
        let adapter = AnthropicStreamAdapter::new(sse_stream, model);

        Ok(Box::pin(adapter))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            extended_thinking: true,
            prompt_caching: true,
            interleaved_thinking: true,
            fine_grained_tool_streaming: true,
        }
    }
}

// ============================================================================
// Stream Adapter
// ============================================================================

/// Adapter that converts Anthropic SSE events to StreamEvents.
struct AnthropicStreamAdapter<S> {
    inner: SseStream<S>,
    model: String,
    started: bool,
    current_content_id: Option<String>,
    current_tool_id: Option<String>,
    current_tool_name: Option<String>,
    current_tool_input: Option<String>,
    usage: Usage,
}

impl<S> AnthropicStreamAdapter<S> {
    fn new(inner: SseStream<S>, model: String) -> Self {
        Self {
            inner,
            model,
            started: false,
            current_content_id: None,
            current_tool_id: None,
            current_tool_name: None,
            current_tool_input: None,
            usage: Usage::default(),
        }
    }
}

impl<S> Stream for AnthropicStreamAdapter<S>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<StreamEvent, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(sse_event))) => {
                    // Skip ping events
                    if sse_event.event == "ping" {
                        continue;
                    }

                    // Handle done
                    if sse_event.is_done() {
                        return Poll::Ready(None);
                    }

                    // Parse and convert event
                    match self.convert_event(&sse_event.event, &sse_event.data) {
                        Ok(Some(event)) => return Poll::Ready(Some(Ok(event))),
                        Ok(None) => continue, // Skip this event
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

impl<S> AnthropicStreamAdapter<S> {
    fn convert_event(
        &mut self,
        event_type: &str,
        data: &str,
    ) -> Result<Option<StreamEvent>, ProviderError> {
        match event_type {
            "message_start" => {
                let msg: AnthropicMessageStart =
                    serde_json::from_str(data).map_err(|e| ProviderError::Stream(e.to_string()))?;

                // Update usage from message start
                if let Some(usage) = msg.message.usage {
                    self.usage.input_tokens = usage.input_tokens;
                }

                if !self.started {
                    self.started = true;
                    return Ok(Some(StreamEvent::Start {
                        model: self.model.clone(),
                        provider: "anthropic".to_string(),
                    }));
                }
                Ok(None)
            }

            "content_block_start" => {
                let block: AnthropicContentBlockStart =
                    serde_json::from_str(data).map_err(|e| ProviderError::Stream(e.to_string()))?;

                let id = format!("block_{}", block.index);

                match &block.content_block {
                    AnthropicContentBlock::Text { .. } => {
                        self.current_content_id = Some(id.clone());
                        Ok(Some(StreamEvent::TextStart { id }))
                    }
                    AnthropicContentBlock::ToolUse {
                        id: tool_id,
                        name,
                        input,
                    } => {
                        self.current_tool_id = Some(tool_id.clone());
                        self.current_tool_name = Some(name.clone());
                        self.current_tool_input =
                            Some(serde_json::to_string(&input).unwrap_or_default());
                        Ok(Some(StreamEvent::ToolInputStart {
                            id: tool_id.clone(),
                            tool_name: name.clone(),
                        }))
                    }
                    AnthropicContentBlock::Thinking { .. } => {
                        self.current_content_id = Some(id.clone());
                        Ok(Some(StreamEvent::ReasoningStart {
                            id,
                            provider_metadata: None,
                        }))
                    }
                }
            }

            "content_block_delta" => {
                let delta: AnthropicContentBlockDelta =
                    serde_json::from_str(data).map_err(|e| ProviderError::Stream(e.to_string()))?;

                match delta.delta {
                    AnthropicDelta::TextDelta { text } => {
                        let id = self.current_content_id.clone().unwrap_or_default();
                        Ok(Some(StreamEvent::TextDelta { id, delta: text }))
                    }
                    AnthropicDelta::InputJsonDelta { partial_json } => {
                        let id = self.current_tool_id.clone().unwrap_or_default();
                        if let Some(buf) = &mut self.current_tool_input {
                            buf.push_str(&partial_json);
                        } else {
                            self.current_tool_input = Some(partial_json.clone());
                        }
                        Ok(Some(StreamEvent::ToolInputDelta {
                            id,
                            delta: partial_json,
                        }))
                    }
                    AnthropicDelta::ThinkingDelta { thinking } => {
                        let id = self.current_content_id.clone().unwrap_or_default();
                        Ok(Some(StreamEvent::ReasoningDelta {
                            id,
                            delta: thinking,
                            provider_metadata: None,
                        }))
                    }
                }
            }

            "content_block_stop" => {
                let stop: AnthropicContentBlockStop =
                    serde_json::from_str(data).map_err(|e| ProviderError::Stream(e.to_string()))?;

                let id = format!("block_{}", stop.index);

                // Determine type from current state
                if self.current_tool_id.is_some() {
                    let tool_id = self.current_tool_id.take().unwrap_or_default();
                    let tool_name = self.current_tool_name.take().unwrap_or_default();
                    let input_raw = self.current_tool_input.take().unwrap_or_default();
                    let input = serde_json::from_str(&input_raw)
                        .unwrap_or_else(|_| serde_json::Value::String(input_raw));

                    Ok(Some(StreamEvent::ToolCall {
                        tool_call_id: tool_id,
                        tool_name,
                        input,
                        provider_metadata: None,
                    }))
                } else {
                    self.current_content_id = None;
                    Ok(Some(StreamEvent::TextEnd { id }))
                }
            }

            "message_delta" => {
                let delta: AnthropicMessageDelta =
                    serde_json::from_str(data).map_err(|e| ProviderError::Stream(e.to_string()))?;

                // Update usage
                if let Some(usage) = delta.usage {
                    self.usage.output_tokens = usage.output_tokens;
                }

                let finish_reason = match delta.delta.stop_reason.as_deref() {
                    Some("end_turn") | Some("stop") => FinishReason::Stop,
                    Some("max_tokens") => FinishReason::Length,
                    Some("tool_use") => FinishReason::ToolCalls,
                    _ => FinishReason::Unknown,
                };

                Ok(Some(StreamEvent::FinishStep {
                    finish_reason,
                    usage: self.usage.clone(),
                    provider_metadata: None,
                }))
            }

            "message_stop" => Ok(Some(StreamEvent::Finish {
                finish_reason: FinishReason::Stop,
                usage: self.usage.clone(),
                provider_metadata: None,
            })),

            "error" => {
                let error: AnthropicError =
                    serde_json::from_str(data).map_err(|e| ProviderError::Stream(e.to_string()))?;

                Ok(Some(StreamEvent::Error {
                    error: StreamError {
                        code: error.error.error_type,
                        message: error.error.message,
                        details: None,
                    },
                }))
            }

            _ => {
                // Unknown event type, skip
                tracing::debug!("Unknown Anthropic event type: {}", event_type);
                Ok(None)
            }
        }
    }
}

// ============================================================================
// Anthropic API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct AnthropicMessageStart {
    message: AnthropicMessage,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicMessage {
    id: String,
    model: String,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicUsage {
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlockStart {
    index: usize,
    content_block: AnthropicContentBlock,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Thinking {
        thinking: String,
    },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicContentBlockDelta {
    index: usize,
    delta: AnthropicDelta,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    ThinkingDelta { thinking: String },
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlockStop {
    index: usize,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageDelta {
    delta: AnthropicMessageDeltaInner,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicMessageDeltaInner {
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicError {
    error: AnthropicErrorInner,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorInner {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

// ============================================================================
// Transform functions
// ============================================================================

/// Transform messages to Anthropic format.
fn transform_messages(messages: &[Message]) -> Result<serde_json::Value, ProviderError> {
    let mut result = Vec::new();

    for msg in messages {
        let role = match msg.role {
            Role::User | Role::Tool => "user",
            Role::Assistant => "assistant",
            Role::System => continue, // System handled separately
        };

        let content: Vec<serde_json::Value> = msg
            .content
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text } => serde_json::json!({
                    "type": "text",
                    "text": text,
                }),
                ContentBlock::Image {
                    source,
                    media_type: _,
                } => match source {
                    crate::message::ImageSource::Base64 {
                        data,
                        media_type: mt,
                    } => {
                        serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": mt,
                                "data": data,
                            }
                        })
                    }
                    crate::message::ImageSource::Url { url } => {
                        serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "url",
                                "url": url,
                            }
                        })
                    }
                },
                ContentBlock::ToolUse { id, name, input } => serde_json::json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": input,
                }),
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let content_value = match content {
                        ToolResultContent::Text(text) => serde_json::json!(text),
                        ToolResultContent::Blocks(blocks) => {
                            // Recursively transform blocks
                            serde_json::json!(
                                blocks
                                    .iter()
                                    .map(|b| match b {
                                        ContentBlock::Text { text } => serde_json::json!({
                                            "type": "text",
                                            "text": text,
                                        }),
                                        _ => serde_json::json!({}),
                                    })
                                    .collect::<Vec<_>>()
                            )
                        }
                    };
                    serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": content_value,
                        "is_error": is_error,
                    })
                }
                ContentBlock::Reasoning { text, .. } => serde_json::json!({
                    "type": "thinking",
                    "thinking": text,
                }),
            })
            .collect();

        result.push(serde_json::json!({
            "role": role,
            "content": content,
        }));
    }

    Ok(serde_json::json!(result))
}

/// Transform tools to Anthropic format.
fn transform_tools(tools: &[Tool]) -> Result<serde_json::Value, ProviderError> {
    let result: Vec<serde_json::Value> = tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
            })
        })
        .collect();

    Ok(serde_json::json!(result))
}

/// Transform tool choice to Anthropic format.
fn transform_tool_choice(choice: &ToolChoice) -> Result<serde_json::Value, ProviderError> {
    Ok(match choice {
        ToolChoice::Auto => serde_json::json!({ "type": "auto" }),
        ToolChoice::None => serde_json::json!({ "type": "none" }),
        ToolChoice::Required => serde_json::json!({ "type": "any" }),
        ToolChoice::Tool { name } => serde_json::json!({
            "type": "tool",
            "name": name,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_messages() {
        let messages = vec![Message::user("Hello"), Message::assistant("Hi there!")];

        let result = transform_messages(&messages).unwrap();
        let arr = result.as_array().unwrap();

        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["role"], "user");
        assert_eq!(arr[1]["role"], "assistant");
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
        assert_eq!(arr[0]["name"], "test_tool");
    }
}
