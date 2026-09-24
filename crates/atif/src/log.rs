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
//! # Two ways to read
//!
//! [`read`] recovers. It splits the file into lines as bytes before it
//! decodes any of them, so a final record torn in the middle of a
//! multibyte character costs that record and nothing before it. Every line
//! that did not contribute is a [`Fault`] on the recording, with its line
//! number and what was wrong, and the recording says whether the session
//! ended or was interrupted. That is the reader for a person looking at a
//! trace.
//!
//! [`read_whole`] is the reader for evidence. It refuses a log with any
//! fault, and a log with no `end` record, so a recovered prefix cannot pass
//! as a complete session. A grader reads through it, or checks
//! [`Recording::whole`] itself, and treats anything less as unverifiable.
//!
//! # The lifecycle a log holds to
//!
//! One `session` record, first. Then steps. Then at most one `end` record,
//! last. A second `session` record, a second `end`, or any record after
//! `end` is a fault: the first header and the first ending stand, and the
//! record at fault is not read. A line that is not UTF-8, not JSON, not a
//! known record, or a record whose payload does not read as its type is a
//! fault too. A final line with no newline was never finished, so it is a
//! fault even when its bytes happen to parse.
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

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
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
        Self::create_at(&dir.join(format!("{}.{EXTENSION}", session.id)), session)
    }

    /// Opens a new log for `session` at `path`, creating the parent
    /// directory when it is missing.
    ///
    /// A caller that names the file owns the name, so this does not derive
    /// one from the session. The file must not already exist, for the same
    /// reason [`Log::create`] refuses to overwrite one.
    ///
    /// # Errors
    ///
    /// Returns the underlying filesystem error when the directory cannot be
    /// created or the file cannot be written.
    pub fn create_at(path: &Path, session: &Session) -> io::Result<Self> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let path = path.to_path_buf();
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
    /// not a part of it. Returns an `InvalidInput` error, and writes
    /// nothing, when the log has been [finished](Log::finish): a step after
    /// the end would be a fault on read.
    pub fn append(&mut self, step: &Step) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} is finished; a step cannot follow the end",
                    self.path.display()
                ),
            ));
        }
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
    /// Lines that did not contribute a record: the count of
    /// [`faults`](Self::faults). A session killed mid-write leaves one.
    pub unreadable_lines: usize,
    /// Every line that did not contribute a record, in file order, with
    /// what was wrong with it.
    pub faults: Vec<Fault>,
    /// Where the log was read from.
    pub path: PathBuf,
}

impl Recording {
    /// The ATIF document this recording renders as.
    ///
    /// `extra` carries `source_path`, `unreadable_lines`, and `faults`, so
    /// a document rendered from a damaged log says so.
    #[must_use]
    pub fn document(&self) -> Value {
        let mut document = document::document(&self.session, &self.steps);
        if let Some(extra) = document.get_mut("extra").and_then(Value::as_object_mut) {
            extra.insert(
                "source_path".to_string(),
                json!(self.path.display().to_string()),
            );
            extra.insert("unreadable_lines".to_string(), json!(self.unreadable_lines));
            extra.insert(
                "faults".to_string(),
                Value::Array(
                    self.faults
                        .iter()
                        .map(|fault| json!({"line": fault.line, "fault": fault.kind.word()}))
                        .collect(),
                ),
            );
        }
        document
    }

    /// Whether the session closed itself.
    #[must_use]
    pub fn ended(&self) -> bool {
        self.session.state == ENDED
    }

    /// Whether the log is fit for evidence: every line read as a record and
    /// the session ended. A recovered prefix is not whole, however much of
    /// it there is.
    #[must_use]
    pub fn whole(&self) -> bool {
        self.faults.is_empty() && self.ended()
    }
}

/// One line of a log that did not contribute a record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    /// The line, counting from 1.
    pub line: usize,
    /// What was wrong with it.
    pub kind: FaultKind,
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.kind)
    }
}

/// What kept a line from being read as a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultKind {
    /// The final line has no newline: the writer did not finish it.
    Torn,
    /// The bytes are not UTF-8.
    NotUtf8,
    /// The text is not a JSON object.
    NotJson,
    /// The object's `record` names no record kind this reader knows.
    UnknownRecord,
    /// A `session` record whose payload does not read as a session.
    BadSession,
    /// A `step` record whose payload does not read as a step.
    BadStep,
    /// A `session` record after the first; the first stands.
    RepeatedSession,
    /// An `end` record after the first; the first stands.
    RepeatedEnd,
    /// A record after the `end` record.
    AfterEnd,
    /// A record before the `session` record.
    BeforeSession,
}

