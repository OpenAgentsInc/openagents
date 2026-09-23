//! `control.monitor`: typed judgments about a running executor session.
//!
//! The monitor watches the normalized events the host loop observes
//! ([`crate::session`]) beside the task's open requirements, and answers
//! four questions at each trigger:
//!
//! - Is the agent making progress on an open requirement?
//! - Is it repeating an approach that already failed?
//! - Is it re-reading evidence the briefing already holds?
//! - Does it claim the task is done?
//!
//! A trigger is a completed operation, an artifact change, an assistant
//! claim, or a long silence, never every event. Each trigger answers the
//! questions twice: by deterministic rules over the prefix, at no cost, and,
//! when Jev is on, by one Jev request whose state is the recent events plus
//! the open requirements, not the whole transcript, so input stays bounded
//! however long the session runs.
//!
//! In shadow mode, the default, a judgment becomes a versioned
//! [`Submission`] the controller records and never acts on. A Jev answer
//! arrives after its latency; the judgment names the version it was asked
//! at, and it is **stale** when the workspace revision or the process
//! generation moved before the answer arrived.
//!
//! [`replay`] feeds a retained native stream through the same monitor one
//! event at a time, so a judgment sees only the prefix available at its
//! trigger, and [`hindsight`] labels each judgment from the rest of the
//! stream and the attempt's outcome. [`Score`] turns labeled judgments into
//! trigger precision per question, for the rules alone and for Jev.

use std::collections::{BTreeMap, VecDeque};

use atif::document::{Source, Step};
use futures_util::future::LocalBoxFuture;
use jev::{Noul, Questions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::component::jev::{self as jevmode, Ask, JevMode, USD_PER_MILLION_INPUT};
use crate::record::{Implementation, Recorder};
use crate::session::{
    Controller, Intent, Observation, Phase, Proposal, Submission, Version, Watch,
};
use crate::stream::{Event, Kind, unwrap_shell};

/// The component's ID.
pub const COMPONENT: &str = "control.monitor";

/// The step extension that holds one judgment.
pub const JUDGMENT_KEY: &str = "monitor_judgment";

/// The schema of a judgment record.
pub const JUDGMENT_SCHEMA: &str = "openagents.coder-one.monitor-judgment.v1";

/// The question set's revision. A change to any question's wording is a
/// new revision, and misses every recorded answer.
pub const QUESTION_SET: &str = "monitor-v1";

/// The Jev decision's name.
pub const DECISION: &str = "jev_monitor";

const Q_PROGRESS: &str = "Over the recent events, is the agent making progress on at least one open requirement, such as changing a file a requirement names or moving a failing check closer to passing?";
const Q_REPEATING: &str = "Is the agent repeating an approach that already failed, such as rerunning a command that failed without changing anything, or rewriting a file the same way?";
const Q_REREADING: &str = "Is the agent re-reading evidence the briefing already holds, such as listing a directory or printing a file whose content the briefing shows?";
const Q_CLAIMS_DONE: &str = "Does the agent's latest message claim the task is done?";

/// Words a completion claim uses, for the rule.
const DONE_WORDS: &[&str] = &[
    "done",
    "complete",
    "completed",
    "finished",
    "implemented",
    "all tests pass",
    "tests pass",
    "passes",
    "is ready",
    "fixed",
    "resolved",
];

/// Commands that print or list what they name.
const READERS: &[&str] = &[
    "cat", "head", "tail", "sed", "nl", "less", "more", "ls", "find", "tree", "stat", "wc", "file",
];

fn yes() -> bool {
    true
}
fn silence() -> Option<u64> {
    Some(120_000)
}
fn window() -> usize {
    8
}
fn event_chars() -> usize {
    300
}
fn requirements_max() -> usize {
    10
}
fn threshold() -> f64 {
    0.5
}
fn stall_commands() -> usize {
    4
}
fn latency() -> u64 {
    800
}

/// The monitor's tunable parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// Trigger at a completed command.
    #[serde(default = "yes")]
    pub on_operation: bool,
    /// Trigger at an artifact change.
    #[serde(default = "yes")]
    pub on_artifact: bool,
    /// Trigger at an assistant claim.
    #[serde(default = "yes")]
    pub on_claim: bool,
    /// Milliseconds with no event that make a long silence; `null` never
    /// triggers on silence.
    #[serde(default = "silence")]
    pub silence_ms: Option<u64>,
    /// How many recent events a Jev request carries.
    #[serde(default = "window")]
    pub window: usize,
    /// The most characters one event carries.
    #[serde(default = "event_chars")]
    pub event_chars: usize,
    /// The most open requirements a request carries.
    #[serde(default = "requirements_max")]
    pub requirements: usize,
    /// A Jev probability at or above this flags a question.
    #[serde(default = "threshold")]
    pub threshold: f64,
    /// Whether Jev is asked at each trigger. Off, the rules decide alone.
    #[serde(default = "yes")]
    pub jev: bool,
    /// The rule's stall: this many completed commands since the last
    /// artifact change.
    #[serde(default = "stall_commands")]
    pub stall_commands: usize,
    /// The latency to assume for a Jev answer that recorded none, in
    /// milliseconds, so staleness is still measured.
    #[serde(default = "latency")]
    pub latency_ms: u64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            on_operation: true,
            on_artifact: true,
            on_claim: true,
            silence_ms: silence(),
            window: window(),
            event_chars: event_chars(),
            requirements: requirements_max(),
            threshold: threshold(),
            jev: true,
            stall_commands: stall_commands(),
            latency_ms: latency(),
        }
    }
}

/// The implementation record for a parameter set.
#[must_use]
pub fn implementation(params: &Params) -> Implementation {
    Implementation::new(
        COMPONENT,
        "trigger rules and Jev over recent events and open requirements",
        &json!({ "version": 1, "questions": QUESTION_SET, "params": params }),
    )
}

/// The four answers, as flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flags {
    /// Not making progress on an open requirement.
    pub stalled: bool,
    pub repeating: bool,
    pub rereading: bool,
    pub claims_done: bool,
}

impl Flags {
    /// The flag names, in report order.
    pub const NAMES: [&'static str; 4] = ["stalled", "repeating", "rereading", "claims_done"];

    /// The flag called `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> bool {
        match name {
            "stalled" => self.stalled,
            "repeating" => self.repeating,
            "rereading" => self.rereading,
            "claims_done" => self.claims_done,
            "intervene" => self.intervene(),
            _ => false,
        }
    }

    /// Whether a flag asks the host to intervene: a stall, a loop, or
    /// re-reading.
    #[must_use]
    pub fn intervene(&self) -> bool {
        self.stalled || self.repeating || self.rereading
    }

    fn set(&mut self, name: &str, value: bool) {
        match name {
            "stalled" => self.stalled = value,
            "repeating" => self.repeating = value,
            "rereading" => self.rereading = value,
            "claims_done" => self.claims_done = value,
            _ => {}
        }
    }
}

