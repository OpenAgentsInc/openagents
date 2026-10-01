//! What a Coder task does, as one typed stream of events.
//!
//! A Coder task records everything it does as an ATIF trajectory, one
//! file per turn (`<task>.<turn>.atif.jsonl`). This module turns those
//! steps into [`CoderEvent`]s, the one stream every surface shows: the
//! `openagents` CLI prints it (text, or NDJSON under `--json`), and the
//! desktop and the phone render the same events in their own views. The
//! mapping lives here, beside the chat service they all share, so a step
//! reads the same everywhere.
//!
//! The events, in the order a turn produces them:
//!
//! - [`CoderEvent::CoderStarted`]: the task, its project and worktree, and
//!   the provider chosen, with why.
//! - [`CoderEvent::Step`]: one thing Coder did or said, a
//!   [`StepKind`] (thinking, a command, a tool call, an observation, its
//!   reply as it is written, a note), naming its ATIF step.
//! - [`CoderEvent::Output`]: a command's output, bounded.
//! - [`CoderEvent::ProviderSwitched`]: a provider refused for a usage or
//!   rate limit and the run moved on.
//! - [`CoderEvent::Progress`]: the loop's step, its bound, and Jev's
//!   judgment of whether the task is done.
//! - A turn ends with exactly one of [`CoderEvent::Result`],
//!   [`CoderEvent::Question`], [`CoderEvent::Approval`],
//!   [`CoderEvent::Failure`], or [`CoderEvent::Stopped`].
//!
//! [`Line`] is the envelope a stream carries: a sequence number, the task,
//! and the thread that started it. Nothing here reads a file, runs a
//! command, or holds a key; the caller feeds the steps and knows what
//! changed in the worktree.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The version of the event stream a [`Line`] belongs to.
pub const SCHEMA: &str = "openagents.coder.events.v1";
/// The most bytes of a command's output an [`Output`] event carries.
pub const MAX_OUTPUT: usize = 4 * 1024;
/// The most bytes of text a [`Step`] event carries.
pub const MAX_TEXT: usize = 2 * 1024;

/// One thing a Coder task did, said, or became.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum CoderEvent {
    CoderStarted(Started),
    Step(Step),
    Output(Output),
    ProviderSwitched(Switched),
    Question(Asked),
    Approval(Asked),
    Progress(Progress),
    Result(Finished),
    Failure(Failure),
    Stopped(Stopped),
}

impl CoderEvent {
    /// The event's name, as `event` spells it.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            CoderEvent::CoderStarted(_) => "coder_started",
            CoderEvent::Step(_) => "step",
            CoderEvent::Output(_) => "output",
            CoderEvent::ProviderSwitched(_) => "provider_switched",
            CoderEvent::Question(_) => "question",
            CoderEvent::Approval(_) => "approval",
            CoderEvent::Progress(_) => "progress",
            CoderEvent::Result(_) => "result",
            CoderEvent::Failure(_) => "failure",
            CoderEvent::Stopped(_) => "stopped",
        }
    }

    /// Whether the event ends a turn.
    #[must_use]
    pub fn ends_turn(&self) -> bool {
        matches!(
            self,
            CoderEvent::Question(_)
                | CoderEvent::Approval(_)
                | CoderEvent::Result(_)
                | CoderEvent::Failure(_)
                | CoderEvent::Stopped(_)
        )
    }
}

/// A turn started: where, and on which provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Started {
    /// The turn, from one; an answer starts the next.
    pub turn: usize,
    /// The project's name: its checkout's folder name.
    pub project: String,
    /// The person's checkout, which Coder never writes in.
    pub checkout: String,
    /// Coder's own worktree of the checkout, where it works.
    pub worktree: String,
    /// The commit the worktree started from.
    pub base: String,
    /// The provider's word: `codex`, `claude`, `grok`, `opencode`, or `devin`.
    pub provider: String,
    pub model: String,
    /// Why this provider, in a sentence ("Codex is signed in and has
    /// capacity", "Codex has reached its usage limit until …; using Claude
    /// Code").
    pub reason: String,
    /// The routes the run fails over to, `provider:model`, in order.
    pub fallbacks: Vec<String>,
    /// `local` when this computer started it for the person at it, `host`
    /// when a host's auto-start did.
    pub via: String,
    /// Who runs the turn and why, as the same prediction the offer showed
    /// ([`Runner`]). Absent from a start recorded before it existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<Runner>,
}

/// Which coding agent a Coder run on this computer will use, predicted
/// before it runs from what the run itself reads: which agents are signed
/// in here, the capacity book's refusals, and a fresh usage reading. The
/// chat shows it beside an offer to run Coder and on `coder_started`, so
/// every surface says the same thing with [`Runner::text`]. It names
/// providers, models, percents, and reset times; never a credential or an
/// account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Runner {
    /// `provider` will do the work. The routes before it in preference
    /// order were passed over, each with why.
    Runs {
        /// The provider's word: `codex`, `claude`, `grok`, `opencode`, or `devin`.
        provider: String,
        model: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        passed: Vec<Passed>,
        /// The provider the person asked for (#10076), as the router's
        /// typed `engine` reading named it on the offer, when they asked
        /// for one: first in `passed`, with why, when it does not run.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        requested: Option<String>,
    },
    /// None of the coding agents a run here may use (`providers`, in
    /// preference order) is signed in on this computer.
    NotSignedIn {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        providers: Vec<String>,
    },
    /// Every agent signed in here has a refusal that still holds; the
    /// earliest ends at `until`, Unix seconds.
    NoCapacity { until: Option<u64> },
}

/// A route passed over before the one that runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Passed {
    /// The provider's word: `codex`, `claude`, `grok`, `opencode`, or `devin`.
    pub provider: String,
    #[serde(flatten)]
    pub why: PassedOver,
}

/// Why a route was passed over.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum PassedOver {
    /// Its agent is not signed in on this computer.
    NotSignedIn,
    /// It refused for a usage or rate limit (`kind`, as the capacity book
    /// names it) that holds until `until`, Unix seconds.
    Refused { kind: String, until: u64 },
    /// A fresh usage reading puts its fullest window at `used_percent`,
    /// at or above the threshold.
    NearLimit { used_percent: u8 },
    /// The person asked for it, and the owner's Coder settings do not
    /// list it among the engines a run here may use (#10076).
    NotAllowed,
}

impl Passed {
    /// `Codex is at 92% of its window`, without a full stop.
    #[must_use]
    pub fn text(&self) -> String {
        let who = provider_name(&Value::String(self.provider.clone()));
        format!("{who} {}", self.clause())
    }

