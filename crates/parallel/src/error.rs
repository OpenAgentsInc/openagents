//! Error types for parallel execution

use thiserror::Error;

/// Result type for parallel operations
pub type ParallelResult<T> = Result<T, ParallelError>;

/// Errors that can occur during parallel execution
#[derive(Error, Debug)]
pub enum ParallelError {
    /// Git operation failed
    #[error("Git error: {0}")]
    GitError(#[from] git2::Error),

    /// Worktree operation failed
    #[error("Worktree error: {0}")]
    WorktreeError(String),

    /// Agent execution failed
    #[error("Agent error: {0}")]
    AgentError(String),

    /// Merge conflict detected
    #[error("Merge conflict in {files:?}")]
    MergeConflict { files: Vec<String> },

    /// Task assignment failed
    #[error("Task assignment error: {0}")]
    TaskAssignmentError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Orchestrator error
    #[error("Orchestrator error: {0}")]
    OrchestratorError(#[from] orchestrator::OrchestratorError),

    /// All agents failed
    #[error("All agents failed")]
    AllAgentsFailed,

    /// Timeout waiting for agents
    #[error("Timeout waiting for agents")]
    Timeout,

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

impl ParallelError {
    /// Create a worktree error
    pub fn worktree(msg: impl Into<String>) -> Self {
        ParallelError::WorktreeError(msg.into())
    }

    /// Create an agent error
    pub fn agent(msg: impl Into<String>) -> Self {
        ParallelError::AgentError(msg.into())
    }

    /// Create a merge conflict error
    pub fn merge_conflict(files: Vec<String>) -> Self {
        ParallelError::MergeConflict { files }
    }
}