impl FaultKind {
    /// The fault as one `snake_case` word, for a document.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Torn => "torn",
            Self::NotUtf8 => "not_utf8",
            Self::NotJson => "not_json",
            Self::UnknownRecord => "unknown_record",
            Self::BadSession => "bad_session",
            Self::BadStep => "bad_step",
            Self::RepeatedSession => "repeated_session",
            Self::RepeatedEnd => "repeated_end",
            Self::AfterEnd => "after_end",
            Self::BeforeSession => "before_session",
        }
    }
}

impl fmt::Display for FaultKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Torn => "the last line was never finished",
            Self::NotUtf8 => "the line is not UTF-8",
            Self::NotJson => "the line is not a JSON object",
            Self::UnknownRecord => "the record kind is unknown",
            Self::BadSession => "the session record is not a valid session",
            Self::BadStep => "the step record is not a valid step",
            Self::RepeatedSession => "the log has a second session record",
            Self::RepeatedEnd => "the log has a second end record",
            Self::AfterEnd => "a record comes after the end record",
            Self::BeforeSession => "a record comes before the session record",
        })
    }
}

/// Reads a session log, recovering what it can.
///
/// Every line that did not contribute is a [`Fault`] on the recording;
/// [`Recording::whole`] says whether there were none and the session
/// ended. Read evidence through [`read_whole`] instead.
///
/// # Errors
///
/// Returns the underlying filesystem error, or an `InvalidData` error when
/// the file holds no session record and so is not a session log.
pub fn read(path: &Path) -> io::Result<Recording> {
    let bytes = fs::read(path)?;
    let mut opened: Option<(u64, Session)> = None;
    let mut steps: Vec<Step> = Vec::new();
    let mut closed: Option<(u64, String)> = None;
    let mut faults: Vec<Fault> = Vec::new();

    let mut lines: Vec<&[u8]> = bytes.split(|byte| *byte == b'\n').collect();
    // A file that ends in a newline splits into one empty piece past the
    // last line; a file that does not ends in a line the writer never
    // finished.
    let torn = match lines.last() {
        Some([]) => {
            lines.pop();
            false
        }
        Some(_) => true,
        None => false,
    };
    let count = lines.len();

    for (index, raw) in lines.into_iter().enumerate() {
        let number = index + 1;
        let mut fault = |kind: FaultKind| faults.push(Fault { line: number, kind });
        if torn && number == count {
            fault(FaultKind::Torn);
            continue;
        }
        let Ok(line) = std::str::from_utf8(raw) else {
            fault(FaultKind::NotUtf8);
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Map<String, Value>>(line) else {
            fault(FaultKind::NotJson);
            continue;
        };
        let kind = record.get("record").and_then(Value::as_str);
        if !matches!(kind, Some("session" | "step" | "end")) {
            fault(FaultKind::UnknownRecord);
            continue;
        }
        if closed.is_some() {
            fault(if kind == Some("end") {
                FaultKind::RepeatedEnd
            } else {
                FaultKind::AfterEnd
            });
            continue;
        }
        if opened.is_none() && kind != Some("session") {
            fault(FaultKind::BeforeSession);
            continue;
        }
        let at = record.get("at").and_then(Value::as_u64).unwrap_or_default();
        match kind {
            Some("session") if opened.is_some() => fault(FaultKind::RepeatedSession),
            Some("session") => {
                match record
                    .get("session")
                    .cloned()
                    .and_then(|value| serde_json::from_value::<Session>(value).ok())
                {
                    Some(session) => opened = Some((at, session)),
                    None => fault(FaultKind::BadSession),
                }
            }
            Some("step") => match record
                .get("step")
                .cloned()
                .and_then(|value| serde_json::from_value::<Step>(value).ok())
            {
                Some(step) => steps.push(step),
                None => fault(FaultKind::BadStep),
            },
            _ => {
                let state = record
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or(ENDED)
                    .to_string();
                closed = Some((at, state));
            }
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
        unreadable_lines: faults.len(),
        faults,
        path: path.to_path_buf(),
    })
}

/// Reads a session log as evidence.
///
/// # Errors
///
/// Returns what [`read`] returns, and an `InvalidData` error naming the
/// first fault when any line did not read, or saying the session was
/// interrupted when it holds no `end` record. Nothing recovered passes.
pub fn read_whole(path: &Path) -> io::Result<Recording> {
    let recording = read(path)?;
    if let Some(fault) = recording.faults.first() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} is damaged at {fault}{}",
                path.display(),
                match recording.faults.len() {
                    1 => String::new(),
                    more => format!(" and {} more", more - 1),
                }
            ),
        ));
    }
    if !recording.ended() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} holds no end record: the session was interrupted",
                path.display()
            ),
        ));
    }
    Ok(recording)
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

    /// A caller that names the file gets that file, missing parent
    /// directories and all, and still never writes over an existing
    /// record.
    #[test]
    fn a_named_log_lands_where_it_was_told_to() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runs").join("one.atif.jsonl");
        let mut log = Log::create_at(&path, &a_session()).unwrap();
        assert_eq!(log.path(), path);
        log.append(&Step::said(Source::User, "count the crates"))
            .unwrap();
        log.finish(ENDED).unwrap();

        let recording = read(&path).unwrap();
        assert!(recording.ended());
        assert_eq!(recording.session.directive, "count the crates");
        assert!(Log::create_at(&path, &a_session()).is_err());
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

    /// A final record torn at any byte — including inside a multibyte
    /// character — costs that record and nothing before it, and the
    /// recording says a line was torn. The whole reader refuses every one
    /// of those files.
    #[test]
    fn a_final_record_torn_at_any_byte_keeps_the_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let session = a_session();
        let mut log = Log::create(dir.path(), &session).unwrap();
        log.append(&Step::said(Source::User, "count the crates"))
            .unwrap();
        let path = log.path().to_path_buf();
        drop(log);
        let whole = fs::read(&path).unwrap();
        let final_line = serde_json::to_string(&json!({
            "record": "step",
            "step": Step::said(Source::Agent, "héllo — 日本語 🦀"),
        }))
        .unwrap();
        let final_bytes = final_line.as_bytes();
        assert!(final_bytes.iter().any(|byte| *byte >= 0x80), "non-ASCII");

        for cut in 1..final_bytes.len() {
            let mut torn = whole.clone();
            torn.extend_from_slice(&final_bytes[..cut]);
            fs::write(&path, &torn).unwrap();

            let recording = read(&path).unwrap_or_else(|e| panic!("cut at {cut}: {e}"));
            assert_eq!(recording.steps.len(), 1, "cut at {cut}");
            assert_eq!(recording.session.state, INTERRUPTED, "cut at {cut}");
            assert_eq!(
                recording.faults,
                vec![Fault {
                    line: 3,
                    kind: FaultKind::Torn
                }],
                "cut at {cut}"
            );
            assert_eq!(recording.unreadable_lines, 1);
            assert!(!recording.whole());
            let error = read_whole(&path).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "cut at {cut}");
            assert!(error.to_string().contains("line 3"), "{error}");
        }

        // The finished line, newline and all, reads as the second step.
        let mut done = whole.clone();
        done.extend_from_slice(final_bytes);
        done.push(b'\n');
        fs::write(&path, &done).unwrap();
        let recording = read(&path).unwrap();
        assert_eq!(recording.steps.len(), 2);
        assert_eq!(recording.steps[1].message, "héllo — 日本語 🦀");
        assert!(recording.faults.is_empty());
    }

    /// A line that is not UTF-8 in the middle of a log is a fault on that
    /// line, and the lines on either side of it read.
    #[test]
    fn an_interior_line_that_is_not_utf8_is_a_fault_on_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = Log::create(dir.path(), &a_session()).unwrap();
        log.append(&Step::said(Source::User, "one")).unwrap();
        let path = log.path().to_path_buf();
        drop(log);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"record\":\"step\",\"step\":\"\xff\xfe\"}\n")
            .unwrap();
        file.write_all(b"not json\n").unwrap();
        file.write_all(b"{\"record\":\"note\"}\n").unwrap();
        file.write_all(b"{\"record\":\"step\",\"step\":{\"at\":\"soon\"}}\n")
            .unwrap();
        drop(file);
        let mut log = Log::create_at(&dir.path().join("other.atif.jsonl"), &a_session()).unwrap();
        log.append(&Step::said(Source::Agent, "two")).unwrap();
        log.finish(ENDED).unwrap();
        let tail = fs::read(log.path()).unwrap();
        let tail = tail.split(|b| *b == b'\n').nth(1).unwrap().to_vec();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&tail).unwrap();
        file.write_all(b"\n").unwrap();
        drop(file);

        let recording = read(&path).unwrap();
        assert_eq!(recording.steps.len(), 2);
        let kinds: Vec<FaultKind> = recording.faults.iter().map(|f| f.kind).collect();
        assert_eq!(
            kinds,
            vec![
                FaultKind::NotUtf8,
                FaultKind::NotJson,
                FaultKind::UnknownRecord,
                FaultKind::BadStep
            ]
        );
        let lines: Vec<usize> = recording.faults.iter().map(|f| f.line).collect();
        assert_eq!(lines, vec![3, 4, 5, 6]);
        let document = recording.document();
        assert_eq!(document["extra"]["faults"][0]["fault"], "not_utf8");
        assert_eq!(document["extra"]["faults"][0]["line"], 3);
    }

    /// One header, then steps, then at most one end. Whatever breaks that
    /// order is a fault, the first header and ending stand, and the whole
    /// reader refuses the file.
    #[test]
    fn the_lifecycle_is_one_header_then_steps_then_one_end() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("life.atif.jsonl");
        let mut first = a_session();
        first.id = "first".to_string();
        let mut second = a_session();
        second.id = "second".to_string();
        let line = |record: Value| {
            let mut line = serde_json::to_string(&record).unwrap();
            line.push('\n');
            line
        };
        let header = |session: &Session| {
            line(
                json!({"record":"session","schema_version":document::SCHEMA_VERSION,
                "at":1_000,"session":session}),
            )
        };
        let step =
            |text: &str| line(json!({"record":"step","step":Step::said(Source::User, text)}));
        let end = |state: &str| line(json!({"record":"end","at":5_000,"state":state}));

        let mut body = String::new();
        body.push_str(&step("early"));
        body.push_str(&header(&first));
        body.push_str(&step("one"));
        body.push_str(&header(&second));
        body.push_str(&step("two"));
        body.push_str(&end(ENDED));
        body.push_str(&step("late"));
        body.push_str(&end("failed"));
        fs::write(&path, body).unwrap();

        let recording = read(&path).unwrap();
        assert_eq!(recording.session.id, "first");
        assert_eq!(recording.session.state, ENDED);
        assert_eq!(recording.session.seconds, 4);
        let messages: Vec<&str> = recording.steps.iter().map(|s| s.message.as_str()).collect();
        assert_eq!(messages, vec!["one", "two"]);
        assert_eq!(
            recording.faults,
            vec![
                Fault {
                    line: 1,
                    kind: FaultKind::BeforeSession
                },
                Fault {
                    line: 4,
                    kind: FaultKind::RepeatedSession
                },
                Fault {
                    line: 7,
                    kind: FaultKind::AfterEnd
                },
                Fault {
                    line: 8,
                    kind: FaultKind::RepeatedEnd
                },
            ]
        );
        let error = read_whole(&path).unwrap_err();
        assert!(error.to_string().contains("and 3 more"), "{error}");

        // Two endings with nothing between them: the second is repeated.
        fs::write(
            &path,
            format!("{}{}{}", header(&first), end(ENDED), end("failed")),
        )
        .unwrap();
        let recording = read(&path).unwrap();
        assert_eq!(recording.session.state, ENDED);
        assert_eq!(recording.faults[0].kind, FaultKind::RepeatedEnd);
    }

    /// A finished log takes no more steps, so a writer cannot produce the
    /// after-end fault a reader would refuse.
    #[test]
    fn a_finished_log_refuses_another_step() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = Log::create(dir.path(), &a_session()).unwrap();
        log.finish(ENDED).unwrap();
        let error = log.append(&Step::said(Source::User, "more")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(log.steps(), 0);
        let recording = read_whole(log.path()).unwrap();
        assert!(recording.whole());
        assert!(recording.steps.is_empty());
    }

    /// The whole reader refuses an interrupted session even when every line
    /// of it reads.
    #[test]
    fn the_whole_reader_refuses_an_interrupted_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = Log::create(dir.path(), &a_session()).unwrap();
        log.append(&Step::said(Source::User, "hello")).unwrap();
        let path = log.path().to_path_buf();
        drop(log);
        assert!(read(&path).unwrap().faults.is_empty());
        let error = read_whole(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("interrupted"), "{error}");
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
