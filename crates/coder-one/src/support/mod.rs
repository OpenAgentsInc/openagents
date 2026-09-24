//! `verify.support`: paired Jev judgments per requirement, and the
//! requirement states they establish.
//!
//! The broad closing check asks one question, "is the task done?", over
//! the executor's report and a change summary. On the v3 Luna trials a 0.5
//! cutoff on it accepted all five failures. This component asks two
//! questions per requirement instead, over the requirement, an excerpt of
//! the artifact that should meet it, and what `verify.checks` observed:
//!
//! - **supports**: does the evidence show the requirement met?
//! - **contradicts**: does the evidence show it not met?
//!
//! The two aren't complementary. Both low means the evidence is
//! insufficient; both high means it conflicts, or the requirement is
//! compound. Each requirement's state records what was observed, what it
//! establishes, the checker that established it, and the candidate
//! revision it holds for. A refusal, a missing answer, or a clipped
//! artifact leaves the requirement unresolved, and a state recorded for
//! one revision says nothing about another.
//!
//! The cutoffs in [`Params::default`] are fitted on the development
//! fixtures only; [`fit`] refits them and reports false accepts and false
//! rejects apart, against the broad "done" judgment.

pub mod cli;
pub mod fit;
pub mod fixtures;

use std::path::Path;

use jev::{Noul, NoulCriteria, Questions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::checks::{self, Candidate, TaskText};
use crate::component::jev::{Ask, Asked, JevMode, ask};
use crate::record::{Finish, Implementation, Outcome, Recorder, Start};

/// The schema of a support report.
pub const SCHEMA: &str = "openagents.coder-one.support.v1";

/// Where a run keeps its support report, relative to its directory.
pub const FILE: &str = "verification/support.json";

/// The supports question.
pub const SUPPORTS: &str = "Does the evidence in `artifact` and `observations` show that the candidate meets the requirement in `requirement.text`, read in the context of the task in `task`? Count only what the evidence shows the candidate doing: an observation of the required behavior, or code that plainly performs it. A claim, a plausible design, or a file that merely exists is not enough.";

/// The contradicts question.
pub const CONTRADICTS: &str = "Does anything in `observations` or `artifact` show that the candidate does not meet the requirement in `requirement.text`, read in the context of the task in `task`? An observed result that differs from the relation the requirement implies, or code that plainly does something else, counts. Missing evidence alone does not.";

fn supports_criteria() -> NoulCriteria {
    NoulCriteria::new()
        .when_true("The observations or the artifact show the requirement being met.")
        .when_false("The evidence does not show the requirement being met, whether because it shows the opposite or because it says too little.")
}

fn contradicts_criteria() -> NoulCriteria {
    NoulCriteria::new()
        .when_true("An observation or the artifact shows the requirement not being met.")
        .when_false("Nothing in the evidence shows the requirement not being met.")
}

/// Which requirements a support run judges first when it can't judge all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Order {
    /// A contradicted requirement, then those a scenario observed, then
    /// unobserved behaviors and deliverables.
    #[default]
    ScenarioFirst,
    /// A contradicted requirement, then behaviors and checks, then
    /// deliverables, then observed constraints: what the result does comes
    /// before whether a file exists. Within each, a requirement the
    /// extraction read as binding comes first.
    BehaviorFirst,
}

impl Order {
    /// Whether this is the default order.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Order::default()
    }

    /// The order's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Order::ScenarioFirst => "scenario-first",
            Order::BehaviorFirst => "behavior-first",
        }
    }
}

/// What the judge reads and how it decides.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Params {
    /// The supports probability at or above which the evidence supports
    /// the requirement.
    pub supports: f64,
    /// The contradicts probability at or above which the evidence
    /// contradicts it.
    pub contradicts: f64,
    /// The most requirements one run judges; one request each.
    pub max_requirements: usize,
    /// The most artifact characters one request carries.
    pub artifact_chars: usize,
    /// The most characters of scenario observations one request carries.
    pub observation_chars: usize,
    /// Which requirements come first under the budget.
    #[serde(default, skip_serializing_if = "Order::is_default")]
    pub order: Order,
}

