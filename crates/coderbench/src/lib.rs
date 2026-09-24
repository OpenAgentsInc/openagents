//! Task manifests and recorded goldens for whole Coder episodes.
//!
//! `crates/gym` scores a door on one item. This crate scores an *episode*:
//! everything from the operator's sentence to the final summary, as one
//! recorded [ATIF](../atif) trace.
//!
//! A golden is not a transcript to match character for character. It is the
//! **path** a correct run takes: which decisions were asked, which way they
//! went, which capabilities were reached, and what came back. Two runs of
//! one task differ in wording and agree on the path, so [`Task::judge`]
//! compares the path and ignores the prose.
//!
//! # A grade answers with three values
//!
//! Whether a run took the path is not the only thing worth saying. A run
//! whose evidence is missing is a third state, and the vocabulary for it
//! already exists here: [`Verdict`] is `crates/gym`'s, where `failed` beats
//! `unverifiable` beats `passed`. This crate uses that type rather than a
//! second word for the same idea.
//!
//! The distinction is the point of the grade. A delegation that recorded no
//! correctness did not answer correctly and did not answer wrongly; nobody
//! looked. A trace nobody compared against the workspace says nothing about
//! what the run wrote. Both are [`Verdict::Unverifiable`], and neither is a
//! pass.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use atif::Outcome;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod drive;
pub mod preflight;
pub mod tune;

/// The three-valued verdict, which is `crates/gym`'s rather than a second
/// copy of it.
pub use gym::gate::Verdict;

/// The schema a task manifest declares.
pub const TASK_SCHEMA: &str = "openagents.coderbench.task.v1";

/// The name a program-selection decision goes by in a trace.
///
/// [`observe`] reads the selected program out of a call with this name, and
/// [`Task::judge`] places a program fault at this step of the path, so the
/// two agree about which call is the selection.
pub const PROGRAM_CALL: &str = "program";

/// The name a delegation goes by in a trace.
pub const DELEGATE_CALL: &str = "delegate";

/// What a golden rests on, which a reader needs before trusting it.
///
/// The distinction is the one [`gym::row::LabelSource`] draws for items: a
/// path that was observed and a path somebody wrote down are different
/// evidence, and a file that does not say which is a file that will be read
/// as the stronger one.
///
/// Three states rather than two, because "recorded" was covering two
/// different things. A recording of the program under test and a recording
/// of something else doing what that program should do are not the same
/// evidence, and the second is the one a reader over-trusts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    /// Coder ran and this is what it did. The only kind that shows the
    /// program under test doing the thing.
    Observed,
    /// Every call is real and something else drove them. A staged golden is
    /// a specification written in the format a run emits, which is useful
    /// and is not evidence about Coder.
    Staged,
    /// Nobody ran it. The path somebody expects.
    Authored,
}

/// What a golden says about itself, beside the trace.
///
/// A sidecar rather than a field inside the trace, because ATIF describes a
/// session and this describes the file. A session cannot say who was
/// driving it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GoldenMeta {
    pub schema: String,
    /// Which task this golden is a golden for.
    pub task: String,
    pub provenance: Provenance,
    /// What drove the episode. `coder` for an observed one; for a staged
    /// one, what stood in for it.
    pub orchestrator: String,
    /// The commit the episode ran at.
    #[serde(default)]
    pub repository_commit: String,
    #[serde(default)]
    pub recorded: String,
    /// Why this golden is not observed yet, when it is not.
    #[serde(default)]
    pub note: String,
}

/// The schema a golden's sidecar declares.
pub const GOLDEN_META_SCHEMA: &str = "openagents.coderbench.golden.v1";

