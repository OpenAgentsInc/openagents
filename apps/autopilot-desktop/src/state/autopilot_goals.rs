//! App-owned autonomous goal specification and persistence model.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalObjective {
    EarnBitcoin {
        min_wallet_delta_sats: u64,
        note: Option<String>,
    },
    SwapBtcToUsd {
        sell_sats: u64,
        note: Option<String>,
    },
    SwapUsdToBtc {
        sell_cents: u64,
        note: Option<String>,
    },
    Custom {
        instruction: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalConstraints {
    pub max_runtime_seconds: u64,
    pub max_attempts: u32,
    pub max_total_spend_sats: Option<u64>,
    pub max_total_swap_cents: Option<u64>,
}

impl Default for GoalConstraints {
    fn default() -> Self {
        Self {
            max_runtime_seconds: 3_600,
            max_attempts: 12,
            max_total_spend_sats: None,
            max_total_swap_cents: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalStopCondition {
    WalletDeltaSatsAtLeast { sats: u64 },
    JobCountAtLeast { count: u32 },
    SuccessCountAtLeast { count: u32 },
    DeadlineEpochSeconds { epoch_seconds: u64 },
    ErrorBudgetExceeded { max_errors: u32 },
    ExternalSignal { key: String, expected: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalRetryPolicy {
    pub max_retries: u32,
    pub backoff_seconds: u64,
    pub exponential_backoff: bool,
}

impl Default for GoalRetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_seconds: 10,
            exponential_backoff: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalScheduleKind {
    Manual,
    IntervalSeconds {
        seconds: u64,
    },
    Cron {
        expression: String,
        timezone: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalScheduleConfig {
    pub enabled: bool,
    pub kind: GoalScheduleKind,
    pub next_run_epoch_seconds: Option<u64>,
}

impl Default for GoalScheduleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: GoalScheduleKind::Manual,
            next_run_epoch_seconds: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalLifecycleStatus {
    Draft,
    Queued,
    Running,
    Paused,
    Succeeded,
    Failed,
    Aborted,
}

impl GoalLifecycleStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Aborted)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalRecord {
    pub goal_id: String,
    pub title: String,
    pub objective: GoalObjective,
    pub constraints: GoalConstraints,
    pub stop_conditions: Vec<GoalStopCondition>,
    pub retry_policy: GoalRetryPolicy,
    pub schedule: GoalScheduleConfig,
    pub lifecycle_status: GoalLifecycleStatus,
    pub created_at_epoch_seconds: u64,
    pub updated_at_epoch_seconds: u64,
    pub last_receipt_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalExecutionReceipt {
    pub receipt_id: String,
    pub goal_id: String,
    pub attempt_index: u32,
    pub started_at_epoch_seconds: u64,
    pub finished_at_epoch_seconds: u64,
    pub lifecycle_status: GoalLifecycleStatus,
    pub wallet_delta_sats: i64,
    pub jobs_completed: u32,
    pub successes: u32,
    pub errors: u32,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AutopilotGoalsDocumentV1 {
    pub schema_version: u16,
    pub active_goals: Vec<GoalRecord>,
    pub historical_goals: Vec<GoalRecord>,
    pub receipts: Vec<GoalExecutionReceipt>,
}

impl Default for AutopilotGoalsDocumentV1 {
    fn default() -> Self {
        Self {
            schema_version: 1,
            active_goals: Vec::new(),
            historical_goals: Vec::new(),
            receipts: Vec::new(),
        }
    }
}

pub struct AutopilotGoalsState {
    pub document: AutopilotGoalsDocumentV1,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    file_path: PathBuf,
}

impl Default for AutopilotGoalsState {
    fn default() -> Self {
        Self::load_from_disk()
    }
}

impl AutopilotGoalsState {
    pub fn load_from_disk() -> Self {
        Self::load_from_path(default_goals_file_path())
    }

    pub fn load_from_path(path: PathBuf) -> Self {
        let mut state = Self {
            document: AutopilotGoalsDocumentV1::default(),
            last_error: None,
            last_action: Some("Goal store ready".to_string()),
            file_path: path.clone(),
        };

        match std::fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<AutopilotGoalsDocumentV1>(&raw) {
                Ok(document) if document.schema_version == 1 => {
                    state.document = document;
                    state.last_action = Some(format!("Loaded goals from {}", path.display()));
                }
                Ok(document) => {
                    state.last_error = Some(format!(
                        "Unsupported goals schema version {}, expected 1",
                        document.schema_version
                    ));
                    state.last_action = Some("Using empty in-memory goal store".to_string());
                }
                Err(error) => {
                    state.last_error = Some(format!("Goals parse error: {error}"));
                    state.last_action = Some("Using empty in-memory goal store".to_string());
                }
            },
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    state.last_error = Some(format!("Goals read error: {error}"));
                }
            }
        }

        state
    }

    pub fn upsert_active_goal(&mut self, mut goal: GoalRecord) -> Result<(), String> {
        goal.updated_at_epoch_seconds = now_epoch_seconds();
        if goal.stop_conditions.is_empty() {
            return Err("Goal must define at least one stop condition".to_string());
        }
        if goal.goal_id.trim().is_empty() {
            return Err("Goal id cannot be empty".to_string());
        }
        if goal.title.trim().is_empty() {
            return Err("Goal title cannot be empty".to_string());
        }
        if goal.lifecycle_status.is_terminal() {
            return Err("Active goal cannot be in a terminal lifecycle status".to_string());
        }

        if let Some(index) = self
            .document
            .active_goals
            .iter()
            .position(|existing| existing.goal_id == goal.goal_id)
        {
            self.document.active_goals[index] = goal.clone();
        } else {
            self.document.active_goals.push(goal.clone());
        }

        self.document
            .historical_goals
            .retain(|existing| existing.goal_id != goal.goal_id);
        self.persist_to_disk()?;
        self.last_action = Some(format!("Upserted active goal {}", goal.goal_id));
        self.last_error = None;
        Ok(())
    }

    pub fn archive_goal(
        &mut self,
        goal_id: &str,
        terminal_status: GoalLifecycleStatus,
    ) -> Result<(), String> {
        if !terminal_status.is_terminal() {
            return Err("Archived goal status must be terminal".to_string());
        }
        let Some(index) = self
            .document
            .active_goals
            .iter()
            .position(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };

        let mut goal = self.document.active_goals.remove(index);
        goal.lifecycle_status = terminal_status;
        goal.updated_at_epoch_seconds = now_epoch_seconds();
        self.document
            .historical_goals
            .retain(|existing| existing.goal_id != goal.goal_id);
        self.document.historical_goals.push(goal);
        if self.document.historical_goals.len() > 512 {
            let remove_count = self.document.historical_goals.len().saturating_sub(512);
            self.document.historical_goals.drain(0..remove_count);
        }

        self.persist_to_disk()?;
        self.last_action = Some(format!("Archived goal {goal_id}"));
        self.last_error = None;
        Ok(())
    }

    pub fn record_receipt(&mut self, receipt: GoalExecutionReceipt) -> Result<(), String> {
        if receipt.receipt_id.trim().is_empty() {
            return Err("Receipt id cannot be empty".to_string());
        }
        if receipt.goal_id.trim().is_empty() {
            return Err("Receipt goal id cannot be empty".to_string());
        }
        if receipt.finished_at_epoch_seconds < receipt.started_at_epoch_seconds {
            return Err(
                "Receipt finished_at_epoch_seconds cannot be before started_at".to_string(),
            );
        }

        let receipt_id = receipt.receipt_id.clone();
        let goal_id = receipt.goal_id.clone();
        self.document.receipts.push(receipt);
        if self.document.receipts.len() > 2_048 {
            let remove_count = self.document.receipts.len().saturating_sub(2_048);
            self.document.receipts.drain(0..remove_count);
        }

        if let Some(goal) = self
            .document
            .active_goals
            .iter_mut()
            .find(|goal| goal.goal_id == goal_id)
        {
            goal.last_receipt_id = Some(receipt_id.clone());
            goal.updated_at_epoch_seconds = now_epoch_seconds();
        }
        if let Some(goal) = self
            .document
            .historical_goals
            .iter_mut()
            .find(|goal| goal.goal_id == goal_id)
        {
            goal.last_receipt_id = Some(receipt_id);
            goal.updated_at_epoch_seconds = now_epoch_seconds();
        }

        self.persist_to_disk()?;
        self.last_action = Some(format!("Recorded receipt for goal {}", goal_id));
        self.last_error = None;
        Ok(())
    }

    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    fn persist_to_disk(&mut self) -> Result<(), String> {
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create goals dir: {error}"))?;
        }
        let payload = serde_json::to_string_pretty(&self.document)
            .map_err(|error| format!("Failed to serialize goals document: {error}"))?;
        std::fs::write(&self.file_path, payload)
            .map_err(|error| format!("Failed to persist goals document: {error}"))?;
        Ok(())
    }
}

fn default_goals_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-goals-v1.json")
}

fn now_epoch_seconds() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AutopilotGoalsState, GoalConstraints, GoalExecutionReceipt, GoalLifecycleStatus,
        GoalObjective, GoalRecord, GoalRetryPolicy, GoalScheduleConfig, GoalStopCondition,
    };

    fn sample_goal(id: &str) -> GoalRecord {
        GoalRecord {
            goal_id: id.to_string(),
            title: "Earn +1000 sats".to_string(),
            objective: GoalObjective::EarnBitcoin {
                min_wallet_delta_sats: 1_000,
                note: None,
            },
            constraints: GoalConstraints::default(),
            stop_conditions: vec![GoalStopCondition::WalletDeltaSatsAtLeast { sats: 1_000 }],
            retry_policy: GoalRetryPolicy::default(),
            schedule: GoalScheduleConfig::default(),
            lifecycle_status: GoalLifecycleStatus::Queued,
            created_at_epoch_seconds: 1_700_000_000,
            updated_at_epoch_seconds: 1_700_000_000,
            last_receipt_id: None,
        }
    }

    #[test]
    fn goals_state_persists_active_historical_and_receipts() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("autopilot-goals-{now_nanos}.json"));

        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        let goal = sample_goal("goal-01");
        state
            .upsert_active_goal(goal)
            .expect("upsert active goal should persist");
        state
            .record_receipt(GoalExecutionReceipt {
                receipt_id: "receipt-01".to_string(),
                goal_id: "goal-01".to_string(),
                attempt_index: 1,
                started_at_epoch_seconds: 1_700_000_010,
                finished_at_epoch_seconds: 1_700_000_020,
                lifecycle_status: GoalLifecycleStatus::Running,
                wallet_delta_sats: 500,
                jobs_completed: 1,
                successes: 0,
                errors: 0,
                notes: Some("partial progress".to_string()),
            })
            .expect("record receipt should persist");
        state
            .archive_goal("goal-01", GoalLifecycleStatus::Succeeded)
            .expect("archive goal should persist");

        let reloaded = AutopilotGoalsState::load_from_path(path.clone());
        assert!(reloaded.document.active_goals.is_empty());
        assert_eq!(reloaded.document.historical_goals.len(), 1);
        assert_eq!(
            reloaded.document.historical_goals[0].lifecycle_status,
            GoalLifecycleStatus::Succeeded
        );
        assert_eq!(reloaded.document.receipts.len(), 1);

        let _ = std::fs::remove_file(path);
    }
}
