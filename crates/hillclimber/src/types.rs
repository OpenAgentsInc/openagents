//! Core domain types for the HillClimber system.
//!
//! These types map to the SQLite schema and define the data structures
//! used throughout the MAP orchestration loop.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Configuration Types
// ============================================================================

/// Task configuration - the "knobs" we're tuning.
///
/// Stored in `hillclimber_configs` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HillClimberConfig {
    pub id: i64,
    pub task_id: String,
    pub hint: Option<String>,
    pub use_skills: bool,
    pub max_turns_override: u32,
    pub config_hash: String,
    pub is_current: bool,
    pub created_at: String,
}

/// Input for creating a new config (without auto-generated fields).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HillClimberConfigInput {
    pub task_id: String,
    pub hint: Option<String>,
    pub use_skills: bool,
    pub max_turns_override: u32,
}

// ============================================================================
// Run Types
// ============================================================================

/// Run record - every execution attempt.
///
/// Stored in `hillclimber_runs` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HillClimberRun {
    pub id: i64,
    pub run_id: String,
    pub task_id: String,
    pub config_id: i64,
    pub passed: bool,
    pub turns: u32,
    pub duration_ms: u64,
    pub step_summary: Option<Vec<String>>,
    pub error_message: Option<String>,
    pub meta_model: Option<String>,
    pub proposed_change: Option<String>,
    pub change_accepted: bool,
    pub score: i32,
    pub is_best: bool,
    pub created_at: String,
}

/// Input for creating a new run (without auto-generated fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HillClimberRunInput {
    pub run_id: String,
    pub task_id: String,
    pub config_id: i64,
    pub passed: bool,
    pub turns: u32,
    pub duration_ms: u64,
    pub step_summary: Option<Vec<String>>,
    pub error_message: Option<String>,
    pub meta_model: Option<String>,
    pub proposed_change: Option<String>,
    pub change_accepted: bool,
    pub score: i32,
}

/// Best config per task (for quick lookup and export).
///
/// Stored in `hillclimber_best_configs` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestConfig {
    pub task_id: String,
    pub config_id: i64,
    pub run_id: i64,
    pub score: i32,
    pub pass_count: u32,
    pub total_runs: u32,
    pub updated_at: String,
}

// ============================================================================
// Task Decomposition Types
// ============================================================================

/// A subtask within the decomposed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    pub id: u32,
    pub name: String,
    pub goal: String,
    pub checkpoint: String,
    pub expected_artifacts: Vec<String>,
    pub depends_on: Vec<u32>,
    pub hints: Vec<String>,
    pub max_turns: u32,
}

/// Complete task decomposition result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDecomposition {
    pub task_id: String,
    pub subtask_count: u32,
    pub subtasks: Vec<Subtask>,
    pub global_hints: Vec<String>,
    pub files_to_read: Vec<String>,
    pub required_outputs: Vec<String>,
}

// ============================================================================
// Execution State Types
// ============================================================================

/// State of a subtask during execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtaskState {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl Default for SubtaskState {
    fn default() -> Self {
        SubtaskState::Pending
    }
}

/// Status of a subtask during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskStatus {
    pub subtask_id: u32,
    pub name: String,
    pub status: SubtaskState,
    pub turns_used: u32,
    pub progress: f64,
    pub checkpoint_passed: bool,
}

/// Mutable execution state during MAP orchestration.
#[derive(Debug, Clone)]
pub struct ExecutionState {
    pub current_subtask: usize,
    pub total_turns: u32,
    pub subtask_turns: u32,
    pub modified_files: Vec<String>,
    pub previous_actions: Vec<String>,
    pub last_evaluation: Option<EvaluatorResult>,
    pub best_progress: f64,
    pub turns_since_improvement: u32,
    pub subtask_status: Vec<SubtaskStatus>,
    pub output: String,
    pub monitor_warning: Option<String>,
}

