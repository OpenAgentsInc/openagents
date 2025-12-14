//! LLM Provider-based session management for MechaCoder.
//!
//! This module provides session handling using the `llm` crate's provider system,
//! converting StreamEvents to ServerMessages for WebSocket delivery.

use futures::StreamExt;
use llm::{
    CompletionRequest, ContentBlock, Message, ProviderRegistry, Role, StreamEvent, Tool,
};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::ServerMessage;

/// Run an LLM session using the provider registry.
pub async fn run_provider_session(
    provider_id: &str,
    model: &str,
    message: String,
    system_prompt: Option<String>,
    tools: Vec<Tool>,
    tx: mpsc::UnboundedSender<ServerMessage>,
    registry: Arc<ProviderRegistry>,
) {
    tracing::debug!(
        provider = %provider_id,
        model = %model,
        "Starting provider session"
    );

    let provider = match registry.get(provider_id).await {
        Some(p) => p,
        None => {
            tracing::error!(provider = %provider_id, "Provider not found");
            let _ = tx.send(ServerMessage::Done {
                error: Some(format!("Provider not found: {}", provider_id)),
            });
            return;
        }
    };

    // Build the completion request
    let mut request = CompletionRequest::new(model)
        .message(Message::user(&message));

    if let Some(system) = system_prompt {
        request = request.system(&system);
    }

    for tool in tools {
        request = request.tool(tool);
    }

    // Start streaming
    let stream = match provider.stream(request).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "Failed to start stream");
            let _ = tx.send(ServerMessage::Done {
                error: Some(format!("Failed to start stream: {}", e)),
            });
            return;
        }
    };

    futures::pin_mut!(stream);

    let mut current_tool_id = String::new();
    let mut current_tool_name = String::new();
    let mut accumulated_tool_input = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(event) => {
                let messages = convert_stream_event(
                    event,
                    &mut current_tool_id,
                    &mut current_tool_name,
                    &mut accumulated_tool_input,
                );
                for msg in messages {
                    if tx.send(msg).is_err() {
                        tracing::warn!("Channel closed");
                        return;
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Stream error");
                let _ = tx.send(ServerMessage::Done {
                    error: Some(format!("Stream error: {}", e)),
                });
                return;
            }
        }
    }

    let _ = tx.send(ServerMessage::Done { error: None });
}

