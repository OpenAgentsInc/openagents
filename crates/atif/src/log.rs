//! The append-only session log: one line per step, written as it happens.
//!
//! # Why a log and not a document
//!
//! A document is the shape a consumer wants, and it is the wrong shape to
//! write during a session. Rewriting the whole document after every step
//! costs bytes quadratic in the session's length, and — the part that
//! matters — a process killed partway through that rewrite leaves a
//! truncated file that parses as nothing at all. The record would be lost
//! exactly when there is most reason to want it.
//!
//! So a session appends. Each record is one complete JSON object on one
//! line, flushed and synced before the call returns. A session that is
//! killed loses at most the line it was in the middle of writing, and
//! [`read`] skips an unreadable line rather than refusing the file, so
//! everything before it still reads back. [`Recording::document`] renders
//! the ATIF document from the lines on read, which is why the log and the
//! document can never disagree: there is only one of them.
//!
//! # The records
//!
//! ```text
//! {"record":"session","schema_version":"ATIF-v1.7","at":…,"session":{…}}
//! {"record":"step","step":{…}}
//! {"record":"end","at":…,"state":"ended"}
//! ```
//!
//! The `session` record comes first and is never rewritten, so it holds only
//! what is true when the session opens. How the session ended and how long
//! it took are computed on read — from the `end` record when there is one,
//! and from the last step when there is not. A log with no `end` record is
//! a session that was interrupted, and it says so.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::document::{self, Session, Step};

/// The extension a session log carries. Two parts: `.jsonl` says how to
/// read the file, and `.atif` says what the lines mean.
pub const EXTENSION: &str = "atif.jsonl";

/// The state a log with no closing record reads back as.
pub const INTERRUPTED: &str = "interrupted";

/// The state a session that closed itself reads back as.
pub const ENDED: &str = "ended";

/// A session log open for writing.
///
/// Every [`append`](Log::append) writes one line and syncs it. The cost is
/// one `fsync` per step, which for a conversational agent is a handful per
/// turn, each already separated by a network round trip. That is the price
/// of the guarantee, and it is not a price worth haggling over.
#[derive(Debug)]
pub struct Log {
    path: PathBuf,
    file: File,
    steps: usize,
    closed: bool,
}

impl Log {
    /// Opens a new log for `session` in `dir`, creating the directory when
    /// it is missing.
    ///
    /// The file is named for the session and must not already exist: a
    /// session that would overwrite another session's record is a bug, not
    /// a thing to do quietly.
    ///
    /// # Errors
    ///
    /// Returns the underlying filesystem error when the directory cannot be
    /// created or the file cannot be written.
    pub fn create(dir: &Path, session: &Session) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        // These are the user's own conversations. The directory is theirs.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
        }
        let path = dir.join(format!("{}.{EXTENSION}", session.id));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        let header = json!({
            "record": "session",
            "schema_version": document::SCHEMA_VERSION,
            "at": document::now_ms(),
            "session": session,
        });
        write_line(&mut file, &header)?;
        Ok(Log {
            path,
            file,
            steps: 0,
            closed: false,
        })
    }

    /// The file this log writes to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many steps have been written.
    #[must_use]
    pub fn steps(&self) -> usize {
        self.steps
    }

    /// Appends one step and syncs it to disk.
    ///
    /// # Errors
    ///
    /// Returns the underlying filesystem error. A caller that cannot record
    /// should say so and carry on: a trace is evidence about a conversation,
    /// not a part of it.
    pub fn append(&mut self, step: &Step) -> io::Result<()> {
        write_line(&mut self.file, &json!({ "record": "step", "step": step }))?;
        self.steps += 1;
        Ok(())
    }

    /// Closes the log with how the session ended. A second call does
    /// nothing, so an explicit close and a close on drop cannot write two
    /// endings.
    ///
    /// # Errors
    ///
    /// Returns the underlying filesystem error.
    pub fn finish(&mut self, state: &str) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        write_line(
            &mut self.file,
            &json!({ "record": "end", "at": document::now_ms(), "state": state }),
        )
    }
}

/// Writes one record as one line, then flushes and syncs it.
///
/// The sync is the whole point of the module: without it the line sits in
/// the operating system's cache, and a killed process leaves a file that is
/// shorter than what the agent had already done.
fn write_line(file: &mut File, record: &Value) -> io::Result<()> {
    let mut line = serde_json::to_string(record).map_err(io::Error::other)?;
    line.push('\n');
    file.write_all(line.as_bytes())?;
    file.flush()?;
    file.sync_data()
}

