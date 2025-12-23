//! Convert between ACP notifications and rlog format
//!
//! Provides bidirectional conversion between ACP session notifications
//! and the rlog (recorder log) line format used for trajectory storage.

use agent_client_protocol_schema as acp;

/// Convert an ACP session notification to an rlog line
///
/// Returns `None` for notification types that don't map to rlog format.
pub fn notification_to_rlog_line(notification: &acp::SessionNotification) -> Option<String> {
    match &notification.update {
        acp::SessionUpdate::UserMessageChunk(chunk) => {
            let text = extract_text(&chunk.content)?;
            let truncated = truncate_line(&text, 200);
            Some(format!("u: {}", truncated))
        }

        acp::SessionUpdate::AgentMessageChunk(chunk) => {
            let text = extract_text(&chunk.content)?;
            let truncated = truncate_line(&text, 200);
            Some(format!("a: {}", truncated))
        }

        acp::SessionUpdate::AgentThoughtChunk(chunk) => {
            let text = extract_text(&chunk.content)?;
            let truncated = truncate_line(&text, 150);
            // Generate a short signature from the content
            let sig = generate_signature(&text);
            Some(format!("th: {} sig={}", truncated, sig))
        }

        acp::SessionUpdate::ToolCall(tool_call) => {
            let id_short = truncate_id(&tool_call.tool_call_id.to_string());
            let args = if let Some(args) = &tool_call.raw_input {
                truncate_line(&args.to_string(), 50)
            } else {
                String::new()
            };
            Some(format!(
                "t!:{} id={} {} → [running]",
                tool_call.title, id_short, args
            ))
        }

        acp::SessionUpdate::ToolCallUpdate(update) => {
            let id_short = truncate_id(&update.tool_call_id.to_string());

            if let Some(result) = &update.fields.raw_output {
                let status = match update.fields.status {
                    Some(acp::ToolCallStatus::Failed) => "[error]",
                    Some(acp::ToolCallStatus::Completed) => "[ok]",
                    _ => "[ok]",
                };

                let output = truncate_line(&result.to_string(), 100);
                Some(format!("o: id={} → {} {}", id_short, status, output))
            } else {
                // In-progress update, no output line needed
                None
            }
        }

        acp::SessionUpdate::Plan(plan) => {
            let items: Vec<String> = plan
                .entries
                .iter()
                .map(|entry| {
                    let status = match entry.status {
                        acp::PlanEntryStatus::Pending => "pending",
                        acp::PlanEntryStatus::InProgress => "in_progress",
                        acp::PlanEntryStatus::Completed => "completed",
                        _ => "pending",
                    };
                    format!("[{}] {}", status, truncate_line(&entry.content, 50))
                })
                .collect();
            Some(format!("td: {}", items.join(" ")))
        }

        acp::SessionUpdate::CurrentModeUpdate(mode) => {
            Some(format!("# mode: {}", mode.current_mode_id))
        }

        // These don't have rlog equivalents
        acp::SessionUpdate::AvailableCommandsUpdate(_) => None,

        _ => None,
    }
}

/// Convert an rlog line to an ACP session notification
///
/// Parses the rlog line format and creates the corresponding notification.
pub fn rlog_line_to_notification(
    session_id: &acp::SessionId,
    line: &str,
) -> Option<acp::SessionNotification> {
    let line = line.trim();

    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    // Parse line type based on prefix
    if let Some(content) = line.strip_prefix("u: ") {
        return Some(acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(content.to_string()),
            ))),
        ));
    }

    if let Some(content) = line.strip_prefix("a: ") {
        return Some(acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(content.to_string()),
            ))),
        ));
    }

    if let Some(content) = line.strip_prefix("th: ") {
        // Remove signature if present
        let text = if let Some(idx) = content.rfind(" sig=") {
            &content[..idx]
        } else {
            content
        };

        return Some(acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(text.to_string()),
            ))),
        ));
    }

    if let Some(content) = line.strip_prefix("t!:") {
        // Parse: t!:ToolName id=xxx args → [running]
        return parse_tool_start(session_id, content);
    }

    if line.starts_with("t:") {
        // Completed tool: t:ToolName id=xxx args → [ok|error]
        let content = &line[2..];
        return parse_tool_complete(session_id, content);
    }

    if let Some(content) = line.strip_prefix("o: ") {
        // Observation: o: id=xxx → [ok|error] output
        return parse_observation(session_id, content);
    }

    if let Some(content) = line.strip_prefix("td: ") {
        // Todo list
        return parse_todo_list(session_id, content);
    }

    None
}

/// Extract text from a content block
fn extract_text(block: &acp::ContentBlock) -> Option<String> {
    match block {
        acp::ContentBlock::Text(text) => Some(text.text.clone()),
        _ => None,
    }
}

/// Truncate a line to a maximum length, adding "..." if truncated
fn truncate_line(s: &str, max_len: usize) -> String {
    // Take first line only
    let first_line = s.lines().next().unwrap_or(s);

    if first_line.len() > max_len {
        format!("{}...", &first_line[..max_len.saturating_sub(3)])
    } else {
        first_line.to_string()
    }
}

/// Truncate an ID to first 8 characters
fn truncate_id(id: &str) -> String {
    if id.len() > 8 {
        id[..8].to_string()
    } else {
        id.to_string()
    }
}

/// Generate a short signature from content
fn generate_signature(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    let hash = hasher.finish();

    // Convert to base64-like string (first 8 chars)
    format!("{:016x}", hash)[..8].to_string()
}

