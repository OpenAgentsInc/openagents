//! Convex write helpers for mapping JSONL events into mutations.
//!
//! This module centralizes the logic for translating Codex JSONL events and
//! streaming deltas into Convex mutations. Keeping this separate from the
//! WebSocket and child-process code makes the mapping testable and easier to
//! evolve independently.

use tracing::{info, warn};

use crate::state::AppState;
use std::sync::atomic::Ordering;
use serde_json::json;
use agent_client_protocol as acp;

/// Convert Convex FunctionResult to JSON for logging or test inspection.
#[allow(dead_code)]
pub fn convex_result_to_json(res: convex::FunctionResult) -> serde_json::Value {
    match res {
        convex::FunctionResult::Value(v) => serde_json::Value::from(v),
        convex::FunctionResult::ErrorMessage(msg) => serde_json::json!({ "$error": msg }),
        convex::FunctionResult::ConvexError(err) => {
            serde_json::json!({ "$error": err.message, "$data": serde_json::Value::from(err.data) })
        }
    }
}

/// Upsert or append a streaming partial into Convex.
///
/// The assistant and reasoning items are updated token-by-token. We buffer
/// last state in AppState::stream_track to reduce writes and attach a seq.
pub async fn stream_upsert_or_append(
    state: &AppState,
    thread_id: &str,
    kind: &str,
    full_text: &str,
) {
    if !state.convex_ready.load(Ordering::Relaxed) {
        let _ = state.tx.send(serde_json::json!({
            "type":"bridge.convex_write","op":"upsertStreamed","threadId":thread_id,
            "kind":kind,"ok":false,"error":"convex not ready"
        }).to_string());
        return;
    }
    use convex::{ConvexClient, Value};
    use std::collections::BTreeMap;
    let mut guard = state.stream_track.lock().await;
    let key = format!("{}|{}", thread_id, kind);
    let entry = guard
        .entry(key.clone())
        .or_insert_with(|| crate::state::StreamEntry {
            item_id: String::new(),
            last_text: String::new(),
            seq: 0,
        });
    entry.seq = entry.seq.saturating_add(1);
    entry.last_text = full_text.to_string();
    let seq_now = entry.seq;
    // Use a stable per-kind item id so Convex can upsert/finalize reliably
    let stable_item_id = if kind == "assistant" {
        "stream:assistant"
    } else if kind == "reason" {
        "stream:reason"
    } else {
        kind
    };
    if entry.item_id.is_empty() {
        entry.item_id = stable_item_id.to_string();
    }
    let item_id = entry.item_id.clone();
    drop(guard);
    let url = format!("http://127.0.0.1:{}", state.opts.convex_port);
    if let Ok(mut client) = ConvexClient::new(&url).await {
        let mut args: BTreeMap<String, Value> = BTreeMap::new();
        args.insert("threadId".into(), Value::from(thread_id.to_string()));
        // Map bridge kinds to canonical Convex kind/role
        // - assistant → kind: "message", role: "assistant"
        // - reason    → kind: "reason"
        let (convex_kind, role_val) = if kind == "assistant" {
            ("message", Some("assistant"))
        } else if kind == "reason" {
            ("reason", None)
        } else {
            (kind, None)
        };
        args.insert("kind".into(), Value::from(convex_kind));
        if let Some(r) = role_val { args.insert("role".into(), Value::from(r)); }
        args.insert("text".into(), Value::from(full_text));
        args.insert("seq".into(), Value::from(seq_now as i64));
        args.insert("itemId".into(), Value::from(item_id.clone()));
        match client.mutation("messages:upsertStreamed", args).await {
            Ok(_) => {
                // Broadcast a concise debug event for tails (tricoder)
                let _ = state.tx.send(serde_json::json!({
                    "type": "bridge.convex_write",
                    "op": "upsertStreamed",
                    "threadId": thread_id,
                    "kind": convex_kind,
                    "itemId": item_id,
                    "len": full_text.len(),
                    "ok": true
                }).to_string());
            }
            Err(e) => {
                warn!(?e, thread_id, kind, "convex upsertStreamed failed");
                let _ = state.tx.send(serde_json::json!({
                    "type": "bridge.convex_write",
                    "op": "upsertStreamed",
                    "threadId": thread_id,
                    "kind": convex_kind,
                    "itemId": item_id,
                    "len": full_text.len(),
                    "ok": false,
                    "error": format!("{}", e)
                }).to_string());
            }
        }
    }
}

