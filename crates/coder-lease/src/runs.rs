//! Local agent runs and the activity behind terminal issue claims.
//!
//! Reads `agent-run.py` records without changing them or controlling a terminal.
//! Times use Unix seconds, matching the run writer.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::Error;

/// A local run with its process and latest known activity.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RunStatus {
    pub id: String,
    pub kind: String,
    pub host: String,
    /// Harness session IDs, without the harness prefix.
    pub sessions: Vec<String>,
    pub root_pid: u32,
    /// Recorded process start time, in Unix seconds.
    pub root_start: f64,
    /// True only when the process start time matches and the run has not ended.
    pub alive: bool,
    pub created: u64,
    pub last_activity: u64,
}

/// What to do with a terminal claim after matching it to a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimAction {
    Report,
    Release,
}

impl RunStatus {
    /// Keep a live, progressing holder; release a dead or silent holder.
    #[must_use]
    pub fn claim_action(&self, now: u64, stall_seconds: u64) -> ClaimAction {
        if self.alive && now.saturating_sub(self.last_activity) < stall_seconds {
            ClaimAction::Report
        } else {
            ClaimAction::Release
        }
    }
}

#[derive(Deserialize)]
struct Record {
    id: String,
    kind: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    sessions: Vec<String>,
    root_pid: u32,
    #[serde(default)]
    root_start: f64,
    created: u64,
    last_activity: Option<u64>,
    ended: Option<f64>,
    transcript_path: Option<PathBuf>,
}

/// Runs on this computer under the current user's home.
///
/// # Errors
/// The home is unknown, or the records cannot be read.
pub fn here() -> Result<Vec<RunStatus>, Error> {
    let home = user_home::home_os().ok_or_else(|| Error::Invalid(user_home::NO_HOME.into()))?;
    list(Path::new(&home))
}

/// Reads runs under `home/.openagents/runs` and checks each root process.
/// An older record falls back to its harness transcript's modification time,
/// then its creation time. Coder's activity file also counts as activity.
/// A missing runs directory means no recorded runs, not a dead remote run.
///
/// # Errors
/// A directory, run record, or process cannot be checked. A failed check must
/// not make its claim appear free.
pub fn list(home: &Path) -> Result<Vec<RunStatus>, Error> {
    read_with(home, &process_start)
}

