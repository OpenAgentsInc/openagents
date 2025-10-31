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
    thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SyncStateFile {
    files: HashMap<String, FileState>,
}

#[derive(Debug)]
pub enum SyncCommand {
    Enable(bool),
    TwoWay(bool),
    FullRescan,
}

fn default_codex_base() -> PathBuf {
    if let Ok(p) = std::env::var("CODEXD_HISTORY_DIR") { return PathBuf::from(p); }
    let base = match std::env::var("HOME") {
        Ok(home) => PathBuf::from(home),
        Err(_) => {
            let tmp = std::env::temp_dir();
            warn!(path=%tmp.display(), "HOME not set; using temp dir for codex sessions base");
            tmp
        }
    };
    base.join(".codex/sessions")
}

fn sync_state_path() -> PathBuf {
    let base = match std::env::var("HOME") {
        Ok(home) => PathBuf::from(home),
        Err(_) => {
            let tmp = std::env::temp_dir();
            warn!(path=%tmp.display(), "HOME not set; using temp dir for sync state");
            tmp
        }
    };
    let dir = base.join(".openagents/sync");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!(?e, path=%dir.display(), "failed to create sync state directory");
    }
    dir.join("state.json")
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
            warn!(?e, path=%path.display(), "failed to write sync state");
        }
    }
}

async fn process_file_append(state: &AppState, file_path: &Path, st: &mut FileState) -> Result<()> {
    let meta = fs::metadata(file_path)?;
    let len = meta.len();
    // If the file shrank (rotation/truncate), reset offset and cached thread id
    if st.offset > len {
        st.offset = 0;
        st.thread_id = None;
    }
    // Nothing new to read
    if st.offset == len { return Ok(()); }
    // Open and seek to last processed offset
    let mut file = fs::File::open(file_path)?;
    file.seek(SeekFrom::Start(st.offset))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut lines_processed: usize = 0;
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 { break; }
        // Update offset to current file position
        st.offset = reader.stream_position()?;
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed.is_empty() { continue; }
        lines_processed += 1;
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            // Extract thread id from top-level or nested msg
            let tid = v.get("thread_id").and_then(|x| x.as_str())
                .or_else(|| v.get("msg").and_then(|m| m.get("thread_id")).and_then(|x| x.as_str()))
                .map(|s| s.to_string())
                .or_else(|| st.thread_id.clone());
            if let Some(tid_s) = tid.clone() { st.thread_id = Some(tid_s); }
            if let Some(update) = acp_event_translator::translate_codex_event_to_acp_update(&v) {
                if let Some(id) = st.thread_id.clone() {
                    // Mirror into Tinyvex
                    crate::tinyvex_write::mirror_acp_update_to_tinyvex(state, "codex", &id, &update).await;
                }
            }
        }
    }
    // Batch the last-read timestamp update to once per file
    if lines_processed > 0 {
        let mut g = state.sync_last_read_ms.lock().await;
        *g = crate::util::now_ms();
    }
    Ok(())
}

fn file_is_new_format(p: &Path) -> bool {
    if let Ok(f) = fs::File::open(p) {
        let r = BufReader::new(f);
        for line in r.lines().flatten().take(50) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if v.get("type").and_then(|x| x.as_str()).is_some() { return true; }
            }
        }
    }
    false
}

fn list_jsonl_files(base: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for ent in rd.flatten() {
                let p = ent.path();
                if p.is_dir() { stack.push(p); continue; }
                if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    if file_is_new_format(&p) { out.push(p); }
                }
            }
        }
    }
    out
}

pub fn spawn_codex_watcher(state: std::sync::Arc<AppState>) -> mpsc::Sender<SyncCommand> {
    let (tx, mut rx) = mpsc::channel::<SyncCommand>(16);
    let base = default_codex_base();
    tokio::spawn(async move {
        info!(base=%base.display(), "codex watcher started");
        let mut state_file = load_state();
        let mut enabled = state.sync_enabled.load(std::sync::atomic::Ordering::Relaxed);
        loop {
            // Handle control commands
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    SyncCommand::Enable(b) => { enabled = b; state.sync_enabled.store(b, std::sync::atomic::Ordering::Relaxed); }
                    SyncCommand::TwoWay(b) => { state.sync_two_way.store(b, std::sync::atomic::Ordering::Relaxed); }
                    SyncCommand::FullRescan => { state_file.files.clear(); save_state(&state_file); }
                }
            }
            if enabled && base.exists() {
                let files = list_jsonl_files(&base);
                for p in files {
                    let key = p.to_string_lossy().to_string();
                    let fs_ent = state_file.files.entry(key.clone()).or_insert_with(FileState::default);
                    if let Err(e) = process_file_append(&state, &p, fs_ent).await {
                        warn!(?e, path=%key, "codex watcher: process file append failed");
                    }
                }
                // Persist offsets after a pass
                save_state(&state_file);
            }
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }
    });
    tx
}

pub fn codex_base_path() -> PathBuf { default_codex_base() }
