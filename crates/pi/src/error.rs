//! Pi agent error types

use thiserror::Error;

/// Errors that can occur during Pi agent operation
#[derive(Error, Debug)]
pub enum PiError {
    /// LLM provider error
    #[error("LLM error: {0}")]
    Llm(#[from] llm::LlmError),

    /// Tool execution error
    #[error("Tool error: {tool_name}: {message}")]
    Tool { tool_name: String, message: String },

    /// Tool not found
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Session error
    #[error("Session error: {0}")]
    Session(String),

    /// Context overflow
    #[error("Context overflow: {current_tokens} tokens exceeds limit of {max_tokens}")]
    ContextOverflow { current_tokens: u32, max_tokens: u32 },

    /// Max turns exceeded
    #[error("Max turns exceeded: {0}")]
    MaxTurnsExceeded(u32),

    /// Operation cancelled
    #[error("Operation cancelled")]
    Cancelled,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Other error
    #[error("{0}")]
    Other(String),
}

/// Result type for Pi agent operations
pub type PiResult<T> = Result<T, PiError>;

impl PiError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            PiError::Llm(e) => {
                // Check common retryable LLM error patterns
                matches!(
                    e,
                    llm::LlmError::RateLimitError(_)
                        | llm::LlmError::TimeoutError(_)
                        | llm::LlmError::NetworkError(_)
                )
            }
            PiError::Cancelled => false,
            PiError::MaxTurnsExceeded(_) => false,
            PiError::ContextOverflow { .. } => false,
            PiError::Config(_) => false,
            PiError::ToolNotFound(_) => false,
            // IO and tool errors might be transient
            PiError::Io(_) => true,
            PiError::Tool { .. } => false,
            PiError::Session(_) => false,
            PiError::Json(_) => false,
            PiError::Other(_) => false,
        }
    }

    /// Create a tool error
    pub fn tool(name: impl Into<String>, message: impl Into<String>) -> Self {
        PiError::Tool {
            tool_name: name.into(),
            message: message.into(),
        }
    }
}
