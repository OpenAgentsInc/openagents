//! What a running `coder` has in flight, written where another program can
//! read it, and the `coder activity` command that reads it back.
//!
//! A window's close key needs to know whether closing the window ends live
//! work. Nothing outside the process could tell before this module: a
//! window with a turn streaming closed the same way as an idle one. The
//! close script, `os/bin/coder-close`, asks `coder activity` first.
//!
//! # The record
//!
//! Each process that has work in flight keeps one file,
//! `~/.openagents/activity/<pid>.json`, and removes it when the work ends
//! and when the process exits. `CODER_ACTIVITY_DIR` moves the directory.
//! Two kinds of work count:
//!
//! - A turn: [`crate::turn::run`] holds a [`turn`] mark for as long as it
//!   runs. The terminal and `--print` both call that function, so both
//!   write the record.
//! - A delegation that has not reported: [`crate::delegate::Delegator::run`]
//!   and the relay door's fan-out hold a [`delegation`] mark per task.
//!
//! The file is written only when a count changes, through a temporary file
//! and a rename, with `0600` permissions in a `0700` directory. Nothing is
//! written until the binary calls [`enable`], so a test or another program
//! that links this crate never writes a record.
//!
//! # The reader
//!
//! `coder activity` reads the records for the processes named with
//! `--pid`, or every record, and adds them together. A record whose
//! process is no longer running is ignored and removed: a process that was
//! killed cannot remove its own. A record that cannot be read counts as
//! nothing running, because the close key treats "nothing running" as
//! "close the window", and a window that cannot be closed is worse than
//! one closed without a question.
//!
//! The command exits `0` while work is in flight and `1` when nothing is,
//! so a script reads the status and a person reads the sentence.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// The variable that moves the records' directory.
pub const DIR_ENV: &str = "CODER_ACTIVITY_DIR";

/// The schema word every record carries.
pub const SCHEMA: &str = "openagents.coder.activity.v1";

/// The exit code for a usage error, matching the rest of `coder`.
const EXIT_USAGE: u8 = 64;

const USAGE: &str = "\
Usage:
  coder activity [--pid PID]... [--json]

Says whether a running coder has work in flight: a turn that is streaming,
or a delegation that has not reported. With --pid, only those processes are
counted; repeat --pid for each one. Without it, every running coder is
counted. --json prints one JSON object instead of a sentence.

Exit codes: 0 work is running, 1 nothing is running, 64 invalid usage.";

/// What one or more processes have in flight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    /// Turns that are running. A terminal runs one at a time, so this is
    /// `0` or `1` for one process.
    pub turns: usize,
    /// Delegations that have not reported.
    pub delegations: usize,
}

impl Activity {
    /// Whether anything is running.
    #[must_use]
    pub fn active(&self) -> bool {
        self.turns > 0 || self.delegations > 0
    }

    /// What is running, as one sentence.
    #[must_use]
    pub fn sentence(&self) -> String {
        let turns = match self.turns {
            0 => None,
            1 => Some("A turn is streaming".to_string()),
            many => Some(format!("{many} turns are streaming")),
        };
        let delegations = match self.delegations {
            0 => None,
            1 => Some("1 delegation has not reported".to_string()),
            many => Some(format!("{many} delegations have not reported")),
        };
        match (turns, delegations) {
            (Some(turns), Some(delegations)) => format!("{turns}, and {delegations}."),
            (Some(turns), None) => format!("{turns}."),
            (None, Some(delegations)) => format!("{delegations}."),
            (None, None) => "Nothing is running.".to_string(),
        }
    }

    fn add(&mut self, other: Activity) {
        self.turns = self.turns.saturating_add(other.turns);
        self.delegations = self.delegations.saturating_add(other.delegations);
    }
}

/// One process's record on disk.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Record {
    schema: String,
    /// The process that wrote it. It is also the file's name, and a record
    /// whose name and field disagree is not read.
    pid: u32,
    turns: usize,
    delegations: usize,
}

/// Where the records live: [`DIR_ENV`], else `~/.openagents/activity`.
/// `None` when neither is set.
#[must_use]
pub fn directory() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os(DIR_ENV).filter(|named| !named.is_empty()) {
        return Some(PathBuf::from(named));
    }
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents").join("activity"))
}

/// Where one process's record lives.
#[must_use]
pub fn record_path(directory: &Path, pid: u32) -> PathBuf {
    directory.join(format!("{pid}.json"))
}

