//! Orchestrator Checkpoint System
//!
//! Provides crash recovery for the orchestrator by persisting state at key phase
//! transitions. When the orchestrator crashes, it can resume from the last checkpoint
//! instead of restarting from the beginning.
//!
//! Design principles:
//! - Atomic writes (write to temp file, then rename)
//! - Checkpoints expire after 24 hours
//! - Git state is captured for validation on resume

use crate::error::AgentResult;
use crate::types::OrchestratorPhase;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Checkpoint file name
pub const CHECKPOINT_FILENAME: &str = "checkpoint.json";

/// Maximum age before checkpoint is considered stale (24 hours in ms)
pub const CHECKPOINT_MAX_AGE_MS: i64 = 24 * 60 * 60 * 1000;

/// Git state captured at checkpoint time for validation on resume
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointGitState {
    pub branch: String,
    pub head_commit: String,
    pub is_dirty: bool,
    pub staged_files: Vec<String>,
}

/// Verification results captured after verify phase
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointVerification {
    pub typecheck_passed: bool,
    pub tests_passed: bool,
    pub verified_at: String,
}

/// Healer invocation record for session auditing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointHealerInvocation {
    pub scenario: String,
    pub outcome: String,
    pub timestamp: String,
}

/// Main checkpoint schema.
/// Captures orchestrator state at phase boundaries for crash recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorCheckpoint {
    /// Schema version for future compatibility
    pub version: u32,
    /// Unique session identifier
    pub session_id: String,
    /// When checkpoint was written
    pub timestamp: String,
    /// Current orchestrator phase
    pub phase: OrchestratorPhase,
    /// When current phase started
    pub phase_started_at: String,
    /// ID of task being worked on
    pub task_id: String,
    /// Title of task for human readability
    pub task_title: String,
    /// IDs of completed subtasks
    pub completed_subtask_ids: Vec<String>,
    /// ID of subtask currently being executed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_subtask_id: Option<String>,
    /// Git repository state at checkpoint
    pub git: CheckpointGitState,
    /// Verification results (populated after verify phase)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<CheckpointVerification>,
    /// Healer audit trail
    pub healer_invocations: Vec<CheckpointHealerInvocation>,
}

/// Result of checkpoint validation
#[derive(Debug, Clone)]
pub enum CheckpointValidation {
    Valid(OrchestratorCheckpoint),
    Invalid { reason: String },
}

impl CheckpointValidation {
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid(_))
    }
}

/// Get the path to the checkpoint file
pub fn get_checkpoint_path(openagents_dir: &str) -> String {
    format!("{}/{}", openagents_dir, CHECKPOINT_FILENAME)
}

/// Get the path to the temporary checkpoint file (used for atomic writes)
fn get_temp_checkpoint_path(openagents_dir: &str) -> String {
    format!("{}/{}.tmp", openagents_dir, CHECKPOINT_FILENAME)
}

/// Write checkpoint atomically (temp file + rename)
pub fn write_checkpoint(openagents_dir: &str, checkpoint: &OrchestratorCheckpoint) -> AgentResult<()> {
    let checkpoint_path = get_checkpoint_path(openagents_dir);
    let temp_path = get_temp_checkpoint_path(openagents_dir);

    // Ensure directory exists
    if let Some(parent) = Path::new(&checkpoint_path).parent() {
        fs::create_dir_all(parent)?;
    }

    // Write to temp file
    let content = serde_json::to_string_pretty(checkpoint)?;
    fs::write(&temp_path, &content)?;

    // Atomic rename
    fs::rename(&temp_path, &checkpoint_path)?;

    Ok(())
}