/// Jev's four probabilities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Answers {
    pub progress: Option<f64>,
    pub repeating: Option<f64>,
    pub rereading: Option<f64>,
    pub claims_done: Option<f64>,
}

impl Answers {
    /// The answers as flags at `threshold`; an unknown answer is no flag.
    #[must_use]
    pub fn flags(&self, threshold: f64) -> Flags {
        let at = |p: Option<f64>| p.is_some_and(|p| p >= threshold);
        Flags {
            stalled: self.progress.is_some_and(|p| p < threshold),
            repeating: at(self.repeating),
            rereading: at(self.rereading),
            claims_done: at(self.claims_done),
        }
    }
}

/// One open requirement, as a request carries it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Open {
    pub id: String,
    pub text: String,
}

/// What the monitor knows before the session starts: the task, its open
/// requirements, and what the briefing already holds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Context {
    pub task: String,
    pub requirements: Vec<Open>,
    /// The paths and commands whose output the briefing carries.
    pub briefed: Vec<String>,
}

impl Context {
    /// The context of a task and its briefing: requirements by rule from
    /// the task's words, and the briefing's evidence headings.
    #[must_use]
    pub fn new(task: &str, briefing: &str) -> Self {
        let map = crate::requirements::mechanical(task);
        Context {
            task: task.to_string(),
            requirements: map
                .requirements
                .iter()
                .map(|r| Open {
                    id: r.id.clone(),
                    text: r.text.split_whitespace().collect::<Vec<_>>().join(" "),
                })
                .collect(),
            briefed: briefed_items(briefing),
        }
    }
}

/// The paths and commands a briefing's evidence headings name: `### path
/// (…)`, `### $ command (…)`, and `### Step N: \`command\` (…)`.
#[must_use]
pub fn briefed_items(briefing: &str) -> Vec<String> {
    let mut items = Vec::new();
    for line in briefing.lines() {
        let Some(heading) = line.strip_prefix("### ") else {
            continue;
        };
        let label = heading.rfind(" (").map_or(heading, |at| &heading[..at]);
        let label = if let Some(rest) = label.strip_prefix("Step ") {
            rest.split('`').nth(1).unwrap_or_default()
        } else {
            label
        };
        let label = label.trim().trim_start_matches("$ ").trim();
        if !label.is_empty() && !items.iter().any(|item| item == label) {
            items.push(label.to_string());
        }
    }
    items
}

fn clip(text: &str, max: usize) -> String {
    crate::judge::clip(&text.split_whitespace().collect::<Vec<_>>().join(" "), max)
}

/// A path as the task sees it: without `/app/`, `./`, or quotes.
fn bare(path: &str) -> String {
    let path = path.trim_matches(|c| c == '\'' || c == '"');
    let path = path.strip_prefix("/app/").unwrap_or(path);
    let path = path.strip_prefix("./").unwrap_or(path);
    path.trim_end_matches('/').to_string()
}

/// Whether `command` reads evidence `briefed` already holds: it is a
/// briefed command, or a reader whose argument is a briefed path, or a
/// listing when the briefing holds one.
#[must_use]
pub fn reads_briefed(command: &str, briefed: &[String]) -> bool {
    let script = unwrap_shell(command);
    let normal = script.split_whitespace().collect::<Vec<_>>().join(" ");
    if normal.is_empty() {
        return false;
    }
    if briefed
        .iter()
        .any(|item| item.split_whitespace().collect::<Vec<_>>().join(" ") == normal)
    {
        return true;
    }
    // Each simple command of a pipeline or a list.
    for part in normal.split(['|', ';', '&']) {
        let words: Vec<&str> = part.split_whitespace().collect();
        let Some(first) = words.first() else { continue };
        let program = first.rsplit('/').next().unwrap_or(first);
        if !READERS.contains(&program) {
            continue;
        }
        if matches!(program, "ls" | "find" | "tree")
            && briefed.iter().any(|item| {
                item.contains("ls ") || item.starts_with("find ") || item.contains("tree")
            })
            && words[1..]
                .iter()
                .filter(|w| !w.starts_with('-'))
                .all(|w| matches!(bare(w).as_str(), "" | "." | "/app"))
        {
            return true;
        }
        let briefed_paths: Vec<String> = briefed
            .iter()
            .filter(|item| !item.contains(' '))
            .map(|item| bare(item))
            .collect();
        if words[1..]
            .iter()
            .filter(|w| !w.starts_with('-'))
            .any(|w| briefed_paths.contains(&bare(w)))
        {
            return true;
        }
    }
    false
}

/// Whether a claim reads as saying the work is done.
#[must_use]
pub fn claims_done(text: &str) -> bool {
    let lower = text.to_lowercase();
    DONE_WORDS.iter().any(|word| {
        lower.match_indices(word).any(|(at, _)| {
            let before = lower[..at].chars().next_back();
            let after = lower[at + word.len()..].chars().next();
            !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
        })
    })
}

/// A command's identity for the repetition rule: its first three words,
/// unwrapped from `bash -lc`.
#[must_use]
pub fn command_key(command: &str) -> String {
    unwrap_shell(command)
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The prefix of a session the monitor has seen, and what the rules keep
/// about it.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    /// Every observation so far.
    pub history: Vec<Observation>,
    /// Command text by observation index, for completed commands whose
    /// stream names them by tool-call ID (Claude Code).
    commands: BTreeMap<usize, String>,
    pending_starts: VecDeque<String>,
    failures_since_artifact: BTreeMap<String, usize>,
    commands_since_artifact: usize,
    last_failed: Option<String>,
    /// The history's length at the last judgment.
    last_look: usize,
}

impl Tracker {
    /// Adds one observation.
    pub fn push(&mut self, observation: &Observation) {
        let index = self.history.len();
        match &observation.event.kind {
            Kind::CommandStarted { command } => {
                self.pending_starts.push_back(command.clone());
                self.commands.insert(index, command.clone());
            }
            Kind::CommandCompleted {
                command, exit_code, ..
            } => {
                let text = if command.starts_with("toolu_") || command.starts_with("call_") {
                    self.pending_starts.pop_front().unwrap_or_default()
                } else {
                    if let Some(at) = self.pending_starts.iter().position(|c| c == command) {
                        self.pending_starts.remove(at);
                    }
                    command.clone()
                };
                self.commands.insert(index, text.clone());
                self.commands_since_artifact += 1;
                if exit_code.is_some_and(|code| code != 0) {
                    let key = command_key(&text);
                    *self.failures_since_artifact.entry(key.clone()).or_default() += 1;
                    self.last_failed = Some(key);
                } else {
                    self.last_failed = None;
                }
            }
            Kind::ArtifactChanged { .. } => {
                self.failures_since_artifact.clear();
                self.commands_since_artifact = 0;
                self.last_failed = None;
            }
            _ => {}
        }
        self.history.push(observation.clone());
    }

    /// The command an observation names, when it names one.
    #[must_use]
    pub fn command(&self, index: usize) -> Option<&str> {
        self.commands.get(&index).map(String::as_str)
    }

