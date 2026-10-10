//! The agent lists of every Coder running on this computer, for the
//! desktop app's Agents panel (#11180).
//!
//! Each running `coder` keeps its background agents in memory
//! ([`crate::Registry`]). A [`Publisher`] writes that list to
//! `~/.openagents/agents/<pid>.json` whenever a row changes, and removes
//! the file when the process ends; [`read`] gathers every live process's
//! list. So a `coder` started in the desktop app's Terminal, or in any
//! other terminal on this computer, shows its agents in the app.
//!
//! **Stop** in the app leaves a request file, `<pid>.<agent id>.stop`,
//! beside the list ([`request_stop`]). The process reads it once
//! ([`Publisher::take_stops`]) and stops the agent the way `/agents` does.
//! Nothing else crosses: the files carry only the rows the agent list
//! already shows.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};

use crate::AgentRow;

/// The schema name a board file carries.
pub const SCHEMA: &str = "openagents.agent-board.v1";
/// The folder under `~/.openagents` the boards live in.
pub const DIR: &str = "agents";
/// The largest board file read, in bytes.
pub const MAX_BYTES: u64 = 1024 * 1024;
/// How often a publisher rewrites an unchanged list, so a reader that
/// can't ask whether the process lives (Windows) can tell a live board
/// from one a crash left behind.
pub const REFRESH: Duration = Duration::from_secs(15);
/// How old an unrefreshed board may be before a reader treats its process
/// as gone, where it can't ask.
pub const STALE: Duration = Duration::from_secs(60);
/// How often a publisher looks for stop requests.
const STOP_POLL: Duration = Duration::from_millis(500);

/// One process's agent list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Board {
    pub schema: String,
    /// The `coder` process that runs these agents.
    pub pid: u32,
    /// Where it was started.
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    pub agents: Vec<AgentRow>,
}

/// `~/.openagents/agents`, given `~/.openagents`.
#[must_use]
pub fn dir(openagents_root: &Path) -> PathBuf {
    openagents_root.join(DIR)
}

/// The board file of process `pid`.
#[must_use]
pub fn path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("{pid}.json"))
}

/// Whether `id` can name a stop request: an agent id such as `agent-3`.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Writes one process's agent list while it runs and removes it after.
#[derive(Debug)]
pub struct Publisher {
    dir: PathBuf,
    pid: u32,
    cwd: Option<PathBuf>,
    /// The rows last written; `None` before the first write.
    last: Option<Vec<AgentRow>>,
    written: Option<Instant>,
    stops_checked: Option<Instant>,
}

impl Publisher {
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>, pid: u32, cwd: Option<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            pid,
            cwd,
            last: None,
            written: None,
            stops_checked: None,
        }
    }

    /// This process's board file.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        path(&self.dir, self.pid)
    }

    /// Writes `rows` when they differ from the last write, or when the
    /// last write is older than [`REFRESH`]. An empty list removes the
    /// file. Returns whether the file changed.
    ///
    /// # Errors
    /// The folder or file can't be written.
    pub fn publish(&mut self, rows: &[AgentRow]) -> std::io::Result<bool> {
        self.publish_at(rows, Instant::now())
    }

    fn publish_at(&mut self, rows: &[AgentRow], now: Instant) -> std::io::Result<bool> {
        let same = self.last.as_deref() == Some(rows);
        if rows.is_empty() {
            if self.last.as_ref().is_some_and(|last| !last.is_empty()) {
                remove(&self.path());
                self.last = Some(Vec::new());
                self.written = Some(now);
                return Ok(true);
            }
            self.last = Some(Vec::new());
            return Ok(false);
        }
        let due = self
            .written
            .is_none_or(|written| now.saturating_duration_since(written) >= REFRESH);
        if same && !due {
            return Ok(false);
        }
        let board = Board {
            schema: SCHEMA.into(),
            pid: self.pid,
            cwd: self.cwd.clone(),
            agents: rows.to_vec(),
        };
        let bytes = serde_json::to_vec(&board).map_err(std::io::Error::other)?;
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path();
        let partial = self.dir.join(format!("{}.json.partial", self.pid));
        std::fs::write(&partial, bytes)?;
        std::fs::rename(&partial, &path)?;
        self.last = Some(rows.to_vec());
        self.written = Some(now);
        Ok(true)
    }

    /// The agents someone asked to stop since the last look, each read
    /// once. Looks at most every half second.
    #[must_use]
    pub fn take_stops(&mut self) -> Vec<String> {
        let now = Instant::now();
        if self
            .stops_checked
            .is_some_and(|checked| now.saturating_duration_since(checked) < STOP_POLL)
        {
            return Vec::new();
        }
        self.stops_checked = Some(now);
        self.take_stops_now()
    }

    fn take_stops_now(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let prefix = format!("{}.", self.pid);
        let mut ids: Vec<String> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let id = name.strip_prefix(&prefix)?.strip_suffix(".stop")?;
                let id = valid_id(id).then(|| id.to_owned());
                remove(&entry.path());
                id
            })
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }
}

