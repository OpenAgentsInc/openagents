//! `checks.contract`: the task's own contract, run.
//!
//! Every check tried before this one was a model's opinion about the
//! candidate. This one is an execution. [`extract`] reads a task's
//! instruction, and the files the instruction points to, and turns what
//! they state into [`Item`]s by code: shell commands, example invocations,
//! paths the output must exist at, and stated expected results such as
//! exact output, file formats, column headers, and exit codes. [`run`]
//! runs each item in the candidate's workspace through a [`host::Host`]
//! and compares what it observed with what the task states, by code.
//!
//! Each item ends in one [`Outcome`]: `matched`, `differed` with a bounded
//! diff, `could_not_run` with the reason, or `not_executable` with the
//! reason the extractor couldn't make it a check.
//!
//! Jev's role is narrow. Where code finds a command but can't tell whether
//! the task states that it should succeed, or finds an example input and a
//! file that may hold its expected output, one Noul decides
//! ([`extract::questions`]). Jev never sees a candidate or its output, and
//! never judges one.
//!
//! The contract check isn't wired into any policy. Its measurement on
//! retained, graded candidates is in
//! `docs/terminal-bench/2026-09-25-executed-contract-checks.md`. Two pieces
//! reuse its extractor and runner: [`entry`], the entry points
//! `evidence.baseline` runs before session 1, and [`executed`], the
//! post-session rule `verify.executed` (issue #9636), measured in
//! `docs/terminal-bench/2026-09-25-verify-executed-offline.md`.

pub mod cli;
pub mod entry;
pub mod executed;
pub mod extract;
pub mod host;
pub mod literals;
pub mod offline;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use host::{Host, Ran, Stat};

/// The schema of a plan: the items one task states.
pub const PLAN_SCHEMA: &str = "openagents.coder-one.contract-plan.v1";

/// The schema of one candidate's report.
pub const REPORT_SCHEMA: &str = "openagents.coder-one.contract-report.v1";

/// The most characters a diff or an observation keeps.
pub const DIFF_CHARS: usize = 1200;

/// The most bytes read from one output file.
pub const READ_MAX: usize = 8 * 1024 * 1024;

/// The wall-time bound of an example with no stated bound, in seconds.
pub const EXAMPLE_SEC: u64 = 120;

/// The wall-time bound of a command with no stated bound, in seconds.
pub const COMMAND_SEC: u64 = 300;

/// What an item checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A path the output must exist at.
    Path,
    /// A stated file format: JSON and its shape, a table's header, one
    /// entry per line, or a binary format's signature.
    Format,
    /// Names a stated module must provide.
    Interface,
    /// An example invocation with a stated expected output.
    Example,
    /// A stated exit status for a stated condition.
    ExitCode,
    /// A shell command the task names, stated to succeed.
    Command,
}

impl Kind {
    /// Every kind, in the order a run checks them.
    pub const ALL: [Kind; 6] = [
        Kind::Path,
        Kind::Format,
        Kind::Interface,
        Kind::Example,
        Kind::ExitCode,
        Kind::Command,
    ];

    /// The kind as the records spell it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Kind::Path => "path",
            Kind::Format => "format",
            Kind::Interface => "interface",
            Kind::Example => "example",
            Kind::ExitCode => "exit_code",
            Kind::Command => "command",
        }
    }
}

/// A stated exit status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Exit {
    Zero,
    NonZero,
}

/// What the top level of a JSON output must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonTop {
    Array,
    Object,
}

/// What the task states the item's result must be.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "expect", rename_all = "snake_case")]
pub enum Expect {
    /// The command exits as stated.
    Exit { exit: Exit },
    /// The command's standard output is exactly the text of `reference`,
    /// a file of the untouched workspace, read when the plan was made.
    StdoutEquals { reference: String, text: String },
    /// The command's standard output has as many characters as
    /// `reference`, a file of the untouched workspace.
    StdoutLength { reference: String, chars: usize },
    /// A file or directory exists at the path.
    Exists,
    /// The file parses as JSON, with the stated top level and every key
    /// of the stated example (`shape`) where the example has it.
    Json {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top: Option<JsonTop>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shape: Option<Value>,
    },
    /// The file is a delimited table whose header holds the stated
    /// columns, in the stated order when `ordered`.
    Table {
        delimiter: char,
        columns: Vec<String>,
        ordered: bool,
    },
    /// One entry per line, with no blank line inside, sorted when stated.
    Lines { sorted: bool },
    /// The file starts with a binary format's signature.
    Signature { format: String },
}

