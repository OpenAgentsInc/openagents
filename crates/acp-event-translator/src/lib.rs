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

    // New-format Codex events (response_item, event_msg)
    if ty == "response_item" {
        if let Some(payload) = v.get("payload") {
            let pty = payload.get("type").and_then(|x| x.as_str()).unwrap_or("");
            if pty == "message" {
                // Aggregate text parts from content[]
                let mut txt = String::new();
                if let Some(arr) = payload.get("content").and_then(|x| x.as_array()) {
                    for part in arr {
                        if let Some(t) = part.get("text").and_then(|x| x.as_str()) {
                            if !txt.is_empty() { txt.push('\n'); }
                            txt.push_str(t);
                        }
                    }
                }
                if !txt.trim().is_empty() {
                    return Some(SessionUpdate::AgentMessageChunk(ContentChunk {
                        content: ContentBlock::Text(TextContent { annotations: None, text: txt, meta: None }),
                        meta: None,
                    }));
                }
            } else if pty == "reasoning" {
                // Prefer summary[] entries with type==summary_text
                let mut txt = String::new();
                if let Some(summary) = payload.get("summary").and_then(|x| x.as_array()) {
                    for s in summary {
                        if s.get("type").and_then(|x| x.as_str()) == Some("summary_text") {
                            if let Some(t) = s.get("text").and_then(|x| x.as_str()) {
                                if !txt.is_empty() { txt.push('\n'); }
                                txt.push_str(t);
                            }
                        }
                    }
                }
                if txt.trim().is_empty() {
                    if let Some(t) = payload.get("text").and_then(|x| x.as_str()) { txt = t.to_string(); }
                }
                if !txt.trim().is_empty() {
                    return Some(SessionUpdate::AgentThoughtChunk(ContentChunk {
                        content: ContentBlock::Text(TextContent { annotations: None, text: txt, meta: None }),
                        meta: None,
                    }));
                }
            }
        }
    }

    if ty == "event_msg" {
        if let Some(p) = v.get("payload") {
            if p.get("type").and_then(|x| x.as_str()) == Some("agent_reasoning") {
                if let Some(text) = p.get("text").and_then(|x| x.as_str()) {
                    return Some(SessionUpdate::AgentThoughtChunk(ContentChunk {
                        content: ContentBlock::Text(TextContent { annotations: None, text: text.to_string(), meta: None }),
                        meta: None,
                    }));
                }
            }
        }
    }

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

#[cfg(test)]
mod tests_codex_legacy {
    use super::*;
    use serde_json::json;

    #[test]
    fn response_item_message_maps_to_agent_message_chunk() {
        let v = json!({
            "type": "response_item",
            "payload": { "type": "message", "content": [ {"type": "text", "text": "Hello"}, {"type":"text","text":" world"} ] }
        });
        let out = translate_codex_event_to_acp_update(&v);
        match out { Some(SessionUpdate::AgentMessageChunk(ch)) => {
            match ch.content { ContentBlock::Text(TextContent{ text, .. }) => assert_eq!(text, "Hello\n world"), _ => panic!("wrong content kind") }
        } _ => panic!("expected AgentMessageChunk") }
    }

    #[test]
    fn response_item_reasoning_uses_summary_text() {
        let v = json!({
            "type": "response_item",
            "payload": { "type": "reasoning", "summary": [ {"type":"summary_text","text":"Summary one"}, {"type":"other","text":"ignored"} ] }
        });
        let out = translate_codex_event_to_acp_update(&v);
        match out { Some(SessionUpdate::AgentThoughtChunk(ch)) => {
            match ch.content { ContentBlock::Text(TextContent{ text, .. }) => assert!(text.contains("Summary one")), _ => panic!("wrong content kind") }
        } _ => panic!("expected AgentThoughtChunk") }
    }

    #[test]
    fn event_msg_agent_reasoning_maps_to_thought() {
        let v = json!({
            "type": "event_msg",
            "payload": { "type": "agent_reasoning", "text": "Thinking..." }
        });
        let out = translate_codex_event_to_acp_update(&v);
        match out { Some(SessionUpdate::AgentThoughtChunk(ch)) => {
            match ch.content { ContentBlock::Text(TextContent{ text, .. }) => assert_eq!(text, "Thinking..."), _ => panic!("wrong content kind") }
        } _ => panic!("expected AgentThoughtChunk") }
    }
}

