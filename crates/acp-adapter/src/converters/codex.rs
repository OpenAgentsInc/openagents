//! Convert Codex ThreadEvent to ACP session notifications
//!
//! Transforms Codex SDK events into ACP `SessionNotification` for unified
//! protocol handling across different agent types.

use agent_client_protocol_schema as acp;
use codex_agent_sdk::events::{
    ItemCompletedEvent, ItemStartedEvent, ItemUpdatedEvent, ThreadEvent, TurnCompletedEvent,
};
use codex_agent_sdk::items::{
    CommandExecutionItem, CommandExecutionStatus, FileChangeItem, McpToolCallItem,
    McpToolCallStatus, PatchApplyStatus, ThreadItem, ThreadItemDetails,
};

/// Convert a Codex ThreadEvent to ACP session notifications
///
/// Returns a vector because some events may produce multiple notifications
/// (e.g., a completed item may include both the result and status update).
pub fn thread_event_to_notifications(
    session_id: &acp::SessionId,
    event: &ThreadEvent,
) -> Vec<acp::SessionNotification> {
    match event {
        ThreadEvent::ThreadStarted(_) => {
            // Thread started - could emit a lifecycle event
            // For now, we don't emit anything as the session is already created
            vec![]
        }

        ThreadEvent::TurnStarted(_) => {
            // Turn started - the agent is processing
            // Could emit a mode update or status, but not strictly necessary
            vec![]
        }

        ThreadEvent::TurnCompleted(turn) => {
            // Turn completed with usage stats
            // We could emit this as metadata, but ACP doesn't have a direct equivalent
            convert_turn_completed(session_id, turn)
        }

        ThreadEvent::TurnFailed(failed) => {
            // Turn failed - emit an error message
            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                    acp::ContentBlock::Text(acp::TextContent::new(format!(
                        "Error: {}",
                        failed.error.message
                    ))),
                )),
            )]
        }

        ThreadEvent::ItemStarted(item) => convert_item_started(session_id, item),

        ThreadEvent::ItemUpdated(item) => convert_item_updated(session_id, item),

        ThreadEvent::ItemCompleted(item) => convert_item_completed(session_id, item),

        ThreadEvent::Error(error) => {
            // Fatal error
            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                    acp::ContentBlock::Text(acp::TextContent::new(format!(
                        "Fatal error: {}",
                        error.message
                    ))),
                )),
            )]
        }
    }
}

/// Convert turn completed event (for usage tracking)
fn convert_turn_completed(
    _session_id: &acp::SessionId,
    _turn: &TurnCompletedEvent,
) -> Vec<acp::SessionNotification> {
    // Usage stats could be tracked but ACP doesn't have a direct equivalent
    // In the future, we could emit this as custom metadata
    vec![]
}

/// Convert item started event
fn convert_item_started(
    session_id: &acp::SessionId,
    event: &ItemStartedEvent,
) -> Vec<acp::SessionNotification> {
    convert_thread_item(session_id, &event.item, ItemPhase::Started)
}

/// Convert item updated event
fn convert_item_updated(
    session_id: &acp::SessionId,
    event: &ItemUpdatedEvent,
) -> Vec<acp::SessionNotification> {
    convert_thread_item(session_id, &event.item, ItemPhase::Updated)
}

/// Convert item completed event
fn convert_item_completed(
    session_id: &acp::SessionId,
    event: &ItemCompletedEvent,
) -> Vec<acp::SessionNotification> {
    convert_thread_item(session_id, &event.item, ItemPhase::Completed)
}

/// Phase of an item in its lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemPhase {
    Started,
    Updated,
    Completed,
}