    /// The rules' answers over the prefix, looking at what arrived since
    /// the last judgment for the reading and claiming questions.
    #[must_use]
    pub fn rules(&self, params: &Params, context: &Context) -> Flags {
        let fresh = self.last_look..self.history.len();
        let rereading = fresh.clone().any(|index| {
            matches!(
                self.history[index].event.kind,
                Kind::CommandStarted { .. } | Kind::CommandCompleted { .. }
            ) && self
                .command(index)
                .is_some_and(|command| reads_briefed(command, &context.briefed))
        });
        let claim = fresh
            .clone()
            .rev()
            .find_map(|index| match &self.history[index].event.kind {
                Kind::AssistantClaim { text } => Some(text.as_str()),
                _ => None,
            });
        Flags {
            stalled: self.commands_since_artifact >= params.stall_commands,
            repeating: self.last_failed.as_ref().is_some_and(|key| {
                self.failures_since_artifact.get(key).copied().unwrap_or(0) >= 2
            }),
            rereading,
            claims_done: claim.is_some_and(claims_done),
        }
    }

    /// One event as a request carries it.
    fn describe(&self, index: usize, max: usize) -> Value {
        let observation = &self.history[index];
        let what = match &observation.event.kind {
            Kind::CommandStarted { .. } => {
                format!(
                    "started: {}",
                    self.command(index).map(unwrap_shell).unwrap_or_default()
                )
            }
            Kind::CommandCompleted {
                exit_code, output, ..
            } => format!(
                "completed with exit {}: {} → {}",
                exit_code.map_or("?".to_string(), |code| code.to_string()),
                self.command(index).map(unwrap_shell).unwrap_or_default(),
                crate::judge::clip_tail(output, max / 2),
            ),
            Kind::ArtifactChanged { path, change } => format!("{change} {path}"),
            Kind::AssistantClaim { text } => format!("said: {text}"),
            Kind::SessionEnded { error, result } => format!(
                "ended{}: {}",
                if *error { " with an error" } else { "" },
                result.clone().unwrap_or_default()
            ),
            Kind::SessionStarted { .. } | Kind::UsageUpdate { .. } => String::new(),
        };
        json!({
            "seq": observation.version.seq,
            "kind": observation.event.kind.word(),
            "what": clip(&what, max),
        })
    }

    /// The Jev state: the task, the open requirements, what the briefing
    /// holds, the recent events, and counts since the last judgment. No
    /// times, so a replay asks the same state however it is paced.
    #[must_use]
    pub fn state(&self, params: &Params, context: &Context, trigger: &str) -> Value {
        let recent: Vec<usize> = (0..self.history.len())
            .rev()
            .filter(|&index| {
                !matches!(
                    self.history[index].event.kind,
                    Kind::UsageUpdate { .. } | Kind::SessionStarted { .. }
                )
            })
            .take(params.window)
            .collect();
        let fresh = &self.history[self.last_look..];
        let count = |f: &dyn Fn(&Kind) -> bool| fresh.iter().filter(|o| f(&o.event.kind)).count();
        json!({
            "task": clip(&context.task, 1_500),
            "open_requirements": context
                .requirements
                .iter()
                .take(params.requirements)
                .map(|r| json!({ "id": r.id, "text": clip(&r.text, 240) }))
                .collect::<Vec<_>>(),
            "briefing_holds": context.briefed.iter().take(20).map(|item| clip(item, 160)).collect::<Vec<_>>(),
            "recent_events": recent.iter().rev().map(|&index| self.describe(index, params.event_chars)).collect::<Vec<_>>(),
            "since_last_look": {
                "events": fresh.len(),
                "artifact_changes": count(&|k| matches!(k, Kind::ArtifactChanged { .. })),
                "failed_commands": count(&|k| matches!(k, Kind::CommandCompleted { exit_code: Some(code), .. } if *code != 0)),
                "commands_since_last_artifact_change": self.commands_since_artifact,
            },
            "trigger": trigger,
        })
    }

    /// The last `n` failing commands with their output tails, newest last.
    #[must_use]
    pub fn last_errors(&self, n: usize) -> Vec<String> {
        let mut errors: Vec<String> = self
            .history
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(index, observation)| match &observation.event.kind {
                Kind::CommandCompleted {
                    exit_code: Some(code),
                    output,
                    ..
                } if *code != 0 => Some(format!(
                    "`{}` exited {code}: {}",
                    clip(&unwrap_shell(self.command(index).unwrap_or_default()), 160),
                    crate::judge::clip_tail(output.trim(), 400)
                )),
                _ => None,
            })
            .take(n)
            .collect();
        errors.reverse();
        errors
    }
}

/// The questions, in one set.
#[must_use]
pub fn questions() -> Questions {
    Questions::new()
        .with("progress", Noul::new(Q_PROGRESS))
        .with("repeating", Noul::new(Q_REPEATING))
        .with("rereading", Noul::new(Q_REREADING))
        .with("claims_done", Noul::new(Q_CLAIMS_DONE))
}

/// The questions' character count, for estimating a request's size.
fn questions_chars() -> usize {
    Q_PROGRESS.len() + Q_REPEATING.len() + Q_REREADING.len() + Q_CLAIMS_DONE.len()
}

/// One judgment at one trigger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Judgment {
    /// Its place among the monitor's judgments, from 1.
    pub n: usize,
    /// `operation`, `artifact`, `claim`, or `silence`.
    pub trigger: String,
    /// The version the judgment was made at.
    pub basis: Version,
    /// When the triggering observation arrived.
    pub basis_ms: u64,
    /// When the answer arrived: the basis time for the rules alone, plus
    /// the Jev latency when Jev was asked.
    pub arrived_ms: u64,
    pub rules: Flags,
    /// How Jev answered: `live`, `recorded`, `miss`, `off`, `failed`, or
    /// `not_asked`.
    pub jev_how: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jev: Option<Answers>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jev_flags: Option<Flags>,
    /// The request's size: the state and the questions, in characters.
    pub state_chars: usize,
    /// Input tokens as the answer reported them, or estimated at four
    /// characters a token.
    pub input_tokens: Option<u64>,
    /// `reported` or `estimated`.
    pub tokens_basis: String,
    /// What the request costs at Jev's rate, whether it was paid now or
    /// when the answer was recorded.
    pub cost_usd: Option<f64>,
    /// What this run spent: a live request's charge; zero replayed.
    pub spent_usd: f64,
    /// Whether the version moved before the answer arrived.
    pub stale: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_why: Option<String>,
    /// What the judgment proposed, when anything.
    pub proposal: Option<Intent>,
    /// Whether the controller would have admitted the proposal.
    pub admissible: Option<bool>,
    /// Whether the proposal was only a shadow.
    pub shadow: bool,
    /// The labels an evaluation attached, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Flags>,
}

impl Judgment {
    /// The flags the judgment acts on: Jev's when Jev answered, the rules'
    /// otherwise.
    #[must_use]
    pub fn decided(&self) -> Flags {
        self.jev_flags.unwrap_or(self.rules)
    }