impl Drop for Publisher {
    fn drop(&mut self) {
        if self.last.as_ref().is_some_and(|last| !last.is_empty()) {
            remove(&self.path());
        }
        let _ = self.take_stops_now();
    }
}

fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// Asks process `pid` to stop agent `id`.
///
/// # Errors
/// `id` is not an agent id, or the request can't be written.
pub fn request_stop(dir: &Path, pid: u32, id: &str) -> std::io::Result<()> {
    if !valid_id(id) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not an agent id",
        ));
    }
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(format!("{pid}.{id}.stop")), b"")
}

/// Every board under `dir` whose process `alive` says still runs, by
/// process id. `alive` gets the process id and when its board was last
/// written; [`alive_by_age`] is the answer where the system can't be asked.
/// Unreadable, oversized, or foreign files are skipped.
#[must_use]
pub fn read(dir: &Path, alive: impl Fn(u32, SystemTime) -> bool) -> Vec<Board> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut boards: Vec<Board> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let pid: u32 = name.strip_suffix(".json")?.parse().ok()?;
            let meta = entry.metadata().ok()?;
            if !meta.is_file() || meta.len() > MAX_BYTES {
                return None;
            }
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if !alive(pid, modified) {
                return None;
            }
            let bytes = std::fs::read(entry.path()).ok()?;
            let board: Board = serde_json::from_slice(&bytes).ok()?;
            (board.schema == SCHEMA && board.pid == pid).then_some(board)
        })
        .collect();
    boards.sort_by_key(|board| board.pid);
    boards
}