    /// Why it was passed over, without its name: `is at 92% of its
    /// window`.
    #[must_use]
    pub fn clause(&self) -> String {
        match &self.why {
            PassedOver::NotSignedIn => "is not signed in here".into(),
            PassedOver::Refused { kind, until } => format!(
                "reached its {} until {}",
                kind.replace('_', " "),
                utc(*until)
            ),
            PassedOver::NearLimit { used_percent } => {
                format!("is at {used_percent}% of its window")
            }
            PassedOver::NotAllowed => "is not one of the engines your Coder settings allow".into(),
        }
    }
}

/// The reason a run names when the person asked for an engine (#10076):
/// "You asked for Claude Code; it reached its usage limit until …, so
/// Codex {then}" when it does not run, "You asked for Claude Code; it
/// {then}" when it does. `None` when they asked for none. `then` is the
/// verb phrase of the surface: "will do this." on an offer, "is running."
/// on a start.
#[must_use]
pub fn requested_reason(
    provider: &str,
    passed: &[Passed],
    requested: Option<&str>,
    runs: &str,
    then: &str,
) -> Option<String> {
    let requested = requested?;
    let asked = provider_name(&Value::String(requested.to_owned()));
    if requested == provider {
        return Some(format!("You asked for {asked}; it {runs}"));
    }
    let who = provider_name(&Value::String(provider.to_owned()));
    let mut why: Vec<String> = Vec::new();
    match passed.iter().find(|p| p.provider == requested) {
        Some(own) => why.push(format!("it {}", own.clause())),
        None => why.push("it can't run here now".into()),
    }
    why.extend(
        passed
            .iter()
            .filter(|p| p.provider != requested)
            .map(Passed::text),
    );
    Some(format!(
        "You asked for {asked}; {}, so {who} {then}",
        why.join("; ")
    ))
}

impl Runner {
    /// The provider that will run, when one will.
    #[must_use]
    pub fn provider(&self) -> Option<&str> {
        match self {
            Runner::Runs { provider, .. } => Some(provider),
            _ => None,
        }
    }

    /// The sentence every surface shows: "Codex will do this.", "Codex is
    /// at 92% of its window; Claude Code will do this.", or why nothing
    /// can run here.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Runner::Runs {
                provider,
                passed,
                requested,
                ..
            } => {
                if let Some(text) = requested_reason(
                    provider,
                    passed,
                    requested.as_deref(),
                    "will do this.",
                    "will do this.",
                ) {
                    return text;
                }
                let who = provider_name(&Value::String(provider.clone()));
                if passed.is_empty() {
                    format!("{who} will do this.")
                } else {
                    let why: Vec<String> = passed.iter().map(Passed::text).collect();
                    format!("{}; {who} will do this.", why.join("; "))
                }
            }
            Runner::NotSignedIn { providers } => {
                let names: Vec<String> = providers
                    .iter()
                    .map(|p| provider_name(&Value::String(p.clone())))
                    .collect();
                match names.as_slice() {
                    [] => "Neither Codex nor Claude Code is signed in on this computer. \
                           Sign in to one to run Coder here."
                        .into(),
                    [one] => format!(
                        "{one} is not signed in on this computer. Sign in to it to run Coder here."
                    ),
                    [a, b] => format!(
                        "Neither {a} nor {b} is signed in on this computer. \
                         Sign in to one to run Coder here."
                    ),
                    many => format!(
                        "None of {} is signed in on this computer. Sign in to one to run Coder here.",
                        many.join(", ")
                    ),
                }
            }
            Runner::NoCapacity { until } => format!(
                "No coding agent signed in on this computer has room now{}.",
                until
                    .map(|at| format!("; the earliest resets {}", utc(at)))
                    .unwrap_or_default()
            ),
        }
    }
}

/// What a [`Step`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// The person's message that started the turn.
    Message,
    /// Coder's own note on what it does next and why.
    Thinking,
    /// A shell command Coder runs.
    Command,
    /// A tool an agent calls (Devin, OpenCode).
    ToolCall,
    /// What a command or tool returned, in a line.
    Observation,
    /// Coder's reply to the person, as it is written.
    Reply,
    /// Something the run says about itself, such as running without Jev.
    Note,
}

/// One thing Coder did or said, from one ATIF step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub turn: usize,
    /// The ATIF step it came from, in the turn's trajectory.
    pub step_id: u64,
    pub kind: StepKind,
    /// `user`, `agent`, or `system`, as ATIF names the source.
    pub source: String,
    /// At most [`MAX_TEXT`] bytes.
    pub text: String,
}

/// A command's output, bounded to [`MAX_OUTPUT`] bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub turn: usize,
    pub step_id: u64,
    pub command: String,
    /// `None` when a signal or the deadline ended it.
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub seconds: f64,
    pub text: String,
    /// The output was longer and was cut.
    pub truncated: bool,
}

/// The run moved off a provider that refused for a usage or rate limit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Switched {
    pub turn: usize,
    pub step_id: u64,
    /// `provider:model`.
    pub from: String,
    /// `provider:model`, or `None` when no admitted route had capacity.
    pub to: Option<String>,
    pub reason: String,
    /// When the refusing provider said its limit resets, Unix seconds.
    pub resets_at: Option<u64>,
}

/// Coder asked the person and waits. `answer` is how to reply from the
/// terminal; the apps answer from their own composer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asked {
    pub turn: usize,
    pub text: String,
    pub answer: Option<String>,
}

/// Where the loop is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    pub turn: usize,
    /// The loop's step, from one.
    pub step: usize,
    pub max_steps: Option<usize>,
    /// Seconds since the turn started.
    pub seconds: f64,
    /// Jev's probability that the task is done, when Jev judged.
    pub done: Option<f64>,
}

/// One file the turn changed in the worktree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    /// `added`, `modified`, `deleted`, or `renamed`.
    pub status: String,
    /// Lines added and removed; `None` for a binary file.
    pub added: Option<u64>,
    pub removed: Option<u64>,
}

/// The turn finished.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finished {
    pub turn: usize,
    /// Coder's reply: what it did and found.
    pub summary: String,
    pub files_changed: Vec<FileChange>,
    pub insertions: u64,
    pub deletions: u64,
    /// Where the changes are: Coder's worktree.
    pub worktree: String,
    /// The turn's ATIF trajectory file.
    pub trajectory: String,
    /// The GitHub issue the run worked, and how it landed, when the run
    /// was the issue flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<IssueLink>,
}

/// The GitHub issue an issue-flow run worked, for a result card: its
/// link, and what the run did with it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueLink {
    /// `owner/name`.
    pub repository: String,
    pub number: u64,
    pub url: String,
    pub title: String,
    /// `landed` (pushed to the default branch), `pull_request`,
    /// `unchanged`, `failed`, or `stopped`.
    pub outcome: String,
    /// The commits the run pushed, newest last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<String>,
    /// The pull request the run opened, in pull-request mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<String>,
    /// Whether the run closed the issue.
    pub closed: bool,
}

