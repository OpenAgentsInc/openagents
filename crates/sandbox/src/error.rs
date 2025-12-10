//! Error types for sandbox operations

use thiserror::Error;

/// Container error reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerErrorReason {
    /// Container runtime not installed or running
    NotAvailable,
    /// Specified image doesn't exist
    ImageNotFound,
    /// Container failed to start
    StartFailed,
    /// Command inside container failed
    ExecutionFailed,
    /// Operation timed out
    Timeout,
    /// User/signal aborted
    Aborted,
}

impl std::fmt::Display for ContainerErrorReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "not_available"),
            Self::ImageNotFound => write!(f, "image_not_found"),
            Self::StartFailed => write!(f, "start_failed"),
            Self::ExecutionFailed => write!(f, "execution_failed"),
            Self::Timeout => write!(f, "timeout"),
            Self::Aborted => write!(f, "aborted"),
        }
    }
}

/// Container execution errors
#[derive(Debug, Error)]
#[error("[{reason}] {message}")]
pub struct ContainerError {
    /// The error reason
    pub reason: ContainerErrorReason,
    /// Human-readable error message
    pub message: String,
    /// Exit code if available
    pub exit_code: Option<i32>,
}

impl ContainerError {
    /// Create a new container error
    pub fn new(reason: ContainerErrorReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
            exit_code: None,
        }
    }

    /// Create a container error with exit code
    pub fn with_exit_code(
        reason: ContainerErrorReason,
        message: impl Into<String>,
        exit_code: i32,
    ) -> Self {
        Self {
            reason,
            message: message.into(),
            exit_code: Some(exit_code),
        }
    }

    /// Container runtime not available
    pub fn not_available(message: impl Into<String>) -> Self {
        Self::new(ContainerErrorReason::NotAvailable, message)
    }

    /// Image not found
    pub fn image_not_found(message: impl Into<String>) -> Self {
        Self::new(ContainerErrorReason::ImageNotFound, message)
    }

    /// Container failed to start
    pub fn start_failed(message: impl Into<String>) -> Self {
        Self::new(ContainerErrorReason::StartFailed, message)
    }

    /// Command execution failed
    pub fn execution_failed(message: impl Into<String>, exit_code: Option<i32>) -> Self {
        Self {
            reason: ContainerErrorReason::ExecutionFailed,
            message: message.into(),
            exit_code,
        }
    }

    /// Operation timed out
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(ContainerErrorReason::Timeout, message)
    }

    /// Operation aborted
    pub fn aborted(message: impl Into<String>) -> Self {
        Self::new(ContainerErrorReason::Aborted, message)
    }
}

/// Credential error reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialErrorReason {
    /// Credentials not found
    NotFound,
    /// Access denied to credentials
    AccessDenied,
    /// Invalid credential format
    InvalidFormat,
    /// Failed to extract credentials
    ExtractionFailed,
}

impl std::fmt::Display for CredentialErrorReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "not_found"),
            Self::AccessDenied => write!(f, "access_denied"),
            Self::InvalidFormat => write!(f, "invalid_format"),
            Self::ExtractionFailed => write!(f, "extraction_failed"),
        }
    }
}

/// Credential extraction errors
#[derive(Debug, Error)]
#[error("[{reason}] {message}")]
pub struct CredentialError {
    /// The error reason
    pub reason: CredentialErrorReason,
    /// Human-readable error message
    pub message: String,
}

impl CredentialError {
    /// Create a new credential error
    pub fn new(reason: CredentialErrorReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    /// Credentials not found
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(CredentialErrorReason::NotFound, message)
    }

    /// Access denied
    pub fn access_denied(message: impl Into<String>) -> Self {
        Self::new(CredentialErrorReason::AccessDenied, message)
    }

    /// Invalid format
    pub fn invalid_format(message: impl Into<String>) -> Self {
        Self::new(CredentialErrorReason::InvalidFormat, message)
    }

    /// Extraction failed
    pub fn extraction_failed(message: impl Into<String>) -> Self {
        Self::new(CredentialErrorReason::ExtractionFailed, message)
    }
}

/// Result type for container operations
pub type ContainerResult<T> = Result<T, ContainerError>;

/// Result type for credential operations
pub type CredentialResult<T> = Result<T, CredentialError>;
