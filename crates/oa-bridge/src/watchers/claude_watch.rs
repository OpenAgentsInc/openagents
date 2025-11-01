use std::{collections::HashMap, path::{Path, PathBuf}, time::Duration};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, warn};
use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct FileState {
    offset: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SyncStateFile {
    files: HashMap<String, FileState>,
}

fn sync_state_path() -> PathBuf {
    let base = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    let dir = base.join(".openagents/sync");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!(?e, path=%dir.display(), "failed to create sync state directory");
    }
    dir.join("claude-state.json")
}

fn load_state() -> SyncStateFile {
    let path = sync_state_path();
    if let Ok(data) = std::fs::read(&path) {
        if let Ok(v) = serde_json::from_slice::<SyncStateFile>(&data) { return v; }
    }
    SyncStateFile::default()
}

fn save_state(state: &SyncStateFile) {
    let path = sync_state_path();
    if let Ok(data) = serde_json::to_vec_pretty(state) {
        if let Err(e) = std::fs::write(&path, data) {
            warn!(?e, path=%path.display(), "failed to write claude sync state");
        }
    }
}

fn claude_projects_base() -> PathBuf {
    if let Ok(p) = std::env::var("CLAUDE_PROJECTS_DIR") {
        return PathBuf::from(p);
    }
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()));
    // Common locations observed in local Claude installs
    let candidates = [
        home.join(".claude").join("projects"),
        home.join(".claude").join("local").join("claude").join("projects"),
        home.join(".claude").join("local").join("projects"),
    ];
    for c in candidates {
        if c.exists() { return c; }
    }
    // Fallback: scan for any 'projects' dir under ~/.claude
    let root = home.join(".claude");
    if let Ok(rd) = std::fs::read_dir(&root) {
        let mut stack: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        while let Some(p) = stack.pop() {
            if p.is_dir() {
                if p.file_name().and_then(|s| s.to_str()) == Some("projects") {
                    return p;
                }
                if let Ok(inner) = std::fs::read_dir(&p) {
                    for e in inner.flatten() { stack.push(e.path()); }
                }
            }
        }
    }
    // Default fallback
    root.join("projects")
}

pub fn claude_projects_base_path() -> PathBuf { claude_projects_base() }

fn list_transcript_files(base: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for ent in rd.flatten() {
                let p = ent.path();
                if p.is_dir() { stack.push(p); continue; }
                if p.extension().and_then(|e| e.to_str()) == Some("jsonl") { out.push(p); }
            }
        }
    }
    out
}

fn session_id_from_filename(p: &Path) -> Option<String> {
    p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
}

fn map_claude_transcript_line_to_acp(v: &serde_json::Value) -> Vec<agent_client_protocol::SessionUpdate> {
    use agent_client_protocol::{SessionUpdate as SU, ContentBlock, ContentChunk, TextContent};
    let mut out = Vec::new();
    let ty = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
    if ty == "assistant" {
        if let Some(msg) = v.get("message") {
            // 1) text content → AgentMessageChunk
            if let Some(arr) = msg.get("content").and_then(|x| x.as_array()) {
                let mut text = String::new();
                for el in arr {
                    if el.get("type").and_then(|x| x.as_str()) == Some("text") {
                        if let Some(t) = el.get("text").and_then(|x| x.as_str()) {
                            if !text.is_empty() { text.push('\n'); }
                            text.push_str(t);
                        }
                    }
                }
                if !text.is_empty() {
                    out.push(SU::AgentMessageChunk(ContentChunk { content: ContentBlock::Text(TextContent { annotations: None, text, meta: None }), meta: None }));
                }
                // 2) tool_use blocks → synthesize content_block_start for translator
                for el in arr {
                    if el.get("type").and_then(|x| x.as_str()) == Some("tool_use") {
                        let stub = serde_json::json!({
                            "type": "content_block_start",
                            "content_block": el
                        });
                        // Translator expects id/name/input inside content_block
                        if let Some(update) = acp_event_translator::translate_claude_event_to_acp_update(&stub) {
                            out.push(update);
                        }
                    }
                }
            }
        }
    } else if ty == "user" {
        if let Some(msg) = v.get("message") {
            // a) Text blocks in content array → aggregate to a single user message
            if let Some(arr) = msg.get("content").and_then(|x| x.as_array()) {
                let mut text = String::new();
                for el in arr {
                    if el.get("type").and_then(|x| x.as_str()) == Some("text") {
                        if let Some(t) = el.get("text").and_then(|x| x.as_str()) {
                            if !text.is_empty() { text.push('\n'); }
                            text.push_str(t);
                        }
                    }
                }
                if !text.is_empty() {
                    out.push(SU::UserMessageChunk(ContentChunk { content: ContentBlock::Text(TextContent { annotations: None, text, meta: None }), meta: None }));
                }
                // b) tool_result objects within the array
                for el in arr {
                    if el.get("type").and_then(|x| x.as_str()) == Some("tool_result") {
                        let stub = serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": el.get("tool_use_id").cloned().unwrap_or(serde_json::Value::Null),
                            "content": el.get("content").cloned().unwrap_or(serde_json::Value::Null),
                        });
                        if let Some(update) = acp_event_translator::translate_claude_event_to_acp_update(&stub) {
                            out.push(update);
                        }
                    }
                }
            } else if let Some(s) = msg.get("content").and_then(|x| x.as_str()) {
                // c) Some transcripts may encode user content as a plain string
                if !s.is_empty() {
                    out.push(SU::UserMessageChunk(ContentChunk { content: ContentBlock::Text(TextContent { annotations: None, text: s.to_string(), meta: None }), meta: None }));
                }
            }
        }
    }
    out
}

