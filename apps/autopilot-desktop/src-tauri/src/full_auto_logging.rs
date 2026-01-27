use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::file_logger::event_log_dir;

const ENV_FULL_AUTO_LOG_DIR: &str = "OPENAGENTS_FULL_AUTO_LOG_DIR";
const ENV_TRACE_BUNDLE_DIR: &str = "OPENAGENTS_TRACE_BUNDLE_DIR";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullAutoRunMetadata {
    pub run_id: String,
    pub workspace_id: String,
    pub thread_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub decision_model: String,
    pub min_confidence: f32,
    pub max_turns: u64,
    pub no_progress_limit: u32,
    pub max_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullAutoRunEvent {
    pub timestamp: DateTime<Utc>,
    pub sequence_id: u64,
    pub event_type: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullAutoDecisionLog {
    pub timestamp: DateTime<Utc>,
    pub sequence_id: u64,
    pub run_id: String,
    pub workspace_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub action: String,
    pub confidence: f32,
    pub reason: String,
    pub next_input_preview: String,
    pub state: String,
    pub model: String,
    pub guardrail: Option<Value>,
    pub summary: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullAutoDecisionRawLog {
    pub timestamp: DateTime<Utc>,
    pub sequence_id: u64,
    pub run_id: String,
    pub workspace_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub raw_prediction: Value,
    pub parse_diagnostics: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullAutoLogPaths {
    pub app_server_log_dir: String,
    pub full_auto_log_dir: String,
    pub trace_bundle_dir: String,
}

pub fn full_auto_log_dir() -> PathBuf {
    if let Ok(path) = std::env::var(ENV_FULL_AUTO_LOG_DIR) {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".openagents")
        .join("autopilot-desktop")
        .join("logs")
        .join("full_auto")
}

pub fn trace_bundle_dir() -> PathBuf {
    if let Ok(path) = std::env::var(ENV_TRACE_BUNDLE_DIR) {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        return cwd.join("docs").join("logs").join(Utc::now().format("%Y%m%d").to_string());
    }
    full_auto_log_dir().join("trace_bundles")
}

pub fn decision_log_path() -> PathBuf {
    full_auto_log_dir().join("fullauto-decisions.jsonl")
}

pub fn raw_decision_log_path() -> PathBuf {
    full_auto_log_dir().join("fullauto-decisions-raw.jsonl")
}

pub fn run_metadata_dir() -> PathBuf {
    full_auto_log_dir().join("runs")
}

pub fn run_metadata_path(run_id: &str) -> PathBuf {
    run_metadata_dir().join(format!("{}.json", run_id))
}

pub fn run_event_log_path(run_id: &str) -> PathBuf {
    run_metadata_dir().join(format!("{}_events.jsonl", run_id))
}

pub fn new_run_id() -> String {
    format!("fullauto-{}", Uuid::new_v4())
}

pub fn write_run_metadata(record: &FullAutoRunMetadata) -> Result<()> {
    let path = run_metadata_path(&record.run_id);
    ensure_parent(&path)?;
    let payload = serde_json::to_string_pretty(record)?;
    std::fs::write(&path, payload)?;
    Ok(())
}

pub fn update_run_thread(run_id: &str, thread_id: &str) -> Result<()> {
    let path = run_metadata_path(run_id);
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let mut record: FullAutoRunMetadata = serde_json::from_str(&content)?;
    if record.thread_id.as_deref() != Some(thread_id) {
        record.thread_id = Some(thread_id.to_string());
        let payload = serde_json::to_string_pretty(&record)?;
        std::fs::write(&path, payload)?;
    }
    Ok(())
}

pub fn append_run_event(run_id: &str, event: &FullAutoRunEvent) -> Result<()> {
    let path = run_event_log_path(run_id);
    ensure_parent(&path)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    let line = serde_json::to_string(event)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

pub fn write_decision_log(record: &FullAutoDecisionLog) -> Result<()> {
    let path = decision_log_path();
    ensure_parent(&path)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    let line = serde_json::to_string(record)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

pub fn write_raw_decision_log(record: &FullAutoDecisionRawLog) -> Result<()> {
    let path = raw_decision_log_path();
    ensure_parent(&path)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    let line = serde_json::to_string(record)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

pub fn log_paths_snapshot() -> FullAutoLogPaths {
    let app_server_dir = event_log_dir().unwrap_or_else(|_| PathBuf::from(""));
    let bundle_dir = trace_bundle_dir();
    FullAutoLogPaths {
        app_server_log_dir: app_server_dir.to_string_lossy().to_string(),
        full_auto_log_dir: full_auto_log_dir().to_string_lossy().to_string(),
        trace_bundle_dir: bundle_dir.to_string_lossy().to_string(),
    }
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create log dir {}", parent.display()))?;
    }
    Ok(())
}
