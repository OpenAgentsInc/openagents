//! A local ACP agent as a process of its own.
//!
//! The agent is the leader of a new process group, so every tool it starts
//! is in the group, and stopping the agent stops the group: `SIGTERM`, a
//! grace, then `SIGKILL`, and the stop reports whether the group is empty.
//! The caller passes the agent's whole environment; nothing is inherited.
//! Standard error is not protocol: it is drained, and its last lines are
//! kept for a failure to quote.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::client::Client;

/// How many of the agent's last standard error lines are kept.
const STDERR_TAIL: usize = 8;
/// The longest kept standard error line.
const STDERR_LINE: usize = 400;

/// How to start one agent.
#[derive(Clone, Debug)]
pub struct Spec {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub cwd: PathBuf,
    /// The agent's whole environment.
    pub environment: Vec<(String, String)>,
}

/// A running agent and the client over its streams.
pub struct Agent {
    child: Child,
    group: i32,
    pub client: Client<ChildStdout, ChildStdin>,
    stderr: Arc<Mutex<VecDeque<String>>>,
}

/// The agent could not start.
#[derive(Debug)]
pub struct Unstartable(pub String);

impl std::fmt::Display for Unstartable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unstartable {}

fn signal_group(group: i32, signal: i32) -> bool {
    if group <= 0 {
        return false;
    }
    // SAFETY: `killpg` reads two integers and returns one. The group is the
    // one this module made for the agent, so no process outside it is in it.
    unsafe { libc::killpg(group, signal) == 0 }
}

impl Agent {
    /// Start the agent described by `spec`.
    ///
    /// # Errors
    /// The program is missing or the operating system refused to start it.
    pub fn start(spec: &Spec) -> Result<Self, Unstartable> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.arguments)
            .current_dir(&spec.cwd)
            .env_clear()
            .envs(spec.environment.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| {
            Unstartable(format!("cannot start {}: {error}", spec.program.display()))
        })?;
        let pid = child.id().unwrap_or_default();
        let group = i32::try_from(pid).unwrap_or_default();
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            signal_group(group, libc::SIGKILL);
            return Err(Unstartable("the agent's streams are unavailable".into()));
        };
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let kept = Arc::clone(&tail);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(mut line)) = lines.next_line().await {
                if line.len() > STDERR_LINE {
                    let mut end = STDERR_LINE;
                    while !line.is_char_boundary(end) {
                        end -= 1;
                    }
                    line.truncate(end);
                }
                if let Ok(mut kept) = kept.lock() {
                    kept.push_back(line);
                    while kept.len() > STDERR_TAIL {
                        kept.pop_front();
                    }
                }
            }
        });
        Ok(Agent {
            child,
            group,
            client: Client::new(stdout, stdin),
            stderr: tail,
        })
    }

    /// The agent's process identifier, which is also its group's.
    #[must_use]
    pub fn pid(&self) -> u32 {
        u32::try_from(self.group).unwrap_or_default()
    }

    /// The agent's last standard error lines, oldest first.
    #[must_use]
    pub fn stderr_tail(&self) -> Vec<String> {
        self.stderr
            .lock()
            .map(|kept| kept.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Stop the agent and its group: `SIGTERM`, up to `grace` for the leader
    /// to exit, then `SIGKILL`. Returns whether the group is empty.
    pub async fn stop(mut self, grace: Duration) -> bool {
        signal_group(self.group, libc::SIGTERM);
        let exited = tokio::time::timeout(grace, self.child.wait()).await.is_ok();
        if !exited || signal_group(self.group, 0) {
            signal_group(self.group, libc::SIGKILL);
            let _ = tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await;
        }
        for _ in 0..20 {
            if !signal_group(self.group, 0) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }
}

/// The first executable file among `candidates`.
#[must_use]
pub fn first_executable(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    candidates.into_iter().find(|path| {
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    })
}

/// `name` in each directory of `PATH`.
#[must_use]
pub fn on_path(name: &str, path: Option<&std::ffi::OsStr>) -> Vec<PathBuf> {
    path.map(|paths| {
        std::env::split_paths(paths)
            .map(|dir| dir.join(name))
            .collect()
    })
    .unwrap_or_default()
}

/// Whether a variable name names a credential the agent must not inherit:
/// it ends in `_API_KEY`, `_TOKEN`, or `_SECRET`, as the task owner's full
/// access leaves out.
#[must_use]
pub fn is_credential_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    ["_API_KEY", "_TOKEN", "_SECRET"]
        .iter()
        .any(|suffix| upper.ends_with(suffix))
}

/// `path` relative to `home`, for a message that names a location without
/// the account's home directory.
#[must_use]
pub fn tilde(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_names_are_recognized_by_suffix() {
        assert!(is_credential_name("OPENAI_API_KEY"));
        assert!(is_credential_name("github_token"));
        assert!(is_credential_name("AWS_SECRET"));
        assert!(!is_credential_name("HOME"));
        assert!(!is_credential_name("TOKENIZER"));
    }

    #[tokio::test]
    async fn stopping_an_agent_empties_its_group() {
        let spec = Spec {
            program: PathBuf::from("/bin/sh"),
            arguments: vec!["-c".into(), "sleep 30 & sleep 30".into()],
            cwd: std::env::temp_dir(),
            environment: vec![("PATH".into(), "/bin:/usr/bin".into())],
        };
        let agent = Agent::start(&spec).unwrap();
        assert!(agent.pid() > 0);
        assert!(agent.stop(Duration::from_millis(500)).await);
    }

    #[tokio::test]
    async fn a_missing_program_is_unstartable() {
        let spec = Spec {
            program: PathBuf::from("/nonexistent/agent"),
            arguments: Vec::new(),
            cwd: std::env::temp_dir(),
            environment: Vec::new(),
        };
        assert!(Agent::start(&spec).is_err());
    }
}
