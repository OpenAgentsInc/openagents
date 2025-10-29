use crate::state::AppState;
use serde_json::json;
use std::sync::atomic::Ordering;
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
    // MVP: rely on JSONL path for streaming text and skip ACP mirror to DB.
    let _ = state.tx.send(json!({ "type": "bridge.tinyvex_noop", "op": "acp.mirror", "threadId": thread_id }).to_string());
    let _ = update; // suppress unused
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
                bootstrap: false,
                convex_bin: None,
                convex_port: 0,
                convex_db: None,
                convex_interface: "127.0.0.1".into(),
                manage_convex: false,
                ws_token: Some("t".into()),
                claude_bin: None,
                claude_args: None,
            },
            last_thread_id: Mutex::new(None),
            history: Mutex::new(Vec::new()),
            current_convex_thread: Mutex::new(None),
            stream_track: Mutex::new(std::collections::HashMap::new()),
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

