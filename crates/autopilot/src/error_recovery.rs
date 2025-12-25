//! Automatic error recovery for common tool failures
//!
//! This module implements automatic recovery strategies for common tool errors
//! to reduce the overall tool error rate from ~15% to the target <5% per d-004.
//!
//! Recovery strategies:
//! - EISDIR (directory read attempt) → Use Glob to list directory contents
//! - ENOENT (file not found) → Search with Grep to locate the file
//! - Permission denied → Suggest alternative approaches
//! - Invalid path → Path normalization and retry

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::sleep;

/// Specific error types that can be automatically recovered
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoverableError {
    /// Attempted to read a directory (EISDIR)
    IsDirectory(PathBuf),
    /// File not found (ENOENT)
    FileNotFound(PathBuf),
    /// Permission denied
    PermissionDenied(PathBuf),
    /// Invalid path format
    InvalidPath(String),
    /// Temporary/transient error
    Transient(String),
}

/// Result of an error recovery attempt
#[derive(Debug)]
pub enum RecoveryResult {
    /// Recovery succeeded with new action to take
    Recovered(RecoveryAction),
    /// Recovery failed, cannot auto-recover
    Failed(String),
    /// Retry the same operation (for transient errors)
    Retry,
}

/// Action to take after successful recovery
#[derive(Debug)]
pub enum RecoveryAction {
    /// Use Glob to list directory contents
    UseGlob(String),
    /// Use Grep to search for file
    UseGrep {
        pattern: String,
        path: Option<PathBuf>,
    },
    /// Retry with corrected path
    RetryWithPath(PathBuf),
    /// Suggest alternative approach
    Suggest(String),
}

/// Recovery statistics for metrics tracking
#[derive(Debug, Clone)]
pub struct RecoveryStats {
    pub error_type: String,
    pub recovery_attempted: bool,
    pub recovery_succeeded: bool,
    pub recovery_action: Option<String>,
    pub retry_count: u32,
}

/// Error recovery engine
pub struct ErrorRecovery {
    /// Maximum retry attempts for transient errors
    max_retries: u32,
    /// Delay between retries (exponential backoff)
    retry_delay_ms: u64,
}

impl Default for ErrorRecovery {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 100,
        }
    }
}

impl ErrorRecovery {
    /// Create new error recovery engine with custom settings
    pub fn new(max_retries: u32, retry_delay_ms: u64) -> Self {
        Self {
            max_retries,
            retry_delay_ms,
        }
    }

    /// Attempt to recover from a tool error
    pub fn recover(&self, error: &RecoverableError) -> Result<RecoveryResult> {
        match error {
            RecoverableError::IsDirectory(path) => self.recover_eisdir(path),
            RecoverableError::FileNotFound(path) => self.recover_enoent(path),
            RecoverableError::PermissionDenied(path) => self.recover_permission(path),
            RecoverableError::InvalidPath(path_str) => self.recover_invalid_path(path_str),
            RecoverableError::Transient(_msg) => Ok(RecoveryResult::Retry),
        }
    }

    /// Recover from EISDIR error (attempted to read a directory)
    fn recover_eisdir(&self, path: &Path) -> Result<RecoveryResult> {
        // Strategy: Use Glob to list directory contents instead
        let pattern = if path.to_string_lossy().contains('*') {
            // Already a pattern, use as-is
            path.to_string_lossy().to_string()
        } else {
            // Convert directory to glob pattern
            format!("{}/*", path.display())
        };

        Ok(RecoveryResult::Recovered(RecoveryAction::UseGlob(pattern)))
    }

    /// Recover from ENOENT error (file not found)
    fn recover_enoent(&self, path: &Path) -> Result<RecoveryResult> {
        // Strategy: Search for the file using Grep
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid filename")?;

        // Extract search pattern from filename
        let search_pattern = if let Some(base) = path.file_stem().and_then(|s| s.to_str()) {
            base.to_string()
        } else {
            filename.to_string()
        };

        // Search in parent directory if available
        let search_path = path.parent().map(|p| p.to_path_buf());

        Ok(RecoveryResult::Recovered(RecoveryAction::UseGrep {
            pattern: search_pattern,
            path: search_path,
        }))
    }

    /// Recover from permission denied error
    fn recover_permission(&self, path: &Path) -> Result<RecoveryResult> {
        // Strategy: Suggest alternative approaches
        let suggestion = format!(
            "Permission denied for '{}'. Consider:\n\
            1. Check if file exists: `ls -la {}`\n\
            2. Check parent directory permissions\n\
            3. Use sudo if appropriate (with caution)\n\
            4. Verify file ownership",
            path.display(),
            path.parent().unwrap_or(path).display()
        );

        Ok(RecoveryResult::Recovered(RecoveryAction::Suggest(
            suggestion,
        )))
    }

    /// Recover from invalid path error
    fn recover_invalid_path(&self, path_str: &str) -> Result<RecoveryResult> {
        // Strategy: Normalize and fix common path issues
        let normalized = self.normalize_path(path_str)?;

        if Path::new(&normalized).exists() {
            Ok(RecoveryResult::Recovered(RecoveryAction::RetryWithPath(
                PathBuf::from(normalized),
            )))
        } else {
            Ok(RecoveryResult::Failed(format!(
                "Path '{}' is invalid and cannot be normalized",
                path_str
            )))
        }
    }

