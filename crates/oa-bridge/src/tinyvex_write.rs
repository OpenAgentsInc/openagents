use crate::state::AppState;
use serde_json::json;

/// Adapter: map writer notifications to existing WS JSON shapes and broadcast.
pub async fn stream_upsert_or_append(state: &AppState, thread_id: &str, kind: &str, full_text: &str) {
    let notifs = state
        .tinyvex_writer
        .stream_upsert_or_append("codex", thread_id, kind, full_text)
        .await;
    for n in notifs {
        match n {
            tinyvex::WriterNotification::ThreadsUpsert { row } => {
                let row_json = json!({
                    "id": row.id,
                    "thread_id": row.thread_id,
                    "title": row.title,
                    "project_id": row.project_id,
                    "resume_id": row.resume_id,
                    "rollout_path": row.rollout_path,
                    "source": row.source,
                    "created_at": row.created_at,
                    "updated_at": row.updated_at,
                    "message_count": row.message_count,
                });
                let _ = state.tx.send(json!({
                    "type":"tinyvex.update",
                    "stream":"threads",
                    "op":"upsert",
                    "threadId": thread_id,
                    "updatedAt": row.updated_at,
                    "row": row_json
                }).to_string());
            }
            tinyvex::WriterNotification::MessagesUpsert { thread_id, item_id, kind, seq, text_len, .. } => {
                let _ = state.tx.send(json!({
                    "type": "bridge.tinyvex_write",
                    "op": "upsertStreamed",
                    "threadId": thread_id,
                    "kind": kind,
                    "itemId": item_id,
                    "len": text_len,
                    "ok": true
                }).to_string());
                let _ = state.tx.send(json!({
                    "type": "tinyvex.update",
                    "stream": "messages",
                    "op": "upsertStreamed",
                    "threadId": thread_id,
                    "kind": kind,
                    "itemId": item_id,
                    "seq": seq
                }).to_string());
            }
            _ => {}
        }
    }
}

#[allow(dead_code)]
pub async fn try_finalize_stream_kind(state: &AppState, thread_id: &str, kind: &str) -> bool {
    if let Some(notifs) = state.tinyvex_writer.try_finalize_stream_kind(thread_id, kind).await {
        for n in notifs {
            if let tinyvex::WriterNotification::MessagesFinalize { thread_id, item_id, kind, text_len } = n {
                let _ = state.tx.send(json!({
                    "type": "bridge.tinyvex_write",
                    "op": "finalizeStreamed",
                    "threadId": thread_id,
                    "kind": kind,
                    "itemId": item_id,
                    "len": text_len,
                    "ok": true
                }).to_string());
                let _ = state.tx.send(json!({
                    "type": "tinyvex.update",
                    "stream": "messages",
                    "op": "finalizeStreamed",
                    "threadId": thread_id,
                    "kind": kind,
                    "itemId": item_id
                }).to_string());
            }
        }
        true
    } else {
        false
    }
}