impl IssueLink {
    /// The issue and what happened to it, in a line.
    #[must_use]
    pub fn line(&self) -> String {
        let what = match self.outcome.as_str() {
            "landed" => match self.commits.last() {
                Some(commit) => format!(
                    "landed {} on the default branch{}",
                    &commit[..commit.len().min(10)],
                    if self.closed { " and closed" } else { "" }
                ),
                None => "landed".to_owned(),
            },
            "pull_request" => match &self.pull_request {
                Some(url) => format!("pull request {url}"),
                None => "pull request".to_owned(),
            },
            "unchanged" => "nothing changed; left open".to_owned(),
            "stopped" => "stopped; left open".to_owned(),
            _ => "not landed; left open with a comment".to_owned(),
        };
        format!("Issue #{} ({}): {what}", self.number, self.url)
    }
}

/// The turn ended without finishing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub turn: usize,
    pub message: String,
    /// The task's ending, such as `no_capacity` or `loop_incomplete`.
    pub ending: Option<String>,
    /// When a provider has capacity again, for `no_capacity`.
    pub resets_at: Option<u64>,
    /// The GitHub issue the run worked, when the run was the issue flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<IssueLink>,
}

/// The turn was stopped, by the person or the host's deadline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stopped {
    pub turn: usize,
    pub message: String,
}

/// One event as a stream carries it: `{"event": ..., "seq": ..., "task":
/// ..., "thread": ..., fields}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// From one, in the order the task produced them; a replay produces
    /// the same numbers.
    pub seq: u64,
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    #[serde(flatten)]
    pub event: CoderEvent,
}

/// Maps a turn's ATIF steps, in order, to events. One mapper follows one
/// turn; [`Mapper::end`] makes the event that ends it.
#[derive(Clone, Debug, Default)]
pub struct Mapper {
    turn: usize,
    /// How to answer a question from the terminal, when there is a way.
    answer: Option<String>,
    max_steps: Option<usize>,
    /// The reply as streamed so far this step.
    streamed: String,
    /// The last whole reply Coder wrote this turn.
    reply: String,
    /// The last loop ending the transcript named, with its detail.
    ending: Option<(String, Option<String>)>,
    /// What a whole coding agent's turn (Devin, OpenCode, Grok Build) says
    /// about how it stopped, from the adapter's summary.
    stopped: Option<String>,
    started: Option<u64>,
}

impl Mapper {
    /// A mapper for `turn`; `answer` is the command that answers a
    /// question (`openagents chat answer --thread ID TEXT`), if any.
    #[must_use]
    pub fn new(turn: usize, answer: Option<String>) -> Self {
        Mapper {
            turn,
            answer,
            ..Mapper::default()
        }
    }

    /// Coder's last whole reply this turn.
    #[must_use]
    pub fn reply(&self) -> &str {
        &self.reply
    }

