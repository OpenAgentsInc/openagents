//! `verify.executed`: the task's own commands, rerun after every session.
//!
//! In every v13 session Luna spent 30 to 60 seconds of model time rerunning
//! the task's CLI, `compileall`, and scenario runs after its edits. The
//! host can run the same commands, by code, in the time it already spends
//! on the frozen score (issue #9636). After each session it copies the
//! workspace to a scratch directory and runs there:
//!
//! - every baseline command (`evidence.baseline`, issue #9633):
//!   [`crate::baseline::Baseline::commands`], with each command's
//!   untouched outcome read from its `stage: "baseline"` record in
//!   [`FILE`];
//! - every command the instruction names ([`named`]), found by the
//!   contract extractor with no model;
//! - a compile or import of the package ([`compile`]): `python3 -m
//!   compileall` and an import of each top-level package, `cargo check`,
//!   or `go build`.
//!
//! Each command is compared with its outcome on the untouched workspace
//! by one rule ([`verdict`]): a command that exited 0 there and doesn't
//! now has regressed; one that didn't exit 0 there either isn't a
//! regression; a timeout, or a command the host couldn't run, is unknown.
//! A candidate with any regressed command is rejected ([`rejects`]). Jev
//! isn't consulted: "ran before, crashes now" is a fact.
//!
//! Every run is one [`Record`] line of [`FILE`], in the shape
//! `docs/gym/run-card.md` documents and `gym runs characterize` reads.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::extract::{self, Pristine};
use super::host::{Contained, Host, Local, Ran};
use super::{Exit, Expect, Kind};

/// The schema of each record.
pub const SCHEMA: &str = crate::baseline::EXECUTED_SCHEMA;

/// The file the records go to: `evidence.baseline`'s, in the lean group's
/// directory under the episode's `artifacts/`.
pub const FILE: &str = crate::baseline::EXECUTED_FILE;

/// A command's wall-time bound when the manifest doesn't set one, in
/// seconds.
pub const COMMAND_SEC: u64 = 60;

/// The most bytes of each stream a record keeps.
pub const HEAD_BYTES: usize = 16 * 1024;

/// When a command ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Before session 1, on the untouched workspace.
    Baseline,
    /// After a session, on its candidate.
    AfterSession,
    /// During the probes.
    Probe,
}

/// What a command's outcome on a candidate says, against its outcome on
/// the untouched workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// It exited 0 on the candidate.
    Ok,
    /// It exited 0 on the untouched workspace and doesn't on the candidate.
    Regressed,
    /// It didn't exit 0 on the untouched workspace either.
    NotARegression,
    /// It timed out, the host couldn't run it, or there's nothing to
    /// compare it with.
    Unknown,
}

impl Verdict {
    /// The verdict as the records spell it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Regressed => "regressed",
            Verdict::NotARegression => "not_a_regression",
            Verdict::Unknown => "unknown",
        }
    }
}

/// One command the host runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Planned {
    /// `named`, `module`, `make`, `script`, `compile`, or `score`.
    pub kind: String,
    pub command: String,
    #[serde(default)]
    pub requirements: Vec<String>,
}

impl Planned {
    fn new(kind: &str, command: impl Into<String>) -> Planned {
        Planned {
            kind: kind.to_string(),
            command: command.into(),
            requirements: Vec::new(),
        }
    }
}

/// How one run ended, as the rule reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observed {
    /// The exit code, or `None` when it didn't finish.
    pub exit: Option<i32>,
    pub timed_out: bool,
}

impl Observed {
    /// What `ran` observed.
    #[must_use]
    pub fn of(ran: &Ran) -> Observed {
        Observed {
            exit: ran.exit,
            timed_out: ran.timed_out,
        }
    }

    /// It ran to exit 0.
    #[must_use]
    pub fn passed(self) -> bool {
        self.exit == Some(0) && !self.timed_out
    }

    fn describe(self) -> String {
        match (self.timed_out, self.exit) {
            (true, _) => "timed out".to_string(),
            (false, Some(code)) => format!("exited {code}"),
            (false, None) => "didn't finish".to_string(),
        }
    }
}

