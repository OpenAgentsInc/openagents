use agent_client_protocol::{
    ContentBlock, ContentChunk, Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus, SessionUpdate,
    TextContent, ToolCall, ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus, ToolKind,
};
use serde_json::Value as JsonValue;
use std::sync::Arc;

/// Translate a single Codex JSONL event (as serde_json::Value) into an ACP SessionUpdate.
/// Returns None when the event does not map to a streaming update (e.g. thread.started).
pub fn translate_codex_event_to_acp_update(v: &JsonValue) -> Option<SessionUpdate> {
    let ty = v.get("type").and_then(|s| s.as_str()).unwrap_or("");

    // Handle streaming text/reasoning deltas/completions explicitly
    if ty == "item.delta" || ty == "item.completed" {
        let item = v
            .get("item")
            .or_else(|| v.get("payload").and_then(|p| p.get("item")));
        if let Some(item) = item {
            let kind = item.get("type").and_then(|x| x.as_str()).unwrap_or("");
            let txt = item.get("text").and_then(|x| x.as_str()).unwrap_or("");
            if kind == "agent_message" {
                return Some(SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent {
                        annotations: None,
                        text: txt.to_string(),
                        meta: None,
                    }),
                    meta: None,
                }));
            } else if kind == "reasoning" {
                return Some(SessionUpdate::AgentThoughtChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent {
                        annotations: None,
                        text: txt.to_string(),
                        meta: None,
                    }),
                    meta: None,
                }));
            }
        }
    }

    // Codex thread item lifecycle (generic mapping for other item.* kinds)
    if ty.starts_with("item.") {
        {
            let item = match v.get("item") {
                Some(i) => i,
                None => return None,
            };
            let item_type = item.get("type").and_then(|x| x.as_str()).unwrap_or("");
            match item_type {
                // Completed agent message in older shape
                "agent_message" => {
                    let txt = item.get("text").and_then(|x| x.as_str()).unwrap_or("");
                    Some(SessionUpdate::AgentMessageChunk(ContentChunk {
                        content: ContentBlock::Text(TextContent {
                            annotations: None,
                            text: txt.to_string(),
                            meta: None,
                        }),
                        meta: None,
                    }))
                }
                // Completed reasoning in older shape
                "reasoning" => {
                    let txt = item.get("text").and_then(|x| x.as_str()).unwrap_or("");
                    Some(SessionUpdate::AgentThoughtChunk(ContentChunk {
                        content: ContentBlock::Text(TextContent {
                            annotations: None,
                            text: txt.to_string(),
                            meta: None,
                        }),
                        meta: None,
                    }))
                }
                // Command execution begin/update/end → ToolCall
                "command_execution" => {
                    let id = item
                        .get("id")
                        .and_then(|x| x.as_str())
                        .map(|s| ToolCallId(Arc::from(s)))
                        .unwrap_or_else(|| ToolCallId(Arc::from("call_command")));
                    let command = item
                        .get("command")
                        .and_then(|x| x.as_str())
                        .unwrap_or("(unknown)");
                    let status_str = item
                        .get("status")
                        .and_then(|x| x.as_str())
                        .unwrap_or("pending");
                    let status = match status_str {
                        "in_progress" => ToolCallStatus::InProgress,
                        "completed" => ToolCallStatus::Completed,
                        "failed" => ToolCallStatus::Failed,
                        _ => ToolCallStatus::Pending,
                    };
                    let mut content_vec: Vec<ToolCallContent> = Vec::new();
                    if let Some(out) = item
                        .get("aggregated_output")
                        .and_then(|x| x.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        content_vec.push(ToolCallContent::from(ContentBlock::Text(
                            TextContent {
                                annotations: None,
                                text: out.to_string(),
                                meta: None,
                            },
                        )));
                    }
                    let call = ToolCall {
                        id,
                        title: format!("Run: {}", command),
                        kind: ToolKind::Execute,
                        status,
                        content: content_vec,
                        locations: vec![],
                        raw_input: None,
                        raw_output: None,
                        meta: None,
                    };
                    Some(SessionUpdate::ToolCall(call))
                }
                // File changes → summarize as an Edit tool call
                "file_change" => {
                    let id = item
                        .get("id")
                        .and_then(|x| x.as_str())
                        .map(|s| ToolCallId(Arc::from(s)))
                        .unwrap_or_else(|| ToolCallId(Arc::from("call_file_change")));
                    let status_str = item
                        .get("status")
                        .and_then(|x| x.as_str())
                        .unwrap_or("completed");
                    let status = match status_str {
                        "failed" => ToolCallStatus::Failed,
                        "in_progress" => ToolCallStatus::InProgress,
                        _ => ToolCallStatus::Completed,
                    };
                    let mut locations = Vec::new();
                    if let Some(changes) = item.get("changes").and_then(|x| x.as_array()) {
                        for ch in changes {
                            if let Some(path) = ch.get("path").and_then(|x| x.as_str()) {
                                locations.push(ToolCallLocation {
                                    path: path.into(),
                                    line: None,
                                    meta: None,
                                });
                            }
                        }
                    }
                    let call = ToolCall {
                        id,
                        title: "Apply file changes".to_string(),
                        kind: ToolKind::Edit,
                        status,
                        content: vec![],
                        locations,
                        raw_input: None,
                        raw_output: None,
                        meta: None,
                    };
                    Some(SessionUpdate::ToolCall(call))
                }
                // MCP tool call → ToolCall with Fetch/Other
                "mcp_tool_call" => {
                    let id = item
                        .get("id")
                        .and_then(|x| x.as_str())
                        .map(|s| ToolCallId(Arc::from(s)))
                        .unwrap_or_else(|| ToolCallId(Arc::from("call_mcp")));
                    let server = item.get("server").and_then(|x| x.as_str()).unwrap_or("");
                    let tool = item.get("tool").and_then(|x| x.as_str()).unwrap_or("");
                    let title = if server.is_empty() && tool.is_empty() {
                        "MCP tool call".to_string()
                    } else {
                        format!("MCP: {}.{}", server, tool)
                    };
                    let status_str = item
                        .get("status")
                        .and_then(|x| x.as_str())
                        .unwrap_or("pending");
                    let status = match status_str {
                        "in_progress" => ToolCallStatus::InProgress,
                        "completed" => ToolCallStatus::Completed,
                        "failed" => ToolCallStatus::Failed,
                        _ => ToolCallStatus::Pending,
                    };
                    let call = ToolCall {
                        id,
                        title,
                        kind: ToolKind::Fetch,
                        status,
                        content: vec![],
                        locations: vec![],
                        raw_input: None,
                        raw_output: None,
                        meta: None,
                    };
                    Some(SessionUpdate::ToolCall(call))
                }
                // Web search → ToolCall Search (completed)
                "web_search" => {
                    let id = item
                        .get("id")
                        .and_then(|x| x.as_str())
                        .map(|s| ToolCallId(Arc::from(s)))
                        .unwrap_or_else(|| ToolCallId(Arc::from("call_web_search")));
                    let query = item.get("query").and_then(|x| x.as_str()).unwrap_or("");
                    let call = ToolCall {
                        id,
                        title: if query.is_empty() {
                            "Web search".into()
                        } else {
                            format!("Web search: {}", query)
                        },
                        kind: ToolKind::Search,
                        status: ToolCallStatus::Completed,
                        content: vec![],
                        locations: vec![],
                        raw_input: None,
                        raw_output: None,
                        meta: None,
                    };
                    Some(SessionUpdate::ToolCall(call))
                }
                // Todo list → Plan
                "todo_list" => {
                    if let Some(items) = item.get("items").and_then(|x| x.as_array()) {
                        let mut entries = Vec::new();
                        for it in items {
                            let text = it.get("text").and_then(|x| x.as_str()).unwrap_or("");
                            let completed = it.get("completed").and_then(|x| x.as_bool()).unwrap_or(false);
                            let status = if completed {
                                PlanEntryStatus::Completed
                            } else {
                                PlanEntryStatus::Pending
                            };
                            entries.push(PlanEntry {
                                content: text.to_string(),
                                priority: PlanEntryPriority::Medium,
                                status,
                                meta: None,
                            });
                        }
                        return Some(SessionUpdate::Plan(Plan { entries, meta: None }));
                    }
                    None
                }
                _ => None,
            }
        }
    } else {
        None
    }
}

