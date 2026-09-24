//! The Microluna executor: short GPT-6 Luna sessions in this process.
//!
//! [`Micro`] is an [`Executor`] beside the Claude Code and Codex adapters,
//! so every policy, record, and Gym view that reads a dispatch reads this
//! one too. It spawns no CLI: each session is a `microluna` session on the
//! operator's Codex login, with Microluna's five native function tools.
//!
//! # Two modes
//!
//! - **`single`** runs one session on the briefing, as a CLI would.
//! - **`requirements`** is the mini-handoff loop of the Luna pivot
//!   (`docs/coder/design/luna-pivot.md`). The requirement map Jev extracted
//!   is split into a few groups. Each group gets short sessions whose
//!   context is rebuilt from scratch: the task first, so every session of
//!   the task shares the cached prefix, then the group's requirements and
//!   only the evidence that informs them, then the current state. After
//!   each session, code runs the checks, asks for the combined verdict
//!   (`checks::verdict`, issue #9584) over the session's report, and Jev
//!   chooses the next move: `next`, `retry`, `stuck`, or `done`. Code keeps
//!   the last word: a move past a requirement a check contradicts becomes a
//!   retry, a verdict of fail keeps the loop from ending, and every loop is
//!   bounded by sessions, spend, and time.
//!
//! # The record
//!
//! Each session is a `microluna.session` invocation. Its tool calls and
//! replies land in the episode log as normalized executor events, the
//! shape the CLI adapters record, so `gym runs show --transcript` and the
//! head-to-head replay show them. Microluna's own ATIF steps, with exact
//! usage and cost per request, go to `artifacts/microluna-<d>-<n>.atif.jsonl`
//! beside them. Each move between sessions is a Jev decision and a
//! `handoff` step, which the Gym shows as a hand-off.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use atif::document::{Source, Step};
use microluna::codex::{CodexTransport, Login};
use microluna::fake::FakeTransport;
use microluna::{Brief, Config, Ending, Evidence, Isolation, TokenUsage, Transport};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::checks::{self, Subject};
use crate::component::jev as jev_component;
use crate::delegate::{Briefing, Executor, Prepared, Report, Status, Summary};
use crate::record::{Finish, Implementation, Outcome, Recorder, Start};
use crate::requirements::Kind;
use crate::session::{self, Observation, Version};
use crate::stream::{Event, Kind as EventKind};

/// The agent's name in the record.
pub const AGENT: &str = "microluna";

/// The component each session is recorded under.
pub const SESSION_COMPONENT: &str = "microluna.session";

/// The component each move between sessions is recorded under.
pub const HANDOFF_COMPONENT: &str = "microluna.handoff";

/// The schema of the loop's record.
pub const LOOP_SCHEMA: &str = "openagents.coder-one.microluna-loop.v1";

/// The Jev question that picks the move after a session.
pub const MOVE_QUESTION: &str = "Session `session.number` of a coding agent worked only on the requirements in `focus` of the task in `task`. Read what the session reported in `session` and what the host's checks observed afterwards in `checks`. What should happen next?";

/// The four moves, in the order the question lists them.
pub const MOVES: [(&str, &str); 4] = [
    (
        "next",
        "The requirements in `focus` are met, or as met as the workspace can show, and the task has other requirements left: move on to the next ones.",
    ),
    (
        "retry",
        "The requirements in `focus` are not met yet, and another short session that starts from the current state could make progress on them.",
    ),
    (
        "stuck",
        "The requirements in `focus` are not met, and another session is unlikely to help: the session was blocked, repeated a failed approach, or lacks something it can't get.",
    ),
    (
        "done",
        "The whole task in `task` is complete: every requirement is met, so no more sessions are needed.",
    ),
];

/// How the executor runs a dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// One session on the briefing.
    Single,
    /// The mini-handoff loop over the requirement map.
    Requirements,
}

impl Mode {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Mode::Single => "single",
            Mode::Requirements => "requirements",
        }
    }
}

/// The manifest's `executor.microluna`: the loop's shape and bounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub mode: Mode,
    /// The most sessions one dispatch runs.
    pub max_sessions: u32,
    /// The most sessions one requirement group gets.
    pub max_attempts: u32,
    /// The most requirement groups: a map with more requirements puts
    /// several in a group.
    pub max_groups: u32,
    /// The most model requests one session makes.
    pub session_turns: usize,
    /// One session's wall-time bound, in seconds.
    pub session_sec: u64,
    /// The dispatch's spend bound in dollars: Luna's list-price estimate
    /// plus Jev's.
    pub spend_usd: f64,
    /// The most characters of evidence one session's brief carries.
    pub evidence_chars: usize,
    /// Whether the checks run between sessions.
    pub checks: bool,
    /// Require evidence before a session's move ends the loop or advances
    /// a group: the session must have edited a file and run a command
    /// after its last edit. A `done` or advancing move without both is a
    /// retry. This is Fable's move that Luna lacks: a claim of done backed
    /// by an edit and a test, not by reading.
    #[serde(default = "yes")]
    pub require_evidence: bool,
    /// Run the first session of each group read-only: it reproduces and
    /// runs the task's own tests before any edit, then always retries into
    /// an edit session, without spending one of the group's attempts. This
    /// is Fable's move of reading longer before the first edit.
    #[serde(default)]
    pub read_first: bool,
    /// Gate a loop-ending move on the checks: the loop ends, or advances
    /// past the last group, only when the checks positively confirm the
    /// focus (every covered requirement observed, none contradicted). The
    /// thesis's "done is a program state, not a model's opinion"
    /// (`docs/coder/design/thesis.md`). A minimal acceptance gate until
    /// #9588's accept.define lands.
    #[serde(default)]
    pub accept: bool,
    /// Group only the requirements a session edits toward — behaviors,
    /// deliverables, and checks — and carry every constraint into each
    /// session's brief as a standing rule instead of grouping it. A
    /// constraint such as "set the random seed to 149" or "you have 28800
    /// seconds" is nothing a session can complete on its own, so a group
    /// of constraints spends sessions that make no edit and get downgraded
    /// to retry then stuck. Off keeps every non-context requirement in a
    /// group, as v1 through v4 do.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub focus_actionable: bool,
    /// Drive the loop by an acceptance suite (`accept.define`, issue
    /// #9588): before any fix, a Microluna session writes executable tests
    /// from the task, code proves them red on the untouched workspace and
    /// Jev checks them, and the suite is frozen. Then edit sessions run,
    /// each briefed with the frozen tests and the red tests' output, until
    /// the suite is green or a bound is hit. Done is the suite green, not a
    /// session's report. When the suite comes out with no tests, the
    /// dispatch falls back to the requirements loop.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub suite: bool,
    /// How the suite loop's `accept.define` writes the suite; absent, as
    /// v6 does: one writer, rewrites on any problem, three rounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suite_writer: Option<SuiteWriter>,
}

/// `executor.microluna.suite_writer`: how the suite is written, for a
/// suite that is on the critical path in seconds rather than minutes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuiteWriter {
    /// Writers in the first round, each on a consecutive share of the
    /// decidable requirements, at the same time; their suites merge into
    /// one that is verified once (`accept::Options::writers`).
    pub writers: u32,
    /// What sends the suite back to a writer: `any` problem, or only
    /// `hard` ones code checks, with Jev's doubts kept as notes on the
    /// tests (`accept::Rewrite`).
    pub rewrite: crate::accept::Rewrite,
    /// The most writing rounds, the first included.
    pub rounds: u32,
    /// A writer's model requests in the first round.
    pub turns: usize,
    /// A targeted repair round's model requests.
    pub repair_turns: usize,
    /// The writers' reasoning effort, or `null` for the provider's default.
    pub effort: Option<String>,
    /// Tell the writer to write every test, then run the suite once.
    pub one_pass: bool,
    /// Tell the writer to find the deciding facts by reading the code's
    /// documentation and running property probes on the untouched code.
    pub discover: bool,
}

impl SuiteWriter {
    fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !(1..=4).contains(&self.writers) {
            problems
                .push("executor.microluna.suite_writer.writers must be from 1 to 4".to_string());
        }
        if self.rounds == 0 || self.turns == 0 || self.repair_turns == 0 {
            problems.push(
                "executor.microluna.suite_writer's rounds, turns, and repair_turns must be at least 1"
                    .to_string(),
            );
        }
        problems
    }

    /// The `accept.define` options this writer policy sets.
    #[must_use]
    pub fn options(&self) -> crate::accept::Options {
        crate::accept::Options {
            max_rounds: self.rounds,
            writers: self.writers as usize,
            rewrite: self.rewrite,
            repair_turns: Some(self.repair_turns),
            one_pass: self.one_pass,
            discover: self.discover,
            ..crate::accept::Options::default()
        }
    }
}

