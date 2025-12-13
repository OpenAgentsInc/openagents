//! Bash/shell execution tool
//!
//! TOOL-030..033: Execute shell commands

use crate::error::{ToolError, ToolResult};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Default timeout in milliseconds
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000; // 2 minutes

/// Maximum timeout in milliseconds
pub const MAX_TIMEOUT_MS: u64 = 600_000; // 10 minutes

/// Maximum output size before truncation
pub const MAX_OUTPUT_SIZE: usize = 30_000;

/// Result of executing a bash command
#[derive(Debug, Clone)]
pub struct BashResult {
    /// The command that was executed
    pub command: String,
    /// Exit code (None if killed/timeout)
    pub exit_code: Option<i32>,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Combined output (stdout + stderr interleaved)
    pub output: String,
    /// Whether command succeeded (exit code 0)
    pub success: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Whether output was truncated
    pub truncated: bool,
    /// Whether command timed out
    pub timed_out: bool,
    /// Working directory used
    pub cwd: Option<String>,
}

/// Bash execution tool
///
/// TOOL-030: Execute shell commands
/// TOOL-031: Handle timeout
/// TOOL-032: Capture stdout/stderr
/// TOOL-033: Return exit code and output
pub struct BashTool;

impl BashTool {
    /// Execute a bash command
    ///
    /// # Arguments
    /// * `command` - The command to execute
    ///
    /// # Returns
    /// BashResult with output and metadata
    pub fn execute(command: &str) -> ToolResult<BashResult> {
        Self::execute_with_options(command, None, None)
    }

    /// Execute a bash command with timeout
    ///
    /// # Arguments
    /// * `command` - The command to execute
    /// * `timeout_ms` - Timeout in milliseconds (max 600000)
    pub fn execute_with_timeout(command: &str, timeout_ms: u64) -> ToolResult<BashResult> {
        Self::execute_with_options(command, Some(timeout_ms), None)
    }

    /// Execute a bash command in a specific directory
    ///
    /// # Arguments
    /// * `command` - The command to execute
    /// * `cwd` - Working directory
    pub fn execute_in_dir(command: &str, cwd: &str) -> ToolResult<BashResult> {
        Self::execute_with_options(command, None, Some(cwd))
    }

