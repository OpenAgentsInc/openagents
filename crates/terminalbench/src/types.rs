//! Terminal-Bench domain types
//!
//! Core types shared between commander, mechacoder, and other tools.

use serde::{Deserialize, Serialize};

/// Terminal-Bench task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TBTask {
    pub id: String,
    pub name: String,
    pub description: String,
    pub difficulty: TBDifficulty,
    pub timeout_ms: u32,
    pub max_turns: u32,
    pub tags: Vec<String>,
}

/// Task difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TBDifficulty {
    Easy,
    Medium,
    Hard,
    Expert,
    Unknown,
}

impl TBDifficulty {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Medium => "Medium",
            Self::Hard => "Hard",
            Self::Expert => "Expert",
            Self::Unknown => "Unknown",
        }
    }
}

impl Default for TBDifficulty {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Terminal-Bench run summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TBRunSummary {
    pub id: String,
    pub task_id: String,
    pub task_name: String,
    pub status: TBRunStatus,
    pub outcome: Option<TBRunOutcome>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub steps_count: u32,
    pub tokens_used: Option<u32>,
}

/// Run execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TBRunStatus {
    Queued,
    Running,
    Completed,
    Error,
}

impl TBRunStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Completed => "Completed",
            Self::Error => "Error",
        }
    }
}

impl Default for TBRunStatus {
    fn default() -> Self {
        Self::Queued
    }
}

/// Run outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TBRunOutcome {
    Success,
    Failure,
    Timeout,
    Error,
    Aborted,
}

impl TBRunOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::Failure => "Failure",
            Self::Timeout => "Timeout",
            Self::Error => "Error",
            Self::Aborted => "Aborted",
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

/// Model options for running benchmarks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TBModelOption {
    #[default]
    ClaudeSonnet,
    ClaudeHaiku,
    Gpt4o,
    Gpt4oMini,
    AppleFM,
}

impl TBModelOption {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ClaudeSonnet => "Claude Sonnet 4",
            Self::ClaudeHaiku => "Claude Haiku",
            Self::Gpt4o => "GPT-4o",
            Self::Gpt4oMini => "GPT-4o Mini",
            Self::AppleFM => "Apple FM (Local)",
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::ClaudeSonnet => "claude-sonnet-4-20250514",
            Self::ClaudeHaiku => "claude-3-5-haiku-20241022",
            Self::Gpt4o => "gpt-4o",
            Self::Gpt4oMini => "gpt-4o-mini",
            Self::AppleFM => "apple-fm",
        }
    }

    pub fn all() -> &'static [TBModelOption] {
        &[
            TBModelOption::ClaudeSonnet,
            TBModelOption::ClaudeHaiku,
            TBModelOption::Gpt4o,
            TBModelOption::Gpt4oMini,
            TBModelOption::AppleFM,
        ]
    }
}

/// Execution settings for running benchmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSettings {
    pub model: TBModelOption,
    pub max_attempts: u32,
    pub timeout_ms: u32,
    pub max_tokens: u32,
    pub save_trajectories: bool,
}

impl Default for ExecutionSettings {
    fn default() -> Self {
        Self {
            model: TBModelOption::default(),
            max_attempts: 5,
            timeout_ms: 120_000,
            max_tokens: 8192,
            save_trajectories: true,
        }
    }
}

/// Run options for starting a benchmark
#[derive(Debug, Clone)]
pub struct TBRunOptions {
    pub task: TBTask,
    pub model: TBModelOption,
    pub timeout_secs: u64,
    pub max_turns: u32,
}

/// Dashboard statistics (for gym UI)
#[derive(Debug, Clone, Default)]
pub struct DashboardStats {
    pub success_rate: f32,
    pub last_50_success_rate: f32,
    pub avg_steps: f32,
    pub avg_duration_secs: f32,
    pub total_runs: u32,
    pub by_difficulty: DifficultyStats,
}

/// Stats broken down by difficulty
#[derive(Debug, Clone, Default)]
pub struct DifficultyStats {
    pub easy: DifficultyCount,
    pub medium: DifficultyCount,
    pub hard: DifficultyCount,
    pub expert: DifficultyCount,
}

/// Count of passed/total for a difficulty
#[derive(Debug, Clone, Default)]
pub struct DifficultyCount {
    pub passed: u32,
    pub total: u32,
}

impl DifficultyCount {
    pub fn rate(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.passed as f32 / self.total as f32
        }
    }
}

/// Helper functions
pub fn format_duration(ms: Option<u64>) -> String {
    match ms {
        None => "-".to_string(),
        Some(ms) => {
            let secs = ms / 1000;
            if secs < 60 {
                format!("{}s", secs)
            } else {
                let mins = secs / 60;
                let rem_secs = secs % 60;
                format!("{}m {}s", mins, rem_secs)
            }
        }
    }
}

pub fn format_percent(value: f32) -> String {
    format!("{:.1}%", value * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_label() {
        assert_eq!(TBDifficulty::Easy.label(), "Easy");
        assert_eq!(TBDifficulty::Expert.label(), "Expert");
    }

    #[test]
    fn test_outcome_is_success() {
        assert!(TBRunOutcome::Success.is_success());
        assert!(!TBRunOutcome::Failure.is_success());
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(None), "-");
        assert_eq!(format_duration(Some(30_000)), "30s");
        assert_eq!(format_duration(Some(90_000)), "1m 30s");
    }

    #[test]
    fn test_model_option_id() {
        assert_eq!(TBModelOption::ClaudeSonnet.id(), "claude-sonnet-4-20250514");
        assert_eq!(TBModelOption::AppleFM.id(), "apple-fm");
    }
}