/// Stub: Translate a Claude Code streaming event to ACP SessionUpdate
pub fn translate_claude_event_to_acp_update(v: &JsonValue) -> Option<SessionUpdate> {
    // Claude Code streams Anthropic-like events. We support a minimal subset:
    // - content_block_start/content_block_delta: text, image, thinking
    // - tool_use/server_tool_use/mcp_tool_use: create a pending ToolCall or Plan (TodoWrite)
    // - tool_result/*: mark a ToolCallUpdate (completed/failed)

    let ty = v.get("type").and_then(|s| s.as_str()).unwrap_or("");

    // Assistant message with aggregated content
    if ty == "assistant" {
        if let Some(msg) = v.get("message") {
            if let Some(contents) = msg.get("content").and_then(|c| c.as_array()) {
                let mut text = String::new();
                for c in contents {
                    let ctype = c.get("type").and_then(|x| x.as_str()).unwrap_or("");
                    if ctype == "text" {
                        if let Some(t) = c.get("text").and_then(|x| x.as_str()) {
                            if !text.is_empty() { text.push_str("\n"); }
                            text.push_str(t);
                        }
                    }
                }
                if !text.is_empty() {
                    return Some(SessionUpdate::AgentMessageChunk(ContentChunk {
                        content: ContentBlock::Text(TextContent { annotations: None, text, meta: None }),
                        meta: None,
                    }));
                }
            }
        }
    }
    // Helpers: extract chunk for content_block_* events
    let chunk = if ty == "content_block_start" {
        v.get("content_block")
    } else if ty == "content_block_delta" {
        v.get("delta")
    } else {
        None
    };

    if let Some(ch) = chunk {
        let ctype = ch.get("type").and_then(|x| x.as_str()).unwrap_or("");
        match ctype {
            // Text or text delta → assistant message chunk
            "text" | "text_delta" => {
                let text = ch
                    .get("text")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                return Some(SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent {
                        annotations: None,
                        text,
                        meta: None,
                    }),
                    meta: None,
                }));
            }
            // Image → agent message chunk image
            "image" => {
                // Anthropic SDK provides a source { type: "base64"|"url", data/media_type or url }
                let (data, mime, uri) = match ch.get("source") {
                    Some(src) => {
                        let st = src.get("type").and_then(|x| x.as_str()).unwrap_or("");
                        if st == "base64" {
                            let data = src.get("data").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            let mime = src
                                .get("media_type")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            (data, mime, None)
                        } else if st == "url" {
                            let url = src.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            (String::new(), String::new(), Some(url))
                        } else {
                            (String::new(), String::new(), None)
                        }
                    }
                    None => (String::new(), String::new(), None),
                };
                return Some(SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Image(agent_client_protocol::ImageContent {
                        annotations: None,
                        data,
                        mime_type: mime,
                        uri,
                        meta: None,
                    }),
                    meta: None,
                }));
            }
            // Thinking → agent thought chunk (text)
            "thinking" | "thinking_delta" => {
                let text = ch
                    .get("thinking")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                return Some(SessionUpdate::AgentThoughtChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent {
                        annotations: None,
                        text,
                        meta: None,
                    }),
                    meta: None,
                }));
            }
            // Tool use(s)
            "tool_use" | "server_tool_use" | "mcp_tool_use" => {
                let id = ch
                    .get("id")
                    .and_then(|x| x.as_str())
                    .map(|s| ToolCallId(Arc::from(s)))
                    .unwrap_or_else(|| ToolCallId(Arc::from("tool_use")));
                let name = ch.get("name").and_then(|x| x.as_str()).unwrap_or("");
                // TodoWrite with todos → Plan (per upstream adapter)
                if name == "TodoWrite" {
                    if let Some(todos) = ch.get("input").and_then(|i| i.get("todos")).and_then(|x| x.as_array()) {
                        let mut entries = Vec::new();
                        for t in todos {
                            let content = t
                                .get("content")
                                .or_else(|| t.get("text"))
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            // Claude todos don’t include status; default pending
                            entries.push(PlanEntry { content, priority: PlanEntryPriority::Medium, status: PlanEntryStatus::Pending, meta: None });
                        }
                        return Some(SessionUpdate::Plan(Plan { entries, meta: None }));
                    }
                }
                // Otherwise build ToolCall using upstream semantics
                if let Some(call) = claude_tool_call_from_tool_use(ch) {
                    return Some(SessionUpdate::ToolCall(ToolCall { id, ..call }));
                }
                // Fallback minimal call
                let call = ToolCall {
                    id,
                    title: if name.is_empty() { "Tool Call".to_string() } else { format!("Tool: {}", name) },
                    kind: ToolKind::Other,
                    status: ToolCallStatus::Pending,
                    content: vec![],
                    locations: vec![],
                    raw_input: ch.get("input").cloned(),
                    raw_output: None,
                    meta: None,
                };
                return Some(SessionUpdate::ToolCall(call));
            }
            _ => {}
        }
    }

    // Tool results → ToolCallUpdate (map status and, when possible, structured content)
    if ty == "tool_result"
        || ty == "web_fetch_tool_result"
        || ty == "web_search_tool_result"
        || ty == "code_execution_tool_result"
        || ty == "bash_code_execution_tool_result"
        || ty == "text_editor_code_execution_tool_result"
        || ty == "mcp_tool_result"
    {
        let tool_id = v
            .get("tool_use_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if tool_id.is_empty() {
            return None;
        }
        let status = if v.get("is_error").and_then(|x| x.as_bool()).unwrap_or(false) {
            ToolCallStatus::Failed
        } else {
            ToolCallStatus::Completed
        };
        let mut fields = agent_client_protocol::ToolCallUpdateFields::default();
        fields.status = Some(status);
        if let Some(res) = v.get("result") {
            // Surface raw output for diagnostics
            fields.raw_output = Some(res.clone());
            // Best-effort content mapping for common Claude outputs
            // 1) Terminal/stdout text → inline text content
            if let Some(stdout) = res.get("stdout").and_then(|x| x.as_str()) {
                if !stdout.is_empty() {
                    let content = vec![ToolCallContent::from(ContentBlock::Text(TextContent {
                        annotations: None,
                        text: stdout.to_string(),
                        meta: None,
                    }))];
                    fields.content = Some(content);
                }
            }
            // 2) Diff-like structures (path/old/new or oldText/newText) → prefer structured diff when available
            // Note: Field names vary across tools; handle common cases conservatively.
            if let (Some(path), Some(old_text), Some(new_text)) = (
                res.get("path").and_then(|x| x.as_str()),
                res.get("oldText").or_else(|| res.get("old")).and_then(|x| x.as_str()),
                res.get("newText").or_else(|| res.get("new")).and_then(|x| x.as_str()),
            ) {
                // Append alongside stdout content if both exist
                let mut content_vec = fields.content.take().unwrap_or_default();
                content_vec.push(ToolCallContent::Diff {
                    diff: agent_client_protocol::Diff {
                        path: path.into(),
                        old_text: Some(old_text.to_string()),
                        new_text: new_text.to_string(),
                        meta: None,
                    },
                });
                fields.content = Some(content_vec);
            }
        }
        return Some(SessionUpdate::ToolCallUpdate(agent_client_protocol::ToolCallUpdate {
            id: ToolCallId(Arc::from(tool_id)),
            fields,
            meta: None,
        }));
    }

    None
}

