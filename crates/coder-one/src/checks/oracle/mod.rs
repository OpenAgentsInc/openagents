//! `checks.oracle`: an acceptance check that doesn't depend on the
//! candidate (issue #9656).
//!
//! In 10 of 11 tasks where Fable 5.1's runs passed, the run got an
//! executable check independent of its own code before or with its first
//! edit: the task's provided checker, a reference program, or an oracle
//! written once from the task's stated definition
//! (`docs/terminal-bench/2026-09-25-fable-pattern-map.md`). Microluna's
//! self-score was the opposite: Luna's own guess, written with the
//! candidate. This component gets the first kind:
//!
//! 1. **Find** ([`find`]). Code looks for a checker the task provides,
//!    among the files and executables the instruction names, and wraps it
//!    as a check. A reference program it finds goes to the writer instead.
//! 2. **Define** ([`define`]). When there's no checker, code lists the
//!    instruction's sentences and stated values, and Jev picks, one Noul
//!    each, the sentences that define a correct result, the values that
//!    are parameters of it, and the sentences that name a boundary input.
//!    The picks become the [`Spec`], and each stated parameter value and
//!    boundary input becomes a case.
//! 3. **Write** ([`write`]). A separate Luna session sees only the spec:
//!    the definition, the input and output formats, the parameters, and
//!    the cases. It never sees the implementation or a candidate: its
//!    commands may read only its own directory, the task files a caller
//!    grants, and the system's programs. Where the task's boundary is a
//!    task container, it runs in a fresh container of the task's image
//!    instead ([`contain`]), and only `oracle.py` comes out. It writes
//!    `oracle.py`, which checks a finished workspace case by case.
//! 4. **Run** ([`run`]). Code runs the oracle on the candidate, in a
//!    network-less container or a writing boundary, and once on the
//!    untouched workspace: an oracle that passes there is trivially
//!    passing. Every result is a [`super::acceptance::Acceptance`] in the
//!    independently supported tier.
//!
//! The lean loop's `oracle` switch (`micro::lean::LeanOracle`) is refused
//! until the offline measurement admits the component ([`ADMITTED`]).

pub mod cli;
pub mod contain;
pub mod define;
pub mod find;
pub mod offline;
#[cfg(test)]
mod tests;
pub mod write;

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::acceptance::{Acceptance, Case, Covers, Provenance, Triviality, Verdict};
use super::contract::clip;
use super::contract::host::{Host, Ran};
use crate::accept::authority::Authority;

/// The schema of a [`Spec`].
pub const SPEC_SCHEMA: &str = "openagents.coder-one.oracle-spec.v1";

/// The schema of an [`Oracle`].
pub const ORACLE_SCHEMA: &str = "openagents.coder-one.oracle.v1";

/// Whether the offline measurement admitted `checks.oracle` into a
/// policy. It didn't: `docs/terminal-bench/2026-09-25-oracle.md`.
pub const ADMITTED: bool = false;

/// One oracle run's wall-time bound, in seconds.
pub const RUN_SEC: u64 = 300;

/// A found checker's wall-time bound, in seconds.
pub const CHECKER_SEC: u64 = 180;

/// The most characters an observed or expected value keeps.
pub const VALUE_CHARS: usize = 400;

/// A stated parameter value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    /// The parameter's name as the task writes it, or the words before
    /// the value when the task gives no name.
    pub name: String,
    pub value: String,
    /// The sentence it comes from.
    pub sentence: String,
    /// Jev's probability that it's a parameter of the computation.
    pub noul: f64,
}

/// The first lines of an input file the task names, read from the
/// untouched workspace: its format, not its code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputHead {
    pub path: String,
    pub head: String,
}

/// One case an oracle answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaseSpec {
    /// `O1` for the task's own inputs, `P<n>` for a parameter value,
    /// `B<n>` for a boundary input.
    pub id: String,
    pub covers: Covers,
}

/// What an oracle writer sees: the task's stated definition, its formats,
/// its parameters, and the cases. Nothing in it comes from a candidate or
/// from the workspace's code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub schema: String,
    pub task: String,
    pub workdir: String,
    /// The digest of the instruction.
    pub instruction: String,
    /// Sentences Jev picked as defining a correct result, in task order.
    pub definition: Vec<String>,
    /// Sentences that name a path, a command, or a format.
    pub formats: Vec<String>,
    pub parameters: Vec<Parameter>,
    /// Sentences Jev picked as naming a boundary input.
    pub boundaries: Vec<String>,
    pub inputs: Vec<InputHead>,
    /// Reference programs the task provides, by path ([`find`]).
    #[serde(default)]
    pub references: Vec<String>,
    pub cases: Vec<CaseSpec>,
    /// The Jev requests, with their keys and answers.
    #[serde(default)]
    pub jev: Vec<Value>,
    pub digest: String,
}

