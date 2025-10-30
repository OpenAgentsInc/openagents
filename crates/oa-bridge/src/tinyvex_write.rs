use crate::state::AppState;
use serde_json::json;
use tracing::warn;
use agent_client_protocol as acp;

fn now_ms() -> i64 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()) as i64
}

pub async fn stream_upsert_or_append(state: &AppState, thread_id: &str, kind: &str, full_text: &str) {
    let mut guard = state.stream_track.lock().await;
    let key = format!("{}|{}", thread_id, kind);
    let entry = guard
        .entry(key.clone())
        .or_insert_with(|| crate::state::StreamEntry { item_id: String::new(), last_text: String::new(), seq: 0 });
    entry.seq = entry.seq.saturating_add(1);
    entry.last_text = full_text.to_string();
    let seq_now = entry.seq;
    // Assign a unique per-turn item id on first write for this kind, then reuse it
    if entry.item_id.is_empty() {
        entry.item_id = format!("turn:{}:{}", now_ms(), kind);
    }
    let item_id = entry.item_id.clone();
    drop(guard);
    let role = if kind == "assistant" { Some("assistant") } else { None };
    let out_kind = if kind == "assistant" { "message" } else if kind == "reason" { "reason" } else { kind };
    let t = now_ms();
    // Ensure a thread row exists/upserted for listings
    let thr = tinyvex::ThreadRow {
        id: thread_id.to_string(),
        thread_id: Some(thread_id.to_string()),
        title: "Thread".into(),
        project_id: None,
        resume_id: Some(thread_id.to_string()),
        rollout_path: None,
        // Mark provider for UI mapping
        source: Some("codex".into()),
        created_at: t,
        updated_at: t,
        message_count: None,
    };
    let _ = state.tinyvex.upsert_thread(&thr);
    // Notify clients that threads list may have changed. Include the row so
    // clients can update state without re-querying threads.list.
    let row = json!({
        "id": thr.id,
        "thread_id": thr.thread_id,
        "title": thr.title,
        "project_id": thr.project_id,
        "resume_id": thr.resume_id,
        "rollout_path": thr.rollout_path,
        "source": thr.source,
        "created_at": thr.created_at,
        "updated_at": thr.updated_at,
        "message_count": thr.message_count,
    });
    let _ = state.tx.send(json!({
        "type": "tinyvex.update",
        "stream": "threads",
        "op": "upsert",
        "threadId": thread_id,
        "updatedAt": t,
        "row": row
    }).to_string());
    if let Err(e) = state.tinyvex.upsert_streamed_message(thread_id, out_kind, role, full_text, &item_id, seq_now as i64, t) {
        warn!(?e, "tinyvex upsert_streamed_message failed");
    }
    let _ = state.tx.send(json!({
        "type": "bridge.tinyvex_write",
        "op": "upsertStreamed",
        "threadId": thread_id,
        "kind": out_kind,
        "itemId": item_id,
        "len": full_text.len(),
        "ok": true
    }).to_string());
    // Send a generic tinyvex.update for subscribers
    let _ = state.tx.send(json!({
        "type": "tinyvex.update",
        "stream": "messages",
        "op": "upsertStreamed",
        "threadId": thread_id,
        "kind": out_kind,
        "itemId": item_id,
        "seq": seq_now
    }).to_string());
}

