//! TBCC domain types

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

    pub fn color(&self) -> &'static str {
        match self {
            Self::Easy => "emerald",
            Self::Medium => "amber",
            Self::Hard => "orange",
            Self::Expert => "red",
            Self::Unknown => "zinc",
        }
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

/// Dashboard statistics
#[derive(Debug, Clone, Default)]
pub struct DashboardStats {
    /// Overall success rate (0.0 - 1.0)
    pub success_rate: f32,
    /// Last 50 runs success rate
    pub last_50_success_rate: f32,
    /// Average steps per run
    pub avg_steps: f32,
    /// Average duration in seconds
    pub avg_duration_secs: f32,
    /// Total number of runs
    pub total_runs: u32,
    /// Stats by difficulty
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

/// Current run information (for active run indicator)
#[derive(Debug, Clone)]
pub struct CurrentRunInfo {
    pub run_id: String,
    pub task_id: String,
    pub task_name: String,
    pub attempt: u32,
    pub max_attempts: u32,
    pub current_step: u32,
    pub total_steps: Option<u32>,
    pub started_at: u64,
}

/// TBCC sub-tab identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TBCCTab {
    #[default]
    Dashboard,
    Tasks,
    Runs,
    Settings,
}

impl TBCCTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Tasks => "Tasks",
            Self::Runs => "Runs",
            Self::Settings => "Settings",
        }
    }

    pub fn all() -> &'static [TBCCTab] {
        &[
            TBCCTab::Dashboard,
            TBCCTab::Tasks,
            TBCCTab::Runs,
            TBCCTab::Settings,
        ]
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

/// Container settings for sandbox execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSettings {
    pub image: String,
    pub memory_limit: String,
    pub cpu_limit: f32,
    pub auto_remove: bool,
}

impl Default for ContainerSettings {
    fn default() -> Self {
        Self {
            image: "mechacoder:latest".to_string(),
            memory_limit: "4G".to_string(),
            cpu_limit: 2.0,
            auto_remove: true,
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