impl ExecutionState {
    /// Create a new execution state from a task decomposition.
    pub fn new(decomposition: &TaskDecomposition) -> Self {
        let subtask_status = decomposition
            .subtasks
            .iter()
            .map(|s| SubtaskStatus {
                subtask_id: s.id,
                name: s.name.clone(),
                status: if s.id == 0 {
                    SubtaskState::InProgress
                } else {
                    SubtaskState::Pending
                },
                turns_used: 0,
                progress: 0.0,
                checkpoint_passed: false,
            })
            .collect();

        Self {
            current_subtask: 0,
            total_turns: 0,
            subtask_turns: 0,
            modified_files: Vec::new(),
            previous_actions: Vec::new(),
            last_evaluation: None,
            best_progress: 0.0,
            turns_since_improvement: 0,
            subtask_status,
            output: String::new(),
            monitor_warning: None,
        }
    }
}

// ============================================================================
// Action Types
// ============================================================================

/// Action from the FM actor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FMAction {
    pub tool_name: String,
    pub tool_args: serde_json::Value,
    pub reasoning: Option<String>,
}

/// Result from executing an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub output: String,
    pub modified_file: Option<String>,
}

// ============================================================================
// Monitor Types
// ============================================================================

/// Decision from the monitor about whether to allow an action.
#[derive(Debug, Clone)]
pub struct MonitorDecision {
    pub allowed: bool,
    pub reason: Option<String>,
    pub warning: Option<String>,
    pub suggestion: Option<String>,
}

impl MonitorDecision {
    /// Create an "allow" decision.
    pub fn allow() -> Self {
        Self {
            allowed: true,
            reason: None,
            warning: None,
            suggestion: None,
        }
    }

    /// Create an "allow with warning" decision.
    pub fn allow_with_warning(warning: impl Into<String>) -> Self {
        Self {
            allowed: true,
            reason: None,
            warning: Some(warning.into()),
            suggestion: None,
        }
    }

    /// Create a "deny" decision.
    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: Some(reason.into()),
            warning: None,
            suggestion: None,
        }
    }

    /// Create a "deny with suggestion" decision.
    pub fn deny_with_suggestion(reason: impl Into<String>, suggestion: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: Some(reason.into()),
            warning: None,
            suggestion: Some(suggestion.into()),
        }
    }
}

/// Context for monitoring an action.
#[derive(Debug, Clone)]
pub struct ActionContext {
    pub tool_name: String,
    pub args: serde_json::Value,
    pub workspace: PathBuf,
    pub task_id: String,
    pub modified_files: Vec<String>,
    pub turn_number: u32,
    pub previous_actions: Vec<String>,
}

// ============================================================================
// Evaluator Types
// ============================================================================

/// Details about a failed test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureDetail {
    pub test_name: String,
    pub line_number: Option<u32>,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub message: String,
}

/// Result from evaluating progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorResult {
    pub passed: bool,
    pub progress: f64,
    pub tests_total: u32,
    pub tests_passing: u32,
    pub failures: Vec<FailureDetail>,
    pub suggestion: Option<String>,
    pub raw_output: String,
    pub duration_ms: u64,
}

impl Default for EvaluatorResult {
    fn default() -> Self {
        Self {
            passed: false,
            progress: 0.0,
            tests_total: 0,
            tests_passing: 0,
            failures: Vec::new(),
            suggestion: None,
            raw_output: String::new(),
            duration_ms: 0,
        }
    }
}

// ============================================================================
// Orchestrator Types
// ============================================================================

/// Options for the MAP orchestrator.
#[derive(Debug, Clone)]
pub struct MAPOrchestratorOptions {
    pub workspace: PathBuf,
    pub timeout_secs: u64,
    pub max_turns: u32,
    pub task_description: String,
    pub verbose: bool,
    pub use_sampling: bool,
}

impl Default for MAPOrchestratorOptions {
    fn default() -> Self {
        Self {
            workspace: PathBuf::from("."),
            timeout_secs: 600, // 10 minutes
            max_turns: 30,
            task_description: String::new(),
            verbose: false,
            use_sampling: true,
        }
    }
}

/// Result from the MAP orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MAPOrchestratorResult {
    pub passed: bool,
    pub turns: u32,
    pub duration_ms: u64,
    pub progress: f64,
    pub output: String,
    pub error: Option<String>,
    pub subtask_status: Vec<SubtaskStatus>,
    pub evaluation: Option<EvaluatorResult>,
}

// ============================================================================
// Sampling Types
// ============================================================================

