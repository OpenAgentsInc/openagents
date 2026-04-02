use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use chrono;

pub const PROJECT_OPS_PILOT_METRICS_SCHEMA_VERSION: u16 = 2;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProjectOpsPilotMetricsDocumentV2 {
    schema_version: u16,
    command_counts: BTreeMap<String, u64>,
    view_counts: BTreeMap<String, u64>,
    projection_rebuild_count: u64,
    last_projection_rebuild_duration_ms: Option<u64>,
    last_checkpoint_seq: u64,
    last_cycle_summary: Option<String>,
    promotion_ledger: HashMap<String, PromotionLedgerEntry>,
    shadow_rollout_state: HashMap<String, ShadowRolloutState>,
    rollback_history: Vec<RollbackEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PromotionLedgerEntry {
    admitted_improvement_id: String,
    promotion_date: String,
    revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ShadowRolloutState {
    admitted_improvement_id: String,
    rollout_date: String,
    state: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RollbackEntry {
    admitted_improvement_id: String,
    rollback_date: String,
    reason: String,
}

pub struct ProjectOpsPilotMetricsState {
    pub command_counts: BTreeMap<String, u64>,
    pub view_counts: BTreeMap<String, u64>,
    pub projection_rebuild_count: u64,
    pub last_projection_rebuild_duration_ms: Option<u64>,
    pub last_checkpoint_seq: u64,
    pub last_cycle_summary: Option<String>,
    pub promotion_ledger: HashMap<String, PromotionLedgerEntry>,
    pub shadow_rollout_state: HashMap<String, ShadowRolloutState>,
    pub rollback_history: Vec<RollbackEntry>,
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
            promotion_ledger: HashMap::new(),
            shadow_rollout_state: HashMap::new(),
            rollback_history: Vec::new(),
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
            promotion_ledger: HashMap::new(),
            shadow_rollout_state: HashMap::new(),
            rollback_history: Vec::new(),
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
        state.promotion_ledger = document.promotion_ledger;
        state.shadow_rollout_state = document.shadow_rollout_state;
        state.rollback_history = document.rollback_history;
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

    pub fn record_promotion(&mut self, admitted_improvement_id: &str, revision: u64) -> Result<(), String> {
        self.promotion_ledger.insert(
            admitted_improvement_id.to_string(),
            PromotionLedgerEntry {
                admitted_improvement_id: admitted_improvement_id.to_string(),
                promotion_date: chrono::Utc::now().to_rfc3339(),
                revision,
            },
        );
        self.persist()
    }

    fn persist(&mut self) -> Result<(), String> {
        if !self.persist_enabled {
            return Ok(());
        }
        let document = ProjectOpsPilotMetricsDocumentV2 {
            schema_version: PROJECT_OPS_PILOT_METRICS_SCHEMA_VERSION,
            command_counts: self.command_counts.clone(),
            view_counts: self.view_counts.clone(),
            projection_rebuild_count: self.projection_rebuild_count,
            last_projection_rebuild_duration_ms: self.last_projection_rebuild_duration_ms,
            last_checkpoint_seq: self.last_checkpoint_seq,
            last_cycle_summary: self.last_cycle_summary.clone(),
            promotion_ledger: self.promotion_ledger.clone(),
            shadow_rollout_state: self.shadow_rollout_state.clone(),
            rollback_history: self.rollback_history.clone(),
        };
        let json = serde_json::to_string(&document).map_err(|e| e.to_string())?;
        fs::write(self.metrics_path.as_path(), json).map_err(|e| e.to_string())
    }
}

fn default_metrics_path() -> PathBuf {
    PathBuf::from("metrics.json")
}

fn load_metrics_document(path: &Path) -> Result<Option<ProjectOpsPilotMetricsDocumentV2>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let document: ProjectOpsPilotMetricsDocumentV2 = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    Ok(Some(document))
}