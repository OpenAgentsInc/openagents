//! UI renderer for SDK messages to recorder HTML components.
//!
//! Maps SdkMessage types from claude-agent-sdk to recorder UI components
//! from the ui crate, generating HTML strings that can be served via SSE.

use claude_agent_sdk::SdkMessage;
use maud::Markup;
use serde_json::Value;
use std::collections::HashMap;
use ui::recorder::molecules::ResultType;
use ui::recorder::organisms::{AgentLine, LifecycleEvent, ToolLine, lifecycle_line};

/// Stateful UI renderer that tracks pending tool calls for result matching.
#[derive(Default)]
pub struct UiRenderer {
    /// Map of tool_use_id -> (tool_name, args)
    pending_calls: HashMap<String, (String, String)>,
}

impl UiRenderer {
    /// Create a new UI renderer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Render an SdkMessage to HTML, optionally returning an update script for SSE.
    ///
    /// Returns (html, optional_update_script).
    pub fn render(&mut self, msg: &SdkMessage) -> (Option<Markup>, Option<String>) {
        match msg {
            SdkMessage::Assistant(asst) => (self.render_assistant_message(asst), None),
            SdkMessage::User(user) => {
                // Tool results may generate update scripts
                self.render_user_message(user)
            }
            SdkMessage::ToolProgress(progress) => (render_tool_progress(progress), None),
            SdkMessage::System(sys) => (render_system_message(sys), None),
            SdkMessage::Result(result) => (render_result_message(result), None),
            _ => (None, None),
        }
    }
}

/// Render an SdkMessage to HTML using recorder components.
///
/// Note: This is a stateless convenience function. For proper tool result matching,
/// use UiRenderer instead.
pub fn render_sdk_message(msg: &SdkMessage) -> Option<Markup> {
    let mut renderer = UiRenderer::new();
    renderer.render(msg).0
}

impl UiRenderer {
    /// Render assistant messages (text blocks and tool_use).
    fn render_assistant_message(
        &mut self,
        asst: &claude_agent_sdk::SdkAssistantMessage,
    ) -> Option<Markup> {
        // Parse content blocks
        let content = asst.message.get("content")?.as_array()?;

        // For now, render only the first block (we could iterate if needed)
        for block in content {
            let block_type = block.get("type")?.as_str()?;

            match block_type {
                "text" => {
                    let text = block.get("text")?.as_str()?;
                    return Some(AgentLine::new(text).build());
                }
                "tool_use" => {
                    let tool_name = block.get("name")?.as_str()?;
                    let tool_id = block.get("id")?.as_str()?;
                    let input = block.get("input")?;

                    // Format args based on tool type
                    let args = format_tool_args(tool_name, input);

                    // Track this pending call
                    self.pending_calls
                        .insert(tool_id.to_string(), (tool_name.to_string(), args.clone()));

                    return Some(
                        ToolLine::new(tool_name, &args, ResultType::Pending)
                            .call_id(tool_id)
                            .build(),
                    );
                }
                _ => continue,
            }
        }

        None
    }

    /// Render user messages (tool results).
    ///
    /// Returns (html, optional_update_script) where update_script replaces the pending ToolLine.
    fn render_user_message(
        &mut self,
        user: &claude_agent_sdk::SdkUserMessage,
    ) -> (Option<Markup>, Option<String>) {
        let content = match user.message.get("content") {
            Some(c) => c,
            None => return (None, None),
        };

        match content {
            Value::Array(arr) => {
                // Look for tool_result blocks
                for block in arr {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                        let tool_id = match block.get("tool_use_id").and_then(|i| i.as_str()) {
                            Some(id) => id,
                            None => continue,
                        };
                        let is_error = block
                            .get("is_error")
                            .and_then(|e| e.as_bool())
                            .unwrap_or(false);

                        let result = if is_error {
                            let error_msg = extract_error_message(block);
                            ResultType::Error(error_msg)
                        } else {
                            ResultType::Ok
                        };

                        // Look up the pending call
                        if let Some((tool_name, args)) = self.pending_calls.remove(tool_id) {
                            // Generate complete ToolLine HTML
                            let complete_line = ToolLine::new(&tool_name, &args, result)
                                .call_id(tool_id)
                                .build();

                            // Generate update script for SSE
                            let update_script = format!(
                                r#"<script>
                                (function() {{
                                    const elem = document.querySelector('[data-call-id="{}"]');
                                    if (elem) {{
                                        elem.outerHTML = `{}`;
                                    }}
                                }})();
                                </script>"#,
                                tool_id,
                                complete_line.into_string().replace('`', "\\`")
                            );

                            return (None, Some(update_script));
                        } else {
                            // No pending call found - return standalone result
                            return (
                                Some(ToolLine::new("(unknown)", tool_id, result).build()),
                                None,
                            );
                        }
                    }
                }
            }
            _ => {}
        }