/// One thing the task states, as a check or as the reason it isn't one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// `K1`, `K2`, …, stable for one plan.
    pub id: String,
    pub kind: Kind,
    /// `instruction`, or the path of the file the instruction points to.
    pub source: String,
    /// The sentence or line the item comes from, bounded.
    pub span: String,
    /// The command it runs, when it runs one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The file it reads, when it reads one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<Expect>,
    /// The command's wall-time bound in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_sec: Option<u64>,
    /// The bound is the task's stated one, so passing it is a difference,
    /// not a failure to run.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stated_bound: bool,
    /// Why the item isn't a check, when it isn't one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_executable: Option<String>,
    /// The Jev question that decided the item, with its answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<Value>,
}

/// The items one task states, frozen with their digest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub schema: String,
    pub task: String,
    pub workdir: String,
    /// The digest of the instruction the plan was made from.
    pub instruction: String,
    pub items: Vec<Item>,
    /// The Jev requests the plan asked, with their answers.
    #[serde(default)]
    pub jev: Vec<Value>,
    /// The digest of everything above.
    pub digest: String,
}

impl Plan {
    /// Seals `items` into a plan with its digest.
    #[must_use]
    pub fn seal(
        task: &str,
        workdir: &str,
        instruction: &str,
        items: Vec<Item>,
        jev: Vec<Value>,
    ) -> Plan {
        let mut plan = Plan {
            schema: PLAN_SCHEMA.to_string(),
            task: task.to_string(),
            workdir: workdir.to_string(),
            instruction: crate::accept::sha256(instruction.as_bytes()),
            items,
            jev,
            digest: String::new(),
        };
        plan.digest = atif::digest(&json!(plan));
        plan
    }

    /// Items that are checks.
    pub fn checks(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|i| i.not_executable.is_none())
    }
}

/// How one item ended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// Observed what the task states.
    Matched { observed: String },
    /// Observed something else. `similarity`, when there is one, is the
    /// share of characters that agree position by position.
    Differed {
        diff: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        similarity: Option<f64>,
    },
    /// The item couldn't run, so it says nothing about the candidate.
    CouldNotRun { why: String },
    /// The extractor couldn't make the item a check.
    NotExecutable { why: String },
}

impl Outcome {
    /// The outcome as the records spell it.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Outcome::Matched { .. } => "matched",
            Outcome::Differed { .. } => "differed",
            Outcome::CouldNotRun { .. } => "could_not_run",
            Outcome::NotExecutable { .. } => "not_executable",
        }
    }
}

/// One item's result on one candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checked {
    pub id: String,
    pub kind: Kind,
    #[serde(flatten)]
    pub outcome: Outcome,
    pub milliseconds: u64,
}

/// What a candidate's results say, by the frozen rule: fail when any item
/// differed, pass when at least one matched and none differed, and
/// nothing otherwise.
#[must_use]
pub fn call(results: &[Checked]) -> Option<&'static str> {
    let differed = results
        .iter()
        .any(|r| matches!(r.outcome, Outcome::Differed { .. }));
    let matched = results
        .iter()
        .any(|r| matches!(r.outcome, Outcome::Matched { .. }));
    if differed {
        Some("fail")
    } else if matched {
        Some("pass")
    } else {
        None
    }
}

/// `matched / (matched + differed)`, or `None` when nothing ran to an
/// answer.
#[must_use]
pub fn score(results: &[Checked]) -> Option<f64> {
    let matched = results
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Matched { .. }))
        .count();
    let differed = results
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Differed { .. }))
        .count();
    #[allow(clippy::cast_precision_loss)]
    (matched + differed > 0).then(|| matched as f64 / (matched + differed) as f64)
}

/// Counts of outcomes by kind.
#[must_use]
pub fn tally(results: &[Checked]) -> Value {
    let mut out: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for r in results {
        *out.entry(r.kind.word())
            .or_default()
            .entry(r.outcome.word())
            .or_default() += 1;
    }
    json!(out)
}

/// `text` cut to `max` characters, with a marker when it was cut.
#[must_use]
pub fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max).collect();
    format!("{kept}…")
}

/// The share of positions at which two texts agree, over the longer one.
#[must_use]
pub fn similarity(expected: &str, observed: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (expected.chars().collect(), observed.chars().collect());
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    let same = a.iter().zip(&b).filter(|(x, y)| x == y).count();
    #[allow(clippy::cast_precision_loss)]
    let share = same as f64 / longest as f64;
    share
}

