//! Error types for the telemetry crate.

use thiserror::Error;

/// Errors that can occur during telemetry operations.
#[derive(Error, Debug)]
pub enum TelemetryError {
    /// Failed to initialize the global subscriber.
    #[error("failed to set global default subscriber: {0}")]
    SetGlobalDefault(#[from] tracing::subscriber::SetGlobalDefaultError),

    /// Invalid correlation ID format.
    #[error("invalid correlation ID: {0}")]
    InvalidCorrelationId(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// OpenTelemetry initialization error (only available with `otel` feature).
    #[cfg(feature = "otel")]
    #[error("OpenTelemetry error: {0}")]
    OpenTelemetry(String),
}

/// Result type for telemetry operations.
pub type Result<T> = std::result::Result<T, TelemetryError>;