/// Read existing checkpoint file
pub fn read_checkpoint(openagents_dir: &str) -> Option<OrchestratorCheckpoint> {
    let checkpoint_path = get_checkpoint_path(openagents_dir);
    let path = Path::new(&checkpoint_path);

    if !path.exists() {
        return None;
    }

    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Clear checkpoint file (on successful completion)
pub fn clear_checkpoint(openagents_dir: &str) {
    let checkpoint_path = get_checkpoint_path(openagents_dir);
    let temp_path = get_temp_checkpoint_path(openagents_dir);

    let _ = fs::remove_file(&checkpoint_path);
    let _ = fs::remove_file(&temp_path);
}

/// Validate a checkpoint for resumption
pub fn validate_checkpoint(
    checkpoint: &OrchestratorCheckpoint,
    current_git_state: &CheckpointGitState,
) -> CheckpointValidation {
    // Check version
    if checkpoint.version != 1 {
        return CheckpointValidation::Invalid {
            reason: format!("Unsupported checkpoint version: {}", checkpoint.version),
        };
    }

    // Check age
    if let Ok(checkpoint_time) = DateTime::parse_from_rfc3339(&checkpoint.timestamp) {
        let age_ms = Utc::now()
            .signed_duration_since(checkpoint_time)
            .num_milliseconds();
        if age_ms > CHECKPOINT_MAX_AGE_MS {
            let hours_old = age_ms / (60 * 60 * 1000);
            return CheckpointValidation::Invalid {
                reason: format!("Checkpoint is stale ({} hours old, max 24 hours)", hours_old),
            };
        }
    }

    // Check branch matches
    if checkpoint.git.branch != current_git_state.branch {
        return CheckpointValidation::Invalid {
            reason: format!(
                "Branch mismatch: checkpoint on '{}', now on '{}'",
                checkpoint.git.branch, current_git_state.branch
            ),
        };
    }

    CheckpointValidation::Valid(checkpoint.clone())
}

/// Create a new checkpoint
pub fn create_checkpoint(
    session_id: &str,
    phase: OrchestratorPhase,
    task_id: &str,
    task_title: &str,
    completed_subtask_ids: Vec<String>,
    current_subtask_id: Option<String>,
    git: CheckpointGitState,
    verification: Option<CheckpointVerification>,
    healer_invocations: Vec<CheckpointHealerInvocation>,
) -> OrchestratorCheckpoint {
    let now = Utc::now().to_rfc3339();
    OrchestratorCheckpoint {
        version: 1,
        session_id: session_id.to_string(),
        timestamp: now.clone(),
        phase,
        phase_started_at: now,
        task_id: task_id.to_string(),
        task_title: task_title.to_string(),
        completed_subtask_ids,
        current_subtask_id,
        git,
        verification,
        healer_invocations,
    }
}

/// Update an existing checkpoint with new phase
pub fn update_checkpoint_phase(
    checkpoint: &OrchestratorCheckpoint,
    phase: OrchestratorPhase,
    updates: Option<CheckpointUpdates>,
) -> OrchestratorCheckpoint {
    let now = Utc::now().to_rfc3339();
    let updates = updates.unwrap_or_default();

    OrchestratorCheckpoint {
        version: checkpoint.version,
        session_id: checkpoint.session_id.clone(),
        timestamp: now.clone(),
        phase,
        phase_started_at: now,
        task_id: checkpoint.task_id.clone(),
        task_title: checkpoint.task_title.clone(),
        completed_subtask_ids: updates
            .completed_subtask_ids
            .unwrap_or_else(|| checkpoint.completed_subtask_ids.clone()),
        current_subtask_id: updates
            .current_subtask_id
            .or_else(|| checkpoint.current_subtask_id.clone()),
        git: updates.git.unwrap_or_else(|| checkpoint.git.clone()),
        verification: updates.verification.or_else(|| checkpoint.verification.clone()),
        healer_invocations: checkpoint.healer_invocations.clone(),
    }
}

/// Updates for checkpoint phase change
#[derive(Debug, Clone, Default)]
pub struct CheckpointUpdates {
    pub completed_subtask_ids: Option<Vec<String>>,
    pub current_subtask_id: Option<String>,
    pub git: Option<CheckpointGitState>,
    pub verification: Option<CheckpointVerification>,
}

/// Add a healer invocation to the checkpoint
pub fn add_healer_invocation(
    checkpoint: &OrchestratorCheckpoint,
    invocation: CheckpointHealerInvocation,
) -> OrchestratorCheckpoint {
    let mut new_checkpoint = checkpoint.clone();
    new_checkpoint.timestamp = Utc::now().to_rfc3339();
    new_checkpoint.healer_invocations.push(invocation);
    new_checkpoint
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_git_state() -> CheckpointGitState {
        CheckpointGitState {
            branch: "main".to_string(),
            head_commit: "abc123".to_string(),
            is_dirty: false,
            staged_files: vec![],
        }
    }

    #[test]
    fn test_get_checkpoint_path() {
        assert_eq!(
            get_checkpoint_path("/home/user/.openagents"),
            "/home/user/.openagents/checkpoint.json"
        );
    }

    #[test]
    fn test_create_checkpoint() {
        let git = make_git_state();
        let checkpoint = create_checkpoint(
            "session-1",
            OrchestratorPhase::Decomposing,
            "task-1",
            "Fix bug",
            vec!["sub-1".to_string()],
            Some("sub-2".to_string()),
            git,
            None,
            vec![],
        );

        assert_eq!(checkpoint.version, 1);
        assert_eq!(checkpoint.session_id, "session-1");
        assert_eq!(checkpoint.phase, OrchestratorPhase::Decomposing);
        assert_eq!(checkpoint.task_id, "task-1");
        assert_eq!(checkpoint.completed_subtask_ids, vec!["sub-1"]);
        assert_eq!(checkpoint.current_subtask_id, Some("sub-2".to_string()));
    }

    #[test]
    fn test_validate_checkpoint_version() {
        let git = make_git_state();
        let mut checkpoint = create_checkpoint(
            "session-1",
            OrchestratorPhase::Decomposing,
            "task-1",
            "Fix bug",
            vec![],
            None,
            git.clone(),
            None,
            vec![],
        );
        checkpoint.version = 99;

        let result = validate_checkpoint(&checkpoint, &git);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_validate_checkpoint_branch_mismatch() {
        let git = make_git_state();
        let checkpoint = create_checkpoint(
            "session-1",
            OrchestratorPhase::Decomposing,
            "task-1",
            "Fix bug",
            vec![],
            None,
            git.clone(),
            None,
            vec![],
        );

        let different_branch = CheckpointGitState {
            branch: "feature-branch".to_string(),
            ..git
        };

        let result = validate_checkpoint(&checkpoint, &different_branch);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_update_checkpoint_phase() {
        let git = make_git_state();
        let checkpoint = create_checkpoint(
            "session-1",
            OrchestratorPhase::Decomposing,
            "task-1",
            "Fix bug",
            vec![],
            None,
            git,
            None,
            vec![],
        );

        let updates = CheckpointUpdates {
            completed_subtask_ids: Some(vec!["sub-1".to_string()]),
            ..Default::default()
        };

        let updated = update_checkpoint_phase(&checkpoint, OrchestratorPhase::Verifying, Some(updates));
        assert_eq!(updated.phase, OrchestratorPhase::Verifying);
        assert_eq!(updated.completed_subtask_ids, vec!["sub-1"]);
    }

    #[test]
    fn test_add_healer_invocation() {
        let git = make_git_state();
        let checkpoint = create_checkpoint(
            "session-1",
            OrchestratorPhase::Decomposing,
            "task-1",
            "Fix bug",
            vec![],
            None,
            git,
            None,
            vec![],
        );

        let invocation = CheckpointHealerInvocation {
            scenario: "typecheck_failed".to_string(),
            outcome: "fixed".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };

        let updated = add_healer_invocation(&checkpoint, invocation);
        assert_eq!(updated.healer_invocations.len(), 1);
        assert_eq!(updated.healer_invocations[0].scenario, "typecheck_failed");
    }
}