    /// Normalize path (fix common issues)
    fn normalize_path(&self, path_str: &str) -> Result<String> {
        // Remove duplicate slashes
        let cleaned = path_str.replace("//", "/");

        // Expand home directory
        let expanded = if cleaned.starts_with('~') {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
            cleaned.replacen('~', &home, 1)
        } else {
            cleaned
        };

        // Convert to absolute if relative
        let absolute = if Path::new(&expanded).is_relative() {
            let cwd = std::env::current_dir()?;
            cwd.join(&expanded).to_string_lossy().to_string()
        } else {
            expanded
        };

        Ok(absolute)
    }

    /// Retry with exponential backoff
    pub async fn retry_with_backoff<F, T, E>(&self, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Result<T, E>,
    {
        let mut attempt = 0;

        loop {
            match operation() {
                Ok(result) => return Ok(result),
                Err(err) => {
                    attempt += 1;
                    if attempt >= self.max_retries {
                        return Err(err);
                    }

                    // Exponential backoff: 100ms, 200ms, 400ms, etc.
                    let delay = self.retry_delay_ms * (2_u64.pow(attempt - 1));
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }
}

/// Parse error message to detect recoverable errors
pub fn parse_error(error_msg: &str, context_path: Option<&Path>) -> Option<RecoverableError> {
    let error_lower = error_msg.to_lowercase();

    if error_lower.contains("is a directory") || error_lower.contains("eisdir") {
        let path = context_path.map(|p| p.to_path_buf()).unwrap_or_else(|| {
            // Try to extract path from error message
            extract_path_from_error(error_msg).unwrap_or_else(|| PathBuf::from("."))
        });
        return Some(RecoverableError::IsDirectory(path));
    }

    if error_lower.contains("no such file") || error_lower.contains("enoent") {
        let path = context_path.map(|p| p.to_path_buf()).unwrap_or_else(|| {
            extract_path_from_error(error_msg).unwrap_or_else(|| PathBuf::from("unknown"))
        });
        return Some(RecoverableError::FileNotFound(path));
    }

    if error_lower.contains("permission denied") || error_lower.contains("eacces") {
        let path = context_path.map(|p| p.to_path_buf()).unwrap_or_else(|| {
            extract_path_from_error(error_msg).unwrap_or_else(|| PathBuf::from("unknown"))
        });
        return Some(RecoverableError::PermissionDenied(path));
    }

    if error_lower.contains("invalid path") || error_lower.contains("malformed") {
        return Some(RecoverableError::InvalidPath(error_msg.to_string()));
    }

    // Detect transient errors (network, timeout, etc.)
    if error_lower.contains("timeout")
        || error_lower.contains("connection")
        || error_lower.contains("temporarily")
    {
        return Some(RecoverableError::Transient(error_msg.to_string()));
    }

    None
}

/// Extract path from error message (heuristic)
fn extract_path_from_error(error_msg: &str) -> Option<PathBuf> {
    // Look for quoted paths
    if let Some(start) = error_msg.find('\'') {
        if let Some(end) = error_msg[start + 1..].find('\'') {
            return Some(PathBuf::from(&error_msg[start + 1..start + 1 + end]));
        }
    }

    // Look for paths after colons
    if let Some(colon_pos) = error_msg.find(':') {
        let after_colon = error_msg[colon_pos + 1..].trim();
        if after_colon.starts_with('/') || after_colon.starts_with("./") {
            let path_str = after_colon.split_whitespace().next()?;
            return Some(PathBuf::from(path_str));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_eisdir_error() {
        let error = "Error: Is a directory (os error 21)";
        let path = Path::new("/some/dir");
        let recoverable = parse_error(error, Some(path));

        assert!(matches!(
            recoverable,
            Some(RecoverableError::IsDirectory(_))
        ));
    }

    #[test]
    fn test_parse_enoent_error() {
        let error = "No such file or directory (os error 2)";
        let path = Path::new("/some/file.txt");
        let recoverable = parse_error(error, Some(path));

        assert!(matches!(
            recoverable,
            Some(RecoverableError::FileNotFound(_))
        ));
    }

    #[test]
    fn test_recover_eisdir() {
        let recovery = ErrorRecovery::default();
        let error = RecoverableError::IsDirectory(PathBuf::from("/some/dir"));

        let result = recovery.recover(&error).unwrap();

        match result {
            RecoveryResult::Recovered(RecoveryAction::UseGlob(pattern)) => {
                assert!(pattern.contains("/some/dir/*"));
            }
            _ => panic!("Expected UseGlob action"),
        }
    }

    #[test]
    fn test_recover_enoent() {
        let recovery = ErrorRecovery::default();
        let error = RecoverableError::FileNotFound(PathBuf::from("/some/path/file.txt"));

        let result = recovery.recover(&error).unwrap();

        match result {
            RecoveryResult::Recovered(RecoveryAction::UseGrep { pattern, .. }) => {
                assert_eq!(pattern, "file");
            }
            _ => panic!("Expected UseGrep action"),
        }
    }

    #[test]
    fn test_normalize_path() {
        let recovery = ErrorRecovery::default();

        // Test duplicate slashes
        let normalized = recovery.normalize_path("//foo//bar").unwrap();
        assert!(!normalized.contains("//"));

        // Test home directory expansion
        // SAFETY: This is a test-only operation that sets HOME env var
        unsafe {
            std::env::set_var("HOME", "/home/test");
        }
        let normalized = recovery.normalize_path("~/file.txt").unwrap();
        assert!(normalized.starts_with("/home/test"));
    }

    #[tokio::test]
    async fn test_retry_with_backoff() {
        let recovery = ErrorRecovery::new(3, 10); // Short delays for testing
        let mut attempts = 0;

        let result = recovery
            .retry_with_backoff(|| {
                attempts += 1;
                if attempts < 2 {
                    Err("Transient error")
                } else {
                    Ok("Success")
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(attempts, 2);
    }
}