impl Default for Params {
    /// The cutoffs [`fit::fit`] chose on the development fixtures with
    /// recorded Jev answers; a test refits them and checks they match.
    fn default() -> Self {
        Params {
            supports: fit::FITTED.0,
            contradicts: fit::FITTED.1,
            max_requirements: 3,
            artifact_chars: 6_000,
            observation_chars: 2_500,
            order: Order::ScenarioFirst,
        }
    }
}

/// The implementation: both questions and every parameter.
#[must_use]
pub fn implementation(params: Params) -> Implementation {
    Implementation::new(
        "verify.support",
        "paired supports and contradicts nouls per requirement",
        &json!({
            "version": 1,
            "supports": SUPPORTS,
            "contradicts": CONTRADICTS,
            "params": params,
            "fitted_on": "development fixtures only",
        }),
    )
}

/// The requirement a judgment is about.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Req {
    pub id: String,
    pub text: String,
    pub kind: String,
}

/// An excerpt of one artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Excerpt {
    /// The file's path, or `program[i]` for an inline program.
    pub path: String,
    pub text: String,
    /// The artifact's whole length in characters.
    pub chars: usize,
    pub clipped: bool,
}

/// What one scenario observed, as it bears on one requirement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub scenario: String,
    /// The scenario's verdict for this requirement.
    pub verdict: String,
    pub expected: String,
    /// The observations, with scratch paths replaced by `<scratch>`.
    pub observed: Value,
    /// What the verdict doesn't establish.
    pub limits: Vec<String>,
}

/// Everything one requirement's judgment reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub requirement: Req,
    pub artifact: Vec<Excerpt>,
    pub observations: Vec<Observation>,
    /// The requirement's state from `verify.checks`.
    pub scenario_state: String,
    /// Whether the observations were cut to fit.
    #[serde(default)]
    pub observations_clipped: bool,
}

impl Evidence {
    /// Whether any artifact excerpt was clipped.
    #[must_use]
    pub fn clipped(&self) -> bool {
        self.artifact.iter().any(|excerpt| excerpt.clipped)
    }

    /// The evidence's digest.
    #[must_use]
    pub fn digest(&self) -> String {
        atif::digest(&serde_json::to_value(self).unwrap_or(Value::Null))
    }
}

/// Both answers, and how they were produced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Judgment {
    pub supports: Option<f64>,
    pub contradicts: Option<f64>,
    /// `live`, `recorded`, `miss`, `off`, `failed`, or `skipped`.
    pub how: String,
    /// The recorded-answer key.
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
}

impl Judgment {
    fn from(asked: &Asked) -> Self {
        Judgment {
            supports: asked.noul("supports"),
            contradicts: asked.noul("contradicts"),
            how: asked.how.to_string(),
            key: asked.key.clone(),
            error: asked.error.clone(),
            input_tokens: asked.input_tokens,
        }
    }
}

/// One requirement's state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub id: String,
    pub text: String,
    /// `supported`, `contradicted`, or `unresolved`.
    pub state: String,
    /// Why, in one sentence.
    pub why: String,
    /// What was observed: each scenario's verdict and each artifact read.
    pub observed: Vec<String>,
    /// What that establishes, in one sentence.
    pub establishes: String,
    /// The checker: the scenarios, both implementations, and the model.
    pub checker: Value,
    /// The candidate revision the state holds for.
    pub candidate: String,
    pub evidence_digest: String,
    /// The requirement's state from `verify.checks`.
    pub scenario_state: String,
    pub judgment: Judgment,
    /// The evidence both judgments read, kept with them.
    pub evidence: Evidence,
}

impl State {
    /// Whether the state still describes `candidate`: a check of one
    /// revision says nothing about another.
    #[must_use]
    pub fn fresh_for(&self, candidate: &str) -> bool {
        self.candidate == candidate
    }
}

/// A requirement the run didn't judge, and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skipped {
    pub id: String,
    pub why: String,
}

/// A whole support run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub implementation: Implementation,
    pub params: Params,
    pub candidate: String,
    pub states: Vec<State>,
    pub skipped: Vec<Skipped>,
}

impl Report {
    /// Counts by state, for a manifest or a list.
    #[must_use]
    pub fn summary(&self) -> Value {
        let count = |word: &str| self.states.iter().filter(|s| s.state == word).count();
        json!({
            "candidate": self.candidate,
            "judged": self.states.len(),
            "supported": count("supported"),
            "contradicted": count("contradicted"),
            "unresolved": count("unresolved"),
            "skipped": self.skipped.len(),
        })
    }

