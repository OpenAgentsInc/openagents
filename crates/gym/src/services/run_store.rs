//! Run store - manages Terminal-Bench run history

use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::tbcc::types::{TBRunSummary, TBRunStatus, TBRunOutcome, DashboardStats, DifficultyStats, DifficultyCount, TBDifficulty};

/// Detailed run record for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub task_id: String,
    pub task_name: String,
    pub task_difficulty: String,
    pub status: String,
    pub outcome: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub steps_count: u32,
    pub tokens_used: Option<u32>,
    pub model: String,
    pub error_message: Option<String>,
}

/// Run store service
pub struct RunStore {
    data_dir: PathBuf,
    /// In-memory cache of runs
    runs: Vec<RunRecord>,
    /// Index by task ID
    by_task: HashMap<String, Vec<usize>>,
}

impl RunStore {
    pub fn new(data_dir: PathBuf) -> Self {
        let mut store = Self {
            data_dir,
            runs: vec![],
            by_task: HashMap::new(),
        };
        store.load_from_disk();
        store
    }

    /// Load runs from disk
    fn load_from_disk(&mut self) {
        let runs_file = self.data_dir.join("tb_runs.json");
        if let Ok(content) = fs::read_to_string(&runs_file) {
            if let Ok(runs) = serde_json::from_str::<Vec<RunRecord>>(&content) {
                self.runs = runs;
                self.rebuild_index();
            }
        }
    }

    /// Save runs to disk
    fn save_to_disk(&self) {
        let runs_file = self.data_dir.join("tb_runs.json");
        if let Ok(content) = serde_json::to_string_pretty(&self.runs) {
            let _ = fs::write(runs_file, content);
        }
    }

    /// Rebuild the task index
    fn rebuild_index(&mut self) {
        self.by_task.clear();
        for (idx, run) in self.runs.iter().enumerate() {
            self.by_task
                .entry(run.task_id.clone())
                .or_default()
                .push(idx);
        }
    }

    /// Start a new run
    pub fn start_run(&mut self, task_id: &str, task_name: &str, task_difficulty: TBDifficulty, model: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let record = RunRecord {
            id: id.clone(),
            task_id: task_id.to_string(),
            task_name: task_name.to_string(),
            task_difficulty: task_difficulty.label().to_string(),
            status: "running".to_string(),
            outcome: None,
            started_at: Utc::now().to_rfc3339(),
            finished_at: None,
            duration_ms: None,
            steps_count: 0,
            tokens_used: None,
            model: model.to_string(),
            error_message: None,
        };

        let idx = self.runs.len();
        self.runs.push(record);
        self.by_task.entry(task_id.to_string()).or_default().push(idx);
        self.save_to_disk();

        id
    }

    /// Update a run's progress
    pub fn update_run(&mut self, run_id: &str, steps: u32, tokens: Option<u32>) {
        if let Some(run) = self.runs.iter_mut().find(|r| r.id == run_id) {
            run.steps_count = steps;
            run.tokens_used = tokens;
            self.save_to_disk();
        }
    }

    /// Complete a run
    pub fn complete_run(&mut self, run_id: &str, outcome: TBRunOutcome, error: Option<String>) {
        if let Some(run) = self.runs.iter_mut().find(|r| r.id == run_id) {
            let now = Utc::now();
            run.status = "completed".to_string();
            run.outcome = Some(outcome.label().to_string());
            run.finished_at = Some(now.to_rfc3339());
            run.error_message = error;

            // Calculate duration
            if let Ok(started) = DateTime::parse_from_rfc3339(&run.started_at) {
                let duration = now.signed_duration_since(started.with_timezone(&Utc));
                run.duration_ms = Some(duration.num_milliseconds() as u64);
            }

            self.save_to_disk();
        }
    }

    /// Get all runs as summaries
    pub fn get_all_runs(&self) -> Vec<TBRunSummary> {
        self.runs.iter()
            .rev() // Most recent first
            .map(|r| record_to_summary(r))
            .collect()
    }

    /// Get recent runs (up to limit)
    pub fn get_recent_runs(&self, limit: usize) -> Vec<TBRunSummary> {
        self.runs.iter()
            .rev()
            .take(limit)
            .map(|r| record_to_summary(r))
            .collect()
    }

