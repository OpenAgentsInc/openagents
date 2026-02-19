//! Experiment grouping for RLM runs.
//!
//! Provides a local, serializable container for grouping runs and
//! computing aggregate metrics for comparison.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRunSummary {
    pub run_id: String,
    pub status: String,
    pub total_cost_sats: u64,
    pub total_duration_ms: u64,
    pub fragment_count: u64,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub runs: Vec<ExperimentRunSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentMetrics {
    pub run_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub success_rate: f64,
    pub avg_cost_sats: u64,
    pub avg_duration_ms: u64,
    pub avg_fragment_count: u64,
}

impl ExperimentGroup {
    pub fn new(name: impl Into<String>, description: Option<String>) -> Self {
        let now = now_ts();
        Self {
            id: format!("exp-{}", now),
            name: name.into(),
            description,
            created_at: now,
            updated_at: now,
            runs: Vec::new(),
        }
    }

    pub fn add_run(&mut self, run: ExperimentRunSummary) {
        self.runs.push(run);
        self.updated_at = now_ts();
    }

    pub fn metrics(&self) -> ExperimentMetrics {
        if self.runs.is_empty() {
            return ExperimentMetrics {
                run_count: 0,
                completed_count: 0,
                failed_count: 0,
                success_rate: 0.0,
                avg_cost_sats: 0,
                avg_duration_ms: 0,
                avg_fragment_count: 0,
            };
        }

        let mut completed = 0usize;
        let mut failed = 0usize;
        let mut total_cost = 0u64;
        let mut total_duration = 0u64;
        let mut total_fragments = 0u64;

        for run in &self.runs {
            if run.status == "completed" {
                completed += 1;
            } else if run.status == "failed" {
                failed += 1;
            }
            total_cost = total_cost.saturating_add(run.total_cost_sats);
            total_duration = total_duration.saturating_add(run.total_duration_ms);
            total_fragments = total_fragments.saturating_add(run.fragment_count);
        }

        let run_count = self.runs.len();
        ExperimentMetrics {
            run_count,
            completed_count: completed,
            failed_count: failed,
            success_rate: completed as f64 / run_count as f64,
            avg_cost_sats: total_cost / run_count as u64,
            avg_duration_ms: total_duration / run_count as u64,
            avg_fragment_count: total_fragments / run_count as u64,
        }
    }

    pub fn to_json_string(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json_str(input: &str) -> serde_json::Result<Self> {
        serde_json::from_str(input)
    }

    pub fn save_to(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(&self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, json)
    }

    pub fn load_from(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let data = fs::read_to_string(path)?;
        serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