async fn process_file_append(state: &AppState, file_path: &Path, st: &mut FileState) -> Result<()> {
    let meta = fs::metadata(file_path)?;
    let len = meta.len();
    if st.offset > len { st.offset = 0; st.session_id = None; }
    if st.offset == len { return Ok(()); }
    let mut file = fs::File::open(file_path)?;
    file.seek(SeekFrom::Start(st.offset))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut lines_processed = 0usize;
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 { break; }
        st.offset = reader.stream_position()?;
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed.is_empty() { continue; }
        lines_processed += 1;
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            // Learn session id
            if st.session_id.is_none() {
                if let Some(sid) = v.get("sessionId").and_then(|x| x.as_str()) {
                    st.session_id = Some(sid.to_string());
                } else if let Some(sid) = session_id_from_filename(file_path) {
                    st.session_id = Some(sid);
                }
            }
            let updates = map_claude_transcript_line_to_acp(&v);
            if updates.is_empty() { continue; }
            let tid = if let Some(s) = st.session_id.clone() { s } else { continue };
            for update in updates {
                // Use the non-streaming mirror so text is persisted even without deltas
                crate::tinyvex_write::mirror_acp_update_to_tinyvex(state, "claude_code", &tid, &update).await;
            }
        }
    }
    if lines_processed > 0 {
        let mut g = state.sync_last_read_ms.lock().await; *g = crate::util::now_ms();
    }
    Ok(())
}

pub fn spawn_claude_watcher(state: std::sync::Arc<AppState>) -> mpsc::Sender<crate::watchers::SyncCommand> {
    let (tx, mut rx) = mpsc::channel::<crate::watchers::SyncCommand>(16);
    let base = claude_projects_base();
    tokio::spawn(async move {
        info!(base=%base.display(), "claude watcher started");
        let mut state_file = load_state();
        // Read enabled flag directly from shared atomic to avoid stale copies
        loop {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    crate::watchers::SyncCommand::Enable(b) => { state.sync_enabled.store(b, std::sync::atomic::Ordering::Relaxed); info!(enabled=b, "claude watcher: enable toggled"); }
                    crate::watchers::SyncCommand::TwoWay(b) => { state.sync_two_way.store(b, std::sync::atomic::Ordering::Relaxed); info!(two_way=b, "claude watcher: two_way toggled"); }
                    crate::watchers::SyncCommand::FullRescan => { state_file.files.clear(); save_state(&state_file); info!("claude watcher: full rescan"); }
                }
            }
            let enabled = state.sync_enabled.load(std::sync::atomic::Ordering::Relaxed);
            if enabled && base.exists() {
                let files = list_transcript_files(&base);
                for p in files {
                    let key = p.to_string_lossy().to_string();
                    let fs_ent = state_file.files.entry(key.clone()).or_insert_with(FileState::default);
                    if fs_ent.session_id.is_none() {
                        if let Some(sid) = session_id_from_filename(&p) { fs_ent.session_id = Some(sid); }
                    }
                    if let Err(e) = process_file_append(&state, &p, fs_ent).await {
                        warn!(?e, path=%key, "claude watcher: process file append failed");
                    }
                }
                save_state(&state_file);
            }
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }
    });
    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn transcript_line_maps_tool_use_and_result() {
        let tool_use = json!({
            "type": "assistant",
            "message": {"content": [{"type":"tool_use","id":"tu_1","name":"Grep","input":{"pattern":"foo"}}] }
        });
        let out = map_claude_transcript_line_to_acp(&tool_use);
        assert!(out.iter().any(|u| matches!(u, agent_client_protocol::SessionUpdate::ToolCall(_))));

        let tool_res = json!({
            "type": "user",
            "message": {"content": [{"type":"tool_result","tool_use_id":"tu_1","content":"Found"}]}
        });
        let out2 = map_claude_transcript_line_to_acp(&tool_res);
        assert!(out2.iter().any(|u| matches!(u, agent_client_protocol::SessionUpdate::ToolCallUpdate(_))));
    }

    #[test]
    fn transcript_user_text_blocks_map_to_user_chunk() {
        let user_line = json!({
            "type": "user",
            "message": {"content": [
                {"type":"text","text":"Line one"},
                {"type":"text","text":"Line two"}
            ]}
        });
        let out = map_claude_transcript_line_to_acp(&user_line);
        assert!(out.iter().any(|u| matches!(u, agent_client_protocol::SessionUpdate::UserMessageChunk(_))));
        // Ensure the aggregated text contains both lines
        let mut found = false;
        for u in out {
            if let agent_client_protocol::SessionUpdate::UserMessageChunk(ch) = u {
                if let agent_client_protocol::ContentBlock::Text(t) = ch.content { assert!(t.text.contains("Line one") && t.text.contains("Line two")); found = true; }
            }
        }
        assert!(found);
    }
}
