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
//!   each session, code runs the checks and Jev chooses the next move:
//!   `next`, `retry`, `stuck`, or `done`. Code keeps the last word: a move
//!   past a requirement a check contradicts becomes a retry, and every
//!   loop is bounded by sessions, spend, and time.
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
use crate::record::{Cost, Finish, Implementation, Outcome, Recorder, Start};
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
        }
    }
}

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

/// The requirement groups: every requirement but context, in map order,
/// split into at most `max` consecutive groups of near-equal size.
#[must_use]
pub fn groups(map: &crate::requirements::RequirementMap, max: u32) -> Vec<Group> {
    let kept: Vec<_> = map
        .requirements
        .iter()
        .filter(|r| r.kind != Kind::Context)
        .collect();
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

/// The code rule over Jev's pick: a check that contradicts the group keeps
/// it from moving on, attempts are bounded, and without an answer the
/// session's own typed status decides.
fn settle(
    picked: Option<Move>,
    ran: &Ran,
    contradicted: bool,
    attempts: u32,
    max_attempts: u32,
) -> (Move, Option<String>) {
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
    async fn session(&self, number: u32, focus: &[String], why: &str, brief: &Brief) -> Ran {
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
            self.recorder.push(Step::said(
                Source::System,
                &format!(
                    "Delegating to {AGENT} ({}) because {why}. Briefing: {} characters, sha256 {}.",
                    self.model,
                    text.chars().count(),
                    sha256(&text)
                ),
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
        let sink = {
            let recorder = self.recorder.clone();
            let session_id = session_id.clone();
            let seq = seq.clone();
            let revision = revision.clone();
            let commands = commands.clone();
            let changed = changed.clone();
            move |step: &Step| {
                if step.call.is_some() || step.source == atif::Source::Agent {
                    crate::say::line(&format!("  {}", microluna::session::line(step)));
                }
                for kind in events_of(step) {
                    match &kind {
                        EventKind::CommandCompleted {
                            command, exit_code, ..
                        } => commands.borrow_mut().push((command.clone(), *exit_code)),
                        EventKind::ArtifactChanged { path, .. } => {
                            revision.set(revision.get() + 1);
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
        let workspace = microluna::Workspace::new(&self.workdir)
            .map(|workspace| workspace.isolated_by(self.isolation));
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
        };
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
                "trace": ran.trace,
            }))
            .cost(match (ran.cost_usd, &ran.ending) {
                (Some(usd), Ending::Deadline) => Cost {
                    usd: Some(usd),
                    provenance: "price_estimate_lower_bound".to_string(),
                },
                (Some(usd), _) => Cost {
                    usd: Some(usd),
                    provenance: "price_estimate".to_string(),
                },
                (None, _) => Cost::unknown(),
            }),
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
            contradicted,
            attempts,
            self.policy.max_attempts,
        );
        let usd = asked.input_tokens.map_or(0.0, |tokens| {
            tokens as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
        });
        let record = json!({
            "after_session": ran.number,
            "focus": group.ids,
            "jev": { "how": asked.how, "picked": picked.map(Move::word), "probabilities": p, "error": asked.error },
            "contradicted": contradicted,
            "checks": checked.summary,
            "attempts": attempts,
            "move": chosen.word(),
            "overridden": overridden,
            "jev_usd": usd,
        });
        (chosen, record, usd)
    }

    /// The mini-handoff loop.
    async fn requirements(&self, prepared: &Prepared) -> (Vec<Ran>, Vec<Value>, String) {
        let groups = groups(&prepared.requirements, self.policy.max_groups);
        let subject = self.subject(prepared);
        let dir = self
            .artifacts
            .parent()
            .map_or_else(|| self.artifacts.clone(), Path::to_path_buf);
        let started = Instant::now();
        let mut sessions: Vec<Ran> = Vec::new();
        let mut moves: Vec<Value> = Vec::new();
        let mut attempts = vec![0u32; groups.len()];
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
            attempts[cursor] += 1;
            let brief = self.brief(
                prepared,
                &groups,
                cursor,
                attempts[cursor],
                &sessions,
                last_checked.as_ref(),
            );
            let why = format!(
                "session {number} works on {} (group {} of {}, attempt {})",
                group.ids.join(", "),
                cursor + 1,
                groups.len(),
                attempts[cursor]
            );
            let ran = self.session(number, &group.ids, &why, &brief).await;
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

    /// One session's brief: the task first, then the group and its
    /// evidence, then the state.
    fn brief(
        &self,
        prepared: &Prepared,
        groups: &[Group],
        cursor: usize,
        attempt: u32,
        sessions: &[Ran],
        checked: Option<&Checked>,
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
        guidance.push_str(
            "When the focus is met and you've checked it by running something, call finish \
             with status done. If you can't meet it, call finish with blocked or failed and \
             say why in the summary.",
        );
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
        let prepared = self
            .prepared
            .clone()
            .filter(|p| !groups(&p.requirements, self.policy.max_groups).is_empty());
        let (sessions, moves, stopped, mode) = match (&self.policy.mode, prepared) {
            (Mode::Requirements, Some(prepared)) => {
                let (sessions, moves, stopped) = self.requirements(&prepared).await;
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
