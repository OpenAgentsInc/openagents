use thiserror::Error;

#[derive(Error, Debug)]
pub enum AtifError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("Invalid step sequence: {0}")]
    InvalidStepSequence(String),

    #[error("Tool call reference error: {0}")]
    ToolCallReferenceError(String),

    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
