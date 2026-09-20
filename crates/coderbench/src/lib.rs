//! Task manifests and recorded goldens for whole Coder episodes.
//!
//! `crates/gym` scores a door on one item. This crate scores an *episode*:
//! everything from the operator's sentence to the final summary, as one
//! recorded [ATIF](../atif) trace.
//!
//! A golden is not a transcript to match character for character. It is the
//! **path** a correct run takes: which decisions were asked, which way they
//! went, which capabilities were reached, and what came back. Two runs of
//! one task differ in wording and agree on the path, so [`Grade`] compares
//! the path and ignores the prose.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The schema a task manifest declares.
pub const TASK_SCHEMA: &str = "openagents.coderbench.task.v1";

/// What a golden rests on, which a reader needs before trusting it.
///
/// The distinction is the one [`gym::row::LabelSource`] draws for items: a
/// path that was observed and a path somebody wrote down are different
/// evidence, and a file that does not say which is a file that will be read
/// as the stronger one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    /// The episode ran and this is what it did.
    Recorded,
    /// Nobody ran it; this is the path somebody expects.
    Authored,
}

/// One task: what the operator asks, what the environment must hold, and
/// how a run is graded.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Task {
    pub schema: String,
    pub id: String,
    pub family: String,
    /// The operator's sentence, verbatim. This is the input.
    pub request: String,
    pub requires: Requires,
    pub grade: Grade,
    pub timeout_secs: u64,
    #[serde(default)]
    pub notes: String,
}

/// What has to be true of the machine before the task can run at all.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Requires {
    /// Capability slugs, as [NIP-CC](../../nips/coder/NIP-CC.md) names them.
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub repository: String,
}

/// The path a correct run takes.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Grade {
    pub kind: String,
    /// The program the run is expected to select.
    #[serde(default)]
    pub program: String,
    /// How many delegations the run is expected to start.
    #[serde(default)]
    pub delegations: usize,
    /// How many of them are expected to answer correctly.
    #[serde(default)]
    pub delegations_correct: usize,
    /// How many files the run is expected to write. Zero for a read-only
    /// task, and a run that writes one has left the path whatever else it
    /// got right.
    #[serde(default)]
    pub writes_expected: usize,
    /// The decisions the run is expected to ask a decision model, by name.
    #[serde(default)]
    pub decisions: Vec<String>,
    /// The deterministic checks the run is expected to run, by call name.
    ///
    /// Kept apart from `decisions` because the difference matters: a check
    /// is code and answers the same way every time, and a decision is a
    /// model and does not. A task that listed them together would accept a
    /// run that asked a model what a check should have settled.
    #[serde(default)]
    pub checks: Vec<String>,
}

/// Everything wrong with a run, named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The run selected a program the task did not expect.
    Program { expected: String, found: String },
    /// The run started the wrong number of delegations.
    DelegationCount { expected: usize, found: usize },
    /// A delegation the run started did not answer correctly.
    DelegationWrong { id: String },
    /// A decision the task expects was never asked.
    DecisionMissing { name: String },
    /// A deterministic check the task expects never ran.
    CheckMissing { name: String },
    /// The run wrote where the task expects no writes.
    UnexpectedWrite { path: String },
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Program { expected, found } => {
                write!(f, "selected program {found}, expected {expected}")
            }
            Self::DelegationCount { expected, found } => {
                write!(f, "started {found} delegations, expected {expected}")
            }
            Self::DelegationWrong { id } => write!(f, "delegation {id} answered wrongly"),
            Self::DecisionMissing { name } => write!(f, "never asked the {name} decision"),
            Self::CheckMissing { name } => write!(f, "never ran the {name} check"),
            Self::UnexpectedWrite { path } => write!(f, "wrote {path}, expected no writes"),
        }
    }
}

/// What a run did, read back out of its trace.
#[derive(Clone, Debug, Default)]
pub struct Observed {
    pub program: Option<String>,
    pub delegations: Vec<Delegation>,
    pub decisions: BTreeMap<String, Value>,
    /// Deterministic calls the run made, by name.
    pub checks: Vec<String>,
    pub writes: Vec<String>,
}

