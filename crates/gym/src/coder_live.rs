//! Coder One attempts as they run: the current component, the executor's
//! latest events, the latest judgments, spend so far, and how stale the
//! view is.
//!
//! Two kinds of attempt are followed:
//!
//! - **Mini-task runs** under `~/.openagents/coder-one/minitasks/`. The
//!   episode log, `episode.atif.jsonl`, is written and synced line by line
//!   on the host, and `manifest.json` appears only when the run ends, so a
//!   run directory with a log and no manifest is in progress.
//! - **Terminal-Bench trials** under `~/.openagents/terminal-bench/jobs/`.
//!   The Harbor adapter copies the new lines of the container's episode
//!   log to `<trial>/agent/live/episode.atif.jsonl` at a fixed interval
//!   while the episode runs, with `live/status.json` saying where the copy
//!   stands (`tbench.live`, schema `openagents.tbench.live-tail.v1`). A
//!   trial whose tail is `following` and that has no `result.json` yet is
//!   in progress.
//!
//! Each look reads every log again from the start, so a reader can refresh
//! as often as it likes and never holds state between looks. Staleness is
//! the time since the last event the log holds, and for a trial also the
//! time since the host last polled the container: a quiet executor and a
//! stalled tail read differently.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::timeline::{self, ExecutorEvent, Timeline};

/// The schema of this module's JSON.
pub const SCHEMA: &str = "openagents.gym.coder-live.v1";

/// The live-tail status schema the Harbor adapter writes.
pub const TAIL_SCHEMA: &str = "openagents.tbench.live-tail.v1";

/// When an attempt with no new event is called stale.
pub const STALE_AFTER: Duration = Duration::from_secs(120);

/// How long an ended attempt stays in the live view.
pub const RECENT: Duration = Duration::from_secs(15 * 60);

/// How many executor events and judgments an attempt shows.
const LATEST: usize = 6;

/// Where to look, and how to judge what is found.
#[derive(Clone, Debug)]
pub struct Sources {
    pub minitasks: Option<PathBuf>,
    pub jobs: Option<PathBuf>,
    pub stale_after: Duration,
    pub recent: Duration,
}

impl Default for Sources {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .map(PathBuf::from);
        Sources {
            minitasks: home
                .as_ref()
                .map(|home| home.join(".openagents/coder-one/minitasks")),
            jobs: home.map(|home| home.join(".openagents/terminal-bench/jobs")),
            stale_after: STALE_AFTER,
            recent: RECENT,
        }
    }
}

/// Where an attempt stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// In progress, with an event within the stale window.
    Running,
    /// In progress by its files, with no event for longer than the window.
    Stale,
    /// Finished.
    Ended,
}

impl State {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            State::Running => "running",
            State::Stale => "STALE",
            State::Ended => "ended",
        }
    }
}

/// One attempt, as last read.
#[derive(Clone, Debug)]
pub struct Attempt {
    /// `mini-task` or `terminal-bench`.
    pub kind: &'static str,
    pub id: String,
    pub dir: PathBuf,
    pub log: PathBuf,
    pub state: State,
    /// Whether its files say it is still going: no manifest for a mini-task
    /// run, and a following tail with no `result.json` for a trial.
    pub in_progress: bool,
    /// The newest event the log holds, in milliseconds since the epoch.
    pub last_event_at: Option<u64>,
    /// When the log file last changed.
    pub modified_at: Option<u64>,
    /// The Harbor tail's status, for a trial.
    pub tail: Option<Value>,
    pub timeline: Result<Timeline, String>,
}

fn millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |elapsed| {
        u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
    })
}

fn modified(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .map(millis)
}

fn iso_ms(text: &str) -> Option<u64> {
    crate::terminal_bench::timestamp_ms(text).and_then(|ms| u64::try_from(ms).ok())
}

impl Attempt {
    /// The invocation the attempt is in: the latest one that started and
    /// hasn't ended.
    #[must_use]
    pub fn current(&self) -> Option<&timeline::Entry> {
        let timeline = self.timeline.as_ref().ok()?;
        timeline
            .entries
            .iter()
            .filter(|entry| entry.outcome == "unknown")
            .max_by_key(|entry| (entry.depth, entry.offset_ms))
    }