/// A session log read back.
#[derive(Clone, Debug)]
pub struct Recording {
    /// The session, with `state`, `seconds`, and `directive` filled in from
    /// what the log turned out to hold.
    pub session: Session,
    /// The steps, in the order they were written.
    pub steps: Vec<Step>,
    /// Lines that did not parse. A session killed mid-write leaves one.
    pub unreadable_lines: usize,
    /// Where the log was read from.
    pub path: PathBuf,
}

impl Recording {
    /// The ATIF document this recording renders as.
    #[must_use]
    pub fn document(&self) -> Value {
        let mut document = document::document(&self.session, &self.steps);
        if let Some(extra) = document.get_mut("extra").and_then(Value::as_object_mut) {
            extra.insert(
                "source_path".to_string(),
                json!(self.path.display().to_string()),
            );
            extra.insert("unreadable_lines".to_string(), json!(self.unreadable_lines));
        }
        document
    }

    /// Whether the session closed itself.
    #[must_use]
    pub fn ended(&self) -> bool {
        self.session.state == ENDED
    }
}

/// Reads a session log.
///
/// # Errors
///
/// Returns the underlying filesystem error, or an `InvalidData` error when
/// the file holds no session record and so is not a session log.
pub fn read(path: &Path) -> io::Result<Recording> {
    let file = File::open(path)?;
    let mut opened: Option<(u64, Session)> = None;
    let mut steps: Vec<Step> = Vec::new();
    let mut closed: Option<(u64, String)> = None;
    let mut unreadable_lines = 0usize;
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Map<String, Value>>(&line) else {
            unreadable_lines += 1;
            continue;
        };
        let at = record.get("at").and_then(Value::as_u64).unwrap_or_default();
        match record.get("record").and_then(Value::as_str) {
            Some("session") => {
                match record
                    .get("session")
                    .cloned()
                    .and_then(|value| serde_json::from_value::<Session>(value).ok())
                {
                    Some(session) => opened = Some((at, session)),
                    None => unreadable_lines += 1,
                }
            }
            Some("step") => match record
                .get("step")
                .cloned()
                .and_then(|value| serde_json::from_value::<Step>(value).ok())
            {
                Some(step) => steps.push(step),
                None => unreadable_lines += 1,
            },
            Some("end") => {
                let state = record
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or(ENDED)
                    .to_string();
                closed = Some((at, state));
            }
            _ => unreadable_lines += 1,
        }
    }
    let Some((started, mut session)) = opened else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} holds no session record", path.display()),
        ));
    };
    let last = closed
        .as_ref()
        .map(|(at, _)| *at)
        .or_else(|| steps.last().map(|step| step.at))
        .unwrap_or(started);
    session.seconds = last.saturating_sub(started) / 1_000;
    session.state = match &closed {
        Some((_, state)) => state.clone(),
        None => INTERRUPTED.to_string(),
    };
    if session.directive.is_empty() {
        session.directive = steps
            .iter()
            .find(|step| step.source == document::Source::User)
            .map(|step| step.message.clone())
            .unwrap_or_default();
    }
    Ok(Recording {
        session,
        steps,
        unreadable_lines,
        path: path.to_path_buf(),
    })
}

/// The directory a session writes to when nothing says otherwise:
/// `~/.openagents/traces`. `None` when the home directory is unknown.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    if home.is_empty() {
        return None;
    }
    Some(PathBuf::from(home).join(".openagents").join("traces"))
}

/// The session logs in `dir`, oldest first. The name carries the start
/// time, so sorting by name sorts by time.
///
/// # Errors
///
/// Returns the underlying filesystem error when the directory cannot be
/// read.
pub fn list(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(EXTENSION))
        })
        .collect();
    found.sort();
    Ok(found)
}

