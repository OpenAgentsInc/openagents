//! Orchestrator/Subagent Architecture Types
//!
//! Following Anthropic's "Effective Harnesses for Long-Running Agents" pattern:
//! - Orchestrator: Manages task selection, decomposition, verification, session coordination
//! - Subagent: Minimal coding agent that implements one subtask at a time

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Subtask Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtaskStatus {
    Pending,
    InProgress,
    Done,
    Verified,
    Failed,
}

impl Default for SubtaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Claude Code session tracking for resumption across orchestrator runs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeSession {
    /// Active Claude Code session ID used for this subtask
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Session ID this run was forked from (when branching)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forked_from_session_id: Option<String>,
    /// Whether the next resume should fork instead of continue
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_strategy: Option<ResumeStrategy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeStrategy {
    Continue,
    Fork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subtask {
    pub id: String,
    pub description: String,
    pub status: SubtaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Number of consecutive failures on this subtask
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_count: Option<u32>,
    /// Last failure reason (for context when resuming)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_reason: Option<String>,
    /// Claude Code session tracking for resumption
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code: Option<ClaudeCodeSession>,
}

impl Subtask {
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            status: SubtaskStatus::Pending,
            started_at: None,
            completed_at: None,
            verified_at: None,
            error: None,
            failure_count: None,
            last_failure_reason: None,
            claude_code: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtaskList {
    pub task_id: String,
    pub task_title: String,
    pub subtasks: Vec<Subtask>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Progress File Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClaudeCodePermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
    DontAsk,
}

impl Default for ClaudeCodePermissionMode {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefer_for_complex_tasks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_turns_per_subtask: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<ClaudeCodePermissionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_to_minimal: Option<bool>,
    /// Abort Claude Code runs that exceed this duration to avoid stuck sessions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms_per_subtask: Option<u64>,
}

/// Token usage from Claude API
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u64>,
}

/// Claude Code session metadata for context bridging
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeSessionMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forked_from_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_used: Option<HashMap<String, u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ApiUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost_usd: Option<f64>,
}

/// Init script result
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitScriptResult {
    pub ran: bool,
    /// true if script exited with 0 or 2 (proceed), false if exit 1 (abort)
    pub success: bool,
    /// true if script exited with 2 (warnings present but proceed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_warnings: Option<bool>,
    /// exit code from the script (0, 1, or 2)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Structured failure type for safe mode recovery
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_type: Option<InitScriptFailureType>,
    /// Whether this failure type can potentially be self-healed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_self_heal: Option<bool>,
}

/// Structured failure types for init script errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitScriptFailureType {
    /// TypeScript/type errors - can self-heal
    TypecheckFailed,
    /// Tests failing - can attempt fix
    TestFailed,
    /// Network issues - can continue in offline mode
    NetworkError,
    /// Disk space issues - cannot self-heal
    DiskFull,
    /// Permission issues - cannot self-heal
    PermissionDenied,
    /// Unknown error - fallback
    Unknown,
}

/// Orientation phase result
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Orientation {
    pub repo_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_session_summary: Option<String>,
    pub tests_passing_at_start: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init_script: Option<InitScriptResult>,
}

/// Work phase tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkProgress {
    pub subtasks_completed: Vec<String>,
    pub subtasks_in_progress: Vec<String>,
    pub files_modified: Vec<String>,
    pub tests_run: bool,
    pub tests_passing_after_work: bool,
    pub e2e_run: bool,
    pub e2e_passing_after_work: bool,
    /// Claude Code session metadata for context bridging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code_session: Option<ClaudeCodeSessionMetadata>,
}

/// Next session suggestions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextSession {
    pub suggested_next_steps: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionProgress {
    pub session_id: String,
    pub started_at: String,
    pub task_id: String,
    pub task_title: String,
    pub orientation: Orientation,
    pub work: WorkProgress,
    pub next_session: NextSession,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

// ============================================================================
// Subagent Types
// ============================================================================

/// Token usage summary
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
}

/// Learning metrics for FM subagent runs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningMetrics {
    /// IDs of skills injected into the prompt
    pub skills_injected: Vec<String>,
    /// IDs of memories injected into the prompt
    pub memories_injected: Vec<String>,
}

/// Extended session metadata from subagent result
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forked_from_session_id: Option<String>,
    /// Tools used during session with counts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_used: Option<HashMap<String, u32>>,
    /// Blockers or errors encountered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockers: Option<Vec<String>>,
    /// Suggested next steps from agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_next_steps: Option<Vec<String>>,
    /// Final assistant message or summary
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Token usage from Claude API
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ApiUsage>,
    /// Total cost in USD from Claude API
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost_usd: Option<f64>,
}

/// Which agent implementation was used
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentType {
    ClaudeCode,
    Minimal,
    Fm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentResult {
    pub success: bool,
    pub subtask_id: String,
    pub files_modified: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub turns: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentType>,
    /// Session ID returned by Claude Code for resumption
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code_session_id: Option<String>,
    /// Original session ID when a forked branch was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code_forked_from_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_outputs: Option<Vec<String>>,
    /// Claude Code session metadata for progress.md bridging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_metadata: Option<SessionMetadata>,
    /// Learning metrics for FM subagent runs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub learning_metrics: Option<LearningMetrics>,
}

// ============================================================================
// Orchestrator Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestratorPhase {
    Idle,
    Orienting,
    SelectingTask,
    Decomposing,
    ExecutingSubtask,
    Verifying,
    Committing,
    UpdatingTask,
    Logging,
    Done,
    Failed,
}