    /// Spend over the finished invocations so far; `None` when a cost is
    /// unknown.
    #[must_use]
    pub fn spend(&self) -> Option<f64> {
        let timeline = self.timeline.as_ref().ok()?;
        timeline
            .entries
            .iter()
            .rev()
            .find(|entry| entry.outcome != "unknown")
            .and_then(|entry| entry.accumulated_usd)
            .or(Some(0.0))
    }

    /// The latest executor events, oldest first.
    #[must_use]
    pub fn latest_events(&self) -> Vec<&ExecutorEvent> {
        let Ok(timeline) = &self.timeline else {
            return Vec::new();
        };
        let skip = timeline.events.len().saturating_sub(LATEST);
        timeline.events.iter().skip(skip).collect()
    }

    /// The latest finished judgments: invocations of Jev-backed, check,
    /// and monitor components, oldest first.
    #[must_use]
    pub fn judgments(&self) -> Vec<&timeline::Entry> {
        let Ok(timeline) = &self.timeline else {
            return Vec::new();
        };
        let judged: Vec<&timeline::Entry> = timeline
            .entries
            .iter()
            .filter(|entry| entry.outcome != "unknown")
            .filter(|entry| {
                let component = entry.component.as_str();
                component.starts_with("verify.")
                    || component.starts_with("monitor.")
                    || component.starts_with("task.")
                    || component == "evidence.select"
                    || component == "evidence.probes"
                    || component == "exec.control"
            })
            .collect();
        let skip = judged.len().saturating_sub(LATEST);
        judged.into_iter().skip(skip).collect()
    }

    /// Milliseconds since the last event, at `now`.
    #[must_use]
    pub fn age_ms(&self, now: u64) -> Option<u64> {
        self.last_event_at.map(|at| now.saturating_sub(at))
    }

    /// Milliseconds since the Harbor tail last polled, at `now`.
    #[must_use]
    pub fn poll_age_ms(&self, now: u64) -> Option<u64> {
        let polled = self.tail.as_ref()?.get("polled_at")?.as_str()?;
        iso_ms(polled).map(|at| now.saturating_sub(at))
    }
}

/// An attempt's files, before they are read.
struct Found {
    kind: &'static str,
    id: String,
    dir: PathBuf,
    log: PathBuf,
    in_progress: bool,
    tail: Option<Value>,
}

/// Reads one log into an attempt.
fn read(found: Found, now: u64, sources: &Sources) -> Attempt {
    let Found {
        kind,
        id,
        dir,
        log,
        in_progress,
        tail,
    } = found;
    let log = log.as_path();
    let recording = atif::log::read(log);
    let last_step = recording
        .as_ref()
        .ok()
        .and_then(|recording| recording.steps.iter().map(|step| step.at).max());
    let log_ended = recording.as_ref().is_ok_and(atif::log::Recording::ended);
    let modified_at = modified(log);
    let last_event_at = last_step.or(modified_at);
    let timeline = if recording.is_ok() {
        timeline::read_log(log)
    } else {
        Err(format!("{} has no session record yet", log.display()))
    };
    let state = if !in_progress || log_ended {
        State::Ended
    } else if last_event_at
        .is_some_and(|at| now.saturating_sub(at) > millis_of(sources.stale_after))
    {
        State::Stale
    } else {
        State::Running
    };
    Attempt {
        kind,
        id,
        dir,
        log: log.to_path_buf(),
        state,
        in_progress,
        last_event_at,
        modified_at,
        tail,
        timeline,
    }
}

