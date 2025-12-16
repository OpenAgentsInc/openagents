//! Shared OpenAI-compatible helpers for providers that use the
//! chat completions streaming format (OpenAI, OpenRouter, Ollama, etc).

use crate::message::{
    CompletionRequest, ContentBlock, Message, Role, Tool, ToolChoice, ToolResultContent,
};
use crate::provider::ProviderError;
use crate::stream::{FinishReason, SseStream, StreamEvent, Usage};
use futures::Stream;
use serde::Deserialize;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Build an OpenAI-format chat request body.
pub(crate) fn build_openai_body(
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

    // Apply OpenAI-specific options if present
    if let Some(opts) = &request.provider_options.openai {
        if let Some(reasoning_effort) = &opts.reasoning_effort {
            body["reasoning_effort"] = serde_json::json!(reasoning_effort);
        }
        if let Some(reasoning_summary) = &opts.reasoning_summary {
            body["reasoning_summary"] = serde_json::json!(reasoning_summary);
        }
        if let Some(prompt_cache_key) = &opts.prompt_cache_key {
            body["prompt_cache_key"] = serde_json::json!(prompt_cache_key);
        }
        if let Some(service_tier) = &opts.service_tier {
            body["service_tier"] = serde_json::json!(service_tier);
        }
    }

    // Pass through any extra provider options
    for (key, value) in &request.provider_options.extra {
        body[key] = value.clone();
    }

    Ok(body)
}

/// Map OpenAI finish reasons to the internal enum.
pub(crate) fn map_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "stop" => FinishReason::Stop,
        "length" => FinishReason::Length,
        "tool_calls" => FinishReason::ToolCalls,
        "content_filter" => FinishReason::ContentFilter,
        _ => FinishReason::Unknown,
    }
}

/// Convert messages to the OpenAI chat format.
pub(crate) fn transform_messages(
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
            // Tool results use "tool" role with id + content
            if let Some(ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            }) = msg.content.first()
            {
                result.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": match content {
                        ToolResultContent::Text(text) => text.clone(),
                        ToolResultContent::Blocks(blocks) => serde_json::to_string(blocks)
                            .unwrap_or_else(|_| String::new()),
                    },
                    "is_error": is_error,
                }));
            }
            continue;
        }

        // Convert content blocks
        let mut converted = Vec::new();
        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    converted.push(serde_json::json!({
                        "type": "text",
                        "text": text,
                    }));
                }
                ContentBlock::Image { source, media_type } => {
                    let image = match source {
                        crate::message::ImageSource::Base64 {
                            data,
                            media_type: mt,
                        } => {
                            serde_json::json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:{};base64,{}", mt, data),
                                }
                            })
                        }
                        crate::message::ImageSource::Url { url } => serde_json::json!({
                            "type": "image_url",
                            "image_url": { "url": url }
                        }),
                    };

                    // OpenAI requires a media type for inline data URLs
                    if let Some(mt) = media_type {
                        converted.push(serde_json::json!({
                            "type": "image_url",
                            "image_url": {
                                "url": match source {
                                    crate::message::ImageSource::Base64 { data, .. } => {
                                        format!("data:{};base64,{}", mt, data)
                                    }
                                    crate::message::ImageSource::Url { url } => url.clone(),
                                },
                            }
                        }));
                    } else {
                        converted.push(image);
                    }
                }
                ContentBlock::ToolUse { id, name, input } => {
                    converted.push(serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": input
                    }));
                }
                ContentBlock::ToolResult { .. } => {
                    // Tool results handled by "tool" role above
                }
                ContentBlock::Reasoning { text, .. } => {
                    converted.push(serde_json::json!({
                        "type": "text",
                        "text": text,
                    }));
                }
            }
        }

        // If only text content, send as string instead of array
        if converted.len() == 1 && converted[0].get("type") == Some(&serde_json::json!("text")) {
            if let Some(text) = converted[0].get("text") {
                result.push(serde_json::json!({
                    "role": role,
                    "content": text,
                }));
                continue;
            }
        }

        result.push(serde_json::json!({
            "role": role,
            "content": converted,
        }));
    }

    Ok(serde_json::Value::Array(result))
}

/// Convert tools to OpenAI function-calling format.
pub(crate) fn transform_tools(tools: &[Tool]) -> Result<serde_json::Value, ProviderError> {
    let converted: Vec<serde_json::Value> = tools
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

    Ok(serde_json::Value::Array(converted))
}