/// Whether a process is running. A process this user cannot signal is
/// still running.
#[cfg(unix)]
#[must_use]
pub fn alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // SAFETY: signal 0 sends nothing; it only checks that the process
    // exists and whether this user may signal it.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Whether a process is running. Without a way to ask, every record is
/// read.
#[cfg(not(unix))]
#[must_use]
pub fn alive(_pid: u32) -> bool {
    true
}

/// One process's record, or `None` when it has none, it cannot be read,
/// or its process is gone. A record left by a process that is gone is
/// removed.
fn one(directory: &Path, pid: u32) -> Option<Activity> {
    let path = record_path(directory, pid);
    let text = std::fs::read_to_string(&path).ok()?;
    if !alive(pid) {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    let record: Record = serde_json::from_str(&text).ok()?;
    if record.schema != SCHEMA || record.pid != pid {
        return None;
    }
    Some(Activity {
        turns: record.turns,
        delegations: record.delegations,
    })
}

/// What the processes in `pids` have in flight, added together, so a
/// window with two sessions in it is reported whole.
#[must_use]
pub fn read_in(directory: &Path, pids: &[u32]) -> Activity {
    let mut pids = pids.to_vec();
    pids.sort_unstable();
    pids.dedup();
    let mut total = Activity::default();
    for pid in pids {
        if let Some(one) = one(directory, pid) {
            total.add(one);
        }
    }
    total
}

/// The processes that have a record in `directory`, running or not.
#[must_use]
pub fn recorded(directory: &Path) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            name.strip_suffix(".json")?.parse::<u32>().ok()
        })
        .collect()
}

/// What every running process with a record has in flight.
#[must_use]
pub fn read_all(directory: &Path) -> Activity {
    read_in(directory, &recorded(directory))
}

/// Writes `pid`'s record for `activity`, or removes it when nothing is in
/// flight.
fn write(directory: &Path, pid: u32, activity: Activity) -> std::io::Result<()> {
    let path = record_path(directory, pid);
    if !activity.active() {
        return match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    private_directory(directory)?;
    let record = Record {
        schema: SCHEMA.to_string(),
        pid,
        turns: activity.turns,
        delegations: activity.delegations,
    };
    let text = serde_json::to_vec(&record).map_err(std::io::Error::other)?;
    let temporary = directory.join(format!(".{pid}.json.tmp"));
    let _ = std::fs::remove_file(&temporary);
    let mut file = private_create(&temporary)?;
    file.write_all(&text)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, &path)
}

#[cfg(unix)]
fn private_directory(directory: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)
}

#[cfg(not(unix))]
fn private_directory(directory: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(directory)
}

