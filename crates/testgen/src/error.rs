//! Error types for TestGen

use thiserror::Error;

/// TestGen error type
#[derive(Debug, Error)]
pub enum TestGenError {
    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// FM Bridge error
    #[error("FM Bridge error: {0}")]
    FmBridge(#[from] fm_bridge::FMError),

    /// Configuration not found
    #[error("Configuration not found: {0}")]
    ConfigNotFound(String),

    /// Run not found
    #[error("Run not found: {0}")]
    RunNotFound(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Generation failed
    #[error("Generation failed: {0}")]
    GenerationFailed(String),

    /// Analysis failed
    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),

    /// Guardrail violation
    #[error("Guardrail violation: {0}")]
    GuardrailViolation(String),

    /// Parse error
    #[error("Parse error: {0}")]
    ParseError(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Timeout
    #[error("Timeout")]
    Timeout,

    /// Other error
    #[error("{0}")]
    Other(String),
}

/// Result type alias for TestGen operations
pub type Result<T> = std::result::Result<T, TestGenError>;

impl From<String> for TestGenError {
    fn from(s: String) -> Self {
        TestGenError::Other(s)
    }
}

impl From<&str> for TestGenError {
    fn from(s: &str) -> Self {
        TestGenError::Other(s.to_string())
    }
}