    /// The judgment as a step extension.
    #[must_use]
    pub fn record(&self) -> Value {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Some(map) = value.as_object_mut() {
            map.insert("schema".to_string(), json!(JUDGMENT_SCHEMA));
        }
        value
    }

    fn step(&self) -> Step {
        let flags = self.decided();
        let named: Vec<&str> = Flags::NAMES
            .iter()
            .copied()
            .filter(|name| flags.get(name))
            .collect();
        Step::said(
            Source::System,
            &format!(
                "monitor judgment {} at {}: {}{}",
                self.n,
                self.trigger,
                if named.is_empty() {
                    "no flags".to_string()
                } else {
                    named.join(", ")
                },
                if self.stale { " (stale)" } else { "" }
            ),
        )
        .noting(JUDGMENT_KEY, self.record())
    }
}

/// What the monitor does with its flags beyond recording them.
#[derive(Clone, Debug, PartialEq)]
pub struct Acting {
    /// What an intervening flag proposes: a steer or a stop.
    pub intent: Intent,
    /// How many judgments in a row must flag before it proposes.
    pub after: usize,
    /// Which flags count; `intervene` counts any of the three.
    pub on: Vec<String>,
    /// The name the proposal carries.
    pub by: String,
}

/// Builds a steer's message from what a monitor has seen.
pub type SteerMessage = Box<dyn Fn(&Monitor) -> String>;

/// The monitor: a [`Watch`] for the host loop.
pub struct Monitor {
    pub params: Params,
    pub context: Context,
    pub jev: JevMode,
    /// `None` is shadow mode: every proposal is only recorded.
    pub acting: Option<Acting>,
    /// Builds a steer's message from what the monitor has seen.
    pub message: Option<SteerMessage>,
    pub tracker: Tracker,
    pending: Vec<Judgment>,
    pub judgments: Vec<Judgment>,
    last_event_ms: u64,
    silence_fired: bool,
    flagged_run: usize,
    acted: bool,
}

impl Monitor {
    /// A shadow monitor.
    #[must_use]
    pub fn new(params: Params, context: Context, jev: JevMode) -> Self {
        Monitor {
            params,
            context,
            jev,
            acting: None,
            message: None,
            tracker: Tracker::default(),
            pending: Vec::new(),
            judgments: Vec::new(),
            last_event_ms: 0,
            silence_fired: false,
            flagged_run: 0,
            acted: false,
        }
    }

    /// When the next pending answer arrives, if one is pending.
    #[must_use]
    pub fn next_arrival(&self) -> Option<u64> {
        self.pending.iter().map(|j| j.arrived_ms).min()
    }

    fn trigger_of(&self, kind: &Kind) -> Option<&'static str> {
        match kind {
            Kind::CommandCompleted { .. } if self.params.on_operation => Some("operation"),
            Kind::ArtifactChanged { .. } if self.params.on_artifact => Some("artifact"),
            Kind::AssistantClaim { .. } if self.params.on_claim => Some("claim"),
            _ => None,
        }
    }

    /// Makes one judgment at `trigger`, asking Jev when it is on.
    async fn judge(&mut self, trigger: &str, basis: Version, basis_ms: u64, recorder: &Recorder) {
        let rules = self.tracker.rules(&self.params, &self.context);
        let state = self.tracker.state(&self.params, &self.context, trigger);
        let state_chars = state.to_string().chars().count() + questions_chars();
        let n = self.judgments.len() + self.pending.len() + 1;
        self.tracker.last_look = self.tracker.history.len();
        let mut judgment = Judgment {
            n,
            trigger: trigger.to_string(),
            basis,
            basis_ms,
            arrived_ms: basis_ms,
            rules,
            jev_how: "not_asked".to_string(),
            jev: None,
            jev_flags: None,
            state_chars,
            input_tokens: None,
            tokens_basis: "estimated".to_string(),
            cost_usd: None,
            spent_usd: 0.0,
            stale: false,
            stale_why: None,
            proposal: None,
            admissible: None,
            shadow: self.acting.is_none(),
            labels: None,
        };
        if matches!(self.jev, JevMode::Off) {
            judgment.jev_how = "off".to_string();
        } else if self.params.jev {
            let asked = jevmode::ask(
                &self.jev,
                recorder,
                Ask {
                    component: COMPONENT,
                    name: DECISION,
                    id: format!("{DECISION}-{n}"),
                    state,
                    questions: questions(),
                    parent: None,
                    deadline: None,
                },
            )
            .await;
            judgment.jev_how = asked.how.to_string();
            if asked.how == "off" {
                self.pending.push(judgment);
                return;
            }
            let estimated = (state_chars as u64).div_ceil(4);
            judgment.input_tokens = Some(asked.input_tokens.unwrap_or(estimated));
            judgment.tokens_basis = if asked.input_tokens.is_some() {
                "reported"
            } else {
                "estimated"
            }
            .to_string();
            judgment.cost_usd = judgment
                .input_tokens
                .map(|tokens| tokens as f64 * USD_PER_MILLION_INPUT / 1_000_000.0);
            if asked.how == "live" {
                judgment.spent_usd = judgment.cost_usd.unwrap_or(0.0);
            }
            judgment.arrived_ms = basis_ms + asked.milliseconds.unwrap_or(self.params.latency_ms);
            if asked.answered() {
                let answers = Answers {
                    progress: asked.noul("progress"),
                    repeating: asked.noul("repeating"),
                    rereading: asked.noul("rereading"),
                    claims_done: asked.noul("claims_done"),
                };
                judgment.jev_flags = Some(answers.flags(self.params.threshold));
                judgment.jev = Some(answers);
            }
        }
        self.pending.push(judgment);
    }

    /// Delivers every pending answer that has arrived by `now_ms`, or all
    /// of them when the session is over.
    fn deliver(
        &mut self,
        now_ms: u64,
        controller: &Controller,
        recorder: &Recorder,
        ending: bool,
    ) -> Vec<Submission> {
        let mut out = Vec::new();
        let (due, later): (Vec<Judgment>, Vec<Judgment>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|j| ending || j.arrived_ms <= now_ms);
        self.pending = later;
        let current = controller.version();
        for mut judgment in due {
            let why = if judgment.basis.generation != current.generation {
                Some(format!(
                    "asked at generation {} and answered at generation {}",
                    judgment.basis.generation, current.generation
                ))
            } else if judgment.basis.revision != current.revision {
                Some(format!(
                    "asked at workspace revision {} and answered at revision {}",
                    judgment.basis.revision, current.revision
                ))
            } else if judgment.arrived_ms > now_ms && controller.phase() != Phase::Running {
                Some("the session ended before the answer arrived".to_string())
            } else {
                None
            };
            judgment.stale = why.is_some();
            judgment.stale_why = why;
            let flags = judgment.decided();
            let intent = if let Some(acting) = &self.acting {
                let counted = acting.on.iter().any(|name| flags.get(name));
                self.flagged_run = if counted { self.flagged_run + 1 } else { 0 };
                (counted && self.flagged_run >= acting.after && !self.acted && !judgment.stale)
                    .then_some(acting.intent)
            } else if flags.intervene() {
                Some(Intent::Steer)
            } else if flags.claims_done {
                Some(Intent::Certify)
            } else {
                None
            };
            if let Some(intent) = intent {
                let proposal = Proposal {
                    intent,
                    basis: judgment.basis.clone(),
                    by: self
                        .acting
                        .as_ref()
                        .map_or_else(|| COMPONENT.to_string(), |acting| acting.by.clone()),
                };
                judgment.proposal = Some(intent);
                judgment.admissible = Some(controller.check(&proposal).is_ok());
                let shadow = self.acting.is_none();
                if !shadow {
                    self.acted = true;
                }
                let message = (intent == Intent::Steer)
                    .then(|| self.message.as_ref().map(|build| build(self)))
                    .flatten();
                out.push(Submission {
                    proposal,
                    message,
                    shadow,
                });
            }
            recorder.push(judgment.step());
            self.judgments.push(judgment);
        }
        out
    }

    /// The open requirements and the last errors, as a steer carries them.
    #[must_use]
    pub fn steer_text(&self) -> String {
        let mut text = String::from(
            "The host's monitor sees no progress on the task's open requirements. Stop repeating what failed.\n",
        );
        let errors = self.tracker.last_errors(2);
        if !errors.is_empty() {
            text.push_str("\nThe last errors:\n");
            for error in errors {
                text.push_str(&format!("- {error}\n"));
            }
        }
        if !self.context.requirements.is_empty() {
            text.push_str("\nThe open requirements:\n");
            for r in self
                .context
                .requirements
                .iter()
                .take(self.params.requirements)
            {
                text.push_str(&format!("- {}: {}\n", r.id, clip(&r.text, 200)));
            }
        }
        text
    }
}