impl GoldenMeta {
    /// Reads a golden's sidecar.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, does not parse, or
    /// declares a schema this version does not know. A golden with no
    /// sidecar is an error rather than a default, because the default a
    /// reader would assume is the strongest one.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let meta: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.schema != GOLDEN_META_SCHEMA {
            return Err(format!(
                "{}: schema is {}, this version reads {GOLDEN_META_SCHEMA}",
                path.display(),
                meta.schema
            ));
        }
        Ok(meta)
    }
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
    /// Capability slugs, as [NIP-CAP](../../nips/openagents/NIP-CAP.md) names them.
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub repository: String,
    /// The commit the task's expected answers were true at.
    ///
    /// A task whose grade depends on repository contents needs one. A
    /// rename moved a path `devin-fan-out-six` reads and invalidated its
    /// first recording, which is how this field got here.
    #[serde(default)]
    pub base: String,
    /// What a capability refuses, beyond being absent.
    ///
    /// A present executor can still decline a particular directory, and a
    /// probe that reports only presence cannot say so.
    #[serde(default)]
    pub capabilities_refuse: BTreeMap<String, Vec<String>>,
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
    ///
    /// A delegation counts here only when the trace says it completed, says
    /// it was correct, and holds the answer. One that recorded none of that
    /// is unverified, and the shortfall it leaves is
    /// [`Verdict::Unverifiable`] rather than a pass.
    ///
    /// When [`Grade::expects`] states the answers itself, a delegation
    /// counts when the recorded call is checked against the manifest's own
    /// expectation instead, and what the run asserted about itself adds
    /// nothing either way.
    #[serde(default)]
    pub delegations_correct: usize,
    /// The answers the task expects, one per delegation, in the order the
    /// request asks the questions.
    ///
    /// This is the manifest's own copy of the answers, so a run is checked
    /// against something other than what it said about itself: the runtime
    /// is never told them, and a `correct` flag the trace recorded is a
    /// claim rather than the check. Each entry pins the prompt that
    /// identifies the delegation and the output it owes, and the list is
    /// positional — a reordered, duplicated, or substituted delegation is
    /// not the delegation the task expects in its place.
    ///
    /// A non-empty `expects` pins every delegation the run owes, so its
    /// length is `delegations`, and every pinned answer must verify, so
    /// `delegations_correct` is the same count. Anything else is a
    /// malformed manifest rather than a passing grade: a prompt or answer
    /// that is blank, a prompt two entries share, or a count that
    /// disagrees fails [`Task::load`] and faults a [`Task::judge`] call on
    /// a task built by hand.
    ///
    /// A task that states no expectations keeps the trace-reported evidence rule:
    /// only a delegation the trace itself records as checked counts, and
    /// one that recorded nothing either way is [`Verdict::Unverifiable`].
    #[serde(default)]
    pub expects: Vec<ExpectedAnswer>,
    /// How many distinct workspace paths the run is expected to change.
    /// This includes directory and metadata changes; a rename names both
    /// its source and destination. Zero describes a read-only task.
    ///
    /// Judged against the workspace rather than against what the run said
    /// about itself. Absent `wrote` metadata is unknown, not proof.
    #[serde(default)]
    pub writes_expected: usize,
    /// The decisions the run is expected to ask a decision model, by name.
    #[serde(default)]
    pub decisions: Vec<String>,
    /// What those decisions have to answer.
    ///
    /// A decision that was asked and answered nothing did not go the right
    /// way, and a name in `decisions` cannot say which way it went. State
    /// the predicate the run gates on, and only that one: a predicate the
    /// run does not act on grades the door rather than the path.
    #[serde(default)]
    pub answers: Vec<Expected>,
    /// The deterministic checks the run is expected to run, by call name.
    ///
    /// Kept apart from `decisions` because the difference matters: a check
    /// is code and answers the same way every time, and a decision is a
    /// model and does not. A task that listed them together would accept a
    /// run that asked a model what a check should have settled.
    #[serde(default)]
    pub checks: Vec<String>,
    /// The steps a correct run takes, in the order it takes them.
    ///
    /// `decisions` and `checks` say which steps a run owes; this says when,
    /// and [`Task::judge`] holds the run to it. A run that admitted a
    /// delegation before it probed for the executor did the steps in an
    /// order that cannot establish what they establish.
    ///
    /// Faults are reported in this order too, so the first fault is the
    /// earliest thing that went wrong rather than the first thing the
    /// checker happened to test. A step this list does not name sorts last.
    #[serde(default)]
    pub path: Vec<String>,
    /// The endings this task allows, by the word [`Ending::word`] spells.
    ///
    /// Empty means `answered`. Stating it per task is what keeps a
    /// timed-out or declined run from grading clean because the partial
    /// trace it left holds the expected names.
    #[serde(default)]
    pub endings: Vec<String>,
}

impl Grade {
    /// The endings this task allows, which is `answered` when it says
    /// nothing.
    #[must_use]
    pub fn allowed(&self) -> Vec<String> {
        if self.endings.is_empty() {
            vec![Ending::Answered.word().to_string()]
        } else {
            self.endings.clone()
        }
    }
}

/// What one question of one decision has to answer.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Expected {
    /// The decision call's name.
    pub decision: String,
    /// The question inside it.
    pub question: String,
    /// What the answer has to satisfy.
    pub holds: Predicate,
}

/// A condition on one typed answer.
///
/// A number reads the answer's `noul` first and its `confidence` second, so
/// one predicate covers a belief and the confidence of a choice.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Predicate {
    /// The choice the answer names.
    Choice(String),
    /// A floor, which is the shape a gate takes.
    AtLeast(f64),
    /// A ceiling.
    AtMost(f64),
}

impl Predicate {
    /// Whether the answer satisfies this predicate, or `None` when the
    /// answer does not carry the field this reads. Unknown is never true.
    #[must_use]
    pub fn holds(&self, answer: &Value) -> Option<bool> {
        match self {
            Self::Choice(wanted) => Some(answer.get("choice")?.as_str()? == wanted),
            Self::AtLeast(floor) => Some(number(answer)? >= *floor),
            Self::AtMost(ceiling) => Some(number(answer)? <= *ceiling),
        }
    }

    /// What this predicate asks for, in the words a fault prints.
    #[must_use]
    pub fn wanted(&self) -> String {
        match self {
            Self::Choice(choice) => choice.clone(),
            Self::AtLeast(floor) => format!("at least {floor}"),
            Self::AtMost(ceiling) => format!("at most {ceiling}"),
        }
    }
}

/// One delegation's expected answer: the prompt that identifies the
/// delegation, and the output it owes.
///
/// The prompt is the identity because it is the one thing a recorded call
/// and the task's request both carry verbatim — a call id is assigned as
/// the run writes it and a request does not name one. It compares
/// exactly, byte for byte: case, spacing, and wording are the question,
/// so a recorded prompt that differs in any of them is a different
/// question however it answered.
///
/// The answer compares after trimming whitespace from the ends, with case
/// and interior spacing intact — `L1, L2, L3` and `l1, l2, l3` are
/// different answers, because identifiers and kind numbers change meaning
/// with their case. A task that needs a looser comparison states it in
/// the prompt's wording instead.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ExpectedAnswer {
    /// The prompt the delegation is asked, exactly as the request's list
    /// item carries it.
    pub prompt: String,
    /// The output the task expects back.
    pub answer: String,
}

/// The number a typed answer carries: a belief, or the confidence of a
/// choice.
fn number(answer: &Value) -> Option<f64> {
    answer
        .get("noul")
        .or_else(|| answer.get("confidence"))
        .and_then(Value::as_f64)
}

/// What a typed answer said, in the words a fault prints.
fn said(answer: &Value) -> String {
    if let Some(choice) = answer.get("choice").and_then(Value::as_str) {
        return choice.to_string();
    }
    match number(answer) {
        Some(number) => format!("{number}"),
        None => answer.to_string(),
    }
}

