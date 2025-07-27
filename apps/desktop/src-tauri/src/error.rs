use std::fmt;
use thiserror::Error;

/// Application-wide error type
#[derive(Debug, Error)]
pub enum AppError {
    /// APM analysis related errors
    #[error("APM analysis error: {0}")]
    ApmError(String),

    /// Session management errors
    #[error("Session error: {0}")]
    SessionError(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Validation errors
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Claude Code specific errors
    #[error("Claude error: {0}")]
    ClaudeError(#[from] crate::claude_code::models::ClaudeError),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Generic errors
    #[error("{0}")]
    Other(String),
}

/// Convenience type alias for Results using AppError
pub type AppResult<T> = Result<T, AppError>;

/// Command result structure for Tauri commands
#[derive(serde::Serialize)]
pub struct CommandResult<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> CommandResult<T> {
    /// Create a successful command result
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Create an error command result
    pub fn error(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg),
        }
    }
}

impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.to_string()
    }
}

impl fmt::Display for CommandResult<()> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.success {
            write!(f, "Success")
        } else {
            write!(f, "Error: {}", self.error.as_ref().unwrap_or(&"Unknown error".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_display() {
        let error = AppError::ApmError("Test APM error".to_string());
        assert_eq!(error.to_string(), "APM analysis error: Test APM error");

        let error = AppError::ValidationError("Invalid input".to_string());
        assert_eq!(error.to_string(), "Validation error: Invalid input");
    }

    #[test]
    fn test_command_result() {
        let success_result = CommandResult::success("data");
        assert!(success_result.success);
        assert_eq!(success_result.data, Some("data"));
        assert!(success_result.error.is_none());

        let error_result: CommandResult<String> = CommandResult::error("Something went wrong".to_string());
        assert!(!error_result.success);
        assert!(error_result.data.is_none());
        assert_eq!(error_result.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let app_error: AppError = io_error.into();
        assert!(matches!(app_error, AppError::Io(_)));
    }
}