impl Watch for Monitor {
    fn look<'a>(
        &'a mut self,
        now_ms: u64,
        observed: &'a [Observation],
        controller: &'a Controller,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Vec<Submission>> {
        Box::pin(async move {
            let mut out = self.deliver(now_ms, controller, recorder, false);
            let mut trigger = None;
            for observation in observed {
                self.tracker.push(observation);
                self.last_event_ms = observation.at_ms;
                self.silence_fired = false;
                if let Some(word) = self.trigger_of(&observation.event.kind) {
                    trigger = Some((word, observation.version.clone(), observation.at_ms));
                }
            }
            if trigger.is_none()
                && observed.is_empty()
                && !self.silence_fired
                && controller.phase() == Phase::Running
                && let Some(silence) = self.params.silence_ms
                && now_ms.saturating_sub(self.last_event_ms) >= silence
            {
                self.silence_fired = true;
                trigger = Some(("silence", controller.version(), now_ms));
            }
            if let Some((word, basis, at)) = trigger {
                self.judge(word, basis, at, recorder).await;
                out.extend(self.deliver(now_ms, controller, recorder, false));
            }
            out
        })
    }

    fn finish(
        &mut self,
        now_ms: u64,
        controller: &Controller,
        recorder: &Recorder,
    ) -> Vec<Submission> {
        self.deliver(now_ms, controller, recorder, true)
    }
}

/// What a host needs to start a monitor on a session: its parameters,
/// where Jev's answers come from, and the task.
#[derive(Clone)]
pub struct Setup {
    pub params: Params,
    pub jev: JevMode,
    pub task: String,
    /// What the monitor proposes beyond recording; `None` is shadow mode.
    pub acting: Option<Acting>,
}

impl Setup {
    /// A shadow monitor, or an acting one, for a session started on
    /// `briefing`. An acting monitor's steer carries the last errors and
    /// the open requirements.
    #[must_use]
    pub fn start(&self, briefing: &str) -> Monitor {
        let mut monitor = Monitor::new(
            self.params.clone(),
            Context::new(&self.task, briefing),
            self.jev.clone(),
        );
        monitor.acting.clone_from(&self.acting);
        if self.acting.is_some() {
            monitor.message = Some(Box::new(Monitor::steer_text));
        }
        monitor
    }
}

/// A session's judgments, summarized for its host-loop record.
#[must_use]
pub fn summary(judgments: &[Judgment]) -> Value {
    let flagged = |name: &str, jev: bool| {
        judgments
            .iter()
            .filter(|j| {
                if jev {
                    j.jev_flags.is_some_and(|f| f.get(name))
                } else {
                    j.rules.get(name)
                }
            })
            .count()
    };
    let asked = judgments
        .iter()
        .filter(|j| j.jev_how != "not_asked" && j.jev_how != "off")
        .count();
    json!({
        "component": COMPONENT,
        "questions": QUESTION_SET,
        "judgments": judgments.len(),
        "by_trigger": judgments.iter().fold(BTreeMap::<String, usize>::new(), |mut counts, j| {
            *counts.entry(j.trigger.clone()).or_default() += 1;
            counts
        }),
        "rules": Flags::NAMES.iter().map(|name| ((*name).to_string(), json!(flagged(name, false)))).collect::<serde_json::Map<String, Value>>(),
        "jev": Flags::NAMES.iter().map(|name| ((*name).to_string(), json!(flagged(name, true)))).collect::<serde_json::Map<String, Value>>(),
        "jev_asked": asked,
        "jev_answered": judgments.iter().filter(|j| j.jev_flags.is_some()).count(),
        "stale": judgments.iter().filter(|j| j.stale).count(),
        "cost_usd": (judgments.iter().filter_map(|j| j.cost_usd).sum::<f64>() * 1e6).round() / 1e6,
        "spent_usd": (judgments.iter().map(|j| j.spent_usd).sum::<f64>() * 1e6).round() / 1e6,
        "shadow": judgments.iter().all(|j| j.shadow),
    })
}

/// Replays `events`, each at its time, through `monitor`, as the host loop
/// would observe them: a judgment sees only the prefix up to its trigger,
/// and a pending answer is delivered at its arrival, before any later
/// event. Returns the controller, with its shadow notes.
pub async fn replay(
    events: &[(u64, Event)],
    monitor: &mut Monitor,
    recorder: &Recorder,
) -> Controller {
    let mut controller = Controller::default();
    controller.begin(0, "replay");
    let mut index = 0;
    let mut now = 0;
    while index < events.len() {
        let at = events[index].0;
        // Quiet looks: pending answers that arrive, and silences, before
        // the next event.
        for _ in 0..10_000 {
            let silence = monitor
                .params
                .silence_ms
                .filter(|_| !monitor.silence_fired)
                .map(|silence| monitor.last_event_ms + silence);
            let next = [monitor.next_arrival(), silence]
                .into_iter()
                .flatten()
                .filter(|t| *t < at && *t >= now)
                .min();
            let Some(next) = next else { break };
            now = next;
            let submissions = monitor.look(now, &[], &controller, recorder).await;
            for submission in submissions {
                controller.note(now, &submission.proposal);
            }
        }
        now = at;
        let mut observed = Vec::new();
        while index < events.len() && events[index].0 == at {
            observed.push(controller.observe(at, events[index].1.clone()));
            index += 1;
        }
        let submissions = monitor.look(now, &observed, &controller, recorder).await;
        for submission in submissions {
            controller.note(now, &submission.proposal);
        }
    }
    controller.ended(now, "the replay ended");
    for submission in monitor.finish(now, &controller, recorder) {
        controller.note(now, &submission.proposal);
    }
    controller
}