fn yes() -> bool {
    true
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            mode: Mode::Requirements,
            max_sessions: 8,
            max_attempts: 2,
            max_groups: 4,
            session_turns: 30,
            session_sec: 600,
            spend_usd: 0.50,
            evidence_chars: 14_000,
            checks: true,
            require_evidence: true,
            read_first: false,
            accept: false,
            focus_actionable: false,
            suite: false,
            suite_writer: None,
        }
    }
}

/// What an edit session in the suite loop is told.
pub const SUITE_GUIDANCE: &str = "A frozen acceptance suite defines done for this task: it was \
written from the task before any fix and fails on the untouched workspace. Change the workspace \
until every test in it passes. You can't change the tests or anything in their directory. Run the \
red tests with the command the evidence gives before your first edit and after every edit, and \
read their output: each red test names the fact it checks. Use the exact rule, value, and format \
the task states, not a simpler one. When every test passes, call finish with status done. If a \
test seems to contradict the task, follow the task and say so in your summary.";

impl Policy {
    /// The problems with the bounds, for [`crate::policy::Manifest::validate`].
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.max_sessions == 0 || self.max_attempts == 0 || self.max_groups == 0 {
            problems.push(
                "executor.microluna's max_sessions, max_attempts, and max_groups must be at least 1"
                    .to_string(),
            );
        }
        if self.session_turns == 0 || self.session_sec == 0 {
            problems.push(
                "executor.microluna's session_turns and session_sec must be at least 1".to_string(),
            );
        }
        if self.spend_usd.is_nan() || self.spend_usd <= 0.0 {
            problems.push("executor.microluna.spend_usd must be above 0".to_string());
        }
        if let Some(writer) = &self.suite_writer {
            problems.extend(writer.validate());
            if !self.suite {
                problems.push("executor.microluna.suite_writer needs suite".to_string());
            }
        }
        problems
    }
}

/// The transport a [`Micro`] sends through: the Codex login, or a script.
pub enum Wire {
    Codex(CodexTransport),
    Fake(FakeTransport),
}

impl Transport for Wire {
    async fn respond(
        &self,
        request: &microluna::Request,
    ) -> Result<microluna::Reply, microluna::TransportError> {
        match self {
            Wire::Codex(transport) => transport.respond(request).await,
            Wire::Fake(transport) => transport.respond(request).await,
        }
    }
}

/// The Codex login's path, as Microluna reads it: `$CODEX_HOME/auth.json`
/// or `~/.codex/auth.json`.
#[must_use]
pub fn login_path() -> Option<PathBuf> {
    Login::default_path()
}

/// The doctor's check of the Codex login, without any inference: the file
/// is there and its access token isn't about to expire. Nothing secret is
/// printed.
#[must_use]
pub fn check_login() -> Vec<String> {
    let Some(path) = login_path() else {
        return vec!["microluna: no CODEX_HOME or HOME to find the Codex login in".to_string()];
    };
    match Login::load(&path) {
        Ok(login) => {
            println!(
                "microluna credential: codex_auth_json at {}{}",
                path.display(),
                login
                    .expires_at()
                    .map(|at| format!(", access token valid until {at} (Unix time)"))
                    .unwrap_or_default()
            );
            Vec::new()
        }
        Err(error) => vec![format!("microluna: {error}")],
    }
}

/// The Codex transport on the login, or why there is none.
///
/// # Errors
///
/// The login's error, as a sentence.
pub fn codex_wire(session_id: &str) -> Result<Wire, String> {
    let path = login_path().ok_or("no CODEX_HOME or HOME to find the Codex login in")?;
    CodexTransport::new(path, session_id)
        .map(Wire::Codex)
        .map_err(|error| error.to_string())
}

/// The Microluna executor.
pub struct Micro {
    pub model: String,
    /// Reasoning effort, or `None` for the provider's default.
    pub effort: Option<String>,
    /// The dispatch's wall-time bound.
    pub deadline: Duration,
    /// The workspace the sessions act in.
    pub workdir: PathBuf,
    /// Where each session's own trace and the loop's record are written.
    pub artifacts: PathBuf,
    /// The episode's recorder: invocations, executor events, and hand-offs.
    pub recorder: Recorder,
    /// Dispatches before this one, so files don't overwrite theirs.
    pub runs: u32,
    pub policy: Policy,
    pub isolation: Isolation,
    /// The transport, or why there is none.
    pub wire: Result<Wire, String>,
    /// The episode deadline every session is bounded by.
    pub episode: crate::deadline::Deadline,
    /// What the checks between sessions read; built from the requirement
    /// map when `None`.
    pub subject: Option<Subject>,
    /// What the briefing was packed from, when the host said.
    pub prepared: Option<Prepared>,
    /// The last dispatch's loop record.
    pub last: Option<Value>,
}

impl Micro {
    /// A Microluna executor on the Codex login, confined by `isolation`.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model: &str,
        effort: Option<String>,
        deadline: Duration,
        workdir: &Path,
        artifacts: &Path,
        recorder: Recorder,
        runs: u32,
        policy: Policy,
        isolation: Isolation,
    ) -> Self {
        let id = format!("microluna-{}-{}", std::process::id(), atif::now_ms());
        Micro {
            model: model.to_string(),
            effort,
            deadline,
            workdir: workdir.to_path_buf(),
            artifacts: artifacts.to_path_buf(),
            recorder,
            runs,
            policy,
            isolation,
            wire: codex_wire(&id),
            episode: crate::deadline::Deadline::unbounded(),
            subject: None,
            prepared: None,
            last: None,
        }
    }

    fn dispatch(&self) -> u32 {
        self.runs + 1
    }
}

/// What one session did, as the loop reads it.
#[derive(Clone, Debug)]
struct Ran {
    number: u32,
    focus: Vec<String>,
    ending: Ending,
    finish: Option<microluna::Finish>,
    turns: usize,
    calls: usize,
    usage: TokenUsage,
    cost_usd: Option<f64>,
    milliseconds: u64,
    session_id: String,
    trace: String,
    commands: Vec<(String, Option<i64>)>,
    changed: Vec<String>,
    /// The session made at least one edit with a file tool.
    edited: bool,
    /// A command ran after the session's last file-tool edit: the edit was
    /// exercised.
    ran_after_edit: bool,
    /// The workspace changed during the session: a file-tool edit, or, in a
    /// Git work tree, a moved HEAD or a changed status. A task done through
    /// commands alone (a git recovery, an `mv`, a `sed -i`) changes it
    /// without a file-tool edit.
    changed_workspace: bool,
    /// A real command ran, not just a file read.
    ran_command: bool,
    /// The session ran read-only, so it was never meant to edit.
    read_only: bool,
}

impl Ran {
    /// Whether the session did work worth ending or advancing on: it
    /// changed the workspace and ran a command that could exercise the
    /// change. A file-tool edit needs a command after it; a change made
    /// through commands already ran one.
    fn evidence(&self) -> bool {
        (self.edited && self.ran_after_edit) || (self.changed_workspace && self.ran_command)
    }
}

impl Ran {
    fn status(&self) -> String {
        match (&self.ending, &self.finish) {
            (Ending::Finished, Some(finish)) => serde_json::to_value(finish.status)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| "finished".to_string()),
            (Ending::Stopped, _) => "stopped".to_string(),
            (Ending::TurnLimit, _) => "turn_limit".to_string(),
            (Ending::Deadline, _) => "deadline".to_string(),
            (Ending::Transport(_), _) => "transport".to_string(),
            (Ending::Finished, None) => "finished".to_string(),
        }
    }

    fn summary(&self) -> String {
        match &self.finish {
            Some(finish) if finish.answer.trim().is_empty() => finish.summary.clone(),
            Some(finish) => format!("{}\n\nAnswer: {}", finish.summary, finish.answer),
            None => match &self.ending {
                Ending::Transport(why) => format!("The session lost its provider: {why}"),
                Ending::Deadline => "The session's time bound passed.".to_string(),
                Ending::TurnLimit => "The session used every turn it had.".to_string(),
                _ => "The session stopped without calling finish.".to_string(),
            },
        }
    }

    fn record(&self) -> Value {
        json!({
            "number": self.number,
            "focus": self.focus,
            "status": self.status(),
            "finish": self.finish,
            "turns": self.turns,
            "calls": self.calls,
            "usage": usage_json(self.usage),
            "cost_usd": self.cost_usd,
            "milliseconds": self.milliseconds,
            "session_id": self.session_id,
            "trace": self.trace,
            "commands": self.commands.iter().map(|(c, e)| json!({ "command": c, "exit": e })).collect::<Vec<_>>(),
            "changed": self.changed,
            "edited": self.edited,
            "ran_after_edit": self.ran_after_edit,
            "changed_workspace": self.changed_workspace,
            "ran_command": self.ran_command,
            "read_only": self.read_only,
        })
    }
}