/// Finalize a streamed item if present; if no stream exists, create a
/// one-shot snapshot with the provided `final_text` and finalize it.
pub async fn finalize_or_snapshot(state: &AppState, thread_id: &str, kind: &str, final_text: &str) {
    let notifs = state
        .tinyvex_writer
        .finalize_or_snapshot("codex", thread_id, kind, final_text)
        .await;
    // Map finalization notifications to insert events for tinyvex.update
    for n in notifs {
        match n {
            tinyvex::WriterNotification::ThreadsUpsert { row } => {
                let row_json = json!({
                    "id": row.id,
                    "thread_id": row.thread_id,
                    "title": row.title,
                    "project_id": row.project_id,
                    "resume_id": row.resume_id,
                    "rollout_path": row.rollout_path,
                    "source": row.source,
                    "created_at": row.created_at,
                    "updated_at": row.updated_at,
                    "message_count": row.message_count,
                });
                let _ = state.tx.send(json!({
                    "type":"tinyvex.update",
                    "stream":"threads",
                    "op":"upsert",
                    "threadId": thread_id,
                    "updatedAt": row.updated_at,
                    "row": row_json
                }).to_string());
            }
            tinyvex::WriterNotification::MessagesUpsert { thread_id, item_id, kind, text_len, .. } => {
                // still surface a progress debug
                let _ = state.tx.send(json!({
                    "type": "bridge.tinyvex_write",
                    "op": "upsertStreamed",
                    "threadId": thread_id,
                    "kind": kind,
                    "itemId": item_id,
                    "len": text_len,
                    "ok": true
                }).to_string());
            }
            tinyvex::WriterNotification::MessagesFinalize { thread_id, item_id, .. } => {
                // For snapshot-style writes, keep legacy op label "insert"
                let _ = state.tx.send(json!({
                    "type":"tinyvex.update",
                    "stream":"messages",
                    "op":"insert",
                    "threadId": thread_id,
                    "itemId": item_id
                }).to_string());
            }
            _ => {}
        }
    }
}

pub async fn finalize_streaming_for_thread(state: &AppState, thread_id: &str) {
    let notifs = state
        .tinyvex_writer
        .finalize_streaming_for_thread(thread_id)
        .await;
    for n in notifs {
        if let tinyvex::WriterNotification::MessagesFinalize { thread_id, item_id, kind, text_len } = n {
            let _ = state.tx.send(json!({
                "type": "bridge.tinyvex_write",
                "op": "finalizeStreamed",
                "threadId": thread_id,
                "kind": kind,
                "itemId": item_id,
                "len": text_len,
                "ok": true
            }).to_string());
            let _ = state.tx.send(json!({
                "type": "tinyvex.update",
                "stream": "messages",
                "op": "finalizeStreamed",
                "threadId": thread_id,
                "kind": kind,
                "itemId": item_id
            }).to_string());
        }
    }
}

pub fn summarize_exec_delta_for_log(line: &str) -> Option<String> {
    if line.len() > 24 * 1024 {
        Some(format!("[jsonl delta ~{} bytes]", line.len()))
    } else {
        None
    }
}