/// Convert a StreamEvent to ServerMessage(s).
fn convert_stream_event(
    event: StreamEvent,
    current_tool_id: &mut String,
    current_tool_name: &mut String,
    accumulated_tool_input: &mut String,
) -> Vec<ServerMessage> {
    let mut out = Vec::new();

    match event {
        StreamEvent::Start { .. } => {
            // Session init is handled separately
        }

        StreamEvent::TextDelta { delta, .. } => {
            out.push(ServerMessage::TextDelta { text: delta });
        }

        StreamEvent::ToolInputStart { id, tool_name } => {
            *current_tool_id = id.clone();
            *current_tool_name = tool_name.clone();
            accumulated_tool_input.clear();
            out.push(ServerMessage::ToolStart {
                tool_use_id: id,
                tool_name,
            });
        }

        StreamEvent::ToolInputDelta { delta, .. } => {
            accumulated_tool_input.push_str(&delta);
            out.push(ServerMessage::ToolInput {
                tool_use_id: current_tool_id.clone(),
                partial_json: delta,
            });
        }

        StreamEvent::ToolInputEnd { .. } => {
            // Tool input complete - the full input is in accumulated_tool_input
        }

        StreamEvent::ToolCall {
            tool_call_id,
            tool_name,
            ..
        } => {
            // Complete tool call ready for execution
            out.push(ServerMessage::ToolStart {
                tool_use_id: tool_call_id,
                tool_name,
            });
        }

        StreamEvent::ToolResult {
            tool_call_id,
            result,
            is_error,
            ..
        } => {
            let output = match result {
                llm::ToolResultContent::Text(text) => text,
                llm::ToolResultContent::Blocks(blocks) => blocks
                    .iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            out.push(ServerMessage::ToolResult {
                tool_use_id: tool_call_id,
                output,
                is_error,
            });
        }

        StreamEvent::Finish { .. } | StreamEvent::FinishStep { .. } => {
            // Handled by the main loop sending Done
        }

        StreamEvent::Error { error } => {
            out.push(ServerMessage::Done {
                error: Some(error.message),
            });
        }

        // Events we don't map to ServerMessage
        StreamEvent::TextStart { .. }
        | StreamEvent::TextEnd { .. }
        | StreamEvent::ReasoningStart { .. }
        | StreamEvent::ReasoningDelta { .. }
        | StreamEvent::ReasoningEnd { .. } => {}
    }

    out
}

/// Run a multi-turn conversation with tool execution.
pub async fn run_conversation(
    provider_id: &str,
    model: &str,
    messages: Vec<Message>,
    system_prompt: Option<String>,
    tools: Vec<Tool>,
    tx: mpsc::UnboundedSender<ServerMessage>,
    registry: Arc<ProviderRegistry>,
    max_turns: usize,
) {
    let mut conversation = messages;

    for _turn in 0..max_turns {

        let provider = match registry.get(provider_id).await {
            Some(p) => p,
            None => {
                let _ = tx.send(ServerMessage::Done {
                    error: Some(format!("Provider not found: {}", provider_id)),
                });
                return;
            }
        };

        // Build request with current conversation
        let mut request = CompletionRequest::new(model);

        if let Some(ref system) = system_prompt {
            request = request.system(system);
        }

        for msg in &conversation {
            request.messages.push(msg.clone());
        }

        for tool in &tools {
            request = request.tool(tool.clone());
        }

        // Stream the response
        let stream = match provider.stream(request).await {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(ServerMessage::Done {
                    error: Some(format!("Stream error: {}", e)),
                });
                return;
            }
        };

        futures::pin_mut!(stream);

        let mut assistant_text = String::new();
        let mut tool_calls: Vec<ToolCallInfo> = Vec::new();
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_tool_input = String::new();
        let mut has_tool_use = false;

        while let Some(result) = stream.next().await {
            match result {
                Ok(event) => {
                    match &event {
                        StreamEvent::TextDelta { delta, .. } => {
                            assistant_text.push_str(delta);
                        }
                        StreamEvent::ToolInputStart { id, tool_name } => {
                            has_tool_use = true;
                            current_tool_id = id.clone();
                            current_tool_name = tool_name.clone();
                            current_tool_input.clear();
                        }
                        StreamEvent::ToolInputDelta { delta, .. } => {
                            current_tool_input.push_str(delta);
                        }
                        StreamEvent::ToolInputEnd { .. } => {
                            // Parse and store tool call
                            if let Ok(input) = serde_json::from_str(&current_tool_input) {
                                tool_calls.push(ToolCallInfo {
                                    id: current_tool_id.clone(),
                                    name: current_tool_name.clone(),
                                    input,
                                });
                            }
                        }
                        StreamEvent::ToolCall {
                            tool_call_id,
                            tool_name,
                            input,
                            ..
                        } => {
                            has_tool_use = true;
                            tool_calls.push(ToolCallInfo {
                                id: tool_call_id.clone(),
                                name: tool_name.clone(),
                                input: input.clone(),
                            });
                        }
                        _ => {}
                    }

                    // Convert and send
                    let messages = convert_stream_event(
                        event,
                        &mut current_tool_id,
                        &mut current_tool_name,
                        &mut current_tool_input,
                    );
                    for msg in messages {
                        if tx.send(msg).is_err() {
                            return;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(ServerMessage::Done {
                        error: Some(format!("Stream error: {}", e)),
                    });
                    return;
                }
            }
        }

        // Add assistant message to conversation
        let mut assistant_content = Vec::new();
        if !assistant_text.is_empty() {
            assistant_content.push(ContentBlock::Text {
                text: assistant_text.clone(),
            });
        }
        for tc in &tool_calls {
            assistant_content.push(ContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input: tc.input.clone(),
            });
        }

        if !assistant_content.is_empty() {
            conversation.push(Message {
                role: Role::Assistant,
                content: assistant_content,
            });
        }

        // If no tool use, we're done
        if !has_tool_use {
            break;
        }

        // Tool execution would happen here - for now we just note the tool calls
        // The actual tool execution should be handled by the caller
        break;
    }

    let _ = tx.send(ServerMessage::Done { error: None });
}

/// Information about a tool call.
#[derive(Debug, Clone)]
struct ToolCallInfo {
    id: String,
    name: String,
    input: serde_json::Value,
}
