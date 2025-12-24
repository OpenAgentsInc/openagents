use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid URL: {0}")]
    Url(#[from] url::ParseError),

    #[error("Server not available at {url}")]
    ServerUnavailable { url: String },

    #[error("Session not found: {id}")]
    SessionNotFound { id: String },

    #[error("Server spawn failed: {message}")]
    SpawnFailed { message: String },

    #[error("Server health check failed after {attempts} attempts")]
    HealthCheckFailed { attempts: u32 },

    #[error("Event stream error: {message}")]
    EventStream { message: String },

    #[error("Operation timeout after {seconds}s")]
    Timeout { seconds: u64 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
