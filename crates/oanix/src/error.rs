//! OANIX error types

use thiserror::Error;

/// Errors that can occur in OANIX operations
#[derive(Error, Debug)]
pub enum OanixError {
    /// Filesystem operation failed
    #[error("filesystem error: {0}")]
    Filesystem(#[from] FsError),

    /// WASI execution error
    #[error("WASI error: {0}")]
    Wasi(String),

    /// Job execution error
    #[error("job error: {0}")]
    Job(String),

    /// Namespace configuration error
    #[error("namespace error: {0}")]
    Namespace(String),

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
}

/// Filesystem-specific errors
#[derive(Error, Debug)]
pub enum FsError {
    /// File or directory not found
    #[error("not found: {0}")]
    NotFound(String),

    /// Permission denied
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Path already exists
    #[error("already exists: {0}")]
    AlreadyExists(String),

    /// Not a directory
    #[error("not a directory: {0}")]
    NotADirectory(String),

    /// Not a file
    #[error("not a file: {0}")]
    NotAFile(String),

    /// IO error
    #[error("io error: {0}")]
    Io(String),

    /// Filesystem is read-only
    #[error("filesystem is read-only")]
    ReadOnly,
}