/// Try to finalize a streamed item for a given (thread, kind). If no prior
/// streaming entry exists, returns false so the caller can create a snapshot
/// message instead.
pub async fn try_finalize_stream_kind(
    state: &AppState,
    thread_id: &str,
    kind: &str,
    final_text: &str,
) -> bool {
    if !state.convex_ready.load(Ordering::Relaxed) {
        let _ = state.tx.send(serde_json::json!({
            "type":"bridge.convex_write","op":"finalizeStreamed","threadId":thread_id,
            "kind":kind,"ok":false,"error":"convex not ready"
        }).to_string());
        return false;
    }
    use convex::{ConvexClient, Value};
    use std::collections::BTreeMap;
    let mut guard = state.stream_track.lock().await;
    let key = format!("{}|{}", thread_id, kind);
    let existed = guard.remove(&key).is_some();
    drop(guard);
    if !existed {
        return false;
    }
    let url = format!("http://127.0.0.1:{}", state.opts.convex_port);
    if let Ok(mut client) = ConvexClient::new(&url).await {
        let mut args: BTreeMap<String, Value> = BTreeMap::new();
        args.insert("threadId".into(), Value::from(thread_id.to_string()));
        // Finalize uses (threadId, itemId) to locate the streamed row
        let stable_item_id = if kind == "assistant" { "stream:assistant" } else if kind == "reason" { "stream:reason" } else { kind };
        args.insert("itemId".into(), Value::from(stable_item_id));
        args.insert("text".into(), Value::from(final_text));
        match client.mutation("messages:finalizeStreamed", args).await {
            Ok(_) => {
                info!(thread_id, kind, "convex finalizeStreamed ok");
                let _ = state.tx.send(serde_json::json!({
                    "type": "bridge.convex_write",
                    "op": "finalizeStreamed",
                    "threadId": thread_id,
                    "kind": kind,
                    "itemId": stable_item_id,
                    "len": final_text.len(),
                    "ok": true
                }).to_string());
            }
            Err(e) => {
                warn!(?e, thread_id, kind, "convex finalizeStreamed failed");
                let _ = state.tx.send(serde_json::json!({
                    "type": "bridge.convex_write",
                    "op": "finalizeStreamed",
                    "threadId": thread_id,
                    "kind": kind,
                    "itemId": stable_item_id,
                    "len": final_text.len(),
                    "ok": false,
                    "error": format!("{}", e)
                }).to_string());
            }
        }
    }
    true
}

/// Finalize any in-flight streamed kinds for a thread.
pub async fn finalize_streaming_for_thread(state: &AppState, thread_id: &str) {
    let keys: Vec<(String, String)> = {
        let guard = state.stream_track.lock().await;
        guard
            .keys()
            .filter_map(|k| {
                let mut parts = k.split('|');
                let tid = parts.next()?;
                let kind = parts.next()?;
                if tid == thread_id {
                    Some((tid.to_string(), kind.to_string()))
                } else {
                    None
                }
            })
            .collect()
    };
    for (tid, k) in keys {
        // attempt finalize with the last cached text
        let text = {
            let guard = state.stream_track.lock().await;
            let key = format!("{}|{}", tid, k);
            guard
                .get(&key)
                .map(|e| e.last_text.clone())
                .unwrap_or_default()
        };
        let _ = try_finalize_stream_kind(state, thread_id, &k, &text).await;
    }
}

/// Keep logs readable by summarizing large deltas.
pub fn summarize_exec_delta_for_log(line: &str) -> Option<String> {
    if line.len() > 24 * 1024 {
        Some(format!("[jsonl delta ~{} bytes]", line.len()))
    } else {
        None
    }
}

// tests live at bottom of file below mirror_acp_update_to_convex

