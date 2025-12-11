//! Error types for the executor module.

use std::fmt;

/// Errors that can occur during executor operations.
#[derive(Debug)]
pub enum ExecutorError {
    /// HTTP request failed
    Http(String),

    /// WebSocket error
    WebSocket(String),

    /// Connection failed
    Connection(String),

    /// Operation timed out
    Timeout(String),

    /// Protocol error (e.g., invalid NIP-01 message)
    Protocol(String),

    /// Executor is shutting down
    ShuttingDown,

    /// Service not attached to executor manager
    NotAttached(String),

    /// Runtime error (e.g., tokio runtime issue)
    Runtime(String),

    /// Configuration error
    Config(String),
}

impl fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutorError::Http(msg) => write!(f, "HTTP request failed: {}", msg),
            ExecutorError::WebSocket(msg) => write!(f, "WebSocket error: {}", msg),
            ExecutorError::Connection(msg) => write!(f, "Connection failed: {}", msg),
            ExecutorError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            ExecutorError::Protocol(msg) => write!(f, "Protocol error: {}", msg),
            ExecutorError::ShuttingDown => write!(f, "Executor is shutting down"),
            ExecutorError::NotAttached(service) => {
                write!(f, "Service not attached: {}", service)
            }
            ExecutorError::Runtime(msg) => write!(f, "Runtime error: {}", msg),
            ExecutorError::Config(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for ExecutorError {}

#[cfg(feature = "net-executor")]
impl From<reqwest::Error> for ExecutorError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ExecutorError::Timeout(err.to_string())
        } else if err.is_connect() {
            ExecutorError::Connection(err.to_string())
        } else {
            ExecutorError::Http(err.to_string())
        }
    }
}

#[cfg(feature = "net-executor")]
impl From<tokio_tungstenite::tungstenite::Error> for ExecutorError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        ExecutorError::WebSocket(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ExecutorError::Http("connection refused".to_string());
        assert_eq!(
            format!("{}", err),
            "HTTP request failed: connection refused"
        );

        let err = ExecutorError::ShuttingDown;
        assert_eq!(format!("{}", err), "Executor is shutting down");

        let err = ExecutorError::NotAttached("HttpFs".to_string());
        assert_eq!(format!("{}", err), "Service not attached: HttpFs");
    }
}