/// How the episode ended.
///
/// A trace records that the session closed itself; it does not record what
/// the turn concluded. So a trace read on its own is [`Ending::Closed`],
/// and the exit code the driver saw is what promotes it to
/// [`Ending::Answered`]. That difference is the one the driver used to
/// print and then drop.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Ending {
    /// The turn finished and the agent answered.
    Answered,
    /// The turn finished and the router declined it. Nothing went wrong.
    Declined,
    /// The turn did not finish.
    Failed,
    /// The turn ran past the task's timeout and was stopped.
    TimedOut,
    /// The trace closed itself and does not say how the turn ended.
    Closed,
    /// Nothing observed the ending.
    #[default]
    Unobserved,
    /// Something else, which the word carries.
    Other(String),
}

impl Ending {
    /// The word a task's `endings` list spells this with.
    #[must_use]
    pub fn word(&self) -> &str {
        match self {
            Self::Answered => "answered",
            Self::Declined => "declined",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::Closed => "closed",
            Self::Unobserved => "unobserved",
            Self::Other(word) => word,
        }
    }
}

impl std::fmt::Display for Ending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.word())
    }
}

impl From<drive::Outcome> for Ending {
    fn from(outcome: drive::Outcome) -> Self {
        match outcome {
            drive::Outcome::Answered => Self::Answered,
            drive::Outcome::Failed => Self::Failed,
            drive::Outcome::Declined => Self::Declined,
            drive::Outcome::TimedOut => Self::TimedOut,
            drive::Outcome::Usage => Self::Other("refused the command line".to_string()),
            drive::Outcome::Unknown(code) => Self::Other(format!("exited {code}")),
            drive::Outcome::Signal => Self::Other("died on a signal".to_string()),
        }
    }
}

/// Everything wrong with a run, named.
///
/// Each fault carries its own [`Verdict`] through [`Fault::verdict`], so a
/// list of faults says which of them were measured and which of them nobody
/// could measure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The run selected a program the task did not expect.
    Program { expected: String, found: String },
    /// The run started the wrong number of delegations.
    DelegationCount { expected: usize, found: usize },
    /// The task expects a delegation asking this, and the run recorded
    /// none in its place.
    DelegationMissing { prompt: String },
    /// The delegation in a slot asked a different question than the one
    /// the task pins there. A reordered, duplicated, or substituted
    /// delegation reads the same way: the recorded prompt is not the
    /// expected one.
    DelegationMisattributed {
        id: String,
        wanted: String,
        found: String,
    },
    /// A delegation's recorded output is not the answer the task expects.
    DelegationAnswered {
        id: String,
        wanted: String,
        found: String,
    },
    /// A delegation the run started did not answer correctly.
    DelegationWrong { id: String },
    /// A delegation did not complete.
    DelegationFailed { id: String, outcome: String },
    /// A delegation recorded no correctness either way, or recorded one
    /// with no answer to show for it.
    DelegationUnverified { id: String },
    /// Fewer delegations are recorded correct than the task requires.
    DelegationsCorrect {
        expected: usize,
        verified: usize,
        unverified: usize,
    },
    /// A decision the task expects was never asked.
    DecisionMissing { name: String },
    /// A decision was asked and the call did not complete.
    DecisionFailed { name: String, outcome: String },
    /// A decision was asked and came back with no answers.
    DecisionUnanswered { name: String },
    /// A decision did not answer a question the task reads.
    AnswerMissing { decision: String, question: String },
    /// A decision answered a question the wrong way.
    AnswerWrong {
        decision: String,
        question: String,
        wanted: String,
        found: String,
    },
    /// A deterministic check the task expects never ran.
    CheckMissing { name: String },
    /// A check ran under the expected name and did not complete.
    CheckFailed { name: String, outcome: String },
    /// A step ran before one the path puts ahead of it.
    OutOfOrder { step: String, before: String },
    /// The run wrote where the task expects no writes.
    UnexpectedWrite { path: String },
    /// The run wrote a different number of files than the task expects.
    WriteCount { expected: usize, found: usize },
    /// Nothing compared the workspace, so what the run wrote is unknown.
    WritesUnobserved,
    /// The episode ended a way the task does not allow.
    Ended { found: String, allowed: Vec<String> },
    /// Nothing observed how the episode ended.
    EndingUnobserved { allowed: Vec<String> },
    /// The trace closed without saying how the episode ended.
    EndingUnstated { allowed: Vec<String> },
    /// The trace has no end record, so the session never closed itself.
    Unfinished,
    /// Part of the trace did not read back.
    TornTrace { lines: usize },
    /// The task's own expectation cannot check a run: it pins no prompt or
    /// no answer, repeats a prompt, or states a count that disagrees with
    /// the delegations it pins. This is a fault about the manifest rather
    /// than about the run, and a manifest that cannot check cannot pass.
    ExpectationMalformed { why: String },
}

