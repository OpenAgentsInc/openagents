//! Error types for tools

use thiserror::Error;

/// Tool execution error reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolErrorReason {
    /// Invalid input arguments
    InvalidArguments,
    /// File or resource not found
    NotFound,
    /// Edit old_text not found in file
    MissingOldText,
    /// Edit old_text matches multiple locations (needs replace_all)
    NotUnique,
    /// Edit would result in no change
    Unchanged,
    /// Command execution failed
    CommandFailed,
    /// Operation was aborted
    Aborted,
    /// Permission denied
    PermissionDenied,
    /// IO error
    IoError,
}

/// Tool execution error
#[derive(Error, Debug)]
#[error("{reason:?}: {message}")]
pub struct ToolError {
    /// Error reason category
    pub reason: ToolErrorReason,
    /// Detailed error message
    pub message: String,
}

impl ToolError {
    pub fn new(reason: ToolErrorReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    pub fn invalid_arguments(msg: impl Into<String>) -> Self {
        Self::new(ToolErrorReason::InvalidArguments, msg)
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(ToolErrorReason::NotFound, msg)
    }

    pub fn missing_old_text(msg: impl Into<String>) -> Self {
        Self::new(ToolErrorReason::MissingOldText, msg)
    }

    pub fn not_unique(msg: impl Into<String>) -> Self {
        Self::new(ToolErrorReason::NotUnique, msg)
    }

    pub fn unchanged(msg: impl Into<String>) -> Self {
        Self::new(ToolErrorReason::Unchanged, msg)
    }

    pub fn command_failed(msg: impl Into<String>) -> Self {
        Self::new(ToolErrorReason::CommandFailed, msg)
    }

    pub fn aborted(msg: impl Into<String>) -> Self {
        Self::new(ToolErrorReason::Aborted, msg)
    }

    pub fn permission_denied(msg: impl Into<String>) -> Self {
        Self::new(ToolErrorReason::PermissionDenied, msg)
    }

    pub fn io_error(msg: impl Into<String>) -> Self {
        Self::new(ToolErrorReason::IoError, msg)
    }
}

pub type ToolResult<T> = Result<T, ToolError>;
