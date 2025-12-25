//! Convert SDK messages to ACP session notifications
//!
//! Transforms claude-agent-sdk `SdkMessage` types into ACP `SessionNotification`
//! for unified protocol handling.

use agent_client_protocol_schema as acp;
use claude_agent_sdk::SdkMessage;

/// Convert an SDK message to an ACP session notification
///
/// Returns `None` for message types that don't have an ACP equivalent.
pub fn sdk_message_to_notification(
    session_id: &acp::SessionId,
    msg: &SdkMessage,
) -> Option<acp::SessionNotification> {
    match msg {
        SdkMessage::Assistant(asst) => {
            // Parse content blocks from assistant message
            if let Some(content) = asst.message.get("content").and_then(|c| c.as_array()) {
                for block in content {
                    let block_type = block.get("type").and_then(|t| t.as_str());

                    match block_type {
                        Some("text") => {
                            let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            return Some(acp::SessionNotification::new(
                                session_id.clone(),
                                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                                    acp::ContentBlock::Text(acp::TextContent::new(
                                        text.to_string(),
                                    )),
                                )),
                            ));
                        }
                        Some("tool_use") => {
                            let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                            let input = block.get("input").cloned().unwrap_or_default();

                            // ACP ToolCall uses tool_call_id and title, not id and name
                            return Some(acp::SessionNotification::new(
                                session_id.clone(),
                                acp::SessionUpdate::ToolCall(
                                    acp::ToolCall::new(
                                        acp::ToolCallId::new(id.to_string()),
                                        name.to_string(), // title
                                    )
                                    .raw_input(input)
                                    .status(acp::ToolCallStatus::InProgress),
                                ),
                            ));
                        }
                        Some("thinking") => {
                            let text = block.get("thinking").and_then(|t| t.as_str()).unwrap_or("");
                            return Some(acp::SessionNotification::new(
                                session_id.clone(),
                                acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
                                    acp::ContentBlock::Text(acp::TextContent::new(
                                        text.to_string(),
                                    )),
                                )),
                            ));
                        }
                        _ => {}
                    }
                }
            }
            None
        }
        SdkMessage::User(user) => {
            // User message - extract content
            if let Some(content) = user.message.get("content") {
                if let Some(text) = content.as_str() {
                    return Some(acp::SessionNotification::new(
                        session_id.clone(),
                        acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(text.to_string())),
                        )),
                    ));
                } else if let Some(arr) = content.as_array() {
                    for block in arr {
                        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                return Some(acp::SessionNotification::new(
                                    session_id.clone(),
                                    acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(
                                        acp::ContentBlock::Text(acp::TextContent::new(
                                            text.to_string(),
                                        )),
                                    )),
                                ));
                            }
                        }
                    }
                }
            }
            None
        }
        SdkMessage::Result(_result) => {
            // Session complete - this is returned separately in PromptResponse
            // We can't easily convert this to a notification
            None
        }
        SdkMessage::ToolProgress(progress) => {
            // Tool in progress - create update with InProgress status
            let fields = acp::ToolCallUpdateFields::default();
            Some(acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                    acp::ToolCallId::new(progress.tool_use_id.clone()),
                    fields,
                )),
            ))
        }
        SdkMessage::System(_) => {
            // System messages (init, etc.) don't map to session updates
            None
        }
        SdkMessage::StreamEvent(_) => {
            // Stream events are internal, handled separately
            None
        }
        SdkMessage::AuthStatus(_) => {
            // Auth status doesn't map to ACP
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_assistant_text() {
        let session_id = acp::SessionId::new("test-session");
        let msg = SdkMessage::Assistant(claude_agent_sdk::SdkAssistantMessage {
            message: serde_json::json!({
                "content": [{"type": "text", "text": "Hello, world!"}]
            }),
            parent_tool_use_id: None,
            error: None,
            uuid: "123".to_string(),
            session_id: "test".to_string(),
        });

        let notification = sdk_message_to_notification(&session_id, &msg);
        assert!(notification.is_some());

        let notification = notification.unwrap();
        if let acp::SessionUpdate::AgentMessageChunk(chunk) = notification.update {
            if let acp::ContentBlock::Text(text) = chunk.content {
                assert_eq!(text.text, "Hello, world!");
            } else {
                panic!("Expected text content");
            }
        } else {
            panic!("Expected AgentMessageChunk");
        }
    }
}