    /// Execute with all options
    ///
    /// # Arguments
    /// * `command` - The command to execute
    /// * `timeout_ms` - Optional timeout in milliseconds
    /// * `cwd` - Optional working directory
    pub fn execute_with_options(
        command: &str,
        timeout_ms: Option<u64>,
        cwd: Option<&str>,
    ) -> ToolResult<BashResult> {
        let start = Instant::now();
        let timeout =
            Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS).min(MAX_TIMEOUT_MS));

        // Build the command
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(command);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Set working directory if provided
        if let Some(dir) = cwd {
            let expanded = shellexpand::tilde(dir).to_string();
            let path = std::path::Path::new(&expanded);
            if !path.exists() {
                return Err(ToolError::not_found(format!(
                    "Working directory not found: {}",
                    dir
                )));
            }
            if !path.is_dir() {
                return Err(ToolError::invalid_arguments(format!(
                    "Path is not a directory: {}",
                    dir
                )));
            }
            cmd.current_dir(path);
        }

        // Spawn the process
        let mut child = cmd
            .spawn()
            .map_err(|e| ToolError::command_failed(format!("Failed to spawn command: {}", e)))?;

        // Wait with timeout
        let output_result = Self::wait_with_timeout(&mut child, timeout);

        let duration_ms = start.elapsed().as_millis() as u64;

        match output_result {
            Ok(output) => {
                let (stdout, stdout_truncated) = Self::process_output(&output.stdout);
                let (stderr, stderr_truncated) = Self::process_output(&output.stderr);

                // Combine output
                let output_combined = if stderr.is_empty() {
                    stdout.clone()
                } else if stdout.is_empty() {
                    stderr.clone()
                } else {
                    format!("{}\n{}", stdout, stderr)
                };

                let exit_code = output.status.code();
                let success = output.status.success();

                Ok(BashResult {
                    command: command.to_string(),
                    exit_code,
                    stdout,
                    stderr,
                    output: output_combined,
                    success,
                    duration_ms,
                    truncated: stdout_truncated || stderr_truncated,
                    timed_out: false,
                    cwd: cwd.map(String::from),
                })
            }
            Err(timed_out) => {
                if timed_out {
                    // Kill the process
                    let _ = child.kill();
                    let _ = child.wait();

                    Ok(BashResult {
                        command: command.to_string(),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: format!("Command timed out after {}ms", timeout.as_millis()),
                        output: format!("Command timed out after {}ms", timeout.as_millis()),
                        success: false,
                        duration_ms,
                        truncated: false,
                        timed_out: true,
                        cwd: cwd.map(String::from),
                    })
                } else {
                    Err(ToolError::command_failed("Failed to wait for command"))
                }
            }
        }
    }

    /// Wait for process with timeout
    fn wait_with_timeout(
        child: &mut std::process::Child,
        timeout: Duration,
    ) -> Result<Output, bool> {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(10);

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process finished, collect output
                    let stdout = child
                        .stdout
                        .take()
                        .map(|mut s| {
                            let mut buf = Vec::new();
                            std::io::Read::read_to_end(&mut s, &mut buf).ok();
                            buf
                        })
                        .unwrap_or_default();

                    let stderr = child
                        .stderr
                        .take()
                        .map(|mut s| {
                            let mut buf = Vec::new();
                            std::io::Read::read_to_end(&mut s, &mut buf).ok();
                            buf
                        })
                        .unwrap_or_default();

                    return Ok(Output {
                        status,
                        stdout,
                        stderr,
                    });
                }
                Ok(None) => {
                    // Still running, check timeout
                    if start.elapsed() >= timeout {
                        return Err(true); // Timed out
                    }
                    std::thread::sleep(poll_interval);
                }
                Err(_) => {
                    return Err(false); // Error waiting
                }
            }
        }
    }

    /// Process output bytes into string, with truncation
    fn process_output(bytes: &[u8]) -> (String, bool) {
        let text = String::from_utf8_lossy(bytes);
        if text.len() > MAX_OUTPUT_SIZE {
            let truncated = format!(
                "{}...\n[Output truncated, {} total characters]",
                &text[..MAX_OUTPUT_SIZE],
                text.len()
            );
            (truncated, true)
        } else {
            (text.to_string(), false)
        }
    }

    /// Execute multiple commands in sequence
    ///
    /// Stops on first failure unless `continue_on_error` is true
    pub fn execute_sequence(
        commands: &[&str],
        continue_on_error: bool,
    ) -> ToolResult<Vec<BashResult>> {
        let mut results = Vec::new();

        for cmd in commands {
            let result = Self::execute(cmd)?;
            let success = result.success;
            results.push(result);

            if !success && !continue_on_error {
                break;
            }
        }

        Ok(results)
    }

    /// Check if a command exists
    pub fn command_exists(cmd: &str) -> bool {
        Self::execute(&format!("command -v {} >/dev/null 2>&1", cmd))
            .map(|r| r.success)
            .unwrap_or(false)
    }

    /// Get the current working directory
    pub fn pwd() -> ToolResult<String> {
        let result = Self::execute("pwd")?;
        if result.success {
            Ok(result.stdout.trim().to_string())
        } else {
            Err(ToolError::command_failed("Failed to get working directory"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        let result = BashTool::execute("echo 'hello world'").unwrap();
        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout.trim(), "hello world");
    }

    #[test]
    fn test_command_failure() {
        let result = BashTool::execute("exit 1").unwrap();
        assert!(!result.success);
        assert_eq!(result.exit_code, Some(1));
    }

    #[test]
    fn test_stderr() {
        let result = BashTool::execute("echo 'error' >&2").unwrap();
        assert!(result.success);
        assert_eq!(result.stderr.trim(), "error");
    }

    #[test]
    fn test_combined_output() {
        let result = BashTool::execute("echo 'out'; echo 'err' >&2").unwrap();
        assert!(result.success);
        assert!(result.output.contains("out"));
        assert!(result.output.contains("err"));
    }

    #[test]
    fn test_timeout() {
        let result = BashTool::execute_with_timeout("sleep 10", 100).unwrap();
        assert!(!result.success);
        assert!(result.timed_out);
        assert!(result.exit_code.is_none());
    }

    #[test]
    fn test_working_directory() {
        let result = BashTool::execute_in_dir("pwd", "/tmp").unwrap();
        assert!(result.success);
        // macOS uses /private/tmp
        assert!(
            result.stdout.trim() == "/tmp" || result.stdout.trim() == "/private/tmp",
            "Unexpected pwd result: {}",
            result.stdout.trim()
        );
    }

    #[test]
    fn test_nonexistent_directory() {
        let result = BashTool::execute_in_dir("pwd", "/nonexistent/dir");
        assert!(result.is_err());
    }

    #[test]
    fn test_command_exists() {
        assert!(BashTool::command_exists("bash"));
        assert!(BashTool::command_exists("ls"));
        assert!(!BashTool::command_exists("nonexistent_command_12345"));
    }

    #[test]
    fn test_pwd() {
        let result = BashTool::pwd().unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_sequence() {
        let results =
            BashTool::execute_sequence(&["echo 'first'", "echo 'second'", "echo 'third'"], false)
                .unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.success));
    }

    #[test]
    fn test_sequence_stops_on_error() {
        let results =
            BashTool::execute_sequence(&["echo 'first'", "exit 1", "echo 'third'"], false).unwrap();
        assert_eq!(results.len(), 2); // Stops after exit 1
        assert!(results[0].success);
        assert!(!results[1].success);
    }

    #[test]
    fn test_sequence_continue_on_error() {
        let results =
            BashTool::execute_sequence(&["echo 'first'", "exit 1", "echo 'third'"], true).unwrap();
        assert_eq!(results.len(), 3); // Continues despite exit 1
    }

    #[test]
    fn test_multiline_output() {
        let result = BashTool::execute("echo 'line1'; echo 'line2'; echo 'line3'").unwrap();
        assert!(result.success);
        let lines: Vec<&str> = result.stdout.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_special_characters() {
        let result = BashTool::execute("echo 'hello \"world\" $HOME'").unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("hello \"world\""));
    }
}