        (None, None)
    }
}

/// Render tool progress updates.
fn render_tool_progress(_progress: &claude_agent_sdk::SdkToolProgressMessage) -> Option<Markup> {
    // Update latency on existing tool line
    // In practice, this would trigger an SSE update to an existing ToolLine
    // For now, return None as this is an update operation
    None
}

/// Render system messages (lifecycle events).
fn render_system_message(sys: &claude_agent_sdk::SdkSystemMessage) -> Option<Markup> {
    use claude_agent_sdk::SdkSystemMessage;

    match sys {
        SdkSystemMessage::Init(init) => {
            Some(lifecycle_line(
                LifecycleEvent::Start {
                    id: init.session_id.clone(),
                    budget: 0.0, // Would need to be passed from QueryOptions
                    duration: "starting".to_string(),
                },
                None,
                None,
            ))
        }
        _ => None,
    }
}

/// Render result messages (end of session).
fn render_result_message(result: &claude_agent_sdk::SdkResultMessage) -> Option<Markup> {
    use claude_agent_sdk::SdkResultMessage;

    match result {
        SdkResultMessage::Success(success) => {
            Some(lifecycle_line(
                LifecycleEvent::End {
                    summary: success.result.clone(),
                    issues_completed: 0, // Would need to track this separately
                    prs_merged: 0,       // Would need to track this separately
                    cost: success.total_cost_usd,
                    duration: format!("{}ms", success.duration_ms),
                },
                None,
                None,
            ))
        }
        SdkResultMessage::ErrorDuringExecution(err)
        | SdkResultMessage::ErrorMaxTurns(err)
        | SdkResultMessage::ErrorMaxBudget(err)
        | SdkResultMessage::ErrorMaxStructuredOutputRetries(err) => {
            let summary = err.errors.join("; ");
            Some(lifecycle_line(
                LifecycleEvent::End {
                    summary,
                    issues_completed: 0,
                    prs_merged: 0,
                    cost: err.total_cost_usd,
                    duration: format!("{}ms", err.duration_ms),
                },
                None,
                None,
            ))
        }
    }
}

