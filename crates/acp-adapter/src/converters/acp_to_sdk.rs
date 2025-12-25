//! Convert ACP session notifications to SDK messages
//!
//! Transforms ACP `SessionNotification` types into claude-agent-sdk `SdkMessage`
//! for trajectory collection and compatibility with existing tooling.

use agent_client_protocol_schema as acp;
use claude_agent_sdk::{SdkAssistantMessage, SdkMessage, SdkToolProgressMessage, SdkUserMessage};

/// Convert an ACP session notification to an SDK message
///
/// Returns `None` for notification types that don't have an SDK equivalent.
pub fn notification_to_sdk_message(notification: &acp::SessionNotification) -> Option<SdkMessage> {
    let session_id = notification.session_id.to_string();

    match &notification.update {
        acp::SessionUpdate::AgentMessageChunk(chunk) => {
            let content = content_block_to_json(&chunk.content);
            Some(SdkMessage::Assistant(SdkAssistantMessage {
                message: serde_json::json!({ "content": [content] }),
                parent_tool_use_id: None,
                error: None,
                uuid: uuid::Uuid::new_v4().to_string(),
                session_id,
            }))
        }

        acp::SessionUpdate::AgentThoughtChunk(chunk) => {
            let text = match &chunk.content {
                acp::ContentBlock::Text(t) => t.text.clone(),
                _ => return None,
            };

            Some(SdkMessage::Assistant(SdkAssistantMessage {
                message: serde_json::json!({
                    "content": [{
                        "type": "thinking",
                        "thinking": text
                    }]
                }),
                parent_tool_use_id: None,
                error: None,
                uuid: uuid::Uuid::new_v4().to_string(),
                session_id,
            }))
        }

        acp::SessionUpdate::UserMessageChunk(chunk) => {
            let content = content_block_to_json(&chunk.content);
            Some(SdkMessage::User(SdkUserMessage {
                message: serde_json::json!({ "content": [content] }),
                parent_tool_use_id: None,
                is_synthetic: Some(false),
                tool_use_result: None,
                uuid: Some(uuid::Uuid::new_v4().to_string()),
                session_id,
                is_replay: None,
            }))
        }

        acp::SessionUpdate::ToolCall(tool_call) => {
            // ACP ToolCall has tool_call_id and title (not id and name)
            // raw_input contains the arguments
            Some(SdkMessage::Assistant(SdkAssistantMessage {
                message: serde_json::json!({
                    "content": [{
                        "type": "tool_use",
                        "id": tool_call.tool_call_id.to_string(),
                        "name": tool_call.title,
                        "input": tool_call.raw_input.clone().unwrap_or_default()
                    }]
                }),
                parent_tool_use_id: None,
                error: None,
                uuid: uuid::Uuid::new_v4().to_string(),
                session_id,
            }))
        }

        acp::SessionUpdate::ToolCallUpdate(update) => {
            // Check if this is a completed update (has status in fields)
            let status = update.fields.status;
            let has_output = update.fields.raw_output.is_some();

            if has_output
                || matches!(
                    status,
                    Some(acp::ToolCallStatus::Completed | acp::ToolCallStatus::Failed)
                )
            {
                // Tool result - convert to user message with tool_result
                let is_error = matches!(status, Some(acp::ToolCallStatus::Failed));
                let result_content = update.fields.raw_output.clone().unwrap_or_default();

                Some(SdkMessage::User(SdkUserMessage {
                    message: serde_json::json!({
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": update.tool_call_id.to_string(),
                            "content": result_content,
                            "is_error": is_error
                        }]
                    }),
                    parent_tool_use_id: Some(update.tool_call_id.to_string()),
                    is_synthetic: Some(true),
                    tool_use_result: None,
                    uuid: Some(uuid::Uuid::new_v4().to_string()),
                    session_id,
                    is_replay: None,
                }))
            } else {
                // Tool in progress
                Some(SdkMessage::ToolProgress(SdkToolProgressMessage {
                    tool_use_id: update.tool_call_id.to_string(),
                    tool_name: update.fields.title.clone().unwrap_or_default(),
                    parent_tool_use_id: None,
                    elapsed_time_seconds: 0.0,
                    uuid: uuid::Uuid::new_v4().to_string(),
                    session_id,
                }))
            }
        }

        acp::SessionUpdate::Plan(plan) => {
            // Convert plan to a synthetic assistant message describing todos
            let todos: Vec<String> = plan
                .entries
                .iter()
                .map(|entry| {
                    let status = match entry.status {
                        acp::PlanEntryStatus::Pending => "pending",
                        acp::PlanEntryStatus::InProgress => "in_progress",
                        acp::PlanEntryStatus::Completed => "completed",
                        _ => "pending",
                    };
                    format!("[{}] {}", status, entry.content)
                })
                .collect();

            Some(SdkMessage::Assistant(SdkAssistantMessage {
                message: serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Plan:\n{}", todos.join("\n"))
                    }]
                }),
                parent_tool_use_id: None,
                error: None,
                uuid: uuid::Uuid::new_v4().to_string(),
                session_id,
            }))
        }

        // These don't have SDK equivalents
        acp::SessionUpdate::AvailableCommandsUpdate(_)
        | acp::SessionUpdate::CurrentModeUpdate(_) => None,

        // Handle any future variants gracefully
        _ => None,
    }
}