impl Fault {
    /// What this fault establishes.
    ///
    /// Measured and wrong is [`Verdict::Failed`]. Missing evidence is
    /// [`Verdict::Unverifiable`], which is not a pass and is not a failure.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        match self {
            Self::Program { .. }
            | Self::DelegationCount { .. }
            | Self::DelegationMissing { .. }
            | Self::DelegationMisattributed { .. }
            | Self::DelegationAnswered { .. }
            | Self::ExpectationMalformed { .. }
            | Self::DelegationWrong { .. }
            | Self::DelegationFailed { .. }
            | Self::DecisionMissing { .. }
            | Self::DecisionFailed { .. }
            | Self::AnswerWrong { .. }
            | Self::CheckMissing { .. }
            | Self::CheckFailed { .. }
            | Self::OutOfOrder { .. }
            | Self::UnexpectedWrite { .. }
            | Self::WriteCount { .. }
            | Self::Ended { .. }
            | Self::Unfinished => Verdict::Failed,
            Self::DelegationUnverified { .. }
            | Self::DecisionUnanswered { .. }
            | Self::AnswerMissing { .. }
            | Self::WritesUnobserved
            | Self::EndingUnobserved { .. }
            | Self::EndingUnstated { .. }
            | Self::TornTrace { .. } => Verdict::Unverifiable,
            // A shortfall the unverified delegations could still cover is
            // unknown. One they could not cover is measured and short.
            Self::DelegationsCorrect {
                expected,
                verified,
                unverified,
            } => {
                if verified + unverified >= *expected {
                    Verdict::Unverifiable
                } else {
                    Verdict::Failed
                }
            }
        }
    }
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
            Self::DelegationMissing { prompt } => {
                write!(f, "no delegation asked {prompt}")
            }
            Self::DelegationMisattributed { id, wanted, found } => {
                if found.is_empty() {
                    write!(f, "delegation {id} asked nothing, expected {wanted}")
                } else {
                    write!(f, "delegation {id} asked {found}, expected {wanted}")
                }
            }
            Self::DelegationAnswered { id, wanted, found } => {
                if found.is_empty() {
                    write!(f, "delegation {id} answered nothing, expected {wanted}")
                } else {
                    write!(f, "delegation {id} answered {found}, expected {wanted}")
                }
            }
            Self::DelegationWrong { id } => write!(f, "delegation {id} answered wrongly"),
            Self::DelegationFailed { id, outcome } => {
                write!(f, "delegation {id} {outcome} rather than completing")
            }
            Self::DelegationUnverified { id } => write!(
                f,
                "delegation {id} has no recorded check of its answer, \
                 so nothing shows that it answered correctly"
            ),
            Self::DelegationsCorrect {
                expected,
                verified,
                unverified,
            } => write!(
                f,
                "{verified} delegations have answers recorded as correct, expected {expected}; \
                 {unverified} have no recorded check"
            ),
            Self::DecisionMissing { name } => write!(f, "never asked the {name} decision"),
            Self::DecisionFailed { name, outcome } => {
                write!(f, "the {name} decision {outcome} rather than completing")
            }
            Self::DecisionUnanswered { name } => {
                write!(f, "the {name} decision came back with no answers")
            }
            Self::AnswerMissing { decision, question } => {
                write!(f, "the {decision} decision did not answer {question}")
            }
            Self::AnswerWrong {
                decision,
                question,
                wanted,
                found,
            } => write!(
                f,
                "the {decision} decision answered {question} {found}, expected {wanted}"
            ),
            Self::CheckMissing { name } => write!(f, "never ran the {name} check"),
            Self::CheckFailed { name, outcome } => {
                write!(f, "the {name} check {outcome} rather than completing")
            }
            Self::OutOfOrder { step, before } => {
                write!(
                    f,
                    "ran {step} before {before}, but the task expects {before} first"
                )
            }
            Self::UnexpectedWrite { path } => write!(f, "wrote {path}, expected no writes"),
            Self::WriteCount { expected, found } => {
                write!(f, "wrote {found} files, expected {expected}")
            }
            Self::WritesUnobserved => write!(
                f,
                "nothing compared the workspace before and after the run, so no evidence shows that it wrote nothing"
            ),
            Self::Ended { found, allowed } => write!(
                f,
                "the episode {found}; the task allows {}",
                allowed.join(", ")
            ),
            Self::EndingUnobserved { allowed } => write!(
                f,
                "nothing observed how the episode ended; the task allows {}",
                allowed.join(", ")
            ),
            Self::EndingUnstated { allowed } => write!(
                f,
                "the trace closed without saying how the episode ended; the task allows {}",
                allowed.join(", ")
            ),
            Self::Unfinished => {
                write!(
                    f,
                    "the trace has no end record, so the session never closed"
                )
            }
            Self::TornTrace { lines } => write!(
                f,
                "{lines} {} of the trace could not be read",
                if *lines == 1 { "line" } else { "lines" }
            ),
            Self::ExpectationMalformed { why } => {
                write!(f, "the task's expected answers cannot check a run: {why}")
            }
        }
    }
}

/// What a task concluded about a run.
#[derive(Clone, Debug)]
pub struct Judgment {
    pub verdict: Verdict,
    /// Every fault, in the order [`Grade::path`] states.
    pub faults: Vec<Fault>,
}

impl Judgment {
    /// Whether the run took the path the task expects, with the evidence to
    /// show it.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.verdict == Verdict::Passed
    }
}

/// What a run did, read back out of its trace and off the machine it ran
/// on.
#[derive(Clone, Debug, Default)]
pub struct Observed {
    pub program: Option<String>,
    pub delegations: Vec<Delegation>,
    /// The decisions the run asked, by name, with what came back.
    pub decisions: BTreeMap<String, Asked>,
    /// Deterministic calls the run made, in order, with how they ended.
    pub checks: Vec<Check>,
    /// Every call's name, in the order the trace records it, delegations
    /// and all. This is what the step order is judged against.
    pub order: Vec<String>,
    /// Writes a call reported making. A self-report: the ones it names
    /// count, and the ones it does not name establish nothing.
    pub writes: Vec<String>,
    /// What the workspace said, read independently of the run.
    pub workspace: Option<Workspace>,
    /// How the episode ended.
    pub ending: Ending,
    /// Whether the trace holds an end record.
    pub closed: bool,
    /// Lines of the trace that did not read back.
    pub unreadable_lines: usize,
    /// Wall clock from the first step to the last, as the trace records it.
    /// Zero when the trace holds fewer than two steps.
    pub milliseconds: u64,
}

