//! TBCC domain types
//!
//! Core types are re-exported from terminalbench crate.
//! Gym-specific types are defined here.

use serde::{Deserialize, Serialize};

// Re-export core types from terminalbench
pub use terminalbench::{
    TBTask,
    TBDifficulty,
    TBRunSummary,
    TBRunStatus,
    TBRunOutcome,
    TBModelOption,
    ExecutionSettings,
    DashboardStats,
    DifficultyStats,
    DifficultyCount,
    format_duration,
    format_percent,
};

/// Color helper for difficulty (gym-specific display)
pub fn difficulty_color(diff: TBDifficulty) -> &'static str {
    match diff {
        TBDifficulty::Easy => "emerald",
        TBDifficulty::Medium => "amber",
        TBDifficulty::Hard => "orange",
        TBDifficulty::Expert => "red",
        TBDifficulty::Unknown => "zinc",
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
