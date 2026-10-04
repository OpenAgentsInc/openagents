//! The Agent Studio coordinator: goals, plans, dependencies, seats, shared
//! memory, and messages over the durable task inbox
//! (`docs/verse/agent-studio.md`, "The host owns state").
//!
//! The coordinator owns only what the task store does not:
//!
//! - **Seats.** Named roles (one or more leads, any number of workers),
//!   each bound to an auto-start route (`PROVIDER:MODEL`), a look, and a
//!   desk. A seat is configuration, not an engine.
//! - **Goals.** The text a person submitted, the admitted repository
//!   (a workspace label and its root), the lead seat, the lead's task,
//!   and the plan entries.
//! - **Plans.** The lead is an ordinary task whose reply ends with a plan
//!   ([`Plan`], schema [`PLAN_SCHEMA`]). [`validate`] checks it (a task
//!   count bound, unique identities, known dependencies, no cycles, known
//!   seats) before anything is created; a plan that fails becomes a
//!   [`Decision`] for the person, never a partial plan.
//! - **Dependencies.** A plan entry is held, not queued, until every task
//!   it depends on is done. Then the coordinator submits it to the inbox
//!   ([`Studio::reconcile`]).
//! - **Shared memory.** Entries every briefing carries: the accepted plan,
//!   decisions, and repository conventions.
//! - **Messages.** A message to a seat with a running task arrives through
//!   the existing steer path ([`super::steer`]), never by editing its
//!   prompt; otherwise it waits and the seat's next briefing carries it.
//!
//! Submission stays the inert inbox submission it always is: a released
//! task records the seat's route model and asks for the seat's provider,
//! and with a host root ([`Studio::with_host_root`]) the coordinator notes
//! it eligible in the auto-start journal, so the owner's policy starts it
//! under its own bounds. The coordinator never chooses a route around that
//! policy, never runs an engine, and never lands anything.
//!
//! State is one private document, `<store>/studio/state.json`, replaced
//! atomically with the task store's own file helpers, under the stable
//! lock `<store>/studio/studio.lock` that a [`Studio`] holds while open.
//! A release first saves the exact command bytes beside the entry, then
//! applies them, then marks the entry submitted; a restart between those
//! steps re-applies the same bytes, which the inbox answers with the
//! original receipt, so no task is created twice and none is lost.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use nostr::contracts::{digest_bytes, parse_strict_bounded};
use serde::{Deserialize, Serialize};

use super::autostart::{self, Route};
use super::capacity::Provider;
use super::{
    Action, COMMAND_SCHEMA, Command, Execution, RequestedConfiguration, Status, Store, Task,
    TaskIntent, Workspace, interaction,
};

/// The persisted coordinator document.
pub const STATE_SCHEMA: &str = "openagents.coder.studio.v1";
/// The plan a lead's reply carries.
pub const PLAN_SCHEMA: &str = "openagents.coder.studio-plan.v1";
/// The coordinator's private directory inside the task store.
pub const DIR: &str = "studio";
/// The coordinator document within [`DIR`].
pub const STATE_FILE: &str = "state.json";
/// The stable lock within [`DIR`].
pub const LOCK_FILE: &str = "studio.lock";
/// The device the auto-start journal records for a released task.
pub const DEVICE: &str = "studio";

/// The most seats.
pub const MAX_SEATS: usize = 16;
/// The most retained goals. No goal is silently pruned.
pub const MAX_GOALS: usize = 256;
/// The most tasks one plan holds.
pub const MAX_PLAN_TASKS: usize = 32;
/// The most dependencies one plan task names.
pub const MAX_DEPENDENCIES: usize = 16;
/// The largest plan document, in bytes.
pub const MAX_PLAN_BYTES: usize = 64 * 1024;
/// The largest goal text, in bytes.
pub const MAX_GOAL_BYTES: usize = 4 * 1024;
/// The largest plan task description, in bytes.
pub const MAX_DESCRIPTION_BYTES: usize = 4 * 1024;
/// The most retained shared memory entries; the oldest goes first.
pub const MAX_MEMORY: usize = 128;
/// The largest memory entry, in bytes.
pub const MAX_MEMORY_BYTES: usize = 2 * 1024;
/// The most retained messages; the oldest delivered one goes first.
pub const MAX_MESSAGES: usize = 256;
/// The largest message, in bytes.
pub const MAX_MESSAGE_BYTES: usize = 4 * 1024;
/// How much of a briefing shared memory may fill, in bytes.
const MEMORY_BUDGET: usize = 12 * 1024;
/// How much of a briefing waiting messages may fill, in bytes.
const MESSAGE_BUDGET: usize = 6 * 1024;
/// The largest coordinator document, in bytes.
const MAX_STATE_BYTES: usize = 8 * 1024 * 1024;
const LOCK_WAIT: Duration = Duration::from_secs(5);

/// The task inbox as the coordinator uses it: apply an exact command,
/// and read a task. [`Store`] is the inbox; a test or the simulated team
/// can stand in for it.
pub trait Inbox {
    /// Apply `command`'s exact bytes, or accept an exact-byte retry.
    ///
    /// # Errors
    /// The inbox refuses the command.
    fn apply(&mut self, command: &[u8]) -> Result<(), super::Error>;

    /// The task `task_id`, if the inbox holds it.
    fn task(&self, task_id: &str) -> Option<Task>;
}

impl Inbox for Store {
    fn apply(&mut self, command: &[u8]) -> Result<(), super::Error> {
        Store::apply(self, command).map(|_| ())
    }

    fn task(&self, task_id: &str) -> Option<Task> {
        self.show(task_id).ok()
    }
}

/// A seat's part in the team.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Plans a goal: its task's reply is the plan.
    Lead,
    /// Works plan entries.
    Worker,
}

/// A named studio role bound to a route, a look, and a desk.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Seat {
    /// Lowercase letters, digits, and hyphens, at most 32 bytes.
    pub name: String,
    pub role: Role,
    /// The auto-start route this seat's tasks ask for. A task starts only
    /// when the owner's policy admits it.
    pub route: Route,
    /// The character look a view draws the seat with.
    pub look: String,
    /// The seat's desk, from zero; unique among seats.
    pub desk: u32,
}

/// An admitted repository: a host workspace label and its root.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    pub label: String,
    pub path: String,
}

/// Where a goal's task is on its way to the inbox.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotState {
    /// Waiting on its dependencies; no task exists.
    Held,
    /// Its exact command is saved and may or may not be applied.
    Releasing,
    /// The inbox holds the task.
    Submitted,
}

/// One task a goal plans or has created.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub task_id: String,
    pub seat: String,
    pub state: SlotState,
    /// The exact command bytes while [`SlotState::Releasing`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// One task of an accepted plan.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    pub slot: Slot,
}