/// One delegated session, as the trace recorded it.
#[derive(Clone, Debug)]
pub struct Delegation {
    pub id: String,
    /// The prompt the call was handed, which is how the task tells one
    /// delegation from another.
    pub prompt: String,
    pub output: String,
    pub milliseconds: u64,
    /// How the call ended.
    pub outcome: Outcome,
    /// Whether the delegate answered correctly, when the trace says.
    pub correct: Option<bool>,
}

impl Delegation {
    /// Whether this delegation is recorded as having completed and answered
    /// correctly.
    ///
    /// Three things have to hold, because any one of them missing means
    /// nobody established the answer: the call completed, the trace says
    /// the answer was correct, and there is an answer to have checked.
    #[must_use]
    pub fn verified(&self) -> bool {
        self.outcome == Outcome::Completed
            && self.correct == Some(true)
            && !self.output.trim().is_empty()
    }

    /// Whether this delegation is the one `want` describes and produced
    /// the answer it owes.
    ///
    /// This is the check [`Grade::expects`] buys, and the one the run's
    /// report counts with: the recorded call asked exactly the question
    /// the task pinned, it completed, and its recorded output is the
    /// answer the task owns. The run's own claim adds nothing either way —
    /// `correct` saying `true` is not needed, and `correct` saying
    /// `false` is the record contradicting the manifest, which cannot
    /// verify. An expectation with a blank prompt or a blank answer is
    /// one nothing can satisfy, so it verifies nothing.
    #[must_use]
    pub fn verified_against(&self, want: &ExpectedAnswer) -> bool {
        self.outcome == Outcome::Completed
            && !want.prompt.trim().is_empty()
            && !want.answer.trim().is_empty()
            && self.prompt == want.prompt
            && self.correct != Some(false)
            && self.output.trim() == want.answer.trim()
    }
}

/// A decision call and what came back.
#[derive(Clone, Debug)]
pub struct Asked {
    /// The typed answers, or null when the call returned none.
    pub answers: Value,
    pub outcome: Outcome,
}

/// A deterministic call and how it ended.
#[derive(Clone, Debug)]
pub struct Check {
    pub name: String,
    pub outcome: Outcome,
}

/// What the checkout looked like, read independently of what the run said
/// about itself.
///
/// A delegate that wrote a file and did not mention it is the case a
/// `wrote` field cannot cover, and it is the case that matters for a task
/// that forbids writes.
#[derive(Clone, Debug, Default)]
pub struct Workspace {
    /// Paths that differ from what was there before the run.
    pub changed: Vec<String>,
}

impl Workspace {
    /// Compares independent filesystem observations of the whole workspace.
    ///
    /// This includes ignored files and changes to files that were already dirty.
    /// It does not use Git's status codes or the delegate's account of its writes.
    ///
    /// # Errors
    ///
    /// Returns why the comparison is unverifiable when either observation is
    /// incomplete or the roots differ. A partial reading cannot establish a
    /// clean workspace.
    pub fn between(
        before: &coder_boundary::snapshot::Snapshot,
        after: &coder_boundary::snapshot::Snapshot,
    ) -> Result<Self, String> {
        use coder_boundary::snapshot::{Change, Verdict, compare};
        let changes = match compare(before, after) {
            Verdict::Clean => return Ok(Self::default()),
            Verdict::Changed(changes) => changes,
            Verdict::Unverifiable(reason) => return Err(reason),
        };
        let mut paths = BTreeSet::new();
        for change in changes {
            match change {
                Change::Created { path }
                | Change::Removed { path }
                | Change::Modified { path }
                | Change::Retyped { path } => {
                    paths.insert(path);
                }
                Change::Renamed { from, to, .. } => {
                    paths.insert(from);
                    paths.insert(to);
                }
            }
        }
        Ok(Self {
            changed: paths.iter().map(|path| workspace_path(path)).collect(),
        })
    }
}

// Keep path labels distinct because write counts compare these labels. Names
// requiring an escape use Debug, including valid names containing literal escape
// characters that could otherwise collide with a non-UTF-8 name's rendering.
fn workspace_path(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        return ".".to_string();
    }
    match path.to_str() {
        Some(text)
            if !text
                .chars()
                .any(|c| c.is_control() || matches!(c, '\\' | '"')) =>
        {
            text.to_string()
        }
        _ => format!("{:?}", path.as_os_str()),
    }
}

#[cfg(all(test, unix))]
mod workspace_path_tests {
    use super::workspace_path;
    use std::collections::BTreeSet;
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    #[test]
    fn escaped_names_do_not_collapse_distinct_write_paths() {
        let invalid = OsString::from_vec(vec![b'f', 0xff]);
        let literal = OsString::from(format!("{invalid:?}"));
        let paths = [invalid, literal, "line\nend".into(), "line\\nend".into()];
        let labels: BTreeSet<String> = paths
            .into_iter()
            .map(|path| workspace_path(&PathBuf::from(path)))
            .collect();
        assert_eq!(labels.len(), 4);
    }
}

/// The word a call's outcome reads as in a fault.
fn outcome_word(outcome: Outcome) -> String {
    match outcome {
        Outcome::Completed => "completed".to_string(),
        Outcome::Failed => "failed".to_string(),
        Outcome::Cancelled => "was cancelled".to_string(),
    }
}

