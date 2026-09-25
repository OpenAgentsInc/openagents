//! Where contract items run: this host, confined, or a task container.
//!
//! A [`Host`] answers three things about one workspace: what is at a path,
//! what a file holds, and what a command does. [`Local`] runs commands
//! through `supervise` inside a `coder-boundary` boundary whose only
//! writable place is the workspace, for a Coder that already runs in the
//! task's container. [`Container`] runs them with `docker exec` in a
//! running container with no network, for offline replay of retained
//! workspaces; the `docker` client itself runs under `supervise`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The most bytes of standard output a run keeps; an expected output is
/// compared only when all of it was kept.
pub const STDOUT_MAX: usize = 1024 * 1024;

/// What is at a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stat {
    Missing,
    Dir,
    /// A file, with its size in bytes.
    File(u64),
}

/// What one command did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ran {
    /// The exit code, or `None` when a signal or the host ended it.
    pub exit: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// The wall-time bound ended it.
    pub timed_out: bool,
    /// Standard output was longer than [`STDOUT_MAX`].
    pub truncated: bool,
    /// It never ran, and why.
    pub failed: Option<String>,
    pub milliseconds: u64,
}

/// Where items run.
pub trait Host {
    /// The host, for the record.
    fn describe(&self) -> Value;

    /// What is at `path`.
    fn stat(&self, path: &str) -> impl Future<Output = Result<Stat, String>>;

    /// Up to `max` bytes of the file at `path`, or `None` when there is no
    /// file there.
    fn read(&self, path: &str, max: usize)
    -> impl Future<Output = Result<Option<Vec<u8>>, String>>;

    /// Runs `command` with `sh -c` from the workspace, bounded by `wall`.
    fn run(&self, command: &str, wall: Duration) -> impl Future<Output = Ran>;
}