impl Spec {
    /// Seals the spec with its digest.
    #[must_use]
    pub fn sealed(mut self) -> Spec {
        self.schema = SPEC_SCHEMA.to_string();
        self.digest = String::new();
        let mut body = json!(self);
        body["jev"] = Value::Null;
        self.digest = atif::digest(&body);
        self
    }

    /// The cases file an oracle reads.
    #[must_use]
    pub fn cases_file(&self) -> Value {
        json!({ "cases": self.cases })
    }
}

/// How an oracle was obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A checker the task provides.
    Found,
    /// A program a Luna session wrote from the [`Spec`].
    Written,
}

impl Source {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Source::Found => "found",
            Source::Written => "written",
        }
    }
}

/// An oracle, frozen with its digest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Oracle {
    pub schema: String,
    pub task: String,
    pub source: Source,
    /// For a found checker, the command that runs it from the working
    /// directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// For a found checker, the file it runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// For a written oracle, its files by name: `oracle.py` and
    /// `cases.json`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, String>,
    /// The digest of the spec it was written from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<String>,
    /// The writing session: its ending, turns, cost, and trace.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub writer: Value,
    pub digest: String,
}

impl Oracle {
    /// Seals `self` with its digest.
    #[must_use]
    pub fn sealed(mut self) -> Oracle {
        self.schema = ORACLE_SCHEMA.to_string();
        self.digest = String::new();
        let mut body = json!(self);
        body["writer"] = Value::Null;
        self.digest = atif::digest(&body);
        self
    }

    /// The command that runs the oracle from the working directory, with
    /// its files staged at `dir`.
    #[must_use]
    pub fn invocation(&self, dir: &str, workdir: &str) -> Option<String> {
        match self.source {
            Source::Found => self.command.clone(),
            Source::Written => self.files.contains_key("oracle.py").then(|| {
                let q = crate::accept::runner::sh_quote;
                format!(
                    "python3 {} {} {}",
                    q(&format!("{dir}/oracle.py")),
                    q(workdir),
                    q(&format!("{dir}/cases.json"))
                )
            }),
        }
    }
}

/// Runs `oracle` with its files staged at `dir` on `host`, whose working
/// directory is `workdir`, and returns its result on `candidate`.
/// `untouched`, when given, is the same oracle's result on the untouched
/// workspace and decides [`Triviality`].
pub async fn run(
    oracle: &Oracle,
    spec: Option<&Spec>,
    host: &impl Host,
    dir: &str,
    workdir: &str,
    candidate: &str,
    untouched: Option<&Acceptance>,
) -> Acceptance {
    let started = std::time::Instant::now();
    let cases = match oracle.invocation(dir, workdir) {
        None => vec![unrunnable("O1", "the oracle has no program to run")],
        Some(command) => {
            let wall = match oracle.source {
                Source::Found => CHECKER_SEC,
                Source::Written => RUN_SEC,
            };
            let ran = super::contract::run_command(host, &command, Duration::from_secs(wall)).await;
            match oracle.source {
                Source::Found => vec![checker_case(&command, &ran)],
                Source::Written => written_cases(spec, &ran),
            }
        }
    };
    let millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut acceptance = Acceptance::new(
        &oracle.task,
        candidate,
        Authority::IndependentlySupported,
        provenance(oracle),
        cases,
        Triviality {
            untouched: untouched.map(|u| u.passed() == Some(true)),
            empty_output: None,
        },
    );
    if let Some(first) = acceptance.cases.first_mut()
        && first.milliseconds == 0
    {
        first.milliseconds = millis;
    }
    acceptance
}

/// The oracle's provenance.
#[must_use]
pub fn provenance(oracle: &Oracle) -> Provenance {
    Provenance {
        component: "checks.oracle".to_string(),
        source: oracle.source.word().to_string(),
        origin: oracle.origin.clone().or_else(|| oracle.command.clone()),
        digest: Some(oracle.digest.clone()),
        detail: json!({ "spec": oracle.spec, "writer": oracle.writer }),
    }
}

fn unrunnable(id: &str, why: &str) -> Case {
    Case {
        id: id.to_string(),
        covers: Covers::default(),
        verdict: Verdict::CouldNotRun,
        observed: None,
        expected: None,
        detail: Some(why.to_string()),
        milliseconds: 0,
    }
}