/// Why a goal waits on the person.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    /// The lead's plan failed validation.
    InvalidPlan,
    /// The lead finished without a plan in its reply.
    NoPlan,
    /// The lead's task failed or was cancelled.
    LeadFailed,
    /// A plan entry's dependency failed or was cancelled, so it cannot start.
    DependencyFailed,
}

/// A goal's open question for the person. Delivering a plan
/// ([`Studio::accept_plan`]) answers the plan kinds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub kind: DecisionKind,
    /// Every reason, in words, naming plan identities, never task output.
    pub reasons: Vec<String>,
    /// The coordinator sequence that opened it.
    pub sequence: u64,
}

/// A submitted goal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    pub goal_id: String,
    pub text: String,
    pub repository: Repository,
    /// The lead's task.
    pub lead: Slot,
    /// Whether a plan was accepted. An accepted plan is never replaced.
    pub planned: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plan: Vec<PlanEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<Decision>,
    /// When it was submitted, in Unix seconds.
    pub submitted_at: u64,
}

/// Who sent or receives a message or wrote a memory entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Party {
    Person,
    Seat {
        name: String,
    },
    /// Every seat; a message to everyone is one message per seat.
    Everyone,
}

impl std::fmt::Display for Party {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Person => formatter.write_str("the person"),
            Self::Seat { name } => write!(formatter, "@{name}"),
            Self::Everyone => formatter.write_str("everyone"),
        }
    }
}

/// How a message reached, or will reach, its recipient.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Delivery {
    /// Left for the seat's running task through the steer path.
    Steered { task_id: String },
    /// Waiting for the seat's next briefing.
    Waiting,
    /// Carried in the briefing of the seat's next task.
    Briefed { task_id: String },
    /// Kept for the person, who reads it in the studio.
    Recorded,
}

/// One message.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub sequence: u64,
    pub at: u64,
    pub from: Party,
    pub to: Party,
    pub text: String,
    pub delivery: Delivery,
}

/// What a shared memory entry records.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// An accepted plan, written by the coordinator.
    Plan,
    /// A decision the team or the person made.
    Decision,
    /// A repository convention.
    Convention,
    Note,
}

/// One entry every briefing carries.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryEntry {
    pub sequence: u64,
    pub kind: MemoryKind,
    pub author: Party,
    /// The goal it belongs to; absent for the whole studio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<String>,
    pub text: String,
}

/// The persisted coordinator document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub schema: String,
    /// Raised by every change; a view compares it to know it is current.
    pub sequence: u64,
    pub seats: Vec<Seat>,
    pub goals: Vec<Goal>,
    pub memory: Vec<MemoryEntry>,
    pub messages: Vec<Message>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            schema: STATE_SCHEMA.into(),
            sequence: 0,
            seats: Vec::new(),
            goals: Vec::new(),
            memory: Vec::new(),
            messages: Vec::new(),
        }
    }
}

impl State {
    /// The seat named `name`.
    #[must_use]
    pub fn seat(&self, name: &str) -> Option<&Seat> {
        self.seats.iter().find(|seat| seat.name == name)
    }

    /// The goal `goal_id`.
    #[must_use]
    pub fn goal(&self, goal_id: &str) -> Option<&Goal> {
        self.goals.iter().find(|goal| goal.goal_id == goal_id)
    }
}

/// A lead's plan, as its reply carries it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub tasks: Vec<PlannedTask>,
}

/// One task of a [`Plan`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedTask {
    /// Lowercase letters, digits, and hyphens, at most 32 bytes.
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// A suggested seat; the coordinator assigns a worker when absent.
    #[serde(default)]
    pub seat: Option<String>,
}

/// A goal to submit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewGoal {
    pub text: String,
    pub repository: Repository,
    /// The lead seat; the first lead seat when absent.
    pub lead: Option<String>,
}

/// A task the coordinator just put in the inbox.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Released {
    pub goal_id: String,
    pub task_id: String,
    pub seat: String,
    /// The repository's workspace label.
    pub workspace: String,
    /// The provider the task asks for first.
    pub provider: Provider,
}

/// What delivering a plan did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanOutcome {
    /// The plan is the goal's; these entries had no dependencies.
    Accepted { released: Vec<Released> },
    /// The plan failed validation and is the goal's open decision.
    Decision(Decision),
}

/// Where a goal's task is, read from the inbox.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Progress {
    /// Not in the inbox yet: waiting on its dependencies.
    Held,
    /// A dependency failed or was cancelled, so it cannot start.
    Blocked,
    Queued,
    Running,
    /// Its turn ended with a question or an approval for the person.
    Waiting,
    Done,
    Failed,
    Cancelled,
    /// The inbox does not hold the task the coordinator recorded.
    Missing,
}

impl Progress {
    /// The task will not change without a person.
    #[must_use]
    pub fn is_final(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Failed | Self::Cancelled | Self::Missing
        )
    }

    /// The task is in the inbox and not over.
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Waiting)
    }
}

/// A task's [`Progress`] from its inbox record.
#[must_use]
pub fn progress_of(task: &Task) -> Progress {
    if interaction::pending(task).is_some() {
        return Progress::Waiting;
    }
    match task.status {
        Status::Queued => Progress::Queued,
        Status::Running | Status::CancelRequested => Progress::Running,
        Status::Cancelled => Progress::Cancelled,
        Status::Finished if task.execution == Execution::Finished => Progress::Done,
        Status::Finished | Status::Unknown => Progress::Failed,
    }
}

/// A goal's state as a view shows it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    /// The lead is planning.
    Planning,
    /// An open [`Decision`] waits on the person.
    Decision,
    /// The plan's tasks are under way.
    Running,
    /// Every plan task is over.
    Done,
}

/// A seat with what it is doing now.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SeatView {
    pub seat: Seat,
    /// Its active task, the newest when more than one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<Progress>,
}

/// A plan entry with its task's progress.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EntryView {
    pub id: String,
    pub title: String,
    pub seat: String,
    pub depends_on: Vec<String>,
    pub task_id: String,
    pub progress: Progress,
}

/// A goal with its progress.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GoalView {
    pub goal_id: String,
    pub text: String,
    pub workspace: String,
    pub lead_seat: String,
    pub lead_task_id: String,
    pub lead_progress: Progress,
    pub status: GoalStatus,
    /// Plan tasks that are over, of [`GoalView::total_tasks`].
    pub final_tasks: usize,
    pub total_tasks: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<Decision>,
    pub entries: Vec<EntryView>,
}

/// The whole coordinator, joined with the inbox: what a view draws.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct View {
    pub sequence: u64,
    pub seats: Vec<SeatView>,
    pub goals: Vec<GoalView>,
    pub memory: Vec<MemoryEntry>,
    pub messages: Vec<Message>,
}