    /// The events one step makes, in the ATIF document form
    /// (`crates/atif`'s `document`) or the log's record form: `extra` or
    /// `extensions` carry what Coder noted.
    pub fn step(&mut self, step: &Value) -> Vec<CoderEvent> {
        let id = step["step_id"].as_u64().unwrap_or(0);
        let source = step["source"]
            .as_str()
            .unwrap_or("system")
            .to_ascii_lowercase();
        let extra = step
            .get("extra")
            .filter(|extra| extra.is_object())
            .or_else(|| step.get("extensions"))
            .cloned()
            .unwrap_or(Value::Null);
        let at = step["timestamp"]
            .as_str()
            .and_then(parse_iso)
            .or_else(|| step["at"].as_u64());
        if self.started.is_none() {
            self.started = at;
        }
        let seconds = |fallback: Option<f64>| {
            fallback.unwrap_or_else(|| {
                self.started
                    .zip(at)
                    .map_or(0.0, |(start, at)| at.saturating_sub(start) as f64 / 1000.0)
            })
        };
        let make = |kind: StepKind, text: &str| {
            // The loop records the model's words and commands as its own
            // observations; they are the agent's.
            let source = match kind {
                StepKind::Thinking | StepKind::Command | StepKind::ToolCall | StepKind::Reply => {
                    "agent".to_owned()
                }
                StepKind::Message => source.clone(),
                StepKind::Observation | StepKind::Note => {
                    if source == "agent" {
                        "system".to_owned()
                    } else {
                        source.clone()
                    }
                }
            };
            CoderEvent::Step(Step {
                turn: self.turn,
                step_id: id,
                kind,
                source,
                text: bounded(text, MAX_TEXT).0,
            })
        };
        let mut events = Vec::new();
        if let Some(configuration) = extra
            .pointer("/admission/grant/adapter_configuration")
            .filter(|value| value.is_object())
        {
            self.max_steps = configuration["max_steps"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok());
            return events;
        }
        if let Some(summary) = extra.get("adapter_summary").and_then(Value::as_object) {
            self.stopped = summary
                .values()
                .filter(|agent| agent.get("engine").is_some())
                .find_map(|agent| agent.get("stopped").and_then(Value::as_str))
                .map(str::to_owned);
            return events;
        }
        if source == "user" {
            if let Some(text) = step["message"].as_str() {
                events.push(make(StepKind::Message, text));
            }
            return events;
        }
        if let Some(switch) = extra
            .get("route_switch")
            .or_else(|| extra.get("route_exhausted"))
        {
            let to = extra.get("route_switch").map(|_| route(&switch["to"]));
            let refusal = &switch["refusal"];
            let resets_at = refusal["resets_at"]
                .as_u64()
                .or_else(|| switch["resets_at"].as_u64())
                .or_else(|| refusal["until"].as_u64());
            let kind = refusal["kind"]
                .as_str()
                .unwrap_or("limit")
                .replace('_', " ");
            let from = route(&switch["from"]);
            let reason = format!(
                "{} refused for a {kind}{}",
                provider_name(&switch["from"]["provider"]),
                resets_at
                    .map(|at| format!(" until {}", utc(at)))
                    .unwrap_or_default()
            );
            events.push(CoderEvent::ProviderSwitched(Switched {
                turn: self.turn,
                step_id: id,
                from,
                to,
                reason,
                resets_at,
            }));
            return events;
        }
        if extra.get("decision_unavailable").is_some() || extra.get("routes_unavailable").is_some()
        {
            if let Some(text) = step["message"].as_str() {
                events.push(make(StepKind::Note, text));
            }
            return events;
        }
        if let Some(record) = extra.get("microcoder") {
            let event = &record["event"];
            let at_seconds = record["seconds"].as_f64();
            match event["event"].as_str() {
                Some("judged") => {
                    let done = event["judgment"]["answers"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|pair| pair[0] == "done")
                        .and_then(|pair| pair[1].as_f64());
                    events.push(CoderEvent::Progress(Progress {
                        turn: self.turn,
                        step: event["step"]
                            .as_u64()
                            .and_then(|n| usize::try_from(n).ok())
                            .unwrap_or(0),
                        max_steps: self.max_steps,
                        seconds: seconds(at_seconds),
                        done,
                    }));
                }
                Some("replying") => {
                    if let Some(text) = event["text"].as_str() {
                        self.streamed.push_str(text);
                        events.push(make(StepKind::Reply, text));
                    }
                }
                Some("generated") => {
                    let action = &event["generated"]["action"];
                    let action = action.get("Ok").unwrap_or(action);
                    if action.get("Err").is_some() || !action.is_object() {
                        self.streamed.clear();
                        return events;
                    }
                    if let Some(rationale) = action["rationale"].as_str()
                        && !rationale.trim().is_empty()
                    {
                        events.push(make(StepKind::Thinking, rationale));
                    }
                    let reply = action["reply"].as_str().unwrap_or("");
                    if !reply.trim().is_empty() {
                        // What streamed already showed; show only the rest.
                        let shown = event["reply_streamed"]
                            .as_u64()
                            .and_then(|n| usize::try_from(n).ok())
                            .filter(|n| reply.is_char_boundary(*n))
                            .unwrap_or(0);
                        let rest = &reply[shown..];
                        if !rest.trim().is_empty() {
                            events.push(make(StepKind::Reply, rest));
                        }
                        self.reply = reply.to_owned();
                    }
                    self.streamed.clear();
                    for command in action["commands"].as_array().into_iter().flatten() {
                        if let Some(command) = command.as_str() {
                            events.push(make(StepKind::Command, command));
                        }
                    }
                }
                Some("ran") => {
                    let result = &event["result"];
                    let command = result["command"].as_str().unwrap_or("").to_owned();
                    let exit = result["exit"].as_i64().and_then(|n| i32::try_from(n).ok());
                    let timed_out = result["timed_out"].as_bool().unwrap_or(false);
                    let took = result["seconds"].as_f64().unwrap_or(0.0);
                    // The command itself is the step before; `output` names it.
                    let summary = match (exit, timed_out) {
                        (_, true) => format!("timed out after {took:.1}s"),
                        (Some(code), _) => format!("exit {code} in {took:.1}s"),
                        (None, _) => format!("stopped after {took:.1}s"),
                    };
                    events.push(make(StepKind::Observation, &summary));
                    let (text, truncated) =
                        bounded(result["output"].as_str().unwrap_or(""), MAX_OUTPUT);
                    events.push(CoderEvent::Output(Output {
                        turn: self.turn,
                        step_id: id,
                        command,
                        exit,
                        timed_out,
                        seconds: took,
                        text,
                        truncated,
                    }));
                }
                Some("ended") => {
                    let ending = &event["outcome"]["ending"];
                    let reason = ending["reason"]
                        .as_str()
                        .or_else(|| ending.as_str())
                        .unwrap_or("ended")
                        .to_owned();
                    let detail = match &ending["detail"] {
                        Value::String(text) => Some(text.clone()),
                        Value::Null => None,
                        other => Some(other.to_string()),
                    };
                    self.ending = Some((reason, detail));
                }
                _ => {}
            }
            return events;
        }
        // A whole coding agent's turn (Devin, OpenCode): its tool calls,
        // their results, and its words.
        if source == "agent" {
            // A decision-model call (a Jev judgment such as
            // `openagents.microcoder.judge.v1`) is evidence for the
            // trajectory, never a row of the person's transcript (#10073).
            let decisions: Vec<&str> = step["tool_calls"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|call| is_decision_call(call))
                .filter_map(|call| call["tool_call_id"].as_str())
                .collect();
            for call in step["tool_calls"].as_array().into_iter().flatten() {
                if is_decision_call(call) {
                    continue;
                }
                let name = call["function_name"].as_str().unwrap_or("tool");
                let arguments = match &call["arguments"] {
                    Value::Null => String::new(),
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                events.push(make(StepKind::ToolCall, &format!("{name} {arguments}")));
            }
            for result in step
                .pointer("/observation/results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|result| {
                    !result["source_call_id"]
                        .as_str()
                        .is_some_and(|id| decisions.contains(&id))
                })
            {
                let content = match &result["content"] {
                    Value::String(text) => text.clone(),
                    Value::Null => continue,
                    other => other.to_string(),
                };
                events.push(make(StepKind::Observation, &content));
            }
            if let Some(reasoning) = step["reasoning_content"].as_str() {
                events.push(make(StepKind::Thinking, reasoning));
            }
            if let Some(text) = step["message"].as_str()
                && !text.trim().is_empty()
                && step["tool_calls"].is_null()
            {
                self.reply = text.to_owned();
                events.push(make(StepKind::Reply, text));
            }
        }
        events
    }

    /// The event that ends the turn, from the task's `ending` (the owner's
    /// result record). `changes` is what changed in `worktree`, and
    /// `trajectory` the turn's ATIF file; `resets_at` names when a provider
    /// has capacity again.
    #[must_use]
    pub fn end(
        &self,
        ending: &str,
        changes: Vec<FileChange>,
        worktree: &str,
        trajectory: &str,
        resets_at: Option<u64>,
    ) -> CoderEvent {
        let turn = self.turn;
        match ending {
            "model_finished" => {
                let insertions = changes.iter().filter_map(|c| c.added).sum();
                let deletions = changes.iter().filter_map(|c| c.removed).sum();
                CoderEvent::Result(Finished {
                    turn,
                    summary: self.reply.clone(),
                    files_changed: changes,
                    insertions,
                    deletions,
                    worktree: worktree.to_owned(),
                    trajectory: trajectory.to_owned(),
                    issue: None,
                })
            }
            "asked_question" => CoderEvent::Question(Asked {
                turn,
                text: self.reply.clone(),
                answer: self.answer.clone(),
            }),
            "asked_approval" => CoderEvent::Approval(Asked {
                turn,
                text: self.reply.clone(),
                answer: self.answer.clone(),
            }),
            "cancelled_or_host_refusal" | "cancelled" => CoderEvent::Stopped(Stopped {
                turn,
                message: "Coder stopped: the task was cancelled or reached its time limit.".into(),
            }),
            // The agent ended the turn itself: after the host refused a
            // tool it asked for, or reporting `cancelled` with no stop from
            // the host. Neither is a cancel or a time limit (#10092).
            "engine_stopped_after_refusal" | "engine_cancelled" => CoderEvent::Stopped(Stopped {
                turn,
                message: format!(
                    "Coder stopped: {}",
                    self.stopped.clone().unwrap_or_else(|| if ending
                        == "engine_cancelled"
                    {
                        "the coding agent ended the turn as cancelled on its own; nobody stopped the task and it did not reach its time limit.".to_owned()
                    } else {
                        "the coding agent stopped after the host refused a tool it asked to run.".to_owned()
                    })
                ),
            }),
            "no_capacity" => CoderEvent::Failure(Failure {
                turn,
                message: format!(
                    "No admitted provider has capacity{}.",
                    resets_at
                        .map(|at| format!("; the earliest resets {}", utc(at)))
                        .unwrap_or_default()
                ),
                ending: Some(ending.into()),
                resets_at,
                issue: None,
            }),
            other => {
                let why = match &self.ending {
                    Some((reason, Some(detail))) => {
                        format!("{} ({})", reason.replace('_', " "), bounded(detail, 300).0)
                    }
                    Some((reason, None)) => reason.replace('_', " "),
                    None => match &self.stopped {
                        Some(stopped) => stopped.trim_end_matches('.').to_owned(),
                        None => other.replace('_', " "),
                    },
                };
                CoderEvent::Failure(Failure {
                    turn,
                    message: format!("Coder stopped before finishing: {why}."),
                    ending: Some(other.into()),
                    resets_at: None,
                    issue: None,
                })
            }
        }
    }
}