fn usage_json(usage: TokenUsage) -> Value {
    json!({
        "input": usage.input,
        "cached": usage.cached,
        "output": usage.output,
        "reasoning": usage.reasoning,
    })
}

/// A group of requirements one or more sessions work on together.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub ids: Vec<String>,
    /// `- R2 (behavior): text` lines.
    pub lines: Vec<String>,
}

/// The requirement groups, in map order, split into at most `max`
/// consecutive groups of near-equal size. Context is always left out. When
/// `actionable_only`, constraints are left out too, so a group holds only
/// the behaviors, deliverables, and checks a session edits toward; the
/// constraints reach every session through the brief instead. When a task
/// has no actionable requirement, `actionable_only` falls back to every
/// non-context requirement so the loop still runs.
#[must_use]
pub fn groups(
    map: &crate::requirements::RequirementMap,
    max: u32,
    actionable_only: bool,
) -> Vec<Group> {
    let actionable = |k: Kind| matches!(k, Kind::Behavior | Kind::Deliverable | Kind::Check);
    let mut kept: Vec<_> = map
        .requirements
        .iter()
        .filter(|r| {
            if actionable_only {
                actionable(r.kind)
            } else {
                r.kind != Kind::Context
            }
        })
        .collect();
    if actionable_only && kept.is_empty() {
        kept = map
            .requirements
            .iter()
            .filter(|r| r.kind != Kind::Context)
            .collect();
    }
    if kept.is_empty() {
        return Vec::new();
    }
    let max = (max.max(1) as usize).min(kept.len());
    let size = kept.len().div_ceil(max);
    kept.chunks(size)
        .map(|chunk| Group {
            ids: chunk.iter().map(|r| r.id.clone()).collect(),
            lines: chunk
                .iter()
                .map(|r| {
                    format!(
                        "- {} ({}): {}",
                        r.id,
                        r.kind.word(),
                        r.text.split_whitespace().collect::<Vec<_>>().join(" ")
                    )
                })
                .collect(),
        })
        .collect()
}