impl Default for OrchestratorPhase {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorState {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtasks: Option<SubtaskList>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<SessionProgress>,
    pub phase: OrchestratorPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Minimal task representation for orchestrator state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

// ============================================================================
// Event Types
// ============================================================================

/// Usage record for token tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRecord {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorEvent {
    SessionStart {
        session_id: String,
        timestamp: String,
    },
    LockAcquired {
        pid: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    LockStaleRemoved {
        stale_pid: u32,
        new_pid: u32,
    },
    LockFailed {
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        existing_pid: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        existing_session_id: Option<String>,
    },
    LockReleased,
    InitScriptStart {
        path: String,
    },
    InitScriptComplete {
        result: InitScriptResult,
    },
    OrientationComplete {
        repo_state: String,
        tests_passing_at_start: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        init_script: Option<InitScriptResult>,
    },
    // Recovery events (two-phase commit crash recovery)
    RecoveryStart {
        pending_count: u32,
    },
    RecoveryTaskClosed {
        task_id: String,
        sha: String,
    },
    RecoveryTaskReset {
        task_id: String,
    },
    RecoveryComplete {
        closed_count: u32,
        reset_count: u32,
        failed_count: u32,
    },
    // Checkpoint events (phase checkpoint crash recovery)
    CheckpointFound {
        session_id: String,
        phase: OrchestratorPhase,
        task_id: String,
    },
    CheckpointResuming {
        phase: OrchestratorPhase,
        task_id: String,
    },
    CheckpointInvalid {
        reason: String,
    },
    CheckpointWritten {
        phase: OrchestratorPhase,
    },
    CheckpointCleared,
    TaskSelected {
        task: Task,
    },
    TaskDecomposed {
        subtasks: Vec<Subtask>,
    },
    SubtaskStart {
        subtask: Subtask,
    },
    SubtaskComplete {
        subtask: Subtask,
        result: SubagentResult,
    },
    SubtaskFailed {
        subtask: Subtask,
        error: String,
    },
    VerificationStart {
        command: String,
    },
    VerificationComplete {
        command: String,
        passed: bool,
        output: String,
    },
    VerificationOutput {
        command: String,
        chunk: String,
        stream: OutputStream,
    },
    E2eStart {
        command: String,
    },
    E2eComplete {
        command: String,
        passed: bool,
        output: String,
    },
    E2eSkipped {
        reason: String,
    },
    CommitCreated {
        sha: String,
        message: String,
    },
    PushComplete {
        branch: String,
    },
    TaskUpdated {
        task: Task,
        status: String,
    },
    ProgressWritten {
        path: String,
    },
    UsageRecorded {
        usage: UsageRecord,
    },
    SessionComplete {
        success: bool,
        summary: String,
    },
    Error {
        phase: OrchestratorPhase,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

// ============================================================================
// Minimal Subagent Prompt
// ============================================================================

/// The subagent prompt should be minimal (~50 tokens).
/// The model is RL-trained for coding - it doesn't need extensive instructions.
pub const SUBAGENT_SYSTEM_PROMPT: &str = r#"You are an expert coding assistant. Complete the subtask below.

Tools: read, write, edit, bash

When done, output: SUBTASK_COMPLETE"#;

pub fn build_subagent_prompt(subtask: &Subtask) -> String {
    format!(
        r#"## Subtask

{}

Complete this subtask. When finished, output SUBTASK_COMPLETE on its own line."#,
        subtask.description
    )
}

// ============================================================================
// Coordination File Paths
// ============================================================================

pub fn get_subtasks_path(openagents_dir: &str, task_id: &str) -> String {
    format!("{}/subtasks/{}.json", openagents_dir, task_id)
}

pub fn get_progress_path(openagents_dir: &str) -> String {
    format!("{}/progress.md", openagents_dir)
}

pub fn get_init_script_path(openagents_dir: &str) -> String {
    format!("{}/init.sh", openagents_dir)
}

pub fn get_agent_lock_path(openagents_dir: &str) -> String {
    format!("{}/agent.lock", openagents_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subtask_creation() {
        let subtask = Subtask::new("sub-1", "Implement feature X");
        assert_eq!(subtask.id, "sub-1");
        assert_eq!(subtask.description, "Implement feature X");
        assert_eq!(subtask.status, SubtaskStatus::Pending);
    }

    #[test]
    fn test_subtask_status_serialization() {
        let status = SubtaskStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"in_progress\"");
    }

    #[test]
    fn test_orchestrator_event_serialization() {
        let event = OrchestratorEvent::SessionStart {
            session_id: "sess-123".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"session_start\""));
        assert!(json.contains("\"session_id\":\"sess-123\""));
    }

    #[test]
    fn test_build_subagent_prompt() {
        let subtask = Subtask::new("sub-1", "Add error handling");
        let prompt = build_subagent_prompt(&subtask);
        assert!(prompt.contains("Add error handling"));
        assert!(prompt.contains("SUBTASK_COMPLETE"));
    }

    #[test]
    fn test_path_helpers() {
        let dir = "/home/user/.openagents";
        assert_eq!(get_subtasks_path(dir, "task-1"), "/home/user/.openagents/subtasks/task-1.json");
        assert_eq!(get_progress_path(dir), "/home/user/.openagents/progress.md");
        assert_eq!(get_init_script_path(dir), "/home/user/.openagents/init.sh");
        assert_eq!(get_agent_lock_path(dir), "/home/user/.openagents/agent.lock");
    }
}