/// A closed refusal classification.
#[derive(Debug)]
pub enum Error {
    /// The task inbox refused.
    Tasks(super::Error),
    /// A field is out of bounds or malformed; the message says which.
    Invalid(String),
    UnknownSeat(String),
    UnknownGoal(String),
    /// The goal or seat is not in a state that allows this.
    State(String),
    LimitExceeded(&'static str),
    Corrupt(&'static str),
}

impl Error {
    /// Stable refusal codes for machine-readable callers.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Tasks(error) => error.code(),
            Self::Invalid(_) => "invalid",
            Self::UnknownSeat(_) => "unknown_seat",
            Self::UnknownGoal(_) => "unknown_goal",
            Self::State(_) => "invalid_state",
            Self::LimitExceeded(_) => "limit_exceeded",
            Self::Corrupt(_) => "corrupt_studio",
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tasks(error) => write!(formatter, "{error}"),
            Self::Invalid(message) | Self::State(message) => formatter.write_str(message),
            Self::UnknownSeat(name) => write!(formatter, "no seat is named `{name}`"),
            Self::UnknownGoal(goal) => write!(formatter, "no goal is `{goal}`"),
            Self::LimitExceeded(what) => write!(formatter, "the studio holds the most {what}"),
            Self::Corrupt(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

impl From<super::Error> for Error {
    fn from(error: super::Error) -> Self {
        Self::Tasks(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Tasks(super::Error::Io(error))
    }
}

/// A seat name or plan identity: lowercase letters, digits, and hyphens,
/// starting with a letter or digit, at most 32 bytes.
#[must_use]
pub fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Parse `PROVIDER:MODEL` as `coder host autostart on --route` takes it.
///
/// # Errors
/// The provider is not one a repository run uses, or the model is
/// missing or malformed.
pub fn parse_route(text: &str) -> Result<Route, Error> {
    autostart::parse_route(text).map_err(|message| {
        Error::Invalid(
            message
                .trim_start_matches("usage: ")
                .replace("--route", "a route"),
        )
    })
}

/// Check `bytes` as a plan for a team of `seats`. Every problem is
/// returned at once, so one decision names them all.
///
/// # Errors
/// The reasons the plan is not acceptable.
pub fn validate(bytes: &[u8], seats: &[Seat]) -> Result<Plan, Vec<String>> {
    if bytes.len() > MAX_PLAN_BYTES {
        return Err(vec![format!(
            "the plan is larger than {MAX_PLAN_BYTES} bytes"
        )]);
    }
    let value = parse_strict_bounded(bytes, MAX_PLAN_BYTES)
        .map_err(|_| vec!["the plan is not strict JSON without duplicate keys".to_owned()])?;
    let plan: Plan = serde_json::from_value(value)
        .map_err(|error| vec![format!("the plan does not match its schema: {error}")])?;
    let mut reasons = Vec::new();
    if plan.schema != PLAN_SCHEMA {
        reasons.push(format!("the plan's schema is not {PLAN_SCHEMA}"));
    }
    if plan.tasks.is_empty() {
        reasons.push("the plan has no tasks".into());
    }
    if plan.tasks.len() > MAX_PLAN_TASKS {
        reasons.push(format!(
            "the plan has {} tasks; the most is {MAX_PLAN_TASKS}",
            plan.tasks.len()
        ));
    }
    let mut ids = BTreeSet::new();
    for task in &plan.tasks {
        if !valid_name(&task.id) {
            reasons.push(format!(
                "task id `{}` is not 1 to 32 lowercase letters, digits, and hyphens",
                cut(&task.id, 40)
            ));
        } else if !ids.insert(task.id.as_str()) {
            reasons.push(format!("task id `{}` appears twice", task.id));
        }
    }
    for task in &plan.tasks {
        let id = cut(&task.id, 40);
        if !super::text(&task.title, 256, false) {
            reasons.push(format!(
                "task `{id}` needs a one-line title of 1 to 256 bytes"
            ));
        }
        if task.description.len() > MAX_DESCRIPTION_BYTES
            || !(task.description.is_empty() || super::text(&task.description, usize::MAX, true))
        {
            reasons.push(format!(
                "task `{id}`'s description is longer than {MAX_DESCRIPTION_BYTES} bytes or holds control characters"
            ));
        }
        if task.depends_on.len() > MAX_DEPENDENCIES {
            reasons.push(format!(
                "task `{id}` depends on more than {MAX_DEPENDENCIES} tasks"
            ));
        }
        let mut seen = BTreeSet::new();
        for dependency in &task.depends_on {
            if dependency == &task.id {
                reasons.push(format!("task `{id}` depends on itself"));
            } else if !ids.contains(dependency.as_str()) {
                reasons.push(format!(
                    "task `{id}` depends on `{}`, which the plan does not hold",
                    cut(dependency, 40)
                ));
            } else if !seen.insert(dependency.as_str()) {
                reasons.push(format!("task `{id}` names `{dependency}` twice"));
            }
        }
        if let Some(seat) = &task.seat
            && !seats.iter().any(|known| &known.name == seat)
        {
            reasons.push(format!(
                "task `{id}` names seat `{}`, which the studio does not have",
                cut(seat, 40)
            ));
        }
    }
    if plan.tasks.iter().any(|task| task.seat.is_none())
        && !seats.iter().any(|seat| seat.role == Role::Worker)
    {
        reasons.push("a task names no seat and the studio has no worker seat".into());
    }
    if reasons.is_empty()
        && let Some(cycle) = cycle(&plan.tasks)
    {
        reasons.push(format!(
            "the dependencies form a cycle through {}",
            cycle
                .iter()
                .map(|id| format!("`{id}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if reasons.is_empty() {
        Ok(plan)
    } else {
        Err(reasons)
    }
}

/// The tasks left after removing every task whose dependencies can all
/// finish: those on or behind a cycle, in plan order. `None` when acyclic.
fn cycle(tasks: &[PlannedTask]) -> Option<Vec<String>> {
    let mut waiting: BTreeMap<&str, usize> = tasks
        .iter()
        .map(|task| (task.id.as_str(), task.depends_on.len()))
        .collect();
    let mut ready: Vec<&str> = waiting
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut done = BTreeSet::new();
    while let Some(id) = ready.pop() {
        done.insert(id);
        for task in tasks {
            if task.depends_on.iter().any(|dependency| dependency == id)
                && let Some(count) = waiting.get_mut(task.id.as_str())
            {
                *count -= 1;
                if *count == 0 {
                    ready.push(task.id.as_str());
                }
            }
        }
    }
    let left: Vec<String> = tasks
        .iter()
        .filter(|task| !done.contains(task.id.as_str()))
        .map(|task| task.id.clone())
        .collect();
    (!left.is_empty()).then_some(left)
}

/// The plan in a lead's reply: the last fenced code block that holds
/// [`PLAN_SCHEMA`], else the whole reply when it is a JSON object.
#[must_use]
pub fn plan_in_reply(reply: &str) -> Option<String> {
    let mut found = None;
    let mut rest = reply;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let body_start = after.find('\n').map_or(after.len(), |line| line + 1);
        let body = &after[body_start..];
        let Some(end) = body.find("```") else {
            break;
        };
        let block = body[..end].trim();
        if block.contains(PLAN_SCHEMA) {
            found = Some(block.to_owned());
        }
        rest = &body[end + 3..];
    }
    found.or_else(|| {
        let whole = reply.trim();
        (whole.starts_with('{') && whole.contains(PLAN_SCHEMA)).then(|| whole.to_owned())
    })
}

/// The coordinator over one task store, holding its lock while open.
pub struct Studio {
    store: PathBuf,
    dir: PathBuf,
    lock: File,
    host_root: Option<PathBuf>,
    state: State,
    #[cfg(test)]
    fault: Option<Fault>,
}

/// Where a test stops a release, as a crash would.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    /// After the command is saved, before the inbox holds it.
    BeforeApply,
    /// After the inbox holds it, before the entry is marked submitted.
    AfterApply,
}

impl Studio {
    /// Whether the task store at `store` holds a studio.
    #[must_use]
    pub fn present(store: &Path) -> bool {
        store.join(DIR).join(STATE_FILE).is_file()
    }

    /// Open the studio of the task store at `store`, creating its private
    /// directory, and hold its lock until dropped.
    ///
    /// # Errors
    /// The directory is not private, another process holds the studio
    /// for more than five seconds, or the document is unreadable.
    pub fn open(store: &Path) -> Result<Self, Error> {
        super::prepare_directory(store)?;
        let store = store.canonicalize()?;
        let dir = store.join(DIR);
        super::prepare_directory(&dir)?;
        let lock_path = dir.join(LOCK_FILE);
        let lock = super::open_lock(&lock_path)?;
        super::take_lock(&lock, LOCK_WAIT)?;
        super::verify_same_file(&lock_path, &lock)?;
        let path = dir.join(STATE_FILE);
        let state = if super::regular_or_absent(&path)? {
            let bytes = std::fs::read(&path)?;
            if bytes.len() > MAX_STATE_BYTES {
                return Err(Error::Corrupt("the studio document is too large"));
            }
            let state: State = serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("the studio document is malformed"))?;
            if state.schema != STATE_SCHEMA {
                return Err(Error::Corrupt(
                    "the studio document's schema is not supported",
                ));
            }
            state
        } else {
            State::default()
        };
        Ok(Self {
            store,
            dir,
            lock,
            host_root: None,
            state,
            #[cfg(test)]
            fault: None,
        })
    }

    /// Note each released task eligible in the auto-start journal of the
    /// host root `root`, so the owner's policy starts it.
    #[must_use]
    pub fn with_host_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.host_root = Some(root.into());
        self
    }

    /// The persisted document.
    #[must_use]
    pub fn state(&self) -> &State {
        &self.state
    }

    /// The task store directory this studio belongs to.
    #[must_use]
    pub fn store(&self) -> &Path {
        &self.store
    }

    fn save(&mut self) -> Result<(), Error> {
        super::verify_same_file(&self.dir.join(LOCK_FILE), &self.lock)?;
        self.state.sequence += 1;
        let bytes = serde_json::to_vec_pretty(&self.state)
            .map_err(|_| Error::Corrupt("the studio document could not be encoded"))?;
        if bytes.len() > MAX_STATE_BYTES {
            return Err(Error::LimitExceeded("bytes in its document"));
        }
        super::replace_file(&self.dir, STATE_FILE, &bytes)?;
        Ok(())
    }

    /// Add `seat`, or replace the seat of the same name. A replaced seat's
    /// route applies to tasks released from now on.
    ///
    /// # Errors
    /// A field is malformed, the desk is another seat's, or the studio
    /// holds [`MAX_SEATS`].
    pub fn set_seat(&mut self, seat: Seat) -> Result<(), Error> {
        if !valid_name(&seat.name) {
            return Err(Error::Invalid(
                "a seat name is 1 to 32 lowercase letters, digits, and hyphens".into(),
            ));
        }
        if !super::identifier(&seat.route.model, true) {
            return Err(Error::Invalid(format!(
                "seat `{}`'s model is not a task model identity",
                seat.name
            )));
        }
        if seat.look.is_empty() || seat.look.len() > 64 || !super::identifier(&seat.look, false) {
            return Err(Error::Invalid(
                "a look is 1 to 64 letters, digits, dots, hyphens, and underscores".into(),
            ));
        }
        if self
            .state
            .seats
            .iter()
            .any(|other| other.name != seat.name && other.desk == seat.desk)
        {
            return Err(Error::Invalid(format!(
                "desk {} is another seat's",
                seat.desk
            )));
        }
        match self
            .state
            .seats
            .iter()
            .position(|other| other.name == seat.name)
        {
            Some(index) => self.state.seats[index] = seat,
            None if self.state.seats.len() >= MAX_SEATS => {
                return Err(Error::LimitExceeded("seats"));
            }
            None => self.state.seats.push(seat),
        }
        self.save()
    }

    /// The lowest desk no seat has.
    #[must_use]
    pub fn free_desk(&self) -> u32 {
        (0..)
            .find(|desk| !self.state.seats.iter().any(|seat| seat.desk == *desk))
            .unwrap_or(0)
    }

    /// Remove the seat `name`.
    ///
    /// # Errors
    /// No such seat, or a goal holds a task for it that is not in the
    /// inbox yet.
    pub fn remove_seat(&mut self, name: &str) -> Result<(), Error> {
        if self.state.seat(name).is_none() {
            return Err(Error::UnknownSeat(name.into()));
        }
        let pending = self.state.goals.iter().any(|goal| {
            std::iter::once(&goal.lead)
                .chain(goal.plan.iter().map(|entry| &entry.slot))
                .any(|slot| slot.seat == name && slot.state != SlotState::Submitted)
        });
        if pending {
            return Err(Error::State(format!(
                "a goal holds a task for `{name}` that has not started; finish or reassign it first"
            )));
        }
        self.state.seats.retain(|seat| seat.name != name);
        self.save()
    }

    /// Add a shared memory entry. Returns its sequence.
    ///
    /// # Errors
    /// The text is empty, too long, or holds control characters, or the
    /// goal is unknown.
    pub fn remember(
        &mut self,
        kind: MemoryKind,
        author: Party,
        goal_id: Option<&str>,
        text: &str,
    ) -> Result<u64, Error> {
        let text = text.trim();
        if !super::text(text, MAX_MEMORY_BYTES, true) {
            return Err(Error::Invalid(format!(
                "a memory entry is 1 to {MAX_MEMORY_BYTES} bytes of text"
            )));
        }
        if let Some(goal) = goal_id
            && self.state.goal(goal).is_none()
        {
            return Err(Error::UnknownGoal(goal.into()));
        }
        self.push_memory(kind, author, goal_id.map(str::to_owned), text.to_owned());
        self.save()?;
        Ok(self.state.sequence)
    }

    fn push_memory(
        &mut self,
        kind: MemoryKind,
        author: Party,
        goal_id: Option<String>,
        text: String,
    ) {
        if self.state.memory.len() >= MAX_MEMORY {
            self.state.memory.remove(0);
        }
        self.state.memory.push(MemoryEntry {
            sequence: self.state.sequence + 1,
            kind,
            author,
            goal_id,
            text: cut(&text, MAX_MEMORY_BYTES),
        });
    }

    /// Submit a goal: record it and put its lead's planning task in the
    /// inbox. Returns the goal identity and the released lead task.
    ///
    /// # Errors
    /// The goal text or repository is malformed, the lead seat is unknown
    /// or not a lead, the studio holds [`MAX_GOALS`], or the inbox refuses.
    pub fn submit_goal(
        &mut self,
        tasks: &mut dyn Inbox,
        goal: NewGoal,
        now: u64,
    ) -> Result<(String, Released), Error> {
        let text = goal.text.trim();
        if !super::text(text, MAX_GOAL_BYTES, true) {
            return Err(Error::Invalid(format!(
                "a goal is 1 to {MAX_GOAL_BYTES} bytes of text"
            )));
        }
        let repository = goal.repository;
        let path = Path::new(&repository.path);
        if !super::identifier(&repository.label, false)
            || !path.is_absolute()
            || !super::text(&repository.path, 4096, false)
        {
            return Err(Error::Invalid(
                "a repository is a workspace label and an absolute path".into(),
            ));
        }
        let lead = match goal.lead {
            Some(name) => {
                let seat = self
                    .state
                    .seat(&name)
                    .ok_or_else(|| Error::UnknownSeat(name.clone()))?;
                if seat.role != Role::Lead {
                    return Err(Error::Invalid(format!("seat `{name}` is not a lead")));
                }
                name
            }
            None => self
                .state
                .seats
                .iter()
                .find(|seat| seat.role == Role::Lead)
                .map(|seat| seat.name.clone())
                .ok_or_else(|| Error::State("the studio has no lead seat".into()))?,
        };
        if self.state.goals.len() >= MAX_GOALS {
            return Err(Error::LimitExceeded("goals"));
        }
        let digest = digest_bytes(format!("{now}\n{text}").as_bytes());
        let goal_id = format!(
            "g{}-{}",
            self.state.goals.len() + 1,
            &digest["sha256:".len().."sha256:".len() + 8]
        );
        self.state.goals.push(Goal {
            goal_id: goal_id.clone(),
            text: text.to_owned(),
            repository,
            lead: Slot {
                task_id: format!("studio-{goal_id}-lead"),
                seat: lead,
                state: SlotState::Held,
                command: None,
            },
            planned: false,
            plan: Vec::new(),
            decision: None,
            submitted_at: now,
        });
        self.save()?;
        let index = self.state.goals.len() - 1;
        let released = self.release(tasks, index, None, now)?;
        Ok((goal_id, released))
    }

    /// Deliver a plan for `goal_id`, as its lead's reply or the person
    /// carries it. A valid plan becomes the goal's, its memory entry is
    /// written, and its entries without dependencies are released; an
    /// invalid one becomes the goal's open decision.
    ///
    /// # Errors
    /// The goal is unknown or already has a plan, or the inbox refuses.
    pub fn accept_plan(
        &mut self,
        tasks: &mut dyn Inbox,
        goal_id: &str,
        bytes: &[u8],
        now: u64,
    ) -> Result<PlanOutcome, Error> {
        let index = self.goal_index(goal_id)?;
        if self.state.goals[index].planned {
            return Err(Error::State(format!("goal `{goal_id}` already has a plan")));
        }
        match validate(bytes, &self.state.seats) {
            Ok(plan) => {
                self.adopt(index, plan);
                self.save()?;
                let released = self.release_ready(tasks, index, now)?;
                Ok(PlanOutcome::Accepted { released })
            }
            Err(reasons) => {
                let decision = self.decide(index, DecisionKind::InvalidPlan, reasons);
                self.save()?;
                Ok(PlanOutcome::Decision(decision))
            }
        }
    }

    fn goal_index(&self, goal_id: &str) -> Result<usize, Error> {
        self.state
            .goals
            .iter()
            .position(|goal| goal.goal_id == goal_id)
            .ok_or_else(|| Error::UnknownGoal(goal_id.into()))
    }

    fn decide(&mut self, index: usize, kind: DecisionKind, reasons: Vec<String>) -> Decision {
        let decision = Decision {
            kind,
            reasons,
            sequence: self.state.sequence + 1,
        };
        self.state.goals[index].decision = Some(decision.clone());
        decision
    }

    /// Make `plan` goal `index`'s, assigning each unassigned task the
    /// worker seat with the fewest tasks so far, and write its memory entry.
    fn adopt(&mut self, index: usize, plan: Plan) {
        let workers: Vec<String> = self
            .state
            .seats
            .iter()
            .filter(|seat| seat.role == Role::Worker)
            .map(|seat| seat.name.clone())
            .collect();
        let mut load: BTreeMap<String, usize> = BTreeMap::new();
        for task in &plan.tasks {
            if let Some(seat) = &task.seat {
                *load.entry(seat.clone()).or_default() += 1;
            }
        }
        let goal_id = self.state.goals[index].goal_id.clone();
        let mut entries = Vec::new();
        for task in plan.tasks {
            let seat = task.seat.unwrap_or_else(|| {
                let chosen = workers
                    .iter()
                    .min_by_key(|name| load.get(*name).copied().unwrap_or(0))
                    .cloned()
                    .unwrap_or_default();
                *load.entry(chosen.clone()).or_default() += 1;
                chosen
            });
            entries.push(PlanEntry {
                slot: Slot {
                    task_id: format!("studio-{goal_id}-{}", task.id),
                    seat,
                    state: SlotState::Held,
                    command: None,
                },
                id: task.id,
                title: task.title,
                description: task.description,
                depends_on: task.depends_on,
            });
        }
        let summary = entries
            .iter()
            .map(|entry| {
                let after = if entry.depends_on.is_empty() {
                    String::new()
                } else {
                    format!(" (after {})", entry.depends_on.join(", "))
                };
                format!(
                    "- {}: {} [@{}]{after}",
                    entry.id, entry.title, entry.slot.seat
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let goal = &mut self.state.goals[index];
        goal.plan = entries;
        goal.planned = true;
        goal.decision = None;
        let lead = goal.lead.seat.clone();
        self.push_memory(
            MemoryKind::Plan,
            Party::Seat { name: lead },
            Some(goal_id.clone()),
            format!("Plan for goal {goal_id}:\n{summary}"),
        );
    }

    /// Bring every goal forward: finish releases a restart interrupted,
    /// read each finished lead's reply for its plan (`lead_reply` answers
    /// a task identity with its reply), release plan entries whose
    /// dependencies are done, and open a decision where a lead or a
    /// dependency failed. Returns the tasks it released.
    ///
    /// # Errors
    /// The inbox or the studio document cannot be written. Releases made
    /// before the failure are kept and listed by the next call.
    pub fn reconcile(
        &mut self,
        tasks: &mut dyn Inbox,
        now: u64,
        lead_reply: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Vec<Released>, Error> {
        let mut released = Vec::new();
        for index in 0..self.state.goals.len() {
            if self.state.goals[index].lead.state != SlotState::Submitted {
                released.push(self.release(tasks, index, None, now)?);
            }
            let goal = &self.state.goals[index];
            if !goal.planned {
                if goal.decision.is_some() {
                    continue;
                }
                let lead = goal.lead.task_id.clone();
                match progress(tasks, &lead) {
                    Progress::Done => match lead_reply(&lead).as_deref().and_then(plan_in_reply) {
                        Some(plan) => {
                            let goal_id = goal.goal_id.clone();
                            if let PlanOutcome::Accepted { released: more } =
                                self.accept_plan(tasks, &goal_id, plan.as_bytes(), now)?
                            {
                                released.extend(more);
                            }
                        }
                        None => {
                            self.decide(
                                index,
                                DecisionKind::NoPlan,
                                vec!["the lead finished without a plan in its reply".into()],
                            );
                            self.save()?;
                        }
                    },
                    Progress::Failed | Progress::Cancelled | Progress::Missing => {
                        self.decide(
                            index,
                            DecisionKind::LeadFailed,
                            vec![format!("the lead's task {lead} did not finish")],
                        );
                        self.save()?;
                    }
                    _ => {}
                }
                continue;
            }
            released.extend(self.release_ready(tasks, index, now)?);
        }
        Ok(released)
    }

    /// Release goal `index`'s plan entries that are held or releasing and
    /// whose dependencies are done; open a decision for entries a failed
    /// dependency blocks.
    fn release_ready(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        now: u64,
    ) -> Result<Vec<Released>, Error> {
        let mut released = Vec::new();
        let mut blocked = Vec::new();
        for entry in 0..self.state.goals[index].plan.len() {
            let goal = &self.state.goals[index];
            let item = &goal.plan[entry];
            let state = item.slot.state;
            match state {
                SlotState::Submitted => continue,
                SlotState::Releasing => {
                    released.push(self.release(tasks, index, Some(entry), now)?);
                    continue;
                }
                SlotState::Held => {}
            }
            let mut ready = true;
            for dependency in &item.depends_on {
                let Some(other) = goal.plan.iter().find(|other| &other.id == dependency) else {
                    continue;
                };
                let state = if other.slot.state == SlotState::Submitted {
                    progress(tasks, &other.slot.task_id)
                } else {
                    Progress::Held
                };
                if state != Progress::Done {
                    ready = false;
                }
                if matches!(
                    state,
                    Progress::Failed | Progress::Cancelled | Progress::Missing
                ) {
                    blocked.push(format!(
                        "task `{}` waits on `{dependency}`, which did not finish",
                        item.id
                    ));
                }
            }
            if ready {
                released.push(self.release(tasks, index, Some(entry), now)?);
            }
        }
        let open = self.state.goals[index].decision.is_some();
        if !blocked.is_empty() && !open {
            self.decide(index, DecisionKind::DependencyFailed, blocked);
            self.save()?;
        }
        Ok(released)
    }

    /// Put goal `index`'s lead (`entry` `None`) or plan entry in the
    /// inbox: save the exact command, apply it, note it eligible, and
    /// mark it submitted. A slot already releasing applies its saved
    /// bytes again, which the inbox answers with the original receipt.
    fn release(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        entry: Option<usize>,
        now: u64,
    ) -> Result<Released, Error> {
        let slot = self.slot(index, entry).clone();
        let seat = self
            .state
            .seat(&slot.seat)
            .cloned()
            .ok_or_else(|| Error::UnknownSeat(slot.seat.clone()))?;
        let bytes = match (&slot.state, &slot.command) {
            (SlotState::Releasing, Some(command)) => command.clone(),
            _ => {
                let (prompt, title, briefed) = self.briefing(index, entry, &seat);
                let goal = &self.state.goals[index];
                let command = Command {
                    schema: COMMAND_SCHEMA.into(),
                    command_id: format!("studio-{}", slot.task_id),
                    task_id: slot.task_id.clone(),
                    expected_revision: None,
                    action: Action::Submit {
                        intent: TaskIntent {
                            title,
                            prompt,
                            workspace: Workspace {
                                path: goal.repository.path.clone(),
                                source_revision: None,
                            },
                            configuration: RequestedConfiguration {
                                adapter: super::adapter::NAME.into(),
                                model: Some(seat.route.model.clone()),
                            },
                            images: Vec::new(),
                        },
                    },
                };
                let bytes = serde_json::to_string(&command)
                    .map_err(|_| Error::Corrupt("a studio command could not be encoded"))?;
                for sequence in briefed {
                    if let Some(message) = self
                        .state
                        .messages
                        .iter_mut()
                        .find(|message| message.sequence == sequence)
                    {
                        message.delivery = Delivery::Briefed {
                            task_id: slot.task_id.clone(),
                        };
                    }
                }
                let target = self.slot_mut(index, entry);
                target.state = SlotState::Releasing;
                target.command = Some(bytes.clone());
                self.save()?;
                bytes
            }
        };
        #[cfg(test)]
        self.crash(Fault::BeforeApply)?;
        tasks.apply(bytes.as_bytes())?;
        #[cfg(test)]
        self.crash(Fault::AfterApply)?;
        let workspace = self.state.goals[index].repository.label.clone();
        if let Some(root) = &self.host_root {
            autostart::note_eligible(
                root,
                now,
                &slot.task_id,
                DEVICE,
                &workspace,
                1,
                Some(seat.route.provider),
            )
            .map_err(|message| Error::Tasks(super::Error::Io(std::io::Error::other(message))))?;
        }
        let target = self.slot_mut(index, entry);
        target.state = SlotState::Submitted;
        target.command = None;
        self.save()?;
        Ok(Released {
            goal_id: self.state.goals[index].goal_id.clone(),
            task_id: slot.task_id,
            seat: seat.name,
            workspace,
            provider: seat.route.provider,
        })
    }

    #[cfg(test)]
    pub(crate) fn fail_at(&mut self, fault: Fault) {
        self.fault = Some(fault);
    }

    #[cfg(test)]
    fn crash(&mut self, point: Fault) -> Result<(), Error> {
        if self.fault == Some(point) {
            self.fault = None;
            return Err(Error::Corrupt("injected studio crash"));
        }
        Ok(())
    }

    fn slot(&self, index: usize, entry: Option<usize>) -> &Slot {
        let goal = &self.state.goals[index];
        entry.map_or(&goal.lead, |entry| &goal.plan[entry].slot)
    }

    fn slot_mut(&mut self, index: usize, entry: Option<usize>) -> &mut Slot {
        let goal = &mut self.state.goals[index];
        match entry {
            Some(entry) => &mut goal.plan[entry].slot,
            None => &mut goal.lead,
        }
    }

    /// The prompt and title of goal `index`'s lead (`entry` `None`) or
    /// plan entry, and the waiting messages it carries. The prompt stays
    /// within the inbox's 32 KiB.
    fn briefing(
        &self,
        index: usize,
        entry: Option<usize>,
        seat: &Seat,
    ) -> (String, String, Vec<u64>) {
        let goal = &self.state.goals[index];
        let mut prompt = String::new();
        let title = match entry {
            None => {
                prompt.push_str(&format!(
                    "You lead an Agent Studio team as seat `{}`. Read the repository and plan \
                     the goal below as tasks for the team. Do not change files.\n\n\
                     Goal: {}\n\nSeats you can assign:\n",
                    seat.name, goal.text
                ));
                for other in &self.state.seats {
                    let role = match other.role {
                        Role::Lead => "lead",
                        Role::Worker => "worker",
                    };
                    prompt.push_str(&format!("- {} ({role}, {})\n", other.name, other.route));
                }
                format!("Plan: {}", one_line(&goal.text, 200))
            }
            Some(entry) => {
                let item = &goal.plan[entry];
                prompt.push_str(&format!(
                    "You are seat `{}` on an Agent Studio team. Do this task in this \
                     checkout and commit your change. Do not push or land it.\n\n\
                     Task: {}\n",
                    seat.name, item.title
                ));
                if !item.description.is_empty() {
                    prompt.push_str(&format!("{}\n", item.description));
                }
                prompt.push_str(&format!("\nIt is part of the goal: {}\n", goal.text));
                if !item.depends_on.is_empty() {
                    prompt.push_str("\nDone before it:\n");
                    for dependency in &item.depends_on {
                        if let Some(other) = goal.plan.iter().find(|other| &other.id == dependency)
                        {
                            prompt.push_str(&format!("- {}: {}\n", other.id, other.title));
                        }
                    }
                }
                one_line(&item.title, 256)
            }
        };
        let memory: Vec<String> = self
            .state
            .memory
            .iter()
            .filter(|item| item.goal_id.as_deref().is_none_or(|id| id == goal.goal_id))
            .map(|item| {
                let kind = match item.kind {
                    MemoryKind::Plan => "plan",
                    MemoryKind::Decision => "decision",
                    MemoryKind::Convention => "convention",
                    MemoryKind::Note => "note",
                };
                format!("- ({kind}, from {}) {}\n", item.author, item.text)
            })
            .collect();
        let memory = newest_within(&memory, MEMORY_BUDGET);
        if !memory.is_empty() {
            prompt.push_str("\nShared memory:\n");
            prompt.push_str(&memory.concat());
        }
        let waiting: Vec<&Message> = self
            .state
            .messages
            .iter()
            .filter(|message| {
                message.delivery == Delivery::Waiting
                    && message.to
                        == Party::Seat {
                            name: seat.name.clone(),
                        }
            })
            .collect();
        let lines: Vec<String> = waiting
            .iter()
            .map(|message| format!("- From {}: {}\n", message.from, message.text))
            .collect();
        let kept = newest_within(&lines, MESSAGE_BUDGET);
        let briefed = waiting[waiting.len() - kept.len()..]
            .iter()
            .map(|message| message.sequence)
            .collect();
        if !kept.is_empty() {
            prompt.push_str("\nMessages for you:\n");
            prompt.push_str(&kept.concat());
        }
        if entry.is_none() {
            prompt.push_str(&format!(
                "\nEnd your reply with the plan as one fenced JSON block:\n```json\n\
                 {{\"schema\":\"{PLAN_SCHEMA}\",\"tasks\":[{{\"id\":\"first-step\",\
                 \"title\":\"One line\",\"description\":\"What to do and how to check it\",\
                 \"depends_on\":[],\"seat\":\"a-worker\"}}]}}\n```\n\
                 Use at most {MAX_PLAN_TASKS} tasks. An id is lowercase letters, digits, and \
                 hyphens. depends_on names the ids that must be done first, with no cycles. \
                 seat names a seat above, or leave it out to let the studio choose a worker.\n"
            ));
        }
        (prompt, title, briefed)
    }

    /// Send `text` from `from` to `to`. A message to a seat whose task is
    /// queued or running is left through the steer path when its engine
    /// reads steering mid-turn; otherwise it waits for the seat's next
    /// briefing. A message to the person is recorded. A message to
    /// everyone is one message per seat. Returns the messages written.
    ///
    /// # Errors
    /// The text is empty, too long, or holds control characters, a seat
    /// is unknown, or the steer file cannot be written.
    pub fn message(
        &mut self,
        tasks: &dyn Inbox,
        from: Party,
        to: Party,
        text: &str,
        now: u64,
    ) -> Result<Vec<Message>, Error> {
        let text = text.trim();
        if !super::text(text, MAX_MESSAGE_BYTES, true) {
            return Err(Error::Invalid(format!(
                "a message is 1 to {MAX_MESSAGE_BYTES} bytes of text"
            )));
        }
        if let Party::Seat { name } = &from
            && self.state.seat(name).is_none()
        {
            return Err(Error::UnknownSeat(name.clone()));
        }
        let recipients: Vec<Party> = match &to {
            Party::Everyone => self
                .state
                .seats
                .iter()
                .filter(|seat| {
                    from != Party::Seat {
                        name: seat.name.clone(),
                    }
                })
                .map(|seat| Party::Seat {
                    name: seat.name.clone(),
                })
                .collect(),
            Party::Seat { name } if self.state.seat(name).is_none() => {
                return Err(Error::UnknownSeat(name.clone()));
            }
            other => vec![other.clone()],
        };
        if recipients.is_empty() {
            return Err(Error::State("the studio has no seat to message".into()));
        }
        let mut written = Vec::new();
        for (offset, recipient) in recipients.into_iter().enumerate() {
            let delivery = match &recipient {
                Party::Seat { name } => self.deliver(tasks, name, text)?,
                _ => Delivery::Recorded,
            };
            written.push(Message {
                sequence: self.state.sequence + 1 + offset as u64,
                at: now,
                from: from.clone(),
                to: recipient,
                text: text.to_owned(),
                delivery,
            });
        }
        for message in &written {
            if self.state.messages.len() >= MAX_MESSAGES {
                // The oldest message no briefing still waits to carry.
                let oldest = self
                    .state
                    .messages
                    .iter()
                    .position(|message| message.delivery != Delivery::Waiting)
                    .unwrap_or(0);
                self.state.messages.remove(oldest);
            }
            self.state.messages.push(message.clone());
        }
        // One save for the whole fan-out; its sequence is the last
        // message's.
        self.state.sequence += written.len().saturating_sub(1) as u64;
        self.save()?;
        Ok(written)
    }

    fn deliver(&self, tasks: &dyn Inbox, seat: &str, text: &str) -> Result<Delivery, Error> {
        let reads_steering = self.state.seat(seat).is_some_and(|seat| {
            !matches!(
                seat.route.provider,
                Provider::Grok | Provider::OpenCode | Provider::Devin
            )
        });
        let active = self.active_task(tasks, seat);
        match active {
            Some((task_id, Progress::Queued | Progress::Running)) if reads_steering => {
                super::steer::add(&self.store, &task_id, text).map_err(Error::Invalid)?;
                Ok(Delivery::Steered { task_id })
            }
            _ => Ok(Delivery::Waiting),
        }
    }

    /// The seat's newest active task and its progress.
    fn active_task(&self, tasks: &dyn Inbox, seat: &str) -> Option<(String, Progress)> {
        self.state
            .goals
            .iter()
            .rev()
            .flat_map(|goal| {
                goal.plan
                    .iter()
                    .rev()
                    .map(|entry| &entry.slot)
                    .chain(std::iter::once(&goal.lead))
            })
            .filter(|slot| slot.seat == seat && slot.state == SlotState::Submitted)
            .map(|slot| (slot.task_id.clone(), progress(tasks, &slot.task_id)))
            .find(|(_, progress)| progress.is_active())
    }

    /// The coordinator joined with the inbox's task states.
    #[must_use]
    pub fn view(&self, tasks: &dyn Inbox) -> View {
        let seats = self
            .state
            .seats
            .iter()
            .map(|seat| {
                let active = self.active_task(tasks, &seat.name);
                SeatView {
                    seat: seat.clone(),
                    task_id: active.as_ref().map(|(id, _)| id.clone()),
                    progress: active.map(|(_, progress)| progress),
                }
            })
            .collect();
        let goals = self
            .state
            .goals
            .iter()
            .map(|goal| goal_view(goal, tasks))
            .collect();
        View {
            sequence: self.state.sequence,
            seats,
            goals,
            memory: self.state.memory.clone(),
            messages: self.state.messages.clone(),
        }
    }
}

fn slot_progress(slot: &Slot, tasks: &dyn Inbox) -> Progress {
    if slot.state == SlotState::Submitted {
        progress(tasks, &slot.task_id)
    } else {
        Progress::Held
    }
}

fn goal_view(goal: &Goal, tasks: &dyn Inbox) -> GoalView {
    let states: BTreeMap<&str, Progress> = goal
        .plan
        .iter()
        .map(|entry| (entry.id.as_str(), slot_progress(&entry.slot, tasks)))
        .collect();
    let entries: Vec<EntryView> = goal
        .plan
        .iter()
        .map(|entry| {
            let mut progress = states[entry.id.as_str()];
            if progress == Progress::Held
                && entry.depends_on.iter().any(|dependency| {
                    states.get(dependency.as_str()).is_some_and(|state| {
                        (state.is_final() && *state != Progress::Done)
                            || *state == Progress::Blocked
                    })
                })
            {
                progress = Progress::Blocked;
            }
            EntryView {
                id: entry.id.clone(),
                title: entry.title.clone(),
                seat: entry.slot.seat.clone(),
                depends_on: entry.depends_on.clone(),
                task_id: entry.slot.task_id.clone(),
                progress,
            }
        })
        .collect();
    let final_tasks = entries
        .iter()
        .filter(|entry| entry.progress.is_final())
        .count();
    let total_tasks = entries.len();
    let status = if goal.decision.is_some() {
        GoalStatus::Decision
    } else if !goal.planned {
        GoalStatus::Planning
    } else if final_tasks == total_tasks {
        GoalStatus::Done
    } else {
        GoalStatus::Running
    };
    GoalView {
        goal_id: goal.goal_id.clone(),
        text: goal.text.clone(),
        workspace: goal.repository.label.clone(),
        lead_seat: goal.lead.seat.clone(),
        lead_task_id: goal.lead.task_id.clone(),
        lead_progress: slot_progress(&goal.lead, tasks),
        status,
        final_tasks,
        total_tasks,
        decision: goal.decision.clone(),
        entries,
    }
}

fn progress(tasks: &dyn Inbox, task_id: &str) -> Progress {
    tasks
        .task(task_id)
        .map_or(Progress::Missing, |task| progress_of(&task))
}

/// The newest of `lines` (oldest first) that fit in `budget` bytes,
/// oldest first.
fn newest_within(lines: &[String], budget: usize) -> Vec<String> {
    let mut used = 0;
    let mut kept: Vec<String> = lines
        .iter()
        .rev()
        .take_while(|line| {
            used += line.len();
            used <= budget
        })
        .cloned()
        .collect();
    kept.reverse();
    kept
}

/// At most `max` bytes of `text`, cut at a character boundary.
fn cut(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// `text`'s first line, at most `max` bytes.
fn one_line(text: &str, max: usize) -> String {
    cut(text.lines().next().unwrap_or("").trim(), max)
}

/// The host's pass over the studio of the task store at `store`, beside
/// the auto-start sweep: reconcile it, reading a finished lead's reply
/// from its recorded run, and note each released task eligible in the
/// journal of the host root `root`. A store without a studio is left
/// alone. Failures go to standard error; the next pass retries.
pub fn sweep(store: &Path, root: &Path, now: u64) {
    if !Studio::present(store) {
        return;
    }
    let result = (|| -> Result<Vec<Released>, Error> {
        let mut tasks = Store::open(store)?;
        let mut studio = Studio::open(store)?.with_host_root(root);
        let reply = |task: &str| super::local::result_in(Some(store), task).map(|run| run.summary);
        studio.reconcile(&mut tasks, now, &reply)
    })();
    if let Err(error) = result {
        eprintln!("openagents host: studio: {error}");
    }
}

#[cfg(test)]
#[path = "studio_tests.rs"]
mod tests;
