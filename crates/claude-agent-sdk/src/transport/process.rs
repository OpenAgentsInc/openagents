//! Process transport implementation for Claude Code CLI.

use crate::error::{Error, Result};
use crate::protocol::{StdinMessage, StdoutMessage};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;

/// Configuration for finding the Claude Code executable.
#[derive(Debug, Clone, Default)]
pub struct ExecutableConfig {
    /// Explicit path to the Claude Code executable (cli.js or claude binary).
    pub path: Option<PathBuf>,
    /// JavaScript runtime to use (node, bun, deno).
    pub executable: Option<String>,
    /// Additional arguments for the runtime.
    pub executable_args: Vec<String>,
}

/// Process transport for communicating with Claude Code CLI.
pub struct ProcessTransport {
    child: Child,
    stdin: ChildStdin,
    stdout_rx: mpsc::Receiver<Result<StdoutMessage>>,
    /// Handle to the stdout reader task.
    _stdout_task: tokio::task::JoinHandle<()>,
}

impl ProcessTransport {
    /// Get the PATH from login shell (includes shell profile additions).
    fn get_shell_path() -> Option<String> {
        // Use $SHELL environment variable, fallback to platform defaults
        let shell = std::env::var("SHELL").ok().unwrap_or_else(|| {
            if cfg!(target_os = "windows") {
                "powershell".to_string()
            } else {
                "/bin/sh".to_string()
            }
        });

        std::process::Command::new(&shell)
            .args(["-lc", "echo $PATH"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Spawn a new Claude Code CLI process.
    pub async fn spawn(
        config: ExecutableConfig,
        args: Vec<String>,
        cwd: Option<PathBuf>,
        env: Option<Vec<(String, String)>>,
    ) -> Result<Self> {
        let (command, command_args) = Self::build_command(&config)?;

        tracing::info!(
            command = %command,
            args = ?command_args,
            extra_args = ?args,
            "Spawning Claude Code CLI"
        );

        let mut cmd = Command::new(&command);
        cmd.args(&command_args)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()); // Let stderr pass through for debugging

        // Use PATH from login shell to include shell profile additions
        if let Some(shell_path) = Self::get_shell_path() {
            cmd.env("PATH", shell_path);
        }

        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }

        if let Some(env_vars) = env {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        let mut child = cmd.spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::SpawnFailed(std::io::Error::other("Failed to capture stdin")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::SpawnFailed(std::io::Error::other("Failed to capture stdout")))?;

        // Create channel for stdout messages
        let (stdout_tx, stdout_rx) = mpsc::channel(256);

        // Spawn task to read stdout
        let stdout_task = tokio::spawn(Self::read_stdout(stdout, stdout_tx));

        Ok(Self {
            child,
            stdin,
            stdout_rx,
            _stdout_task: stdout_task,
        })
    }

    /// Build the command and arguments for spawning.
    fn build_command(config: &ExecutableConfig) -> Result<(String, Vec<String>)> {
        // If explicit path is provided, use it
        if let Some(path) = &config.path {
            let path_str = path.display().to_string();

            // Check if it's a .js file (needs runtime)
            if path_str.ends_with(".js") {
                let runtime = config.executable.clone().unwrap_or_else(|| {
                    // Try to detect available runtime
                    if which::which("bun").is_ok() {
                        "bun".to_string()
                    } else {
                        "node".to_string() // Default to node
                    }
                });

                let mut args = config.executable_args.clone();
                args.push(path_str);
                return Ok((runtime, args));
            }

            // Direct binary
            return Ok((path_str, config.executable_args.clone()));
        }

        // Try to find claude in PATH
        if let Ok(claude_path) = which::which("claude") {
            return Ok((claude_path.display().to_string(), Vec::new()));
        }

        // Try common locations
        let home = std::env::var("HOME").unwrap_or_default();
        let possible_paths = [
            format!("{}/.claude/local/claude", home),
            format!("{}/.npm-global/bin/claude", home),
            format!("{}/.local/bin/claude", home),
            "/usr/local/bin/claude".to_string(),
            "/opt/homebrew/bin/claude".to_string(),
        ];

        for path in &possible_paths {
            if std::path::Path::new(path).exists() {
                return Ok((path.clone(), Vec::new()));
            }
        }

        // Try using login shell to get PATH from shell profile
        if let Ok(output) = std::process::Command::new("zsh")
            .args(["-lc", "which claude"])
            .output()
            && output.status.success()
        {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && std::path::Path::new(&path).exists() {
                return Ok((path, Vec::new()));
            }
        }

        Err(Error::ExecutableNotFound(
            "Could not find 'claude' executable. Install Claude Code CLI or provide explicit path."
                .to_string(),
        ))
    }

    /// Read stdout lines and parse as JSONL messages.
    async fn read_stdout(stdout: ChildStdout, tx: mpsc::Sender<Result<StdoutMessage>>) {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.is_empty() {
                        continue;
                    }

                    tracing::debug!(line = %line, "Received line from CLI");

                    match serde_json::from_str::<StdoutMessage>(&line) {
                        Ok(msg) => {
                            if tx.send(Ok(msg)).await.is_err() {
                                // Receiver dropped, exit
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, line = %line, "Failed to parse JSONL message");
                            // Continue reading, don't fail on parse errors
                        }
                    }
                }
                Ok(None) => {
                    // EOF
                    tracing::debug!("CLI stdout closed (EOF)");
                    break;
                }
                Err(e) => {
                    tracing::error!(error = %e, "Error reading from CLI stdout");
                    let _ = tx.send(Err(Error::StdoutRead(e))).await;
                    break;
                }
            }
        }
    }

    /// Send a message to the CLI via stdin.
    pub async fn send(&mut self, message: &StdinMessage) -> Result<()> {
        let json = serde_json::to_string(message)?;
        tracing::debug!(json = %json, "Sending message to CLI");

        self.stdin
            .write_all(json.as_bytes())
            .await
            .map_err(Error::StdinWrite)?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(Error::StdinWrite)?;
        self.stdin.flush().await.map_err(Error::StdinWrite)?;

        Ok(())
    }

    /// Receive the next message from the CLI.
    pub async fn recv(&mut self) -> Option<Result<StdoutMessage>> {
        self.stdout_rx.recv().await
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Kill the CLI process.
    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }

    /// Wait for the process to exit and return the exit code.
    pub async fn wait(&mut self) -> Result<Option<i32>> {
        let status = self.child.wait().await?;
        Ok(status.code())
    }
}

