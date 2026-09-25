//! `evidence.baseline`: the host runs the task's own program before
//! session 1 (issue #9633, Microluna v18 change 2).
//!
//! In each v13 trial on `embedding-drift-monitor`, Luna's first two to four
//! turns went to finding the entry point, running it on the task's inputs,
//! and reading the numbers, and the first run's `RuntimeWarning` from
//! `normalize.py` pointed at a defect Jev's suspects didn't name. Code can
//! produce that output before the session starts.
//!
//! [`run`] finds the entry points with
//! [`crate::checks::contract::entry::find`], #9628's extractor extended
//! with modules, Makefile targets, and named scripts, and runs each once
//! in its own scratch copy of the untouched workspace:
//!
//! - bounded to [`WALL`] and [`STREAM_BYTES`] per stream by `supervise`;
//! - inside a `coder-boundary` writing boundary on the copy with the
//!   network denied, or refused when the host can't enforce one, except
//!   in a task container, where the container is the boundary as it is
//!   for the session's own commands;
//! - with the workspace's path in the command changed to the copy's, and
//!   the copy's path in the output changed back, so nothing is written to
//!   the real workspace and the session reads the paths it knows.
//!
//! A command that looks like it needs the network isn't run. The result is
//! a [`Baseline`]: the briefing section "Baseline behavior"
//! ([`Baseline::evidence`]), the host-executed command records the run
//! card reads ([`Baseline::records`], `docs/gym/run-card.md`), and the
//! commands later pieces rerun ([`Baseline::commands`]).

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub use crate::checks::contract::entry::{Entry, EntryKind};
use crate::record::Implementation;

/// The component's ID.
pub const COMPONENT: &str = "evidence.baseline";

/// The wall-time bound of one run.
pub const WALL: Duration = Duration::from_secs(60);

/// The bytes each of stdout and stderr keeps.
pub const STREAM_BYTES: usize = 16 * 1024;

/// The characters of each stream one run shows in the briefing.
pub const BRIEF_CHARS: usize = 2_000;

/// Lines past the briefing's head that name a warning or an error, shown
/// at most.
pub const MAX_NOTED: usize = 8;

/// The schema of each host-executed command record.
pub const EXECUTED_SCHEMA: &str = "openagents.coder-one.executed-command.v1";

/// The file the records go to, in the episode's `artifacts/` or a lean
/// group's directory under it.
pub const EXECUTED_FILE: &str = "executed-commands.jsonl";

/// The briefing section's label.
pub const LABEL: &str = "Baseline behavior";

/// What the briefing says before the runs.
pub const NOTE: &str = "Before this session, the host ran the task's own program once on an \
untouched copy of the workspace, each command bounded to 60 seconds with the network off. Nothing \
it did touched the workspace. Each command is exactly as run, from the working directory, so you \
can rerun it to compare.";

/// How one run was confined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confine {
    /// A `coder-boundary` writing boundary on the copy, network denied.
    Boundary,
    /// The task container, as for the session's own commands.
    Container,
}

/// Where and how the runs happen.
#[derive(Clone, Debug)]
pub struct Setup {
    /// The untouched workspace on this host.
    pub root: PathBuf,
    /// The directory the instruction calls the workspace, such as `/app`;
    /// the same as `root` outside an offline replay.
    pub alias: String,
    /// The bound of each run.
    pub wall: Duration,
    /// Run without a boundary when the host can't build one: true only in
    /// a task container.
    pub container: bool,
}

impl Setup {
    /// A setup for a workspace the instruction calls by its own path.
    #[must_use]
    pub fn local(root: &Path) -> Setup {
        Setup {
            root: root.to_path_buf(),
            alias: root.display().to_string(),
            wall: WALL,
            container: false,
        }
    }
}

/// One entry point, run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub kind: EntryKind,
    /// The command as it ran, from the working directory: the stated one,
    /// or `python3` for a stated `python` the host doesn't have.
    pub command: String,
    /// The command as the instruction states it, when it differs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stated: Option<String>,
    pub why: String,
    /// When it started, in milliseconds since the epoch.
    pub at: u64,
    pub exit: Option<i32>,
    pub timed_out: bool,
    /// The bound it ran under, in seconds.
    pub wall_sec: u64,
    pub ms: u64,
    pub stdout_head: String,
    pub stderr_head: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    /// It never ran, and why.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confine: Option<Confine>,
}