/// Convert a thread item to ACP notifications
fn convert_thread_item(
    session_id: &acp::SessionId,
    item: &ThreadItem,
    phase: ItemPhase,
) -> Vec<acp::SessionNotification> {
    match &item.details {
        ThreadItemDetails::AgentMessage(msg) => {
            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                    acp::ContentBlock::Text(acp::TextContent::new(msg.text.clone())),
                )),
            )]
        }

        ThreadItemDetails::Reasoning(reasoning) => {
            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
                    acp::ContentBlock::Text(acp::TextContent::new(reasoning.text.clone())),
                )),
            )]
        }

        ThreadItemDetails::CommandExecution(cmd) => {
            convert_command_execution(session_id, &item.id, cmd, phase)
        }

        ThreadItemDetails::FileChange(fc) => {
            convert_file_change(session_id, &item.id, fc, phase)
        }

        ThreadItemDetails::McpToolCall(mcp) => {
            convert_mcp_tool_call(session_id, &item.id, mcp, phase)
        }

        ThreadItemDetails::WebSearch(search) => {
            // Web search as a tool call
            let status = match phase {
                ItemPhase::Started | ItemPhase::Updated => acp::ToolCallStatus::InProgress,
                ItemPhase::Completed => acp::ToolCallStatus::Completed,
            };

            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::ToolCall(
                    acp::ToolCall::new(
                        acp::ToolCallId::new(item.id.clone()),
                        "WebSearch".to_string(),
                    )
                    .raw_input(serde_json::json!({ "query": search.query }))
                    .status(status),
                ),
            )]
        }

        ThreadItemDetails::TodoList(todos) => {
            convert_todo_list(session_id, todos)
        }

        ThreadItemDetails::Error(error) => {
            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                    acp::ContentBlock::Text(acp::TextContent::new(format!(
                        "Error: {}",
                        error.message
                    ))),
                )),
            )]
        }
    }
}

/// Convert command execution to ACP tool call
fn convert_command_execution(
    session_id: &acp::SessionId,
    item_id: &str,
    cmd: &CommandExecutionItem,
    phase: ItemPhase,
) -> Vec<acp::SessionNotification> {
    let tool_call_id = acp::ToolCallId::new(item_id.to_string());

    match phase {
        ItemPhase::Started => {
            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::ToolCall(
                    acp::ToolCall::new(tool_call_id, "Bash".to_string())
                        .raw_input(serde_json::json!({ "command": cmd.command }))
                        .status(acp::ToolCallStatus::InProgress),
                ),
            )]
        }
        ItemPhase::Updated | ItemPhase::Completed => {
            let status = match cmd.status {
                CommandExecutionStatus::InProgress => acp::ToolCallStatus::InProgress,
                CommandExecutionStatus::Completed => acp::ToolCallStatus::Completed,
                CommandExecutionStatus::Failed | CommandExecutionStatus::Declined => {
                    acp::ToolCallStatus::Failed
                }
            };

            let mut fields = acp::ToolCallUpdateFields::default();
            fields.status = Some(status);

            if phase == ItemPhase::Completed || !cmd.aggregated_output.is_empty() {
                fields.raw_output = Some(serde_json::json!({
                    "output": cmd.aggregated_output,
                    "exit_code": cmd.exit_code
                }));
            }

            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(tool_call_id, fields)),
            )]
        }
    }
}

/// Convert file change to ACP tool call
fn convert_file_change(
    session_id: &acp::SessionId,
    item_id: &str,
    fc: &FileChangeItem,
    phase: ItemPhase,
) -> Vec<acp::SessionNotification> {
    let tool_call_id = acp::ToolCallId::new(item_id.to_string());

    // Build description of changes
    let changes_desc: Vec<String> = fc
        .changes
        .iter()
        .map(|c| format!("{:?}: {}", c.kind, c.path))
        .collect();

    match phase {
        ItemPhase::Started => {
            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::ToolCall(
                    acp::ToolCall::new(tool_call_id, "FileChange".to_string())
                        .raw_input(serde_json::json!({ "changes": changes_desc }))
                        .status(acp::ToolCallStatus::InProgress),
                ),
            )]
        }
        ItemPhase::Updated | ItemPhase::Completed => {
            let status = match fc.status {
                PatchApplyStatus::InProgress => acp::ToolCallStatus::InProgress,
                PatchApplyStatus::Completed => acp::ToolCallStatus::Completed,
                PatchApplyStatus::Failed => acp::ToolCallStatus::Failed,
            };

            let mut fields = acp::ToolCallUpdateFields::default();
            fields.status = Some(status);
            fields.raw_output = Some(serde_json::json!({
                "changes": changes_desc,
                "status": format!("{:?}", fc.status)
            }));

            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(tool_call_id, fields)),
            )]
        }
    }
}

