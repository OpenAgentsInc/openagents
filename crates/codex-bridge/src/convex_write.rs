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

/// Mirror ACP SessionUpdate into Convex rows. This complements the legacy JSONL→Convex
/// mapping so the app can subscribe to Convex as the single source of truth.
pub async fn mirror_acp_update_to_convex(
    state: &AppState,
    thread_id: &str,
    update: &acp::SessionUpdate,
) {
    if !state.convex_ready.load(Ordering::Relaxed) {
        let _ = state.tx.send(json!({
            "type":"bridge.convex_write","op":"acp.mirror","ok":false,
            "threadId":thread_id, "reason":"convex not ready"
        }).to_string());
        return;
    }
    use convex::{ConvexClient, Value};
    use std::collections::BTreeMap;
    let url = format!("http://127.0.0.1:{}", state.opts.convex_port);
    let Ok(mut client) = ConvexClient::new(&url).await else { return };

    match update {
        acp::SessionUpdate::UserMessageChunk(u) => {
            // Usually written by the app via runs:enqueue; skip to avoid duplicate rows.
            let _ = state.tx.send(json!({
                "type":"bridge.convex_write","op":"skip.user_message_chunk","threadId":thread_id
            }).to_string());
        }
        acp::SessionUpdate::AgentMessageChunk(a) => {
            // Stream assistant text (JSONL path already handles this; skip to avoid duplication)
            let _ = state.tx.send(json!({
                "type":"bridge.convex_write","op":"skip.agent_message_chunk","threadId":thread_id
            }).to_string());
        }
        acp::SessionUpdate::AgentThoughtChunk(t) => {
            // Stream reasoning text (JSONL path already handles this; skip to avoid duplication)
            let _ = state.tx.send(json!({
                "type":"bridge.convex_write","op":"skip.agent_thought_chunk","threadId":thread_id
            }).to_string());
        }
        acp::SessionUpdate::ToolCall(tc) => {
            let mut args: BTreeMap<String, Value> = BTreeMap::new();
            args.insert("threadId".into(), Value::from(thread_id.to_string()));
            let id_str = format!("{:?}", tc.id);
            args.insert("itemId".into(), Value::from(id_str));
            args.insert("kind".into(), Value::from("tool"));
            // Store full structured payload as JSON string for portability
            let json_str = serde_json::to_string(tc).unwrap_or_else(|_| String::from("{}"));
            args.insert("text".into(), Value::from(json_str));
            let _ = client.mutation("messages:upsertStreamed", args).await;
        }
        acp::SessionUpdate::Plan(p) => {
            let mut args: BTreeMap<String, Value> = BTreeMap::new();
            args.insert("threadId".into(), Value::from(thread_id.to_string()));
            args.insert("itemId".into(), Value::from("state:plan"));
            args.insert("kind".into(), Value::from("plan"));
            let json_str = serde_json::to_string(&p).unwrap_or_else(|_| String::from("{}"));
            args.insert("text".into(), Value::from(json_str));
            let _ = client.mutation("messages:upsertStreamed", args).await;
        }
        acp::SessionUpdate::AvailableCommandsUpdate(ac) => {
            let mut args: BTreeMap<String, Value> = BTreeMap::new();
            args.insert("threadId".into(), Value::from(thread_id.to_string()));
            args.insert("itemId".into(), Value::from("state:available_commands"));
            args.insert("kind".into(), Value::from("available_commands_update"));
            let json_str = serde_json::to_string(&ac).unwrap_or_else(|_| String::from("{}"));
            args.insert("text".into(), Value::from(json_str));
            let _ = client.mutation("messages:upsertStreamed", args).await;
        }
        acp::SessionUpdate::CurrentModeUpdate(cm) => {
            let mut args: BTreeMap<String, Value> = BTreeMap::new();
            args.insert("threadId".into(), Value::from(thread_id.to_string()));
            args.insert("itemId".into(), Value::from("state:current_mode"));
            args.insert("kind".into(), Value::from("current_mode_update"));
            let json_str = serde_json::to_string(&cm).unwrap_or_else(|_| String::from("{}"));
            args.insert("text".into(), Value::from(json_str));
            let _ = client.mutation("messages:upsertStreamed", args).await;
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
}