impl Run {
    /// Whether it ran to an exit the program itself chose: not stopped by
    /// the bound, not refused, and not a missing program or interpreter.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.failed.is_none() && !self.timed_out && self.exit.is_some_and(|e| e != 126 && e != 127)
    }

    /// The run as a host-executed command record.
    #[must_use]
    pub fn record(&self, cwd: &str) -> Value {
        json!({
            "schema": EXECUTED_SCHEMA,
            "at": self.at,
            "stage": "baseline",
            "session": null,
            "candidate": null,
            "kind": self.kind.word(),
            "command": self.command,
            "stated": self.stated,
            "cwd": cwd,
            "exit": self.exit,
            "timed_out": self.timed_out,
            "ms": self.ms,
            "stdout_digest": crate::accept::sha256(self.stdout_head.as_bytes()),
            "stderr_digest": crate::accept::sha256(self.stderr_head.as_bytes()),
            "stdout_head": self.stdout_head,
            "stderr_head": self.stderr_head,
            "stdout_bytes": self.stdout_bytes,
            "stderr_bytes": self.stderr_bytes,
            "failed": self.failed,
            "confine": self.confine,
            "requirements": [],
            "verdict": null,
            "rule": null,
        })
    }
}

/// What the baseline found and ran.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    /// The directory the commands ran from, as the instruction calls it.
    pub cwd: String,
    pub runs: Vec<Run>,
    /// Entry points code found and didn't run, with why.
    pub refused: Vec<Entry>,
    /// Whether the real workspace was the same after the runs, or `None`
    /// when nothing ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untouched: Option<bool>,
    /// Why nothing ran at all, when nothing did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub none: Option<String>,
    pub ms: u64,
}

impl Baseline {
    /// The commands later pieces rerun: each run that finished, as it ran,
    /// from the working directory. The finish rule (#9638) requires one of
    /// them after the last edit, and the post-session checks (#9636) rerun
    /// them on each candidate.
    #[must_use]
    pub fn commands(&self) -> Vec<String> {
        self.runs
            .iter()
            .filter(|r| r.finished())
            .map(|r| r.command.clone())
            .collect()
    }

    /// One host-executed command record per run, in the run card's shape.
    #[must_use]
    pub fn records(&self) -> Vec<Value> {
        self.runs.iter().map(|r| r.record(&self.cwd)).collect()
    }

    /// Appends the records to `file`, one JSON object a line.
    ///
    /// # Errors
    ///
    /// Returns a message when the file can't be written.
    pub fn append_records(&self, file: &Path) -> Result<(), String> {
        use std::io::Write;
        if self.runs.is_empty() {
            return Ok(());
        }
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
            .map_err(|e| format!("{}: {e}", file.display()))?;
        for record in self.records() {
            writeln!(out, "{record}").map_err(|e| format!("{}: {e}", file.display()))?;
        }
        Ok(())
    }

    /// The briefing section, when anything ran.
    #[must_use]
    pub fn evidence(&self) -> Option<microluna::Evidence> {
        if self.runs.is_empty() {
            return None;
        }
        let mut text = NOTE.to_string();
        for run in &self.runs {
            text.push_str("\n\n");
            text.push_str(&brief_run(run));
        }
        Some(microluna::Evidence {
            label: LABEL.to_string(),
            text,
        })
    }

    /// The record the lean loop keeps: everything but the stream heads,
    /// which the executed-command records carry.
    #[must_use]
    pub fn summary(&self) -> Value {
        json!({
            "kind": "lean.baseline",
            "cwd": self.cwd,
            "runs": self.runs.iter().map(|r| json!({
                "kind": r.kind.word(),
                "command": r.command,
                "stated": r.stated,
                "exit": r.exit,
                "timed_out": r.timed_out,
                "failed": r.failed,
                "ms": r.ms,
            })).collect::<Vec<_>>(),
            "refused": self.refused,
            "commands": self.commands(),
            "untouched": self.untouched,
            "none": self.none,
            "ms": self.ms,
        })
    }
}

/// The baseline commands recorded in an executed-commands file: each
/// `baseline` record that ran to its own exit, in order. For a later piece
/// that has the records and not the [`Baseline`].
#[must_use]
pub fn read_commands(file: &Path) -> Vec<String> {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|r| r["schema"] == EXECUTED_SCHEMA && r["stage"] == "baseline")
        .filter(|r| {
            !r["timed_out"].as_bool().unwrap_or(true)
                && r["failed"].is_null()
                && r["exit"].as_i64().is_some_and(|e| e != 126 && e != 127)
        })
        .filter_map(|r| r["command"].as_str().map(str::to_string))
        .collect()
}