/// Mirror ACP SessionUpdate into Convex rows. This complements the legacy JSONL→Convex
/// mapping so the app can subscribe to Convex as the single source of truth.
pub async fn mirror_acp_update_to_convex(
    state: &AppState,
    thread_id: &str,
    update: &acp::SessionUpdate,
) {
    let noop = std::env::var("OPENAGENTS_CONVEX_NOOP").ok().as_deref() == Some("1");
    if !state.convex_ready.load(Ordering::Relaxed) && !noop {
        let _ = state.tx.send(json!({
            "type":"bridge.convex_write","op":"acp.mirror","ok":false,
            "threadId":thread_id, "reason":"convex not ready"
        }).to_string());
        return;
    }
    if noop {
        // Emit a concise debug note indicating what would have been written.
        let (target, kind) = match update {
            acp::SessionUpdate::UserMessageChunk(_) => ("messages", "user"),
            acp::SessionUpdate::AgentMessageChunk(_) => ("messages", "assistant"),
            acp::SessionUpdate::AgentThoughtChunk(_) => ("messages", "reason"),
            acp::SessionUpdate::ToolCall(_) => ("acp_tool_calls", "tool"),
            acp::SessionUpdate::Plan(_) => ("acp_plan", "plan"),
            acp::SessionUpdate::AvailableCommandsUpdate(_) => ("acp_state", "available_commands"),
            acp::SessionUpdate::CurrentModeUpdate(_) => ("acp_state", "current_mode"),
            acp::SessionUpdate::ToolCallUpdate(_) => ("acp_tool_calls", "tool_update"),
        };
        let _ = state.tx.send(json!({
            "type": "bridge.convex_noop",
            "threadId": thread_id,
            "target": target,
            "kind": kind
        }).to_string());
        return;
    }
    use convex::{ConvexClient, Value};
    use std::collections::BTreeMap;
    let url = format!("http://127.0.0.1:{}", state.opts.convex_port);
    let Ok(mut client) = ConvexClient::new(&url).await else { return };

    // Helpers to build Convex Value objects/arrays
    fn v_str<S: Into<String>>(s: S) -> convex::Value { convex::Value::from(s.into()) }
    fn v_num(n: i64) -> convex::Value { convex::Value::from(n) }
    fn v_arr(items: Vec<convex::Value>) -> convex::Value { convex::Value::from(items) }

    match update {
        acp::SessionUpdate::UserMessageChunk(_) => {
            // Usually written by the app via runs:enqueue; skip to avoid duplicate rows.
            let _ = state.tx.send(json!({
                "type":"bridge.convex_write","op":"skip.user_message_chunk","threadId":thread_id
            }).to_string());
        }
        acp::SessionUpdate::AgentMessageChunk(_) => {
            // Stream assistant text (JSONL path already handles this; skip to avoid duplication)
            let _ = state.tx.send(json!({
                "type":"bridge.convex_write","op":"skip.agent_message_chunk","threadId":thread_id
            }).to_string());
        }
        acp::SessionUpdate::AgentThoughtChunk(_) => {
            // Stream reasoning text (JSONL path already handles this; skip to avoid duplication)
            let _ = state.tx.send(json!({
                "type":"bridge.convex_write","op":"skip.agent_thought_chunk","threadId":thread_id
            }).to_string());
        }
        acp::SessionUpdate::ToolCall(tc) => {
            // Upsert into dedicated ACP tool_calls table with typed arrays
            let tc_id = format!("{:?}", tc.id);
            let mut args: BTreeMap<String, Value> = BTreeMap::new();
            args.insert("threadId".into(), v_str(thread_id));
            args.insert("toolCallId".into(), v_str(tc_id));
            args.insert("title".into(), v_str(tc.title.clone()));
            args.insert("kind".into(), v_str(format!("{:?}", tc.kind)));
            args.insert("status".into(), v_str(format!("{:?}", tc.status)));
            // Prefer typed flattened vectors over JSON strings to fit convex::Value limitations
            let text_chunks: Vec<String> = tc
                .content
                .iter()
                .filter_map(|c| match c {
                    acp::ToolCallContent::Content { content } => match content {
                        acp::ContentBlock::Text(t) => Some(t.text.clone()),
                        _ => None,
                    },
                    _ => None,
                })
                .collect();
            if !text_chunks.is_empty() {
                args.insert("content_texts".into(), v_arr(text_chunks.into_iter().map(v_str).collect()));
            }
            // Flatten diffs if present
            let mut diff_paths: Vec<convex::Value> = Vec::new();
            let mut diff_new: Vec<convex::Value> = Vec::new();
            let mut diff_old: Vec<convex::Value> = Vec::new();
            for c in &tc.content {
                if let acp::ToolCallContent::Diff { diff } = c {
                    diff_paths.push(v_str(diff.path.to_string_lossy().to_string()));
                    diff_new.push(v_str(diff.new_text.clone()));
                    match &diff.old_text {
                        Some(s) => diff_old.push(v_str(s.clone())),
                        None => diff_old.push(convex::Value::from(Option::<String>::None)),
                    }
                }
            }
            if !diff_paths.is_empty() {
                args.insert("content_diff_paths".into(), v_arr(diff_paths));
                args.insert("content_diff_new_texts".into(), v_arr(diff_new));
                args.insert("content_diff_old_texts".into(), v_arr(diff_old));
            }
            // Flatten terminal ids
            let term_ids: Vec<convex::Value> = tc
                .content
                .iter()
                .filter_map(|c| match c { acp::ToolCallContent::Terminal { terminal_id } => Some(v_str(terminal_id.to_string())), _ => None })
                .collect();
            if !term_ids.is_empty() {
                args.insert("content_terminal_ids".into(), v_arr(term_ids));
            }
            if !tc.locations.is_empty() {
                let paths: Vec<String> = tc
                    .locations
                    .iter()
                    .map(|l| format!("{}", l.path.display()))
                    .collect();
                let lines: Vec<convex::Value> = tc
                    .locations
                    .iter()
                    .map(|l| match l.line { Some(n) => v_num(n as i64), None => convex::Value::from(Option::<i64>::None) })
                    .collect();
                args.insert("locations_paths".into(), v_arr(paths.into_iter().map(v_str).collect()));
                args.insert("locations_lines".into(), v_arr(lines));
            }
            let _ = client.mutation("acp_tool_calls:upsert", args).await;
        }
        acp::SessionUpdate::Plan(p) => {
            let mut args: BTreeMap<String, Value> = BTreeMap::new();
            args.insert("threadId".into(), v_str(thread_id));
            if !p.entries.is_empty() {
                let ec: Vec<convex::Value> = p.entries.iter().map(|e| v_str(e.content.clone())).collect();
                let ep: Vec<convex::Value> = p
                    .entries
                    .iter()
                    .map(|e| v_str(format!("{:?}", e.priority).to_lowercase()))
                    .collect();
                let es: Vec<convex::Value> = p
                    .entries
                    .iter()
                    .map(|e| {
                        let s: &str = match e.status {
                            acp::PlanEntryStatus::Pending => "pending",
                            acp::PlanEntryStatus::InProgress => "in_progress",
                            acp::PlanEntryStatus::Completed => "completed",
                        };
                        v_str(s)
                    })
                    .collect();
                args.insert("entries_content".into(), v_arr(ec));
                args.insert("entries_priority".into(), v_arr(ep));
                args.insert("entries_status".into(), v_arr(es));
            }
            let _ = client.mutation("acp_plan:set", args).await;
        }
        acp::SessionUpdate::AvailableCommandsUpdate(ac) => {
            let mut args: BTreeMap<String, Value> = BTreeMap::new();
            args.insert("threadId".into(), v_str(thread_id));
            if !ac.available_commands.is_empty() {
                let names: Vec<convex::Value> = ac.available_commands.iter().map(|c| v_str(c.name.clone())).collect();
                let descs: Vec<convex::Value> = ac
                    .available_commands
                    .iter()
                    .map(|c| v_str(c.description.clone()))
                    .collect();
                args.insert("available_command_names".into(), v_arr(names));
                args.insert("available_command_descriptions".into(), v_arr(descs));
            }
            let _ = client.mutation("acp_state:update", args).await;
        }
        acp::SessionUpdate::CurrentModeUpdate(cm) => {
            let mut args: BTreeMap<String, Value> = BTreeMap::new();
            args.insert("threadId".into(), v_str(thread_id));
            args.insert("currentModeId".into(), v_str(format!("{}", cm.current_mode_id)));
            let _ = client.mutation("acp_state:update", args).await;
        }
        acp::SessionUpdate::ToolCallUpdate(_) => {
            // TODO: refine mapping if distinct from ToolCall events in our translator
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compacts_large_delta_lines() {
        let long = "{".to_string() + &"x".repeat(30_000);
        assert!(summarize_exec_delta_for_log(&long).is_some());
        let small = "{\"type\":\"ok\"}";
        assert!(summarize_exec_delta_for_log(small).is_none());
    }

    #[test]
    fn maps_function_result() {
        let v = convex_result_to_json(convex::FunctionResult::Value(convex::Value::from(5_i64)));
        assert!(!v.is_null());
        let e = convex_result_to_json(convex::FunctionResult::ErrorMessage("bad".into()));
        assert_eq!(e.get("$error").and_then(|x| x.as_str()), Some("bad"));
    }

    #[tokio::test]
    async fn stream_upsert_emits_debug_when_convex_not_ready() {
        use tokio::sync::{broadcast, Mutex};
        let (tx, mut rx) = broadcast::channel(8);
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
                
            },
            last_thread_id: Mutex::new(None),
            history: Mutex::new(Vec::new()),
            current_convex_thread: Mutex::new(None),
            stream_track: Mutex::new(std::collections::HashMap::new()),
            convex_ready: std::sync::atomic::AtomicBool::new(false),
        };
        // Call with convex_ready=false triggers a debug write and returns
        stream_upsert_or_append(&state, "th", "assistant", "hello").await;
        // Receive at least one line
        let msg = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(&msg).unwrap_or_default();
        assert_eq!(v.get("type").and_then(|x| x.as_str()), Some("bridge.convex_write"));
        assert_eq!(v.get("op").and_then(|x| x.as_str()), Some("upsertStreamed"));
        assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));
    }
}