/// Claude-specific helper: produce ToolCallUpdateFields.content/title from a tool_result
/// using the same logic as zed-industries/claude-code-acp's tools.ts.
///
/// This function intentionally does not guess beyond the upstream mapping; it mirrors
/// `toolUpdateFromToolResult` and `toAcpContentUpdate` behaviors.
pub fn claude_tool_update_from_tool_result(
    tool_result: &JsonValue,
    tool_use: Option<&JsonValue>,
) -> agent_client_protocol::ToolCallUpdateFields {
    let mut fields = agent_client_protocol::ToolCallUpdateFields::default();
    // Helper: wrap text in fenced code block if is_error
    fn to_acp_content_update(
        content: &JsonValue,
        is_error: bool,
    ) -> Option<Vec<ToolCallContent>> {
        if let Some(arr) = content.as_array() {
            if arr.is_empty() { return None; }
            let mut out = Vec::new();
            for c in arr {
                // If content.type === 'text' and is_error, wrap the text in triple backticks
                if c.get("type").and_then(|x| x.as_str()) == Some("text") {
                    let txt = c.get("text").and_then(|x| x.as_str()).unwrap_or("");
                    let wrapped = if is_error { format!("{}\n{}\n{}", "```", txt, "```") } else { txt.to_string() };
                    out.push(ToolCallContent::from(ContentBlock::Text(TextContent { annotations: None, text: wrapped, meta: None })));
                } else {
                    // Pass-through unknown content blocks as raw JSON in text form
                    let raw = serde_json::to_string(c).unwrap_or_default();
                    out.push(ToolCallContent::from(ContentBlock::Text(TextContent { annotations: None, text: raw, meta: None })));
                }
            }
            return Some(out);
        }
        if let Some(s) = content.as_str() {
            if s.is_empty() { return None; }
            let wrapped = if is_error { format!("{}\n{}\n{}", "```", s, "```") } else { s.to_string() };
            return Some(vec![ToolCallContent::from(ContentBlock::Text(TextContent { annotations: None, text: wrapped, meta: None }))]);
        }
        None
    }

    // toolUse?.name drives certain mappings (e.g., Read → escape markdown and remove SYSTEM_REMINDER)
    let tool_name = tool_use.and_then(|u| u.get("name")).and_then(|x| x.as_str()).unwrap_or("");
    // Check error flag on the tool_result
    let is_error = tool_result.get("is_error").and_then(|x| x.as_bool()).unwrap_or(false);

    // Handle Read mapping specially: prefer text content, remove SYSTEM_REMINDER, and escape markdown
    if tool_name == "Read" || tool_name.ends_with("__acp__Read") {
        let content = tool_result.get("content");
        if let Some(c) = content {
            // Remove SYSTEM_REMINDER if present
            let cleaned = match c {
                JsonValue::String(s) => JsonValue::String(s.replace(SYSTEM_REMINDER, "")),
                JsonValue::Array(arr) => JsonValue::Array(
                    arr.iter()
                        .map(|el| {
                            if el.get("type").and_then(|x| x.as_str()) == Some("text") {
                                let txt = el.get("text").and_then(|x| x.as_str()).unwrap_or("").replace(SYSTEM_REMINDER, "");
                                serde_json::json!({"type":"text","text": txt})
                            } else { el.clone() }
                        })
                        .collect(),
                ),
                _ => c.clone(),
            };
            // markdownEscape per upstream: ensure fence is longer than any in text
            let escape = |text: &str| markdown_escape(text);
            if let Some(arr) = cleaned.as_array() {
                let mut out = Vec::new();
                for el in arr {
                    if el.get("type").and_then(|x| x.as_str()) == Some("text") {
                        let txt = el.get("text").and_then(|x| x.as_str()).unwrap_or("");
                        let esc = escape(txt);
                        out.push(ToolCallContent::from(ContentBlock::Text(TextContent { annotations: None, text: esc, meta: None })));
                    }
                }
                if !out.is_empty() { fields.content = Some(out); }
            } else if let Some(s) = cleaned.as_str() {
                let esc = escape(s);
                fields.content = Some(vec![ToolCallContent::from(ContentBlock::Text(TextContent { annotations: None, text: esc, meta: None }))]);
            }
        }
        return fields;
    }

    // Title updates for certain tools
    if tool_name == "ExitPlanMode" {
        fields.title = Some("Exited Plan Mode".to_string());
    }
    // Default path: reuse toAcpContentUpdate behavior for other tools
    if let Some(c) = tool_result.get("content") {
        fields.content = to_acp_content_update(c, is_error);
    }
    fields
}