impl Drop for ProcessTransport {
    fn drop(&mut self) {
        // Try to kill the process on drop
        let _ = self.child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_command_finds_claude() {
        // This test will pass if claude is installed, skip otherwise
        let config = ExecutableConfig::default();
        let result = ProcessTransport::build_command(&config);

        // Either finds claude or returns an error
        match result {
            Ok((cmd, _)) => {
                assert!(cmd.contains("claude"));
            }
            Err(Error::ExecutableNotFound(_)) => {
                // Expected if claude not installed
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_build_command_with_explicit_path() {
        let config = ExecutableConfig {
            path: Some(PathBuf::from("/usr/bin/claude")),
            ..Default::default()
        };

        let (cmd, args) = ProcessTransport::build_command(&config).unwrap();
        assert_eq!(cmd, "/usr/bin/claude");
        assert!(args.is_empty());
    }

    #[test]
    fn test_build_command_with_js_file() {
        let config = ExecutableConfig {
            path: Some(PathBuf::from("/path/to/cli.js")),
            executable: Some("bun".to_string()),
            executable_args: vec!["--smol".to_string()],
        };

        let (cmd, args) = ProcessTransport::build_command(&config).unwrap();
        assert_eq!(cmd, "bun");
        assert_eq!(args, vec!["--smol", "/path/to/cli.js"]);
    }
}
