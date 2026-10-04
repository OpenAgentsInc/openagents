//! Process transport implementation for Claude Code CLI.

use crate::error::{Error, Result};
use crate::protocol::{StdinMessage, StdoutMessage};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;
use tracing::{debug, error, trace, warn};

/// Configuration for finding the Claude Code executable.
#[derive(Debug, Clone)]
pub struct ExecutableConfig {
    /// Explicit path to the Claude Code executable (cli.js or claude binary).
    pub path: Option<PathBuf>,
    /// JavaScript runtime to use (node, bun, deno).
    pub executable: Option<String>,
    /// Additional arguments for the runtime.
    pub executable_args: Vec<String>,
}

impl Default for ExecutableConfig {
    fn default() -> Self {
        Self {
            path: None,
            executable: None,
            executable_args: Vec::new(),
        }
    }
}

/// Process transport for communicating with Claude Code CLI.
pub struct ProcessTransport {
    child: Child,
    /// The child's process ID, which is also its process group's on Unix.
    #[cfg_attr(not(unix), allow(dead_code))]
    pid: Option<u32>,
    /// Whether the process group was signalled already.
    stopped: bool,
    stdin: ChildStdin,
    stdout_rx: Option<mpsc::Receiver<Result<StdoutMessage>>>,
    /// Handle to the stdout reader task.
    _stdout_task: tokio::task::JoinHandle<()>,
}

impl ProcessTransport {
    /// Spawn a new Claude Code CLI process.
    pub async fn spawn(
        config: ExecutableConfig,
        args: Vec<String>,
        cwd: Option<PathBuf>,
        env: Option<Vec<(String, String)>>,
    ) -> Result<Self> {
        Self::spawn_with(config, args, cwd, env, &[]).await
    }

    /// [`ProcessTransport::spawn`], with the variables named in `removed`
    /// left out of what the CLI inherits. On Unix the CLI leads a process
    /// group of its own, so stopping it stops every process it started.
    pub async fn spawn_with(
        config: ExecutableConfig,
        args: Vec<String>,
        cwd: Option<PathBuf>,
        env: Option<Vec<(String, String)>>,
        removed: &[String],
    ) -> Result<Self> {
        let (command, command_args) = Self::build_command(&config)?;

        debug!(
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

        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }

        for key in removed {
            cmd.env_remove(key);
        }
        if let Some(env_vars) = env {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }
        #[cfg(unix)]
        cmd.process_group(0);

        let mut child = cmd.spawn()?;
        let pid = child.id();

        let stdin = child.stdin.take().ok_or_else(|| {
            Error::SpawnFailed(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to capture stdin",
            ))
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            Error::SpawnFailed(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to capture stdout",
            ))
        })?;

        // Create channel for stdout messages
        let (stdout_tx, stdout_rx) = mpsc::channel(256);

        // Spawn task to read stdout
        let stdout_task = tokio::spawn(Self::read_stdout(stdout, stdout_tx));

        Ok(Self {
            child,
            pid,
            stopped: false,
            stdin,
            stdout_rx: Some(stdout_rx),
            _stdout_task: stdout_task,
        })
    }

    /// Take the stdout receiver so a reader task can wait without holding
    /// the stdin lock. A second take returns an empty closed channel.
    pub fn take_stdout_rx(&mut self) -> mpsc::Receiver<Result<StdoutMessage>> {
        self.stdout_rx.take().unwrap_or_else(|| {
            let (_tx, rx) = mpsc::channel(1);
            rx
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
                    } else if which::which("node").is_ok() {
                        "node".to_string()
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
            "/usr/local/bin/claude".to_string(),
            "/opt/homebrew/bin/claude".to_string(),
        ];

        for path in possible_paths {
            if std::path::Path::new(&path).exists() {
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

                    trace!(line = %line, "Received line from CLI");

                    match crate::protocol::parse_stdout_line(&line) {
                        Ok(msg) => {
                            if tx.send(Ok(msg)).await.is_err() {
                                // Receiver dropped, exit
                                break;
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, line = %line, "Unrecognized JSONL message");
                            // Surface the defect; keep reading so one bad line
                            // cannot kill the stream.
                            if tx.send(Err(e)).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                Ok(None) => {
                    // EOF
                    debug!("CLI stdout closed");
                    break;
                }
                Err(e) => {
                    error!(error = %e, "Error reading from CLI stdout");
                    let _ = tx.send(Err(Error::StdoutRead(e))).await;
                    break;
                }
            }
        }
    }

    /// Send a message to the CLI via stdin.
    pub async fn send(&mut self, message: &StdinMessage) -> Result<()> {
        let json = serde_json::to_string(message)?;
        trace!(json = %json, "Sending message to CLI");

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
        match self.stdout_rx.as_mut() {
            Some(rx) => rx.recv().await,
            None => None,
        }
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }

    /// Kill the CLI process and, on Unix, its process group, then wait
    /// for the CLI to exit.
    pub async fn kill(&mut self) -> Result<()> {
        self.kill_group();
        self.child.wait().await?;
        Ok(())
    }

    /// Start killing the CLI's process group (on Unix) and the CLI itself,
    /// without waiting. The group is signalled once, even after the CLI
    /// exited, so a command it left running in the background stops too.
    pub fn kill_group(&mut self) {
        if !self.stopped {
            self.stopped = true;
            #[cfg(unix)]
            if let Some(group) = self.pid.and_then(|pid| i32::try_from(pid).ok()) {
                // SAFETY: `kill` takes no pointers; a negative ID names the
                // group the CLI leads, which `spawn_with` created.
                unsafe {
                    libc::kill(-group, libc::SIGKILL);
                }
            }
        }
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.start_kill();
        }
    }
}

impl Drop for ProcessTransport {
    fn drop(&mut self) {
        self.kill_group();
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
