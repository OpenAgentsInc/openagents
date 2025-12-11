//! Claude Code Availability Detection
//!
//! Detects whether Claude Code CLI and SDK are available for use.
//! Checks for the `claude` CLI binary which authenticates via Claude Max subscription.

use std::process::Command;

/// Result of Claude Code availability check
#[derive(Debug, Clone)]
pub struct ClaudeCodeAvailability {
    /// Whether Claude Code is available for use
    pub available: bool,
    /// Version string if available
    pub version: Option<String>,
    /// Path to the CLI binary if found
    pub cli_path: Option<String>,
    /// Reason if not available
    pub reason: Option<String>,
}

impl ClaudeCodeAvailability {
    /// Create an available result
    pub fn ok() -> Self {
        Self {
            available: true,
            version: None,
            cli_path: None,
            reason: None,
        }
    }

    /// Create an unavailable result with reason
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            version: None,
            cli_path: None,
            reason: Some(reason.into()),
        }
    }

    /// Set the CLI path
    pub fn with_cli_path(mut self, path: impl Into<String>) -> Self {
        self.cli_path = Some(path.into());
        self
    }

    /// Set the version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}

/// Options for detecting Claude Code
#[derive(Debug, Clone, Default)]
pub struct DetectClaudeCodeOptions {
    /// Whether to run an optional health check
    pub health_check: bool,
}

/// Check if the Claude CLI is available
fn check_cli() -> (bool, Option<String>) {
    let result = Command::new("which").arg("claude").output();

    match result {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (true, Some(path))
        }
        _ => (false, None),
    }
}

/// Get the Claude CLI version
fn get_cli_version() -> Option<String> {
    let result = Command::new("claude").arg("--version").output();

    match result {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // Parse version from output like "claude 1.0.0"
            let version = version
                .split_whitespace()
                .last()
                .map(|s| s.to_string())
                .unwrap_or(version);
            Some(version)
        }
        _ => None,
    }
}

/// Run a basic health check on Claude Code
fn run_health_check() -> Result<(), String> {
    // Try to run claude with a simple command
    let result = Command::new("claude").arg("--help").output();

    match result {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Claude health check failed: {}", stderr.trim()))
        }
        Err(e) => Err(format!("Failed to run claude: {}", e)),
    }
}

/// Detect whether Claude Code is available for use
pub fn detect_claude_code(options: &DetectClaudeCodeOptions) -> ClaudeCodeAvailability {
    // Check for claude CLI binary
    let (cli_available, cli_path) = check_cli();

    if !cli_available {
        return ClaudeCodeAvailability::unavailable(
            "Claude CLI not found. Install: npm install -g @anthropic-ai/claude-code",
        );
    }

    // Get version
    let version = get_cli_version();

    // Run health check if requested
    if options.health_check {
        if let Err(reason) = run_health_check() {
            let mut result = ClaudeCodeAvailability::unavailable(reason);
            if let Some(path) = cli_path {
                result = result.with_cli_path(path);
            }
            if let Some(v) = version {
                result = result.with_version(v);
            }
            return result;
        }
    }

    // All checks passed
    let mut result = ClaudeCodeAvailability::ok();
    if let Some(path) = cli_path {
        result = result.with_cli_path(path);
    }
    if let Some(v) = version {
        result = result.with_version(v);
    }
    result
}

/// Quick check if Claude Code is available (no health check)
pub fn is_claude_code_available() -> bool {
    let (available, _) = check_cli();
    available
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_availability_ok() {
        let result = ClaudeCodeAvailability::ok();
        assert!(result.available);
        assert!(result.reason.is_none());
    }

    #[test]
    fn test_availability_unavailable() {
        let result = ClaudeCodeAvailability::unavailable("not installed");
        assert!(!result.available);
        assert_eq!(result.reason, Some("not installed".to_string()));
    }

    #[test]
    fn test_availability_with_cli_path() {
        let result = ClaudeCodeAvailability::ok().with_cli_path("/usr/local/bin/claude");
        assert!(result.available);
        assert_eq!(result.cli_path, Some("/usr/local/bin/claude".to_string()));
    }

    #[test]
    fn test_availability_with_version() {
        let result = ClaudeCodeAvailability::ok().with_version("1.0.0");
        assert!(result.available);
        assert_eq!(result.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_detect_options_default() {
        let options = DetectClaudeCodeOptions::default();
        assert!(!options.health_check);
    }

    // Note: We don't test detect_claude_code() directly because it depends
    // on the actual system state (whether claude is installed).
    // In TypeScript, this was handled with dependency injection.
    // For Rust, we could add trait-based DI if needed for testing.
}