/// Mirror ACP session updates into Tinyvex tables and broadcast minimal deltas.
pub async fn mirror_acp_update_to_tinyvex(
    state: &AppState,
    provider: &str,
    thread_id: &str,
    update: &agent_client_protocol::SessionUpdate,
) {
    let notifs = state
        .tinyvex_writer
        .mirror_acp_update_to_tinyvex(provider, thread_id, update)
        .await;
    for n in notifs {
        match n {
            tinyvex::WriterNotification::ThreadsUpsert { row } => {
                let row_json = json!({
                    "id": row.id,
                    "thread_id": row.thread_id,
                    "title": row.title,
                    "project_id": row.project_id,
                    "resume_id": row.resume_id,
                    "rollout_path": row.rollout_path,
                    "source": row.source,
                    "created_at": row.created_at,
                    "updated_at": row.updated_at,
                    "message_count": row.message_count,
                });
                let _ = state.tx.send(json!({
                    "type":"tinyvex.update",
                    "stream":"threads",
                    "op":"upsert",
                    "threadId": thread_id,
                    "updatedAt": row.updated_at,
                    "row": row_json
                }).to_string());
            }
            tinyvex::WriterNotification::MessagesUpsert { thread_id, item_id, kind, text_len, .. } => {
                // Progress debug for visibility
                let _ = state.tx.send(json!({
                    "type": "bridge.tinyvex_write",
                    "op": "upsertStreamed",
                    "threadId": thread_id,
                    "kind": kind,
                    "itemId": item_id,
                    "len": text_len,
                    "ok": true
                }).to_string());
            }
            tinyvex::WriterNotification::MessagesFinalize { thread_id, item_id, .. } => {
                // Keep legacy insert op for ACP snapshots
                let _ = state.tx.send(json!({
                    "type":"tinyvex.update",
                    "stream":"messages",
                    "op":"insert",
                    "threadId": thread_id,
                    "itemId": item_id
                }).to_string());
            }
            tinyvex::WriterNotification::ToolCallUpsert { thread_id, tool_call_id } => {
                // Debug event + typed tinyvex.update for tools
                let _ = state.tx.send(json!({
                    "type": "bridge.tinyvex_write",
                    "op": "toolCallUpsert",
                    "threadId": thread_id,
                    "toolCallId": tool_call_id,
                    "ok": true
                }).to_string());
                let _ = state.tx.send(json!({
                    "type":"tinyvex.update",
                    "stream":"toolCalls",
                    "op":"upsert",
                    "threadId": thread_id,
                    "toolCallId": tool_call_id
                }).to_string());
            }
            tinyvex::WriterNotification::ToolCallUpdate { thread_id, tool_call_id } => {
                let _ = state.tx.send(json!({
                    "type": "bridge.tinyvex_write",
                    "op": "toolCallUpdate",
                    "threadId": thread_id,
                    "toolCallId": tool_call_id,
                    "ok": true
                }).to_string());
                let _ = state.tx.send(json!({
                    "type":"tinyvex.update",
                    "stream":"toolCalls",
                    "op":"update",
                    "threadId": thread_id,
                    "toolCallId": tool_call_id
                }).to_string());
            }
            tinyvex::WriterNotification::PlanUpsert { thread_id } => {
                let _ = state.tx.send(json!({
                    "type": "bridge.tinyvex_write",
                    "op": "planUpsert",
                    "threadId": thread_id,
                    "ok": true
                }).to_string());
                let _ = state.tx.send(json!({
                    "type":"tinyvex.update",
                    "stream":"plan",
                    "op":"upsert",
                    "threadId": thread_id
                }).to_string());
            }
            tinyvex::WriterNotification::StateUpsert { thread_id } => {
                let _ = state.tx.send(json!({
                    "type": "bridge.tinyvex_write",
                    "op": "stateUpsert",
                    "threadId": thread_id,
                    "ok": true
                }).to_string());
                let _ = state.tx.send(json!({
                    "type":"tinyvex.update",
                    "stream":"state",
                    "op":"upsert",
                    "threadId": thread_id
                }).to_string());
            }
        }
    }

    // Optional: two-way writer — persist non-Codex provider sessions as Codex-compatible JSONL
    // Guarded by `sync_two_way` and only for non-"codex" provider.
    if state.sync_two_way.load(std::sync::atomic::Ordering::Relaxed) {
        let thread = thread_id.to_string();
        let prov = provider.to_string();
        let upd = update.clone();
        tokio::spawn(async move {
            let res = match prov.as_str() {
                "codex" => Ok(()), // Codex persists its own logs
                "claude_code" => append_two_way_jsonl_claude(&thread, &upd).await,
                _ => append_two_way_jsonl_codex_compat(&thread, &upd).await,
            };
            if let Err(e) = res {
                tracing::warn!(?e, thread_id=%thread, provider=%prov, "two_way_writer: append failed");
            }
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamMode { Delta, Finalize }

/// Variant that allows callers to indicate whether a chunk is a streaming delta or a finalization.
pub async fn mirror_acp_update_to_tinyvex_mode(
    state: &AppState,
    provider: &str,
    thread_id: &str,
    update: &agent_client_protocol::SessionUpdate,
    mode: StreamMode,
) {
    use agent_client_protocol::SessionUpdate as SU;
    match update {
        SU::UserMessageChunk(ch) => {
            let txt = if let agent_client_protocol::ContentBlock::Text(t) = &ch.content { t.text.clone() } else { String::new() };
            if txt.is_empty() { return; }
            match mode {
                StreamMode::Delta => stream_upsert_or_append(state, thread_id, "user", &txt).await,
                StreamMode::Finalize => { let _ = try_finalize_stream_kind(state, thread_id, "user").await; },
            }
            return;
        }
        SU::AgentMessageChunk(ch) => {
            let txt = if let agent_client_protocol::ContentBlock::Text(t) = &ch.content { t.text.clone() } else { String::new() };
            if txt.is_empty() { return; }
            match mode {
                StreamMode::Delta => stream_upsert_or_append(state, thread_id, "assistant", &txt).await,
                StreamMode::Finalize => { let _ = try_finalize_stream_kind(state, thread_id, "assistant").await; },
            }
            return;
        }
        SU::AgentThoughtChunk(ch) => {
            let txt = if let agent_client_protocol::ContentBlock::Text(t) = &ch.content { t.text.clone() } else { String::new() };
            if txt.is_empty() { return; }
            match mode {
                StreamMode::Delta => stream_upsert_or_append(state, thread_id, "reason", &txt).await,
                StreamMode::Finalize => { let _ = try_finalize_stream_kind(state, thread_id, "reason").await; },
            }
            return;
        }
        _ => {}
    }
    // Fallback for other update kinds — reuse existing behavior
    mirror_acp_update_to_tinyvex(state, provider, thread_id, update).await;
}

fn two_way_base_dir_codex_openagents() -> std::path::PathBuf {
    // Honor CODEXD_HISTORY_DIR via the shared helper used by the watcher,
    // then append our provider namespace to avoid collisions.
    let p = crate::watchers::codex_base_path().join("openagents");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn claude_sessions_base_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("CLAUDE_SESSIONS_DIR") {
        let dir = std::path::PathBuf::from(p);
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }
    // Default to ~/.claude/sessions
    let base = std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or(std::env::temp_dir());
    let dir = base.join(".claude").join("sessions");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

async fn append_two_way_jsonl_codex_compat(thread_id: &str, update: &agent_client_protocol::SessionUpdate) -> anyhow::Result<()> {
    use agent_client_protocol::{SessionUpdate as SU, ContentBlock, ToolCallStatus, ToolKind};
    let base = two_way_base_dir_codex_openagents();
    let file_path = base.join(format!("{}.jsonl", thread_id));

    // Helper to append lines with a thread.started header on first write
    let write_lines = |file_path: &std::path::Path, thread: &str, lines: Vec<serde_json::Value>| -> anyhow::Result<()> {
        if lines.is_empty() { return Ok(()); }
        use std::io::Write;
        let first_write = !file_path.exists() || std::fs::metadata(file_path).map(|m| m.len() == 0).unwrap_or(true);
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file_path)?;
        if first_write {
            let started = serde_json::json!({"type":"thread.started","thread_id": thread});
            writeln!(f, "{}", started.to_string())?;
        }
        for v in lines { writeln!(f, "{}", v.to_string())?; }
        Ok(())
    };

    let mut lines: Vec<serde_json::Value> = Vec::new();
    match update {
        // Textual items → completed lines
        SU::UserMessageChunk(ch) => {
            let text = match &ch.content { ContentBlock::Text(t) => t.text.clone(), _ => String::new() };
            if !text.is_empty() { lines.push(serde_json::json!({"type":"item.completed","item":{"type":"user_message","text": text}})); }
        }
        SU::AgentMessageChunk(ch) => {
            let text = match &ch.content { ContentBlock::Text(t) => t.text.clone(), _ => String::new() };
            if !text.is_empty() { lines.push(serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text": text}})); }
        }
        SU::AgentThoughtChunk(ch) => {
            let text = match &ch.content { ContentBlock::Text(t) => t.text.clone(), _ => String::new() };
            if !text.is_empty() { lines.push(serde_json::json!({"type":"item.completed","item":{"type":"reasoning","text": text}})); }
        }
        // ToolCall → item.started (status in_progress)
        SU::ToolCall(call) => {
            match call.kind {
                ToolKind::Execute => {
                    let mut v = serde_json::json!({"type":"command_execution","status":"in_progress","id": call.id.0.as_ref()});
                    if let Some(raw) = &call.raw_input {
                        if let Some(cmd) = raw.get("command").and_then(|x| x.as_str()) { v["command"] = serde_json::json!(cmd); }
                        if let Some(cmd) = raw.get("cmd").and_then(|x| x.as_str()) { v["command"] = serde_json::json!(cmd); }
                    }
                    lines.push(serde_json::json!({"type":"item.started","item": v}));
                }
                ToolKind::Edit => {
                    let mut changes: Vec<serde_json::Value> = Vec::new();
                    for loc in &call.locations {
                        let p = &loc.path;
                        let non_empty = match p.as_os_str().is_empty() { false => true, true => false };
                        if non_empty {
                            let path_str = p.to_string_lossy().to_string();
                            changes.push(serde_json::json!({"path": path_str, "kind": "edit"}));
                        }
                    }
                    let mut v = serde_json::json!({"type":"file_change","status":"in_progress","id": call.id.0.as_ref()});
                    if !changes.is_empty() { v["changes"] = serde_json::json!(changes); }
                    lines.push(serde_json::json!({"type":"item.started","item": v}));
                }
                ToolKind::Search => {
                    let mut v = serde_json::json!({"type":"web_search","status":"in_progress","id": call.id.0.as_ref()});
                    if let Some(raw) = &call.raw_input { if let Some(q) = raw.get("query").and_then(|x| x.as_str()) { v["query"] = serde_json::json!(q); } }
                    lines.push(serde_json::json!({"type":"item.started","item": v}));
                }
                ToolKind::Fetch => {
                    let mut v = serde_json::json!({"type":"web_fetch","status":"in_progress","id": call.id.0.as_ref()});
                    if let Some(raw) = &call.raw_input { if let Some(u) = raw.get("url").and_then(|x| x.as_str()) { v["url"] = serde_json::json!(u); } }
                    lines.push(serde_json::json!({"type":"item.started","item": v}));
                }
                _ => {
                    let v = serde_json::json!({"type":"mcp_tool_call","status":"in_progress","id": call.id.0.as_ref()});
                    lines.push(serde_json::json!({"type":"item.started","item": v}));
                }
            }
        }
        // ToolCallUpdate → item.completed with status and optional stdout/text summary
        SU::ToolCallUpdate(up) => {
            let status = match up.fields.status.unwrap_or(ToolCallStatus::Completed) {
                ToolCallStatus::Failed => "failed",
                ToolCallStatus::InProgress => "in_progress",
                ToolCallStatus::Pending => "pending",
                ToolCallStatus::Completed => "completed",
            };
            let mut summary: Option<String> = None;
            if let Some(content) = &up.fields.content {
                let mut buf = String::new();
                for c in content {
                    if let Ok(val) = serde_json::to_value(c) {
                        if val.get("type").and_then(|x| x.as_str()) == Some("text") {
                            if let Some(t) = val.get("text").and_then(|x| x.as_str()) {
                                if !buf.is_empty() { buf.push('\n'); }
                                buf.push_str(t);
                            }
                        }
                    }
                }
                if !buf.trim().is_empty() { summary = Some(buf); }
            }
            if summary.is_none() {
                if let Some(raw) = &up.fields.raw_output {
                    if let Some(stdout) = raw.get("stdout").and_then(|x| x.as_str()) { if !stdout.is_empty() { summary = Some(stdout.to_string()); } }
                }
            }
            let mut item = serde_json::json!({"type":"mcp_tool_call","id": up.id.0.as_ref(), "status": status});
            if let Some(text) = summary { item["stdout"] = serde_json::json!(text); }
            lines.push(serde_json::json!({"type":"item.completed","item": item}));
        }
        // Plan → todo_list completion
        SU::Plan(plan) => {
            let mut items: Vec<serde_json::Value> = Vec::new();
            for e in &plan.entries {
                let completed = matches!(e.status, agent_client_protocol::PlanEntryStatus::Completed);
                items.push(serde_json::json!({"text": e.content, "completed": completed}));
            }
            if !items.is_empty() {
                lines.push(serde_json::json!({"type":"item.completed","item": {"type":"todo_list","items": items}}));
            }
        }
        _ => {}
    }

    if lines.is_empty() { return Ok(()); }
    tokio::task::spawn_blocking({
        let thread = thread_id.to_string();
        let file_path = file_path.clone();
        move || -> anyhow::Result<()> { write_lines(&file_path, &thread, lines) }
    }).await??;
    Ok(())
}

async fn append_two_way_jsonl_claude(thread_id: &str, update: &agent_client_protocol::SessionUpdate) -> anyhow::Result<()> {
    use agent_client_protocol::{SessionUpdate as SU, ContentBlock, ToolCallStatus, ToolKind};
    let base = claude_sessions_base_path();
    let file_path = base.join(format!("{}.stream.jsonl", thread_id));

    // Helper to append lines with a system init header on first write
    let write_lines = |file_path: &std::path::Path, session_id: &str, lines: Vec<serde_json::Value>| -> anyhow::Result<()> {
        if lines.is_empty() { return Ok(()); }
        use std::io::Write;
        let first_write = !file_path.exists() || std::fs::metadata(file_path).map(|m| m.len() == 0).unwrap_or(true);
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file_path)?;
        if first_write {
            let init = serde_json::json!({"type":"system","subtype":"init","session_id": session_id});
            writeln!(f, "{}", init.to_string())?;
        }
        for v in lines { writeln!(f, "{}", v.to_string())?; }
        Ok(())
    };

    let mut lines: Vec<serde_json::Value> = Vec::new();
    match update {
        // Messages
        SU::UserMessageChunk(ch) => {
            if let ContentBlock::Text(t) = &ch.content {
                if !t.text.is_empty() {
                    lines.push(serde_json::json!({
                        "type": "user",
                        "message": {"content": [{"type":"text","text": t.text}]}
                    }));
                }
            }
        }
        SU::AgentMessageChunk(ch) | SU::AgentThoughtChunk(ch) => {
            if let ContentBlock::Text(t) = &ch.content {
                if !t.text.is_empty() {
                    lines.push(serde_json::json!({
                        "type": "assistant",
                        "message": {"content": [{"type":"text","text": t.text}]}
                    }));
                }
            }
        }
        // Tool call start → assistant.tool_use
        SU::ToolCall(call) => {
            let name = match call.kind {
                ToolKind::Execute => "Bash",
                ToolKind::Edit => "Edit",
                ToolKind::Read => "Read",
                ToolKind::Search => "WebSearch",
                ToolKind::Fetch => "WebFetch",
                _ => "Tool",
            };
            let input = call.raw_input.clone().unwrap_or(serde_json::json!({}));
            lines.push(serde_json::json!({
                "type": "assistant",
                "message": {"content": [{"type":"tool_use","id": call.id.0.as_ref(), "name": name, "input": input}]}
            }));
        }
        // Tool call update → user.tool_result
        SU::ToolCallUpdate(up) => {
            let is_error = matches!(up.fields.status.unwrap_or(ToolCallStatus::Completed), ToolCallStatus::Failed);
            // Prefer text aggregation from content; else raw_output.stdout
            let mut text: Option<String> = None;
            if let Some(content) = &up.fields.content {
                let mut buf = String::new();
                for c in content {
                    if let Ok(val) = serde_json::to_value(c) {
                        if val.get("type").and_then(|x| x.as_str()) == Some("text") {
                            if let Some(t) = val.get("text").and_then(|x| x.as_str()) {
                                if !buf.is_empty() { buf.push('\n'); }
                                buf.push_str(t);
                            }
                        }
                    }
                }
                if !buf.trim().is_empty() { text = Some(buf); }
            }
            if text.is_none() {
                if let Some(raw) = &up.fields.raw_output {
                    if let Some(stdout) = raw.get("stdout").and_then(|x| x.as_str()) {
                        if !stdout.is_empty() { text = Some(stdout.to_string()); }
                    }
                }
            }
            let content_val = text.map(|s| serde_json::json!({"type":"text","text": s})).unwrap_or(serde_json::json!({"type":"text","text":""}));
            lines.push(serde_json::json!({
                "type": "user",
                "message": {"content": [{"type":"tool_result","tool_use_id": up.id.0.as_ref(), "content": content_val}], "is_error": is_error}
            }));
        }
        // Plan → assistant text (simple checklist)
        SU::Plan(plan) => {
            if !plan.entries.is_empty() {
                let mut s = String::new();
                for e in &plan.entries {
                    let done = matches!(e.status, agent_client_protocol::PlanEntryStatus::Completed);
                    let mark = if done { "[x]" } else { "[ ]" };
                    s.push_str(&format!("- {} {}\n", mark, e.content));
                }
                lines.push(serde_json::json!({
                    "type": "assistant",
                    "message": {"content": [{"type":"text","text": s}]}
                }));
            }
        }
        _ => {}
    }

    if lines.is_empty() { return Ok(()); }
    tokio::task::spawn_blocking({
        let sid = thread_id.to_string();
        let file_path = file_path.clone();
        move || -> anyhow::Result<()> { write_lines(&file_path, &sid, lines) }
    }).await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::{broadcast, Mutex};

    #[tokio::test]
    async fn tinyvex_stream_upsert_broadcasts() {
        let (tx, mut rx) = broadcast::channel(8);
        let tvx = std::sync::Arc::new(tinyvex::Tinyvex::open(tempfile::NamedTempFile::new().unwrap().path()).unwrap());
        let state = crate::state::AppState {
            tx,
            child_stdin: Mutex::new(None),
            child_pid: Mutex::new(None),
            opts: crate::Opts {
                bind: "127.0.0.1:0".into(),
                codex_bin: None,
                codex_args: None,
                extra: vec![],
                ws_token: Some("t".into()),
                claude_bin: None,
                claude_args: None,
            },
            last_thread_id: Mutex::new(None),
            history: Mutex::new(Vec::new()),
            current_thread_doc: Mutex::new(None),
            stream_track: Mutex::new(std::collections::HashMap::new()),
            pending_user_text: Mutex::new(std::collections::HashMap::new()),
            sessions_by_client_doc: Mutex::new(std::collections::HashMap::new()),
            bridge_ready: std::sync::atomic::AtomicBool::new(true),
            tinyvex: tvx.clone(),
            tinyvex_writer: std::sync::Arc::new(tinyvex::Writer::new(tvx.clone())),
            sync_enabled: std::sync::atomic::AtomicBool::new(true),
            sync_two_way: std::sync::atomic::AtomicBool::new(false),
            sync_last_read_ms: Mutex::new(0),
            sync_cmd_tx: Mutex::new(None),
            client_doc_by_session: Mutex::new(std::collections::HashMap::new()),
        };
        stream_upsert_or_append(&state, "th", "assistant", "hello").await;
        // Drain until we see the bridge.tinyvex_write event
        let mut saw = false;
        for _ in 0..4 {
            if let Ok(Ok(msg)) = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
                let v: serde_json::Value = serde_json::from_str(&msg).unwrap_or_default();
                if v.get("type").and_then(|x| x.as_str()) == Some("bridge.tinyvex_write") {
                    assert_eq!(v.get("op").and_then(|x| x.as_str()), Some("upsertStreamed"));
                    saw = true;
                    break;
                }
            }
        }
        assert!(saw, "expected bridge.tinyvex_write event");
    }

    #[tokio::test]
    async fn finalize_or_snapshot_creates_single_row_without_stream() {
        let (tx, _rx) = broadcast::channel(4);
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("tvx.sqlite3");
        let tvx = std::sync::Arc::new(tinyvex::Tinyvex::open(&db_path).unwrap());
        let state = crate::state::AppState {
            tx,
            child_stdin: Mutex::new(None),
            child_pid: Mutex::new(None),
            opts: crate::Opts {
                bind: "127.0.0.1:0".into(),
                codex_bin: None,
                codex_args: None,
                extra: vec![],
                ws_token: Some("t".into()),
                claude_bin: None,
                claude_args: None,
            },
            last_thread_id: Mutex::new(None),
            history: Mutex::new(Vec::new()),
            current_thread_doc: Mutex::new(None),
            stream_track: Mutex::new(std::collections::HashMap::new()),
            pending_user_text: Mutex::new(std::collections::HashMap::new()),
            sessions_by_client_doc: Mutex::new(std::collections::HashMap::new()),
            bridge_ready: std::sync::atomic::AtomicBool::new(true),
            tinyvex: tvx.clone(),
            tinyvex_writer: std::sync::Arc::new(tinyvex::Writer::new(tvx.clone())),
            sync_enabled: std::sync::atomic::AtomicBool::new(true),
            sync_two_way: std::sync::atomic::AtomicBool::new(false),
            sync_last_read_ms: Mutex::new(0),
            sync_cmd_tx: Mutex::new(None),
            client_doc_by_session: Mutex::new(std::collections::HashMap::new()),
        };
        finalize_or_snapshot(&state, "th", "assistant", "hello world").await;
        let rows = state.tinyvex.list_messages("th", 50).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text.as_deref(), Some("hello world"));
        assert_eq!(rows[0].partial, Some(0));
    }

    #[tokio::test]
    async fn upsert_then_finalize_does_not_duplicate() {
        let (tx, _rx) = broadcast::channel(4);
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("tvx.sqlite3");
        let tvx = std::sync::Arc::new(tinyvex::Tinyvex::open(&db_path).unwrap());
        let state = crate::state::AppState {
            tx,
            child_stdin: Mutex::new(None),
            child_pid: Mutex::new(None),
            opts: crate::Opts {
                bind: "127.0.0.1:0".into(),
                codex_bin: None,
                codex_args: None,
                extra: vec![],
                ws_token: Some("t".into()),
                claude_bin: None,
                claude_args: None,
            },
            last_thread_id: Mutex::new(None),
            history: Mutex::new(Vec::new()),
            current_thread_doc: Mutex::new(None),
            stream_track: Mutex::new(std::collections::HashMap::new()),
            pending_user_text: Mutex::new(std::collections::HashMap::new()),
            sessions_by_client_doc: Mutex::new(std::collections::HashMap::new()),
            bridge_ready: std::sync::atomic::AtomicBool::new(true),
            tinyvex: tvx.clone(),
            tinyvex_writer: std::sync::Arc::new(tinyvex::Writer::new(tvx.clone())),
            sync_enabled: std::sync::atomic::AtomicBool::new(true),
            sync_two_way: std::sync::atomic::AtomicBool::new(false),
            sync_last_read_ms: Mutex::new(0),
            sync_cmd_tx: Mutex::new(None),
            client_doc_by_session: Mutex::new(std::collections::HashMap::new()),
        };
        // Simulate deltas then final.
        stream_upsert_or_append(&state, "th", "assistant", "hello").await;
        finalize_or_snapshot(&state, "th", "assistant", "hello").await;
        let rows = state.tinyvex.list_messages("th", 50).unwrap();
        assert_eq!(rows.len(), 1, "expected exactly one finalized row");
        assert_eq!(rows[0].partial, Some(0));
    }
}