#[cfg(unix)]
fn private_create(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn private_create(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

/// One process's counts and where they are written.
///
/// A write that fails is not a reason to stop a turn: the reader counts a
/// missing record as nothing running, so the worst outcome is a window
/// closed without a question.
#[derive(Debug)]
pub struct Publisher {
    directory: PathBuf,
    pid: u32,
    now: Activity,
    closed: bool,
}

impl Publisher {
    /// A publisher that writes `pid`'s record into `directory`.
    #[must_use]
    pub fn at(directory: PathBuf, pid: u32) -> Self {
        Self {
            directory,
            pid,
            now: Activity::default(),
            closed: false,
        }
    }

    /// What this process has in flight.
    #[must_use]
    pub fn now(&self) -> Activity {
        self.now
    }

    /// Counts one more piece of work of `kind`, and writes the record.
    pub fn start(&mut self, kind: Kind) {
        match kind {
            Kind::Turn => self.now.turns = self.now.turns.saturating_add(1),
            Kind::Delegation => self.now.delegations = self.now.delegations.saturating_add(1),
        }
        self.flush();
    }

    /// Counts one piece of work of `kind` as finished, and writes the
    /// record, or removes it when nothing is left.
    pub fn finish(&mut self, kind: Kind) {
        match kind {
            Kind::Turn => self.now.turns = self.now.turns.saturating_sub(1),
            Kind::Delegation => self.now.delegations = self.now.delegations.saturating_sub(1),
        }
        self.flush();
    }

    /// Removes the record and writes nothing more. The process calls this
    /// on the way out, when work it abandoned may still hold marks.
    pub fn close(&mut self) {
        self.closed = true;
        let _ = write(&self.directory, self.pid, Activity::default());
    }

    fn flush(&self) {
        if !self.closed {
            let _ = write(&self.directory, self.pid, self.now);
        }
    }
}

/// A kind of work that counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A turn that is running.
    Turn,
    /// A delegation that has not reported.
    Delegation,
}

/// This process's publisher, once [`enable`] has made one.
static PUBLISHER: Mutex<Option<Publisher>> = Mutex::new(None);

fn with_publisher(act: impl FnOnce(&mut Publisher)) {
    let mut slot = match PUBLISHER.lock() {
        Ok(slot) => slot,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(publisher) = slot.as_mut() {
        act(publisher);
    }
}

/// Starts writing this process's record. The `coder` binary calls this
/// before a conversation and before a `--print` turn. Without a home
/// directory or [`DIR_ENV`], nothing is written.
pub fn enable() {
    let Some(directory) = directory() else {
        return;
    };
    let mut slot = match PUBLISHER.lock() {
        Ok(slot) => slot,
        Err(poisoned) => poisoned.into_inner(),
    };
    if slot.is_none() {
        *slot = Some(Publisher::at(directory, std::process::id()));
    }
}

/// Removes this process's record and stops writing it. The binary calls
/// this before it exits.
pub fn close() {
    with_publisher(Publisher::close);
}

/// A piece of work that counts until it is dropped.
#[derive(Debug)]
#[must_use = "the work counts only while the mark is held"]
pub struct Mark {
    kind: Kind,
}

impl Drop for Mark {
    fn drop(&mut self) {
        let kind = self.kind;
        with_publisher(|publisher| publisher.finish(kind));
    }
}

fn mark(kind: Kind) -> Mark {
    with_publisher(|publisher| publisher.start(kind));
    Mark { kind }
}

/// Counts a turn until the mark is dropped.
pub fn turn() -> Mark {
    mark(Kind::Turn)
}

/// Counts a delegation until the mark is dropped.
pub fn delegation() -> Mark {
    mark(Kind::Delegation)
}

/// What `coder activity` was asked.
#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    pids: Vec<u32>,
    json: bool,
    help: bool,
}

fn parse(arguments: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        let (flag, attached) = match argument.split_once('=') {
            Some((flag, value)) if flag.starts_with('-') => (flag, Some(value.to_string())),
            _ => (argument.as_str(), None),
        };
        match flag {
            "-h" | "--help" => options.help = true,
            "--json" => options.json = true,
            "--pid" => {
                let value = attached
                    .or_else(|| rest.next().cloned())
                    .ok_or("--pid needs a process ID")?;
                let pid = value
                    .parse::<u32>()
                    .ok()
                    .filter(|pid| *pid > 0)
                    .ok_or_else(|| format!("--pid needs a process ID, not {value}"))?;
                options.pids.push(pid);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(options)
}

/// What the command prints for `activity` under `--json`.
fn json(activity: Activity) -> serde_json::Value {
    serde_json::json!({
        "schema": SCHEMA,
        "active": activity.active(),
        "turns": activity.turns,
        "delegations": activity.delegations,
        "sentence": activity.sentence(),
    })
}

/// `coder activity`: what the named processes, or every running coder,
/// have in flight. `arguments` is everything after `activity`.
#[must_use]
pub fn cli(arguments: &[String]) -> u8 {
    let options = match parse(arguments) {
        Ok(options) => options,
        Err(why) => {
            eprintln!("coder activity: {why}\n\n{USAGE}");
            return EXIT_USAGE;
        }
    };
    if options.help {
        println!("{USAGE}");
        return 0;
    }
    let activity = match directory() {
        None => Activity::default(),
        Some(directory) if options.pids.is_empty() => read_all(&directory),
        Some(directory) => read_in(&directory, &options.pids),
    };
    if options.json {
        println!("{}", json(activity));
    } else {
        println!("{}", activity.sentence());
    }
    u8::from(!activity.active())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_string()).collect()
    }

    /// A process ID that is not running: a child that has been reaped.
    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[test]
    fn the_sentence_says_what_is_running_and_how_many() {
        let say = |turns, delegations| Activity { turns, delegations }.sentence();
        assert_eq!(say(0, 0), "Nothing is running.");
        assert_eq!(say(1, 0), "A turn is streaming.");
        assert_eq!(say(0, 1), "1 delegation has not reported.");
        assert_eq!(
            say(1, 2),
            "A turn is streaming, and 2 delegations have not reported."
        );
        assert_eq!(say(2, 0), "2 turns are streaming.");
    }

    #[test]
    fn a_publisher_writes_while_work_runs_and_removes_the_record_after() {
        let dir = tempfile::tempdir().unwrap();
        let pid = std::process::id();
        let mut publisher = Publisher::at(dir.path().to_path_buf(), pid);
        let path = record_path(dir.path(), pid);

        publisher.start(Kind::Turn);
        publisher.start(Kind::Delegation);
        publisher.start(Kind::Delegation);
        assert!(path.exists());
        assert_eq!(
            read_in(dir.path(), &[pid]),
            Activity {
                turns: 1,
                delegations: 2
            }
        );

        publisher.finish(Kind::Turn);
        publisher.finish(Kind::Delegation);
        assert_eq!(
            read_all(dir.path()).sentence(),
            "1 delegation has not reported."
        );

        publisher.finish(Kind::Delegation);
        assert!(!path.exists(), "an idle process has no record");
        assert!(!read_all(dir.path()).active());
    }

    #[cfg(unix)]
    #[test]
    fn the_record_is_private_and_leaves_no_temporary_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let records = dir.path().join("activity");
        let pid = std::process::id();
        let mut publisher = Publisher::at(records.clone(), pid);
        publisher.start(Kind::Turn);
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&records), 0o700);
        assert_eq!(mode(&record_path(&records, pid)), 0o600);
        let names: Vec<_> = std::fs::read_dir(&records)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from(format!("{pid}.json"))]);
    }

    #[test]
    fn a_closed_publisher_removes_its_record_and_writes_no_more() {
        let dir = tempfile::tempdir().unwrap();
        let pid = std::process::id();
        let mut publisher = Publisher::at(dir.path().to_path_buf(), pid);
        publisher.start(Kind::Turn);
        publisher.start(Kind::Delegation);
        publisher.close();
        assert!(!record_path(dir.path(), pid).exists());
        // Abandoned work that ends after the close leaves nothing behind.
        publisher.finish(Kind::Delegation);
        assert!(!record_path(dir.path(), pid).exists());
    }

    #[test]
    fn a_record_from_a_process_that_is_gone_is_ignored_and_swept() {
        let dir = tempfile::tempdir().unwrap();
        let pid = dead_pid();
        let busy = Activity {
            turns: 1,
            delegations: 0,
        };
        write(dir.path(), pid, busy).unwrap();
        assert!(record_path(dir.path(), pid).exists());

        assert_eq!(read_in(dir.path(), &[pid]), Activity::default());
        assert!(!record_path(dir.path(), pid).exists());
    }

    #[cfg(unix)]
    #[test]
    fn only_the_named_processes_are_counted() {
        let dir = tempfile::tempdir().unwrap();
        let me = std::process::id();
        write(
            dir.path(),
            me,
            Activity {
                turns: 1,
                delegations: 0,
            },
        )
        .unwrap();
        // Another running process, this one's parent, with nothing
        // recorded, answers nothing.
        let parent = std::os::unix::process::parent_id();
        assert!(!read_in(dir.path(), &[parent]).active());
        assert!(read_in(dir.path(), &[parent, me]).active());
        assert_eq!(
            read_in(dir.path(), &[me, me]).turns,
            1,
            "a process named twice counts once"
        );
    }

    #[test]
    fn an_unreadable_or_mismatched_record_counts_as_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let me = std::process::id();
        std::fs::write(record_path(dir.path(), me), "{not json").unwrap();
        assert!(!read_in(dir.path(), &[me]).active());
        let other = serde_json::json!({
            "schema": SCHEMA, "pid": me + 1, "turns": 1, "delegations": 0,
        });
        std::fs::write(record_path(dir.path(), me), other.to_string()).unwrap();
        assert!(!read_in(dir.path(), &[me]).active());
    }

    #[test]
    fn the_command_line_takes_repeated_pids_and_json() {
        assert_eq!(
            parse(&arguments(&["--pid", "12", "--pid=34", "--json"])).unwrap(),
            Options {
                pids: vec![12, 34],
                json: true,
                help: false,
            }
        );
        assert!(parse(&arguments(&[])).unwrap().pids.is_empty());
        assert!(parse(&arguments(&["--pid"])).is_err());
        assert!(parse(&arguments(&["--pid", "zero"])).is_err());
        assert!(parse(&arguments(&["--pid", "0"])).is_err());
        assert!(parse(&arguments(&["busy"])).is_err());
    }

    #[test]
    fn the_json_form_carries_the_counts_and_the_sentence() {
        let value = json(Activity {
            turns: 1,
            delegations: 2,
        });
        assert_eq!(value["active"], true);
        assert_eq!(value["delegations"], 2);
        assert_eq!(
            value["sentence"],
            "A turn is streaming, and 2 delegations have not reported."
        );
    }
}