/// A bounded, line-by-line account of how `observed` differs from
/// `expected`: the line counts, then up to three differing lines.
#[must_use]
pub fn diff(expected: &str, observed: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (expected.lines().collect(), observed.lines().collect());
    let mut out = format!(
        "expected {} lines and {} characters, observed {} lines and {} characters",
        a.len(),
        expected.chars().count(),
        b.len(),
        observed.chars().count()
    );
    let mut shown = 0;
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied(), b.get(i).copied());
        if x == y {
            continue;
        }
        out.push_str(&format!(
            "\nline {}: expected {:?}, observed {:?}",
            i + 1,
            clip(x.unwrap_or("<none>"), 160),
            clip(y.unwrap_or("<none>"), 160)
        ));
        shown += 1;
        if shown == 3 {
            break;
        }
    }
    clip(&out, DIFF_CHARS)
}

/// Judges one ran command against an exit or output expectation.
#[must_use]
pub fn judge_run(item: &Item, ran: &Ran) -> Outcome {
    if let Some(why) = &ran.failed {
        return Outcome::CouldNotRun {
            why: format!("the command didn't run: {why}"),
        };
    }
    let wall = item.wall_sec.unwrap_or(COMMAND_SEC);
    if ran.timed_out {
        return if item.stated_bound {
            Outcome::Differed {
                diff: format!("took longer than the stated {wall} s"),
                similarity: None,
            }
        } else {
            Outcome::CouldNotRun {
                why: format!("timed out after {wall} s"),
            }
        };
    }
    if matches!(ran.exit, Some(126 | 127)) || ran.exit.is_none() {
        return Outcome::CouldNotRun {
            why: format!(
                "exit {:?}: the command or its interpreter wasn't found or was stopped: {}",
                ran.exit,
                clip(ran.stderr.trim(), 300)
            ),
        };
    }
    let exit = ran.exit.unwrap_or(-1);
    let tail = || {
        let mut text = ran.stderr.trim().to_string();
        if text.is_empty() {
            text = ran.stdout.trim().to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        let start = chars.len().saturating_sub(600);
        chars[start..].iter().collect::<String>()
    };
    match &item.expect {
        Some(Expect::Exit { exit: Exit::Zero }) => {
            if exit == 0 {
                Outcome::Matched {
                    observed: "exit 0".to_string(),
                }
            } else {
                Outcome::Differed {
                    diff: clip(
                        &format!("expected exit 0, observed exit {exit}\n{}", tail()),
                        DIFF_CHARS,
                    ),
                    similarity: None,
                }
            }
        }
        Some(Expect::Exit {
            exit: Exit::NonZero,
        }) => {
            if exit == 0 {
                Outcome::Differed {
                    diff: "expected a non-zero exit, observed exit 0".to_string(),
                    similarity: None,
                }
            } else {
                Outcome::Matched {
                    observed: format!("exit {exit}"),
                }
            }
        }
        Some(Expect::StdoutEquals { text, .. }) => {
            if ran.truncated {
                return Outcome::CouldNotRun {
                    why: "the output was longer than the host keeps".to_string(),
                };
            }
            if ran.stdout == *text {
                Outcome::Matched {
                    observed: format!(
                        "exit {exit}; output identical, {} characters",
                        text.chars().count()
                    ),
                }
            } else {
                Outcome::Differed {
                    diff: format!("exit {exit}; {}", diff(text, &ran.stdout)),
                    similarity: Some(similarity(text, &ran.stdout)),
                }
            }
        }
        Some(Expect::StdoutLength { chars, .. }) => {
            if ran.truncated {
                return Outcome::CouldNotRun {
                    why: "the output was longer than the host keeps".to_string(),
                };
            }
            let observed = ran.stdout.chars().count();
            if observed == *chars {
                Outcome::Matched {
                    observed: format!("{observed} characters"),
                }
            } else {
                Outcome::Differed {
                    diff: format!("expected {chars} characters, observed {observed}"),
                    similarity: None,
                }
            }
        }
        other => Outcome::NotExecutable {
            why: format!("no run expectation: {other:?}"),
        },
    }
}

/// Every key path in `shape` that `value` lacks, up to eight. An array in
/// the example stands for arrays whose every element has the shape of its
/// first element; an object in the example where the output has an array
/// stands for each element of that array.
fn missing_keys(shape: &Value, value: &Value, at: &str, out: &mut Vec<String>) {
    if out.len() >= 8 {
        return;
    }
    match (shape, value) {
        (Value::Object(s), Value::Object(v)) => {
            for (key, inner) in s {
                let here = format!("{at}.{key}");
                match v.get(key) {
                    Some(found) => missing_keys(inner, found, &here, out),
                    None => out.push(format!("{here} is missing")),
                }
            }
        }
        (Value::Array(s), Value::Array(v)) => {
            if let Some(first) = s.first() {
                for (i, element) in v.iter().enumerate() {
                    missing_keys(first, element, &format!("{at}[{i}]"), out);
                    if out.len() >= 8 {
                        return;
                    }
                }
            }
        }
        (Value::Object(_), Value::Array(v)) if at.is_empty() => {
            for (i, element) in v.iter().enumerate() {
                missing_keys(shape, element, &format!("[{i}]"), out);
                if out.len() >= 8 {
                    return;
                }
            }
        }
        (Value::Object(_), other) => out.push(format!(
            "{} is {}, not an object",
            if at.is_empty() { "the top level" } else { at },
            kind_of(other)
        )),
        (Value::Array(_), other) => out.push(format!(
            "{} is {}, not an array",
            if at.is_empty() { "the top level" } else { at },
            kind_of(other)
        )),
        _ => {}
    }
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Splits one delimited line, dropping surrounding quotes and spaces.
fn cells(line: &str, delimiter: char) -> Vec<String> {
    line.trim_end_matches(['\r', '\n'])
        .split(delimiter)
        .map(|c| c.trim().trim_matches('"').trim().to_string())
        .collect()
}

/// Judges a file's bytes against a format expectation.
#[must_use]
pub fn judge_file(expect: &Expect, bytes: &[u8]) -> Outcome {
    let differed = |diff: String| Outcome::Differed {
        diff: clip(&diff, DIFF_CHARS),
        similarity: None,
    };
    match expect {
        Expect::Json { top, shape } => {
            let value: Value = match serde_json::from_slice(bytes) {
                Ok(value) => value,
                Err(error) => return differed(format!("not JSON: {error}")),
            };
            match (top, &value) {
                (Some(JsonTop::Array), Value::Array(_))
                | (Some(JsonTop::Object), Value::Object(_))
                | (None, _) => {}
                (Some(want), other) => {
                    return differed(format!(
                        "the top level is {}, not {}",
                        kind_of(other),
                        if *want == JsonTop::Array {
                            "an array"
                        } else {
                            "an object"
                        }
                    ));
                }
            }
            if let Some(shape) = shape {
                let mut missing = Vec::new();
                missing_keys(shape, &value, "", &mut missing);
                if !missing.is_empty() {
                    return differed(format!("not the stated shape: {}", missing.join("; ")));
                }
            }
            Outcome::Matched {
                observed: format!("JSON, {}", kind_of(&value)),
            }
        }
        Expect::Table {
            delimiter,
            columns,
            ordered,
        } => {
            let text = String::from_utf8_lossy(bytes);
            let text = text.trim_start_matches('\u{feff}');
            let Some(header) = text.lines().find(|l| !l.trim().is_empty()) else {
                return differed("the table is empty".to_string());
            };
            let found = cells(header, *delimiter);
            if columns.is_empty() {
                return if found.len() > 1 || text.lines().count() > 1 {
                    Outcome::Matched {
                        observed: format!("{} columns", found.len()),
                    }
                } else {
                    differed(format!("no {delimiter:?}-delimited header: {header:?}"))
                };
            }
            let ok = if *ordered {
                found == *columns
            } else {
                columns.iter().all(|c| found.contains(c))
            };
            if ok {
                Outcome::Matched {
                    observed: format!("header {}", clip(&found.join(","), 300)),
                }
            } else {
                differed(format!(
                    "expected header {}{}, observed {}",
                    columns.join(","),
                    if *ordered { " in that order" } else { "" },
                    found.join(",")
                ))
            }
        }
        Expect::Lines { sorted } => {
            let text = String::from_utf8_lossy(bytes);
            let lines: Vec<&str> = text.trim_end_matches(['\n', '\r']).lines().collect();
            if lines.is_empty() || lines.iter().all(|l| l.trim().is_empty()) {
                return differed("the file is empty".to_string());
            }
            if let Some(i) = lines.iter().position(|l| l.trim().is_empty()) {
                return differed(format!("line {} is blank", i + 1));
            }
            if *sorted {
                let trimmed: Vec<&str> = lines.iter().map(|l| l.trim()).collect();
                let mut want = trimmed.clone();
                want.sort_unstable();
                if want != trimmed {
                    let i = trimmed
                        .iter()
                        .zip(&want)
                        .position(|(a, b)| a != b)
                        .unwrap_or(0);
                    return differed(format!(
                        "not in order at line {}: {:?} before {:?}",
                        i + 1,
                        trimmed[i],
                        trimmed.get(i + 1).copied().unwrap_or_default()
                    ));
                }
            }
            Outcome::Matched {
                observed: format!("{} lines", lines.len()),
            }
        }
        Expect::Signature { format } => {
            let ok = match format.as_str() {
                "zip" => bytes.starts_with(b"PK\x03\x04"),
                "npy" => bytes.starts_with(b"\x93NUMPY"),
                "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
                "pdf" => bytes.starts_with(b"%PDF"),
                "safetensors" => safetensors_header(bytes).is_some(),
                "sqlite" => bytes.starts_with(b"SQLite format 3\0"),
                _ => false,
            };
            if ok {
                Outcome::Matched {
                    observed: format!("{format}, {} bytes", bytes.len()),
                }
            } else {
                differed(format!(
                    "not a {format} file: it starts with {:?}",
                    String::from_utf8_lossy(&bytes[..bytes.len().min(16)])
                ))
            }
        }
        other => Outcome::NotExecutable {
            why: format!("no file expectation: {other:?}"),
        },
    }
}

/// A safetensors file's JSON header: an eight-byte little-endian length,
/// then that many bytes of JSON.
fn safetensors_header(bytes: &[u8]) -> Option<Value> {
    let length = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    let end = 8usize.checked_add(usize::try_from(length).ok()?)?;
    serde_json::from_slice(bytes.get(8..end)?).ok()
}

fn millis(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Runs a command, retrying once with `python3` when the task wrote
/// `python` and the host has no such program.
pub(crate) async fn run_command(host: &impl Host, command: &str, wall: Duration) -> Ran {
    let ran = host.run(command, wall).await;
    if ran.exit == Some(127)
        && let Some(rest) = command.strip_prefix("python ")
    {
        return host.run(&format!("python3 {rest}"), wall).await;
    }
    ran
}

/// Runs `plan`'s items on `host`, reading files first, then running
/// commands, one command once however many items read its result.
pub async fn run(plan: &Plan, host: &impl Host) -> Vec<Checked> {
    let mut ran: BTreeMap<(String, u64), Ran> = BTreeMap::new();
    let mut out = Vec::new();
    for kind in Kind::ALL {
        for item in plan.items.iter().filter(|i| i.kind == kind) {
            let started = Instant::now();
            let outcome = one(item, host, &mut ran).await;
            out.push(Checked {
                id: item.id.clone(),
                kind: item.kind,
                outcome,
                milliseconds: millis(started),
            });
        }
    }
    let order: BTreeMap<&str, usize> = plan
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.id.as_str(), i))
        .collect();
    out.sort_by_key(|c| order.get(c.id.as_str()).copied().unwrap_or(usize::MAX));
    out
}

async fn one(item: &Item, host: &impl Host, ran: &mut BTreeMap<(String, u64), Ran>) -> Outcome {
    if let Some(why) = &item.not_executable {
        return Outcome::NotExecutable { why: why.clone() };
    }
    if let Some(command) = &item.command {
        let wall = item.wall_sec.unwrap_or(COMMAND_SEC);
        let key = (command.clone(), wall);
        if !ran.contains_key(&key) {
            let result = run_command(host, command, Duration::from_secs(wall)).await;
            ran.insert(key.clone(), result);
        }
        return judge_run(item, &ran[&key]);
    }
    let Some(path) = &item.path else {
        return Outcome::NotExecutable {
            why: "neither a command nor a path".to_string(),
        };
    };
    match &item.expect {
        Some(Expect::Exists) => match host.stat(path).await {
            Ok(Stat::Missing) => Outcome::Differed {
                diff: format!("nothing exists at {path}"),
                similarity: None,
            },
            Ok(Stat::Dir) => Outcome::Matched {
                observed: "a directory".to_string(),
            },
            Ok(Stat::File(bytes)) => Outcome::Matched {
                observed: format!("a file of {bytes} bytes"),
            },
            Err(why) => Outcome::CouldNotRun { why },
        },
        Some(expect) => match host.read(path, READ_MAX).await {
            Ok(Some(bytes)) => judge_file(expect, &bytes),
            Ok(None) => Outcome::CouldNotRun {
                why: format!("no file at {path} to read"),
            },
            Err(why) => Outcome::CouldNotRun { why },
        },
        None => Outcome::NotExecutable {
            why: "no expectation".to_string(),
        },
    }
}

/// One candidate's report.
#[must_use]
pub fn report(plan: &Plan, candidate: &str, results: &[Checked]) -> Value {
    json!({
        "schema": REPORT_SCHEMA,
        "task": plan.task,
        "plan": plan.digest,
        "candidate": candidate,
        "call": call(results),
        "score": score(results),
        "tally": tally(results),
        "items": results,
    })
}