/// Convert an ACP content block to JSON for SDK message format
fn content_block_to_json(block: &acp::ContentBlock) -> serde_json::Value {
    match block {
        acp::ContentBlock::Text(text) => {
            serde_json::json!({
                "type": "text",
                "text": text.text
            })
        }
        acp::ContentBlock::Image(image) => {
            serde_json::json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": image.mime_type,
                    "data": image.data
                }
            })
        }
        acp::ContentBlock::Resource(resource) => {
            serde_json::json!({
                "type": "resource",
                "resource": resource
            })
        }
        acp::ContentBlock::ResourceLink(link) => {
            serde_json::json!({
                "type": "resource_link",
                "uri": link.uri
            })
        }
        acp::ContentBlock::Audio(audio) => {
            serde_json::json!({
                "type": "audio",
                "data": audio.data,
                "mime_type": audio.mime_type
            })
        }
        // Handle any future variants gracefully
        _ => {
            serde_json::json!({
                "type": "unknown"
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_agent_message() {
        let session_id = acp::SessionId::new("test-session");
        let notification = acp::SessionNotification::new(
            session_id,
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("Hello from agent".to_string()),
            ))),
        );

        let sdk_msg = notification_to_sdk_message(&notification);
        assert!(sdk_msg.is_some());

        match sdk_msg.unwrap() {
            SdkMessage::Assistant(asst) => {
                let content = asst.message.get("content").unwrap();
                let text = content[0].get("text").unwrap().as_str().unwrap();
                assert_eq!(text, "Hello from agent");
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_convert_tool_call() {
        let session_id = acp::SessionId::new("test-session");
        let notification = acp::SessionNotification::new(
            session_id,
            acp::SessionUpdate::ToolCall(
                acp::ToolCall::new(acp::ToolCallId::new("call-123"), "Read file")
                    .raw_input(serde_json::json!({"file_path": "/test.txt"})),
            ),
        );

        let sdk_msg = notification_to_sdk_message(&notification);
        assert!(sdk_msg.is_some());

        match sdk_msg.unwrap() {
            SdkMessage::Assistant(asst) => {
                let content = asst.message.get("content").unwrap();
                let tool_use = &content[0];
                assert_eq!(tool_use.get("type").unwrap(), "tool_use");
                assert_eq!(tool_use.get("name").unwrap(), "Read file");
            }
            _ => panic!("Expected Assistant message"),
        }
    }
}