/// The result of a task's last turn, from its events, once that turn has
/// ended with a result, a failure, or a stop (#10094): the context a
/// follow-up carries to the chat ([`crate::router::CoderRun`]). `None`
/// while the turn runs, when it ended by asking the person, and for a task
/// with no ended turn. The engine is the one the turn started on, or the
/// one it switched to last.
#[must_use]
pub fn run_result(lines: &[Line]) -> Option<crate::router::CoderRun> {
    use crate::router::{CoderRun, RunEnding, RunFile};
    let ended = lines.iter().rposition(|line| line.event.ends_turn())?;
    // A later start is a turn that has not ended.
    if lines[ended + 1..]
        .iter()
        .any(|line| matches!(line.event, CoderEvent::CoderStarted(_)))
    {
        return None;
    }
    let (turn, ending, summary, files) = match &lines[ended].event {
        CoderEvent::Result(result) => (
            result.turn,
            RunEnding::Finished,
            result.summary.clone(),
            result
                .files_changed
                .iter()
                .map(|file| RunFile {
                    path: file.path.clone(),
                    status: file.status.clone(),
                })
                .collect(),
        ),
        CoderEvent::Failure(failure) => (
            failure.turn,
            RunEnding::Failed,
            failure.message.clone(),
            Vec::new(),
        ),
        CoderEvent::Stopped(stopped) => (
            stopped.turn,
            RunEnding::Stopped,
            stopped.message.clone(),
            Vec::new(),
        ),
        _ => return None,
    };
    let this_turn = &lines[..ended];
    let mut engine = None;
    let mut model = None;
    let mut commands = Vec::new();
    for line in this_turn {
        match &line.event {
            CoderEvent::CoderStarted(started) if started.turn == turn => {
                engine = Some(started.provider.clone());
                model = Some(started.model.clone());
            }
            CoderEvent::ProviderSwitched(switch) if switch.turn == turn => {
                if let Some((provider, to)) = switch.to.as_deref().and_then(|to| to.split_once(':'))
                {
                    engine = Some(provider.to_owned());
                    model = Some(to.to_owned());
                }
            }
            CoderEvent::Step(step) if step.turn == turn && step.kind == StepKind::Command => {
                commands.push(step.text.clone());
            }
            _ => {}
        }
    }
    Some(CoderRun {
        ending,
        turn,
        engine,
        model,
        summary,
        files,
        commands,
    })
}

/// A compact line for a terminal, or `None` for an event a terminal shows
/// no line for.
#[must_use]
pub fn text(event: &CoderEvent) -> Option<String> {
    let first = |text: &str| -> String {
        let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        bounded(line.trim(), 200).0
    };
    Some(match event {
        CoderEvent::CoderStarted(s) => format!(
            "Coder started turn {} in {} ({}) on {}:{}. {}",
            s.turn, s.project, s.worktree, s.provider, s.model, s.reason
        ),
        CoderEvent::Step(step) => match step.kind {
            StepKind::Message => return None,
            StepKind::Thinking => format!("  · {}", first(&step.text)),
            StepKind::Command => format!("  $ {}", first(&step.text)),
            StepKind::ToolCall => format!("  > {}", first(&step.text)),
            StepKind::Observation => format!("    {}", first(&step.text)),
            StepKind::Reply => return None,
            StepKind::Note => format!("  ! {}", first(&step.text)),
        },
        CoderEvent::Output(output) => {
            let lines: Vec<&str> = output.text.lines().collect();
            let tail = lines.len().saturating_sub(3);
            lines[tail..]
                .iter()
                .map(|line| format!("    | {}", bounded(line, 160).0))
                .collect::<Vec<_>>()
                .join("\n")
        }
        CoderEvent::ProviderSwitched(s) => match &s.to {
            Some(to) => format!("  ~ {}; switching from {} to {to}", s.reason, s.from),
            None => format!("  ~ {}; no other provider has capacity", s.reason),
        },
        CoderEvent::Progress(p) => format!(
            "  [step {}{}{}, {:.0}s]",
            p.step,
            p.max_steps.map(|m| format!("/{m}")).unwrap_or_default(),
            p.done
                .map(|d| format!(", done {:.0}%", d * 100.0))
                .unwrap_or_default(),
            p.seconds
        ),
        CoderEvent::Question(a) | CoderEvent::Approval(a) => format!(
            "Coder asks: {}{}",
            a.text.trim(),
            a.answer
                .as_ref()
                .map(|how| format!("\nanswer: {how}"))
                .unwrap_or_default()
        ),
        CoderEvent::Result(r) => {
            let mut out = format!(
                "Coder finished turn {}: {} file{} changed, +{} -{} in {}",
                r.turn,
                r.files_changed.len(),
                if r.files_changed.len() == 1 { "" } else { "s" },
                r.insertions,
                r.deletions,
                r.worktree
            );
            for file in &r.files_changed {
                out.push_str(&format!(
                    "\n  {} {} (+{} -{})",
                    file.status,
                    file.path,
                    file.added.map_or("?".into(), |n| n.to_string()),
                    file.removed.map_or("?".into(), |n| n.to_string())
                ));
            }
            if let Some(issue) = &r.issue {
                out.push_str(&format!("\n{}", issue.line()));
            }
            out
        }
        CoderEvent::Failure(f) => match &f.issue {
            Some(issue) => format!("{}\n{}", f.message, issue.line()),
            None => f.message.clone(),
        },
        CoderEvent::Stopped(s) => s.message.clone(),
    })
}