/// The constraints every session must honor: the `- R7 (constraint): text`
/// lines for the map's constraint requirements, in map order. These are
/// the standing rules a focused session works under when `focus_actionable`
/// keeps them out of the groups. Context is left out, since it repeats the
/// task rather than constraining the work.
#[must_use]
pub fn constraints(map: &crate::requirements::RequirementMap) -> Vec<String> {
    map.requirements
        .iter()
        .filter(|r| r.kind == Kind::Constraint)
        .map(|r| {
            format!(
                "- {} ({}): {}",
                r.id,
                r.kind.word(),
                r.text.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .collect()
}

/// The evidence one group's sessions read: the items that inform any of
/// its requirements, by relevance, within `chars`. With none, the three
/// most relevant items.
#[must_use]
pub fn evidence_for(prepared: &Prepared, group: &Group, chars: usize) -> Vec<Evidence> {
    let informs = |id: &str| {
        prepared
            .informs
            .get(id)
            .is_some_and(|ids| ids.iter().any(|r| group.ids.contains(r)))
    };
    let mut chosen: Vec<&crate::pack::Item> = prepared
        .items
        .iter()
        .filter(|item| informs(&item.id) && !item.text.trim().is_empty())
        .collect();
    if chosen.is_empty() {
        chosen = prepared
            .items
            .iter()
            .filter(|item| !item.text.trim().is_empty())
            .collect();
        chosen.sort_by(|a, b| b.p.unwrap_or(0.0).total_cmp(&a.p.unwrap_or(0.0)));
        chosen.truncate(3);
    } else {
        chosen.sort_by(|a, b| b.p.unwrap_or(0.0).total_cmp(&a.p.unwrap_or(0.0)));
    }
    let share = (chars / chosen.len().max(1)).max(1_500);
    let mut left = chars;
    let mut out = Vec::new();
    for item in chosen {
        if left < 400 {
            break;
        }
        let room = share.min(left);
        let text = if item.text.chars().count() > room {
            format!(
                "{}\n[trimmed: {} of {} characters shown]",
                crate::judge::clip(&item.text, room),
                room,
                item.text.chars().count()
            )
        } else {
            item.text.clone()
        };
        left = left.saturating_sub(text.chars().count());
        out.push(Evidence {
            label: item.label.clone(),
            text,
        });
    }
    out
}

/// The next move after a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Next,
    Retry,
    Stuck,
    Done,
}

impl Move {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Move::Next => "next",
            Move::Retry => "retry",
            Move::Stuck => "stuck",
            Move::Done => "done",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        match word {
            "next" => Some(Move::Next),
            "retry" => Some(Move::Retry),
            "stuck" => Some(Move::Stuck),
            "done" => Some(Move::Done),
            _ => None,
        }
    }
}

/// What the checks said about one group, for the brief and for Jev.
#[derive(Clone, Debug, Default)]
struct Checked {
    /// `R2 observed`, one per requirement the checks covered.
    states: Vec<(String, String)>,
    /// Failed scenarios' short accounts, for the group's requirements.
    failures: Vec<String>,
    summary: Value,
}

impl Checked {
    fn from_report(report: &checks::Report, group: &Group) -> Self {
        let states = report
            .coverage
            .iter()
            .map(|c| (c.id.clone(), c.state.clone()))
            .collect();
        let failures = report
            .packets
            .iter()
            .filter(|p| group.ids.contains(&p.requirement))
            .map(|p| {
                format!(
                    "{} failed for {}: expected {}; observed {}",
                    p.scenario,
                    p.requirement,
                    crate::judge::clip(
                        &serde_json::to_string(&p.expected).unwrap_or_default(),
                        300
                    ),
                    crate::judge::clip(
                        &serde_json::to_string(&p.observations).unwrap_or_default(),
                        500
                    )
                )
            })
            .collect();
        Checked {
            states,
            failures,
            summary: report.summary(),
        }
    }

    fn contradicted(&self, group: &Group) -> bool {
        !self.failures.is_empty()
            || self
                .states
                .iter()
                .any(|(id, state)| group.ids.contains(id) && state == "contradicted")
    }

    /// Whether the checks positively confirm the group: every focus
    /// requirement the checks cover is observed, none contradicted, and at
    /// least one is observed. `None` when the checks didn't run or cover the
    /// group, so the caller can't gate on them. This is the thesis's "done
    /// is a program state": the loop ends on green checks, not the model's
    /// opinion.
    fn accepts(&self, group: &Group) -> Option<bool> {
        if self.contradicted(group) {
            return Some(false);
        }
        let covered: Vec<&str> = self
            .states
            .iter()
            .filter(|(id, _)| group.ids.contains(id))
            .map(|(_, state)| state.as_str())
            .collect();
        if covered.is_empty() {
            return None;
        }
        Some(covered.contains(&"observed"))
    }

    fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.states.is_empty() {
            out.push(
                self.states
                    .iter()
                    .map(|(id, state)| format!("{id} {state}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        out.extend(self.failures.iter().cloned());
        out
    }
}

/// What the code rule reads besides Jev's pick.
#[derive(Clone, Copy, Debug, Default)]
struct Signals {
    /// A check contradicts a requirement of the group.
    contradicted: bool,
    /// The combined verdict (`checks::verdict`) calls the candidate failed.
    verdict_fail: bool,
    /// The group is the last one.
    last: bool,
    /// The session ran read-only, so it only reconnoiters and retries.
    read_only: bool,
    /// The session edited a file and ran a command after its last edit.
    evidence: bool,
    /// Whether a move that ends the loop needs that evidence.
    require_evidence: bool,
    /// Whether the checks confirm the focus, when the accept gate is on:
    /// `Some(false)` blocks an ending move, `None` leaves it to the other
    /// rules (the gate is off, or the checks don't cover the group).
    accepts: Option<bool>,
    attempts: u32,
    max_attempts: u32,
}

/// The code rule over Jev's pick: a check that contradicts the group keeps
/// it from moving on; a combined verdict of fail keeps the loop from
/// ending, by `done` or past the last group; attempts are bounded; and
/// without an answer the session's own typed status decides.
fn settle(picked: Option<Move>, ran: &Ran, signals: Signals) -> (Move, Option<String>) {
    let Signals {
        contradicted,
        verdict_fail,
        last,
        read_only,
        evidence,
        require_evidence,
        accepts,
        attempts,
        max_attempts,
    } = signals;
    // A read-only session never advances: it reconnoiters, then an edit
    // session follows on the same group.
    if read_only {
        return (
            Move::Retry,
            Some("the read-only first session retries into an edit session".to_string()),
        );
    }
    let finished_done = matches!(
        ran.finish.as_ref().map(|f| f.status),
        Some(microluna::FinishStatus::Done)
    );
    let fallback = if finished_done && !contradicted {
        Move::Next
    } else {
        Move::Retry
    };
    let (mut chosen, mut why) = match picked {
        Some(chosen) => (chosen, None),
        None => (
            fallback,
            Some("Jev gave no answer, so the session's own status decided".to_string()),
        ),
    };
    if contradicted && matches!(chosen, Move::Next | Move::Done) {
        why = Some(format!(
            "a check contradicts the focus, so {} became retry",
            chosen.word()
        ));
        chosen = Move::Retry;
    }
    let ends = chosen == Move::Done || (last && chosen == Move::Next);
    if require_evidence && ends && !evidence {
        why = Some(format!(
            "{}the session made no edit it then tested, so {} became retry",
            why.map(|w| format!("{w}; ")).unwrap_or_default(),
            chosen.word()
        ));
        chosen = Move::Retry;
    }
    let ends = chosen == Move::Done || (last && chosen == Move::Next);
    if accepts == Some(false) && ends {
        why = Some(format!(
            "{}the checks don't confirm the focus yet, so {} became retry",
            why.map(|w| format!("{w}; ")).unwrap_or_default(),
            chosen.word()
        ));
        chosen = Move::Retry;
    }
    let ends = chosen == Move::Done || (last && chosen == Move::Next);
    if verdict_fail && ends {
        why = Some(format!(
            "the combined verdict calls the candidate failed, so {} became retry",
            chosen.word()
        ));
        chosen = Move::Retry;
    }
    if chosen == Move::Retry && attempts >= max_attempts {
        why = Some(format!(
            "{}the group had its {max_attempts} attempts, so retry became stuck",
            why.map(|w| format!("{w}; ")).unwrap_or_default()
        ));
        chosen = Move::Stuck;
    }
    (chosen, why)
}

fn clip_lines(text: &str, max: usize) -> String {
    crate::judge::clip(text.trim(), max)
}

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A signature of the Git work tree at `dir`: the HEAD commit and the
/// porcelain status, so a session that moves HEAD or changes a file is
/// seen even when it edits through commands. `None` when `dir` isn't a Git
/// work tree, where a file-tool edit is the only change worth counting.
fn git_signature(dir: &Path) -> Option<String> {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    if run(&["rev-parse", "--is-inside-work-tree"])?.trim() != "true" {
        return None;
    }
    let head = run(&["rev-parse", "HEAD"]).unwrap_or_default();
    let status = run(&["status", "--porcelain"]).unwrap_or_default();
    Some(format!("{head}\n{status}"))
}

fn millis(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The normalized events one Microluna step becomes.
fn events_of(step: &Step) -> Vec<EventKind> {
    if let Some(call) = &step.call {
        let arguments = &call.arguments;
        let arg = |key: &str| arguments.get(key).and_then(Value::as_str).unwrap_or("");
        let completed = call.outcome == atif::Outcome::Completed;
        return match call.name.as_str() {
            "run_command" => vec![
                EventKind::CommandStarted {
                    command: arg("command").to_string(),
                },
                EventKind::CommandCompleted {
                    command: arg("command").to_string(),
                    exit_code: call.extra.get("exit").and_then(Value::as_i64),
                    output: crate::judge::clip(&call.output, crate::stream::OUTPUT_CHARS),
                },
            ],
            "read_file" => {
                let from = arguments
                    .get("start_line")
                    .and_then(Value::as_u64)
                    .unwrap_or(1);
                let command = format!("read_file {} (from line {from})", arg("path"));
                vec![
                    EventKind::CommandStarted {
                        command: command.clone(),
                    },
                    EventKind::CommandCompleted {
                        command,
                        exit_code: Some(i64::from(!completed)),
                        output: crate::judge::clip(&call.output, 600),
                    },
                ]
            }
            "apply_patch" if completed => arg("patch")
                .lines()
                .filter_map(|line| {
                    [
                        ("*** Add File: ", "add"),
                        ("*** Update File: ", "update"),
                        ("*** Delete File: ", "delete"),
                    ]
                    .iter()
                    .find_map(|(prefix, change)| {
                        line.strip_prefix(prefix)
                            .map(|path| EventKind::ArtifactChanged {
                                path: path.trim().to_string(),
                                change: (*change).to_string(),
                            })
                    })
                })
                .collect(),
            "apply_patch" => vec![
                EventKind::CommandStarted {
                    command: "apply_patch".to_string(),
                },
                EventKind::CommandCompleted {
                    command: "apply_patch".to_string(),
                    exit_code: Some(1),
                    output: crate::judge::clip(&call.output, 600),
                },
            ],
            "write_file" if completed => vec![EventKind::ArtifactChanged {
                path: arg("path").to_string(),
                change: "write".to_string(),
            }],
            "finish" => vec![EventKind::SessionEnded {
                error: arg("status") != "done",
                result: Some(
                    [arg("summary"), arg("answer")]
                        .iter()
                        .filter(|t| !t.trim().is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                ),
            }],
            _ => Vec::new(),
        };
    }
    if step.source == atif::Source::Agent && !step.message.trim().is_empty() {
        return vec![EventKind::AssistantClaim {
            text: crate::judge::clip(&step.message, crate::stream::CLAIM_CHARS),
        }];
    }
    Vec::new()
}

impl Micro {
    /// Runs one session, recording it; `why` names what it works on.
    async fn session(
        &self,
        number: u32,
        focus: &[String],
        why: &str,
        brief: &Brief,
        read_only: bool,
    ) -> Ran {
        let dispatch = self.dispatch();
        let session_id = format!("microluna-{dispatch}-{number}");
        let text: String = brief
            .input()
            .iter()
            .filter_map(|item| item["content"][0]["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if self.policy.mode == Mode::Requirements {
            // A section per session: the Gym shows each as a takeover.
            self.recorder.push(crate::delegate::with_briefing(
                Step::said(
                    Source::System,
                    &format!(
                        "Delegating to {AGENT} ({}) because {why}. Briefing: {} characters, sha256 {}.",
                        self.model,
                        text.chars().count(),
                        sha256(&text)
                    ),
                ),
                &text,
            ));
        }
        let invocation = self.recorder.enter(
            Start::new(
                SESSION_COMPONENT,
                Implementation::new(
                    SESSION_COMPONENT,
                    &format!("{AGENT} {}", self.model),
                    &json!({
                        "model": self.model,
                        "effort": self.effort,
                        "turns": self.policy.session_turns,
                        "session_sec": self.policy.session_sec,
                        "instructions": microluna::session::INSTRUCTIONS,
                        "tools": microluna::tools::declarations(),
                        "isolation": self.isolation.word(),
                    }),
                ),
            )
            .named(&format!("session {number}: {}", focus.join(", ")))
            .reading_digest(sha256(&text))
            .with_effects(),
        );
        let trace = format!("microluna-{dispatch}-{number}.atif.jsonl");
        let mut atif_session = atif::Session::opening(
            &session_id,
            &self.model,
            "codex-login",
            &self.workdir.display().to_string(),
            &crate::episode::version(),
        );
        atif_session.directive = why.to_string();
        let started = Instant::now();
        let seq = Rc::new(Cell::new(0u64));
        let revision = Rc::new(Cell::new(0u64));
        let commands = Rc::new(std::cell::RefCell::new(Vec::<(String, Option<i64>)>::new()));
        let changed = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        // Set when an edit is seen, cleared when a real command runs after
        // it: at the end it says whether the last edit went untested.
        let untested_edit = Rc::new(Cell::new(false));
        let ran_after_edit = Rc::new(Cell::new(false));
        // Set when any command that isn't a file read runs.
        let ran_command = Rc::new(Cell::new(false));
        let before = git_signature(&self.workdir);
        let sink = {
            let recorder = self.recorder.clone();
            let session_id = session_id.clone();
            let seq = seq.clone();
            let revision = revision.clone();
            let commands = commands.clone();
            let changed = changed.clone();
            let untested_edit = untested_edit.clone();
            let ran_after_edit = ran_after_edit.clone();
            let ran_command = ran_command.clone();
            move |step: &Step| {
                if step.call.is_some() || step.source == atif::Source::Agent {
                    crate::say::line(&format!("  {}", microluna::session::line(step)));
                }
                for kind in events_of(step) {
                    match &kind {
                        EventKind::CommandCompleted {
                            command, exit_code, ..
                        } => {
                            commands.borrow_mut().push((command.clone(), *exit_code));
                            // A read_file's synthetic command doesn't test an edit.
                            if !command.starts_with("read_file ") {
                                ran_command.set(true);
                                if untested_edit.get() {
                                    untested_edit.set(false);
                                    ran_after_edit.set(true);
                                }
                            }
                        }
                        EventKind::ArtifactChanged { path, .. } => {
                            revision.set(revision.get() + 1);
                            untested_edit.set(true);
                            ran_after_edit.set(false);
                            if !changed.borrow().contains(path) {
                                changed.borrow_mut().push(path.clone());
                            }
                        }
                        _ => {}
                    }
                    seq.set(seq.get() + 1);
                    let observation = Observation {
                        version: Version {
                            session_id: Some(session_id.clone()),
                            generation: 1,
                            seq: seq.get(),
                            revision: revision.get(),
                        },
                        at_ms: millis(started),
                        event: Event {
                            seq: seq.get(),
                            line: usize::try_from(seq.get()).unwrap_or(usize::MAX),
                            offset: None,
                            kind,
                        },
                    };
                    recorder.push(session::observation_step(AGENT, &observation));
                }
            }
        };
        let mut recorder = microluna::Recorder::new().forwarding(sink);
        match atif::Log::create_at(&self.artifacts.join(&trace), &atif_session) {
            Ok(log) => recorder = recorder.logging(log),
            Err(error) => crate::say::line(&format!(
                "  microluna ▸ not tracing session {number}: {error}"
            )),
        }
        let left = self
            .episode
            .allowance()
            .map_or(self.deadline, |left| left.min(self.deadline));
        let config = Config {
            model: self.model.clone(),
            effort: self.effort.clone(),
            max_turns: self.policy.session_turns,
            cache_key: format!(
                "microluna-{}",
                &sha256(
                    &self
                        .prepared
                        .as_ref()
                        .map_or(String::new(), |p| p.instruction.clone())
                )[..16]
            ),
            deadline: Some(Duration::from_secs(self.policy.session_sec).min(left)),
        };
        let workspace = microluna::Workspace::new(&self.workdir).map(|workspace| {
            let workspace = workspace.isolated_by(self.isolation);
            if read_only {
                workspace.reading_only()
            } else {
                workspace
            }
        });
        let report = match (&self.wire, workspace) {
            (Ok(wire), Ok(workspace)) => {
                microluna::run(wire, &workspace, brief, &config, &mut recorder).await
            }
            (Err(why), _) => microluna::Report {
                ending: Ending::Transport(why.clone()),
                finish: None,
                turns: 0,
                calls: 0,
                usage: TokenUsage::default(),
                cost_usd: Some(0.0),
                milliseconds: 0,
            },
            (_, Err(error)) => microluna::Report {
                ending: Ending::Transport(format!(
                    "the workspace {} can't be used: {error}",
                    self.workdir.display()
                )),
                finish: None,
                turns: 0,
                calls: 0,
                usage: TokenUsage::default(),
                cost_usd: Some(0.0),
                milliseconds: 0,
            },
        };
        recorder.close(match report.ending {
            Ending::Finished => atif::log::ENDED,
            _ => atif::log::INTERRUPTED,
        });
        let ran = Ran {
            number,
            focus: focus.to_vec(),
            ending: report.ending.clone(),
            finish: report.finish.clone(),
            turns: report.turns,
            calls: report.calls,
            usage: report.usage,
            cost_usd: report.cost_usd,
            milliseconds: report.milliseconds,
            session_id,
            trace: format!("artifacts/{trace}"),
            commands: commands.borrow().clone(),
            changed: changed.borrow().clone(),
            edited: !changed.borrow().is_empty(),
            ran_after_edit: ran_after_edit.get(),
            changed_workspace: !changed.borrow().is_empty()
                || git_signature(&self.workdir)
                    .is_some_and(|after| Some(&after) != before.as_ref()),
            ran_command: ran_command.get(),
            read_only,
        };
        // The dispatch's exec.session carries the sessions' cost, as it
        // does for a CLI; each session states its own share in its summary,
        // so a reader that sums invocation costs counts it once.
        self.recorder.end(
            &invocation,
            Finish::new(match ran.ending {
                Ending::Finished => Outcome::Completed,
                _ => Outcome::Failed,
            })
            .summary(json!({
                "status": ran.status(),
                "summary": crate::judge::clip(&ran.summary(), 600),
                "turns": ran.turns,
                "calls": ran.calls,
                "usage": usage_json(ran.usage),
                "cost_usd": ran.cost_usd,
                "cost_provenance": match ran.ending {
                    Ending::Deadline => "price_estimate_lower_bound",
                    _ => "price_estimate",
                },
                "trace": ran.trace,
            })),
        );
        crate::say::line(&format!(
            "  microluna ▸ session {number} ({}) {} in {:.1}s · {} turns · {} calls · in {} (cached {}) out {} · ${:.5}",
            focus.join(", "),
            ran.status(),
            ran.milliseconds as f64 / 1000.0,
            ran.turns,
            ran.calls,
            ran.usage.input,
            ran.usage.cached,
            ran.usage.output,
            ran.cost_usd.unwrap_or_default()
        ));
        ran
    }

    /// The single mode: one session on the briefing.
    async fn single(&self, briefing: &Briefing) -> Vec<Ran> {
        let brief = Brief::task(&briefing.text);
        vec![
            self.session(
                1,
                &["the briefing".to_string()],
                "the host briefed it",
                &brief,
                false,
            )
            .await,
        ]
    }

    /// The checks' subject: the one the host set, or the task's words and
    /// the requirement map against the live workspace.
    fn subject(&self, prepared: &Prepared) -> Subject {
        let mut subject = self.subject.clone().unwrap_or_else(|| Subject {
            label: "microluna workspace".to_string(),
            task: checks::TaskText {
                title: prepared.title.clone(),
                instruction: prepared.instruction.clone(),
            },
            requirements: None,
            provided: Vec::new(),
            inputs: (prepared.instruction.contains("logs/") && self.workdir.join("logs").is_dir())
                .then(|| "logs".to_string()),
            budget: checks::Budget::default(),
            live: Some(checks::generic::Workspace {
                dir: self.workdir.to_string_lossy().into_owned(),
                claimed: Vec::new(),
                command_sec: 60,
                report: None,
                options: checks::generic::Options::default(),
                root: None,
                collected: Vec::new(),
            }),
            distrust: Vec::new(),
        });
        subject.requirements = Some(prepared.requirements.clone());
        subject
    }

    /// Asks Jev for the move after `ran`, and settles it with the code rule.
    #[allow(clippy::too_many_arguments)]
    async fn decide(
        &self,
        prepared: &Prepared,
        group: &Group,
        later: &[Group],
        ran: &Ran,
        checked: &Checked,
        attempts: u32,
    ) -> (Move, Value, f64) {
        // The combined verdict over the task and this session's report:
        // the handoff signal issue #9584 calibrated against the verifier.
        let (_, verdict, verdict_asked) = checks::verdict::assess(
            &prepared.jev,
            &self.recorder,
            &prepared.instruction,
            &ran.summary(),
            prepared.deadline.clone(),
            &checks::verdict::fitted(),
        )
        .await;
        let state = json!({
            "task": clip_lines(&prepared.instruction, 4_000),
            "focus": group.lines,
            "later_requirements": later.iter().flat_map(|g| g.lines.clone()).collect::<Vec<_>>(),
            "session": {
                "number": ran.number,
                "attempt_on_focus": attempts,
                "status": ran.status(),
                "report": clip_lines(&ran.summary(), 1_500),
                "commands": ran.commands.iter().rev().take(8).rev()
                    .map(|(c, e)| json!({ "command": crate::judge::clip(c, 200), "exit": e }))
                    .collect::<Vec<_>>(),
                "changed_files": ran.changed,
            },
            "checks": {
                "ran": self.policy.checks,
                "requirement_states": checked.states.iter().map(|(id, s)| json!({ "id": id, "state": s })).collect::<Vec<_>>(),
                "failures_on_focus": checked.failures,
            },
            "verdict": {
                "call": verdict.call,
                "failure_probability": verdict.p_fail,
            },
        });
        let mut choice = jev::Choice::new(MOVE_QUESTION, indexmap::IndexMap::new());
        for (name, meaning) in MOVES {
            choice = choice.option(name, meaning);
        }
        let questions = jev::Questions::new().with("next_move", choice);
        let asked = jev_component::ask(
            &prepared.jev,
            &self.recorder,
            jev_component::Ask {
                component: HANDOFF_COMPONENT,
                name: "jev_next_move",
                id: format!("jev-next-move-{}-{}", self.dispatch(), ran.number),
                state,
                questions,
                parent: None,
                deadline: prepared.deadline.clone(),
            },
        )
        .await;
        let picked = asked.choice("next_move").and_then(Move::parse);
        let p = asked
            .answers
            .as_ref()
            .and_then(|a| a.pointer("/next_move/probabilities"))
            .cloned()
            .unwrap_or(Value::Null);
        let contradicted = checked.contradicted(group);
        let (chosen, overridden) = settle(
            picked,
            ran,
            Signals {
                contradicted,
                verdict_fail: verdict.call == "fail",
                last: later.is_empty(),
                read_only: ran.read_only,
                evidence: ran.evidence(),
                require_evidence: self.policy.require_evidence,
                accepts: if self.policy.accept {
                    checked.accepts(group)
                } else {
                    None
                },
                attempts,
                max_attempts: self.policy.max_attempts,
            },
        );
        let usd = [asked.input_tokens, verdict_asked.input_tokens]
            .iter()
            .flatten()
            .map(|tokens| *tokens as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0)
            .sum::<f64>();
        let record = json!({
            "after_session": ran.number,
            "focus": group.ids,
            "jev": { "how": asked.how, "picked": picked.map(Move::word), "probabilities": p, "error": asked.error },
            "contradicted": contradicted,
            "checks": checked.summary,
            "verdict": verdict,
            "edited": ran.edited,
            "ran_after_edit": ran.ran_after_edit,
            "read_only": ran.read_only,
            "attempts": attempts,
            "move": chosen.word(),
            "overridden": overridden,
            "jev_usd": usd,
        });
        (chosen, record, usd)
    }

    /// The mini-handoff loop.
    async fn requirements(&self, prepared: &Prepared) -> (Vec<Ran>, Vec<Value>, String) {
        let groups = groups(
            &prepared.requirements,
            self.policy.max_groups,
            self.policy.focus_actionable,
        );
        let subject = self.subject(prepared);
        let dir = self
            .artifacts
            .parent()
            .map_or_else(|| self.artifacts.clone(), Path::to_path_buf);
        let started = Instant::now();
        let mut sessions: Vec<Ran> = Vec::new();
        let mut moves: Vec<Value> = Vec::new();
        let mut attempts = vec![0u32; groups.len()];
        let mut read_done = vec![false; groups.len()];
        let mut spent = 0.0;
        let mut cursor = 0;
        let mut last_checked: Option<Checked> = None;
        let mut stopped = "every requirement group had its turn".to_string();
        while cursor < groups.len() {
            let number = u32::try_from(sessions.len()).unwrap_or(u32::MAX) + 1;
            if number > self.policy.max_sessions {
                stopped = format!("the bound of {} sessions", self.policy.max_sessions);
                break;
            }
            if spent >= self.policy.spend_usd {
                stopped = format!(
                    "the spend bound of ${:.2}: ${spent:.4} spent",
                    self.policy.spend_usd
                );
                break;
            }
            let left = self
                .episode
                .allowance()
                .map_or(self.deadline, |left| left.min(self.deadline))
                .saturating_sub(started.elapsed());
            if left < Duration::from_secs(30) {
                stopped = "the time bound".to_string();
                break;
            }
            let group = &groups[cursor];
            // The read-only first session of a group reconnoiters before an
            // edit session; it doesn't spend one of the group's attempts.
            let is_read = self.policy.read_first && !read_done[cursor];
            if !is_read {
                attempts[cursor] += 1;
            }
            let brief = self.brief(
                prepared,
                &groups,
                cursor,
                attempts[cursor],
                &sessions,
                last_checked.as_ref(),
                is_read,
            );
            let why = if is_read {
                format!(
                    "session {number} reads before editing {} (group {} of {})",
                    group.ids.join(", "),
                    cursor + 1,
                    groups.len()
                )
            } else {
                format!(
                    "session {number} works on {} (group {} of {}, attempt {})",
                    group.ids.join(", "),
                    cursor + 1,
                    groups.len(),
                    attempts[cursor]
                )
            };
            let ran = self
                .session(number, &group.ids, &why, &brief, is_read)
                .await;
            spent += ran.cost_usd.unwrap_or(0.0);
            let lost = matches!(ran.ending, Ending::Transport(_));
            sessions.push(ran);
            let ran = sessions.last().cloned().expect("a session was just pushed");
            if lost {
                stopped = format!("session {number} lost its provider");
                break;
            }
            let checked = if self.policy.checks {
                let file = format!(
                    "verification/checks-microluna-{}-{number}.json",
                    self.dispatch()
                );
                let mut subject = subject.clone();
                if let Some(live) = &mut subject.live {
                    live.report = Some(ran.summary());
                }
                let (_, report) =
                    checks::check_subject_as(&subject, &self.workdir, &dir, &self.recorder, &file)
                        .await;
                Checked::from_report(&report, group)
            } else {
                Checked::default()
            };
            let (chosen, record, usd) = self
                .decide(
                    prepared,
                    group,
                    &groups[cursor + 1..],
                    &ran,
                    &checked,
                    attempts[cursor],
                )
                .await;
            spent += usd;
            if is_read {
                read_done[cursor] = true;
            }
            let to = match chosen {
                Move::Retry => {
                    format!("{AGENT} session {} on {}", number + 1, group.ids.join(", "))
                }
                Move::Next | Move::Stuck if cursor + 1 < groups.len() => format!(
                    "{AGENT} session {} on {}",
                    number + 1,
                    groups[cursor + 1].ids.join(", ")
                ),
                _ => "the host's closing checks".to_string(),
            };
            let trigger = format!(
                "Jev chose {}{}{}",
                record["jev"]["picked"].as_str().unwrap_or("nothing"),
                record["overridden"]
                    .as_str()
                    .map(|why| format!(" and code settled on {} because {why}", chosen.word()))
                    .unwrap_or_default(),
                if checked.states.is_empty() {
                    String::new()
                } else {
                    format!("; the checks say {}", checked.lines().join("; "))
                }
            );
            let mut handoff = record.clone();
            handoff["from"] = json!(format!(
                "{AGENT} session {number} on {}",
                group.ids.join(", ")
            ));
            handoff["to"] = json!(to);
            handoff["trigger"] = json!(trigger);
            handoff["pattern"] = json!("microluna-requirements");
            handoff["action"] = json!(chosen.word());
            self.recorder.push(
                Step::said(
                    Source::System,
                    &format!(
                        "After session {number} on {}: {}.",
                        group.ids.join(", "),
                        chosen.word()
                    ),
                )
                .noting(crate::handoff::KEY, handoff),
            );
            crate::say::line(&format!(
                "  microluna ▸ after session {number}: {} ({trigger})",
                chosen.word()
            ));
            moves.push(record);
            last_checked = Some(checked);
            match chosen {
                Move::Retry => {}
                Move::Next | Move::Stuck => cursor += 1,
                Move::Done => {
                    stopped = format!("Jev judged the task done after session {number}");
                    break;
                }
            }
        }
        (sessions, moves, stopped)
    }

    /// The suite loop: `accept.define` writes and freezes an acceptance
    /// suite, then edit sessions run until it's green or a bound is hit.
    /// `None` when there's no transport or the suite has no tests, so the
    /// caller falls back to the requirements loop.
    #[allow(clippy::too_many_lines)]
    async fn suite_loop(&self, prepared: &Prepared) -> Option<(Vec<Ran>, Vec<Value>, String)> {
        let Ok(wire) = &self.wire else {
            return None;
        };
        let started = Instant::now();
        let time_left = || {
            self.episode
                .allowance()
                .map_or(self.deadline, |left| left.min(self.deadline))
                .saturating_sub(started.elapsed())
        };
        let base = self
            .artifacts
            .parent()
            .map_or_else(|| self.artifacts.clone(), Path::to_path_buf);
        let suite_dir = base.join(format!("accept-suite-{}", self.dispatch()));
        let task = crate::accept::Task {
            title: prepared.title.clone(),
            instruction: prepared.instruction.clone(),
        };
        let everything = Group {
            ids: prepared
                .requirements
                .requirements
                .iter()
                .map(|r| r.id.clone())
                .collect(),
            lines: Vec::new(),
        };
        let evidence = evidence_for(prepared, &everything, self.policy.evidence_chars);
        let key = format!("microluna-{}", &sha256(&prepared.instruction)[..16]);
        let written_by = self.policy.suite_writer.as_ref();
        let writer = crate::accept::MicrolunaWriter {
            transport: wire,
            config: Config {
                model: self.model.clone(),
                effort: written_by
                    .and_then(|w| w.effort.clone())
                    .or_else(|| self.effort.clone()),
                max_turns: written_by.map_or(self.policy.session_turns.max(40), |w| w.turns),
                cache_key: format!("{key}-writer"),
                deadline: Some(Duration::from_secs(self.policy.session_sec).min(time_left())),
            },
            isolation: self.isolation,
            traces: Some(self.artifacts.clone()),
            echo: false,
        };
        let confine = match self.isolation {
            Isolation::TaskContainer => crate::accept::Confine::TaskContainer,
            Isolation::ReadOnly => crate::accept::Confine::ReadOnly,
            Isolation::Boundary => crate::accept::Confine::Writing,
        };
        let runner = crate::accept::Local {
            confine,
            test_sec: 120,
        };
        let inputs = crate::accept::Inputs {
            task: &task,
            requirements: &prepared.requirements,
            evidence: &evidence,
            workspace: &self.workdir,
            suite_dir: &suite_dir,
            workspace_note: format!(
                "the task's workspace, {}, which your commands can read but you must not change",
                self.workdir.display()
            ),
            target: None,
        };
        crate::say::line("  microluna ▸ writing the acceptance suite before any fix");
        let suite = crate::accept::define(
            &inputs,
            &writer,
            &runner,
            &prepared.jev,
            &self.recorder,
            &written_by.map_or_else(crate::accept::Options::default, SuiteWriter::options),
        )
        .await;
        crate::say::line(&format!("  microluna ▸ {}", suite.headline()));
        let mut moves = vec![json!({
            "kind": "suite",
            "headline": suite.headline(),
            "status": suite.status,
            "digest": suite.digest,
            "tests": suite.tests.len(),
            "rejected": suite.rejected.len(),
            "rounds": suite.rounds.len(),
            "gaps": suite.gaps,
            "writer_usd": suite.writer_usd,
            "jev_usd": suite.jev_usd,
            "milliseconds": suite.milliseconds,
            "record": crate::accept::AcceptanceSuite::record_path(&suite_dir),
        })];
        if suite.tests.is_empty() {
            crate::say::line(
                "  microluna ▸ the suite has no tests, so the requirements loop runs instead",
            );
            return None;
        }
        let mut spent = suite.writer_usd + suite.jev_usd;
        let mut sessions: Vec<Ran> = Vec::new();
        let mut best = 0usize;
        let mut since_progress = 0u32;
        let stopped;
        let mut number = 0u32;
        loop {
            let label = if number == 0 {
                "start".to_string()
            } else {
                format!("after session {number}")
            };
            let result = match crate::accept::run(
                &suite,
                &self.workdir,
                &runner,
                Some(&self.recorder),
                &label,
            )
            .await
            {
                Ok(result) => result,
                Err(tampered) => {
                    stopped = format!("the suite can't be trusted: {tampered}");
                    break;
                }
            };
            crate::say::line(&format!(
                "  microluna ▸ suite {label}: {} of {} green",
                result.passed, result.total
            ));
            // The frozen suite must fail the way its red-first proof did. When
            // most red tests now fail differently (a missing harness file, a
            // shell error), no edit can turn them green, so the requirements
            // loop takes over instead.
            if number == 0
                && let Some(proof) = suite.start.as_ref()
            {
                let differ = result
                    .tests
                    .iter()
                    .filter(|now| {
                        proof
                            .tests
                            .iter()
                            .find(|then| then.id == now.id)
                            .is_some_and(|then| then.green != now.green || then.exit != now.exit)
                    })
                    .count();
                if differ * 2 > result.tests.len().max(1) {
                    crate::say::line(&format!(
                        "  microluna ▸ the frozen suite doesn't reproduce its proof ({differ} of {} tests \
                         differ), so the requirements loop runs instead",
                        result.tests.len()
                    ));
                    return None;
                }
            }
            moves.push(json!({
                "kind": "run",
                "after_session": number,
                "passed": result.passed,
                "total": result.total,
                "green": result.green,
                "complete": result.complete,
                "red": result.red_requirements(),
            }));
            if result.green {
                stopped = format!(
                    "the acceptance suite is green after session {number} ({} of {})",
                    result.passed, result.total
                );
                break;
            }
            if result.passed > best {
                best = result.passed;
                since_progress = 0;
            } else if number > 0 {
                since_progress += 1;
            }
            if number >= self.policy.max_sessions {
                stopped = format!("the bound of {} sessions", self.policy.max_sessions);
                break;
            }
            if spent >= self.policy.spend_usd {
                stopped = format!(
                    "the spend bound of ${:.2}: ${spent:.4} spent",
                    self.policy.spend_usd
                );
                break;
            }
            if time_left() < Duration::from_secs(60) {
                stopped = "the time bound".to_string();
                break;
            }
            number += 1;
            let red_ids = result.red_requirements();
            let mut guidance = SUITE_GUIDANCE.to_string();
            let facts = constraints(&prepared.requirements);
            if !facts.is_empty() {
                guidance.push_str(&format!(
                    "\n\nThe task's constraints hold throughout; honor each one exactly:\n\n{}",
                    facts.join("\n")
                ));
            }
            let mut state = vec![format!(
                "Session {number} of at most {}. The suite is {} of {} green; the best so far \
                 is {best}.",
                self.policy.max_sessions, result.passed, result.total
            )];
            if since_progress >= 3 {
                state.push(format!(
                    "The last {since_progress} sessions turned no new test green. Don't repeat \
                     their approach: reread the task and each red test's output for the exact \
                     rule it checks, and change what the earlier sessions left alone."
                ));
            }
            state.push(format!(
                "The red tests and their output:\n{}",
                result.red_lines(&suite, 800).join("\n")
            ));
            for ran in sessions.iter().rev().take(4).rev() {
                state.push(format!(
                    "Session {} ended {}: {}",
                    ran.number,
                    ran.status(),
                    crate::judge::clip(&ran.summary(), 400)
                ));
            }
            if !sessions.is_empty() {
                let changes = crate::delegate::changes(&self.workdir, None);
                state.push(format!(
                    "What the workspace shows as changed now:\n{}",
                    crate::judge::clip(&changes, 1_500)
                ));
            }
            let mut session_evidence = vec![suite.evidence()];
            session_evidence.extend(evidence.iter().cloned());
            let brief = Brief {
                task: prepared.instruction.clone(),
                guidance,
                evidence: session_evidence,
                state,
            };
            let why = format!(
                "session {number} works toward a green suite ({} of {} green; red: {})",
                result.passed,
                result.total,
                red_ids.join(", ")
            );
            let ran = self.session(number, &red_ids, &why, &brief, false).await;
            spent += ran.cost_usd.unwrap_or(0.0);
            let lost = matches!(ran.ending, Ending::Transport(_));
            sessions.push(ran);
            if lost {
                stopped = format!("session {number} lost its provider");
                break;
            }
            // Two blocked sessions in a row: the next one would be blocked
            // by the same thing.
            if sessions.len() >= 2
                && sessions[sessions.len() - 2..]
                    .iter()
                    .all(|ran| ran.status() == "blocked")
            {
                stopped = format!(
                    "sessions {} and {number} were both blocked: {}",
                    number - 1,
                    crate::judge::clip(&sessions[sessions.len() - 1].summary(), 300)
                );
                break;
            }
        }
        Some((sessions, moves, stopped))
    }

    /// One session's brief: the task first, then the group and its
    /// evidence, then the state.
    #[allow(clippy::too_many_arguments)]
    fn brief(
        &self,
        prepared: &Prepared,
        groups: &[Group],
        cursor: usize,
        attempt: u32,
        sessions: &[Ran],
        checked: Option<&Checked>,
        read_only: bool,
    ) -> Brief {
        let group = &groups[cursor];
        let later: Vec<String> = groups[cursor + 1..]
            .iter()
            .flat_map(|g| g.ids.clone())
            .collect();
        let earlier: Vec<String> = groups[..cursor]
            .iter()
            .flat_map(|g| g.ids.clone())
            .collect();
        let mut guidance = format!(
            "This task runs as several short sessions. This session works only on {}:\n\n{}\n\n",
            if group.ids.len() == 1 {
                "this requirement"
            } else {
                "these requirements"
            },
            group.lines.join("\n")
        );
        if !earlier.is_empty() {
            guidance.push_str(&format!(
                "Earlier sessions worked on {}; keep their work intact. ",
                earlier.join(", ")
            ));
        }
        if !later.is_empty() {
            guidance.push_str(&format!(
                "Later sessions work on {}; leave them unless this work needs them. ",
                later.join(", ")
            ));
        }
        if self.policy.focus_actionable {
            let constraints = constraints(&prepared.requirements);
            if !constraints.is_empty() {
                guidance.push_str(&format!(
                    "\n\nThese constraints are the decisive facts of the task; honor every one \
                     exactly in what you do here. Use the exact values, formulas, seeds, \
                     column names, ranges, and formats they state. Do not substitute a \
                     simpler rule or an approximation for what a constraint specifies, and do \
                     not skip one because it looks minor.\n\n{}\n\n",
                    constraints.join("\n")
                ));
            }
        }
        if read_only {
            guidance.push_str(
                "This is a read-only session: you can't edit files, only read and run \
                 commands. Reproduce the problem and run the task's own tests or example so \
                 the next session starts from what actually happens. When you've seen enough, \
                 call finish with a summary of what you found and what the edit session should \
                 change.",
            );
        } else if self.policy.require_evidence {
            guidance.push_str(
                "Make the change the focus needs, then run something that exercises it: the \
                 task's test, its example, or a command that shows the new behavior. Call \
                 finish with status done only once the workspace shows the change and you've \
                 seen it work; if the focus asks only a question, answer it in finish and say \
                 what you ran to be sure. If you can't meet the focus, call finish with blocked \
                 or failed and say why.",
            );
        } else {
            guidance.push_str(
                "Do what the focus needs: change a file if it asks for a change, or answer in \
                 finish if it asks a question. Check your work by running something when you \
                 can, then call finish with status done. If the focus needs no change, say so \
                 and finish. If you can't meet it, call finish with blocked or failed and say \
                 why.",
            );
        }
        let mut state = vec![format!(
            "Session {} of at most {}; attempt {attempt} on this focus.",
            sessions.len() + 1,
            self.policy.max_sessions
        )];
        for ran in sessions {
            state.push(format!(
                "Session {} on {} ended {}: {}",
                ran.number,
                ran.focus.join(", "),
                ran.status(),
                crate::judge::clip(&ran.summary(), 400)
            ));
        }
        if !sessions.is_empty() {
            let changes = crate::delegate::changes(&self.workdir, None);
            state.push(format!(
                "What the workspace shows as changed now:\n{}",
                crate::judge::clip(&changes, 1_500)
            ));
        }
        if let Some(checked) = checked {
            let lines = checked.lines();
            if !lines.is_empty() {
                state.push(format!(
                    "The host's checks after the last session: {}",
                    lines.join("; ")
                ));
            }
        }
        Brief {
            task: prepared.instruction.clone(),
            guidance,
            evidence: evidence_for(prepared, group, self.policy.evidence_chars),
            state,
        }
    }
}

impl Executor for Micro {
    fn agent(&self) -> &str {
        AGENT
    }

    fn cost_provenance(&self) -> &'static str {
        "price_estimate"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn deadline(&self) -> Duration {
        self.deadline
    }

    fn describe(&self) -> Map<String, Value> {
        let mut map = Map::new();
        map.insert("transport".to_string(), json!("chatgpt-codex-responses"));
        map.insert("credential".to_string(), json!("codex_auth_json"));
        map.insert("isolation".to_string(), json!(self.isolation.word()));
        map.insert(
            "microluna".to_string(),
            serde_json::to_value(&self.policy).unwrap_or(Value::Null),
        );
        let (capabilities, note) = crate::adapter::capabilities(crate::delegate::Agent::Microluna);
        map.insert("capabilities".to_string(), capabilities.record(AGENT, note));
        map
    }

    fn take_evidence(&mut self, prepared: &Prepared) {
        self.prepared = Some(prepared.clone());
    }

    async fn execute(&mut self, briefing: &Briefing) -> Report {
        let started = Instant::now();
        let prepared = self.prepared.clone().filter(|p| {
            !groups(
                &p.requirements,
                self.policy.max_groups,
                self.policy.focus_actionable,
            )
            .is_empty()
        });
        let (sessions, moves, stopped, mode) = match (&self.policy.mode, prepared) {
            (Mode::Requirements, Some(prepared)) => {
                let suited = if self.policy.suite {
                    self.suite_loop(&prepared).await
                } else {
                    None
                };
                let (sessions, moves, stopped) = match suited {
                    Some(done) => done,
                    None => self.requirements(&prepared).await,
                };
                (sessions, moves, stopped, Mode::Requirements)
            }
            _ => (
                self.single(briefing).await,
                Vec::new(),
                "the one session ended".to_string(),
                Mode::Single,
            ),
        };
        let mut usage = TokenUsage::default();
        let mut cost = Some(0.0);
        for ran in &sessions {
            usage.add(ran.usage);
            cost = match (cost, ran.cost_usd) {
                (Some(total), Some(one)) => Some(total + one),
                _ => None,
            };
        }
        let turns: usize = sessions.iter().map(|r| r.turns).sum();
        let calls: usize = sessions.iter().map(|r| r.calls).sum();
        let all_lost = !sessions.is_empty()
            && sessions
                .iter()
                .all(|r| matches!(r.ending, Ending::Transport(_)));
        let status = if sessions.is_empty() {
            Status::Harness(format!("no session ran: {stopped}"))
        } else if all_lost {
            let detail = match &sessions[0].ending {
                Ending::Transport(why) => why.clone(),
                _ => String::new(),
            };
            Status::Transport {
                detail,
                reached: turns > 0,
            }
        } else if matches!(sessions.last().map(|r| &r.ending), Some(Ending::Deadline)) {
            Status::TimedOut
        } else {
            Status::Answered
        };
        let last = sessions.last();
        let mut result = format!(
            "Microluna ran {} session{} ({}): {stopped}.",
            sessions.len(),
            if sessions.len() == 1 { "" } else { "s" },
            mode.word()
        );
        for ran in &sessions {
            result.push_str(&format!(
                "\n- Session {} on {}: {}. {}",
                ran.number,
                ran.focus.join(", "),
                ran.status(),
                crate::judge::clip(&ran.summary(), 300)
            ));
        }
        if let Some(finish) = last.and_then(|r| r.finish.as_ref())
            && !finish.answer.trim().is_empty()
        {
            result.push_str(&format!("\n\nAnswer: {}", finish.answer));
        }
        let record = json!({
            "schema": LOOP_SCHEMA,
            "mode": mode.word(),
            "policy": self.policy,
            "model": self.model,
            "stopped": stopped,
            "sessions": sessions.iter().map(Ran::record).collect::<Vec<_>>(),
            "moves": moves,
            "usage": usage_json(usage),
            "cost_usd": cost,
            "stopped_by": Value::Null,
            "session_id": last.map(|r| r.session_id.clone()),
        });
        let file = self
            .artifacts
            .join(format!("microluna-{}.json", self.dispatch()));
        if let Ok(text) = serde_json::to_string_pretty(&record) {
            let _ = crate::record::write_atomic(&file, text.as_bytes());
        }
        self.last = Some(record);
        self.runs += 1;
        let milliseconds = millis(started);
        Report {
            status,
            summary: Summary {
                has_result: true,
                result: Some(result),
                is_error: Some(all_lost),
                subtype: Some(format!("microluna-{}", mode.word())),
                num_turns: u64::try_from(turns).ok(),
                total_cost_usd: cost,
                duration_ms: Some(milliseconds),
                duration_api_ms: None,
                session_id: last.map(|r| r.session_id.clone()),
                usage: Some(json!({
                    "input_tokens": usage.uncached(),
                    "cache_read_input_tokens": usage.cached,
                    "output_tokens": usage.output,
                    "reasoning_output_tokens": usage.reasoning,
                })),
                model_usage: Some(json!({
                    self.model.clone(): usage_json(usage),
                })),
                model: Some(self.model.clone()),
                version: Some(crate::episode::version()),
                api_key_source: Some("codex_auth_json".to_string()),
                api_calls: u64::try_from(turns).ok(),
                completed_items: u64::try_from(calls).ok(),
                input_per_call: Vec::new(),
                cost_provenance: Some("price_estimate"),
                cost_note: Some(microluna::price::COST_NOTE),
                limit: None,
            },
            milliseconds,
            stderr: String::new(),
            stream: Some(json!({
                "path": format!("artifacts/microluna-{}.json", self.runs),
                "sessions": sessions.iter().map(|r| r.trace.clone()).collect::<Vec<_>>(),
            })),
        }
    }
}

#[cfg(test)]
mod tests;