impl Task {
    /// Reads a task manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, does not parse, or
    /// declares a schema this version does not know. An unknown schema is
    /// an error rather than a warning: a manifest a reader half-understands
    /// grades a run against a rule nobody stated. So is a manifest whose
    /// `expects` entries cannot check a run — a blank prompt or answer, a
    /// repeated prompt, or a count that does not pin one per delegation —
    /// because grading against half an expectation silently passes what it
    /// cannot verify.
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
        let malformed = task.malformed_expectations();
        if !malformed.is_empty() {
            return Err(format!("{}: {}", path.display(), malformed.join("; ")));
        }
        Ok(task)
    }

    /// Judges what a run did against the path this task expects.
    ///
    /// Returns every fault rather than the first, because a run that took
    /// the wrong program usually gets several things wrong afterwards and
    /// the first one is rarely the informative one.
    ///
    /// The faults come back in the order [`Grade::path`] states, so the
    /// first one is the earliest thing that went wrong. Reporting them in
    /// the order the checker tests them would put a missing delegation
    /// above the missing probe that explains it. Faults about the record
    /// itself sort after every step, because they are about the trace
    /// rather than about a place in the path.
    #[must_use]
    pub fn judge(&self, run: &Observed) -> Judgment {
        let mut faults = Vec::new();
        self.judge_program(run, &mut faults);
        self.judge_delegations(run, &mut faults);
        self.judge_decisions(run, &mut faults);
        self.judge_checks(run, &mut faults);
        self.judge_order(run, &mut faults);
        self.judge_writes(run, &mut faults);
        self.judge_record(run, &mut faults);
        // A stable sort, so faults that share a step keep the order they
        // were found in: how many delegations ran, then which of them
        // answered wrongly, then what they wrote.
        faults.sort_by_key(|fault| self.stage(fault));
        let verdict = if faults.is_empty() {
            Verdict::Passed
        } else {
            Verdict::over(faults.iter().map(Fault::verdict))
        };
        Judgment { verdict, faults }
    }

    fn judge_program(&self, run: &Observed, faults: &mut Vec<Fault>) {
        if self.grade.program.is_empty() {
            return;
        }
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

    fn judge_delegations(&self, run: &Observed, faults: &mut Vec<Fault>) {
        if run.delegations.len() != self.grade.delegations {
            faults.push(Fault::DelegationCount {
                expected: self.grade.delegations,
                found: run.delegations.len(),
            });
        }
        if self.grade.expects.is_empty() {
            self.judge_reported(run, faults);
            return;
        }
        let malformed = self.malformed_expectations();
        if !malformed.is_empty() {
            // A manifest that cannot check cannot pass, whatever the run
            // did — and a hand-built task reaches `judge` without going
            // through `load`, so the check lives here too.
            faults.extend(
                malformed
                    .into_iter()
                    .map(|why| Fault::ExpectationMalformed { why }),
            );
            return;
        }
        self.judge_expected(run, faults);
    }

    /// Every way `grade.expects` fails to state a checkable expectation,
    /// empty when the manifest can check.
    ///
    /// A task that owns the answers pins one entry per delegation, so
    /// `expects.len()` is `delegations`, and every pinned answer must
    /// verify, so `delegations_correct` is the same count. Each entry
    /// needs a prompt that is not blank — it is the delegation's
    /// identity — and an answer that is not blank, because an empty
    /// answer compared to an empty output would "verify" a delegation
    /// that said nothing. Two entries asking the same prompt leave one
    /// question answering for two places.
    fn malformed_expectations(&self) -> Vec<String> {
        let expects = &self.grade.expects;
        if expects.is_empty() {
            return Vec::new();
        }
        let mut problems = Vec::new();
        if expects.len() != self.grade.delegations {
            problems.push(format!(
                "expects pins {} answers for {} delegations; it owes one per delegation",
                expects.len(),
                self.grade.delegations
            ));
        }
        if self.grade.delegations_correct != expects.len() {
            problems.push(format!(
                "delegations_correct is {}, but {} pinned answers means all {} must verify",
                self.grade.delegations_correct,
                expects.len(),
                expects.len()
            ));
        }
        for (place, want) in expects.iter().enumerate() {
            if want.prompt.trim().is_empty() {
                problems.push(format!("expects[{place}] pins no prompt"));
            } else if expects[..place]
                .iter()
                .any(|earlier| earlier.prompt == want.prompt)
            {
                problems.push(format!(
                    "expects[{place}] repeats a prompt an earlier entry pins"
                ));
            }
            if want.answer.trim().is_empty() {
                problems.push(format!("expects[{place}] pins no answer"));
            }
        }
        problems
    }

    /// Correctness as the run reported it, for a task that states no
    /// answers of its own.
    ///
    /// Only a delegation the trace itself records as checked counts: the
    /// call completed, `correct` says `true`, and there is an answer to
    /// have checked. One that recorded none of that is unverified, and
    /// the shortfall it leaves is [`Verdict::Unverifiable`] rather than a
    /// pass — nobody looked.
    fn judge_reported(&self, run: &Observed, faults: &mut Vec<Fault>) {
        let mut verified = 0usize;
        let mut unverified = 0usize;
        for delegation in &run.delegations {
            if delegation.outcome != Outcome::Completed {
                faults.push(Fault::DelegationFailed {
                    id: delegation.id.clone(),
                    outcome: outcome_word(delegation.outcome),
                });
                continue;
            }
            match delegation.correct {
                Some(false) => faults.push(Fault::DelegationWrong {
                    id: delegation.id.clone(),
                }),
                Some(true) if delegation.verified() => verified += 1,
                // Either nothing said whether it was right, or something
                // said so about an empty answer. Neither is a check.
                _ => {
                    unverified += 1;
                    faults.push(Fault::DelegationUnverified {
                        id: delegation.id.clone(),
                    });
                }
            }
        }
        if verified < self.grade.delegations_correct {
            faults.push(Fault::DelegationsCorrect {
                expected: self.grade.delegations_correct,
                verified,
                unverified,
            });
        }
    }

    /// Each delegation against the answer the task owns for its place.
    ///
    /// `grade.expects` is positional, so the run's first delegation must
    /// be the request's first question: a missing, reordered, duplicated,
    /// or substituted delegation faults rather than matching wherever it
    /// lands. The check is the manifest's, and every shortfall against it
    /// is measured — there is no unverified state when the task holds the
    /// answers itself.
    fn judge_expected(&self, run: &Observed, faults: &mut Vec<Fault>) {
        let mut verified = 0usize;
        for (place, want) in self.grade.expects.iter().enumerate() {
            let Some(delegation) = run.delegations.get(place) else {
                faults.push(Fault::DelegationMissing {
                    prompt: want.prompt.clone(),
                });
                continue;
            };
            if delegation.verified_against(want) {
                verified += 1;
                continue;
            }
            // Not the expectation's answer — name the first clause that
            // failed, in the order `verified_against` reads them.
            if delegation.outcome != Outcome::Completed {
                faults.push(Fault::DelegationFailed {
                    id: delegation.id.clone(),
                    outcome: outcome_word(delegation.outcome),
                });
            } else if delegation.prompt != want.prompt {
                faults.push(Fault::DelegationMisattributed {
                    id: delegation.id.clone(),
                    wanted: want.prompt.clone(),
                    found: delegation.prompt.clone(),
                });
            } else if delegation.correct == Some(false) {
                // The trace calls its own answer wrong: a measured claim
                // the manifest cannot confirm, whatever the output holds.
                faults.push(Fault::DelegationWrong {
                    id: delegation.id.clone(),
                });
            } else {
                faults.push(Fault::DelegationAnswered {
                    id: delegation.id.clone(),
                    wanted: want.answer.clone(),
                    found: delegation.output.clone(),
                });
            }
        }
        if verified < self.grade.delegations_correct {
            faults.push(Fault::DelegationsCorrect {
                expected: self.grade.delegations_correct,
                verified,
                unverified: 0,
            });
        }
    }

    fn judge_decisions(&self, run: &Observed, faults: &mut Vec<Fault>) {
        for name in &self.grade.decisions {
            let Some(asked) = run.decisions.get(name) else {
                faults.push(Fault::DecisionMissing { name: name.clone() });
                continue;
            };
            if asked.outcome != Outcome::Completed {
                faults.push(Fault::DecisionFailed {
                    name: name.clone(),
                    outcome: outcome_word(asked.outcome),
                });
                continue;
            }
            if empty_answers(&asked.answers) {
                faults.push(Fault::DecisionUnanswered { name: name.clone() });
            }
        }
        for expected in &self.grade.answers {
            let Some(asked) = run.decisions.get(&expected.decision) else {
                // A missing decision the task already names is already a
                // fault, and a second one adds nothing.
                if !self.grade.decisions.contains(&expected.decision) {
                    faults.push(Fault::DecisionMissing {
                        name: expected.decision.clone(),
                    });
                }
                continue;
            };
            let answer = asked.answers.get(&expected.question);
            match answer.and_then(|answer| expected.holds.holds(answer)) {
                Some(true) => {}
                Some(false) => faults.push(Fault::AnswerWrong {
                    decision: expected.decision.clone(),
                    question: expected.question.clone(),
                    wanted: expected.holds.wanted(),
                    found: said(answer.unwrap_or(&Value::Null)),
                }),
                None => faults.push(Fault::AnswerMissing {
                    decision: expected.decision.clone(),
                    question: expected.question.clone(),
                }),
            }
        }
    }

    fn judge_checks(&self, run: &Observed, faults: &mut Vec<Fault>) {
        for name in &self.grade.checks {
            let ran: Vec<&Check> = run
                .checks
                .iter()
                .filter(|check| &check.name == name)
                .collect();
            if ran.is_empty() {
                faults.push(Fault::CheckMissing { name: name.clone() });
                continue;
            }
            for check in ran {
                if check.outcome != Outcome::Completed {
                    faults.push(Fault::CheckFailed {
                        name: name.clone(),
                        outcome: outcome_word(check.outcome),
                    });
                }
            }
        }
    }

    /// Whether the steps the path names happened in the order it names
    /// them.
    ///
    /// A step is placed by where it first appears, so six delegations count
    /// once. A step the run never took is not out of order; it is missing,
    /// and the check that owns it says so.
    ///
    /// Each step is compared against the one before it rather than against
    /// the furthest step so far, so one swapped pair reports one fault
    /// about that pair instead of a fault for every step after it.
    fn judge_order(&self, run: &Observed, faults: &mut Vec<Fault>) {
        let mut previous: Option<(usize, &String)> = None;
        for step in &self.grade.path {
            let Some(at) = run.order.iter().position(|name| name == step) else {
                continue;
            };
            if let Some((was, before)) = previous
                && at < was
            {
                faults.push(Fault::OutOfOrder {
                    step: step.clone(),
                    before: before.clone(),
                });
            }
            previous = Some((at, step));
        }
    }

    fn judge_writes(&self, run: &Observed, faults: &mut Vec<Fault>) {
        let mut wrote: BTreeSet<String> = run.writes.iter().cloned().collect();
        match &run.workspace {
            Some(workspace) => wrote.extend(workspace.changed.iter().cloned()),
            None => faults.push(Fault::WritesUnobserved),
        }
        if self.grade.writes_expected == 0 {
            for path in wrote {
                faults.push(Fault::UnexpectedWrite { path });
            }
        } else if let Some(workspace) = &run.workspace {
            // A reported write can contradict a read-only task, but it cannot
            // supply positive evidence that a required change occurred.
            let observed = workspace.changed.iter().collect::<BTreeSet<_>>().len();
            if observed != self.grade.writes_expected {
                faults.push(Fault::WriteCount {
                    expected: self.grade.writes_expected,
                    found: observed,
                });
            }
        }
    }

    /// Whether the record is whole, and whether the episode ended a way the
    /// task allows.
    fn judge_record(&self, run: &Observed, faults: &mut Vec<Fault>) {
        if !run.closed {
            faults.push(Fault::Unfinished);
        }
        if run.unreadable_lines > 0 {
            faults.push(Fault::TornTrace {
                lines: run.unreadable_lines,
            });
        }
        let allowed = self.grade.allowed();
        if allowed.iter().any(|word| word == run.ending.word()) {
            return;
        }
        match &run.ending {
            Ending::Unobserved => faults.push(Fault::EndingUnobserved { allowed }),
            Ending::Closed => faults.push(Fault::EndingUnstated { allowed }),
            found => faults.push(Fault::Ended {
                found: found.word().to_string(),
                allowed,
            }),
        }
    }

    /// Where in the expected path the step a fault is about falls.
    ///
    /// A step [`Grade::path`] does not name sorts after every step it does,
    /// rather than before them, because a path that does not mention a step
    /// cannot say when it happens. A fault about the record rather than
    /// about a step sorts after both.
    fn stage(&self, fault: &Fault) -> usize {
        let last = self.grade.path.len();
        let step = match fault {
            Fault::Program { .. } => PROGRAM_CALL,
            Fault::DecisionMissing { name }
            | Fault::DecisionFailed { name, .. }
            | Fault::DecisionUnanswered { name }
            | Fault::CheckMissing { name }
            | Fault::CheckFailed { name, .. } => name,
            Fault::AnswerMissing { decision, .. } | Fault::AnswerWrong { decision, .. } => decision,
            Fault::OutOfOrder { step, .. } => step,
            Fault::DelegationCount { .. }
            | Fault::DelegationMissing { .. }
            | Fault::DelegationMisattributed { .. }
            | Fault::DelegationAnswered { .. }
            | Fault::DelegationWrong { .. }
            | Fault::DelegationFailed { .. }
            | Fault::DelegationUnverified { .. }
            | Fault::DelegationsCorrect { .. }
            | Fault::UnexpectedWrite { .. }
            | Fault::WriteCount { .. } => DELEGATE_CALL,
            Fault::WritesUnobserved
            | Fault::Ended { .. }
            | Fault::EndingUnobserved { .. }
            | Fault::EndingUnstated { .. }
            | Fault::Unfinished
            | Fault::TornTrace { .. }
            // A fault about the manifest rather than the run sorts with
            // the record faults: it is not about a place in the path.
            | Fault::ExpectationMalformed { .. } => return last + 1,
        };
        self.grade
            .path
            .iter()
            .position(|named| named == step)
            .unwrap_or(last)
    }
}

