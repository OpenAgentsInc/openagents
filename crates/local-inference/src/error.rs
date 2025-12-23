use thiserror::Error;

#[derive(Error, Debug)]
pub enum LocalModelError {
    #[error("Model initialization failed: {0}")]
    InitializationError(String),

    #[error("Inference failed: {0}")]
    InferenceError(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Timeout error: operation timed out")]
    Timeout,

    #[error("Backend error: {0}")]
    BackendError(String),
}