/// The implementation record.
#[must_use]
pub fn implementation() -> Implementation {
    Implementation::new(
        COMPONENT,
        "entry points by code, each run once in a bounded scratch copy",
        &json!({
            "wall_sec": WALL.as_secs(),
            "stream_bytes": STREAM_BYTES,
            "brief_chars": BRIEF_CHARS,
            "max_entries": crate::checks::contract::entry::MAX_ENTRIES,
            "network": "off",
        }),
    )
}

fn head(text: &str, max: usize) -> (String, bool) {
    if text.chars().count() <= max {
        (text.to_string(), false)
    } else {
        (text.chars().take(max).collect(), true)
    }
}

/// Whether a line names a warning or an error.
fn notable(line: &str) -> bool {
    let lower = line.to_lowercase();
    ["warning", "error", "traceback", "exception", "failed"]
        .iter()
        .any(|w| lower.contains(w))
}

/// One run as the briefing shows it.
fn brief_run(run: &Run) -> String {
    let mut out = format!("$ {}\n", run.command);
    let status = if let Some(why) = &run.failed {
        format!("did not run: {why}")
    } else if run.timed_out {
        format!("stopped at the {} s bound before it finished", run.wall_sec)
    } else {
        match run.exit {
            Some(code) => format!("exit {code} after {:.1} s", run.ms as f64 / 1000.0),
            None => "ended by a signal".to_string(),
        }
    };
    out.push_str(&format!("({}: {status})", run.kind.word()));
    if let Some(stated) = &run.stated {
        out.push_str(&format!(
            "\nThe instruction states `{stated}`; this host has no `python`, so it ran as above."
        ));
    }
    for (name, text, bytes) in [
        ("stdout", &run.stdout_head, run.stdout_bytes),
        ("stderr", &run.stderr_head, run.stderr_bytes),
    ] {
        if text.trim().is_empty() {
            out.push_str(&format!("\n{name}: empty"));
            continue;
        }
        let (shown, cut) = head(text.trim_end(), BRIEF_CHARS);
        out.push_str(&format!(
            "\n{name}{}:\n{shown}",
            if cut || bytes > text.len() as u64 {
                format!(
                    " (first {} characters of {bytes} bytes)",
                    shown.chars().count()
                )
            } else {
                String::new()
            }
        ));
        if cut {
            let rest: String = text.chars().skip(BRIEF_CHARS).collect();
            let noted: Vec<&str> = rest
                .lines()
                .filter(|l| notable(l))
                .take(MAX_NOTED)
                .collect();
            if !noted.is_empty() {
                out.push_str(&format!(
                    "\nWarnings and errors later in {name}:\n{}",
                    noted.join("\n")
                ));
            }
        }
    }
    out
}