fn ended_to_ran(ended: &supervise::Ended, started: Instant) -> Ran {
    let (exit, timed_out, failed) = match &ended.ending {
        supervise::Ending::Exited(code) => (*code, false, None),
        supervise::Ending::TimedOut => (None, true, None),
        supervise::Ending::Failed(why) => (None, false, Some(why.clone())),
    };
    Ran {
        exit,
        stdout: ended.stdout.text.clone(),
        stderr: ended.stderr.text.clone(),
        timed_out,
        truncated: ended.stdout.truncated,
        failed,
        milliseconds: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}

/// A workspace on this host. Paths are read as given; commands run from
/// `workdir` inside a writing boundary on it, so a command may build
/// there but can't change anything else. A host that can't enforce the
/// boundary refuses to run the command.
#[derive(Clone, Debug)]
pub struct Local {
    pub workdir: PathBuf,
}

impl Host for Local {
    fn describe(&self) -> Value {
        json!({ "host": "local", "workdir": self.workdir, "confine": "writing" })
    }

    async fn stat(&self, path: &str) -> Result<Stat, String> {
        match std::fs::metadata(path) {
            Ok(meta) if meta.is_dir() => Ok(Stat::Dir),
            Ok(meta) => Ok(Stat::File(meta.len())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Stat::Missing),
            Err(error) => Err(format!("{path}: {error}")),
        }
    }

    async fn read(&self, path: &str, max: usize) -> Result<Option<Vec<u8>>, String> {
        match self.stat(path).await? {
            Stat::File(_) => {
                let mut bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
                bytes.truncate(max);
                Ok(Some(bytes))
            }
            _ => Ok(None),
        }
    }

    async fn run(&self, command: &str, wall: Duration) -> Ran {
        let started = Instant::now();
        let refused = |why: String| Ran {
            failed: Some(why),
            ..Ran::default()
        };
        let boundary = match coder_boundary::Boundary::writing(&self.workdir)
            .owned_scratch_under(std::env::temp_dir())
            .build()
        {
            Ok(boundary) => boundary,
            Err(error) => return refused(format!("no enforced boundary: {error}")),
        };
        let mut built = match boundary.command("/bin/sh", ["-c", command]) {
            Ok(built) => built,
            Err(error) => return refused(error.to_string()),
        };
        built.current_dir(&self.workdir);
        if let Some(scratch) = boundary.scratch() {
            built.env("TMPDIR", scratch);
        }
        let ended = supervise::Job::from_command(built)
            .bounded(supervise::Limits::within(wall).keeping(STDOUT_MAX))
            .run_holding(boundary.hold())
            .await;
        ended_to_ran(&ended, started)
    }
}

/// A running container, reached with `docker exec`. The caller starts it
/// with no network and removes it.
#[derive(Clone, Debug)]
pub struct Container {
    pub id: String,
    pub workdir: String,
    /// Where files copied out of the container land.
    pub scratch: PathBuf,
}

impl Container {
    async fn exec(&self, script: &str, wall: Duration) -> Ran {
        let started = Instant::now();
        let mut command = Command::new("docker");
        command.args(["exec", "-w", &self.workdir, &self.id, "sh", "-c", script]);
        let ended = supervise::Job::from_command(command)
            .bounded(supervise::Limits::within(wall).keeping(STDOUT_MAX))
            .run()
            .await;
        ended_to_ran(&ended, started)
    }
}

impl Host for Container {
    fn describe(&self) -> Value {
        json!({ "host": "docker", "container": self.id, "workdir": self.workdir, "network": "none" })
    }

    async fn stat(&self, path: &str) -> Result<Stat, String> {
        let quoted = crate::accept::runner::sh_quote(path);
        let ran = self
            .exec(
                &format!(
                    "if [ -d {quoted} ]; then echo dir; elif [ -e {quoted} ]; then wc -c < {quoted}; else echo missing; fi"
                ),
                Duration::from_secs(30),
            )
            .await;
        if ran.exit != Some(0) {
            return Err(format!(
                "cannot stat {path}: {}",
                ran.failed.unwrap_or(ran.stderr)
            ));
        }
        match ran.stdout.trim() {
            "dir" => Ok(Stat::Dir),
            "missing" => Ok(Stat::Missing),
            size => Ok(Stat::File(size.trim().parse().unwrap_or(0))),
        }
    }

    async fn read(&self, path: &str, max: usize) -> Result<Option<Vec<u8>>, String> {
        if !matches!(self.stat(path).await?, Stat::File(_)) {
            return Ok(None);
        }
        static COPIES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let into = self.scratch.join(format!(
            "read-{}",
            COPIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&self.scratch).map_err(|e| e.to_string())?;
        let copied = crate::accept::runner::docker(&[
            "cp",
            "-L",
            &format!("{}:{path}", self.id),
            &into.display().to_string(),
        ]);
        let bytes = copied.and_then(|_| read_bounded(&into, max));
        let _ = std::fs::remove_file(&into);
        bytes.map(Some)
    }

    async fn run(&self, command: &str, wall: Duration) -> Ran {
        // The inner `timeout` stops the command inside the container; the
        // outer bound stops the client if the container doesn't answer.
        let seconds = wall.as_secs().max(1);
        let script = format!(
            "if command -v timeout >/dev/null 2>&1; then exec timeout {seconds} sh -c {c}; else exec sh -c {c}; fi",
            c = crate::accept::runner::sh_quote(command)
        );
        let mut ran = self.exec(&script, wall + Duration::from_secs(20)).await;
        if ran.exit == Some(124) {
            ran.timed_out = true;
            ran.exit = None;
        }
        ran
    }
}

fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(max as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// A workspace held in memory, for tests and for plans made from files
/// alone. Commands don't run.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    pub files: std::collections::BTreeMap<String, Vec<u8>>,
    pub dirs: std::collections::BTreeSet<String>,
    /// Canned results by command.
    pub runs: std::collections::BTreeMap<String, Ran>,
}

impl Host for Memory {
    fn describe(&self) -> Value {
        json!({ "host": "memory" })
    }

    async fn stat(&self, path: &str) -> Result<Stat, String> {
        let trimmed = path.trim_end_matches('/');
        if let Some(bytes) = self.files.get(trimmed) {
            return Ok(Stat::File(bytes.len() as u64));
        }
        let prefix = format!("{trimmed}/");
        if self.dirs.contains(trimmed) || self.files.keys().any(|k| k.starts_with(&prefix)) {
            return Ok(Stat::Dir);
        }
        Ok(Stat::Missing)
    }

    async fn read(&self, path: &str, max: usize) -> Result<Option<Vec<u8>>, String> {
        Ok(self.files.get(path).map(|b| b[..b.len().min(max)].to_vec()))
    }

    async fn run(&self, command: &str, _wall: Duration) -> Ran {
        self.runs.get(command).cloned().unwrap_or(Ran {
            exit: Some(127),
            stderr: format!("sh: {command}: not found"),
            ..Ran::default()
        })
    }
}