/// Convert MCP tool call to ACP tool call
fn convert_mcp_tool_call(
    session_id: &acp::SessionId,
    item_id: &str,
    mcp: &McpToolCallItem,
    phase: ItemPhase,
) -> Vec<acp::SessionNotification> {
    let tool_call_id = acp::ToolCallId::new(item_id.to_string());
    let tool_name = format!("{}:{}", mcp.server, mcp.tool);

    match phase {
        ItemPhase::Started => {
            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::ToolCall(
                    acp::ToolCall::new(tool_call_id, tool_name)
                        .raw_input(mcp.arguments.clone())
                        .status(acp::ToolCallStatus::InProgress),
                ),
            )]
        }
        ItemPhase::Updated | ItemPhase::Completed => {
            let status = match mcp.status {
                McpToolCallStatus::InProgress => acp::ToolCallStatus::InProgress,
                McpToolCallStatus::Completed => acp::ToolCallStatus::Completed,
                McpToolCallStatus::Failed => acp::ToolCallStatus::Failed,
            };

            let mut fields = acp::ToolCallUpdateFields::default();
            fields.status = Some(status);

            // Include result or error
            if let Some(ref result) = mcp.result {
                fields.raw_output = Some(serde_json::json!({
                    "content": result.content,
                    "structured_content": result.structured_content
                }));
            } else if let Some(ref error) = mcp.error {
                fields.raw_output = Some(serde_json::json!({
                    "error": error.message
                }));
            }

            vec![acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(tool_call_id, fields)),
            )]
        }
    }
}