fn tail(ran: &Ran) -> String {
    let mut text = ran.stderr.trim().to_string();
    if text.is_empty() {
        text = ran.stdout.trim().to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let start = chars.len().saturating_sub(VALUE_CHARS);
    chars[start..].iter().collect()
}

/// Why a run says nothing about the candidate, when it doesn't.
fn not_run(ran: &Ran) -> Option<String> {
    if let Some(why) = &ran.failed {
        return Some(format!("the oracle didn't run: {why}"));
    }
    if ran.timed_out {
        return Some("the oracle ran out of time".to_string());
    }
    if matches!(ran.exit, Some(126 | 127)) || ran.exit.is_none() {
        return Some(format!(
            "exit {:?}: the program or its interpreter wasn't found or was stopped: {}",
            ran.exit,
            clip(ran.stderr.trim(), 300)
        ));
    }
    None
}

/// A found checker's one case: exit 0 passes.
#[must_use]
pub fn checker_case(command: &str, ran: &Ran) -> Case {
    let covers = Covers {
        input: Some(command.to_string()),
        from: Some("command".to_string()),
        ..Covers::default()
    };
    let (verdict, observed, detail) = match not_run(ran) {
        Some(why) => (Verdict::CouldNotRun, None, Some(why)),
        None if ran.exit == Some(0) => (Verdict::Passed, Some("exit 0".to_string()), None),
        None => (
            Verdict::Failed,
            Some(format!("exit {}", ran.exit.unwrap_or(-1))),
            Some(tail(ran)),
        ),
    };
    Case {
        id: "O1".to_string(),
        covers,
        verdict,
        observed,
        expected: Some("exit 0".to_string()),
        detail,
        milliseconds: ran.milliseconds,
    }
}

fn text_of(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Null => None,
        Value::String(s) => Some(clip(s, VALUE_CHARS)),
        other => Some(clip(&other.to_string(), VALUE_CHARS)),
    }
}

/// The result lines a written oracle printed, by case ID: every line of
/// standard output that parses as a JSON object with a `case` field.
#[must_use]
pub fn parse_lines(stdout: &str) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for line in stdout.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let Some(id) = value.get("case").and_then(Value::as_str) else {
            continue;
        };
        out.entry(id.to_string()).or_insert(value);
    }
    out
}

/// A written oracle's cases: one per case of the spec, in spec order. A
/// case the oracle printed nothing for couldn't run.
#[must_use]
pub fn written_cases(spec: Option<&Spec>, ran: &Ran) -> Vec<Case> {
    let printed = parse_lines(&ran.stdout);
    let expected: Vec<CaseSpec> = spec.map_or_else(
        || {
            printed
                .keys()
                .map(|id| CaseSpec {
                    id: id.clone(),
                    covers: Covers::default(),
                })
                .collect()
        },
        |s| s.cases.clone(),
    );
    let why_none = not_run(ran).unwrap_or_else(|| {
        format!(
            "the oracle printed no result for this case (exit {:?}): {}",
            ran.exit,
            clip(&tail(ran), 300)
        )
    });
    expected
        .into_iter()
        .map(|case| match printed.get(&case.id) {
            None => Case {
                id: case.id,
                covers: case.covers,
                verdict: Verdict::CouldNotRun,
                observed: None,
                expected: None,
                detail: Some(why_none.clone()),
                milliseconds: 0,
            },
            Some(line) => {
                let verdict = match line.get("verdict").and_then(Value::as_str) {
                    Some("passed" | "pass") => Verdict::Passed,
                    Some("failed" | "fail") => Verdict::Failed,
                    _ => Verdict::CouldNotRun,
                };
                Case {
                    id: case.id,
                    covers: case.covers,
                    verdict,
                    observed: text_of(line.get("observed")),
                    expected: text_of(line.get("expected")),
                    detail: text_of(line.get("detail")),
                    milliseconds: 0,
                }
            }
        })
        .collect()
}

/// Whether the oracle may stand for the self-score: it ran to an answer
/// on the untouched workspace and wasn't passing there.
#[must_use]
pub fn usable(untouched: &Acceptance) -> bool {
    untouched.passed() == Some(false)
}

/// The lines a lean session reads about the oracle's result.
#[must_use]
pub fn brief_line(number: u32, result: &Acceptance) -> String {
    match result.passed() {
        Some(true) => format!(
            "After session {number}, the host's independent oracle passed: {}.",
            result.summary()
        ),
        Some(false) => format!(
            "After session {number}, the host's independent oracle failed, so the host won't \
             accept a finish yet: {}. The oracle was written from the task's stated definition \
             without seeing your code; if you're sure it's wrong, say why in the summary.",
            result.summary()
        ),
        None => format!(
            "After session {number}, the host's independent oracle couldn't run: {}.",
            result
                .cases
                .first()
                .and_then(|c| c.detail.clone())
                .unwrap_or_default()
        ),
    }
}
