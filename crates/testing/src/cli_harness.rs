//! CLI Test Harness
//!
//! Provides infrastructure for running CLI commands against the openagents binary
//! and capturing/asserting on their output.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::process::Command;

/// Result of running a CLI command
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// The arguments that were passed
    pub args: Vec<String>,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Exit code (None if killed by signal)
    pub exit_code: Option<i32>,
    /// How long the command took
    pub duration_ms: u64,
}

impl CommandOutput {
    /// Check if the command succeeded (exit code 0)
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Get combined stdout + stderr
    pub fn combined(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

/// CLI test harness for running openagents commands
pub struct CliHarness {
    /// Path to the openagents binary
    binary_path: PathBuf,
    /// Working directory for commands (owned temp dir)
    _workdir: TempDir,
    /// Working directory path
    workdir_path: PathBuf,
    /// Environment variables to set
    env: HashMap<String, String>,
    /// History of all command outputs
    outputs: Vec<CommandOutput>,
    /// Default timeout for commands
    timeout: Duration,
}

impl CliHarness {
    /// Create a new CLI harness
    ///
    /// Automatically finds the openagents binary in target/debug or target/release
    pub async fn new() -> anyhow::Result<Self> {
        let binary_path = Self::find_binary()?;
        let workdir = tempfile::tempdir()?;
        let workdir_path = workdir.path().to_path_buf();

        Ok(Self {
            binary_path,
            _workdir: workdir,
            workdir_path,
            env: HashMap::new(),
            outputs: Vec::new(),
            timeout: Duration::from_secs(60),
        })
    }

    /// Create a harness with a specific binary path
    pub async fn with_binary(binary_path: PathBuf) -> anyhow::Result<Self> {
        let workdir = tempfile::tempdir()?;
        let workdir_path = workdir.path().to_path_buf();

        Ok(Self {
            binary_path,
            _workdir: workdir,
            workdir_path,
            env: HashMap::new(),
            outputs: Vec::new(),
            timeout: Duration::from_secs(60),
        })
    }

    /// Create a harness using a specific working directory
    pub async fn with_workdir(workdir: PathBuf) -> anyhow::Result<Self> {
        let binary_path = Self::find_binary()?;
        let temp = tempfile::tempdir()?;

        Ok(Self {
            binary_path,
            _workdir: temp,
            workdir_path: workdir,
            env: HashMap::new(),
            outputs: Vec::new(),
            timeout: Duration::from_secs(60),
        })
    }

    /// Find the openagents binary
    pub fn find_binary() -> anyhow::Result<PathBuf> {
        // Try common locations
        let candidates = [
            // Debug build
            PathBuf::from("target/debug/openagents"),
            // Release build
            PathBuf::from("target/release/openagents"),
            // From workspace root
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("target/debug/openagents"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("target/release/openagents"),
        ];

        for path in &candidates {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // Try using `which` to find in PATH
        if let Ok(output) = std::process::Command::new("which")
            .arg("openagents")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Ok(PathBuf::from(path));
                }
            }
        }

        anyhow::bail!(
            "Could not find openagents binary. Tried: {:?}. Run `cargo build --bin openagents` first.",
            candidates
        )
    }

    /// Set an environment variable for subsequent commands
    pub fn set_env(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.env.insert(key.into(), value.into());
    }

    /// Set the default timeout
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Get the working directory path
    pub fn workdir(&self) -> &PathBuf {
        &self.workdir_path
    }

    /// Get all command outputs so far
    pub fn outputs(&self) -> &[CommandOutput] {
        &self.outputs
    }

    /// Get the last command output
    pub fn last_output(&self) -> Option<&CommandOutput> {
        self.outputs.last()
    }

    /// Run a CLI command and return the result
    pub async fn run(&mut self, args: &[&str]) -> anyhow::Result<CommandOutput> {
        self.run_with_timeout(args, self.timeout).await
    }

    /// Run a CLI command with a specific timeout
    pub async fn run_with_timeout(
        &mut self,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandOutput> {
        let start = Instant::now();

        let mut cmd = Command::new(&self.binary_path);
        cmd.args(args)
            .current_dir(&self.workdir_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        // Spawn the process
        let child = cmd.spawn()?;

        // Wait with timeout
        let result = tokio::time::timeout(timeout, child.wait_with_output()).await;

        let output = match result {
            Ok(Ok(output)) => {
                let exit_code = output.status.code();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                CommandOutput {
                    args: args.iter().map(|s| s.to_string()).collect(),
                    stdout,
                    stderr,
                    exit_code,
                    duration_ms: start.elapsed().as_millis() as u64,
                }
            }
            Ok(Err(e)) => {
                anyhow::bail!("Failed to run command: {}", e);
            }
            Err(_) => {
                anyhow::bail!("Command timed out after {:?}", timeout);
            }
        };

        self.outputs.push(output.clone());
        Ok(output)
    }

    /// Run a command and expect it to succeed (exit code 0)
    ///
    /// Returns stdout on success, or error with stderr
    pub async fn run_ok(&mut self, args: &[&str]) -> anyhow::Result<String> {
        let output = self.run(args).await?;
        if output.success() {
            Ok(output.stdout)
        } else {
            anyhow::bail!(
                "Command failed with exit code {:?}:\nstdout: {}\nstderr: {}",
                output.exit_code,
                output.stdout,
                output.stderr
            )
        }
    }

    /// Run a command and expect it to fail (non-zero exit code)
    ///
    /// Returns stderr on expected failure, or error if it succeeds
    pub async fn run_err(&mut self, args: &[&str]) -> anyhow::Result<String> {
        let output = self.run(args).await?;
        if !output.success() {
            Ok(output.stderr)
        } else {
            anyhow::bail!(
                "Expected command to fail but it succeeded:\nstdout: {}",
                output.stdout
            )
        }
    }

    /// Run a command in the background
    ///
    /// Returns a handle that can be used to wait for completion or kill the process
    pub async fn run_background(&self, args: &[&str]) -> anyhow::Result<tokio::process::Child> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.args(args)
            .current_dir(&self.workdir_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        let child = cmd.spawn()?;
        Ok(child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_find_binary_or_skip() {
        // This test will be skipped if the binary doesn't exist
        match CliHarness::find_binary() {
            Ok(path) => {
                assert!(path.exists());
                println!("Found binary at: {:?}", path);
            }
            Err(_) => {
                println!("Binary not found, skipping test");
            }
        }
    }

    #[tokio::test]
    async fn test_set_env() {
        if CliHarness::find_binary().is_err() {
            println!("Binary not found, skipping test");
            return;
        }

        let mut harness = CliHarness::new().await.unwrap();
        harness.set_env("TEST_VAR", "test_value");
        assert_eq!(harness.env.get("TEST_VAR"), Some(&"test_value".to_string()));
    }

    #[tokio::test]
    async fn test_workdir_created() {
        if CliHarness::find_binary().is_err() {
            println!("Binary not found, skipping test");
            return;
        }

        let harness = CliHarness::new().await.unwrap();
        assert!(harness.workdir().exists());
    }

    #[tokio::test]
    async fn test_run_help() {
        if CliHarness::find_binary().is_err() {
            println!("Binary not found, skipping test");
            return;
        }

        let mut harness = CliHarness::new().await.unwrap();
        let output = harness.run(&["--help"]).await.unwrap();

        // --help should succeed
        assert!(
            output.success() || output.exit_code == Some(2),
            "Help should succeed or show usage"
        );
    }

    #[tokio::test]
    async fn test_outputs_tracked() {
        if CliHarness::find_binary().is_err() {
            println!("Binary not found, skipping test");
            return;
        }

        let mut harness = CliHarness::new().await.unwrap();
        harness.run(&["--version"]).await.ok();
        harness.run(&["--help"]).await.ok();

        assert_eq!(harness.outputs().len(), 2);
    }
}
