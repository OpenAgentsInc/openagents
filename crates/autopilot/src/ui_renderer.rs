//! UI renderer for SDK messages to recorder HTML components.
//!
//! Maps SdkMessage types from claude-agent-sdk to recorder UI components
//! from the ui crate, generating HTML strings that can be served via SSE.

use claude_agent_sdk::SdkMessage;
use maud::Markup;
use serde_json::Value;
use ui::recorder::organisms::{AgentLine, ToolLine, LifecycleEvent, lifecycle_line};
use ui::recorder::molecules::ResultType;

/// Render an SdkMessage to HTML using recorder components.
pub fn render_sdk_message(msg: &SdkMessage) -> Option<Markup> {
    match msg {
        SdkMessage::Assistant(asst) => render_assistant_message(asst),
        SdkMessage::User(user) => render_user_message(user),
        SdkMessage::ToolProgress(progress) => render_tool_progress(progress),
        SdkMessage::System(sys) => render_system_message(sys),
        SdkMessage::Result(result) => render_result_message(result),
        _ => None,
    }
}

/// Render assistant messages (text blocks and tool_use).
fn render_assistant_message(asst: &claude_agent_sdk::SdkAssistantMessage) -> Option<Markup> {
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

                return Some(
                    ToolLine::new(tool_name, &args, ResultType::Pending)
                        .call_id(tool_id)
                        .build()
                );
            }
            _ => continue,
        }
    }

    None
}

/// Render user messages (tool results).
fn render_user_message(user: &claude_agent_sdk::SdkUserMessage) -> Option<Markup> {
    let content = user.message.get("content")?;

    match content {
        Value::Array(arr) => {
            // Look for tool_result blocks
            for block in arr {
                if block.get("type")?.as_str()? == "tool_result" {
                    let tool_id = block.get("tool_use_id")?.as_str()?;
                    let is_error = block.get("is_error")?.as_bool().unwrap_or(false);

                    let result = if is_error {
                        let error_msg = extract_error_message(block);
                        ResultType::Error(error_msg)
                    } else {
                        ResultType::Ok
                    };

                    // Note: In practice, we'd need to update an existing ToolLine
                    // For now, we'll return a placeholder that shows the result
                    // This would need to be handled via SSE updates in the actual implementation
                    return Some(
                        ToolLine::new("(completed)", tool_id, result)
                            .build()
                    );
                }
            }
        }
        _ => {}
    }

    None
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
        "Bash" => {
            input
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
                .unwrap_or_default()
        }
        "Read" | "Write" | "Edit" => {
            input
                .get("file_path")
                .and_then(|p| p.as_str())
                .map(|p| format!("file_path={}", p))
                .unwrap_or_default()
        }
        "Glob" => {
            input
                .get("pattern")
                .and_then(|p| p.as_str())
                .map(|p| format!("pattern=\"{}\"", p))
                .unwrap_or_default()
        }
        "Grep" => {
            input
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
                .unwrap_or_default()
        }
        "Task" => {
            input
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
                .unwrap_or_default()
        }
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
    use serde_json::json;

    #[test]
    fn test_format_tool_args_bash() {
        let input = json!({"command": "ls -la"});
        assert_eq!(format_tool_args("Bash", &input), "cmd=\"ls -la\"");
    }

    #[test]
    fn test_format_tool_args_read() {
        let input = json!({"file_path": "/path/to/file.rs"});
        assert_eq!(format_tool_args("Read", &input), "file_path=/path/to/file.rs");
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
}