    /// The requirements whose state no longer describes `candidate`.
    #[must_use]
    pub fn stale_for(&self, candidate: &str) -> Vec<String> {
        self.states
            .iter()
            .filter(|state| !state.fresh_for(candidate))
            .map(|state| state.id.clone())
            .collect()
    }
}

/// Replaces scratch directories in `text` with `<scratch>`: a path under
/// the temporary directory whose component starts with `coder-one-`, or
/// any path through a `checks-scratch` directory. Keeps the evidence, and
/// the recorded-answer key made from it, the same from run to run.
#[must_use]
pub fn scrub(text: &str) -> String {
    let tmp = std::env::temp_dir()
        .to_string_lossy()
        .trim_end_matches('/')
        .to_string();
    let prefix = format!("{tmp}/");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let temp = rest.match_indices(&prefix).find_map(|(at, _)| {
            // A temporary directory may already end in a separator when
            // a logged path appends another one.
            let tail = rest[at + prefix.len()..].trim_start_matches('/');
            tail.starts_with("coder-one-")
                .then_some((at, rest.len() - tail.len()))
        });
        let checks = rest.find("/checks-scratch").map(|at| {
            let start = rest[..at]
                .rfind(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '(')
                .map_or(0, |i| i + 1);
            (start, at + 1)
        });
        let found = match (temp, checks) {
            (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
            (a, b) => a.or(b),
        };
        let Some((start, component)) = found else {
            out.push_str(rest);
            return out;
        };
        let end = rest[component..]
            .find(|c: char| c == '/' || c.is_whitespace() || c == '"' || c == '\'')
            .map_or(rest.len(), |i| component + i);
        out.push_str(&rest[..start]);
        out.push_str("<scratch>");
        rest = &rest[end..];
    }
}

