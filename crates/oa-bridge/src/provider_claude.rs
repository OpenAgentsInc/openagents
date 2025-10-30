use std::sync::Arc;

use anyhow::Result;
use serde_json::json;
use tracing::info;

use crate::tinyvex_write::mirror_acp_update_to_tinyvex;
use crate::state::AppState;

/// Minimal in-repo Claude Code provider stub.
///
/// For now, synthesize a few representative Claude-style events and route them
/// through the translator, then mirror to Convex. Later, replace this with a
/// real in-repo adapter that talks to Claude Code.
#[allow(dead_code)]
pub async fn run_prompt(state: Arc<AppState>, thread_doc_id: &str, _cwd: Option<std::path::PathBuf>, text: &str) -> Result<()> {
    info!(thread_doc_id, len = text.len(), "provider.claude: run_prompt");
    // Remember target thread id for writes
    {
        *state.current_thread_doc.lock().await = Some(thread_doc_id.to_string());
    }
    // Synthetic minimal stream of Claude-like events
    let events = vec![
        json!({"type":"content_block_delta","delta": {"type":"thinking_delta","thinking":"Analyzing request..."}}),
        json!({"type":"content_block_delta","delta": {"type":"text_delta","text":"Hello! Claude Code path is wired."}}),
        json!({"type":"content_block_start","content_block": {"type":"tool_use","id":"tu_1","name":"bash","input":{"command":"echo hi"}}}),
        json!({"type":"tool_result","tool_use_id":"tu_1","is_error": false, "result": {"stdout":"hi\n"}}),
    ];
    for ev in events {
        if let Some(update) = acp_event_translator::translate_claude_event_to_acp_update(&ev) {
            mirror_acp_update_to_tinyvex(&state, thread_doc_id, &update).await;
            if std::env::var("BRIDGE_ACP_EMIT").ok().as_deref() == Some("1") {
                let line = serde_json::to_string(&json!({"type":"bridge.acp","notification":{"sessionId": thread_doc_id, "update": update}})).unwrap_or_default();
                let _ = state.tx.send(line);
            }
        }
    }
    Ok(())
}
