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

    // Container-related errors

    /// Container not available (not provisioned or lost)
    #[error("Container not available: {0}")]
    ContainerNotAvailable(String),

    /// Container start failed
    #[error("Container start failed: {0}")]
    ContainerStartFailed(String),

    /// Container execution failed
    #[error("Container execution failed: {0}")]
    ContainerExecutionFailed(String),

    /// Git clone failed in container
    #[error("Git clone failed for {url}: {message}")]
    CloneFailed { url: String, message: String },

    /// Git push failed from container
    #[error("Git push failed to {branch}: {message}")]
    PushFailed { branch: String, message: String },

    /// Container lost (crashed, OOM, etc.)
    #[error("Container lost: {agent_id}")]
    ContainerLost { agent_id: String },

    /// Credential extraction failed
    #[error("Credential extraction failed: {0}")]
    CredentialError(String),
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

    /// Create a container not available error
    pub fn container_not_available(msg: impl Into<String>) -> Self {
        ParallelError::ContainerNotAvailable(msg.into())
    }

    /// Create a container start failed error
    pub fn container_start_failed(msg: impl Into<String>) -> Self {
        ParallelError::ContainerStartFailed(msg.into())
    }

    /// Create a container execution failed error
    pub fn container_execution_failed(msg: impl Into<String>) -> Self {
        ParallelError::ContainerExecutionFailed(msg.into())
    }

    /// Create a clone failed error
    pub fn clone_failed(url: impl Into<String>, message: impl Into<String>) -> Self {
        ParallelError::CloneFailed {
            url: url.into(),
            message: message.into(),
        }
    }

    /// Create a push failed error
    pub fn push_failed(branch: impl Into<String>, message: impl Into<String>) -> Self {
        ParallelError::PushFailed {
            branch: branch.into(),
            message: message.into(),
        }
    }

    /// Create a container lost error
    pub fn container_lost(agent_id: impl Into<String>) -> Self {
        ParallelError::ContainerLost {
            agent_id: agent_id.into(),
        }
    }

    /// Create a credential error
    pub fn credential_error(msg: impl Into<String>) -> Self {
        ParallelError::CredentialError(msg.into())
    }
}