/// One line of [`FILE`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub schema: String,
    /// When the command started, in milliseconds since the epoch.
    pub at: u64,
    pub stage: Stage,
    /// The session whose candidate ran, after a session.
    pub session: Option<u32>,
    /// The candidate's retained directory under `artifacts/`, such as
    /// `lean-1/session-1`.
    pub candidate: Option<String>,
    pub kind: String,
    /// The command as planned, before it was moved to the scratch copy.
    pub command: String,
    /// The working directory as the task names it.
    pub cwd: String,
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub ms: u64,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub stdout_head: String,
    pub stderr_head: String,
    #[serde(default)]
    pub requirements: Vec<String>,
    pub verdict: Option<Verdict>,
    pub rule: Option<String>,
}

impl Record {
    /// The outcome the rule compares with.
    #[must_use]
    pub fn observed(&self) -> Observed {
        Observed {
            exit: self.exit,
            timed_out: self.timed_out,
        }
    }
}

/// The verdict on a candidate's run, against the command's outcome on the
/// untouched workspace, with the rule in words.
#[must_use]
pub fn verdict(untouched: Option<Observed>, now: &Ran, wall_sec: u64) -> (Verdict, String) {
    if let Some(why) = &now.failed {
        return (
            Verdict::Unknown,
            format!("the host couldn't run it on the candidate: {why}"),
        );
    }
    if now.timed_out {
        return (
            Verdict::Unknown,
            format!("timed out after {wall_sec} s on the candidate; a timeout isn't a regression"),
        );
    }
    let Some(before) = untouched else {
        return (
            Verdict::Unknown,
            "no outcome on the untouched workspace to compare with".to_string(),
        );
    };
    match (before.passed(), now.exit) {
        (true, Some(0)) => (
            Verdict::Ok,
            "exited 0 on the untouched workspace and on the candidate".to_string(),
        ),
        (true, Some(code)) => (
            Verdict::Regressed,
            format!("exited 0 on the untouched workspace and {code} on the candidate"),
        ),
        (true, None) => (
            Verdict::Regressed,
            "exited 0 on the untouched workspace; on the candidate a signal ended it".to_string(),
        ),
        (false, Some(0)) => (
            Verdict::Ok,
            format!(
                "exited 0 on the candidate; on the untouched workspace it {}",
                before.describe()
            ),
        ),
        (false, _) => (
            Verdict::NotARegression,
            format!(
                "it didn't exit 0 on the untouched workspace either: there it {}, here it {}",
                before.describe(),
                Observed::of(now).describe()
            ),
        ),
    }
}

/// Whether a candidate's records reject it: any command regressed.
#[must_use]
pub fn rejects(records: &[Record]) -> bool {
    records
        .iter()
        .any(|r| r.verdict == Some(Verdict::Regressed))
}

/// Words that begin a command that installs or fetches rather than runs
/// the task's code.
const INSTALLS: &[&str] = &[
    "pip install",
    "pip3 install",
    "python -m pip",
    "python3 -m pip",
    "uv pip",
    "uv add",
    "npm install",
    "npm i ",
    "npm ci",
    "yarn add",
    "yarn install",
    "pnpm add",
    "pnpm install",
    "bun install",
    "bun add",
    "go get",
    "go install",
    "cargo install",
    "cargo add",
    "gem install",
];

fn installs(command: &str) -> bool {
    let trimmed = command.trim_start();
    INSTALLS.iter().any(|i| trimmed.starts_with(i))
}

/// Every command the instruction names that the contract extractor can
/// run, found by code with no model: stated commands, example runs, and
/// exit-status runs, less those stated to fail and those that install
/// something.
#[must_use]
pub fn named(instruction: &str, workdir: &str, pristine: &Pristine) -> Vec<Planned> {
    let draft = extract::draft(instruction, workdir, pristine);
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for item in &draft.items {
        let Some(command) = &item.command else {
            continue;
        };
        if item.not_executable.is_some()
            || !matches!(item.kind, Kind::Command | Kind::Example | Kind::ExitCode)
            || matches!(
                item.expect,
                Some(Expect::Exit {
                    exit: Exit::NonZero
                })
            )
            || installs(command)
            || !seen.insert(command.clone())
        {
            continue;
        }
        out.push(Planned::new("named", command.clone()));
    }
    out
}