pub async fn try_finalize_stream_kind(state: &AppState, thread_id: &str, kind: &str) -> bool {
    // Remove the tracked streaming entry, get its last item_id and text, and finalize it
    let (item_id, final_text) = {
        let mut guard = state.stream_track.lock().await;
        let key = format!("{}|{}", thread_id, kind);
        if let Some(entry) = guard.remove(&key) {
            (entry.item_id.clone(), entry.last_text.clone())
        } else {
            return false;
        }
    };
    let t = now_ms();
    if let Err(e) = state.tinyvex.finalize_streamed_message(thread_id, &item_id, &final_text, t) {
        warn!(?e, "tinyvex finalize_streamed_message failed");
        return false;
    }
    let _ = state.tx.send(json!({
        "type": "bridge.tinyvex_write",
        "op": "finalizeStreamed",
        "threadId": thread_id,
        "kind": kind,
        "itemId": item_id,
        "len": final_text.len(),
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
    true
}

/// Finalize a streamed item if present; if no stream exists, create a
/// one-shot snapshot with the provided `final_text` and finalize it.
pub async fn finalize_or_snapshot(state: &AppState, thread_id: &str, kind: &str, final_text: &str) {
    if !try_finalize_stream_kind(state, thread_id, kind).await {
        // No prior streaming entry — create a snapshot row and finalize it.
        stream_upsert_or_append(state, thread_id, kind, final_text).await;
        let _ = try_finalize_stream_kind(state, thread_id, kind).await;
    }
}

pub async fn finalize_streaming_for_thread(state: &AppState, thread_id: &str) {
    let kinds: Vec<String> = {
        let guard = state.stream_track.lock().await;
        guard
            .keys()
            .filter_map(|k| { let mut p = k.split('|'); let tid = p.next()?; let kind = p.next()?; if tid == thread_id { Some(kind.to_string()) } else { None } })
            .collect()
    };
    for kind in kinds {
        let _ = try_finalize_stream_kind(state, thread_id, &kind).await;
    }
}

pub fn summarize_exec_delta_for_log(line: &str) -> Option<String> {
    if line.len() > 24 * 1024 { Some(format!("[jsonl delta ~{} bytes]", line.len())) } else { None }
}

/// Mirror ACP session updates into Tinyvex tables (no Convex).
pub async fn mirror_acp_update_to_tinyvex(state: &AppState, provider: &str, thread_id: &str, update: &agent_client_protocol::SessionUpdate) {
    // Mirror ACP session updates into Tinyvex tables for tools/plan/state
    let t = now_ms();
    match update {
        agent_client_protocol::SessionUpdate::ToolCall(tc) => {
            let id = format!("{:?}", tc.id);
            let title = tc.title.as_str();
            let kind = format!("{:?}", tc.kind);
            let status = format!("{:?}", tc.status);
            let content_json = serde_json::to_string(&tc.content).unwrap_or("[]".into());
            let locations_json = serde_json::to_string(&tc.locations).unwrap_or("[]".into());
            let _ = state.tinyvex.upsert_acp_tool_call(thread_id, &id, title, &kind, &status, &content_json, &locations_json, t);
        }
        agent_client_protocol::SessionUpdate::ToolCallUpdate(tc) => {
            let id = format!("{:?}", tc.id);
            // Fields on ToolCallUpdate live under `fields`
            let title: &str = tc
                .fields
                .title
                .as_deref()
                .unwrap_or("");
            let kind: String = tc
                .fields
                .kind
                .as_ref()
                .map(|k| format!("{:?}", k))
                .unwrap_or_else(|| "".to_string());
            let status: String = tc
                .fields
                .status
                .as_ref()
                .map(|s| format!("{:?}", s))
                .unwrap_or_else(|| "".to_string());
            let content_json: String = tc
                .fields
                .content
                .as_ref()
                .map(|c| serde_json::to_string(c).unwrap_or("[]".into()))
                .unwrap_or_else(|| "[]".into());
            let locations_json: String = tc
                .fields
                .locations
                .as_ref()
                .map(|l| serde_json::to_string(l).unwrap_or("[]".into()))
                .unwrap_or_else(|| "[]".into());
            let _ = state
                .tinyvex
                .upsert_acp_tool_call(
                    thread_id,
                    &id,
                    title,
                    &kind,
                    &status,
                    &content_json,
                    &locations_json,
                    t,
                );
        }
        agent_client_protocol::SessionUpdate::Plan(p) => {
            let entries_json = serde_json::to_string(&p.entries).unwrap_or("[]".into());
            let _ = state.tinyvex.upsert_acp_plan(thread_id, &entries_json, t);
        }
        agent_client_protocol::SessionUpdate::AvailableCommandsUpdate(ac) => {
            let cmds_json = serde_json::to_string(&ac.available_commands).unwrap_or("[]".into());
            let _ = state.tinyvex.upsert_acp_state(thread_id, None, Some(&cmds_json), t);
        }
        agent_client_protocol::SessionUpdate::CurrentModeUpdate(_cm) => {
            // CurrentModeUpdate carries a SessionModeId wrapper; store later when adapter needs it
            let _ = state.tinyvex.upsert_acp_state(thread_id, None, None, t);
        }
        agent_client_protocol::SessionUpdate::UserMessageChunk(ch) => {
            let txt = content_to_text(&ch.content);
            if !txt.is_empty() {
                // Upsert thread row and insert finalized message row
                let thr = tinyvex::ThreadRow {
                    id: thread_id.to_string(),
                    thread_id: Some(thread_id.to_string()),
                    title: "Thread".into(),
                    project_id: None,
                    resume_id: Some(thread_id.to_string()),
                    rollout_path: None,
                    source: Some(provider.to_string()),
                    created_at: t,
                    updated_at: t,
                    message_count: None,
                };
                let _ = state.tinyvex.upsert_thread(&thr);
                let row = json!({
                    "id": thr.id,
                    "thread_id": thr.thread_id,
                    "title": thr.title,
                    "project_id": thr.project_id,
                    "resume_id": thr.resume_id,
                    "rollout_path": thr.rollout_path,
                    "source": thr.source,
                    "created_at": thr.created_at,
                    "updated_at": thr.updated_at,
                    "message_count": thr.message_count,
                });
                let _ = state.tx.send(json!({"type":"tinyvex.update","stream":"threads","op":"upsert","threadId": thread_id, "updatedAt": t, "row": row}).to_string());
                let item_id = format!("acp:user:{}", t);
                let _ = state.tinyvex.upsert_streamed_message(thread_id, "message", Some("user"), &txt, &item_id, 0, t);
                let _ = state.tinyvex.finalize_streamed_message(thread_id, &item_id, &txt, t);
                let _ = state.tx.send(json!({"type":"tinyvex.update","stream":"messages","op":"insert","threadId": thread_id, "itemId": item_id}).to_string());
            }
        }
        agent_client_protocol::SessionUpdate::AgentMessageChunk(ch) => {
            let txt = content_to_text(&ch.content);
            if !txt.is_empty() {
                let thr = tinyvex::ThreadRow {
                    id: thread_id.to_string(),
                    thread_id: Some(thread_id.to_string()),
                    title: "Thread".into(),
                    project_id: None,
                    resume_id: Some(thread_id.to_string()),
                    rollout_path: None,
                    source: Some(provider.to_string()),
                    created_at: t,
                    updated_at: t,
                    message_count: None,
                };
                let _ = state.tinyvex.upsert_thread(&thr);
                let row = json!({
                    "id": thr.id,
                    "thread_id": thr.thread_id,
                    "title": thr.title,
                    "project_id": thr.project_id,
                    "resume_id": thr.resume_id,
                    "rollout_path": thr.rollout_path,
                    "source": thr.source,
                    "created_at": thr.created_at,
                    "updated_at": thr.updated_at,
                    "message_count": thr.message_count,
                });
                let _ = state.tx.send(json!({"type":"tinyvex.update","stream":"threads","op":"upsert","threadId": thread_id, "updatedAt": t, "row": row}).to_string());
                let item_id = format!("acp:assistant:{}", t);
                let _ = state.tinyvex.upsert_streamed_message(thread_id, "message", Some("assistant"), &txt, &item_id, 0, t);
                let _ = state.tinyvex.finalize_streamed_message(thread_id, &item_id, &txt, t);
                let _ = state.tx.send(json!({"type":"tinyvex.update","stream":"messages","op":"insert","threadId": thread_id, "itemId": item_id}).to_string());
            }
        }
        agent_client_protocol::SessionUpdate::AgentThoughtChunk(ch) => {
            let txt = content_to_text(&ch.content);
            if !txt.is_empty() {
                let thr = tinyvex::ThreadRow {
                    id: thread_id.to_string(),
                    thread_id: Some(thread_id.to_string()),
                    title: "Thread".into(),
                    project_id: None,
                    resume_id: Some(thread_id.to_string()),
                    rollout_path: None,
                    source: Some(provider.to_string()),
                    created_at: t,
                    updated_at: t,
                    message_count: None,
                };
                let _ = state.tinyvex.upsert_thread(&thr);
                let _ = state.tx.send(json!({"type":"tinyvex.update","stream":"threads","op":"upsert","threadId": thread_id, "updatedAt": t}).to_string());
                let item_id = format!("acp:reason:{}", t);
                let _ = state.tinyvex.upsert_streamed_message(thread_id, "reason", None, &txt, &item_id, 0, t);
                let _ = state.tinyvex.finalize_streamed_message(thread_id, &item_id, &txt, t);
                let _ = state.tx.send(json!({"type":"tinyvex.update","stream":"messages","op":"insert","threadId": thread_id, "itemId": item_id}).to_string());
            }
        }
    }
}

fn content_to_text(content: &acp::ContentBlock) -> String {
    match content {
        acp::ContentBlock::Text(acp::TextContent { text, .. }) => text.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::{broadcast, Mutex};

    #[tokio::test]
    async fn tinyvex_stream_upsert_broadcasts() {
        let (tx, mut rx) = broadcast::channel(8);
        let tvx = tinyvex::Tinyvex::open(tempfile::NamedTempFile::new().unwrap().path()).unwrap();
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
            tinyvex: std::sync::Arc::new(tvx),
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
        let tvx = tinyvex::Tinyvex::open(&db_path).unwrap();
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
            tinyvex: std::sync::Arc::new(tvx),
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
        let tvx = tinyvex::Tinyvex::open(&db_path).unwrap();
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
            tinyvex: std::sync::Arc::new(tvx),
        };
        // Simulate deltas then final.
        stream_upsert_or_append(&state, "th", "assistant", "hello").await;
        finalize_or_snapshot(&state, "th", "assistant", "hello").await;
        let rows = state.tinyvex.list_messages("th", 50).unwrap();
        assert_eq!(rows.len(), 1, "expected exactly one finalized row");
        assert_eq!(rows[0].partial, Some(0));
    }
}
