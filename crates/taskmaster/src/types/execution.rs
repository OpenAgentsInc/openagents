//! Execution context types for container-based agent execution
//!
//! These types track WHERE and HOW a task is being executed,
//! enabling strict container isolation for parallel agents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Where the task is being executed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Not being executed / no execution context
    #[default]
    None,
    /// Running on local machine (legacy worktree approach)
    Local,
    /// Running in isolated container (recommended)
    Container,
}

impl ExecutionMode {
    /// Returns true if this is container execution
    pub fn is_container(&self) -> bool {
        matches!(self, ExecutionMode::Container)
    }

    /// Returns true if this is local execution
    pub fn is_local(&self) -> bool {
        matches!(self, ExecutionMode::Local)
    }

    /// Returns true if no execution is happening
    pub fn is_none(&self) -> bool {
        matches!(self, ExecutionMode::None)
    }

    /// Get all possible values
    pub fn all() -> &'static [ExecutionMode] {
        &[
            ExecutionMode::None,
            ExecutionMode::Local,
            ExecutionMode::Container,
        ]
    }
}

impl fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionMode::None => write!(f, "none"),
            ExecutionMode::Local => write!(f, "local"),
            ExecutionMode::Container => write!(f, "container"),
        }
    }
}

/// Error parsing ExecutionMode from string
#[derive(Debug, Clone, thiserror::Error)]
#[error("invalid execution mode: {0}")]
pub struct ParseExecutionModeError(String);

impl FromStr for ExecutionMode {
    type Err = ParseExecutionModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" | "" => Ok(ExecutionMode::None),
            "local" => Ok(ExecutionMode::Local),
            "container" => Ok(ExecutionMode::Container),
            other => Err(ParseExecutionModeError(other.to_string())),
        }
    }
}

/// Execution state for tracking container lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    /// Not scheduled for execution
    #[default]
    Unscheduled,
    /// Queued for container execution
    Queued,
    /// Container starting (clone in progress)
    Provisioning,
    /// Agent actively working
    Running,
    /// Completed successfully
    Succeeded,
    /// Failed (see exit_code/stderr)
    Failed,
    /// Container crashed or lost connection
    Lost,
    /// Manually cancelled
    Cancelled,
}

impl ExecutionState {
    /// Returns true if execution is in progress
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            ExecutionState::Queued | ExecutionState::Provisioning | ExecutionState::Running
        )
    }

    /// Returns true if execution has completed (success or failure)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ExecutionState::Succeeded
                | ExecutionState::Failed
                | ExecutionState::Lost
                | ExecutionState::Cancelled
        )
    }

    /// Returns true if execution succeeded
    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionState::Succeeded)
    }

    /// Returns true if execution failed (any failure mode)
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            ExecutionState::Failed | ExecutionState::Lost | ExecutionState::Cancelled
        )
    }

    /// Check if transition to target state is valid
    pub fn can_transition_to(&self, target: ExecutionState) -> bool {
        use ExecutionState::*;
        match (self, target) {
            // From Unscheduled: can queue
            (Unscheduled, Queued) => true,
            (Unscheduled, Cancelled) => true,

            // From Queued: can start provisioning, or cancel
            (Queued, Provisioning) => true,
            (Queued, Cancelled) => true,
            (Queued, Lost) => true,

            // From Provisioning: can start running, fail, or lose
            (Provisioning, Running) => true,
            (Provisioning, Failed) => true,
            (Provisioning, Lost) => true,
            (Provisioning, Cancelled) => true,

            // From Running: can succeed, fail, lose, or cancel
            (Running, Succeeded) => true,
            (Running, Failed) => true,
            (Running, Lost) => true,
            (Running, Cancelled) => true,

            // Terminal states can reset to Unscheduled (retry)
            (Succeeded, Unscheduled) => true,
            (Failed, Unscheduled) => true,
            (Lost, Unscheduled) => true,
            (Cancelled, Unscheduled) => true,

            // Same state is always valid (no-op)
            (a, b) if *a == b => true,

            // Everything else is invalid
            _ => false,
        }
    }

    /// Get all possible values
    pub fn all() -> &'static [ExecutionState] {
        &[
            ExecutionState::Unscheduled,
            ExecutionState::Queued,
            ExecutionState::Provisioning,
            ExecutionState::Running,
            ExecutionState::Succeeded,
            ExecutionState::Failed,
            ExecutionState::Lost,
            ExecutionState::Cancelled,
        ]
    }
}

impl fmt::Display for ExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionState::Unscheduled => write!(f, "unscheduled"),
            ExecutionState::Queued => write!(f, "queued"),
            ExecutionState::Provisioning => write!(f, "provisioning"),
            ExecutionState::Running => write!(f, "running"),
            ExecutionState::Succeeded => write!(f, "succeeded"),
            ExecutionState::Failed => write!(f, "failed"),
            ExecutionState::Lost => write!(f, "lost"),
            ExecutionState::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Error parsing ExecutionState from string
#[derive(Debug, Clone, thiserror::Error)]
#[error("invalid execution state: {0}")]
pub struct ParseExecutionStateError(String);