/// Parse a tool start line
fn parse_tool_start(
    session_id: &acp::SessionId,
    content: &str,
) -> Option<acp::SessionNotification> {
    // Format: ToolName id=xxx args → [running]
    let parts: Vec<&str> = content.splitn(2, ' ').collect();
    let tool_name = parts.first()?.to_string();

    // Extract ID if present
    let id = if content.contains("id=") {
        let id_start = content.find("id=")? + 3;
        let id_end = content[id_start..].find(' ').unwrap_or(content.len() - id_start);
        content[id_start..id_start + id_end].to_string()
    } else {
        uuid::Uuid::new_v4().to_string()
    };

    Some(acp::SessionNotification::new(
        session_id.clone(),
        acp::SessionUpdate::ToolCall(
            acp::ToolCall::new(acp::ToolCallId::new(id), tool_name)
                .status(acp::ToolCallStatus::InProgress),
        ),
    ))
}

/// Parse a completed tool line
fn parse_tool_complete(
    session_id: &acp::SessionId,
    content: &str,
) -> Option<acp::SessionNotification> {
    // Similar to tool start but with completion status
    parse_tool_start(session_id, content).map(|mut notif| {
        if let acp::SessionUpdate::ToolCall(ref mut tool) = notif.update {
            let status = if content.contains("[error]") {
                acp::ToolCallStatus::Failed
            } else {
                acp::ToolCallStatus::Completed
            };
            *tool = tool.clone().status(status);
        }
        notif
    })
}

/// Parse an observation line
fn parse_observation(
    session_id: &acp::SessionId,
    content: &str,
) -> Option<acp::SessionNotification> {
    // Format: id=xxx → [ok|error] output
    let id = if content.contains("id=") {
        let id_start = content.find("id=")? + 3;
        let id_end = content[id_start..]
            .find(' ')
            .unwrap_or(content.len() - id_start);
        content[id_start..id_start + id_end].to_string()
    } else {
        return None;
    };

    let is_error = content.contains("[error]");
    let status = if is_error {
        acp::ToolCallStatus::Failed
    } else {
        acp::ToolCallStatus::Completed
    };

    // Extract output after status
    let output = if let Some(idx) = content.find(']') {
        content[idx + 1..].trim().to_string()
    } else {
        String::new()
    };

    // Create ToolCallUpdateFields with status and raw_output
    let mut fields = acp::ToolCallUpdateFields::default();
    fields.status = Some(status);
    fields.raw_output = Some(serde_json::json!(output));

    Some(acp::SessionNotification::new(
        session_id.clone(),
        acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
            acp::ToolCallId::new(id),
            fields,
        )),
    ))
}

/// Parse a todo list line
fn parse_todo_list(
    session_id: &acp::SessionId,
    content: &str,
) -> Option<acp::SessionNotification> {
    // Format: [status] item [status] item ...
    let mut entries = Vec::new();

    // Simple parsing - find [status] patterns
    let mut remaining = content;
    while let Some(start) = remaining.find('[') {
        if let Some(end) = remaining[start..].find(']') {
            let status_str = &remaining[start + 1..start + end];
            let status = match status_str {
                "completed" => acp::PlanEntryStatus::Completed,
                "in_progress" => acp::PlanEntryStatus::InProgress,
                _ => acp::PlanEntryStatus::Pending,
            };

            // Find content until next [ or end
            let content_start = start + end + 1;
            let content_end = remaining[content_start..]
                .find('[')
                .unwrap_or(remaining.len() - content_start);
            let item_content = remaining[content_start..content_start + content_end].trim();

            if !item_content.is_empty() {
                entries.push(acp::PlanEntry::new(
                    item_content.to_string(),
                    acp::PlanEntryPriority::Medium,
                    status,
                ));
            }

            remaining = &remaining[content_start + content_end..];
        } else {
            break;
        }
    }

    if entries.is_empty() {
        return None;
    }

    Some(acp::SessionNotification::new(
        session_id.clone(),
        acp::SessionUpdate::Plan(acp::Plan::new(entries)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_to_rlog_user() {
        let session_id = acp::SessionId::new("test");
        let notification = acp::SessionNotification::new(
            session_id,
            acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("Hello world".to_string()),
            ))),
        );

        let line = notification_to_rlog_line(&notification);
        assert_eq!(line, Some("u: Hello world".to_string()));
    }

    #[test]
    fn test_notification_to_rlog_agent() {
        let session_id = acp::SessionId::new("test");
        let notification = acp::SessionNotification::new(
            session_id,
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("I'll help you".to_string()),
            ))),
        );

        let line = notification_to_rlog_line(&notification);
        assert_eq!(line, Some("a: I'll help you".to_string()));
    }

    #[test]
    fn test_rlog_to_notification_user() {
        let session_id = acp::SessionId::new("test");
        let notification = rlog_line_to_notification(&session_id, "u: Hello world");

        assert!(notification.is_some());
        if let acp::SessionUpdate::UserMessageChunk(chunk) = notification.unwrap().update {
            if let acp::ContentBlock::Text(text) = chunk.content {
                assert_eq!(text.text, "Hello world");
            } else {
                panic!("Expected text content");
            }
        } else {
            panic!("Expected UserMessageChunk");
        }
    }

    #[test]
    fn test_roundtrip_agent_message() {
        let session_id = acp::SessionId::new("test");
        let original = acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("Test message".to_string()),
            ))),
        );

        let line = notification_to_rlog_line(&original).unwrap();
        let reconstructed = rlog_line_to_notification(&session_id, &line).unwrap();

        if let (
            acp::SessionUpdate::AgentMessageChunk(orig),
            acp::SessionUpdate::AgentMessageChunk(recon),
        ) = (original.update, reconstructed.update)
        {
            let orig_text = extract_text(&orig.content).unwrap();
            let recon_text = extract_text(&recon.content).unwrap();
            assert_eq!(orig_text, recon_text);
        } else {
            panic!("Update types don't match");
        }
    }
}