/// Whether a decision came back with nothing anybody can read.
fn empty_answers(answers: &Value) -> bool {
    match answers {
        Value::Object(map) => map.is_empty(),
        _ => true,
    }
}

/// Reads an ATIF trace back into the shape [`Task::judge`] reads.
///
/// What the trace holds is kept rather than summarized: how each call
/// ended, whether the session closed itself, how many lines did not read
/// back, and the order the calls came in. A grade that cannot see those
/// cannot tell a run that worked from a run that stopped halfway with the
/// right names in it.
///
/// # Errors
///
/// Returns an error when the trace cannot be read.
pub fn observe(path: &Path) -> Result<Observed, String> {
    let recording = atif::log::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let closed = recording.ended();
    let mut out = Observed {
        closed,
        unreadable_lines: recording.unreadable_lines,
        // A trace says the session closed itself and not what the turn
        // concluded. Only an exit code says that, and only a driver sees
        // one.
        ending: if closed {
            Ending::Closed
        } else {
            Ending::Unobserved
        },
        ..Observed::default()
    };
    for step in &recording.steps {
        let Some(call) = &step.call else { continue };
        out.order.push(call.name.clone());
        // A decision renders as a call carrying the decision-call schema in
        // `extra`, so a reader tells the two apart by that rather than by
        // the call's name.
        if call.is_decision() {
            let answers = call.extra.get("answers").cloned().unwrap_or(Value::Null);
            if call.name == PROGRAM_CALL {
                out.program = answers
                    .get("program")
                    .and_then(|a| a.get("choice"))
                    .and_then(|c| c.as_str())
                    .map(str::to_string);
            }
            out.decisions.insert(
                call.name.clone(),
                Asked {
                    answers,
                    outcome: call.outcome,
                },
            );
        } else {
            out.checks.push(Check {
                name: call.name.clone(),
                outcome: call.outcome,
            });
            if call.name == DELEGATE_CALL {
                out.delegations.push(Delegation {
                    id: call.id.clone(),
                    prompt: call
                        .arguments
                        .get("prompt")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    output: call.output.clone(),
                    milliseconds: call.milliseconds,
                    outcome: call.outcome,
                    correct: call.extra.get("correct").and_then(Value::as_bool),
                });
            }
            match call.extra.get("wrote") {
                Some(Value::String(written)) => out.writes.push(written.clone()),
                Some(Value::Array(written)) => out
                    .writes
                    .extend(written.iter().filter_map(Value::as_str).map(str::to_string)),
                _ => {}
            }
        }
    }
    out.milliseconds = recording
        .steps
        .last()
        .map(|step| step.at)
        .unwrap_or_default()
        .saturating_sub(
            recording
                .steps
                .first()
                .map(|step| step.at)
                .unwrap_or_default(),
        );
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

/// The workspace's own capability registry, which is where a manifest is
/// read from when the checkout being measured predates one.
#[must_use]
pub fn capabilities_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("capabilities")
}

/// Reads a task by identifier or by path.
///
/// A bare identifier names a task in [`tasks_dir`]. Anything holding a
/// separator or ending in `.json` is read as a path, so a manifest kept
/// beside a scratch experiment runs the same way a shipped one does.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read or does not parse.
pub fn load_task(name: &str) -> Result<Task, String> {
    let manifest = if name.ends_with(".json") || name.contains(std::path::MAIN_SEPARATOR) {
        PathBuf::from(name)
    } else {
        tasks_dir().join(name).join("task.json")
    };
    Task::load(&manifest)
}