/// Labels a judgment from the whole stream, knowing what the monitor could
/// not: what happened after its trigger and how the attempt ended.
///
/// - `stalled`: the attempt did not pass, and after the trigger it changed
///   no artifact while running at least two more commands. Intervening
///   there would have lost nothing.
/// - `repeating`: the last command before the trigger failed, and the same
///   command failed again after it before any artifact changed.
/// - `rereading`: a command since the previous judgment read evidence the
///   briefing holds. This is a fact of the prefix, so the rule defines it.
/// - `claims_done`: a claim since the previous judgment that was the
///   session's last, with no artifact changed after it: the final report
///   on the candidate.
#[must_use]
pub fn hindsight(
    tracker: &Tracker,
    judgment: &Judgment,
    previous_seq: u64,
    passed: Option<bool>,
    context: &Context,
) -> Flags {
    let history = &tracker.history;
    let cut = history
        .iter()
        .position(|o| o.version.seq > judgment.basis.seq)
        .unwrap_or(history.len());
    let (before, after) = history.split_at(cut);
    let changes_after = after
        .iter()
        .any(|o| matches!(o.event.kind, Kind::ArtifactChanged { .. }));
    let commands_after = after
        .iter()
        .filter(|o| matches!(o.event.kind, Kind::CommandCompleted { .. }))
        .count();
    let stalled = passed != Some(true) && !changes_after && commands_after >= 2;
    let last_failed = before
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, o)| match &o.event.kind {
            Kind::CommandCompleted { exit_code, .. } => Some(
                exit_code
                    .is_some_and(|code| code != 0)
                    .then(|| command_key(tracker.command(index).unwrap_or_default())),
            ),
            _ => None,
        })
        .flatten();
    let repeating = last_failed.is_some_and(|key| {
        for (offset, o) in after.iter().enumerate() {
            match &o.event.kind {
                Kind::ArtifactChanged { .. } => return false,
                Kind::CommandCompleted {
                    exit_code: Some(code),
                    ..
                } if *code != 0
                    && command_key(tracker.command(cut + offset).unwrap_or_default()) == key =>
                {
                    return true;
                }
                _ => {}
            }
        }
        false
    });
    let since = |o: &&Observation| o.version.seq > previous_seq;
    let rereading = before
        .iter()
        .enumerate()
        .filter(|(_, o)| since(o))
        .any(|(index, o)| {
            matches!(
                o.event.kind,
                Kind::CommandStarted { .. } | Kind::CommandCompleted { .. }
            ) && tracker
                .command(index)
                .is_some_and(|command| reads_briefed(command, &context.briefed))
        });
    let claimed = before
        .iter()
        .filter(since)
        .any(|o| matches!(&o.event.kind, Kind::AssistantClaim { .. }));
    let claims_after = after
        .iter()
        .any(|o| matches!(&o.event.kind, Kind::AssistantClaim { .. }));
    Flags {
        stalled,
        repeating,
        rereading,
        claims_done: claimed && !claims_after && !changes_after,
    }
}

/// A known interval of one flag, for a scripted stream whose stalls and
/// loops are written in: the flag is true for a judgment whose trigger
/// arrived in `[from_ms, to_ms)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Onset {
    pub flag: String,
    pub from_ms: u64,
    #[serde(default)]
    pub to_ms: Option<u64>,
}

/// Labels a judgment from known onsets.
#[must_use]
pub fn from_onsets(judgment: &Judgment, onsets: &[Onset]) -> Flags {
    let mut flags = Flags::default();
    for onset in onsets {
        let within = judgment.basis_ms >= onset.from_ms
            && onset.to_ms.is_none_or(|to| judgment.basis_ms < to);
        if within {
            flags.set(&onset.flag, true);
        }
    }
    flags
}

/// One question's counts for one decider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counts {
    /// Judgments it answered.
    pub answered: usize,
    pub flagged: usize,
    /// Flags the label agrees with.
    pub correct: usize,
    /// Labels it missed.
    pub missed: usize,
}

impl Counts {
    /// Correct flags over flags, or `None` without a flag.
    #[must_use]
    pub fn precision(&self) -> Option<f64> {
        (self.flagged > 0).then(|| self.correct as f64 / self.flagged as f64)
    }

    /// Correct flags over labels, or `None` without a label.
    #[must_use]
    pub fn recall(&self) -> Option<f64> {
        let labels = self.correct + self.missed;
        (labels > 0).then(|| self.correct as f64 / labels as f64)
    }

    fn add(&mut self, other: &Counts) {
        self.answered += other.answered;
        self.flagged += other.flagged;
        self.correct += other.correct;
        self.missed += other.missed;
    }
}

/// Labeled judgments, scored: trigger precision per question for the
/// rules and for Jev, stale answers, and cost.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub triggers: usize,
    pub by_trigger: BTreeMap<String, usize>,
    /// Judgments with each label.
    pub labels: BTreeMap<String, usize>,
    pub rules: BTreeMap<String, Counts>,
    pub jev: BTreeMap<String, Counts>,
    /// Jev requests made or replayed, and those that answered.
    pub jev_requests: usize,
    pub jev_answered: usize,
    /// Answers that arrived after the version they were asked at moved.
    pub stale: usize,
    pub input_tokens: u64,
    /// What the requests cost at Jev's rate.
    pub cost_usd: f64,
    /// What this run paid for live requests.
    pub spent_usd: f64,
    pub state_chars: usize,
}

impl Score {
    /// Adds labeled judgments.
    pub fn add(&mut self, judgments: &[Judgment]) {
        for judgment in judgments {
            let Some(labels) = judgment.labels else {
                continue;
            };
            self.triggers += 1;
            *self.by_trigger.entry(judgment.trigger.clone()).or_default() += 1;
            for name in Flags::NAMES.iter().copied().chain(["intervene"]) {
                let label = labels.get(name);
                if label {
                    *self.labels.entry(name.to_string()).or_default() += 1;
                }
                let count = |flag: bool, counts: &mut Counts| {
                    counts.answered += 1;
                    if flag {
                        counts.flagged += 1;
                        if label {
                            counts.correct += 1;
                        }
                    } else if label {
                        counts.missed += 1;
                    }
                };
                count(
                    judgment.rules.get(name),
                    self.rules.entry(name.to_string()).or_default(),
                );
                if let Some(flags) = judgment.jev_flags {
                    count(
                        flags.get(name),
                        self.jev.entry(name.to_string()).or_default(),
                    );
                }
            }
            if judgment.jev_how != "not_asked" && judgment.jev_how != "off" {
                self.jev_requests += 1;
                self.input_tokens += judgment.input_tokens.unwrap_or(0);
                self.cost_usd += judgment.cost_usd.unwrap_or(0.0);
                self.spent_usd += judgment.spent_usd;
                self.state_chars += judgment.state_chars;
                if judgment.stale {
                    self.stale += 1;
                }
            }
            if judgment.jev_flags.is_some() {
                self.jev_answered += 1;
            }
        }
    }

