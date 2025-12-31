//! Process-based transport for the Codex CLI.
//!
//! This module spawns the `codex exec --experimental-json` command and
//! communicates via JSONL over stdin/stdout.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

use crate::error::{Error, Result};
use crate::events::ThreadEvent;

/// Find the codex executable in PATH or common installation locations.
pub fn find_codex_executable() -> Result<PathBuf> {
    // First try PATH
    if let Ok(path) = which::which("codex") {
        return Ok(path);
    }

    // Try common locations
    let home = std::env::var("HOME").unwrap_or_default();
    let common_paths = [
        format!("{}/.npm-global/bin/codex", home),
        format!("{}/node_modules/.bin/codex", home),
        "/usr/local/bin/codex".to_string(),
        "/opt/homebrew/bin/codex".to_string(),
    ];

    for path in common_paths {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
    }

    Err(Error::ExecutableNotFound(
        "codex not found in PATH or common locations. Install with: npm install -g @openai/codex"
            .to_string(),
    ))
}

/// Process transport for communicating with the Codex CLI.
pub struct ProcessTransport {
    child: Child,
    #[allow(dead_code)]
    stdin: ChildStdin,
    stdout_rx: mpsc::Receiver<Result<ThreadEvent>>,
    _stdout_task: Option<tokio::task::JoinHandle<()>>,
}

impl ProcessTransport {
    /// Spawn a new Codex process with the given arguments.
    pub async fn spawn(
        executable: PathBuf,
        args: Vec<String>,
        cwd: Option<PathBuf>,
        env: Option<HashMap<String, String>>,
    ) -> Result<Self> {
        let mut cmd = Command::new(&executable);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }

        if let Some(env) = env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }

        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take().expect("stdin not captured");
        let stdout = child.stdout.take().expect("stdout not captured");

        let (tx, rx) = mpsc::channel(100);

        let stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }

                let event = serde_json::from_str::<ThreadEvent>(&line).map_err(|e| Error::Json(e));

                if tx.send(event).await.is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            stdout_rx: rx,
            _stdout_task: Some(stdout_task),
        })
    }

    /// Receive the next event from the Codex process.
    pub async fn recv(&mut self) -> Option<Result<ThreadEvent>> {
        self.stdout_rx.recv().await
    }

    /// Kill the Codex process.
    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }

    /// Wait for the process to exit and return the exit code.
    pub async fn wait(&mut self) -> Result<Option<i32>> {
        let status = self.child.wait().await?;
        Ok(status.code())
    }

    /// Gracefully shutdown the process and await background tasks
    pub async fn shutdown(mut self) -> Result<()> {
        // Kill the process first
        self.child.kill().await?;

        // Wait for stdout task to complete if present
        // The task will exit once stdout is closed (when process dies)
        if let Some(task) = self._stdout_task.take() {
            let _ = task.await;
        }

        Ok(())
    }
}

impl Drop for ProcessTransport {
    fn drop(&mut self) {
        // Abort the stdout task on drop if present
        // Note: The task cannot be awaited in Drop (not async)
        // Use shutdown() method for graceful cleanup
        if let Some(task) = self._stdout_task.take() {
            task.abort();
        }
    }
}