fn millis_of(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// One Terminal-Bench trial directory's live tail, when the Harbor adapter
/// wrote one, whether the trial is still going or not.
#[must_use]
pub fn read_trial(trial: &Path, now: u64, sources: &Sources) -> Option<Attempt> {
    Some(read(found_trial(trial)?, now, sources))
}

/// One trial directory's live tail, before its log is read: the status
/// file alone says whether the trial is still going.
fn found_trial(trial: &Path) -> Option<Found> {
    let live = trial.join("agent/live");
    let text = std::fs::read_to_string(live.join("status.json")).ok()?;
    let status = serde_json::from_str::<Value>(&text).ok()?;
    if status.get("schema").and_then(Value::as_str) != Some(TAIL_SCHEMA) {
        return None;
    }
    let following = status.get("state").and_then(Value::as_str) == Some("following");
    let in_progress = following && !trial.join("result.json").is_file();
    let name = |path: &Path| {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    let id = format!(
        "{}/{}",
        trial.parent().map(name).unwrap_or_default(),
        name(trial)
    );
    Some(Found {
        kind: "terminal-bench",
        id,
        dir: trial.to_path_buf(),
        log: live.join("episode.atif.jsonl"),
        in_progress,
        tail: Some(status),
    })
}

/// Every attempt in progress, and every one that ended within the recent
/// window, running ones first and then newest first.
#[must_use]
pub fn discover(sources: &Sources, now: u64) -> Vec<Attempt> {
    let recent = millis_of(sources.recent);
    let mut attempts = Vec::new();
    if let Some(dir) = &sources.minitasks {
        for run in subdirs(dir) {
            let log = run.join("episode.atif.jsonl");
            if !log.is_file() {
                continue;
            }
            let in_progress = !run.join("manifest.json").is_file();
            if !in_progress && modified(&log).is_none_or(|at| now.saturating_sub(at) > recent) {
                continue;
            }
            let id = run
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            attempts.push(read(
                Found {
                    kind: "mini-task",
                    id,
                    dir: run.clone(),
                    log,
                    in_progress,
                    tail: None,
                },
                now,
                sources,
            ));
        }
    }
    if let Some(dir) = &sources.jobs {
        for job in subdirs(dir) {
            for trial in subdirs(&job) {
                let log = trial.join("agent/live/episode.atif.jsonl");
                let recently = modified(&log).is_some_and(|at| now.saturating_sub(at) <= recent);
                // The status file decides before the log is read: an
                // ended trial's whole log is parsed only while it is
                // recent.
                if let Some(found) = found_trial(&trial)
                    && (found.in_progress || recently)
                {
                    attempts.push(read(found, now, sources));
                }
            }
        }
    }
    attempts.sort_by(|a, b| {
        (a.state == State::Ended)
            .cmp(&(b.state == State::Ended))
            .then(b.last_event_at.cmp(&a.last_event_at))
    });
    attempts
}

/// The time now, in milliseconds since the epoch.
#[must_use]
pub fn now_ms() -> u64 {
    millis(SystemTime::now())
}

fn ago(ms: Option<u64>) -> String {
    ms.map_or("—".to_owned(), |ms| {
        if ms < 60_000 {
            format!("{:.1}s ago", ms as f64 / 1000.0)
        } else {
            format!("{}m{:02}s ago", ms / 60_000, (ms % 60_000) / 1000)
        }
    })
}

fn money(value: Option<f64>) -> String {
    value.map_or("—".to_owned(), |usd| format!("${usd:.4}"))
}

fn clip(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn label(entry: &timeline::Entry) -> String {
    format!(
        "{}{}",
        entry.component,
        entry
            .name
            .as_deref()
            .map_or(String::new(), |name| format!(" · {name}"))
    )
}

/// Every attempt as text rows, with each in-progress attempt's detail.
#[must_use]
pub fn lines(attempts: &[Attempt], sources: &Sources, now: u64) -> Vec<String> {
    let running = attempts.iter().filter(|a| a.state != State::Ended).count();
    let mut lines = vec![
        format!(
            "Live · {running} in progress, {} recently ended · read at {} · stale after {}s",
            attempts.len() - running,
            atif::document::iso(now),
            sources.stale_after.as_secs()
        ),
        format!(
            "  {:<14} {:<52} {:<8} {:<34} {:<10} last event",
            "kind", "attempt", "state", "current component", "spend"
        ),
    ];
    if attempts.is_empty() {
        lines.push(
            "  Nothing in progress. Start one with `coder-one minitask run ID --speed 1`, or a Harbor trial of a coder-one arm.".to_owned(),
        );
    }
    for attempt in attempts {
        lines.push(format!(
            "  {:<14} {:<52} {:<8} {:<34} {:<10} {}",
            attempt.kind,
            clip(&attempt.id, 52),
            attempt.state.word(),
            clip(&attempt.current().map_or("—".to_owned(), label), 34),
            money(attempt.spend()),
            ago(attempt.age_ms(now)),
        ));
    }
    for attempt in attempts.iter().filter(|a| a.state != State::Ended) {
        lines.push(String::new());
        lines.extend(detail(attempt, now));
    }
    lines
}

/// One attempt's detail rows.
#[must_use]
pub fn detail(attempt: &Attempt, now: u64) -> Vec<String> {
    let mut lines = vec![format!(
        "{} · {} · {} · last event {}",
        attempt.id,
        attempt.kind,
        attempt.state.word(),
        ago(attempt.age_ms(now))
    )];
    lines.push(format!("  log: {}", attempt.log.display()));
    if let Some(tail) = &attempt.tail {
        lines.push(format!(
            "  host log copy: {} · {} bytes read · {} polls · last poll {} · {} errors",
            tail["state"].as_str().unwrap_or("?"),
            tail["offset"].as_u64().unwrap_or(0),
            tail["polls"].as_u64().unwrap_or(0),
            ago(attempt.poll_age_ms(now)),
            tail["errors"].as_u64().unwrap_or(0),
        ));
    }
    match &attempt.timeline {
        Err(error) => lines.push(format!("  {error}")),
        Ok(timeline) => {
            if let Some(entry) = attempt.current() {
                let started = entry.started_at.unwrap_or(now);
                lines.push(format!(
                    "  current: {} · running {}",
                    label(entry),
                    ago(Some(now.saturating_sub(started))).trim_end_matches(" ago")
                ));
            }
            lines.push(format!(
                "  spend so far: {} over {} finished component calls",
                money(attempt.spend()),
                timeline
                    .entries
                    .iter()
                    .filter(|entry| entry.outcome != "unknown")
                    .count()
            ));
            let events = attempt.latest_events();
            lines.push(format!(
                "  executor: {} events, latest {}",
                timeline.events.len(),
                events.len()
            ));
            lines.extend(events.iter().map(|event| event.row()));
            let judgments = attempt.judgments();
            if !judgments.is_empty() {
                lines.push("  latest judgments:".to_owned());
            }
            for entry in judgments {
                let output = entry.output.to_string();
                lines.push(format!(
                    "    {:<40} {:<10} {}",
                    clip(&label(entry), 40),
                    entry.outcome,
                    if entry.output.is_null() {
                        String::new()
                    } else {
                        clip(&output, 80)
                    }
                ));
            }
        }
    }
    lines
}

/// The attempts as versioned JSON.
#[must_use]
pub fn to_json(attempts: &[Attempt], sources: &Sources, now: u64) -> Value {
    json!({
        "schema": SCHEMA,
        "read_at": atif::document::iso(now),
        "stale_after_sec": sources.stale_after.as_secs(),
        "attempts": attempts.iter().map(|attempt| json!({
            "kind": attempt.kind,
            "id": attempt.id,
            "dir": attempt.dir.display().to_string(),
            "log": attempt.log.display().to_string(),
            "state": attempt.state.word().to_lowercase(),
            "last_event_at": attempt.last_event_at.map(atif::document::iso),
            "last_event_age_ms": attempt.age_ms(now),
            "tail": attempt.tail,
            "tail_poll_age_ms": attempt.poll_age_ms(now),
            "current": attempt.current().map(|entry| json!({
                "id": entry.id, "component": entry.component, "name": entry.name,
                "started_at": entry.started_at.map(atif::document::iso),
            })),
            "spend_usd": attempt.spend(),
            "events": attempt.latest_events().iter().map(|event| event.to_json()).collect::<Vec<_>>(),
            "event_count": attempt.timeline.as_ref().map_or(0, |t| t.events.len()),
            "judgments": attempt.judgments().iter().map(|entry| json!({
                "id": entry.id, "component": entry.component, "name": entry.name,
                "outcome": entry.outcome, "output": entry.output,
            })).collect::<Vec<_>>(),
            "error": attempt.timeline.as_ref().err(),
        })).collect::<Vec<_>>(),
    })
}

const HELP: &str = "\
gym coder live [--follow] [--interval SECONDS] [--count N] [--json]
               [--minitasks-dir PATH] [--jobs-dir PATH]
               [--stale SECONDS] [--recent SECONDS]

Shows Coder One attempts in progress: mini-task runs whose log has no
manifest yet, and Terminal-Bench trials whose host tail is following. For
each: the current component, the executor's latest events, the latest
judgments, spend so far, and how long since the last event. An attempt with
no event for --stale seconds (120) is marked STALE; ended attempts stay
listed for --recent seconds (900).

--follow reads again every --interval seconds (2) until interrupted, or
--count times. With --json, each read is one line of JSON.";

/// `gym coder live …`.
///
/// # Errors
///
/// Returns a message for an unknown option.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let mut sources = Sources::default();
    let mut follow = false;
    let mut json_output = false;
    let mut interval = Duration::from_secs(2);
    let mut count: Option<u64> = None;
    let mut index = 0;
    let seconds = |value: &str, flag: &str| {
        value
            .parse::<f64>()
            .ok()
            .filter(|seconds| *seconds >= 0.0)
            .map(Duration::from_secs_f64)
            .ok_or_else(|| format!("{flag} needs a number of seconds"))
    };
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            "--json" => json_output = true,
            "--follow" => follow = true,
            "--interval" | "--count" | "--stale" | "--recent" | "--minitasks-dir"
            | "--jobs-dir" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{argument} needs a value"))?;
                match argument {
                    "--interval" => interval = seconds(value, argument)?,
                    "--stale" => sources.stale_after = seconds(value, argument)?,
                    "--recent" => sources.recent = seconds(value, argument)?,
                    "--count" => {
                        count = Some(
                            value
                                .parse()
                                .map_err(|_| "--count needs a whole number".to_owned())?,
                        );
                    }
                    "--minitasks-dir" => sources.minitasks = Some(value.into()),
                    _ => sources.jobs = Some(value.into()),
                }
                index += 1;
            }
            other => return Err(format!("unknown option {other}\n\n{HELP}")),
        }
        index += 1;
    }
    let reads = match (follow, count) {
        (false, _) => 1,
        (true, Some(count)) => count.max(1),
        (true, None) => u64::MAX,
    };
    for read in 0..reads {
        if read > 0 {
            std::thread::sleep(interval);
        }
        let now = now_ms();
        let attempts = discover(&sources, now);
        if json_output {
            let value = to_json(&attempts, &sources, now);
            if follow {
                serde_json::to_writer(&mut *out, &value).map_err(|error| error.to_string())?;
            } else {
                serde_json::to_writer_pretty(&mut *out, &value)
                    .map_err(|error| error.to_string())?;
            }
            writeln!(out).map_err(|error| error.to_string())?;
        } else {
            if read > 0 {
                writeln!(out, "\n{}", "─".repeat(72)).map_err(|error| error.to_string())?;
            }
            for line in lines(&attempts, &sources, now) {
                writeln!(out, "{line}").map_err(|error| error.to_string())?;
            }
        }
        out.flush().map_err(|error| error.to_string())?;
    }
    Ok(0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn step(at: u64, extensions: Value) -> atif::document::Step {
        serde_json::from_value(json!({
            "at": at, "source": "System", "message": "", "extensions": extensions
        }))
        .unwrap()
    }

    fn invocation(event: &str, id: &str, component: &str, at: u64) -> Value {
        json!({ "invocation": {
            "schema": timeline::INVOCATION_SCHEMA, "event": event, "id": id,
            "parent": if id == "inv-1" { Value::Null } else { json!("inv-1") },
            "component": component, "name": component, "at": at,
            "outcome": "completed", "milliseconds": 5,
            "cost": { "usd": 0.001, "provenance": "price_estimate" },
            "output": { "summary": { "answer": "ready" } },
        }})
    }

    fn executor(seq: u64, kind: Value) -> Value {
        json!({ "executor_event": {
            "adapter": "scripted", "session_id": "s-1", "generation": 1, "seq": seq,
            "revision": 0, "at_ms": seq, "event": kind,
        }})
    }

    /// A mini-task run directory whose episode is mid-session.
    pub(crate) fn running_run(root: &Path, name: &str, at: u64) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let session = atif::Session::opening(name, "none", "mini-task", "/w", "v");
        let mut log = atif::Log::create_at(&dir.join("episode.atif.jsonl"), &session).unwrap();
        log.append(&step(at, invocation("start", "inv-1", "episode", at)))
            .unwrap();
        log.append(&step(
            at + 1,
            invocation("start", "inv-2", "verify.close", at + 1),
        ))
        .unwrap();
        log.append(&step(
            at + 2,
            invocation("end", "inv-2", "verify.close", at + 2),
        ))
        .unwrap();
        log.append(&step(
            at + 3,
            invocation("start", "inv-3", "exec.session", at + 3),
        ))
        .unwrap();
        log.append(&step(
            at + 4,
            executor(
                1,
                json!({"seq": 1, "line": 1, "kind": "command_started", "command": "pytest"}),
            ),
        ))
        .unwrap();
        dir
    }

    #[test]
    fn a_run_without_a_manifest_is_in_progress_with_its_current_component() {
        let root = std::env::temp_dir().join(format!("gym-live-{}", std::process::id()));
        let now = now_ms();
        running_run(
            &root.join("minitasks"),
            "minitask-a-scripted-good-1",
            now - 1_000,
        );
        let old = running_run(
            &root.join("minitasks"),
            "minitask-b-scripted-good-2",
            now - 600_000,
        );
        let sources = Sources {
            minitasks: Some(root.join("minitasks")),
            jobs: Some(root.join("jobs")),
            stale_after: Duration::from_secs(120),
            recent: Duration::from_secs(900),
        };
        let attempts = discover(&sources, now);
        assert_eq!(attempts.len(), 2);
        let fresh = &attempts[0];
        assert_eq!(fresh.state, State::Running);
        assert_eq!(fresh.current().unwrap().component, "exec.session");
        assert_eq!(fresh.latest_events().len(), 1);
        assert_eq!(fresh.judgments()[0].component, "verify.close");
        assert!((fresh.spend().unwrap() - 0.001).abs() < 1e-9);
        assert!(fresh.age_ms(now).unwrap() < 5_000);
        assert_eq!(attempts[1].state, State::Stale);
        let text = lines(&attempts, &sources, now).join("\n");
        assert!(text.contains("2 in progress"), "{text}");
        assert!(text.contains("STALE"), "{text}");
        assert!(text.contains("▸ command_started"), "{text}");
        assert!(text.contains("current: exec.session"), "{text}");
        // A finished run leaves the view once it is older than the window.
        std::fs::write(old.join("manifest.json"), "{}").unwrap();
        let attempts = discover(&sources, now + 1_000_000);
        assert!(
            attempts
                .iter()
                .all(|a| a.id != "minitask-b-scripted-good-2")
        );
        let value = to_json(&discover(&sources, now), &sources, now);
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["attempts"][0]["current"]["component"], "exec.session");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_trial_is_followed_through_its_host_tail() {
        let root = std::env::temp_dir().join(format!("gym-live-trial-{}", std::process::id()));
        let now = now_ms();
        let trial = root.join("jobs/job-1/task__abc");
        let live = trial.join("agent/live");
        std::fs::create_dir_all(&live).unwrap();
        let run = running_run(&root.join("scratch"), "episode", now - 2_000);
        std::fs::copy(
            run.join("episode.atif.jsonl"),
            live.join("episode.atif.jsonl"),
        )
        .unwrap();
        let status = json!({
            "schema": TAIL_SCHEMA, "state": "following", "offset": 1234, "polls": 7, "errors": 0,
            "polled_at": atif::document::iso(now - 3_000),
        });
        std::fs::write(live.join("status.json"), status.to_string()).unwrap();
        let sources = Sources {
            minitasks: None,
            jobs: Some(root.join("jobs")),
            ..Sources::default()
        };
        let attempts = discover(&sources, now);
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].kind, "terminal-bench");
        assert_eq!(attempts[0].id, "job-1/task__abc");
        assert_eq!(attempts[0].state, State::Running);
        let poll = attempts[0].poll_age_ms(now).unwrap();
        assert!((2_000..10_000).contains(&poll), "{poll}");
        let text = lines(&attempts, &sources, now).join("\n");
        assert!(
            text.contains("host log copy: following · 1234 bytes read · 7 polls"),
            "{text}"
        );
        // Harbor's result ends it.
        std::fs::write(trial.join("result.json"), "{}").unwrap();
        assert_eq!(discover(&sources, now)[0].state, State::Ended);
        let mut out = Vec::new();
        command(
            &[
                "--jobs-dir".to_owned(),
                root.join("jobs").display().to_string(),
                "--minitasks-dir".to_owned(),
                root.join("none").display().to_string(),
                "--follow".to_owned(),
                "--count".to_owned(),
                "2".to_owned(),
                "--interval".to_owned(),
                "0".to_owned(),
                "--json".to_owned(),
            ],
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().count(), 2);
        let first: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(first["attempts"][0]["state"], "ended");
        let _ = std::fs::remove_dir_all(root);
    }
}