/// Convert todo list to ACP plan
fn convert_todo_list(
    session_id: &acp::SessionId,
    todos: &codex_agent_sdk::items::TodoListItem,
) -> Vec<acp::SessionNotification> {
    let entries: Vec<acp::PlanEntry> = todos
        .items
        .iter()
        .map(|item| {
            let status = if item.completed {
                acp::PlanEntryStatus::Completed
            } else {
                acp::PlanEntryStatus::Pending
            };
            acp::PlanEntry::new(item.text.clone(), acp::PlanEntryPriority::Medium, status)
        })
        .collect();

    vec![acp::SessionNotification::new(
        session_id.clone(),
        acp::SessionUpdate::Plan(acp::Plan::new(entries)),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_agent_sdk::events::ThreadStartedEvent;
    use codex_agent_sdk::items::{AgentMessageItem, ReasoningItem, TodoItem};

    #[test]
    fn test_convert_agent_message() {
        let session_id = acp::SessionId::new("test");
        let event = ThreadEvent::ItemCompleted(ItemCompletedEvent {
            item: ThreadItem {
                id: "item_0".to_string(),
                details: ThreadItemDetails::AgentMessage(AgentMessageItem {
                    text: "Hello, world!".to_string(),
                }),
            },
        });

        let notifications = thread_event_to_notifications(&session_id, &event);
        assert_eq!(notifications.len(), 1);

        match &notifications[0].update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                if let acp::ContentBlock::Text(text) = &chunk.content {
                    assert_eq!(text.text, "Hello, world!");
                } else {
                    panic!("Expected text content");
                }
            }
            _ => panic!("Expected AgentMessageChunk"),
        }
    }

    #[test]
    fn test_convert_reasoning() {
        let session_id = acp::SessionId::new("test");
        let event = ThreadEvent::ItemStarted(ItemStartedEvent {
            item: ThreadItem {
                id: "item_1".to_string(),
                details: ThreadItemDetails::Reasoning(ReasoningItem {
                    text: "Let me think about this...".to_string(),
                }),
            },
        });

        let notifications = thread_event_to_notifications(&session_id, &event);
        assert_eq!(notifications.len(), 1);

        match &notifications[0].update {
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                if let acp::ContentBlock::Text(text) = &chunk.content {
                    assert_eq!(text.text, "Let me think about this...");
                } else {
                    panic!("Expected text content");
                }
            }
            _ => panic!("Expected AgentThoughtChunk"),
        }
    }

    #[test]
    fn test_convert_command_execution_started() {
        let session_id = acp::SessionId::new("test");
        let event = ThreadEvent::ItemStarted(ItemStartedEvent {
            item: ThreadItem {
                id: "item_2".to_string(),
                details: ThreadItemDetails::CommandExecution(CommandExecutionItem {
                    command: "ls -la".to_string(),
                    aggregated_output: String::new(),
                    exit_code: None,
                    status: CommandExecutionStatus::InProgress,
                }),
            },
        });

        let notifications = thread_event_to_notifications(&session_id, &event);
        assert_eq!(notifications.len(), 1);

        match &notifications[0].update {
            acp::SessionUpdate::ToolCall(tool) => {
                assert_eq!(tool.title, "Bash");
                assert_eq!(tool.status, acp::ToolCallStatus::InProgress);
            }
            _ => panic!("Expected ToolCall"),
        }
    }

    #[test]
    fn test_convert_command_execution_completed() {
        let session_id = acp::SessionId::new("test");
        let event = ThreadEvent::ItemCompleted(ItemCompletedEvent {
            item: ThreadItem {
                id: "item_2".to_string(),
                details: ThreadItemDetails::CommandExecution(CommandExecutionItem {
                    command: "ls -la".to_string(),
                    aggregated_output: "file1.txt\nfile2.txt".to_string(),
                    exit_code: Some(0),
                    status: CommandExecutionStatus::Completed,
                }),
            },
        });

        let notifications = thread_event_to_notifications(&session_id, &event);
        assert_eq!(notifications.len(), 1);

        match &notifications[0].update {
            acp::SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.fields.status, Some(acp::ToolCallStatus::Completed));
                assert!(update.fields.raw_output.is_some());
            }
            _ => panic!("Expected ToolCallUpdate"),
        }
    }

    #[test]
    fn test_convert_todo_list() {
        let session_id = acp::SessionId::new("test");
        let event = ThreadEvent::ItemCompleted(ItemCompletedEvent {
            item: ThreadItem {
                id: "item_3".to_string(),
                details: ThreadItemDetails::TodoList(
                    codex_agent_sdk::items::TodoListItem {
                        items: vec![
                            TodoItem {
                                text: "Fix bug".to_string(),
                                completed: true,
                            },
                            TodoItem {
                                text: "Add tests".to_string(),
                                completed: false,
                            },
                        ],
                    },
                ),
            },
        });

        let notifications = thread_event_to_notifications(&session_id, &event);
        assert_eq!(notifications.len(), 1);

        match &notifications[0].update {
            acp::SessionUpdate::Plan(plan) => {
                assert_eq!(plan.entries.len(), 2);
                assert_eq!(plan.entries[0].content, "Fix bug");
                assert_eq!(plan.entries[0].status, acp::PlanEntryStatus::Completed);
                assert_eq!(plan.entries[1].content, "Add tests");
                assert_eq!(plan.entries[1].status, acp::PlanEntryStatus::Pending);
            }
            _ => panic!("Expected Plan"),
        }
    }

    #[test]
    fn test_thread_started_no_notification() {
        let session_id = acp::SessionId::new("test");
        let event = ThreadEvent::ThreadStarted(ThreadStartedEvent {
            thread_id: "thread_123".to_string(),
        });

        let notifications = thread_event_to_notifications(&session_id, &event);
        assert!(notifications.is_empty());
    }
}
