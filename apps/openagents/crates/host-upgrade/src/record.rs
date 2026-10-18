//! The upgrade's files in `<data>/host-upgrade`: the bundle handed to the
//! app (`pending.json`), the log (`upgrades.jsonl`), and the lock that keeps
//! upgrades one at a time (`upgrade.lock`).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const PENDING_FILE: &str = "pending.json";
pub const LOG_FILE: &str = "upgrades.jsonl";
pub const LOCK_FILE: &str = "upgrade.lock";
/// The chats whose queued messages run on once the app reopens.
pub const RESUME_QUEUES_FILE: &str = "resume-queues.json";
pub const LOG_SCHEMA: &str = "openagents.host-upgrade.v1";
/// Written by an app that takes hand-overs: its pid.
pub const WATCHER_FILE: &str = "watcher.pid";

/// A built and checked bundle, handed to the app to swap in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub to: String,
    /// The staged bundle.
    pub staged: PathBuf,
    /// The bundle it replaces.
    pub bundle: PathBuf,
    /// Where the replaced bundle goes.
    pub previous: PathBuf,
    /// The agent (chat ID) that asked, or `owner`.
    pub requested_by: String,
    pub reason: String,
    pub requested_at_ms: u64,
    pub phase: Phase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Phase {
    /// Waiting for a quiet moment in the app.
    Ready,
    /// Swapped in; the app is restarting onto `chat`.
    #[serde(rename_all = "camelCase")]
    Restarting {
        at_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chat: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The new app reopened on the new commit.
    Installed,
    /// `main` is what's running.
    UpToDate,
    /// The build failed; the old app kept running.
    BuildFailed,
    /// The built app failed a check; the old app kept running.
    VerifyFailed,
    /// The new app didn't come back, so the old one was put back.
    RolledBack,
    /// No grant, or another upgrade is running.
    Refused,
    /// Anything else that stopped it, such as fetching `main`.
    Failed,
    /// Handed to the app, which hasn't found a quiet moment yet. Returned,
    /// never logged: the app logs the upgrade when it reopens.
    Waiting,
}

impl Outcome {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Outcome::Installed => "installed",
            Outcome::UpToDate => "already up to date",
            Outcome::BuildFailed => "build failed",
            Outcome::VerifyFailed => "check failed",
            Outcome::RolledBack => "rolled back",
            Outcome::Refused => "refused",
            Outcome::Failed => "failed",
            Outcome::Waiting => "waiting for a quiet moment",
        }
    }
}

/// One upgrade in the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub schema: String,
    pub id: String,
    /// When, in Chicago time.
    pub at: String,
    pub at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub requested_by: String,
    pub reason: String,
    pub outcome: Outcome,
    pub detail: String,
}

impl LogEntry {
    #[must_use]
    pub fn new(
        id: &str,
        at_ms: u64,
        requested_by: &str,
        reason: &str,
        outcome: Outcome,
        detail: impl Into<String>,
    ) -> LogEntry {
        LogEntry {
            schema: LOG_SCHEMA.to_owned(),
            id: id.to_owned(),
            at: chicago(at_ms),
            at_ms,
            from: None,
            to: None,
            requested_by: requested_by.to_owned(),
            reason: reason.to_owned(),
            outcome,
            detail: detail.into(),
        }
    }

    #[must_use]
    pub fn commits(mut self, from: Option<&str>, to: Option<&str>) -> LogEntry {
        self.from = from.map(str::to_owned);
        self.to = to.map(str::to_owned);
        self
    }

    /// One line for people: `installed 0123456789 → abcdef0123 at 9:41:02 PM CDT`.
    #[must_use]
    pub fn summary(&self) -> String {
        let commits = match (&self.from, &self.to) {
            (Some(from), Some(to)) => format!(" {} → {}", crate::short(from), crate::short(to)),
            (None, Some(to)) => format!(" to {}", crate::short(to)),
            _ => String::new(),
        };
        let detail = if self.detail.is_empty() {
            String::new()
        } else {
            format!(": {}", self.detail)
        };
        format!("{}{commits} at {}{detail}", self.outcome.label(), self.at)
    }
}

/// `at_ms` in Chicago time, like the build sweep's log.
#[must_use]
pub fn chicago(at_ms: u64) -> String {
    coder_lease::agent_builds::sweep::chicago(
        std::time::UNIX_EPOCH + std::time::Duration::from_millis(at_ms),
    )
}

