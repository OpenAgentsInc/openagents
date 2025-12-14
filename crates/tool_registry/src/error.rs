//! Error types for tool execution.

use thiserror::Error;

/// Errors that can occur during tool execution.
#[derive(Debug, Error)]
pub enum ToolError {
    /// The tool was cancelled via the cancellation token.
    #[error("Tool execution was cancelled")]
    Cancelled,

    /// Permission was denied for the tool operation.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Invalid input parameters.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// The requested resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// A command or operation failed.
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Timeout waiting for operation.
    #[error("Operation timed out after {0}ms")]
    Timeout(u64),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Generic error with message.
    #[error("{0}")]
    Other(String),
}

impl ToolError {
    /// Create a permission denied error.
    pub fn permission_denied(msg: impl Into<String>) -> Self {
        Self::PermissionDenied(msg.into())
    }

    /// Create an invalid input error.
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a not found error.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create an execution failed error.
    pub fn execution_failed(msg: impl Into<String>) -> Self {
        Self::ExecutionFailed(msg.into())
    }

    /// Create a timeout error.
    pub fn timeout(ms: u64) -> Self {
        Self::Timeout(ms)
    }

    /// Create a generic error.
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Check if this error indicates the tool was cancelled.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Check if this error indicates permission was denied.
    pub fn is_permission_denied(&self) -> bool {
        matches!(self, Self::PermissionDenied(_))
    }

    /// Convert to a user-friendly error message.
    pub fn user_message(&self) -> String {
        match self {
            Self::Cancelled => "The operation was cancelled.".to_string(),
            Self::PermissionDenied(msg) => format!("Permission denied: {}", msg),
            Self::InvalidInput(msg) => format!("Invalid input: {}", msg),
            Self::NotFound(msg) => format!("Not found: {}", msg),
            Self::ExecutionFailed(msg) => format!("Execution failed: {}", msg),
            Self::Timeout(ms) => format!("Operation timed out after {}ms", ms),
            Self::Io(e) => format!("I/O error: {}", e),
            Self::Json(e) => format!("JSON error: {}", e),
            Self::Other(msg) => msg.clone(),
        }
    }
}

/// Result type for tool operations.
pub type ToolResult<T> = Result<T, ToolError>;

/// Convert from the legacy tools crate error.
impl From<tools::ToolError> for ToolError {
    fn from(err: tools::ToolError) -> Self {
        use tools::ToolErrorReason;
        match err.reason {
            ToolErrorReason::InvalidArguments => Self::InvalidInput(err.message),
            ToolErrorReason::NotFound => Self::NotFound(err.message),
            ToolErrorReason::MissingOldText => Self::InvalidInput(err.message),
            ToolErrorReason::NotUnique => Self::InvalidInput(err.message),
            ToolErrorReason::Unchanged => Self::InvalidInput(err.message),
            ToolErrorReason::CommandFailed => Self::ExecutionFailed(err.message),
            ToolErrorReason::Aborted => Self::Cancelled,
            ToolErrorReason::PermissionDenied => Self::PermissionDenied(err.message),
            ToolErrorReason::IoError => Self::Other(err.message),
        }
    }
}