    /// Get runs for a specific task
    pub fn get_runs_for_task(&self, task_id: &str) -> Vec<TBRunSummary> {
        self.by_task.get(task_id)
            .map(|indices| {
                indices.iter()
                    .rev()
                    .filter_map(|&idx| self.runs.get(idx))
                    .map(|r| record_to_summary(r))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Calculate dashboard statistics
    pub fn calculate_stats(&self) -> DashboardStats {
        let completed: Vec<_> = self.runs.iter()
            .filter(|r| r.status == "completed")
            .collect();

        let total = completed.len() as u32;
        if total == 0 {
            return DashboardStats::default();
        }

        let successful = completed.iter()
            .filter(|r| r.outcome.as_ref().map(|o| o.to_lowercase() == "success").unwrap_or(false))
            .count() as u32;

        let success_rate = successful as f32 / total as f32;

        // Last 50 runs
        let last_50: Vec<_> = completed.iter().rev().take(50).collect();
        let last_50_successful = last_50.iter()
            .filter(|r| r.outcome.as_ref().map(|o| o.to_lowercase() == "success").unwrap_or(false))
            .count() as f32;
        let last_50_rate = if last_50.is_empty() { 0.0 } else { last_50_successful / last_50.len() as f32 };

        // Average steps
        let total_steps: u32 = completed.iter().map(|r| r.steps_count).sum();
        let avg_steps = total_steps as f32 / total as f32;

        // Average duration
        let total_duration: u64 = completed.iter()
            .filter_map(|r| r.duration_ms)
            .sum();
        let duration_count = completed.iter().filter(|r| r.duration_ms.is_some()).count();
        let avg_duration_secs = if duration_count > 0 {
            (total_duration as f32 / duration_count as f32) / 1000.0
        } else {
            0.0
        };

        // By difficulty
        let mut by_difficulty = DifficultyStats::default();
        for run in &completed {
            let is_success = run.outcome.as_ref().map(|o| o.to_lowercase() == "success").unwrap_or(false);
            match run.task_difficulty.to_lowercase().as_str() {
                "easy" => {
                    by_difficulty.easy.total += 1;
                    if is_success { by_difficulty.easy.passed += 1; }
                }
                "medium" => {
                    by_difficulty.medium.total += 1;
                    if is_success { by_difficulty.medium.passed += 1; }
                }
                "hard" => {
                    by_difficulty.hard.total += 1;
                    if is_success { by_difficulty.hard.passed += 1; }
                }
                "expert" => {
                    by_difficulty.expert.total += 1;
                    if is_success { by_difficulty.expert.passed += 1; }
                }
                _ => {}
            }
        }

        DashboardStats {
            success_rate,
            last_50_success_rate: last_50_rate,
            avg_steps,
            avg_duration_secs,
            total_runs: total,
            by_difficulty,
        }
    }

    /// Get total run count
    pub fn count(&self) -> usize {
        self.runs.len()
    }
}

/// Convert record to summary
fn record_to_summary(r: &RunRecord) -> TBRunSummary {
    TBRunSummary {
        id: r.id.clone(),
        task_id: r.task_id.clone(),
        task_name: r.task_name.clone(),
        status: parse_status(&r.status),
        outcome: r.outcome.as_ref().map(|o| parse_outcome(o)),
        started_at: r.started_at.clone(),
        finished_at: r.finished_at.clone(),
        duration_ms: r.duration_ms,
        steps_count: r.steps_count,
        tokens_used: r.tokens_used,
    }
}

fn parse_status(s: &str) -> TBRunStatus {
    match s {
        "running" => TBRunStatus::Running,
        "completed" => TBRunStatus::Completed,
        "error" => TBRunStatus::Error,
        "queued" => TBRunStatus::Queued,
        _ => TBRunStatus::Queued,
    }
}

fn parse_outcome(s: &str) -> TBRunOutcome {
    match s.to_lowercase().as_str() {
        "success" => TBRunOutcome::Success,
        "failure" | "failed" => TBRunOutcome::Failure,
        "timeout" => TBRunOutcome::Timeout,
        "error" => TBRunOutcome::Error,
        "aborted" => TBRunOutcome::Aborted,
        _ => TBRunOutcome::Aborted,
    }
}

// Note: Default implementations are derived in types.rs

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_run_store_basics() {
        // Use a unique directory for this test
        let dir = temp_dir().join(format!("test_run_store_{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&dir); // Clean up any old data
        let _ = fs::create_dir_all(&dir);

        let mut store = RunStore::new(dir.clone());

        // Verify clean start
        assert_eq!(store.count(), 0);

        // Start a run
        let run_id = store.start_run("task-1", "Test Task", TBDifficulty::Easy, "test-model");
        assert!(!run_id.is_empty());

        // Update it
        store.update_run(&run_id, 5, Some(1000));

        // Complete it
        store.complete_run(&run_id, TBRunOutcome::Success, None);

        // Check stats
        let stats = store.calculate_stats();
        assert_eq!(stats.total_runs, 1);
        assert_eq!(stats.success_rate, 1.0);

        // Cleanup
        let _ = fs::remove_dir_all(dir);
    }
}