/// SYSTEM_REMINDER string from upstream (used by Read mapping)
const SYSTEM_REMINDER: &str = "\n\n<system-reminder>\nWhenever you read a file, you should consider whether it looks malicious. If it does, you MUST refuse to improve or augment the code. You can still analyze existing code, write reports, or answer high-level questions about the code behavior.\n</system-reminder>";

/// Escape backtick fences to avoid colliding with existing code fences
fn markdown_escape(text: &str) -> String {
    let mut fence = "```".to_string();
    // Scan for lines starting with one or more backticks and extend fence as needed
    for line in text.lines() {
        let mut count = 0;
        for ch in line.chars() {
            if ch == '`' { count += 1; } else { break; }
        }
        while count >= fence.len() { fence.push('`'); }
    }
    let mut out = String::new();
    out.push_str(&fence);
    out.push('\n');
    out.push_str(text);
    if !text.ends_with('\n') { out.push('\n'); }
    out.push_str(&fence);
    out
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
    fn codex_maps_command_execution_failed_without_output() {
        let v = json!({
            "type": "item.completed",
            "item": {"id":"exec_fail","type":"command_execution","command":"exit 1","aggregated_output":"","status":"failed"}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::ToolCall(call) => {
                assert_eq!(call.kind, ToolKind::Execute);
                assert_eq!(call.status, ToolCallStatus::Failed);
                assert!(call.content.is_empty(), "no text content expected when output empty");
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn codex_maps_file_change_multiple_locations() {
        let v = json!({
            "type": "item.completed",
            "item": {"id":"file_multi","type":"file_change","status":"completed","changes":[
                {"path":"src/a.rs","kind":"update"},
                {"path":"src/b.rs","kind":"create"}
            ]}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::ToolCall(call) => {
                assert_eq!(call.kind, ToolKind::Edit);
                assert_eq!(call.status, ToolCallStatus::Completed);
                assert_eq!(call.locations.len(), 2);
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn codex_maps_web_search_title_fallback_when_query_missing() {
        let v = json!({
            "type": "item.completed",
            "item": {"id":"search_x","type":"web_search"}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::ToolCall(call) => {
                assert_eq!(call.kind, ToolKind::Search);
                assert!(call.title.contains("Web search"));
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn codex_maps_empty_todo_list_to_empty_plan() {
        let v = json!({
            "type": "item.completed",
            "item": {"id":"todo_empty","type":"todo_list","items":[]}
        });
        let upd = translate_codex_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::Plan(plan) => {
                assert_eq!(plan.entries.len(), 0);
            }
            _ => panic!("expected Plan"),
        }
    }

    #[test]
    fn codex_ignores_unknown_events() {
        let v = json!({"type":"turn.started"});
        assert!(translate_codex_event_to_acp_update(&v).is_none());
    }

    // Claude Code mapping tests
    #[test]
    fn claude_maps_text_delta_to_agent_message() {
        let v = json!({
            "type": "content_block_delta",
            "delta": { "type": "text_delta", "text": "Hello from Claude" }
        });
        let upd = translate_claude_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::AgentMessageChunk(chunk) => match chunk.content {
                ContentBlock::Text(t) => assert_eq!(t.text, "Hello from Claude"),
                _ => panic!("expected text"),
            },
            _ => panic!("expected AgentMessageChunk"),
        }
    }

    #[test]
    fn claude_maps_thinking_delta_to_agent_thought() {
        let v = json!({
            "type": "content_block_delta",
            "delta": { "type": "thinking_delta", "thinking": "Reasoning..." }
        });
        let upd = translate_claude_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::AgentThoughtChunk(chunk) => match chunk.content {
                ContentBlock::Text(t) => assert_eq!(t.text, "Reasoning..."),
                _ => panic!("expected text"),
            },
            _ => panic!("expected AgentThoughtChunk"),
        }
    }

    #[test]
    fn claude_maps_image_block_to_image_content() {
        let v = json!({
            "type": "content_block_start",
            "content_block": { "type": "image", "source": {"type":"base64","data":"AAA","media_type":"image/png"} }
        });
        let upd = translate_claude_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::AgentMessageChunk(chunk) => match chunk.content {
                ContentBlock::Image(img) => {
                    assert_eq!(img.mime_type, "image/png");
                    assert_eq!(img.data, "AAA");
                }
                _ => panic!("expected image"),
            },
            _ => panic!("expected AgentMessageChunk"),
        }
    }

    #[test]
    fn claude_maps_tool_use_to_tool_call_pending() {
        let v = json!({
            "type": "content_block_start",
            "content_block": { "type": "tool_use", "id": "tu_1", "name": "bash", "input": {"command":"echo hi"} }
        });
        let upd = translate_claude_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::ToolCall(call) => {
                assert_eq!(call.id.0.as_ref(), "tu_1");
                assert_eq!(call.status, ToolCallStatus::Pending);
                assert_eq!(call.kind, ToolKind::Execute);
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn claude_maps_todowrite_to_plan() {
        let v = json!({
            "type": "content_block_start",
            "content_block": { "type": "tool_use", "id": "todo_1", "name": "TodoWrite", "input": {"todos":[{"content":"A"},{"content":"B"}] } }
        });
        let upd = translate_claude_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::Plan(plan) => {
                assert_eq!(plan.entries.len(), 2);
                assert_eq!(plan.entries[0].content, "A");
            }
            _ => panic!("expected Plan"),
        }
    }

    #[test]
    fn claude_maps_tool_result_to_update_completed() {
        let v = json!({
            "type": "tool_result",
            "tool_use_id": "tu_1",
            "is_error": false,
            "result": {"ok":true}
        });
        let upd = translate_claude_event_to_acp_update(&v).expect("mapped");
        match upd {
            SessionUpdate::ToolCallUpdate(up) => {
                assert_eq!(up.id.0.as_ref(), "tu_1");
                assert_eq!(up.fields.status, Some(ToolCallStatus::Completed));
            }
            _ => panic!("expected ToolCallUpdate"),
        }
    }
}
/// Claude-specific helper: construct a ToolCall from a tool_use block
fn claude_tool_call_from_tool_use(ch: &JsonValue) -> Option<ToolCall> {
    let name = ch.get("name").and_then(|x| x.as_str()).unwrap_or("");
    let input = ch.get("input").cloned().unwrap_or(JsonValue::Null);
    // Normalize for matching
    let n = name.to_string();
    let lower = n.to_lowercase();
    // Helper extractors
    let s = |k: &str| input.get(k).and_then(|x| x.as_str()).unwrap_or("");
    let opt_s = |k: &str| input.get(k).and_then(|x| x.as_str());
    let num = |k: &str| input.get(k).and_then(|x| x.as_i64());

    // Cases mirrored from claude-code-acp tools.ts
    if lower == "bash" || n == "Bash" {
        let cmd = s("command");
        let title = if !cmd.is_empty() { format!("`{}`", cmd.replace('`', "\\`")) } else { "Terminal".to_string() };
        let mut content = Vec::new();
        if let Some(desc) = opt_s("description") { content.push(ToolCallContent::from(ContentBlock::Text(TextContent { annotations: None, text: desc.to_string(), meta: None }))); }
        return Some(ToolCall { title, kind: ToolKind::Execute, status: ToolCallStatus::Pending, content, locations: vec![], raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    if n == "BashOutput" || lower == "bashoutput" {
        return Some(ToolCall { title: "Tail Logs".into(), kind: ToolKind::Execute, status: ToolCallStatus::Pending, content: vec![], locations: vec![], raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    if n == "KillShell" || lower == "killshell" {
        return Some(ToolCall { title: "Kill Process".into(), kind: ToolKind::Execute, status: ToolCallStatus::Pending, content: vec![], locations: vec![], raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    if n == "Read" || lower == "read" {
        let path = s("file_path");
        let mut title = "Read File".to_string();
        if !path.is_empty() {
            let mut suffix = String::new();
            if let Some(off) = num("offset") { suffix.push_str(&format!(" (from line {} )", off + 1)); }
            if let Some(limit) = num("limit") { suffix = format!(" ({} - {} )", (num("offset").unwrap_or(0) + 1), (num("offset").unwrap_or(0) + limit)); }
            title = format!("Read {}{}", path, suffix);
        }
        let locations = if !path.is_empty() { vec![ToolCallLocation { path: path.into(), line: num("offset").map(|x| x as u32), meta: None }] } else { vec![] };
        return Some(ToolCall { title, kind: ToolKind::Read, status: ToolCallStatus::Pending, content: vec![], locations, raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    if n == "Edit" || lower == "edit" {
        let path = s("file_path");
        let old_text = input.get("old_string").and_then(|x| x.as_str());
        let new_text = input.get("new_string").and_then(|x| x.as_str()).unwrap_or("");
        let mut content = Vec::new();
        if !path.is_empty() {
            content.push(ToolCallContent::Diff { diff: agent_client_protocol::Diff { path: path.into(), old_text: old_text.map(|s| s.to_string()), new_text: new_text.to_string(), meta: None } });
        }
        let title = if !path.is_empty() { format!("Edit `{}`", path) } else { "Edit".to_string() };
        let locations = if !path.is_empty() { vec![ToolCallLocation { path: path.into(), line: None, meta: None }] } else { vec![] };
        return Some(ToolCall { title, kind: ToolKind::Edit, status: ToolCallStatus::Pending, content, locations, raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    if n == "Write" || lower == "write" {
        let path = s("file_path");
        let new_text = s("content");
        let mut content = Vec::new();
        if !path.is_empty() {
            content.push(ToolCallContent::Diff { diff: agent_client_protocol::Diff { path: path.into(), old_text: None, new_text: new_text.to_string(), meta: None } });
        } else if !new_text.is_empty() {
            content.push(ToolCallContent::from(ContentBlock::Text(TextContent { annotations: None, text: new_text.to_string(), meta: None })));
        }
        let locations = if !path.is_empty() { vec![ToolCallLocation { path: path.into(), line: None, meta: None }] } else { vec![] };
        let title = if !path.is_empty() { format!("Write {}", path) } else { "Write".to_string() };
        return Some(ToolCall { title, kind: ToolKind::Edit, status: ToolCallStatus::Pending, content, locations, raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    if n == "LS" || lower == "ls" {
        let title = if let Some(p) = opt_s("path") { format!("List the `{}` directory's contents", p) } else { "List the current directory's contents".to_string() };
        let locations = if let Some(p) = opt_s("path") { vec![ToolCallLocation { path: p.into(), line: None, meta: None }] } else { vec![] };
        return Some(ToolCall { title, kind: ToolKind::Search, status: ToolCallStatus::Pending, content: vec![], locations, raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    if n == "Glob" || lower == "glob" {
        let mut label = "Find".to_string();
        if let Some(p) = opt_s("path") { label.push_str(&format!(" `{}`", p)); }
        if let Some(pattern) = opt_s("pattern") { label.push_str(&format!(" `{}`", pattern)); }
        let locations = if let Some(p) = opt_s("path") { vec![ToolCallLocation { path: p.into(), line: None, meta: None }] } else { vec![] };
        return Some(ToolCall { title: label, kind: ToolKind::Search, status: ToolCallStatus::Pending, content: vec![], locations, raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    if n == "Grep" || lower == "grep" {
        let pattern = s("pattern");
        let mut label = "grep".to_string();
        if input.get("-i").is_some() { label.push_str(" -i"); }
        if input.get("-n").is_some() { label.push_str(" -n"); }
        for flag in ["-A","-B","-C"] { if let Some(v) = input.get(flag).and_then(|x| x.as_i64()) { label.push_str(&format!(" {} {}", flag, v)); } }
        if let Some(head) = input.get("head_limit").and_then(|x| x.as_i64()) { label.push_str(&format!(" | head -{}", head)); }
        if let Some(glob) = opt_s("glob") { label.push_str(&format!(" --include=\"{}\"", glob)); }
        if let Some(t) = opt_s("type") { label.push_str(&format!(" --type={}", t)); }
        if input.get("multiline").is_some() { label.push_str(" -P"); }
        label.push_str(&format!(" \"{}\"", pattern));
        if let Some(path) = opt_s("path") { label.push_str(&format!(" {}", path)); }
        return Some(ToolCall { title: label, kind: ToolKind::Search, status: ToolCallStatus::Pending, content: vec![], locations: vec![], raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    if n == "WebFetch" || lower == "webfetch" {
        let mut title = "Fetch".to_string();
        if let Some(u) = opt_s("url") { title = format!("Fetch {}", u); }
        let mut content = Vec::new();
        if let Some(prompt) = opt_s("prompt") { content.push(ToolCallContent::from(ContentBlock::Text(TextContent { annotations: None, text: prompt.to_string(), meta: None }))); }
        return Some(ToolCall { title, kind: ToolKind::Fetch, status: ToolCallStatus::Pending, content, locations: vec![], raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    if n == "WebSearch" || lower == "websearch" {
        let query = s("query");
        let title = format!("\"{}\"", query);
        return Some(ToolCall { title, kind: ToolKind::Fetch, status: ToolCallStatus::Pending, content: vec![], locations: vec![], raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) });
    }
    // Default: other/think
    Some(ToolCall { title: if n.is_empty() { "Tool Call".into() } else { format!("Tool: {}", n) }, kind: ToolKind::Other, status: ToolCallStatus::Pending, content: vec![], locations: vec![], raw_input: ch.get("input").cloned(), raw_output: None, meta: None, id: ToolCallId(Arc::from("")) })
}