/// One delegated session, as the trace recorded it.
#[derive(Clone, Debug)]
pub struct Delegation {
    pub id: String,
    pub output: String,
    pub milliseconds: u64,
    /// Whether the delegate answered correctly, when the task can say.
    pub correct: Option<bool>,
}

impl Task {
    /// Reads a task manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, does not parse, or
    /// declares a schema this version does not know. An unknown schema is
    /// an error rather than a warning: a manifest a reader half-understands
    /// grades a run against a rule nobody stated.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let task: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if task.schema != TASK_SCHEMA {
            return Err(format!(
                "{}: schema is {}, this version reads {TASK_SCHEMA}",
                path.display(),
                task.schema
            ));
        }
        Ok(task)
    }

    /// Judges what a run did against the path this task expects.
    ///
    /// Returns every fault rather than the first, because a run that took
    /// the wrong program usually gets several things wrong afterwards and
    /// the first one is rarely the informative one.
    #[must_use]
    pub fn judge(&self, run: &Observed) -> Vec<Fault> {
        let mut faults = Vec::new();
        if !self.grade.program.is_empty() {
            match &run.program {
                Some(found) if found == &self.grade.program => {}
                Some(found) => faults.push(Fault::Program {
                    expected: self.grade.program.clone(),
                    found: found.clone(),
                }),
                None => faults.push(Fault::Program {
                    expected: self.grade.program.clone(),
                    found: "none".into(),
                }),
            }
        }
        if run.delegations.len() != self.grade.delegations {
            faults.push(Fault::DelegationCount {
                expected: self.grade.delegations,
                found: run.delegations.len(),
            });
        }
        for delegation in &run.delegations {
            if delegation.correct == Some(false) {
                faults.push(Fault::DelegationWrong { id: delegation.id.clone() });
            }
        }
        for name in &self.grade.decisions {
            if !run.decisions.contains_key(name) {
                faults.push(Fault::DecisionMissing { name: name.clone() });
            }
        }
        for name in &self.grade.checks {
            if !run.checks.iter().any(|ran| ran == name) {
                faults.push(Fault::CheckMissing { name: name.clone() });
            }
        }
        if self.grade.writes_expected == 0 {
            for path in &run.writes {
                faults.push(Fault::UnexpectedWrite { path: path.clone() });
            }
        }
        faults
    }
}

/// Reads an ATIF trace back into the shape [`Task::judge`] reads.
///
/// # Errors
///
/// Returns an error when the trace cannot be read.
pub fn observe(path: &Path) -> Result<Observed, String> {
    let recording = atif::log::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Observed::default();
    for step in &recording.steps {
        let Some(call) = &step.call else { continue };
        // A decision renders as a call carrying the decision-call schema in
        // `extra`, so a reader tells the two apart by that rather than by
        // the call's name.
        let is_decision = call.extra.get("schema").and_then(Value::as_str)
            == Some(atif::document::DECISION_CALL_SCHEMA);
        if is_decision {
            let answers = call.extra.get("answers").cloned().unwrap_or(Value::Null);
            if call.name == "program" {
                out.program = answers
                    .get("program")
                    .and_then(|a| a.get("choice"))
                    .and_then(|c| c.as_str())
                    .map(str::to_string);
            }
            out.decisions.insert(call.name.clone(), answers);
        } else {
            out.checks.push(call.name.clone());
            if call.name == "delegate" {
                out.delegations.push(Delegation {
                    id: call.id.clone(),
                    output: call.output.clone(),
                    milliseconds: call.milliseconds,
                    correct: call.extra.get("correct").and_then(Value::as_bool),
                });
            }
            if let Some(written) = call.extra.get("wrote").and_then(Value::as_str) {
                out.writes.push(written.to_string());
            }
        }
    }
    Ok(out)
}

/// The directory holding this crate's tasks.
#[must_use]
pub fn tasks_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tasks")
}

/// The directory holding this crate's goldens.
#[must_use]
pub fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("goldens")
}