/// Format tool arguments for display.
fn format_tool_args(tool_name: &str, input: &Value) -> String {
    match tool_name {
        "Bash" => input
            .get("command")
            .and_then(|c| c.as_str())
            .map(|c| {
                let truncated = if c.len() > 50 {
                    format!("{}...", &c[..47])
                } else {
                    c.to_string()
                };
                format!("cmd=\"{}\"", truncated)
            })
            .unwrap_or_default(),
        "Read" | "Write" | "Edit" => input
            .get("file_path")
            .and_then(|p| p.as_str())
            .map(|p| format!("file_path={}", p))
            .unwrap_or_default(),
        "Glob" => input
            .get("pattern")
            .and_then(|p| p.as_str())
            .map(|p| format!("pattern=\"{}\"", p))
            .unwrap_or_default(),
        "Grep" => input
            .get("pattern")
            .and_then(|p| p.as_str())
            .map(|p| {
                let truncated = if p.len() > 30 {
                    format!("{}...", &p[..27])
                } else {
                    p.to_string()
                };
                format!("pattern=\"{}\"", truncated)
            })
            .unwrap_or_default(),
        "Task" => input
            .get("description")
            .and_then(|d| d.as_str())
            .map(|d| {
                let truncated = if d.len() > 40 {
                    format!("{}...", &d[..37])
                } else {
                    d.to_string()
                };
                format!("desc=\"{}\"", truncated)
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Extract error message from tool result.
fn extract_error_message(block: &Value) -> String {
    match block.get("content") {
        Some(Value::String(s)) => {
            // Truncate long error messages
            if s.len() > 100 {
                format!("{}...", &s[..97])
            } else {
                s.clone()
            }
        }
        Some(Value::Array(arr)) => {
            let msg = arr
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join(" ");
            if msg.len() > 100 {
                format!("{}...", &msg[..97])
            } else {
                msg
            }
        }
        _ => "Unknown error".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_agent_sdk::{SdkAssistantMessage, SdkUserMessage};
    use serde_json::json;

    #[test]
    fn test_format_tool_args_bash() {
        let input = json!({"command": "ls -la"});
        assert_eq!(format_tool_args("Bash", &input), "cmd=\"ls -la\"");
    }

    #[test]
    fn test_format_tool_args_read() {
        let input = json!({"file_path": "/path/to/file.rs"});
        assert_eq!(
            format_tool_args("Read", &input),
            "file_path=/path/to/file.rs"
        );
    }

    #[test]
    fn test_format_tool_args_glob() {
        let input = json!({"pattern": "**/*.rs"});
        assert_eq!(format_tool_args("Glob", &input), "pattern=\"**/*.rs\"");
    }

    #[test]
    fn test_format_tool_args_truncation() {
        let long_cmd = "a".repeat(100);
        let input = json!({"command": long_cmd});
        let result = format_tool_args("Bash", &input);
        assert!(result.len() < 100);
        assert!(result.ends_with("...\""));
    }

    #[test]
    fn test_ui_renderer_tracks_pending_calls() {
        let mut renderer = UiRenderer::new();

        let asst_msg = SdkAssistantMessage {
            message: json!({
                "content": [
                    {
                        "type": "tool_use",
                        "id": "call_123",
                        "name": "Bash",
                        "input": {"command": "ls -la"}
                    }
                ]
            }),
            parent_tool_use_id: None,
            error: None,
            uuid: "uuid-1".to_string(),
            session_id: "session-1".to_string(),
        };

        let html = renderer.render_assistant_message(&asst_msg);
        assert!(html.is_some());
        assert_eq!(renderer.pending_calls.len(), 1);
        assert!(renderer.pending_calls.contains_key("call_123"));
    }

    #[test]
    fn test_ui_renderer_matches_result_to_call() {
        let mut renderer = UiRenderer::new();

        // First, create a tool call
        let asst_msg = SdkAssistantMessage {
            message: json!({
                "content": [
                    {
                        "type": "tool_use",
                        "id": "call_456",
                        "name": "Read",
                        "input": {"file_path": "/test.rs"}
                    }
                ]
            }),
            parent_tool_use_id: None,
            error: None,
            uuid: "uuid-2".to_string(),
            session_id: "session-1".to_string(),
        };
        renderer.render_assistant_message(&asst_msg);

        // Then, send a tool result
        let user_msg = SdkUserMessage {
            message: json!({
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "call_456",
                        "is_error": false,
                        "content": "file contents"
                    }
                ]
            }),
            parent_tool_use_id: None,
            is_synthetic: None,
            tool_use_result: None,
            uuid: Some("uuid-3".to_string()),
            session_id: "session-1".to_string(),
            is_replay: None,
        };

        let (html, script) = renderer.render_user_message(&user_msg);
        assert!(html.is_none()); // Should return update script, not new HTML
        assert!(script.is_some()); // Should generate update script

        let update_script = script.unwrap();
        assert!(update_script.contains("call_456")); // Should reference tool_id
        assert!(update_script.contains("data-call-id")); // Should query by call ID
        assert!(update_script.contains("outerHTML")); // Should replace element

        // Pending call should be removed
        assert_eq!(renderer.pending_calls.len(), 0);
    }

    #[test]
    fn test_ui_renderer_handles_error_result() {
        let mut renderer = UiRenderer::new();

        // Create a tool call
        let asst_msg = SdkAssistantMessage {
            message: json!({
                "content": [
                    {
                        "type": "tool_use",
                        "id": "call_789",
                        "name": "Bash",
                        "input": {"command": "bad command"}
                    }
                ]
            }),
            parent_tool_use_id: None,
            error: None,
            uuid: "uuid-4".to_string(),
            session_id: "session-1".to_string(),
        };
        renderer.render_assistant_message(&asst_msg);

        // Send error result
        let user_msg = SdkUserMessage {
            message: json!({
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "call_789",
                        "is_error": true,
                        "content": "Command failed"
                    }
                ]
            }),
            parent_tool_use_id: None,
            is_synthetic: None,
            tool_use_result: None,
            uuid: Some("uuid-5".to_string()),
            session_id: "session-1".to_string(),
            is_replay: None,
        };

        let (html, script) = renderer.render_user_message(&user_msg);
        assert!(html.is_none());
        assert!(script.is_some());

        let update_script = script.unwrap();
        assert!(update_script.contains("call_789"));
        assert!(renderer.pending_calls.len() == 0);
    }

    #[test]
    fn test_ui_renderer_unknown_result_without_pending_call() {
        let mut renderer = UiRenderer::new();

        // Send a result without prior call
        let user_msg = SdkUserMessage {
            message: json!({
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "call_unknown",
                        "is_error": false,
                        "content": "result"
                    }
                ]
            }),
            parent_tool_use_id: None,
            is_synthetic: None,
            tool_use_result: None,
            uuid: Some("uuid-6".to_string()),
            session_id: "session-1".to_string(),
            is_replay: None,
        };

        let (html, script) = renderer.render_user_message(&user_msg);
        assert!(html.is_some()); // Should return standalone HTML
        assert!(script.is_none()); // No update script
    }

    #[test]
    fn test_ui_renderer_multiple_pending_calls() {
        let mut renderer = UiRenderer::new();

        // Create two tool calls
        let asst_msg1 = SdkAssistantMessage {
            message: json!({
                "content": [
                    {
                        "type": "tool_use",
                        "id": "call_a",
                        "name": "Read",
                        "input": {"file_path": "/a.rs"}
                    }
                ]
            }),
            parent_tool_use_id: None,
            error: None,
            uuid: "uuid-7".to_string(),
            session_id: "session-1".to_string(),
        };
        renderer.render_assistant_message(&asst_msg1);

        let asst_msg2 = SdkAssistantMessage {
            message: json!({
                "content": [
                    {
                        "type": "tool_use",
                        "id": "call_b",
                        "name": "Bash",
                        "input": {"command": "ls"}
                    }
                ]
            }),
            parent_tool_use_id: None,
            error: None,
            uuid: "uuid-8".to_string(),
            session_id: "session-1".to_string(),
        };
        renderer.render_assistant_message(&asst_msg2);

        assert_eq!(renderer.pending_calls.len(), 2);

        // Resolve first call
        let user_msg = SdkUserMessage {
            message: json!({
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "call_a",
                        "is_error": false,
                        "content": "content"
                    }
                ]
            }),
            parent_tool_use_id: None,
            is_synthetic: None,
            tool_use_result: None,
            uuid: Some("uuid-9".to_string()),
            session_id: "session-1".to_string(),
            is_replay: None,
        };

        let (_, script) = renderer.render_user_message(&user_msg);
        assert!(script.is_some());
        assert_eq!(renderer.pending_calls.len(), 1);
        assert!(renderer.pending_calls.contains_key("call_b"));
    }
}