/// `provider:model` of a route as a grant or transcript names it.
fn route(value: &Value) -> String {
    match (value["provider"].as_str(), value["model"].as_str()) {
        (Some(provider), Some(model)) => format!("{provider}:{model}"),
        (Some(provider), None) => provider.into(),
        _ => value.as_str().unwrap_or("unknown").into(),
    }
}

/// The provider's product name.
#[must_use]
pub fn provider_name(provider: &Value) -> String {
    match provider.as_str() {
        Some("codex") => "Codex".into(),
        Some("claude") => "Claude Code".into(),
        Some("devin") => "Devin".into(),
        Some("opencode") => "OpenCode".into(),
        Some("grok") => "Grok Build".into(),
        Some(other) => other.into(),
        None => "The provider".into(),
    }
}

/// At most `max` bytes of `text`, cut at a character boundary, and whether
/// it was cut.
fn bounded(text: &str, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text.to_owned(), false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}…", &text[..end]), true)
}

/// Unix seconds as `2026-10-04 13:27 UTC`.
#[must_use]
pub fn utc(seconds: u64) -> String {
    let iso = atif::iso(seconds.saturating_mul(1000));
    // `2026-10-04T13:27:03.000Z`
    match (iso.get(..10), iso.get(11..16)) {
        (Some(day), Some(time)) => format!("{day} {time} UTC"),
        _ => iso,
    }
}

/// Milliseconds of an ISO time `crates/atif` wrote (`…T…Z`).
fn parse_iso(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    let millis = text
        .get(20..23)
        .and_then(|part| part.parse::<i64>().ok())
        .unwrap_or(0);
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let ms = ((days * 86_400 + hour * 3600 + minute * 60 + second) * 1000) + millis;
    u64::try_from(ms).ok()
}

