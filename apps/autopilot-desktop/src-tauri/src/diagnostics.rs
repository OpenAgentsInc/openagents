use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;

use crate::file_logger::event_log_dir;
use crate::full_auto::{decision_model, FullAutoConfig};
use crate::full_auto_logging::{
    decision_log_path, full_auto_log_dir, raw_decision_log_path, trace_bundle_dir,
    FullAutoDecisionLog, FullAutoLogPaths,
};

#[derive(Debug, Serialize)]
struct FullAutoDecisionSummary {
    decision_count: usize,
    last_action: Option<String>,
    last_reason: Option<String>,
    last_confidence: Option<f32>,
    last_run_id: Option<String>,
    average_confidence: Option<f32>,
}

#[derive(Debug, Serialize)]
struct FullAutoConfigSnapshot {
    decision_model: String,
    min_confidence: f32,
    max_turns: u64,
    no_progress_limit: u32,
    max_tokens: Option<u64>,
    log_paths: FullAutoLogPaths,
}

#[tauri::command]
pub async fn export_full_auto_trace_bundle() -> Result<String, String> {
    let bundle_dir = create_bundle_dir().map_err(|e| e.to_string())?;

    let event_dir = event_log_dir().map_err(|e| e.to_string())?;
    copy_latest_log(&event_dir, "app-server-events_", &bundle_dir);
    copy_latest_log(&event_dir, "acp-events_", &bundle_dir);

    copy_if_exists(&decision_log_path(), &bundle_dir);
    copy_if_exists(&raw_decision_log_path(), &bundle_dir);

    if let Ok(entries) = fs::read_dir(full_auto_log_dir().join("runs")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                copy_if_exists(&path, &bundle_dir);
            }
        }
    }

    let logs = read_decision_logs();
    let summary = summarize_decisions(&logs);
    write_json(&bundle_dir.join("fullauto-decision-summary.json"), &summary);
    write_markdown_summary(&bundle_dir.join("fullauto-decision-summary.md"), &summary);

    let config_snapshot = FullAutoConfigSnapshot {
        decision_model: decision_model(),
        min_confidence: FullAutoConfig::default().min_confidence,
        max_turns: FullAutoConfig::default().max_turns,
        no_progress_limit: FullAutoConfig::default().no_progress_limit,
        max_tokens: FullAutoConfig::default().max_tokens,
        log_paths: crate::full_auto_logging::log_paths_snapshot(),
    };
    write_json(&bundle_dir.join("fullauto-config-snapshot.json"), &config_snapshot);

    Ok(bundle_dir.to_string_lossy().to_string())
}

fn create_bundle_dir() -> anyhow::Result<PathBuf> {
    let base = trace_bundle_dir();
    fs::create_dir_all(&base)?;
    let stamp = Utc::now().format("%H%M%S").to_string();
    let bundle_dir = base.join(format!("fullauto-trace-{}", stamp));
    fs::create_dir_all(&bundle_dir)?;
    Ok(bundle_dir)
}

fn copy_if_exists(path: &Path, dest_dir: &Path) {
    if !path.exists() {
        return;
    }
    if let Some(filename) = path.file_name() {
        let _ = fs::copy(path, dest_dir.join(filename));
    }
}

fn copy_latest_log(dir: &Path, prefix: &str, dest_dir: &Path) {
    let mut latest: Option<PathBuf> = None;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with(prefix) {
                continue;
            }
            let pick = match &latest {
                Some(existing) => path.metadata().ok().and_then(|meta| {
                    existing.metadata().ok().map(|other| meta.modified().ok() > other.modified().ok())
                }) == Some(true),
                None => true,
            };
            if pick {
                latest = Some(path.clone());
            }
        }
    }
    if let Some(path) = latest {
        copy_if_exists(&path, dest_dir);
    }
}

fn read_decision_logs() -> Vec<FullAutoDecisionLog> {
    let path = decision_log_path();
    let content = fs::read_to_string(&path).unwrap_or_default();
    content
        .lines()
        .filter_map(|line| serde_json::from_str::<FullAutoDecisionLog>(line).ok())
        .collect()
}

fn summarize_decisions(logs: &[FullAutoDecisionLog]) -> FullAutoDecisionSummary {
    let decision_count = logs.len();
    let mut avg_confidence = None;
    if !logs.is_empty() {
        let total: f32 = logs.iter().map(|log| log.confidence).sum();
        avg_confidence = Some(total / logs.len() as f32);
    }
    let last = logs.last();
    FullAutoDecisionSummary {
        decision_count,
        last_action: last.map(|log| log.action.clone()),
        last_reason: last.map(|log| log.reason.clone()),
        last_confidence: last.map(|log| log.confidence),
        last_run_id: last.map(|log| log.run_id.clone()),
        average_confidence: avg_confidence,
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    if let Ok(payload) = serde_json::to_string_pretty(value) {
        let _ = fs::write(path, payload);
    }
}

fn write_markdown_summary(path: &Path, summary: &FullAutoDecisionSummary) {
    let content = format!(
        "# Full Auto Decision Summary\n\n- Decisions: {}\n- Last action: {}\n- Last confidence: {}\n- Last run id: {}\n- Avg confidence: {}\n",
        summary.decision_count,
        summary.last_action.clone().unwrap_or_else(|| "--".to_string()),
        summary.last_confidence
            .map(|v| format!("{:.2}", v))
            .unwrap_or_else(|| "--".to_string()),
        summary.last_run_id.clone().unwrap_or_else(|| "--".to_string()),
        summary.average_confidence
            .map(|v| format!("{:.2}", v))
            .unwrap_or_else(|| "--".to_string())
    );
    let _ = fs::write(path, content);
}