/// A compile or import of the workspace's package, chosen by code from its
/// files (paths relative to the workspace): `python3 -m compileall` over
/// the top-level packages and scripts and an import of each top-level
/// package, `cargo check` for a Cargo package, and `go build` for a Go
/// module.
#[must_use]
pub fn compile(files: &[String]) -> Vec<Planned> {
    let mut out = Vec::new();
    let has = |name: &str| files.iter().any(|f| f == name);
    let packages: BTreeSet<&str> = files
        .iter()
        .filter_map(|f| f.strip_suffix("/__init__.py"))
        .filter(|dir| !dir.contains('/') && is_module_name(dir))
        .collect();
    let scripts: BTreeSet<&str> = files
        .iter()
        .map(String::as_str)
        .filter(|f| !f.contains('/') && f.ends_with(".py"))
        .collect();
    if files.iter().any(|f| f.ends_with(".py")) {
        let targets: Vec<&str> = packages.iter().chain(scripts.iter()).copied().collect();
        let targets = if targets.is_empty() {
            ".".to_string()
        } else {
            targets
                .iter()
                .map(|t| crate::accept::runner::sh_quote(t))
                .collect::<Vec<_>>()
                .join(" ")
        };
        out.push(Planned::new(
            "compile",
            format!("python3 -m compileall -q {targets}"),
        ));
        for package in &packages {
            out.push(Planned::new(
                "compile",
                format!("python3 -c 'import {package}'"),
            ));
        }
    }
    if has("Cargo.toml") {
        out.push(Planned::new("compile", "cargo check --offline --quiet"));
    }
    if has("go.mod") {
        out.push(Planned::new("compile", "go build ./..."));
    }
    out
}

fn is_module_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The commands to run after each session, each once: the baseline
/// commands first (`evidence.baseline`'s
/// [`crate::baseline::Baseline::commands`], the runs that reached their
/// own exit), each with the kind its record gives, then the named ones,
/// then the compile ones.
#[must_use]
pub fn commands(
    baseline: &[String],
    records: &[Record],
    named: Vec<Planned>,
    compile: Vec<Planned>,
) -> Vec<Planned> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let from_baseline = baseline.iter().map(|command| {
        let record = records
            .iter()
            .rev()
            .find(|r| r.stage == Stage::Baseline && r.command == *command);
        Planned {
            kind: record.map_or_else(|| "named".to_string(), |r| r.kind.clone()),
            command: command.clone(),
            requirements: record.map(|r| r.requirements.clone()).unwrap_or_default(),
        }
    });
    for planned in from_baseline.chain(named).chain(compile) {
        if seen.insert(planned.command.clone()) {
            out.push(planned);
        }
    }
    out
}

/// Each command's last outcome in the baseline records.
#[must_use]
pub fn untouched(records: &[Record]) -> BTreeMap<String, Observed> {
    records
        .iter()
        .filter(|r| r.stage == Stage::Baseline)
        .map(|r| (r.command.clone(), r.observed()))
        .collect()
}

/// The records in `path` whose schema is [`SCHEMA`]; other lines are
/// skipped.
#[must_use]
pub fn read(path: &Path) -> Vec<Record> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|value| value["schema"] == SCHEMA)
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect()
}