impl FromStr for ExecutionState {
    type Err = ParseExecutionStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "unscheduled" | "" => Ok(ExecutionState::Unscheduled),
            "queued" => Ok(ExecutionState::Queued),
            "provisioning" => Ok(ExecutionState::Provisioning),
            "running" => Ok(ExecutionState::Running),
            "succeeded" => Ok(ExecutionState::Succeeded),
            "failed" => Ok(ExecutionState::Failed),
            "lost" => Ok(ExecutionState::Lost),
            "cancelled" | "canceled" => Ok(ExecutionState::Cancelled),
            other => Err(ParseExecutionStateError(other.to_string())),
        }
    }
}

/// Full execution context for a task
///
/// This captures all information about where and how a task is being executed.
/// Used for tracking, recovery, and debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Execution mode (local vs container)
    pub mode: ExecutionMode,
    /// Execution state (lifecycle)
    pub state: ExecutionState,
    /// Container ID (if mode == Container)
    pub container_id: Option<String>,
    /// Agent ID executing this task
    pub agent_id: Option<String>,
    /// Git branch for this execution
    pub branch: Option<String>,
    /// Remote repository URL
    pub remote_url: Option<String>,
    /// When execution started
    pub started_at: Option<DateTime<Utc>>,
    /// When execution finished
    pub finished_at: Option<DateTime<Utc>>,
    /// Exit code from container (if applicable)
    pub exit_code: Option<i32>,
    /// Stdout capture (truncated to 64KB)
    pub stdout: Option<String>,
    /// Stderr capture (truncated to 64KB)
    pub stderr: Option<String>,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            mode: ExecutionMode::None,
            state: ExecutionState::Unscheduled,
            container_id: None,
            agent_id: None,
            branch: None,
            remote_url: None,
            started_at: None,
            finished_at: None,
            exit_code: None,
            stdout: None,
            stderr: None,
        }
    }
}

impl ExecutionContext {
    /// Create a new empty execution context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context for container execution
    pub fn container(agent_id: impl Into<String>, branch: impl Into<String>) -> Self {
        Self {
            mode: ExecutionMode::Container,
            state: ExecutionState::Unscheduled,
            agent_id: Some(agent_id.into()),
            branch: Some(branch.into()),
            ..Default::default()
        }
    }

    /// Create a context for local execution
    pub fn local(agent_id: impl Into<String>) -> Self {
        Self {
            mode: ExecutionMode::Local,
            state: ExecutionState::Unscheduled,
            agent_id: Some(agent_id.into()),
            ..Default::default()
        }
    }

    /// Check if execution is active
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Check if execution has completed
    pub fn is_complete(&self) -> bool {
        self.state.is_terminal()
    }

    /// Get duration if both started_at and finished_at are set
    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.finished_at) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_mode_parse() {
        assert_eq!(
            ExecutionMode::from_str("none").unwrap(),
            ExecutionMode::None
        );
        assert_eq!(
            ExecutionMode::from_str("local").unwrap(),
            ExecutionMode::Local
        );
        assert_eq!(
            ExecutionMode::from_str("container").unwrap(),
            ExecutionMode::Container
        );
        assert_eq!(
            ExecutionMode::from_str("CONTAINER").unwrap(),
            ExecutionMode::Container
        );
        assert!(ExecutionMode::from_str("invalid").is_err());
    }

    #[test]
    fn test_execution_state_parse() {
        assert_eq!(
            ExecutionState::from_str("unscheduled").unwrap(),
            ExecutionState::Unscheduled
        );
        assert_eq!(
            ExecutionState::from_str("running").unwrap(),
            ExecutionState::Running
        );
        assert_eq!(
            ExecutionState::from_str("cancelled").unwrap(),
            ExecutionState::Cancelled
        );
        assert_eq!(
            ExecutionState::from_str("canceled").unwrap(),
            ExecutionState::Cancelled
        );
        assert!(ExecutionState::from_str("invalid").is_err());
    }

    #[test]
    fn test_execution_state_transitions() {
        use ExecutionState::*;

        // Valid transitions
        assert!(Unscheduled.can_transition_to(Queued));
        assert!(Queued.can_transition_to(Provisioning));
        assert!(Provisioning.can_transition_to(Running));
        assert!(Running.can_transition_to(Succeeded));
        assert!(Running.can_transition_to(Failed));

        // Invalid transitions
        assert!(!Unscheduled.can_transition_to(Running));
        assert!(!Succeeded.can_transition_to(Running));
        assert!(!Failed.can_transition_to(Succeeded));

        // Retry (terminal -> unscheduled)
        assert!(Failed.can_transition_to(Unscheduled));
        assert!(Lost.can_transition_to(Unscheduled));
    }

    #[test]
    fn test_execution_state_is_active() {
        assert!(!ExecutionState::Unscheduled.is_active());
        assert!(ExecutionState::Queued.is_active());
        assert!(ExecutionState::Provisioning.is_active());
        assert!(ExecutionState::Running.is_active());
        assert!(!ExecutionState::Succeeded.is_active());
        assert!(!ExecutionState::Failed.is_active());
    }

    #[test]
    fn test_execution_context_container() {
        let ctx = ExecutionContext::container("agent-0", "agent/agent-0");
        assert_eq!(ctx.mode, ExecutionMode::Container);
        assert_eq!(ctx.agent_id, Some("agent-0".to_string()));
        assert_eq!(ctx.branch, Some("agent/agent-0".to_string()));
    }
}