/// Whether an ATIF document's tool call is a decision-model call: its
/// `extra` names [`atif::DECISION_CALL_SCHEMA`], as `atif::Call::is_decision`
/// reads the log's record.
fn is_decision_call(call: &Value) -> bool {
    call.pointer("/extra/schema").and_then(Value::as_str) == Some(atif::DECISION_CALL_SCHEMA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(id: u64, source: &str, message: &str, extra: Value) -> Value {
        json!({"step_id": id, "timestamp": format!("2026-09-30T12:00:{:02}.000Z", id), "source": source, "message": message, "extra": extra})
    }

    fn mc(event: Value) -> Value {
        json!({"microcoder": {"seconds": 1.5, "event": event}})
    }

    /// A turn as a scripted provider writes it: a failover, a command, its
    /// output, a streamed reply, and the end.
    fn scripted() -> Vec<Value> {
        vec![
            step(1, "user", "add a unit test for slugify", json!({})),
            step(
                2,
                "system",
                "Repository adapter admitted by the local operator.",
                json!({"admission": {"grant": {"adapter_configuration": {"max_steps": 24}}}}),
            ),
            step(
                3,
                "system",
                "judged",
                mc(json!({"event": "judged", "step": 1,
                "judgment": {"answers": [["done", 0.02], ["progress", 0.5]]}})),
            ),
            step(
                4,
                "system",
                "The provider refused for a usage or rate limit; the run switches to the next admitted route.",
                json!({"route_switch": {"from": {"provider": "codex", "model": "gpt-6-luna"},
                    "to": {"provider": "claude", "model": "claude-opus-5-5"},
                    "refusal": {"provider": "codex", "kind": "usage_limit", "resets_at": 1_791_050_823u64, "until": 1_791_050_823u64}}}),
            ),
            step(
                5,
                "system",
                "generated",
                mc(json!({"event": "generated", "step": 1,
                "generated": {"action": {"Ok": {"rationale": "Write the test file.", "commands": ["cat > test_slug.py <<'EOF'\nEOF", "python3 -m unittest"], "finished": false, "reply": ""}}}})),
            ),
            step(
                6,
                "system",
                "ran",
                mc(json!({"event": "ran", "step": 1,
                "result": {"command": "python3 -m unittest", "exit": 0, "timed_out": false, "seconds": 0.2, "output": "OK\n"}})),
            ),
            step(
                7,
                "system",
                "replying",
                mc(json!({"event": "replying", "text": "I added "})),
            ),
            step(
                8,
                "system",
                "generated",
                mc(json!({"event": "generated", "step": 2, "reply_streamed": 8,
                "generated": {"action": {"Ok": {"rationale": "Done.", "commands": [], "finished": true, "reply": "I added test_slug.py; it passes."}}}})),
            ),
            step(
                9,
                "system",
                "ended",
                mc(json!({"event": "ended", "outcome": {"ending": "finished", "steps": 2}})),
            ),
        ]
    }

    #[test]
    fn a_scripted_turn_maps_to_every_step_event_in_order() {
        let mut mapper = Mapper::new(1, Some("openagents chat answer --thread T TEXT".into()));
        let events: Vec<CoderEvent> = scripted().iter().flat_map(|s| mapper.step(s)).collect();
        let names: Vec<&str> = events.iter().map(CoderEvent::name).collect();
        assert_eq!(
            names,
            [
                "step",
                "progress",
                "provider_switched",
                "step",
                "step",
                "step",
                "step",
                "output",
                "step",
                "step",
                "step"
            ]
        );
        let CoderEvent::ProviderSwitched(switch) = &events[2] else {
            panic!()
        };
        assert_eq!(switch.from, "codex:gpt-6-luna");
        assert_eq!(switch.to.as_deref(), Some("claude:claude-opus-5-5"));
        assert!(
            switch
                .reason
                .starts_with("Codex refused for a usage limit until 2026-")
        );
        let CoderEvent::Progress(progress) = &events[1] else {
            panic!()
        };
        assert_eq!((progress.step, progress.max_steps), (1, Some(24)));
        // The streamed start of the reply is not repeated.
        let replies: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                CoderEvent::Step(s) if s.kind == StepKind::Reply => Some(s.text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(replies, ["I added ", "test_slug.py; it passes."]);
        let end = mapper.end(
            "model_finished",
            vec![FileChange {
                path: "test_slug.py".into(),
                status: "added".into(),
                added: Some(9),
                removed: Some(0),
            }],
            "/w",
            "/s/t.1.atif.jsonl",
            None,
        );
        let CoderEvent::Result(result) = &end else {
            panic!()
        };
        assert_eq!(result.summary, "I added test_slug.py; it passes.");
        assert_eq!((result.insertions, result.deletions), (9, 0));
        assert!(end.ends_turn());
    }

    #[test]
    fn endings_map_to_their_events() {
        let mapper = Mapper::new(2, Some("answer here".into()));
        let end = |ending: &str| mapper.end(ending, vec![], "/w", "/t", Some(1_791_050_823));
        assert_eq!(end("asked_question").name(), "question");
        assert_eq!(end("asked_approval").name(), "approval");
        assert_eq!(end("cancelled_or_host_refusal").name(), "stopped");
        let CoderEvent::Failure(failure) = end("no_capacity") else {
            panic!()
        };
        assert_eq!(failure.resets_at, Some(1_791_050_823));
        assert_eq!(end("loop_incomplete").name(), "failure");
        let CoderEvent::Question(asked) = end("asked_question") else {
            panic!()
        };
        assert_eq!(asked.answer.as_deref(), Some("answer here"));
        let CoderEvent::Stopped(stopped) = end("engine_cancelled") else {
            panic!()
        };
        assert!(
            !stopped.message.contains("cancelled or reached"),
            "{}",
            stopped.message
        );
        assert!(stopped.message.contains("on its own"));
    }

    /// A whole coding agent's turn that the agent ended after the host
    /// refused a tool says what happened, never "cancelled or reached its
    /// time limit" (#10092).
    #[test]
    fn an_agent_that_stopped_after_a_refusal_says_so() {
        let mut mapper = Mapper::new(1, None);
        let summary = json!({"step_id": 9, "source": "system", "message": "Repository adapter ended; independent checks are separate.",
            "extensions": {"adapter_summary": {"configuration": {"provider": "grok"},
                "grok": {"engine": "grok-acp", "stop_reason": "cancelled",
                    "stopped": "Grok Build stopped after the host refused a tool it asked to run (Write /etc/hosts: it would write /etc/hosts, outside the workspace)."}}}});
        assert!(mapper.step(&summary).is_empty());
        let CoderEvent::Stopped(stopped) =
            mapper.end("engine_stopped_after_refusal", vec![], "/w", "/t", None)
        else {
            panic!()
        };
        assert_eq!(
            stopped.message,
            "Coder stopped: Grok Build stopped after the host refused a tool it asked to run (Write /etc/hosts: it would write /etc/hosts, outside the workspace)."
        );
        let CoderEvent::Failure(failure) =
            mapper.end("engine_incomplete", vec![], "/w", "/t", None)
        else {
            panic!()
        };
        assert!(
            failure.message.contains("Grok Build stopped after"),
            "{}",
            failure.message
        );
    }

    #[test]
    fn a_line_round_trips_with_its_envelope() {
        let line = Line {
            seq: 3,
            task: "t".into(),
            thread: Some("a".repeat(32)),
            event: CoderEvent::Stopped(Stopped {
                turn: 1,
                message: "stopped".into(),
            }),
        };
        let text = serde_json::to_string(&line).unwrap();
        assert!(text.starts_with("{\"seq\":3,\"task\":\"t\",\"thread\":"));
        assert!(text.contains("\"event\":\"stopped\""));
        let back: Line = serde_json::from_str(&text).unwrap();
        assert_eq!(back, line);
    }

    /// A decision call in the exported ATIF document (a Jev judgment the
    /// loop asked) is not a transcript row; a real tool call still is
    /// (#10073).
    #[test]
    fn a_decision_call_is_not_a_transcript_row() {
        let mut mapper = Mapper::new(1, None);
        let judged = json!({
            "step_id": 2,
            "source": "agent",
            "message": "",
            "tool_calls": [{
                "tool_call_id": "decision-openagents.microcoder.judge.v1",
                "function_name": "openagents.microcoder.judge.v1",
                "arguments": {"model": "jev-1.13.0", "state": {"task": "t"}},
                "extra": {"schema": atif::DECISION_CALL_SCHEMA, "model": "jev-1.13.0"}
            }],
            "observation": {"results": [{
                "source_call_id": "decision-openagents.microcoder.judge.v1",
                "content": "{\"done\":{\"noul\":0.21}}"
            }]}
        });
        assert!(mapper.step(&judged).is_empty());
        let tool = json!({
            "step_id": 3,
            "source": "agent",
            "message": "",
            "tool_calls": [{"tool_call_id": "c1", "function_name": "read", "arguments": "a.rs"}],
            "observation": {"results": [{"source_call_id": "c1", "content": "fn main() {}"}]}
        });
        let events = mapper.step(&tool);
        assert!(
            matches!(&events[0], CoderEvent::Step(s) if s.kind == StepKind::ToolCall && s.text == "read a.rs")
        );
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn output_is_bounded_and_times_parse() {
        let mut mapper = Mapper::new(1, None);
        let long = "x".repeat(MAX_OUTPUT * 2);
        let events = mapper.step(&step(
            1,
            "system",
            "ran",
            mc(json!({"event": "ran", "step": 1, "result": {"command": "yes", "exit": null, "timed_out": true, "seconds": 3.0, "output": long}})),
        ));
        let CoderEvent::Output(output) = &events[1] else {
            panic!()
        };
        assert!(output.truncated && output.text.len() <= MAX_OUTPUT + 3);
        assert_eq!(parse_iso("1970-01-01T00:00:01.500Z"), Some(1500));
        assert_eq!(utc(1_791_050_823), "2026-10-03 18:07 UTC");
    }

    /// When the person asked for an engine, the runner says so first, and
    /// why another runs when it does (#10076).
    #[test]
    fn a_runner_states_the_engine_the_person_asked_for() {
        let honored = Runner::Runs {
            provider: "claude".into(),
            model: "claude-opus-5-5".into(),
            passed: vec![],
            requested: Some("claude".into()),
        };
        assert_eq!(
            honored.text(),
            "You asked for Claude Code; it will do this."
        );
        let wire = serde_json::to_value(&honored).unwrap();
        assert_eq!(wire["requested"], "claude");
        assert_eq!(serde_json::from_value::<Runner>(wire).unwrap(), honored);
        let limited = Runner::Runs {
            provider: "codex".into(),
            model: "gpt-6-luna".into(),
            passed: vec![Passed {
                provider: "claude".into(),
                why: PassedOver::Refused {
                    kind: "usage_limit".into(),
                    until: 1_791_050_823,
                },
            }],
            requested: Some("claude".into()),
        };
        assert_eq!(
            limited.text(),
            "You asked for Claude Code; it reached its usage limit until 2026-10-03 18:07 UTC, \
             so Codex will do this."
        );
        for (why, clause) in [
            (PassedOver::NotSignedIn, "it is not signed in here"),
            (
                PassedOver::NotAllowed,
                "it is not one of the engines your Coder settings allow",
            ),
            (
                PassedOver::NearLimit { used_percent: 95 },
                "it is at 95% of its window",
            ),
        ] {
            let runner = Runner::Runs {
                provider: "codex".into(),
                model: "gpt-6-luna".into(),
                passed: vec![Passed {
                    provider: "grok".into(),
                    why,
                }],
                requested: Some("grok".into()),
            };
            assert_eq!(
                runner.text(),
                format!("You asked for Grok Build; {clause}, so Codex will do this.")
            );
        }
        // An old runner, without the field, reads as no request.
        let old: Runner = serde_json::from_value(
            json!({"state": "runs", "provider": "codex", "model": "gpt-6-luna"}),
        )
        .unwrap();
        assert_eq!(old.text(), "Codex will do this.");
    }

    /// The three things an offer can say, from their typed fields, and
    /// the wire form a phone reads back.
    #[test]
    fn a_runner_says_who_will_run_or_why_none_can() {
        let codex = Runner::Runs {
            provider: "codex".into(),
            model: "gpt-6-luna".into(),
            passed: vec![],
            requested: None,
        };
        assert_eq!(codex.text(), "Codex will do this.");
        assert_eq!(codex.provider(), Some("codex"));
        assert_eq!(
            serde_json::to_value(&codex).unwrap(),
            json!({"state": "runs", "provider": "codex", "model": "gpt-6-luna"})
        );

        let claude = Runner::Runs {
            provider: "claude".into(),
            model: "claude-opus-5-5".into(),
            passed: vec![Passed {
                provider: "codex".into(),
                why: PassedOver::NearLimit { used_percent: 92 },
            }],
            requested: None,
        };
        assert_eq!(
            claude.text(),
            "Codex is at 92% of its window; Claude Code will do this."
        );
        let wire = serde_json::to_value(&claude).unwrap();
        assert_eq!(
            wire["passed"],
            json!([{"provider": "codex", "why": "near_limit", "used_percent": 92}])
        );
        assert_eq!(serde_json::from_value::<Runner>(wire).unwrap(), claude);
        let refused = Runner::Runs {
            provider: "claude".into(),
            model: "claude-opus-5-5".into(),
            passed: vec![Passed {
                provider: "codex".into(),
                why: PassedOver::Refused {
                    kind: "usage_limit".into(),
                    until: 1_791_050_823,
                },
            }],
            requested: None,
        };
        assert_eq!(
            refused.text(),
            "Codex reached its usage limit until 2026-10-03 18:07 UTC; Claude Code will do this."
        );

        let nobody = Runner::NotSignedIn {
            providers: vec!["codex".into(), "claude".into()],
        };
        assert_eq!(
            nobody.text(),
            "Neither Codex nor Claude Code is signed in on this computer. \
             Sign in to one to run Coder here."
        );
        assert_eq!(nobody.provider(), None);
        assert_eq!(
            serde_json::to_value(&nobody).unwrap(),
            json!({"state": "not_signed_in", "providers": ["codex", "claude"]})
        );
        assert_eq!(
            Runner::NotSignedIn {
                providers: vec!["claude".into()]
            }
            .text(),
            "Claude Code is not signed in on this computer. Sign in to it to run Coder here."
        );
        assert_eq!(
            Runner::NoCapacity {
                until: Some(1_791_050_823)
            }
            .text(),
            "No coding agent signed in on this computer has room now; \
             the earliest resets 2026-10-03 18:07 UTC."
        );
    }

    fn at(seq: u64, event: CoderEvent) -> Line {
        Line {
            seq,
            task: "t".into(),
            thread: None,
            event,
        }
    }

    fn started(turn: usize, provider: &str, model: &str) -> CoderEvent {
        CoderEvent::CoderStarted(Started {
            turn,
            project: "p".into(),
            checkout: "/c".into(),
            worktree: "/w".into(),
            base: "abc".into(),
            provider: provider.into(),
            model: model.into(),
            reason: "ready".into(),
            fallbacks: vec![],
            via: "local".into(),
            runner: None,
        })
    }

    fn command(turn: usize, text: &str) -> CoderEvent {
        CoderEvent::Step(Step {
            turn,
            step_id: 1,
            kind: StepKind::Command,
            source: "agent".into(),
            text: text.into(),
        })
    }

    /// A follow-up after a run carries the last turn's result: how it
    /// ended, the engine it switched to, its summary, files, and commands;
    /// a turn that runs or asks carries none (#10094).
    #[test]
    fn a_run_result_is_the_last_ended_turn() {
        use crate::router::{RunEnding, RunFile};
        let mut lines = vec![
            at(1, started(1, "claude", "claude-opus-5-5")),
            at(
                2,
                CoderEvent::ProviderSwitched(Switched {
                    turn: 1,
                    step_id: 2,
                    from: "claude:claude-opus-5-5".into(),
                    to: Some("codex:gpt-6-luna".into()),
                    reason: "Claude Code is at 99%".into(),
                    resets_at: None,
                }),
            ),
            at(3, command(1, "ls\nsecond line")),
            at(
                4,
                CoderEvent::Result(Finished {
                    turn: 1,
                    summary: "I listed the files.".into(),
                    files_changed: vec![FileChange {
                        path: "NOTE.md".into(),
                        status: "added".into(),
                        added: Some(3),
                        removed: Some(0),
                    }],
                    insertions: 3,
                    deletions: 0,
                    worktree: "/w".into(),
                    trajectory: "/t".into(),
                    issue: None,
                }),
            ),
        ];
        let run = run_result(&lines).unwrap();
        assert_eq!(run.ending, RunEnding::Finished);
        assert_eq!(run.turn, 1);
        assert_eq!(run.engine.as_deref(), Some("codex"));
        assert_eq!(run.model.as_deref(), Some("gpt-6-luna"));
        assert_eq!(run.summary, "I listed the files.");
        assert_eq!(
            run.files,
            vec![RunFile {
                path: "NOTE.md".into(),
                status: "added".into()
            }]
        );
        let json = run.json();
        assert_eq!(json["commands"], json!(["ls"]));
        assert_eq!(json["engine"], "codex");
        // The next turn runs: no result until it ends.
        lines.push(at(5, started(2, "codex", "gpt-6-luna")));
        assert!(run_result(&lines).is_none());
        lines.push(at(
            6,
            CoderEvent::Question(Asked {
                turn: 2,
                text: "Which crate?".into(),
                answer: None,
            }),
        ));
        assert!(
            run_result(&lines).is_none(),
            "a question waits for an answer"
        );
        lines.push(at(
            7,
            CoderEvent::Stopped(Stopped {
                turn: 2,
                message: "Stopped from the desktop.".into(),
            }),
        ));
        let stopped = run_result(&lines).unwrap();
        assert_eq!(stopped.ending, RunEnding::Stopped);
        assert_eq!(stopped.turn, 2);
        assert!(stopped.commands.is_empty());
        assert!(run_result(&[]).is_none());
    }
}