/// The bundle handed to the app, if any.
#[must_use]
pub fn read_pending(state_dir: &Path) -> Option<Pending> {
    let text = std::fs::read_to_string(state_dir.join(PENDING_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Hand `pending` to the app (or record its new phase).
///
/// # Errors
/// The file can't be written.
pub fn write_pending(state_dir: &Path, pending: &Pending) -> std::io::Result<()> {
    write_atomic(
        &state_dir.join(PENDING_FILE),
        &serde_json::to_vec_pretty(pending).map_err(std::io::Error::other)?,
    )
}

pub fn clear_pending(state_dir: &Path) {
    let _ = std::fs::remove_file(state_dir.join(PENDING_FILE));
}

/// Append `entry` to the log.
///
/// # Errors
/// The log can't be written.
pub fn log(state_dir: &Path, entry: &LogEntry) -> std::io::Result<()> {
    std::fs::create_dir_all(state_dir)?;
    let mut line = serde_json::to_vec(entry).map_err(std::io::Error::other)?;
    line.push(b'\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state_dir.join(LOG_FILE))?;
    file.write_all(&line)
}

/// The log, oldest first. Lines that don't parse are skipped.
#[must_use]
pub fn read_log(state_dir: &Path) -> Vec<LogEntry> {
    std::fs::read_to_string(state_dir.join(LOG_FILE))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// The last log entry for upgrade `id`.
#[must_use]
pub fn outcome_of(state_dir: &Path, id: &str) -> Option<LogEntry> {
    read_log(state_dir)
        .into_iter()
        .rev()
        .find(|entry| entry.id == id)
}

/// The chats whose queues run on after the restart, then forgets them.
#[must_use]
pub fn take_resume_queues(state_dir: &Path) -> Vec<String> {
    let path = state_dir.join(RESUME_QUEUES_FILE);
    let chats = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let _ = std::fs::remove_file(path);
    chats
}

/// Record the chats whose queues run on after the restart.
///
/// # Errors
/// The file can't be written.
pub fn write_resume_queues(state_dir: &Path, chats: &[String]) -> std::io::Result<()> {
    write_atomic(
        &state_dir.join(RESUME_QUEUES_FILE),
        &serde_json::to_vec(chats).map_err(std::io::Error::other)?,
    )
}

/// Record that process `pid`, the app, takes hand-overs.
///
/// # Errors
/// The file can't be written.
pub fn write_watcher(state_dir: &Path, pid: u32) -> std::io::Result<()> {
    write_atomic(&state_dir.join(WATCHER_FILE), pid.to_string().as_bytes())
}

/// The pid of the app that last said it takes hand-overs.
#[must_use]
pub fn watcher(state_dir: &Path) -> Option<u32> {
    std::fs::read_to_string(state_dir.join(WATCHER_FILE))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// One upgrade at a time: held while the upgrader runs, released on drop.
#[derive(Debug)]
pub struct Lock {
    path: PathBuf,
}

impl Lock {
    /// Take the lock, unless a live process holds it.
    ///
    /// # Errors
    /// Another upgrade is running (its pid), or the file can't be written.
    pub fn take(state_dir: &Path, alive: &dyn Fn(u32) -> bool) -> Result<Lock, String> {
        std::fs::create_dir_all(state_dir)
            .map_err(|e| format!("couldn't create {}: {e}", state_dir.display()))?;
        let path = state_dir.join(LOCK_FILE);
        for _ in 0..2 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let _ = write!(file, "{}", std::process::id());
                    return Ok(Lock { path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let holder = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|text| text.trim().parse::<u32>().ok());
                    match holder {
                        Some(pid) if pid == std::process::id() || alive(pid) => {
                            return Err(format!("another upgrade is running (pid {pid})"));
                        }
                        _ => {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
                Err(e) => return Err(format!("couldn't take {}: {e}", path.display())),
            }
        }
        Err(format!("couldn't take {}", path.display()))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_reads_back_and_summarizes() {
        let dir = tempfile::tempdir().unwrap();
        let entry = LogEntry::new(
            "u1",
            0,
            "owner",
            "test",
            Outcome::BuildFailed,
            "error[E0425]",
        )
        .commits(Some("0123456789abcdef"), Some("fedcba9876543210"));
        log(dir.path(), &entry).unwrap();
        log(
            dir.path(),
            &LogEntry::new("u2", 0, "owner", "test", Outcome::UpToDate, ""),
        )
        .unwrap();
        assert_eq!(read_log(dir.path()).len(), 2);
        let found = outcome_of(dir.path(), "u1").unwrap();
        assert_eq!(found, entry);
        assert!(
            found
                .summary()
                .starts_with("build failed 0123456789 → fedcba9876 at "),
            "{}",
            found.summary()
        );
        assert!(found.summary().ends_with(": error[E0425]"));
    }

    #[test]
    fn the_lock_refuses_a_live_holder_and_takes_a_dead_ones() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LOCK_FILE), "4242").unwrap();
        let err = Lock::take(dir.path(), &|pid| pid == 4242).unwrap_err();
        assert!(err.contains("4242"), "{err}");
        let lock = Lock::take(dir.path(), &|_| false).unwrap();
        assert!(Lock::take(dir.path(), &|_| true).is_err());
        drop(lock);
        assert!(!dir.path().join(LOCK_FILE).exists());
    }

    #[test]
    fn resume_queues_are_read_once() {
        let dir = tempfile::tempdir().unwrap();
        write_resume_queues(dir.path(), &["c1".into(), "c2".into()]).unwrap();
        assert_eq!(take_resume_queues(dir.path()), vec!["c1", "c2"]);
        assert!(take_resume_queues(dir.path()).is_empty());
    }
}
