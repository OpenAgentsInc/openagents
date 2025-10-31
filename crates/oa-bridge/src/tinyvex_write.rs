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
        };
        // Simulate deltas then final.
        stream_upsert_or_append(&state, "th", "assistant", "hello").await;
        finalize_or_snapshot(&state, "th", "assistant", "hello").await;
        let rows = state.tinyvex.list_messages("th", 50).unwrap();
        assert_eq!(rows.len(), 1, "expected exactly one finalized row");
        assert_eq!(rows[0].partial, Some(0));
    }
}