/// Convert tool choice to OpenAI format.
pub(crate) fn transform_tool_choice(
    choice: &ToolChoice,
) -> Result<serde_json::Value, ProviderError> {
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

/// Streaming adapter for OpenAI-compatible responses.
pub struct OpenAIStreamAdapter<S> {
    inner: SseStream<S>,
    model: String,
    provider: String,
    started: bool,
    current_content_id: Option<String>,
    content_counter: usize,
    current_tool_calls: Vec<ToolCallState>,
    usage: Usage,
    pending: VecDeque<StreamEvent>,
}

impl<S> OpenAIStreamAdapter<S> {
    /// Create a new adapter.
    pub fn new(inner: SseStream<S>, model: String, provider: impl Into<String>) -> Self {
        Self {
            inner,
            model,
            provider: provider.into(),
            started: false,
            current_content_id: None,
            content_counter: 0,
            current_tool_calls: Vec::new(),
            usage: Usage::default(),
            pending: VecDeque::new(),
        }
    }

    fn convert_event(&mut self, data: &str) -> Result<Vec<StreamEvent>, ProviderError> {
        let chunk: OpenAIStreamChunk =
            serde_json::from_str(data).map_err(|e| ProviderError::Stream(e.to_string()))?;

        let mut events = Vec::new();

        if !self.started {
            self.started = true;
            events.push(StreamEvent::Start {
                model: chunk.model.clone().unwrap_or_else(|| self.model.clone()),
                provider: self.provider.clone(),
            });
        }

        if let Some(usage) = chunk.usage {
            self.usage.input_tokens = usage.prompt_tokens.unwrap_or(0);
            self.usage.output_tokens = usage.completion_tokens.unwrap_or(0);
        }

        for choice in chunk.choices {
            if let Some(reason) = choice.finish_reason {
                let finish_reason = map_finish_reason(&reason);

                if matches!(finish_reason, FinishReason::ToolCalls) {
                    events.extend(self.flush_tool_calls());
                }

                events.push(StreamEvent::FinishStep {
                    finish_reason,
                    usage: self.usage.clone(),
                    provider_metadata: None,
                });
                continue;
            }

            if let Some(delta) = choice.delta {
                if let Some(content) = delta.content {
                    let emit_start = self.current_content_id.is_none();
                    let id = self
                        .current_content_id
                        .get_or_insert_with(|| {
                            let id = format!("content_{}", self.content_counter);
                            self.content_counter += 1;
                            id
                        })
                        .clone();

                    if emit_start {
                        events.push(StreamEvent::TextStart { id: id.clone() });
                    }

                    events.push(StreamEvent::TextDelta { id, delta: content });
                }

                if let Some(tool_calls) = delta.tool_calls {
                    for tc in tool_calls {
                        let index = tc.index.unwrap_or(0);

                        while self.current_tool_calls.len() <= index {
                            self.current_tool_calls
                                .push(ToolCallState::new(self.current_tool_calls.len()));
                        }

                        let current = &mut self.current_tool_calls[index];

                        if let Some(id) = tc.id {
                            current.id = id;
                        }

                        if let Some(function) = tc.function {
                            if let Some(name) = function.name {
                                if current.name.is_empty() {
                                    current.name = name.clone();
                                }

                                if !current.started && !current.name.is_empty() {
                                    current.started = true;
                                    events.push(StreamEvent::ToolInputStart {
                                        id: current.id.clone(),
                                        tool_name: current.name.clone(),
                                    });
                                }
                            }

                            if let Some(args) = function.arguments {
                                if !args.is_empty() {
                                    current.arguments.push_str(&args);
                                    events.push(StreamEvent::ToolInputDelta {
                                        id: current.id.clone(),
                                        delta: args,
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

    fn flush_tool_calls(&mut self) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        for state in self.current_tool_calls.drain(..) {
            if state.started {
                events.push(StreamEvent::ToolInputEnd {
                    id: state.id.clone(),
                });
            }

            let input = serde_json::from_str(&state.arguments)
                .unwrap_or_else(|_| serde_json::Value::String(state.arguments.clone()));

            events.push(StreamEvent::ToolCall {
                tool_call_id: state.id,
                tool_name: state.name,
                input,
                provider_metadata: None,
            });
        }

        events
    }
}

impl<S> Stream for OpenAIStreamAdapter<S>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<StreamEvent, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Drain any queued events first
        if let Some(event) = self.pending.pop_front() {
            return Poll::Ready(Some(Ok(event)));
        }

        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(sse_event))) => {
                    if sse_event.is_done() {
                        let mut finish_events = VecDeque::new();

                        if !self.current_tool_calls.is_empty() {
                            let calls = self.flush_tool_calls();
                            finish_events.extend(calls);
                        }

                        finish_events.push_back(StreamEvent::Finish {
                            finish_reason: FinishReason::Stop,
                            usage: self.usage.clone(),
                            provider_metadata: None,
                        });

                        self.pending.extend(finish_events);

                        if let Some(event) = self.pending.pop_front() {
                            return Poll::Ready(Some(Ok(event)));
                        }
                        continue;
                    }

                    match self.convert_event(&sse_event.data) {
                        Ok(mut events) => {
                            self.pending.extend(events.drain(..));
                            if let Some(event) = self.pending.pop_front() {
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

/// Internal tool call accumulation state.
struct ToolCallState {
    id: String,
    name: String,
    arguments: String,
    started: bool,
}

impl ToolCallState {
    fn new(index: usize) -> Self {
        Self {
            id: format!("tool_call_{}", index),
            name: String::new(),
            arguments: String::new(),
            started: false,
        }
    }
}

// ============================================================================
// OpenAI API Types
// ============================================================================

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIStreamChunk {
    id: Option<String>,
    model: Option<String>,
    #[serde(default)]
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIChoice {
    index: Option<usize>,
    delta: Option<OpenAIDelta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIDelta {
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
#[allow(dead_code)]
struct OpenAIUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_messages_includes_system() {
        let messages = vec![Message::user("Hello"), Message::assistant("Hi")];
        let result = transform_messages(&messages, Some("Be nice")).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["role"], "system");
    }

    #[test]
    fn test_map_finish_reason() {
        assert_eq!(map_finish_reason("stop"), FinishReason::Stop);
        assert_eq!(map_finish_reason("length"), FinishReason::Length);
        assert_eq!(map_finish_reason("tool_calls"), FinishReason::ToolCalls);
        assert_eq!(map_finish_reason("other"), FinishReason::Unknown);
    }
}
