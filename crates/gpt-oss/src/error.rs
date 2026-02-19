use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("llama-server binary not found in PATH")]
    BinaryNotFound,

    #[error("Model file not found: {0}")]
    ModelNotFound(PathBuf),

    #[error("No model files discovered in common locations")]
    NoModelsDiscovered,

    #[error("Failed to spawn llama-server: {0}")]
    SpawnFailed(#[source] std::io::Error),

    #[error("Health check timed out after {0:?}")]
    HealthCheckTimeout(Duration),

    #[error("Server exited unexpectedly: {0}")]
    ServerExited(String),

    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),
}

#[derive(Error, Debug)]
pub enum GptOssError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("API error (HTTP {status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Initialization failed: {0}")]
    InitializationError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Harmony error: {0}")]
    HarmonyError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<GptOssError> for local_inference::LocalModelError {
    fn from(err: GptOssError) -> Self {
        match err {
            GptOssError::HttpError(e) => {
                local_inference::LocalModelError::BackendError(e.to_string())
            }
            GptOssError::ApiError { status, message } => {
                local_inference::LocalModelError::InferenceError(format!(
                    "API error (HTTP {}): {}",
                    status, message
                ))
            }
            GptOssError::JsonError(e) => {
                local_inference::LocalModelError::SerializationError(e.to_string())
            }
            GptOssError::StreamError(e) => local_inference::LocalModelError::StreamError(e),
            GptOssError::ConfigError(e) => local_inference::LocalModelError::ConfigError(e),
            GptOssError::ModelNotFound(e) => local_inference::LocalModelError::ModelNotFound(e),
            GptOssError::InitializationError(e) => {
                local_inference::LocalModelError::InitializationError(e)
            }
            GptOssError::InvalidRequest(e) => local_inference::LocalModelError::InvalidRequest(e),
            GptOssError::HarmonyError(e) => local_inference::LocalModelError::InvalidRequest(e),
            GptOssError::IoError(e) => local_inference::LocalModelError::IoError(e),
        }
    }
}

pub type Result<T> = std::result::Result<T, GptOssError>;