/// A session identifier: the UTC start time, then eight hex digits that
/// separate two sessions started in the same second.
///
/// The time leads so that a directory listing is a history, and the file
/// name is the identifier so that a document and the file it came from
/// cannot drift apart.
#[must_use]
pub fn session_id(at: u64) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.subsec_nanos())
        .unwrap_or_default();
    let salt = u64::from(nanos) ^ (u64::from(std::process::id()) << 19);
    format!("{}-{:08x}", document::stamp(at), salt as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Call, Outcome, Source};

    fn a_session() -> Session {
        Session::opening(
            &session_id(document::now_ms()),
            "a-model",
            "stub",
            "/tmp/repo",
            "0.1.0",
        )
    }

    #[test]
    fn a_log_round_trips_through_a_document() {
        let dir = tempfile::tempdir().unwrap();
        let session = a_session();
        let mut log = Log::create(dir.path(), &session).unwrap();
        log.append(&Step::said(Source::User, "what crates are here"))
            .unwrap();
        log.append(&Step::called(Call {
            id: "call-1".to_string(),
            name: "shell".to_string(),
            arguments: json!({"command": "ls crates", "workdir": "/tmp/repo"}),
            output: "atif\ncoder".to_string(),
            outcome: Outcome::Completed,
            milliseconds: 8,
            purpose: Some("list them".to_string()),
            extra: Map::new(),
        }))
        .unwrap();
        log.append(&Step::said(Source::Agent, "atif and coder"))
            .unwrap();
        log.finish(ENDED).unwrap();

        let recording = read(log.path()).unwrap();
        assert!(recording.ended());
        assert_eq!(recording.steps.len(), 3);
        assert_eq!(recording.unreadable_lines, 0);
        // The directive is the first thing the user asked, which the header
        // could not know when it was written.
        assert_eq!(recording.session.directive, "what crates are here");
        let document = recording.document();
        assert_eq!(document["schema_version"], document::SCHEMA_VERSION);
        assert_eq!(document["final_metrics"]["total_steps"], 3);
        assert_eq!(
            document["steps"][1]["tool_calls"][0]["function_name"],
            "shell"
        );
        assert_eq!(document["extra"]["state"], ENDED);
        assert_eq!(document["extra"]["unreadable_lines"], 0);
    }

    /// A log with no closing record is a session that was interrupted, and
    /// every step it had written still reads.
    #[test]
    fn a_log_without_an_ending_reads_as_interrupted() {
        let dir = tempfile::tempdir().unwrap();
        let session = a_session();
        let mut log = Log::create(dir.path(), &session).unwrap();
        log.append(&Step::said(Source::User, "hello")).unwrap();
        log.append(&Step::said(Source::Agent, "hi")).unwrap();
        let path = log.path().to_path_buf();
        drop(log);

        let recording = read(&path).unwrap();
        assert_eq!(recording.session.state, INTERRUPTED);
        assert_eq!(recording.steps.len(), 2);
        assert_eq!(recording.document()["extra"]["state"], INTERRUPTED);
    }

    /// A process killed in the middle of writing a line leaves a partial
    /// line behind. The lines before it are the record, and they read.
    #[test]
    fn a_half_written_line_does_not_cost_the_lines_before_it() {
        let dir = tempfile::tempdir().unwrap();
        let session = a_session();
        let mut log = Log::create(dir.path(), &session).unwrap();
        log.append(&Step::said(Source::User, "count the crates"))
            .unwrap();
        let path = log.path().to_path_buf();
        drop(log);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(br#"{"record":"step","step":{"at":1,"sour"#)
            .unwrap();
        drop(file);

        let recording = read(&path).unwrap();
        assert_eq!(recording.steps.len(), 1);
        assert_eq!(recording.unreadable_lines, 1);
        assert_eq!(recording.session.state, INTERRUPTED);
        assert_eq!(recording.document()["extra"]["unreadable_lines"], 1);
    }

    /// A file that is not a session log says so rather than reading back as
    /// an empty session.
    #[test]
    fn a_file_with_no_session_record_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-trace.atif.jsonl");
        fs::write(&path, "{\"record\":\"step\",\"step\":{}}\n").unwrap();
        let error = read(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    /// A session never overwrites another session's record.
    #[test]
    fn two_sessions_cannot_share_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let session = a_session();
        let _first = Log::create(dir.path(), &session).unwrap();
        assert!(Log::create(dir.path(), &session).is_err());
    }

    /// The name carries the start time, so a directory listing is a
    /// history and nothing has to open a file to order it.
    #[test]
    fn logs_list_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        for at in [1_758_290_553_000_u64, 1_658_290_553_000, 1_858_290_553_000] {
            let mut session = a_session();
            session.id = format!("{}-000000ff", document::stamp(at));
            Log::create(dir.path(), &session).unwrap();
        }
        fs::write(dir.path().join("notes.txt"), "not a trace").unwrap();
        let names: Vec<String> = list(dir.path())
            .unwrap()
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 3, "{names:?}");
        assert!(names[0] < names[1] && names[1] < names[2], "{names:?}");
        assert!(names[0].starts_with(&document::stamp(1_658_290_553_000)));
    }

    /// A session identifier leads with the time it started.
    #[test]
    fn a_session_id_leads_with_its_start_time() {
        let id = session_id(1_758_290_553_123);
        assert!(id.starts_with("20250919T140233Z-"), "{id}");
        assert_eq!(id.len(), "20250919T140233Z-".len() + 8);
    }
}
