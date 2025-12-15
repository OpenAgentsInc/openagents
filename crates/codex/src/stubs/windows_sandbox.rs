//! Stub Windows sandbox implementation
//!
//! This is a placeholder for Windows sandbox functionality.
//! For full Windows sandbox support, implement proper Windows-specific sandboxing.

use std::io;
use std::path::Path;

/// Result type for sandbox operations
pub type SandboxResult<T> = Result<T, SandboxError>;

/// Error type for sandbox operations
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("Windows sandbox not supported in stub implementation")]
    NotSupported,
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Capture output from a sandboxed command (stub)
pub fn run_windows_sandbox_capture(
    _cmd: &str,
    _args: &[&str],
    _cwd: &Path,
) -> SandboxResult<String> {
    Err(SandboxError::NotSupported)
}

/// Capture output from an elevated sandboxed command (stub)
pub fn run_windows_sandbox_capture_elevated(
    _cmd: &str,
    _args: &[&str],
    _cwd: &Path,
) -> SandboxResult<String> {
    Err(SandboxError::NotSupported)
}
