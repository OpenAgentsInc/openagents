use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const PROJECT_OPS_PILOT_METRICS_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProjectOpsPilotMetricsDocumentV1 {
    schema_version: u16,
    command_counts: BTreeMap<String, u64>,
    view_counts: BTreeMap<String, u64>,
    projection_rebuild_count: u64,
    last_projection_rebuild_duration_ms: Option<u64>,
    last_checkpoint_seq: u64,
    last_cycle_summary: Option<String>,
}

pub struct ProjectOpsPilotMetricsState {
    pub command_counts: BTreeMap<String, u64>,
    pub view_counts: BTreeMap<String, u64>,
    pub projection_rebuild_count: u64,
    pub last_projection_rebuild_duration_ms: Option<u64>,
    pub last_checkpoint_seq: u64,
    pub last_cycle_summary: Option<String>,
    metrics_path: PathBuf,
    persist_enabled: bool,
}

impl ProjectOpsPilotMetricsState {
    pub fn disabled() -> Self {
        Self {
            command_counts: BTreeMap::new(),
            view_counts: BTreeMap::new(),
            projection_rebuild_count: 0,
            last_projection_rebuild_duration_ms: None,
            last_checkpoint_seq: 0,
            last_cycle_summary: None,
            metrics_path: PathBuf::new(),
            persist_enabled: false,
        }
    }

    pub fn load_or_new_default() -> Result<Self, String> {
        Self::load_or_new(default_metrics_path())
    }

    #[cfg(test)]
    pub(crate) fn from_metrics_path_for_tests(metrics_path: PathBuf) -> Result<Self, String> {
        Self::load_or_new(metrics_path)
    }

    fn load_or_new(metrics_path: PathBuf) -> Result<Self, String> {
        let mut state = Self {
            command_counts: BTreeMap::new(),
            view_counts: BTreeMap::new(),
            projection_rebuild_count: 0,
            last_projection_rebuild_duration_ms: None,
            last_checkpoint_seq: 0,
            last_cycle_summary: None,
            metrics_path,
            persist_enabled: true,
        };
        let Some(document) = load_metrics_document(state.metrics_path.as_path())? else {
            return Ok(state);
        };
        if document.schema_version != PROJECT_OPS_PILOT_METRICS_SCHEMA_VERSION {
            return Ok(state);
        }
        state.command_counts = document.command_counts;
        state.view_counts = document.view_counts;
        state.projection_rebuild_count = document.projection_rebuild_count;
        state.last_projection_rebuild_duration_ms = document.last_projection_rebuild_duration_ms;
        state.last_checkpoint_seq = document.last_checkpoint_seq;
        state.last_cycle_summary = document.last_cycle_summary;
        Ok(state)
    }

    pub fn record_command(&mut self, command_name: &str) -> Result<(), String> {
        let key = command_name.trim();
        if key.is_empty() {
            return Ok(());
        }
        *self.command_counts.entry(key.to_string()).or_insert(0) += 1;
        self.persist()
    }

    pub fn record_commands(&mut self, command_names: &[&str]) -> Result<(), String> {
        for command_name in command_names {
            let key = command_name.trim();
            if key.is_empty() {
                continue;
            }
            *self.command_counts.entry(key.to_string()).or_insert(0) += 1;
        }
        self.persist()
    }

    pub fn record_view(&mut self, view_id: &str) -> Result<(), String> {
        let key = view_id.trim();
        if key.is_empty() {
            return Ok(());
        }
        *self.view_counts.entry(key.to_string()).or_insert(0) += 1;
        self.persist()
    }

    pub fn record_projection_rebuild(
        &mut self,
        duration_ms: u64,
        checkpoint_seq: u64,
    ) -> Result<(), String> {
        self.projection_rebuild_count = self.projection_rebuild_count.saturating_add(1);
        self.last_projection_rebuild_duration_ms = Some(duration_ms);
        self.last_checkpoint_seq = checkpoint_seq;
        self.persist()
    }

    pub fn record_cycle_summary(&mut self, summary: impl Into<String>) -> Result<(), String> {
        self.last_cycle_summary = Some(summary.into());
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        if !self.persist_enabled {
            return Ok(());
        }
        if let Some(parent) = self.metrics_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create PM pilot metrics dir: {error}"))?;
        }
        let document = ProjectOpsPilotMetricsDocumentV1 {
            schema_version: PROJECT_OPS_PILOT_METRICS_SCHEMA_VERSION,
            command_counts: self.command_counts.clone(),
            view_counts: self.view_counts.clone(),
            projection_rebuild_count: self.projection_rebuild_count,
            last_projection_rebuild_duration_ms: self.last_projection_rebuild_duration_ms,
            last_checkpoint_seq: self.last_checkpoint_seq,
            last_cycle_summary: self.last_cycle_summary.clone(),
        };
        let payload = serde_json::to_string_pretty(&document)
            .map_err(|error| format!("Failed to encode PM pilot metrics: {error}"))?;
        let temp_path = self.metrics_path.with_extension("tmp");
        fs::write(temp_path.as_path(), payload)
            .map_err(|error| format!("Failed to write PM pilot metrics temp file: {error}"))?;
        fs::rename(temp_path.as_path(), self.metrics_path.as_path())
            .map_err(|error| format!("Failed to persist PM pilot metrics: {error}"))?;
        Ok(())
    }
}

fn default_metrics_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-pm-pilot-metrics-v1.json")
}

fn load_metrics_document(path: &Path) -> Result<Option<ProjectOpsPilotMetricsDocumentV1>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Failed to read PM pilot metrics: {error}")),
    };
    let document = serde_json::from_str::<ProjectOpsPilotMetricsDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse PM pilot metrics: {error}"))?;
    Ok(Some(document))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::ProjectOpsPilotMetricsState;

    static UNIQUE_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let counter = UNIQUE_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "openagents-project-ops-pilot-{name}-{nanos}-{counter}.json"
        ))
    }

    #[test]
    fn pilot_metrics_persist_and_reload() {
        let path = unique_temp_path("metrics");
        let mut metrics = ProjectOpsPilotMetricsState::from_metrics_path_for_tests(path.clone())
            .expect("metrics should initialize");
        metrics
            .record_projection_rebuild(14, 3)
            .expect("rebuild should record");
        metrics.record_view("my-work").expect("view should record");
        metrics
            .record_commands(&["CreateWorkItem", "EditWorkItemFields"])
            .expect("commands should record");
        metrics
            .record_cycle_summary("scripted Step 0 pilot cycle executed")
            .expect("cycle summary should record");

        let restored = ProjectOpsPilotMetricsState::from_metrics_path_for_tests(path)
            .expect("metrics should reload");
        assert_eq!(restored.projection_rebuild_count, 1);
        assert_eq!(restored.last_projection_rebuild_duration_ms, Some(14));
        assert_eq!(restored.last_checkpoint_seq, 3);
        assert_eq!(restored.view_counts.get("my-work"), Some(&1));
        assert_eq!(restored.command_counts.get("CreateWorkItem"), Some(&1));
        assert_eq!(restored.command_counts.get("EditWorkItemFields"), Some(&1));
        assert_eq!(
            restored.last_cycle_summary.as_deref(),
            Some("scripted Step 0 pilot cycle executed")
        );
    }
}
