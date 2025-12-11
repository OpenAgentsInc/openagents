//! Orchestrator Crash Recovery
//!
//! Handles recovery from crashes that occur during the two-phase commit pattern.
//! When the orchestrator crashes between creating a git commit and updating the task,
//! the task remains in "commit_pending" status with a pendingCommit record.
//!
//! On restart, this module:
//! 1. Finds all tasks in "commit_pending" status
//! 2. Checks if the recorded commit SHA exists in git history
//! 3. If commit exists: completes the transition to "closed"
//! 4. If commit doesn't exist: resets to "in_progress" for retry

use crate::error::AgentResult;
use crate::git::run_git;
use crate::types::OrchestratorEvent;
use serde::{Deserialize, Serialize};

/// Recovery result for a batch of tasks
#[derive(Debug, Clone, Default)]
pub struct RecoveryResult {
    /// Number of tasks successfully closed (commit existed)
    pub closed_count: u32,
    /// Number of tasks reset to in_progress (commit didn't exist)
    pub reset_count: u32,
    /// Number of tasks that failed recovery
    pub failed_count: u32,
    /// Details of each recovery action
    pub details: Vec<RecoveryAction>,
}

/// Individual recovery action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAction {
    pub task_id: String,
    pub action: RecoveryActionType,
    pub commit_sha: Option<String>,
    pub error: Option<String>,
}

/// Type of recovery action taken
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryActionType {
    Closed,
    Reset,
    Failed,
}

/// Pending commit record for a task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingCommit {
    pub sha: Option<String>,
    pub message: Option<String>,
    pub timestamp: String,
}

/// Recovery options
#[derive(Debug, Clone)]
pub struct RecoveryOptions {
    /// Working directory for git operations
    pub cwd: String,
}

impl Default for RecoveryOptions {
    fn default() -> Self {
        Self {
            cwd: ".".to_string(),
        }
    }
}

/// Check if a commit SHA exists in the git repository
pub fn commit_exists(sha: &str, cwd: &str) -> bool {
    let result = run_git(cwd, &["cat-file", "-t", sha]);
    result.exit_code == 0
}

/// Recover a single pending commit
pub fn recover_pending_commit(
    task_id: &str,
    pending: &PendingCommit,
    options: &RecoveryOptions,
    mut emit: Option<impl FnMut(OrchestratorEvent)>,
) -> RecoveryAction {
    // Check if we have a SHA to verify
    if let Some(ref sha) = pending.sha {
        if commit_exists(sha, &options.cwd) {
            // Commit exists - can safely close the task
            if let Some(ref mut emit_fn) = emit {
                emit_fn(OrchestratorEvent::RecoveryTaskClosed {
                    task_id: task_id.to_string(),
                    sha: sha.clone(),
                });
            }
            return RecoveryAction {
                task_id: task_id.to_string(),
                action: RecoveryActionType::Closed,
                commit_sha: Some(sha.clone()),
                error: None,
            };
        }
    }

    // No SHA or commit doesn't exist - reset to in_progress
    if let Some(ref mut emit_fn) = emit {
        emit_fn(OrchestratorEvent::RecoveryTaskReset {
            task_id: task_id.to_string(),
        });
    }

    RecoveryAction {
        task_id: task_id.to_string(),
        action: RecoveryActionType::Reset,
        commit_sha: pending.sha.clone(),
        error: None,
    }
}

/// Recover multiple pending commits
pub fn recover_all_pending(
    pending_tasks: &[(String, PendingCommit)],
    options: &RecoveryOptions,
    mut emit: Option<impl FnMut(OrchestratorEvent)>,
) -> RecoveryResult {
    if let Some(ref mut emit_fn) = emit {
        emit_fn(OrchestratorEvent::RecoveryStart {
            pending_count: pending_tasks.len() as u32,
        });
    }

    let mut result = RecoveryResult::default();

    for (task_id, pending) in pending_tasks {
        let action = recover_pending_commit(task_id, pending, options, emit.as_mut());

        match action.action {
            RecoveryActionType::Closed => result.closed_count += 1,
            RecoveryActionType::Reset => result.reset_count += 1,
            RecoveryActionType::Failed => result.failed_count += 1,
        }

        result.details.push(action);
    }

    if let Some(ref mut emit_fn) = emit {
        emit_fn(OrchestratorEvent::RecoveryComplete {
            closed_count: result.closed_count,
            reset_count: result.reset_count,
            failed_count: result.failed_count,
        });
    }

    result
}

/// Check if there are any pending commits that need recovery
pub fn has_pending_recovery(pending_tasks: &[(String, PendingCommit)]) -> bool {
    !pending_tasks.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_result_default() {
        let result = RecoveryResult::default();
        assert_eq!(result.closed_count, 0);
        assert_eq!(result.reset_count, 0);
        assert_eq!(result.failed_count, 0);
    }

    #[test]
    fn test_has_pending_recovery() {
        assert!(!has_pending_recovery(&[]));

        let pending = vec![(
            "task-1".to_string(),
            PendingCommit {
                sha: Some("abc123".to_string()),
                message: Some("Test".to_string()),
                timestamp: "2025-01-01T00:00:00Z".to_string(),
            },
        )];
        assert!(has_pending_recovery(&pending));
    }

    #[test]
    fn test_recovery_action_type() {
        let action = RecoveryAction {
            task_id: "task-1".to_string(),
            action: RecoveryActionType::Closed,
            commit_sha: Some("abc123".to_string()),
            error: None,
        };
        assert_eq!(action.action, RecoveryActionType::Closed);
    }

    #[test]
    fn test_recovery_options_default() {
        let opts = RecoveryOptions::default();
        assert_eq!(opts.cwd, ".");
    }
}
