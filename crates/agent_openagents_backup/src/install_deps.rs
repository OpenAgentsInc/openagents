//! Dependency Installation Helper
//!
//! Shared helper for installing dependencies in worktrees and sandboxes.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Default install timeout (15 minutes)
pub const DEFAULT_INSTALL_TIMEOUT_MS: u64 = 15 * 60 * 1000;

/// Default install args
pub const DEFAULT_INSTALL_ARGS: &[&str] = &["--frozen-lockfile"];

/// Settings for dependency installation
#[derive(Debug, Clone)]
pub struct InstallSettings {
    /// Arguments to pass to the install command
    pub args: Vec<String>,
    /// Timeout in milliseconds
    pub timeout_ms: u64,
    /// Whether to skip installation entirely
    pub skip_install: bool,
}

impl Default for InstallSettings {
    fn default() -> Self {
        Self {
            args: DEFAULT_INSTALL_ARGS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            timeout_ms: DEFAULT_INSTALL_TIMEOUT_MS,
            skip_install: false,
        }
    }
}

impl InstallSettings {
    /// Create settings from explicit options
    pub fn from_options(
        install_args: Option<&[String]>,
        install_timeout_ms: Option<u64>,
    ) -> Self {
        let args = install_args
            .filter(|a| !a.is_empty())
            .map(|a| a.to_vec())
            .unwrap_or_else(|| {
                DEFAULT_INSTALL_ARGS
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            });

        let skip_install = args.iter().any(|a| a == "--skip-install");
        let args: Vec<String> = args.into_iter().filter(|a| a != "--skip-install").collect();

        Self {
            args,
            timeout_ms: install_timeout_ms.unwrap_or(DEFAULT_INSTALL_TIMEOUT_MS),
            skip_install,
        }
    }
}

/// Result of a dependency installation
#[derive(Debug, Clone)]
pub struct InstallResult {
    /// Whether installation succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Whether the operation timed out
    pub timed_out: bool,
}

impl InstallResult {
    /// Create a success result
    pub fn ok() -> Self {
        Self {
            success: true,
            error: None,
            timed_out: false,
        }
    }

    /// Create an error result
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(error.into()),
            timed_out: false,
        }
    }

    /// Create a timeout result
    pub fn timeout(timeout_secs: u64) -> Self {
        Self {
            success: false,
            error: Some(format!("bun install timed out after {}s", timeout_secs)),
            timed_out: true,
        }
    }
}

/// Install dependencies using bun
pub fn install_deps(cwd: &str, settings: &InstallSettings) -> InstallResult {
    if settings.skip_install {
        return InstallResult::ok();
    }

    let timeout = Duration::from_millis(settings.timeout_ms);
    let start = Instant::now();

    // Build command
    let mut cmd = Command::new("bun");
    cmd.arg("install");
    cmd.args(&settings.args);
    cmd.current_dir(cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Spawn process
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return InstallResult::err(format!("Failed to spawn bun install: {}", e)),
    };

    // Wait with timeout
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process finished
                if status.success() {
                    return InstallResult::ok();
                } else {
                    // Get stderr for error message
                    let stderr = child
                        .stderr
                        .take()
                        .and_then(|mut s| {
                            let mut buf = String::new();
                            std::io::Read::read_to_string(&mut s, &mut buf).ok()?;
                            Some(buf)
                        })
                        .unwrap_or_default();
                    return InstallResult::err(format!("bun install failed: {}", stderr));
                }
            }
            Ok(None) => {
                // Still running, check timeout
                if start.elapsed() > timeout {
                    // Kill the process
                    let _ = child.kill();
                    let _ = child.wait();
                    return InstallResult::timeout(settings.timeout_ms / 1000);
                }
                // Sleep briefly before checking again
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return InstallResult::err(format!("Failed to wait for bun install: {}", e));
            }
        }
    }
}

/// Install dependencies asynchronously (returns immediately, runs in background)
pub fn install_deps_async(cwd: &str, settings: &InstallSettings) -> std::io::Result<std::process::Child> {
    if settings.skip_install {
        // Return a dummy "true" command that succeeds immediately
        return Command::new("true").spawn();
    }

    let mut cmd = Command::new("bun");
    cmd.arg("install");
    cmd.args(&settings.args);
    cmd.current_dir(cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    cmd.spawn()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_settings_default() {
        let settings = InstallSettings::default();
        assert_eq!(settings.args, vec!["--frozen-lockfile"]);
        assert_eq!(settings.timeout_ms, DEFAULT_INSTALL_TIMEOUT_MS);
        assert!(!settings.skip_install);
    }

    #[test]
    fn test_install_settings_from_options() {
        let args = vec!["--frozen-lockfile".to_string(), "--production".to_string()];
        let settings = InstallSettings::from_options(Some(&args), Some(60000));

        assert_eq!(settings.args, args);
        assert_eq!(settings.timeout_ms, 60000);
        assert!(!settings.skip_install);
    }

    #[test]
    fn test_install_settings_skip_install() {
        let args = vec!["--skip-install".to_string()];
        let settings = InstallSettings::from_options(Some(&args), None);

        assert!(settings.skip_install);
        assert!(settings.args.is_empty()); // --skip-install is filtered out
    }

    #[test]
    fn test_install_result_ok() {
        let result = InstallResult::ok();
        assert!(result.success);
        assert!(result.error.is_none());
        assert!(!result.timed_out);
    }

    #[test]
    fn test_install_result_err() {
        let result = InstallResult::err("something failed");
        assert!(!result.success);
        assert_eq!(result.error, Some("something failed".to_string()));
        assert!(!result.timed_out);
    }

    #[test]
    fn test_install_result_timeout() {
        let result = InstallResult::timeout(300);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("300"));
        assert!(result.timed_out);
    }

    #[test]
    fn test_install_deps_skip() {
        let settings = InstallSettings {
            skip_install: true,
            ..Default::default()
        };
        let result = install_deps(".", &settings);
        assert!(result.success);
    }
}