/// Whether a board written at `modified` is recent enough to trust
/// without asking the system about its process.
#[must_use]
pub fn alive_by_age(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified).is_ok_and(|age| age <= STALE) || modified > now
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Status;

    fn row(id: &str, status: Status) -> AgentRow {
        AgentRow {
            id: id.into(),
            name: format!("{id}-name"),
            engine: "codex".into(),
            place: "this computer".into(),
            status,
            task: "fix the flaky login test".into(),
            started_ms: 1_000,
            ended_ms: None,
            earlier_seconds: 0,
            tokens: 0,
            cost_usd: None,
            worktree: None,
            branch: None,
            parent_session: None,
            transcript: None,
            report: None,
            error: None,
            pending_messages: 0,
            runs: 1,
            run_started_ms: 1_000,
        }
    }

    fn everyone(_: u32, _: SystemTime) -> bool {
        true
    }

    #[test]
    fn a_publisher_writes_changes_and_removes_its_board_when_dropped() {
        let home = tempfile::tempdir().unwrap();
        let dir = dir(home.path());
        let mut publisher = Publisher::new(&dir, 4242, Some("/work".into()));
        assert!(!publisher.publish(&[]).unwrap());
        assert!(!publisher.path().exists());

        let rows = vec![row("agent-1", Status::Running)];
        assert!(publisher.publish(&rows).unwrap());
        assert!(!publisher.publish(&rows).unwrap(), "unchanged rows wait");
        let boards = read(&dir, everyone);
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].pid, 4242);
        assert_eq!(boards[0].cwd.as_deref(), Some(Path::new("/work")));
        assert_eq!(boards[0].agents, rows);

        let mut done = rows.clone();
        done[0].status = Status::Done;
        done[0].tokens = 1200;
        assert!(publisher.publish(&done).unwrap());
        assert_eq!(read(&dir, everyone)[0].agents[0].status, Status::Done);

        drop(publisher);
        assert!(read(&dir, everyone).is_empty());
        assert!(!path(&dir, 4242).exists());
    }

    #[test]
    fn an_unchanged_list_is_rewritten_after_the_refresh() {
        let home = tempfile::tempdir().unwrap();
        let mut publisher = Publisher::new(dir(home.path()), 7, None);
        let rows = vec![row("agent-1", Status::Running)];
        let start = Instant::now();
        assert!(publisher.publish_at(&rows, start).unwrap());
        assert!(!publisher.publish_at(&rows, start + REFRESH / 2).unwrap());
        assert!(publisher.publish_at(&rows, start + REFRESH).unwrap());
    }

    #[test]
    fn an_emptied_list_removes_the_board() {
        let home = tempfile::tempdir().unwrap();
        let dir = dir(home.path());
        let mut publisher = Publisher::new(&dir, 9, None);
        publisher
            .publish(&[row("agent-1", Status::Running)])
            .unwrap();
        assert!(publisher.path().exists());
        assert!(publisher.publish(&[]).unwrap());
        assert!(!publisher.path().exists());
    }

    #[test]
    fn readers_skip_dead_processes_and_foreign_files() {
        let home = tempfile::tempdir().unwrap();
        let dir = dir(home.path());
        let mut live = Publisher::new(&dir, 10, None);
        live.publish(&[row("agent-1", Status::Running)]).unwrap();
        let mut dead = Publisher::new(&dir, 11, None);
        dead.publish(&[row("agent-2", Status::Running)]).unwrap();
        std::fs::write(dir.join("notes.json"), b"{}").unwrap();
        std::fs::write(dir.join("12.json"), b"not json").unwrap();
        // A board whose pid disagrees with its name.
        let mut other = Board {
            schema: SCHEMA.into(),
            pid: 99,
            cwd: None,
            agents: vec![],
        };
        std::fs::write(dir.join("13.json"), serde_json::to_vec(&other).unwrap()).unwrap();
        other.pid = 14;
        other.schema = "something.else".into();
        std::fs::write(dir.join("14.json"), serde_json::to_vec(&other).unwrap()).unwrap();

        let boards = read(&dir, |pid, _| pid != 11);
        assert_eq!(
            boards.iter().map(|board| board.pid).collect::<Vec<_>>(),
            vec![10]
        );
        assert!(read(&home.path().join("missing"), everyone).is_empty());
    }

    #[test]
    fn a_stop_request_is_read_once_by_its_process_only() {
        let home = tempfile::tempdir().unwrap();
        let dir = dir(home.path());
        let mut mine = Publisher::new(&dir, 20, None);
        let mut theirs = Publisher::new(&dir, 21, None);
        request_stop(&dir, 20, "agent-3").unwrap();
        request_stop(&dir, 20, "agent-3").unwrap();
        request_stop(&dir, 21, "agent-1").unwrap();
        assert!(request_stop(&dir, 20, "../escape").is_err());
        assert!(request_stop(&dir, 20, "").is_err());

        assert_eq!(mine.take_stops(), vec!["agent-3".to_owned()]);
        mine.stops_checked = None;
        assert!(mine.take_stops().is_empty(), "read once");
        assert_eq!(theirs.take_stops(), vec!["agent-1".to_owned()]);
    }

    #[test]
    fn age_stands_in_for_asking_about_the_process() {
        let now = SystemTime::now();
        assert!(alive_by_age(now, now));
        assert!(alive_by_age(now - STALE, now));
        assert!(!alive_by_age(now - STALE - Duration::from_secs(1), now));
        assert!(alive_by_age(now + Duration::from_secs(5), now));
    }

    #[test]
    fn agent_ids_are_the_only_request_names() {
        assert!(valid_id("agent-12"));
        assert!(!valid_id("Agent"));
        assert!(!valid_id("a/b"));
        assert!(!valid_id("a.b"));
        assert!(!valid_id(&"a".repeat(65)));
    }
}