/// A fresh directory under the system's temporary directory.
fn scratch(name: &str) -> PathBuf {
    static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "{name}-{}-{}-{}",
        std::process::id(),
        atif::now_ms(),
        MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `text` with the copy's paths, as given and as the system resolves them,
/// changed to `to`.
fn unmap(text: &str, copy: &Path, to: &str) -> String {
    // The resolved path first: the given one may be its suffix, as
    // `/var/...` is of `/private/var/...` on macOS.
    let mut out = text.to_string();
    if let Ok(real) = copy.canonicalize() {
        out = crate::micro::lean::rebase_text(&out, &real.display().to_string(), to);
    }
    crate::micro::lean::rebase_text(&out, &copy.display().to_string(), to)
}

/// Runs `command` once in a fresh copy of the workspace.
async fn run_one(setup: &Setup, command: &str) -> Run {
    let at = atif::now_ms();
    let started = Instant::now();
    let mut run = Run {
        kind: EntryKind::Named,
        command: command.to_string(),
        stated: None,
        why: String::new(),
        at,
        exit: None,
        timed_out: false,
        wall_sec: setup.wall.as_secs(),
        ms: 0,
        stdout_head: String::new(),
        stderr_head: String::new(),
        stdout_bytes: 0,
        stderr_bytes: 0,
        failed: None,
        confine: None,
    };
    let copy = scratch("baseline-copy");
    let _cleanup = Cleanup(copy.clone());
    if let Err(error) = crate::handoff::copy_tree(&setup.root, &copy) {
        run.failed = Some(format!("the workspace couldn't be copied: {error}"));
        return run;
    }
    let root = setup.root.display().to_string();
    let copy_text = copy.display().to_string();
    let mut rebased = crate::micro::lean::rebase_text(command, &setup.alias, &copy_text);
    if root != setup.alias {
        rebased = crate::micro::lean::rebase_text(&rebased, &root, &copy_text);
    }
    let boundary = coder_boundary::Boundary::writing(&copy)
        .offline()
        .owned_scratch_under(std::env::temp_dir())
        .build();
    let ended = match boundary {
        Ok(boundary) => {
            let mut built = match boundary.command("/bin/sh", ["-c", rebased.as_str()]) {
                Ok(built) => built,
                Err(error) => {
                    run.failed = Some(error.to_string());
                    return run;
                }
            };
            built.current_dir(&copy);
            if let Some(scratch) = boundary.scratch() {
                built.env("TMPDIR", scratch);
            }
            microluna::tools::withhold_credentials(&mut built);
            run.confine = Some(Confine::Boundary);
            supervise::Job::from_command(built)
                .bounded(supervise::Limits::within(setup.wall).keeping(STREAM_BYTES))
                .run_holding(boundary.hold())
                .await
        }
        Err(_) if setup.container => {
            let mut built = std::process::Command::new("/bin/sh");
            built.args(["-c", rebased.as_str()]).current_dir(&copy);
            microluna::tools::withhold_credentials(&mut built);
            run.confine = Some(Confine::Container);
            supervise::Job::from_command(built)
                .bounded(supervise::Limits::within(setup.wall).keeping(STREAM_BYTES))
                .run()
                .await
        }
        Err(error) => {
            run.failed = Some(format!("no enforced boundary: {error}"));
            return run;
        }
    };
    match &ended.ending {
        supervise::Ending::Exited(code) => run.exit = *code,
        supervise::Ending::TimedOut => run.timed_out = true,
        supervise::Ending::Failed(why) => run.failed = Some(why.clone()),
    }
    run.stdout_head = unmap(&ended.stdout.text, &copy, &setup.alias);
    run.stderr_head = unmap(&ended.stderr.text, &copy, &setup.alias);
    run.stdout_bytes = ended.stdout.bytes;
    run.stderr_bytes = ended.stderr.bytes;
    run.ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    run
}

/// Runs one entry point, retrying once with `python3` when the command
/// says `python` and the host has no such program.
async fn run_entry(setup: &Setup, entry: &Entry) -> Run {
    let mut run = run_one(setup, &entry.command).await;
    if run.exit == Some(127)
        && let Some(rest) = entry.command.strip_prefix("python ")
    {
        let mut again = run_one(setup, &format!("python3 {rest}")).await;
        again.stated = Some(entry.command.clone());
        run = again;
    }
    run.kind = entry.kind;
    run.why.clone_from(&entry.why);
    run
}

/// Finds the task's entry points in the untouched workspace and runs each
/// once, all at the same time, each in its own copy.
pub async fn run(instruction: &str, setup: &Setup) -> Baseline {
    let started = Instant::now();
    let mut baseline = Baseline {
        cwd: setup.alias.clone(),
        ..Baseline::default()
    };
    let found = crate::checks::contract::entry::find(instruction, &setup.root, &setup.alias).await;
    let (refused, runnable): (Vec<Entry>, Vec<Entry>) =
        found.into_iter().partition(|e| e.refused.is_some());
    baseline.refused = refused;
    if runnable.is_empty() {
        baseline.none = Some(if baseline.refused.is_empty() {
            "no entry point: the instruction names no command or script, and the workspace has \
             no package with __main__.py and no Makefile test or check target"
                .to_string()
        } else {
            "every entry point found was refused".to_string()
        });
        baseline.ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        return baseline;
    }
    if !crate::micro::parallel::copyable(&setup.root) {
        baseline.none = Some("the workspace is too large to copy once per command".to_string());
        baseline.refused.extend(runnable.into_iter().map(|mut e| {
            e.refused = baseline.none.clone();
            e
        }));
        baseline.ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        return baseline;
    }
    let before = crate::micro::parallel::tree(&setup.root);
    baseline.runs =
        futures_util::future::join_all(runnable.iter().map(|e| run_entry(setup, e))).await;
    baseline.untouched = Some(crate::micro::parallel::tree(&setup.root) == before);
    baseline.ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    baseline
}
