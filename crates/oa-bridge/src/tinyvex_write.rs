use crate::state::AppState;
use serde_json::json;
use tracing::warn;

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
    let stable_item_id = if kind == "assistant" { "stream:assistant" } else if kind == "reason" { "stream:reason" } else { kind };
    if entry.item_id.is_empty() { entry.item_id = stable_item_id.to_string(); }
    let item_id = entry.item_id.clone();
    drop(guard);
    let role = if kind == "assistant" { Some("assistant") } else { None };
    let out_kind = if kind == "assistant" { "message" } else if kind == "reason" { "reason" } else { kind };
    let t = now_ms();
    // Ensure a thread row exists/upserted for listings
    let thr = tinyvex::ThreadRow { id: thread_id.to_string(), thread_id: Some(thread_id.to_string()), title: "Thread".into(), project_id: None, resume_id: Some(thread_id.to_string()), rollout_path: None, source: Some("stream".into()), created_at: t, updated_at: t };
    let _ = state.tinyvex.upsert_thread(&thr);
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

pub async fn try_finalize_stream_kind(state: &AppState, thread_id: &str, kind: &str, final_text: &str) -> bool {
    let mut guard = state.stream_track.lock().await;
    let key = format!("{}|{}", thread_id, kind);
    let existed = guard.remove(&key).is_some();
    drop(guard);
    if !existed { return false; }
    let stable_item_id = if kind == "assistant" { "stream:assistant" } else if kind == "reason" { "stream:reason" } else { kind };
    let t = now_ms();
    if let Err(e) = state.tinyvex.finalize_streamed_message(thread_id, stable_item_id, final_text, t) {
        warn!(?e, "tinyvex finalize_streamed_message failed");
        return false;
    }
    let _ = state.tx.send(json!({
        "type": "bridge.tinyvex_write",
        "op": "finalizeStreamed",
        "threadId": thread_id,
        "kind": kind,
        "itemId": stable_item_id,
        "len": final_text.len(),
        "ok": true
    }).to_string());
    let _ = state.tx.send(json!({
        "type": "tinyvex.update",
        "stream": "messages",
        "op": "finalizeStreamed",
        "threadId": thread_id,
        "kind": kind,
        "itemId": stable_item_id
    }).to_string());
    true
}

pub async fn finalize_streaming_for_thread(state: &AppState, thread_id: &str) {
    let keys: Vec<(String, String)> = {
        let guard = state.stream_track.lock().await;
        guard
            .keys()
            .filter_map(|k| { let mut p = k.split('|'); let tid = p.next()?; let kind = p.next()?; if tid == thread_id { Some((tid.to_string(), kind.to_string())) } else { None } })
            .collect()
    };
    for (tid, k) in keys {
        let text = {
            let guard = state.stream_track.lock().await;
            let key = format!("{}|{}", tid, k);
            guard.get(&key).map(|e| e.last_text.clone()).unwrap_or_default()
        };
        let _ = try_finalize_stream_kind(state, thread_id, &k, &text).await;
    }
}

pub fn summarize_exec_delta_for_log(line: &str) -> Option<String> {
    if line.len() > 24 * 1024 { Some(format!("[jsonl delta ~{} bytes]", line.len())) } else { None }
}

pub async fn mirror_acp_update_to_convex(state: &AppState, thread_id: &str, update: &agent_client_protocol::SessionUpdate) {
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
        agent_client_protocol::SessionUpdate::UserMessageChunk(_) => {}
        agent_client_protocol::SessionUpdate::AgentMessageChunk(_) => {}
        agent_client_protocol::SessionUpdate::AgentThoughtChunk(_) => {}
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
            current_convex_thread: Mutex::new(None),
            stream_track: Mutex::new(std::collections::HashMap::new()),
            pending_user_text: Mutex::new(std::collections::HashMap::new()),
            convex_ready: std::sync::atomic::AtomicBool::new(true),
            tinyvex: std::sync::Arc::new(tvx),
        };
        stream_upsert_or_append(&state, "th", "assistant", "hello").await;
        let msg = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await.ok().and_then(Result::ok).unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(&msg).unwrap_or_default();
        assert_eq!(v.get("type").and_then(|x| x.as_str()), Some("bridge.tinyvex_write"));
        assert_eq!(v.get("op").and_then(|x| x.as_str()), Some("upsertStreamed"));
    }
}