/// Stub: Translate a Claude Code streaming event to ACP SessionUpdate
pub fn translate_claude_event_to_acp_update(_v: &JsonValue) -> Option<SessionUpdate> {
    // TODO: implement once Claude Code event schema is finalized in this repo
    None
}

/// Stub: Translate an OpenCode SSE event to ACP SessionUpdate
pub fn translate_opencode_event_to_acp_update(_v: &JsonValue) -> Option<SessionUpdate> {
    // TODO: implement once OpenCode event schema is finalized in this repo
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn codex_maps_agent_message_completed() {
        let v = json!({
            "type": "item.completed",
            "item": {"id": "item_1", "type": "agent_message", "text": "Hello"}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::AgentMessageChunk(chunk) => match chunk.content {
                ContentBlock::Text(t) => assert_eq!(t.text, "Hello"),
                _ => panic!("expected text"),
            },
            _ => panic!("expected AgentMessageChunk"),
        }
    }

    #[test]
    fn codex_maps_command_execution_started() {
        let v = json!({
            "type": "item.started",
            "item": {"id":"item_0","type":"command_execution","command":"echo hi","aggregated_output":"","status":"in_progress"}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::ToolCall(call) => {
                assert_eq!(call.kind, ToolKind::Execute);
                assert_eq!(call.status, ToolCallStatus::InProgress);
                assert!(call.title.contains("echo hi"));
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn codex_maps_file_change_completed() {
        let v = json!({
            "type": "item.completed",
            "item": {"id":"item_2","type":"file_change","status":"completed","changes":[{"path":"src/main.rs","kind":"update"}]}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::ToolCall(call) => {
                assert_eq!(call.kind, ToolKind::Edit);
                assert_eq!(call.status, ToolCallStatus::Completed);
                assert_eq!(call.locations.len(), 1);
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn codex_maps_agent_message_delta() {
        let v = json!({
            "type": "item.delta",
            "item": {"id": "item_x", "type": "agent_message", "text": "Partial"}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::AgentMessageChunk(chunk) => match chunk.content {
                ContentBlock::Text(t) => assert_eq!(t.text, "Partial"),
                _ => panic!("expected text"),
            },
            _ => panic!("expected AgentMessageChunk"),
        }
    }

    #[test]
    fn codex_maps_reasoning_delta() {
        let v = json!({
            "type": "item.delta",
            "item": {"id": "item_r", "type": "reasoning", "text": "Thinking..."}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::AgentThoughtChunk(chunk) => match chunk.content {
                ContentBlock::Text(t) => assert_eq!(t.text, "Thinking..."),
                _ => panic!("expected text"),
            },
            _ => panic!("expected AgentThoughtChunk"),
        }
    }

    #[test]
    fn codex_maps_mcp_tool_call_completed() {
        let v = json!({
            "type": "item.completed",
            "item": {"id":"mcp_1","type":"mcp_tool_call","server":"fs","tool":"read","status":"completed"}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::ToolCall(call) => {
                assert_eq!(call.kind, ToolKind::Fetch);
                assert_eq!(call.status, ToolCallStatus::Completed);
                assert!(call.title.contains("fs.read") || call.title.contains("MCP"));
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn codex_maps_web_search_completed() {
        let v = json!({
            "type": "item.completed",
            "item": {"id":"search_1","type":"web_search","query":"rust unit tests"}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::ToolCall(call) => {
                assert_eq!(call.kind, ToolKind::Search);
                assert_eq!(call.status, ToolCallStatus::Completed);
                assert!(call.title.contains("rust unit tests") || call.title.contains("Web search"));
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn codex_maps_todo_list_to_plan() {
        let v = json!({
            "type": "item.completed",
            "item": {"id":"todo_1","type":"todo_list","items":[{"text":"A","completed":false},{"text":"B","completed":true}]}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::Plan(plan) => {
                assert_eq!(plan.entries.len(), 2);
                assert_eq!(plan.entries[0].content, "A");
                assert_eq!(plan.entries[1].status, PlanEntryStatus::Completed);
            }
            _ => panic!("expected Plan"),
        }
    }

    #[test]
    fn codex_maps_command_execution_with_output() {
        let v = json!({
            "type": "item.completed",
            "item": {"id":"exec_1","type":"command_execution","command":"echo hi","aggregated_output":"hi\n","status":"completed"}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::ToolCall(call) => {
                assert_eq!(call.kind, ToolKind::Execute);
                assert_eq!(call.status, ToolCallStatus::Completed);
                assert!(!call.content.is_empty(), "expected aggregated output content");
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn codex_ignores_unknown_events() {
        let v = json!({"type":"turn.started"});
        assert!(translate_codex_event_to_acp_update(&v).is_none());
    }
}