    /// Folds another score into this one.
    pub fn merge(&mut self, other: &Score) {
        self.triggers += other.triggers;
        for (k, v) in &other.by_trigger {
            *self.by_trigger.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &other.labels {
            *self.labels.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &other.rules {
            self.rules.entry(k.clone()).or_default().add(v);
        }
        for (k, v) in &other.jev {
            self.jev.entry(k.clone()).or_default().add(v);
        }
        self.jev_requests += other.jev_requests;
        self.jev_answered += other.jev_answered;
        self.stale += other.stale;
        self.input_tokens += other.input_tokens;
        self.cost_usd += other.cost_usd;
        self.spent_usd += other.spent_usd;
        self.state_chars += other.state_chars;
    }

    /// The score with precision and recall spelled out, rounded.
    #[must_use]
    pub fn summary(&self) -> Value {
        let round = |value: Option<f64>| value.map(crate::component::pack::round);
        let table = |counts: &BTreeMap<String, Counts>| {
            counts
                .iter()
                .map(|(name, c)| {
                    (
                        name.clone(),
                        json!({
                            "answered": c.answered,
                            "flagged": c.flagged,
                            "correct": c.correct,
                            "missed": c.missed,
                            "precision": round(c.precision()),
                            "recall": round(c.recall()),
                        }),
                    )
                })
                .collect::<serde_json::Map<String, Value>>()
        };
        json!({
            "triggers": self.triggers,
            "by_trigger": self.by_trigger,
            "labels": self.labels,
            "rules": table(&self.rules),
            "jev": table(&self.jev),
            "jev_requests": self.jev_requests,
            "jev_answered": self.jev_answered,
            "stale": self.stale,
            "stale_rate": round((self.jev_requests > 0).then(|| self.stale as f64 / self.jev_requests as f64)),
            "input_tokens": self.input_tokens,
            "cost_usd": (self.cost_usd * 1e6).round() / 1e6,
            "spent_usd": (self.spent_usd * 1e6).round() / 1e6,
            "mean_state_chars": (self.jev_requests > 0).then(|| self.state_chars / self.jev_requests),
        })
    }
}

/// Normalized events of a native stream placed on a clock: each native
/// line `ms_per_line` after the last.
#[must_use]
pub fn paced(stream: &str, ms_per_line: u64) -> Vec<(u64, Event)> {
    let format = crate::stream::Format::detect(stream).unwrap_or(crate::stream::Format::Codex);
    crate::stream::normalize(format, stream)
        .into_iter()
        .map(|event| (event.line as u64 * ms_per_line, event))
        .collect()
}

/// Labels every judgment by hindsight over the whole session `tracker`
/// saw.
pub fn label_by_hindsight(
    judgments: &mut [Judgment],
    tracker: &Tracker,
    passed: Option<bool>,
    context: &Context,
) {
    let mut previous = 0;
    for judgment in judgments.iter_mut() {
        judgment.labels = Some(hindsight(tracker, judgment, previous, passed, context));
        previous = judgment.basis.seq;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(seq: u64, kind: Kind) -> Event {
        Event {
            seq,
            line: seq as usize,
            offset: None,
            kind,
        }
    }

    fn failed(seq: u64, command: &str) -> Event {
        event(
            seq,
            Kind::CommandCompleted {
                command: command.to_string(),
                exit_code: Some(1),
                output: "1 failed".to_string(),
            },
        )
    }

    fn rules_only() -> Params {
        Params {
            jev: false,
            ..Params::default()
        }
    }

    #[test]
    fn the_briefing_headings_name_what_it_holds() {
        let briefing = "## Files\n\n### run.py (Jev p=0.91)\n\n```\nx\n```\n### $ pwd && ls -la (Jev p=0.75)\n### Step 1: `cat notes.txt` (Jev p=0.40)\n";
        let items = briefed_items(briefing);
        assert_eq!(items, vec!["run.py", "pwd && ls -la", "cat notes.txt"]);
        assert!(reads_briefed("/bin/bash -lc 'cat /app/run.py'", &items));
        assert!(reads_briefed("ls -la", &items));
        assert!(!reads_briefed("cat other.py", &items));
        assert!(!reads_briefed("python3 run.py", &items));
    }

    #[test]
    fn a_claim_reads_as_done_only_on_whole_words() {
        assert!(claims_done("Implemented the fix; all tests pass."));
        assert!(claims_done("Done."));
        assert!(!claims_done("I'll implement the function next."));
        assert!(!claims_done("abandoned"));
    }

    #[tokio::test]
    async fn a_loop_flags_repeating_and_a_stall_without_jev() {
        let events: Vec<(u64, Event)> =
            (1..=5).map(|i| (i * 100, failed(i, "pytest -q"))).collect();
        let mut monitor = Monitor::new(rules_only(), Context::default(), JevMode::Off);
        let recorder = Recorder::default();
        let controller = replay(&events, &mut monitor, &recorder).await;
        assert_eq!(monitor.judgments.len(), 5);
        assert!(!monitor.judgments[0].rules.repeating);
        assert!(monitor.judgments[1].rules.repeating);
        assert!(!monitor.judgments[2].rules.stalled);
        assert!(monitor.judgments[3].rules.stalled);
        // Every judgment is a shadow proposal the controller recorded.
        assert_eq!(controller.shadow.len(), 4);
        assert!(controller.shadow.iter().all(|note| note.admissible));
        // Each judgment is a step in the record.
        let steps = recorder.steps();
        assert_eq!(
            steps
                .iter()
                .filter(|s| s.extensions.contains_key(JUDGMENT_KEY))
                .count(),
            5
        );
    }

    #[tokio::test]
    async fn a_late_answer_about_a_changed_workspace_is_stale() {
        // A command completes at 100; a write lands at 150, before an
        // 800 ms answer arrives.
        let events = vec![
            (100, failed(1, "pytest -q")),
            (
                150,
                event(
                    2,
                    Kind::ArtifactChanged {
                        path: "run.py".to_string(),
                        change: "write".to_string(),
                    },
                ),
            ),
            (2_000, failed(3, "pytest -q")),
        ];
        let params = Params {
            on_artifact: false,
            ..Params::default()
        };
        let mut monitor = Monitor::new(
            params,
            Context::default(),
            JevMode::Recorded(jevmode::Recorded::empty()),
        );
        let recorder = Recorder::default();
        let _ = replay(&events, &mut monitor, &recorder).await;
        assert_eq!(monitor.judgments.len(), 2);
        let first = &monitor.judgments[0];
        assert_eq!(first.jev_how, "miss");
        assert_eq!(first.arrived_ms, 900);
        assert!(first.stale, "{first:?}");
        assert!(first.stale_why.as_deref().unwrap().contains("revision 0"));
        assert!(!monitor.judgments[1].stale || monitor.judgments[1].arrived_ms > 2_000);
    }

    #[tokio::test]
    async fn a_silence_triggers_once() {
        let events = vec![
            (100, failed(1, "make")),
            (
                10_000,
                event(
                    2,
                    Kind::AssistantClaim {
                        text: "Done.".to_string(),
                    },
                ),
            ),
        ];
        let params = Params {
            silence_ms: Some(1_000),
            ..rules_only()
        };
        let mut monitor = Monitor::new(params, Context::default(), JevMode::Off);
        let _ = replay(&events, &mut monitor, &Recorder::default()).await;
        let triggers: Vec<&str> = monitor
            .judgments
            .iter()
            .map(|j| j.trigger.as_str())
            .collect();
        assert_eq!(triggers, vec!["operation", "silence", "claim"]);
        assert_eq!(monitor.judgments[1].basis_ms, 1_100);
        assert!(monitor.judgments[2].rules.claims_done);
    }

    #[tokio::test]
    async fn hindsight_sees_what_the_prefix_could_not() {
        let events = vec![
            (100, failed(1, "pytest -q")),
            (200, failed(2, "pytest -q")),
            (300, failed(3, "pytest -q")),
        ];
        let mut monitor = Monitor::new(rules_only(), Context::default(), JevMode::Off);
        let recorder = Recorder::default();
        let _ = replay(&events, &mut monitor, &recorder).await;
        let mut judgments = monitor.judgments.clone();
        label_by_hindsight(
            &mut judgments,
            &monitor.tracker,
            Some(false),
            &Context::default(),
        );
        let labels: Vec<Flags> = judgments.iter().map(|j| j.labels.unwrap()).collect();
        // The first failure is followed by the same failure: a loop, and
        // two more commands with no change: a stall.
        assert!(labels[0].repeating && labels[0].stalled);
        // The last has nothing after it.
        assert!(!labels[2].repeating && !labels[2].stalled);
        let mut score = Score::default();
        score.add(&judgments);
        assert_eq!(score.triggers, 3);
        let repeating = score.rules["repeating"];
        // The rule flags the second and third; the label holds for the
        // first two.
        assert_eq!((repeating.flagged, repeating.correct), (2, 1));
        assert_eq!(repeating.precision(), Some(0.5));
    }

    fn looping_script() -> crate::scripted::Script {
        crate::scripted::Script::parse(
            &json!({
                "schema": crate::scripted::SCRIPT_SCHEMA,
                "name": "loop",
                "events": [
                    { "at_ms": 0, "do": "command", "command": "pytest -q", "output": "1 failed", "exit_code": 1 },
                    { "at_ms": 20, "do": "command", "command": "pytest -q", "output": "1 failed", "exit_code": 1 },
                    { "at_ms": 40, "do": "command", "command": "pytest -q", "output": "1 failed", "exit_code": 1 },
                    { "at_ms": 60, "do": "hang" }
                ],
                "on_steer": [
                    { "at_ms": 10, "do": "write", "path": "fixed.txt", "content": "ok\n" },
                    { "at_ms": 20, "do": "end" }
                ]
            })
            .to_string(),
        )
        .unwrap()
    }

    fn briefing() -> crate::delegate::Briefing {
        crate::delegate::Briefing::build(
            &crate::delegate::BriefingInputs {
                instruction: "Make the tests pass.".to_string(),
                requirements: Vec::new(),
                files: Vec::new(),
                spans: Vec::new(),
                commands: Vec::new(),
                last_output: None,
                conclusion: String::new(),
                directions: String::new(),
            },
            1_000,
        )
    }

    async fn drive_loop(acting: Option<Acting>) -> (crate::session::Driven, Monitor) {
        let dir = std::env::temp_dir().join(format!(
            "coder-one-monitor-drive-{}-{}",
            std::process::id(),
            atif::now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut scripted = crate::scripted::Scripted::new(looping_script(), dir.clone());
        let recorder = Recorder::default();
        let mut monitor = Monitor::new(rules_only(), Context::default(), JevMode::Off);
        monitor.acting = acting;
        monitor.message = Some(Box::new(Monitor::steer_text));
        let controls = crate::session::Controls {
            deadline_ms: 1_000,
            ..crate::session::Controls::default()
        };
        let driven = crate::session::drive_watched(
            &mut scripted,
            &briefing(),
            &controls,
            &recorder,
            &mut crate::session::virtual_time(),
            Some(&mut monitor as &mut dyn Watch),
        )
        .await;
        let _ = std::fs::remove_dir_all(&dir);
        (driven, monitor)
    }

    #[tokio::test]
    async fn a_shadow_monitor_records_proposals_and_never_acts() {
        let (driven, monitor) = drive_loop(None).await;
        assert!(monitor.judgments.iter().any(|j| j.rules.repeating));
        assert!(!driven.shadow.is_empty());
        assert!(
            driven
                .shadow
                .iter()
                .all(|note| note.proposal.by == COMPONENT)
        );
        // Nothing steered it: the loop hung until the host deadline.
        assert!(
            !driven
                .actions
                .iter()
                .any(|a| a.capability == crate::session::Capability::Steer)
        );
        assert_eq!(driven.stopped_by.as_deref(), Some("host deadline"));
    }

    #[tokio::test]
    async fn an_acting_monitor_steers_through_the_controller() {
        let acting = Acting {
            intent: Intent::Steer,
            after: 1,
            on: vec!["repeating".to_string()],
            by: "control.handoff".to_string(),
        };
        let (driven, _) = drive_loop(Some(acting)).await;
        let steer = driven
            .actions
            .iter()
            .find(|a| a.capability == crate::session::Capability::Steer)
            .expect("a steer");
        assert_eq!(steer.outcome, "done", "{steer:?}");
        assert!(steer.detail.contains("control.handoff"));
        assert_eq!(driven.report.status.word(), "answered");
    }

    #[test]
    fn onsets_label_by_trigger_time() {
        let judgment = Judgment {
            n: 1,
            trigger: "operation".to_string(),
            basis: Version::default(),
            basis_ms: 500,
            arrived_ms: 500,
            rules: Flags::default(),
            jev_how: "not_asked".to_string(),
            jev: None,
            jev_flags: None,
            state_chars: 0,
            input_tokens: None,
            tokens_basis: "estimated".to_string(),
            cost_usd: None,
            spent_usd: 0.0,
            stale: false,
            stale_why: None,
            proposal: None,
            admissible: None,
            shadow: true,
            labels: None,
        };
        let onsets = vec![
            Onset {
                flag: "repeating".to_string(),
                from_ms: 400,
                to_ms: None,
            },
            Onset {
                flag: "rereading".to_string(),
                from_ms: 0,
                to_ms: Some(500),
            },
        ];
        let flags = from_onsets(&judgment, &onsets);
        assert!(flags.repeating && !flags.rereading);
    }
}
