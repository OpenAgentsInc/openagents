//! TBCC domain types

/// Terminal-Bench task definition
#[derive(Debug, Clone)]
pub struct TBTask {
    pub id: String,
    pub name: String,
    pub description: String,
    pub difficulty: TBDifficulty,
    pub timeout: u32,
    pub max_turns: u32,
}

/// Task difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TBDifficulty {
    Easy,
    Medium,
    Hard,
    Expert,
    Unknown,
}

/// Terminal-Bench run summary
#[derive(Debug, Clone)]
pub struct TBRunSummary {
    pub id: String,
    pub task_id: String,
    pub task_name: String,
    pub status: TBRunStatus,
    pub outcome: Option<TBRunOutcome>,
    pub duration_ms: Option<u64>,
    pub steps: u32,
}

/// Run execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TBRunStatus {
    Queued,
    Running,
    Completed,
    Error,
}

/// Run outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TBRunOutcome {
    Success,
    Failure,
    Timeout,
    Error,
    Aborted,
}

/// Dashboard statistics
#[derive(Debug, Clone, Default)]
pub struct DashboardStats {
    pub success_rate: f32,
    pub avg_steps: f32,
    pub avg_duration: f32,
    pub total_runs: u32,
}