/// Appends `records` to `path`, one JSON object a line.
///
/// # Errors
///
/// A message when the file can't be written.
pub fn append(path: &Path, records: &[Record]) -> Result<(), String> {
    if records.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let mut text = String::new();
    for record in records {
        text.push_str(&serde_json::to_string(record).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn head(text: &str) -> String {
    if text.len() <= HEAD_BYTES {
        return text.to_string();
    }
    let mut end = HEAD_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Where a record says it ran.
#[derive(Clone, Debug, Default)]
pub struct At {
    pub stage: Option<Stage>,
    pub session: Option<u32>,
    pub candidate: Option<String>,
    /// The working directory as the task names it.
    pub cwd: String,
}

/// One command's run, with when it started.
#[derive(Clone, Debug)]
pub struct Run {
    pub planned: Planned,
    pub ran: Ran,
    pub at: u64,
    pub wall_sec: u64,
}

/// The record of one run, with the verdict when there is one.
#[must_use]
pub fn record(at: &At, run: &Run, verdict: Option<(Verdict, String)>) -> Record {
    let (verdict, rule) = match verdict {
        Some((v, r)) => (Some(v), Some(r)),
        None => (None, run.ran.failed.clone()),
    };
    let mut stderr = run.ran.stderr.clone();
    if let Some(why) = &run.ran.failed
        && stderr.is_empty()
    {
        stderr = format!("the host couldn't run it: {why}");
    }
    Record {
        schema: SCHEMA.to_string(),
        at: run.at,
        stage: at.stage.unwrap_or(Stage::AfterSession),
        session: at.session,
        candidate: at.candidate.clone(),
        kind: run.planned.kind.clone(),
        command: run.planned.command.clone(),
        cwd: at.cwd.clone(),
        exit: run.ran.exit,
        timed_out: run.ran.timed_out,
        ms: run.ran.milliseconds,
        stdout_digest: crate::accept::sha256(run.ran.stdout.as_bytes()),
        stderr_digest: crate::accept::sha256(run.ran.stderr.as_bytes()),
        stdout_head: head(&run.ran.stdout),
        stderr_head: head(&stderr),
        requirements: run.planned.requirements.clone(),
        verdict,
        rule,
    }
}

/// The after-session records of `runs`, each judged against its untouched
/// outcome.
#[must_use]
pub fn judge(at: &At, runs: &[Run], untouched: &BTreeMap<String, Observed>) -> Vec<Record> {
    runs.iter()
        .map(|run| {
            let judged = verdict(
                untouched.get(&run.planned.command).copied(),
                &run.ran,
                run.wall_sec,
            );
            record(at, run, Some(judged))
        })
        .collect()
}

/// Runs each command on `host`, one after another, each bounded by `wall`
/// and all by `budget`. `rebase` moves a command to where the host runs
/// it. A command left with no time isn't run and says so.
pub async fn run_each(
    host: &impl Host,
    commands: &[Planned],
    wall: Duration,
    budget: Duration,
    rebase: &dyn Fn(&str) -> String,
) -> Vec<Run> {
    let started = Instant::now();
    let mut out = Vec::new();
    for planned in commands {
        let left = budget.saturating_sub(started.elapsed());
        let bound = wall.min(left);
        let at = now_ms();
        let ran = if bound < Duration::from_secs(1) {
            Ran {
                failed: Some("no time was left to run it".to_string()),
                ..Ran::default()
            }
        } else {
            super::run_command(host, &rebase(&planned.command), bound).await
        };
        out.push(Run {
            planned: planned.clone(),
            ran,
            at,
            wall_sec: bound.as_secs(),
        });
    }
    out
}

/// Where the host runs the commands: a scratch copy of a workspace.
#[derive(Clone, Debug)]
pub struct Place {
    /// The workspace as the task names it; commands that name it are moved
    /// to the copy.
    pub workdir: PathBuf,
    /// Inside the task's own container, where no writing boundary is
    /// built ([`Contained`]); otherwise each command runs inside one on
    /// the copy ([`Local`]).
    pub contained: bool,
    pub wall: Duration,
    pub budget: Duration,
}

fn scratch_dir() -> PathBuf {
    static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "verify-executed-{}-{}-{}",
        std::process::id(),
        now_ms(),
        MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

/// Copies `source` to a scratch directory, runs `commands` there, and
/// removes the copy.
///
/// # Errors
///
/// A message when the copy can't be made.
pub async fn in_copy(
    source: &Path,
    place: &Place,
    commands: &[Planned],
) -> Result<Vec<Run>, String> {
    let copy = scratch_dir();
    crate::handoff::copy_tree(source, &copy)?;
    let (from, to) = (
        place.workdir.display().to_string(),
        copy.display().to_string(),
    );
    let rebase = |command: &str| crate::micro::lean::rebase_text(command, &from, &to);
    let runs = if place.contained {
        let host = Contained {
            workdir: copy.clone(),
        };
        run_each(&host, commands, place.wall, place.budget, &rebase).await
    } else {
        let host = Local {
            workdir: copy.clone(),
        };
        run_each(&host, commands, place.wall, place.budget, &rebase).await
    };
    let _ = std::fs::remove_dir_all(&copy);
    Ok(runs)
}

/// What the host runs after every session, with each command's outcome
/// on the untouched workspace.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Baseline {
    pub commands: Vec<Planned>,
    pub untouched: BTreeMap<String, Observed>,
}

/// Plans the commands for the workspace at `place.workdir`, whose
/// untouched copy is `pristine_copy`: `baseline`, the commands
/// `evidence.baseline` ran to their own exit, whose outcomes are in
/// `known`, its records; then the named and compile commands. Runs on a
/// scratch copy of the untouched workspace each one that has no baseline
/// record. Returns the plan and the new baseline records.
pub async fn prepare(
    instruction: &str,
    pristine_copy: &Path,
    place: &Place,
    baseline: &[String],
    known: &[Record],
) -> (Baseline, Vec<Record>) {
    let workdir = place.workdir.display().to_string();
    let reader = Local {
        workdir: place.workdir.clone(),
    };
    let pristine = extract::gather(&reader, instruction, &workdir).await;
    let files = crate::micro::parallel::workspace_files(pristine_copy);
    let commands = commands(
        baseline,
        known,
        named(instruction, &workdir, &pristine),
        compile(&files),
    );
    let mut untouched = untouched(known);
    let missing: Vec<Planned> = commands
        .iter()
        .filter(|p| !untouched.contains_key(&p.command))
        .cloned()
        .collect();
    let at = At {
        stage: Some(Stage::Baseline),
        session: None,
        candidate: None,
        cwd: workdir,
    };
    let mut records = Vec::new();
    if !missing.is_empty() {
        match in_copy(pristine_copy, place, &missing).await {
            Ok(runs) => {
                for run in &runs {
                    untouched.insert(run.planned.command.clone(), Observed::of(&run.ran));
                    records.push(record(&at, run, None));
                }
            }
            Err(why) => {
                for planned in &missing {
                    let run = Run {
                        planned: planned.clone(),
                        ran: Ran {
                            failed: Some(format!("no scratch copy: {why}")),
                            ..Ran::default()
                        },
                        at: now_ms(),
                        wall_sec: place.wall.as_secs(),
                    };
                    records.push(record(&at, &run, None));
                }
            }
        }
    }
    (
        Baseline {
            commands,
            untouched,
        },
        records,
    )
}

/// Runs the plan on a scratch copy of the candidate at `place.workdir` and
/// judges each command.
///
/// # Errors
///
/// A message when the copy can't be made.
pub async fn after_session(
    baseline: &Baseline,
    place: &Place,
    session: u32,
    candidate: Option<String>,
) -> Result<Vec<Record>, String> {
    let runs = in_copy(&place.workdir, place, &baseline.commands).await?;
    let at = At {
        stage: Some(Stage::AfterSession),
        session: Some(session),
        candidate,
        cwd: place.workdir.display().to_string(),
    };
    Ok(judge(&at, &runs, &baseline.untouched))
}

/// What the next session is told when the host rejected a candidate.
#[must_use]
pub fn note(session: u32, records: &[Record], kept: Option<u32>) -> String {
    let regressed: Vec<String> = records
        .iter()
        .filter(|r| r.verdict == Some(Verdict::Regressed))
        .take(3)
        .map(|r| {
            let tail: String = {
                let text = if r.stderr_head.trim().is_empty() {
                    r.stdout_head.trim()
                } else {
                    r.stderr_head.trim()
                };
                let chars: Vec<char> = text.chars().collect();
                chars[chars.len().saturating_sub(400)..].iter().collect()
            };
            format!(
                "`{}` {}{}",
                r.command,
                r.rule.as_deref().unwrap_or("regressed"),
                if tail.is_empty() {
                    String::new()
                } else {
                    format!(":\n{tail}")
                }
            )
        })
        .collect();
    format!(
        "The host rejected the workspace after session {session}: a command that ran on the \
         untouched workspace fails now. {} {}\n{}",
        match kept {
            Some(n) => format!("The host keeps session {n}'s workspace as the best so far."),
            None => "No earlier workspace is kept.".to_string(),
        },
        "Fix the regression without undoing the task's fix; each command must still run.",
        regressed.join("\n")
    )
}

#[cfg(test)]
#[path = "executed_tests.rs"]
mod tests;