/// Result from a single candidate in parallel sampling.
#[derive(Debug, Clone)]
pub struct CandidateResult {
    pub index: usize,
    pub temperature: f32,
    pub variation_hint: String,
    pub workspace: PathBuf,
    pub passed: bool,
    pub progress: f64,
    pub tests_passing: u32,
    pub tests_total: u32,
    pub solution: Option<String>,
}

/// Result from parallel sampling.
#[derive(Debug, Clone)]
pub struct SamplingResult {
    pub best: CandidateResult,
    pub all: Vec<CandidateResult>,
    pub average_progress: f64,
    pub improvement: f64,
}

// ============================================================================
// Task Types (TB2 compatible)
// ============================================================================

/// Verification configuration for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    #[serde(rename = "type")]
    pub verification_type: String, // "test", "script", etc.
    pub command: Option<String>,
    pub script: Option<String>,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            verification_type: "test".to_string(),
            command: None,
            script: None,
        }
    }
}

/// A Terminal-Bench task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBenchTask {
    pub id: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<PathBuf>,
    #[serde(default)]
    pub verification: VerificationConfig,
}

// ============================================================================
// Statistics Types
// ============================================================================

/// Stats for a single task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskStats {
    pub task_id: String,
    pub total_runs: u64,
    pub pass_count: u64,
    pub pass_rate: f64,
    pub best_score: i32,
    pub avg_turns: f64,
    pub last_run_at: Option<String>,
    pub current_config_id: Option<i64>,
    pub best_config_id: Option<i64>,
}

/// Aggregate stats across all tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HillClimberStats {
    pub total_runs: u64,
    pub total_passes: u64,
    pub overall_pass_rate: f64,
    pub unique_tasks: u64,
    pub unique_configs: u64,
    pub by_task: HashMap<String, TaskStats>,
}

// ============================================================================
// Config Change Types
// ============================================================================

/// Type of config change proposed by meta-reasoner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigChangeType {
    Keep,
    UpdateHint,
    ToggleSkills,
    AdjustTurns,
}

impl Default for ConfigChangeType {
    fn default() -> Self {
        ConfigChangeType::Keep
    }
}

/// Proposed config change from meta-reasoner.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigChange {
    #[serde(rename = "type")]
    pub change_type: ConfigChangeType,
    pub new_hint: Option<String>,
    pub new_use_skills: Option<bool>,
    pub new_max_turns: Option<u32>,
    pub reasoning: Option<String>,
    pub model: Option<String>,
}

// ============================================================================
// Step Decision
// ============================================================================

/// Decision about what to do next in the orchestrator loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepDecision {
    /// Continue with current subtask.
    Continue,
    /// Advance to next subtask.
    Advance,
    /// Task is complete (all tests pass).
    Complete,
    /// No progress being made, try different approach.
    NoProgress,
}

// ============================================================================
// FM Context
// ============================================================================

/// Context passed to FM for generating the next action.
#[derive(Debug, Clone)]
pub struct FMContext {
    pub task_description: String,
    pub current_subtask: Subtask,
    pub previous_actions: Vec<String>,
    pub verification_feedback: Option<String>,
    pub hints: Vec<String>,
    pub global_hints: Vec<String>,
    pub file_contents: HashMap<String, String>,
}

// ============================================================================
// Helpers
// ============================================================================

/// Generate a unique run ID.
pub fn generate_run_id() -> String {
    let now = chrono::Utc::now();
    let date_str = now.format("%Y%m%d").to_string();
    let time_str = now.format("%H%M%S").to_string();
    let random: String = uuid::Uuid::new_v4().to_string()[..6].to_string();
    format!("hc-{}-{}-{}", date_str, time_str, random)
}

/// Generate a unique session ID.
pub fn generate_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Create an action signature for tracking previous actions from an FMAction.
pub fn action_to_signature(action: &FMAction) -> String {
    match action.tool_name.as_str() {
        "write_file" => {
            let path = action
                .tool_args
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("write_file:{}", path)
        }
        "read_file" => {
            let path = action
                .tool_args
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("read_file:{}", path)
        }
        "run_command" => {
            let cmd = action
                .tool_args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let truncated = if cmd.len() > 50 {
                format!("{}...", &cmd[..50])
            } else {
                cmd.to_string()
            };
            format!("run_command:{}", truncated)
        }
        other => other.to_string(),
    }
}