fn read_with(
    home: &Path,
    started: &dyn Fn(u32) -> Result<Option<f64>, Error>,
) -> Result<Vec<RunStatus>, Error> {
    crate::root::refuse_real_home(home);
    let directory = home.join(".openagents/runs");
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path().join("run.json");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let record: Record = serde_json::from_slice(&bytes)
            .map_err(|e| Error::Corrupt(format!("{}: {e}", path.display())))?;
        records.push(record);
    }
    // Read only project directory names; never load transcript contents.
    let mut projects = Vec::new();
    match std::fs::read_dir(home.join(".claude/projects")) {
        Ok(entries) => {
            for entry in entries {
                let path = entry?.path();
                if std::fs::metadata(&path)?.is_dir() {
                    projects.push(path);
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let mut runs = Vec::new();
    for record in records {
        let alive = record.root_pid > 1
            && record.root_start > 0.0
            && record.ended.unwrap_or(0.0) == 0.0
            && started(record.root_pid)?.is_some_and(|actual| actual == record.root_start);
        let mut last_activity = record.last_activity.unwrap_or(0).max(record.created);
        if let Some(path) = &record.transcript_path {
            last_activity = last_activity.max(modified(path)?);
        }
        for session in &record.sessions {
            // Session IDs are file names, never paths supplied by a caller.
            if !safe_session(session) {
                continue;
            }
            for project in &projects {
                last_activity =
                    last_activity.max(modified(&project.join(format!("{session}.jsonl")))?);
            }
        }
        if record.kind == "coder" {
            last_activity = last_activity.max(modified(
                &home.join(format!(".openagents/activity/{}.json", record.root_pid)),
            )?);
        }
        runs.push(RunStatus {
            id: record.id,
            kind: record.kind,
            host: record.host,
            sessions: record.sessions,
            root_pid: record.root_pid,
            root_start: record.root_start,
            alive,
            created: record.created,
            last_activity,
        });
    }
    runs.sort_by(|a, b| b.last_activity.cmp(&a.last_activity).then(a.id.cmp(&b.id)));
    Ok(runs)
}

fn safe_session(session: &str) -> bool {
    !session.is_empty()
        && session
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}

fn modified(path: &Path) -> Result<u64, Error> {
    match std::fs::metadata(path) {
        Ok(meta) => Ok(meta
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e.into()),
    }
}

#[cfg(unix)]
fn process_start(pid: u32) -> Result<Option<f64>, Error> {
    let output = std::process::Command::new("ps")
        .env("LC_ALL", "C")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()?;
    process_output(pid, output)
}

#[cfg(unix)]
fn process_output(pid: u32, output: std::process::Output) -> Result<Option<f64>, Error> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.code() == Some(1) && stdout.trim().is_empty() && output.stderr.is_empty() {
        return Ok(None);
    }
    if !output.status.success() {
        return Err(Error::Invalid(format!(
            "couldn't check process {pid}: ps returned {}",
            output.status
        )));
    }
    parse_start(stdout.trim())
        .map(Some)
        .ok_or_else(|| Error::Invalid(format!("couldn't read the start time of process {pid}")))
}

#[cfg(unix)]
fn parse_start(text: &str) -> Option<f64> {
    let text = std::ffi::CString::new(text).ok()?;
    // SAFETY: tm is initialized, and strptime reads two nul-terminated strings
    // and writes only to tm. mktime accepts the same initialized structure.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_isdst = -1;
        let end = libc::strptime(text.as_ptr(), c"%a %b %e %H:%M:%S %Y".as_ptr(), &mut tm);
        if end.is_null() || *end != 0 {
            return None;
        }
        let epoch = libc::mktime(&mut tm);
        (epoch > 0).then_some(epoch as f64)
    }
}

#[cfg(not(unix))]
fn process_start(_: u32) -> Result<Option<f64>, Error> {
    Err(Error::Invalid(
        "terminal process checks are unavailable on this computer".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(home: &Path, id: &str, pid: u32, last: u64) -> PathBuf {
        let dir = home.join(".openagents/runs").join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({
                "id":id,"kind":"claude","host":"fixture","root_pid":pid,
                "root_start":1000.0,"created":1000,"last_activity":last,
                "sessions":[format!("session-{id}")],"ended":null
            }))
            .unwrap(),
        )
        .unwrap();
        path
    }

    #[test]
    fn progressing_silent_and_dead_runs_have_distinct_claim_actions() {
        let home = tempfile::tempdir().unwrap();
        record(home.path(), "progressing", 42, 2900);
        record(home.path(), "silent", 43, 1800);
        record(home.path(), "dead", 44, 2999);
        let runs = read_with(home.path(), &|pid| Ok((pid != 44).then_some(1000.0))).unwrap();
        let run = |id| runs.iter().find(|run| run.id == id).unwrap();
        assert_eq!(
            run("progressing").claim_action(3000, 1200),
            ClaimAction::Report
        );
        assert_eq!(run("silent").claim_action(3000, 1200), ClaimAction::Release);
        assert_eq!(run("dead").claim_action(3000, 1200), ClaimAction::Release);
        assert!(!run("dead").alive);
        assert_eq!(run("progressing").sessions, ["session-progressing"]);
    }

    #[test]
    fn a_reused_pid_or_ended_run_is_dead_even_with_recent_activity() {
        let home = tempfile::tempdir().unwrap();
        record(home.path(), "reused", 42, 2900);
        let path = record(home.path(), "ended", 43, 2900);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["ended"] = json!(2950);
        std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        let runs = read_with(home.path(), &|pid| {
            Ok(Some(if pid == 42 { 2000.0 } else { 1000.0 }))
        })
        .unwrap();
        assert!(
            runs.iter()
                .all(|run| !run.alive && run.claim_action(3000, 1200) == ClaimAction::Release)
        );
    }

    #[test]
    fn old_records_use_session_transcript_and_coder_activity_times() {
        let home = tempfile::tempdir().unwrap();
        let path = record(home.path(), "old", 42, 1000);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value.as_object_mut().unwrap().remove("last_activity");
        std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        let transcript = home
            .path()
            .join(".claude/projects/project/session-old.jsonl");
        std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        std::fs::write(&transcript, "").unwrap();
        let file = std::fs::File::options()
            .write(true)
            .open(&transcript)
            .unwrap();
        file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(2900))
            .unwrap();
        let path = record(home.path(), "coder", 43, 1000);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["kind"] = json!("coder");
        std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        let activity = home.path().join(".openagents/activity/43.json");
        std::fs::create_dir_all(activity.parent().unwrap()).unwrap();
        std::fs::write(&activity, "{}").unwrap();
        std::fs::File::options()
            .write(true)
            .open(activity)
            .unwrap()
            .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(2950))
            .unwrap();
        let runs = read_with(home.path(), &|_| Ok(Some(1000.0))).unwrap();
        assert_eq!(runs[0].last_activity, 2950);
        assert_eq!(runs[1].last_activity, 2900);
        assert!(
            runs.iter()
                .all(|run| run.claim_action(3000, 1200) == ClaimAction::Report)
        );
    }

    #[test]
    fn missing_runs_are_empty_but_unreadable_records_are_errors() {
        let home = tempfile::tempdir().unwrap();
        assert!(read_with(home.path(), &|_| Ok(None)).unwrap().is_empty());
        let path = record(home.path(), "corrupt", 42, 1000);
        std::fs::write(path, "{").unwrap();
        assert!(read_with(home.path(), &|_| Ok(None)).is_err());
    }

    #[test]
    fn a_failed_process_check_does_not_report_a_releasable_claim() {
        let home = tempfile::tempdir().unwrap();
        record(home.path(), "unreadable", 42, 2900);
        let result = read_with(home.path(), &|_| {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "ps unavailable").into())
        });
        assert!(matches!(result, Err(Error::Io(_))));
    }

    #[cfg(unix)]
    #[test]
    fn ps_start_time_round_trips() {
        assert!(process_start(std::process::id()).unwrap().is_some());
        assert_eq!(parse_start("not a process start"), None);
    }

    #[cfg(unix)]
    #[test]
    fn absent_processes_are_distinct_from_failed_or_unreadable_checks() {
        use std::os::unix::process::ExitStatusExt;

        let output = |code, stdout: &[u8], stderr: &[u8]| std::process::Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        };
        assert_eq!(process_output(42, output(1, b"", b"")).unwrap(), None);
        for result in [
            output(0, b"not a process start", b""),
            output(1, b"", b"ps failed"),
            output(2, b"", b""),
        ] {
            assert!(process_output(42, result).is_err());
        }
    }
}