fn scrub_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(scrub(text)),
        Value::Array(items) => Value::Array(items.iter().map(scrub_value).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), scrub_value(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn clip_chars(text: &str, max: usize) -> (String, bool) {
    match text.char_indices().nth(max) {
        Some((cut, _)) => (text[..cut].to_string(), true),
        None => (text.to_string(), false),
    }
}

fn stem(path: &str) -> &str {
    let base = checks::base_name(path);
    base.rsplit_once('.').map_or(base, |(stem, _)| stem)
}

/// The artifact excerpts for a requirement: the candidate's files that
/// the requirement or a scenario's interface names, or every file when
/// none is named, or its inline programs when it wrote no file. The
/// budget is split evenly, and an excerpt past its share is clipped.
#[must_use]
pub fn excerpts(
    candidate: &Candidate,
    requirement: &str,
    interfaces: &[&str],
    budget: usize,
) -> Vec<Excerpt> {
    let named: Vec<(&String, &String)> = candidate
        .files
        .iter()
        .filter(|(path, _)| {
            let stem = stem(path);
            stem.len() > 2
                && (requirement.contains(checks::base_name(path))
                    || interfaces.iter().any(|i| i.contains(stem)))
        })
        .collect();
    let sources: Vec<(String, &String)> = if !named.is_empty() {
        named.into_iter().map(|(p, t)| (p.clone(), t)).collect()
    } else if !candidate.files.is_empty() {
        candidate
            .files
            .iter()
            .map(|(p, t)| (p.clone(), t))
            .collect()
    } else {
        candidate
            .programs
            .iter()
            .enumerate()
            .map(|(i, p)| (format!("program[{i}] ({})", p.interpreter), &p.source))
            .collect()
    };
    let share = budget / sources.len().max(1);
    sources
        .into_iter()
        .map(|(path, text)| {
            let (excerpt, clipped) = clip_chars(text, share);
            Excerpt {
                path,
                chars: text.chars().count(),
                text: excerpt,
                clipped,
            }
        })
        .collect()
}

/// Where a requirement falls under [`Order::ScenarioFirst`]: one a
/// scenario contradicted first, then those a scenario observed (behaviors
/// and deliverables first), then behaviors and deliverables no scenario
/// observed; `None` leaves it out.
fn scenario_rank(covered: &checks::Covered) -> Option<u8> {
    let primary = matches!(covered.kind.as_str(), "behavior" | "deliverable");
    match (
        covered.state.as_str(),
        covered.scenarios.is_empty(),
        primary,
    ) {
        ("contradicted", _, _) => Some(0),
        (_, false, true) => Some(1),
        (_, false, false) => Some(2),
        (_, true, true) => Some(3),
        _ => None,
    }
}

/// Where a requirement falls under [`Order::BehaviorFirst`]: contradicted,
/// then behaviors and checks, then deliverables, then constraints a
/// scenario observed; each split by whether the extraction read it as
/// binding. Constraints and context no scenario observed are left out.
fn behavior_rank(covered: &checks::Covered, binding: bool) -> Option<u8> {
    let tier = match (
        covered.state.as_str(),
        covered.kind.as_str(),
        covered.scenarios.is_empty(),
    ) {
        ("contradicted", _, _) => 0,
        (_, "behavior" | "check", _) => 1,
        (_, "deliverable", _) => 2,
        (_, _, false) => 3,
        _ => return None,
    };
    Some(tier * 2 + u8::from(!binding))
}

/// The evidence for each requirement worth judging, and the ones left
/// out, in the order `params.order` names. At most
/// `params.max_requirements`.
#[must_use]
pub fn evidence(
    candidate: &Candidate,
    report: &checks::Report,
    params: Params,
) -> (Vec<Evidence>, Vec<Skipped>) {
    evidence_with(candidate, report, params, &[])
}

/// [`evidence`], with the requirement IDs the extraction was unsure bind,
/// which [`Order::BehaviorFirst`] judges after the binding ones.
#[must_use]
pub fn evidence_with(
    candidate: &Candidate,
    report: &checks::Report,
    params: Params,
    uncertain: &[String],
) -> (Vec<Evidence>, Vec<Skipped>) {
    let rank = |covered: &checks::Covered| -> Option<u8> {
        match params.order {
            Order::ScenarioFirst => scenario_rank(covered),
            Order::BehaviorFirst => behavior_rank(covered, !uncertain.contains(&covered.id)),
        }
    };
    let mut ranked: Vec<(u8, usize, &checks::Covered)> = report
        .coverage
        .iter()
        .enumerate()
        .filter_map(|(i, c)| rank(c).map(|r| (r, i, c)))
        .collect();
    ranked.sort_by_key(|(r, i, _)| (*r, *i));
    let mut chosen = Vec::new();
    let mut skipped = Vec::new();
    for (_, _, covered) in ranked {
        if chosen.len() >= params.max_requirements {
            skipped.push(Skipped {
                id: covered.id.clone(),
                why: format!(
                    "over the budget of {} requirements per run",
                    params.max_requirements
                ),
            });
            continue;
        }
        chosen.push(covered);
    }
    for covered in &report.coverage {
        if !chosen.iter().any(|c| c.id == covered.id) && !skipped.iter().any(|s| s.id == covered.id)
        {
            skipped.push(Skipped {
                id: covered.id.clone(),
                why: "a constraint or context no scenario observed".to_string(),
            });
        }
    }
    let evidence = chosen
        .into_iter()
        .map(|covered| {
            let scenarios: Vec<&checks::Scenario> = report
                .scenarios
                .iter()
                .filter(|s| s.requirements.contains(&covered.id))
                .collect();
            let interfaces: Vec<&str> = scenarios.iter().map(|s| s.interface.as_str()).collect();
            let mut used = 0;
            let mut observations_clipped = false;
            let observations = scenarios
                .iter()
                .filter_map(|scenario| {
                    let verdict = report.verdicts.iter().find(|v| v.scenario == scenario.id)?;
                    let observed = scrub_value(&json!(verdict.observations));
                    let text = observed.to_string();
                    let room = params.observation_chars.saturating_sub(used);
                    let observed = if text.chars().count() > room {
                        observations_clipped = true;
                        let (cut, _) = clip_chars(&text, room);
                        used += room;
                        json!(format!("{cut}…"))
                    } else {
                        used += text.chars().count();
                        observed
                    };
                    Some(Observation {
                        scenario: scenario.id.clone(),
                        verdict: verdict.for_requirement(scenario, &covered.id).to_string(),
                        expected: scenario.expected.statement.clone(),
                        observed,
                        limits: verdict.coverage.iter().map(|l| scrub(l)).collect(),
                    })
                })
                .collect();
            Evidence {
                requirement: Req {
                    id: covered.id.clone(),
                    text: covered.text.clone(),
                    kind: covered.kind.clone(),
                },
                artifact: excerpts(candidate, &covered.text, &interfaces, params.artifact_chars),
                observations,
                scenario_state: covered.state.clone(),
                observations_clipped,
            }
        })
        .collect();
    (evidence, skipped)
}

/// The instruction a request carries: at most this many characters.
const TASK_CHARS: usize = 4_000;

/// One requirement's request: the task, the requirement, the artifact,
/// and the observations, with both questions.
#[must_use]
pub fn request(task: &TaskText, evidence: &Evidence) -> (Value, Questions) {
    let observations = if evidence.observations.is_empty() {
        json!("No scenario observed this requirement.")
    } else {
        json!(
            evidence
                .observations
                .iter()
                .map(|o| json!({
                    "scenario": o.scenario,
                    "expected": o.expected,
                    "verdict": o.verdict,
                    "observed": o.observed,
                    "limits": o.limits,
                }))
                .collect::<Vec<_>>()
        )
    };
    let state = json!({
        "task": { "title": task.title, "instruction": clip_chars(&task.instruction, TASK_CHARS).0 },
        "requirement": { "id": evidence.requirement.id, "text": evidence.requirement.text, "kind": evidence.requirement.kind },
        "artifact": evidence.artifact.iter().map(|e| json!({
            "path": e.path,
            "text": e.text,
            "complete": !e.clipped,
        })).collect::<Vec<_>>(),
        "observations": observations,
    });
    let questions = Questions::new()
        .with(
            "supports",
            Noul::with_criteria(SUPPORTS, supports_criteria()),
        )
        .with(
            "contradicts",
            Noul::with_criteria(CONTRADICTS, contradicts_criteria()),
        );
    (state, questions)
}

/// What a judgment establishes under `params`: the state word and why.
#[must_use]
pub fn establish(judgment: &Judgment, evidence: &Evidence, params: Params) -> (String, String) {
    let (Some(s), Some(c)) = (judgment.supports, judgment.contradicts) else {
        return (
            "unresolved".to_string(),
            format!(
                "Jev gave no answer ({}{})",
                judgment.how,
                judgment
                    .error
                    .as_deref()
                    .map_or(String::new(), |e| format!(": {e}"))
            ),
        );
    };
    if evidence.clipped() {
        return (
            "unresolved".to_string(),
            "the artifact excerpt was clipped, so the judgment didn't read the whole source"
                .to_string(),
        );
    }
    let supports = s >= params.supports;
    let contradicts = c >= params.contradicts;
    let (word, why) = match (supports, contradicts) {
        (true, false) => (
            "supported",
            "the evidence supports it and doesn't contradict it",
        ),
        (false, true) => ("contradicted", "the evidence contradicts it"),
        (true, true) => (
            "unresolved",
            "the evidence both supports and contradicts it: conflicting evidence or a compound requirement",
        ),
        (false, false) => (
            "unresolved",
            "the evidence neither supports nor contradicts it: insufficient evidence",
        ),
    };
    (word.to_string(), why.to_string())
}

/// A state from a judgment of `evidence` against `candidate`.
#[must_use]
pub fn state_of(evidence: Evidence, judgment: Judgment, candidate: &str, params: Params) -> State {
    let (word, why) = establish(&judgment, &evidence, params);
    let mut observed: Vec<String> = evidence
        .observations
        .iter()
        .map(|o| format!("scenario {} {}", o.scenario, o.verdict))
        .collect();
    if observed.is_empty() {
        observed.push("no scenario observed it".to_string());
    }
    observed.extend(evidence.artifact.iter().map(|e| {
        format!(
            "read {} ({} of {} characters)",
            e.path,
            e.text.chars().count(),
            e.chars
        )
    }));
    let establishes = match word.as_str() {
        "supported" => format!(
            "{} is met by candidate {}",
            evidence.requirement.id,
            short(candidate)
        ),
        "contradicted" => format!(
            "{} is not met by candidate {}",
            evidence.requirement.id,
            short(candidate)
        ),
        _ => format!(
            "nothing yet about {} for candidate {}",
            evidence.requirement.id,
            short(candidate)
        ),
    };
    State {
        id: evidence.requirement.id.clone(),
        text: evidence.requirement.text.clone(),
        state: word,
        why,
        observed,
        establishes,
        checker: json!({
            "scenarios": evidence.observations.iter().map(|o| o.scenario.clone()).collect::<Vec<_>>(),
            "checks": checks::implementation().digest,
            "support": implementation(params).digest,
            "model": crate::credentials::JEV_MODEL,
        }),
        candidate: candidate.to_string(),
        evidence_digest: evidence.digest(),
        scenario_state: evidence.scenario_state.clone(),
        judgment,
        evidence,
    }
}

fn short(digest: &str) -> &str {
    &digest[..digest.len().min(12)]
}

/// Asks both questions for each piece of evidence under one
/// `verify.support` invocation and returns the report.
#[allow(clippy::too_many_arguments)]
pub async fn judge_evidence(
    task: &TaskText,
    candidate: &str,
    evidence: Vec<Evidence>,
    skipped: Vec<Skipped>,
    jev: &JevMode,
    recorder: &Recorder,
    params: Params,
    deadline: Option<crate::deadline::Deadline>,
) -> Report {
    let parent = recorder.enter(
        Start::new("verify.support", implementation(params))
            .named(candidate.get(..12).unwrap_or(candidate))
            .reading(&json!({ "candidate": candidate, "evidence": evidence.iter().map(Evidence::digest).collect::<Vec<_>>() })),
    );
    let mut states = Vec::new();
    for (i, evidence) in evidence.into_iter().enumerate() {
        let (state, questions) = request(task, &evidence);
        let asked = ask(
            jev,
            recorder,
            Ask {
                component: "verify.support",
                name: "jev_support",
                id: format!("jev-support-{}", i + 1),
                state,
                questions,
                parent: Some(parent.clone()),
                deadline: deadline.clone(),
            },
        )
        .await;
        states.push(state_of(
            evidence,
            Judgment::from(&asked),
            candidate,
            params,
        ));
    }
    let report = Report {
        schema: SCHEMA.to_string(),
        implementation: implementation(params),
        params,
        candidate: candidate.to_string(),
        states,
        skipped,
    };
    recorder.end(
        &parent,
        Finish::new(Outcome::Completed).output(json!({
            "summary": report.summary(),
            "states": report.states.iter().map(|s| json!({
                "id": s.id,
                "state": s.state,
                "supports": s.judgment.supports,
                "contradicts": s.judgment.contradicts,
                "scenario_state": s.scenario_state,
            })).collect::<Vec<_>>(),
        })),
    );
    report
}

/// Judges the requirements a check report covers, against its candidate.
pub async fn judge(
    input: &checks::Input,
    report: &checks::Report,
    jev: &JevMode,
    recorder: &Recorder,
    params: Params,
    deadline: Option<crate::deadline::Deadline>,
) -> Report {
    let uncertain: Vec<String> = input
        .requirements
        .as_ref()
        .map(|map| {
            map.requirements
                .iter()
                .filter(|r| r.binding == crate::requirements::Binding::Uncertain)
                .map(|r| r.id.clone())
                .collect()
        })
        .unwrap_or_default();
    let (evidence, skipped) = evidence_with(&input.candidate, report, params, &uncertain);
    judge_evidence(
        &input.task,
        &input.candidate.digest(),
        evidence,
        skipped,
        jev,
        recorder,
        params,
        deadline,
    )
    .await
}

/// Writes `report` to `<dir>/verification/support.json`.
///
/// # Errors
///
/// Returns a message when it can't be written.
pub fn save(report: &Report, dir: &Path) -> Result<(), String> {
    save_as(report, dir, FILE)
}

/// Writes `report` to `<dir>/<file>`.
///
/// # Errors
///
/// Returns a message when it can't be written.
pub fn save_as(report: &Report, dir: &Path, file: &str) -> Result<(), String> {
    let text = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
    crate::record::write_atomic(&dir.join(file), text.as_bytes())
}

#[cfg(test)]
mod tests;
