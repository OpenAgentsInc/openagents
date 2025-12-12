//! Error types for repository operations

use crate::types::IssueStatus;
use thiserror::Error;

/// Errors that can occur in taskmaster operations
#[derive(Error, Debug)]
pub enum TaskmasterError {
    /// Issue not found
    #[error("issue not found: {0}")]
    NotFound(String),

    /// Issue already exists
    #[error("issue already exists: {0}")]
    AlreadyExists(String),

    /// Invalid issue data
    #[error("invalid issue data: {0}")]
    ValidationError(String),

    /// Invalid state transition
    #[error("invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: IssueStatus, to: IssueStatus },

    /// Dependency cycle detected
    #[error("dependency cycle detected: {0}")]
    CycleDetected(String),

    /// Dependency not found
    #[error("dependency not found: {issue_id} -> {dep_id}")]
    DependencyNotFound { issue_id: String, dep_id: String },

    /// Database error
    #[error("database error: {0}")]
    DatabaseError(String),

    /// Migration error
    #[error("migration error: {0}")]
    MigrationError(String),

    /// IO error
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON serialization error
    #[error("json error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// SQLite error
    #[error("sqlite error: {0}")]
    SqliteError(#[from] rusqlite::Error),
}

impl TaskmasterError {
    /// Create a not found error
    pub fn not_found(id: impl Into<String>) -> Self {
        TaskmasterError::NotFound(id.into())
    }

    /// Create an already exists error
    pub fn already_exists(id: impl Into<String>) -> Self {
        TaskmasterError::AlreadyExists(id.into())
    }

    /// Create a validation error
    pub fn validation(msg: impl Into<String>) -> Self {
        TaskmasterError::ValidationError(msg.into())
    }

    /// Create an invalid state transition error
    pub fn invalid_transition(from: IssueStatus, to: IssueStatus) -> Self {
        TaskmasterError::InvalidStateTransition { from, to }
    }

    /// Create a cycle detected error
    pub fn cycle_detected(msg: impl Into<String>) -> Self {
        TaskmasterError::CycleDetected(msg.into())
    }

    /// Create a database error
    pub fn database(msg: impl Into<String>) -> Self {
        TaskmasterError::DatabaseError(msg.into())
    }

    /// Create a migration error
    pub fn migration(msg: impl Into<String>) -> Self {
        TaskmasterError::MigrationError(msg.into())
    }

    /// Check if this is a not found error
    pub fn is_not_found(&self) -> bool {
        matches!(self, TaskmasterError::NotFound(_))
    }

    /// Check if this is an already exists error
    pub fn is_already_exists(&self) -> bool {
        matches!(self, TaskmasterError::AlreadyExists(_))
    }
}

/// Result type for taskmaster operations
pub type Result<T> = std::result::Result<T, TaskmasterError>;
