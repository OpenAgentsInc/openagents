//! Claude SDK session management for MechaCoder.
//!
//! Handles streaming messages from the Claude Agent SDK and converting
//! them to ServerMessage types for WebSocket delivery.

use claude_agent_sdk::{query, QueryOptions, SdkMessage};
use futures::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::ServerMessage;

/// Run a Claude session and stream messages to the provided channel.
pub async fn run_claude_session(
    message: String,
    cwd: String,
    tx: mpsc::UnboundedSender<ServerMessage>,
) {
    eprintln!("[mechacoder] Starting Claude session with message: {}", message);

    let options = QueryOptions::new()
        .cwd(&cwd)
        .model("claude-haiku-4-5-20251001")
        .include_partial_messages(true);

    let stream = match query(&message, options).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[mechacoder] Failed to start session: {e}");
            let _ = tx.send(ServerMessage::Done {
                error: Some(format!("Failed to start Claude session: {e}")),
            });
            return;
        }
    };

    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        match result {
            Ok(msg) => {
                eprintln!("[mechacoder] Received SDK message: {:?}", std::mem::discriminant(&msg));
                let messages = process_sdk_message(msg);
                eprintln!("[mechacoder] Processed into {} ServerMessages", messages.len());
                for server_msg in messages {
                    eprintln!("[mechacoder] Sending: {:?}", std::mem::discriminant(&server_msg));
                    if tx.send(server_msg).is_err() {
                        eprintln!("[mechacoder] Channel closed!");
                        return;
                    }
                }
            }
            Err(e) => {
                eprintln!("[mechacoder] Stream error: {e}");
                let _ = tx.send(ServerMessage::Done {
                    error: Some(format!("Stream error: {e}")),
                });
                return;
            }
        }
    }

    eprintln!("[mechacoder] Stream ended, sending Done");
    let _ = tx.send(ServerMessage::Done { error: None });
}

/// Process an SDK message into ServerMessage(s).
fn process_sdk_message(msg: SdkMessage) -> Vec<ServerMessage> {
    let mut out = Vec::new();

    match msg {
        SdkMessage::System(system_msg) => {
            if let claude_agent_sdk::SdkSystemMessage::Init(init) = system_msg {
                out.push(ServerMessage::SessionInit {
                    session_id: init.session_id,
                });
            }
        }

        SdkMessage::StreamEvent(event) => {
            if let Some(entries) = process_stream_event(&event.event) {
                out.extend(entries);
            }
        }

        SdkMessage::ToolProgress(progress) => {
            out.push(ServerMessage::ToolProgress {
                tool_use_id: progress.tool_use_id,
                elapsed_seconds: progress.elapsed_time_seconds,
            });
        }

        SdkMessage::Assistant(assistant_msg) => {
            if let Some(entries) = process_assistant_message(&assistant_msg.message) {
                out.extend(entries);
            }
        }

        SdkMessage::Result(result) => {
            let error = match &result {
                claude_agent_sdk::SdkResultMessage::ErrorDuringExecution(e) => {
                    Some(e.errors.join(", "))
                }
                claude_agent_sdk::SdkResultMessage::ErrorMaxTurns(_) => {
                    Some("Max turns exceeded".to_string())
                }
                claude_agent_sdk::SdkResultMessage::ErrorMaxBudget(_) => {
                    Some("Max budget exceeded".to_string())
                }
                claude_agent_sdk::SdkResultMessage::ErrorMaxStructuredOutputRetries(_) => {
                    Some("Max structured output retries exceeded".to_string())
                }
                claude_agent_sdk::SdkResultMessage::Success(_) => None,
            };
            out.push(ServerMessage::Done { error });
        }

        SdkMessage::User(_) => {}
        SdkMessage::AuthStatus(_) => {}
    }

    out
}

/// Process a stream event into ServerMessages.
fn process_stream_event(event: &Value) -> Option<Vec<ServerMessage>> {
    let event_type = event.get("type")?.as_str()?;
    let mut out = Vec::new();

    match event_type {
        "content_block_start" => {
            if let Some(content_block) = event.get("content_block") {
                let block_type = content_block.get("type")?.as_str()?;
                if block_type == "tool_use" {
                    let tool_use_id = content_block.get("id")?.as_str()?.to_string();
                    let tool_name = content_block.get("name")?.as_str()?.to_string();
                    out.push(ServerMessage::ToolStart {
                        tool_use_id,
                        tool_name,
                    });
                }
            }
        }

        "content_block_delta" => {
            if let Some(delta) = event.get("delta") {
                let delta_type = delta.get("type")?.as_str()?;
                match delta_type {
                    "text_delta" => {
                        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                            out.push(ServerMessage::TextDelta {
                                text: text.to_string(),
                            });
                        }
                    }
                    "input_json_delta" => {
                        if let Some(json) = delta.get("partial_json").and_then(|j| j.as_str()) {
                            // We need to track which tool this is for
                            // For now, we'll send it without the ID (the client tracks active tools)
                            out.push(ServerMessage::ToolInput {
                                tool_use_id: String::new(),
                                partial_json: json.to_string(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        _ => {}
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Process an assistant message into ServerMessages.
fn process_assistant_message(message: &Value) -> Option<Vec<ServerMessage>> {
    let content = message.get("content")?.as_array()?;
    let mut out = Vec::new();

    for block in content {
        let block_type = block.get("type")?.as_str()?;
        match block_type {
            "text" => {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    out.push(ServerMessage::TextDelta {
                        text: text.to_string(),
                    });
                }
            }
            "tool_use" => {
                let tool_use_id = block.get("id")?.as_str()?.to_string();
                let tool_name = block.get("name")?.as_str()?.to_string();
                let input = block
                    .get("input")
                    .map(|i| serde_json::to_string_pretty(i).unwrap_or_default())
                    .unwrap_or_default();

                out.push(ServerMessage::ToolStart {
                    tool_use_id: tool_use_id.clone(),
                    tool_name,
                });
                if !input.is_empty() && input != "{}" {
                    out.push(ServerMessage::ToolInput {
                        tool_use_id,
                        partial_json: input,
                    });
                }
            }
            "tool_result" => {
                let tool_use_id = block.get("tool_use_id")?.as_str()?.to_string();
                let is_error = block.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false);

                let output = if let Some(content) = block.get("content") {
                    if let Some(text) = content.as_str() {
                        text.to_string()
                    } else if let Some(arr) = content.as_array() {
                        arr.iter()
                            .filter_map(|c| {
                                if c.get("type")?.as_str()? == "text" {
                                    c.get("text")?.as_str().map(|s| s.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else {
                        serde_json::to_string_pretty(content).unwrap_or_default()
                    }
                } else {
                    String::new()
                };

                out.push(ServerMessage::ToolResult {
                    tool_use_id,
                    output,
                    is_error,
                });
            }
            _ => {}
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
