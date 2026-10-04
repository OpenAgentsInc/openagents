//! Agent Studio over NIP-HOST: the intents a view sends, the snapshot and
//! sequenced updates the host answers, and the mirror a client keeps
//! (`docs/verse/agent-studio.md`, "The client is a view").
//!
//! The host's studio coordinator is the source of truth; a view holds no
//! business logic. It sends intents (`studio.goal.submit`, `studio.seat.*`,
//! `studio.task.*`, `studio.decision.answer`, `studio.decision.always`,
//! `studio.review.open`, and `studio.merge.decide`), each one NIP-HOST
//! operation with exactly one
//! required right, and it draws what the host answers:
//!
//! - `studio.snapshot` answers a full [`Snapshot`]: goals, seats, tasks,
//!   decisions, repository summaries, a bounded log tail per seat, and the
//!   newest shared memory entries with the current plan pinned.
//! - `studio.update` answers an [`Update`] from the sequence the client
//!   holds to the host's current one: the items that changed and the ones
//!   that went away. A host that no longer holds that sequence, or a
//!   different stream (the host started again), refuses as `stale`, and
//!   the client reads a fresh snapshot. [`Mirror`] does this bookkeeping.
//!
//! Every list in a [`View`] is in key order with distinct keys, so applying
//! an update to the view it was computed from gives exactly the newer view.
//! A view and an update each encode in at most [`MAX_VIEW_BYTES`], inside
//! one relay frame: the host drops the oldest log lines, then the oldest
//! finished goals, before it answers ([`View::fit`]).
//!
//! Log lines are display text the host derives from ATIF steps under its
//! disclosure policy: an activity and a tool's name, or the first line of
//! what the agent said. They never carry a call's arguments or output.
//!
//! Each message to a seat carries its delivery under NIP-SESS steering
//! ([`SeatMessage`]): the native mode it reached the engine by, and
//! whether the engine consumed it, because an accepted steer is not a
//! consumed one.
//!
//! A merge decision binds to the review's three revisions (base, `HEAD`
//! commit, and content tree). The host reads the review again and refuses
//! a decision whose revisions differ as `stale`, so the client reloads the
//! review rather than landing a change nobody read.
//!
//! An approval whose engine named its step carries an [`Approval`]: the
//! tool, the exact command, the working directory, the reason, and the
//! host's [`Risk`] for it. When the host would keep a standing rule for
//! the step, the approval also carries that rule's exact text, and
//! `studio.decision.always` echoes the text back: the host records the rule
//! only when the text still matches the step it holds, so a rule is never
//! wider than the one the person saw. The host applies a standing rule;
//! a client never does.
use crate::protocol::{Operation, Outcome};
use crate::review::{Publication, revision};
use crate::{Code, Error, Result, fail};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// The most seats a view carries.
pub const MAX_SEATS: usize = 16;
/// The most goals a view carries.
pub const MAX_GOALS: usize = 64;
/// The most tasks a view carries, leads included.
pub const MAX_TASKS: usize = 256;
/// The most open decisions a view carries.
pub const MAX_DECISIONS: usize = 64;
/// The most repository summaries a view carries.
pub const MAX_REPOSITORIES: usize = 64;
/// The most dependencies a task row names.
pub const MAX_DEPENDENCIES: usize = 16;
/// The most lines in one seat's log tail.
pub const MAX_LOG_LINES: usize = 8;
/// The longest log line.
pub const MAX_LINE: usize = 160;
/// The longest goal text a view carries; the host keeps the whole goal.
pub const MAX_GOAL_TEXT: usize = 512;
/// The longest task title a view carries.
pub const MAX_TITLE: usize = 200;
/// The longest decision text a view carries.
pub const MAX_DECISION_TEXT: usize = 1024;
/// The longest goal a `studio.goal.submit` carries.
pub const MAX_SUBMIT: usize = 4 * 1024;
/// The longest message a `studio.seat.message` carries.
pub const MAX_MESSAGE: usize = 4 * 1024;
/// The longest tool name an [`Approval`] carries.
pub const MAX_STEP_TOOL: usize = 64;
/// The longest command an [`Approval`] carries.
pub const MAX_STEP_COMMAND: usize = 512;
/// The longest working directory an [`Approval`] carries.
pub const MAX_STEP_CWD: usize = 512;
/// The longest reason an [`Approval`] carries.
pub const MAX_STEP_REASON: usize = 512;
/// The longest standing rule text an [`Approval`] or a
/// `studio.decision.always` carries.
pub const MAX_RULE_TEXT: usize = 1536;
/// The longest answer a `studio.decision.answer` carries: a question's
/// answer, or a plan for a goal's plan decision.
pub const MAX_ANSWER: usize = 64 * 1024;
/// The longest text a **Request changes** or **Reject** carries.
pub const MAX_REVIEW_TEXT: usize = 16 * 1024;
/// The largest encoded view or update, inside one relay frame.
pub const MAX_VIEW_BYTES: usize = 48 * 1024;
/// How many earlier views a host keeps to compute updates from.
pub const MAX_HISTORY: usize = 64;
/// The most shared memory entries a view carries: the newest, and the
/// pinned plan.
pub const MAX_MEMORY: usize = 16;
/// The longest memory entry text a view carries.
pub const MAX_MEMORY_TEXT: usize = 1024;
/// The longest memory entry author a view carries.
pub const MAX_AUTHOR: usize = 64;
/// The most messages to seats a view carries, the newest.
pub const MAX_MESSAGES: usize = 32;

/// What model calls cost, summed from each ended turn's recorded cost:
/// the providers' reported cost, or tokens at list price where Coder
/// prices them. Information only, never a limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spend {
    /// Micro-dollars of every known part.
    pub microusd: u64,
    /// Ended turns whose whole cost is not known; [`Spend::microusd`]
    /// holds only their known parts.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub unpriced: u32,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

impl Spend {
    /// Nothing spent and nothing unpriced.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        *self == Self::default()
    }

    /// This spend and `other` together.
    #[must_use]
    pub fn plus(self, other: Self) -> Self {
        Self {
            microusd: self.microusd.saturating_add(other.microusd),
            unpriced: self.unpriced.saturating_add(other.unpriced),
        }
    }

    /// The amount in dollars to the cent (`$1.25`), `<$0.01` for a smaller
    /// amount above zero, and a trailing `+` when a turn's cost is not
    /// wholly known.
    #[must_use]
    pub fn label(&self) -> String {
        let amount = if self.microusd > 0 && self.microusd < 5_000 {
            "<$0.01".to_owned()
        } else {
            let cents = self.microusd.saturating_add(5_000) / 10_000;
            format!("${}.{:02}", cents / 100, cents % 100)
        };
        if self.unpriced > 0 {
            format!("{amount}+")
        } else {
            amount
        }
    }
}

/// A goal's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    /// The lead is planning.
    Planning,
    /// An open decision waits on a person.
    Decision,
    /// The plan's tasks are under way.
    Running,
    /// Every plan task is over.
    Done,
}

/// One goal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    /// The goal's identity, its key.
    pub goal: String,
    /// The goal as submitted, at most [`MAX_GOAL_TEXT`] bytes of it.
    pub text: String,
    /// The repository's workspace label; never a path.
    pub workspace: String,
    /// The lead seat.
    pub lead: String,
    pub status: GoalStatus,
    /// Plan tasks that are over, of `total_tasks`.
    pub final_tasks: u32,
    pub total_tasks: u32,
    /// When it was submitted, in Unix seconds.
    pub submitted_at: u64,
    /// What its tasks spent, earlier attempts included.
    #[serde(default, skip_serializing_if = "Spend::is_zero")]
    pub spend: Spend,
}

/// A seat's part in the team.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Lead,
    Worker,
}

/// What a seat is doing, from its task and its trace's newest step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    /// No task, or a task waiting in the queue.
    Idle,
    Reading,
    Editing,
    Running,
    Testing,
    Judging,
    Thinking,
    /// Waiting on a person's answer or approval.
    Waiting,
    /// Refused: a missing grant, or no provider with capacity.
    Blocked,
    /// A person paused or stopped the seat.
    Paused,
    Done,
    Failed,
}

impl Activity {
    /// The station the activity happens at, when nothing about the step
    /// says otherwise.
    #[must_use]
    pub fn station(self) -> Station {
        match self {
            Activity::Reading => Station::Library,
            Activity::Idle | Activity::Editing | Activity::Thinking => Station::Desk,
            Activity::Running => Station::Workbench,
            Activity::Testing => Station::ProvingGround,
            Activity::Judging => Station::Oracle,
            Activity::Waiting => Station::Podium,
            Activity::Blocked | Activity::Paused => Station::Lounge,
            Activity::Done | Activity::Failed => Station::TaskWall,
        }
    }
}

/// Where in the studio a seat is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Station {
    Desk,
    Library,
    Workbench,
    ProvingGround,
    Oracle,
    Podium,
    Lounge,
    TaskWall,
}

/// One seat and what it is doing now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seat {
    /// The seat's name, its key.
    pub seat: String,
    pub role: Role,
    /// The auto-start route its tasks ask for, as `PROVIDER:MODEL`.
    pub route: String,
    /// The character look a view draws it with.
    pub look: String,
    pub desk: u32,
    pub activity: Activity,
    pub station: Station,
    /// Its active task, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// A person paused it: it takes no new task until resumed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paused: bool,
    /// What its tasks spent, across every goal.
    #[serde(default, skip_serializing_if = "Spend::is_zero")]
    pub spend: Spend,
}

/// Where a task is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Planned, waiting on its dependencies; not in the inbox yet.
    Held,
    /// A dependency failed or was cancelled, so it cannot start.
    Blocked,
    Queued,
    Running,
    /// Its turn ended with a question or an approval for a person.
    Waiting,
    Done,
    Failed,
    Cancelled,
    /// The inbox does not hold the task the coordinator recorded.
    Missing,
}

impl TaskStatus {
    /// The task will not change without a person.
    #[must_use]
    pub fn is_final(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Failed | Self::Cancelled | Self::Missing
        )
    }
}

/// One task: a goal's lead or one of its plan's entries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// The task's identity in the host's inbox, its key.
    pub task: String,
    pub goal: String,
    /// The plan entry's identity, or `lead`.
    pub entry: String,
    /// Its place on the goal's board: the lead is 0, plan entries follow.
    pub position: u32,
    pub title: String,
    /// The seat assigned to it.
    pub seat: String,
    /// The plan entries it waits on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    pub status: TaskStatus,
    /// What this task spent, across its turns.
    #[serde(default, skip_serializing_if = "Spend::is_zero")]
    pub spend: Spend,
}

/// Why a decision waits on a person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    /// A task's engine asked a question.
    Question,
    /// A task's engine asked to approve a step.
    Approval,
    /// The lead's plan failed validation; an answer is a corrected plan.
    InvalidPlan,
    /// The lead finished without a plan; an answer is a plan.
    NoPlan,
    /// The lead's task failed; an answer is a plan, or retry the lead.
    LeadFailed,
    /// A plan entry's dependency did not finish; retry or cancel it.
    DependencyFailed,
}

/// One open decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// Its identity, its key: the waiting task's, or the goal's.
    pub decision: String,
    pub goal: String,
    /// The task that asks, for a question or an approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The seat that asks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seat: Option<String>,
    pub kind: DecisionKind,
    /// The question, or the reasons, at most [`MAX_DECISION_TEXT`] bytes.
    pub text: String,
    /// What `studio.decision.answer` names as `based_on`: the waiting
    /// task's revision, or the goal decision's sequence.
    pub based_on: u64,
    /// The step an approval asks to take, when its engine named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<Approval>,
}

/// How much a step can harm, as the host classifies it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Reads, or changes inside the task's own worktree.
    Low,
    /// Reaches the network, installs or downloads code, or changes shared
    /// repository state such as branches and tags.
    Medium,
    /// Destroys or rewrites work, publishes, pushes, touches credentials,
    /// or raises privileges. A high-risk step is approved once at a time.
    High,
}

impl Risk {
    /// The chip's words: "Low risk", "Medium risk", or "High risk".
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low risk",
            Self::Medium => "Medium risk",
            Self::High => "High risk",
        }
    }
}

/// The step an approval asks to take.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    /// The tool the step uses, such as `shell`.
    pub tool: String,
    /// The exact command, at most [`MAX_STEP_COMMAND`] bytes.
    pub command: String,
    /// The absolute working directory the command runs in.
    pub cwd: String,
    /// Why the engine asks, possibly empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    pub risk: Risk,
    /// The exact standing rule **Always allow for this seat** records,
    /// or `None` when the host keeps no standing rule for this step, as
    /// for a high-risk one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always: Option<String>,
}

impl Approval {
    /// Check the step's bounds.
    ///
    /// # Errors
    /// `bounds` or `malformed` for a field outside its bound.
    pub fn validate(&self) -> Result<()> {
        line(&self.tool, MAX_STEP_TOOL)?;
        text(&self.command, MAX_STEP_COMMAND)?;
        line(&self.cwd, MAX_STEP_CWD)?;
        if !self.cwd.starts_with('/') {
            return fail(Code::Malformed, "an approval's directory is absolute");
        }
        optional_text(&self.reason, MAX_STEP_REASON)?;
        if let Some(always) = &self.always {
            text(always, MAX_RULE_TEXT)?;
            if self.risk == Risk::High {
                return fail(Code::Malformed, "a high-risk step has no standing rule");
            }
        }
        Ok(())
    }
}

/// One admitted repository's summary. Its root stays on the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    /// The workspace label, its key.
    pub workspace: String,
    pub goals: u32,
    /// Tasks queued, running, or waiting.
    pub open_tasks: u32,
}

/// One line of a seat's log tail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogLine {
    /// When the step happened, in milliseconds since the epoch.
    pub at: u64,
    pub activity: Activity,
    /// Display text, at most [`MAX_LINE`] bytes on one line.
    pub text: String,
}

/// A seat's log tail, oldest line first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Log {
    /// The seat, its key.
    pub seat: String,
    /// The task the lines come from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    pub lines: Vec<LogLine>,
}

/// What a shared memory entry records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// A goal's accepted plan.
    Plan,
    /// A decision the team or the person made.
    Decision,
    /// A repository convention.
    Convention,
    Note,
}

/// One shared memory entry, which every briefing carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    /// Its key: [`memory_key`] of the entry's sequence, so key order is
    /// the order the entries were written.
    pub entry: String,
    pub kind: MemoryKind,
    /// Who wrote it, as display text such as `@ada` or `the person`.
    pub author: String,
    /// The goal it belongs to; absent for the whole studio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// Its text, at most [`MAX_MEMORY_TEXT`] bytes.
    pub text: String,
    /// The current plan, which the library shows first. At most one entry
    /// is pinned, and only a plan.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
}

/// The key of the memory entry with `sequence`: `m` and twelve digits, so
/// key order is sequence order.
#[must_use]
pub fn memory_key(sequence: u64) -> String {
    format!("m{sequence:012}")
}

/// How a message reached, or will reach, its seat's engine: the native
/// mode of NIP-SESS steering the delivery used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryMode {
    /// Left for the seat's running turn, which reads it between steps.
    MidTurn,
    /// Carried by the briefing of the seat's next task, when it starts.
    TurnBoundary,
}

/// Whether a message's engine has it. An accepted steer is not a consumed
/// one (NIP-SESS, "Steering capability").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    /// Held for the seat's next task.
    Waiting,
    /// Accepted for the engine, which has not read it yet.
    Accepted,
    /// The engine read it: the running turn took it, or the task whose
    /// briefing carries it started.
    Consumed,
}

/// One message to a seat and its delivery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatMessage {
    /// Its key: the coordinator sequence that sent it, as 20 digits, so
    /// key order is sending order.
    pub message: String,
    /// The seat it is for.
    pub seat: String,
    /// The seat that sent it; none for the person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// When it was sent, in Unix seconds.
    pub at: u64,
    /// Its first line, at most [`MAX_LINE`] bytes.
    pub text: String,
    pub mode: DeliveryMode,
    pub state: DeliveryState,
    /// The task it was left for or briefed into; none while it waits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
}

impl SeatMessage {
    /// Who sent it, as a panel names them: `You` or the seat.
    #[must_use]
    pub fn sender(&self) -> &str {
        self.from.as_deref().unwrap_or("You")
    }

    /// What a panel shows of its delivery, such as `read mid-turn`.
    #[must_use]
    pub fn delivery(&self) -> &'static str {
        match (self.mode, self.state) {
            (_, DeliveryState::Waiting) => "waits for its next task",
            (DeliveryMode::MidTurn, DeliveryState::Accepted) => "sent mid-turn, not read yet",
            (DeliveryMode::MidTurn, DeliveryState::Consumed) => "read mid-turn",
            (DeliveryMode::TurnBoundary, DeliveryState::Accepted) => {
                "in its next task's briefing, not started yet"
            }
            (DeliveryMode::TurnBoundary, DeliveryState::Consumed) => "read when its task started",
        }
    }
}

/// The key of the message the coordinator sent at `sequence`.
#[must_use]
pub fn message_key(sequence: u64) -> String {
    format!("{sequence:020}")
}

/// The studio as a view draws it. Every list is in key order with
/// distinct keys. In an [`Update`], it holds only what changed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub goals: Vec<Goal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seats: Vec<Seat>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Task>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<Decision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repositories: Vec<Repository>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<Log>,
    /// The newest shared memory entries and the pinned plan. A host that
    /// predates shared memory in the view sends none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory: Vec<Memory>,
    /// The newest messages to seats, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<SeatMessage>,
}

/// The kind of item an update removes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Goal,
    Seat,
    Task,
    Decision,
    Repository,
    Log,
    Memory,
    Message,
}

/// An item an update removes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Removed {
    pub kind: Kind,
    pub id: String,
}

/// A full view at one point of the host's stream (`studio.snapshot`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    /// The stream: one per host process. A sequence means nothing in
    /// another stream.
    pub stream: String,
    /// The stream's sequence this view is at. It only grows.
    pub sequence: u64,
    pub view: View,
}

/// What changed from sequence `from` to `sequence` (`studio.update`).
/// `from` equal to `sequence` means nothing changed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    pub stream: String,
    pub from: u64,
    pub sequence: u64,
    /// Items added or changed, whole.
    #[serde(default, skip_serializing_if = "View::is_empty")]
    pub put: View,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<Removed>,
}

/// A reviewer's choice for a task's reviewed change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Hand the reviewed change to the host's landing path.
    Merge,
    /// Send the reviewer's text back to the same seat as a follow-up turn.
    RequestChanges,
    /// Close the task; its worktree stays for inspection until archive.
    Reject,
}

/// A merge decision at the reviewed revisions (`studio.merge.decide`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeDecision {
    pub task: String,
    /// The review's three revisions: the decision's identity.
    pub base: String,
    pub head_commit: String,
    pub head: String,
    pub verdict: Verdict,
    /// For **Request changes**, the changes asked for; for **Reject**, an
    /// optional reason. Empty for **Merge**.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// A 64-hex ID the device mints once, the follow-up turn's command ID
    /// for **Request changes**, so a replay never sends it twice.
    pub command: String,
    /// When the device minted the decision, in Unix seconds.
    pub issued_at: u64,
}

/// What the host did with a merge decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Merged {
    pub task: String,
    pub base: String,
    pub head_commit: String,
    pub head: String,
    pub verdict: Verdict,
    /// For **Merge**, the landing path's publication, which may itself be
    /// refused or uncertain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<Publication>,
}

/// A studio identity: a goal, a task, or a decision. 1 to 128 ASCII
/// letters, digits, dots, hyphens, and underscores, starting with a letter
/// or digit. A host-issued 64-hex task ID is one too.
///
/// # Errors
/// `malformed` for anything else.
pub fn id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return fail(Code::Malformed, "not a studio identity");
    }
    Ok(())
}

/// A seat name: 1 to 32 lowercase letters, digits, and hyphens, starting
/// with a letter or digit.
///
/// # Errors
/// `malformed` for anything else.
pub fn seat_name(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 32
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return fail(Code::Malformed, "not a seat name");
    }
    Ok(())
}

/// A stream identity: 1 to 64 lowercase hex characters.
///
/// # Errors
/// `malformed` for anything else.
pub fn stream_id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return fail(Code::Malformed, "not a studio stream");
    }
    Ok(())
}

/// One line of display text: 1 to `max` bytes, no control characters.
fn line(value: &str, max: usize) -> Result<()> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return fail(Code::Bounds, "studio text exceeds its bound");
    }
    Ok(())
}

/// Text of several lines: not blank, at most `max` bytes, no control
/// characters but line breaks and tabs.
///
/// # Errors
/// `bounds` for anything else.
pub fn text(value: &str, max: usize) -> Result<()> {
    if value.trim().is_empty()
        || value.len() > max
        || value
            .chars()
            .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
    {
        return fail(Code::Bounds, "studio text exceeds its bound");
    }
    Ok(())
}

/// Text of several lines that may be empty.
fn optional_text(value: &str, max: usize) -> Result<()> {
    if value.is_empty() {
        Ok(())
    } else {
        text(value, max)
    }
}

fn count(value: usize, max: usize) -> Result<()> {
    if value > max {
        return fail(Code::Bounds, "studio list exceeds its bound");
    }
    Ok(())
}

/// At most `max` bytes of `text`, cut at a character boundary.
#[must_use]
pub fn cut(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// `text`'s first nonblank line, trimmed, at most `max` bytes, with any
/// control character dropped: what a log line or a title shows.
#[must_use]
pub fn first_line(text: &str, max: usize) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    let clean: String = line.chars().filter(|ch| !ch.is_control()).collect();
    cut(&clean, max)
}

/// An item of a [`View`] list, keyed by its identity.
trait Keyed: Clone + PartialEq {
    const KIND: Kind;
    fn key(&self) -> &str;
    fn validate(&self) -> Result<()>;
}

impl Keyed for Goal {
    const KIND: Kind = Kind::Goal;
    fn key(&self) -> &str {
        &self.goal
    }
    fn validate(&self) -> Result<()> {
        id(&self.goal)?;
        text(&self.text, MAX_GOAL_TEXT)?;
        line(&self.workspace, 128)?;
        seat_name(&self.lead)?;
        if self.final_tasks > self.total_tasks {
            return fail(Code::Malformed, "a goal has more final tasks than tasks");
        }
        Ok(())
    }
}

impl Keyed for Seat {
    const KIND: Kind = Kind::Seat;
    fn key(&self) -> &str {
        &self.seat
    }
    fn validate(&self) -> Result<()> {
        seat_name(&self.seat)?;
        line(&self.route, 160)?;
        line(&self.look, 64)?;
        if let Some(task) = &self.task {
            id(task)?;
        }
        Ok(())
    }
}

impl Keyed for Task {
    const KIND: Kind = Kind::Task;
    fn key(&self) -> &str {
        &self.task
    }
    fn validate(&self) -> Result<()> {
        id(&self.task)?;
        id(&self.goal)?;
        id(&self.entry)?;
        line(&self.title, MAX_TITLE)?;
        seat_name(&self.seat)?;
        count(self.depends_on.len(), MAX_DEPENDENCIES)?;
        for dependency in &self.depends_on {
            id(dependency)?;
        }
        Ok(())
    }
}

impl Keyed for Decision {
    const KIND: Kind = Kind::Decision;
    fn key(&self) -> &str {
        &self.decision
    }
    fn validate(&self) -> Result<()> {
        id(&self.decision)?;
        id(&self.goal)?;
        if let Some(task) = &self.task {
            id(task)?;
        }
        if let Some(seat) = &self.seat {
            seat_name(seat)?;
        }
        let task_kind = matches!(self.kind, DecisionKind::Question | DecisionKind::Approval);
        if task_kind != self.task.is_some() {
            return fail(
                Code::Malformed,
                "a question or approval names its task, a goal decision none",
            );
        }
        if let Some(approval) = &self.approval {
            if self.kind != DecisionKind::Approval {
                return fail(Code::Malformed, "only an approval names a step");
            }
            approval.validate()?;
        }
        text(&self.text, MAX_DECISION_TEXT)
    }
}

impl Keyed for Repository {
    const KIND: Kind = Kind::Repository;
    fn key(&self) -> &str {
        &self.workspace
    }
    fn validate(&self) -> Result<()> {
        line(&self.workspace, 128)
    }
}

impl Keyed for Log {
    const KIND: Kind = Kind::Log;
    fn key(&self) -> &str {
        &self.seat
    }
    fn validate(&self) -> Result<()> {
        seat_name(&self.seat)?;
        if let Some(task) = &self.task {
            id(task)?;
        }
        count(self.lines.len(), MAX_LOG_LINES)?;
        for entry in &self.lines {
            line(&entry.text, MAX_LINE)?;
        }
        Ok(())
    }
}

impl Keyed for Memory {
    const KIND: Kind = Kind::Memory;
    fn key(&self) -> &str {
        &self.entry
    }
    fn validate(&self) -> Result<()> {
        id(&self.entry)?;
        line(&self.author, MAX_AUTHOR)?;
        if let Some(goal) = &self.goal {
            id(goal)?;
        }
        if self.pinned && self.kind != MemoryKind::Plan {
            return fail(Code::Malformed, "only a plan is pinned");
        }
        text(&self.text, MAX_MEMORY_TEXT)
    }
}

impl Keyed for SeatMessage {
    const KIND: Kind = Kind::Message;
    fn key(&self) -> &str {
        &self.message
    }
    fn validate(&self) -> Result<()> {
        message_id(&self.message)?;
        seat_name(&self.seat)?;
        if let Some(from) = &self.from {
            seat_name(from)?;
        }
        line(&self.text, MAX_LINE)?;
        if let Some(task) = &self.task {
            id(task)?;
        }
        let waits = self.state == DeliveryState::Waiting;
        if waits != self.task.is_none() || (waits && self.mode != DeliveryMode::TurnBoundary) {
            return fail(
                Code::Malformed,
                "a waiting message names no task and waits for a turn boundary",
            );
        }
        Ok(())
    }
}

/// A message key: exactly 20 ASCII digits.
fn message_id(value: &str) -> Result<()> {
    if value.len() != 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return fail(Code::Malformed, "not a studio message key");
    }
    Ok(())
}

/// Check a list's items, bound, and key order.
fn check<T: Keyed>(items: &[T], max: usize) -> Result<()> {
    count(items.len(), max)?;
    for (index, item) in items.iter().enumerate() {
        item.validate()?;
        if index > 0 && items[index - 1].key() >= item.key() {
            return fail(
                Code::Malformed,
                "studio lists must be in key order and distinct",
            );
        }
    }
    Ok(())
}

/// Sort a list by key and keep the first item of each key.
fn canonical<T: Keyed>(items: &mut Vec<T>) {
    items.sort_by(|a, b| a.key().cmp(b.key()));
    items.dedup_by(|a, b| a.key() == b.key());
}

/// What changed between two lists in key order.
fn diff<T: Keyed>(old: &[T], new: &[T], put: &mut Vec<T>, removed: &mut Vec<Removed>) {
    for item in new {
        match old.binary_search_by(|other| other.key().cmp(item.key())) {
            Ok(index) if old[index] == *item => {}
            _ => put.push(item.clone()),
        }
    }
    for item in old {
        if new
            .binary_search_by(|other| other.key().cmp(item.key()))
            .is_err()
        {
            removed.push(Removed {
                kind: T::KIND,
                id: item.key().to_owned(),
            });
        }
    }
}

/// Apply one list's part of an update, keeping key order.
fn apply<T: Keyed>(items: &mut Vec<T>, put: &[T], removed: &[Removed]) {
    items.retain(|item| {
        !removed
            .iter()
            .any(|gone| gone.kind == T::KIND && gone.id == item.key())
    });
    for item in put {
        match items.binary_search_by(|other| other.key().cmp(item.key())) {
            Ok(index) => items[index] = item.clone(),
            Err(index) => items.insert(index, item.clone()),
        }
    }
}

fn encoded_len(value: &impl Serialize) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len())
}

impl View {
    /// Whether the view holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == View::default()
    }

    /// Put every list in key order, keeping the first item of each key.
    pub fn canonicalize(&mut self) {
        canonical(&mut self.goals);
        canonical(&mut self.seats);
        canonical(&mut self.tasks);
        canonical(&mut self.decisions);
        canonical(&mut self.repositories);
        canonical(&mut self.logs);
        canonical(&mut self.memory);
        canonical(&mut self.messages);
    }

    /// Check every item's bounds, each list's bound and key order, and the
    /// encoded size.
    ///
    /// # Errors
    /// `bounds` or `malformed` for the first item or list out of bounds.
    pub fn validate(&self) -> Result<()> {
        check(&self.goals, MAX_GOALS)?;
        check(&self.seats, MAX_SEATS)?;
        check(&self.tasks, MAX_TASKS)?;
        check(&self.decisions, MAX_DECISIONS)?;
        check(&self.repositories, MAX_REPOSITORIES)?;
        check(&self.logs, MAX_SEATS)?;
        check(&self.memory, MAX_MEMORY)?;
        if self.memory.iter().filter(|entry| entry.pinned).count() > 1 {
            return fail(Code::Malformed, "a view pins at most one plan");
        }
        check(&self.messages, MAX_MESSAGES)?;
        if encoded_len(self) > MAX_VIEW_BYTES {
            return fail(Code::Bounds, "studio view exceeds its bound");
        }
        Ok(())
    }

    /// Shrink the view until it encodes in `max` bytes: drop the oldest
    /// line of the longest log tail, then the oldest memory entry that is
    /// not pinned, then the oldest message, then the oldest finished goal
    /// with its tasks and decisions. Returns whether it fits.
    pub fn fit(&mut self, max: usize) -> bool {
        while encoded_len(&*self) > max {
            if let Some(log) = self
                .logs
                .iter_mut()
                .filter(|log| !log.lines.is_empty())
                .max_by_key(|log| log.lines.len())
            {
                log.lines.remove(0);
                continue;
            }
            if let Some(index) = self.memory.iter().position(|entry| !entry.pinned) {
                self.memory.remove(index);
                continue;
            }
            if !self.messages.is_empty() {
                self.messages.remove(0);
                continue;
            }
            let oldest = self
                .goals
                .iter()
                .filter(|goal| goal.status == GoalStatus::Done)
                .min_by_key(|goal| goal.submitted_at)
                .map(|goal| goal.goal.clone());
            let Some(goal) = oldest else {
                return false;
            };
            self.goals.retain(|other| other.goal != goal);
            self.tasks.retain(|task| task.goal != goal);
            self.decisions.retain(|decision| decision.goal != goal);
        }
        true
    }

    /// What changed from `old` to `new`, both in key order.
    #[must_use]
    pub fn diff(old: &View, new: &View) -> (View, Vec<Removed>) {
        let mut put = View::default();
        let mut removed = Vec::new();
        diff(&old.goals, &new.goals, &mut put.goals, &mut removed);
        diff(&old.seats, &new.seats, &mut put.seats, &mut removed);
        diff(&old.tasks, &new.tasks, &mut put.tasks, &mut removed);
        diff(
            &old.decisions,
            &new.decisions,
            &mut put.decisions,
            &mut removed,
        );
        diff(
            &old.repositories,
            &new.repositories,
            &mut put.repositories,
            &mut removed,
        );
        diff(&old.logs, &new.logs, &mut put.logs, &mut removed);
        diff(&old.memory, &new.memory, &mut put.memory, &mut removed);
        diff(
            &old.messages,
            &new.messages,
            &mut put.messages,
            &mut removed,
        );
        (put, removed)
    }

    /// Apply what changed: remove, then put each item at its key.
    pub fn apply(&mut self, put: &View, removed: &[Removed]) {
        apply(&mut self.goals, &put.goals, removed);
        apply(&mut self.seats, &put.seats, removed);
        apply(&mut self.tasks, &put.tasks, removed);
        apply(&mut self.decisions, &put.decisions, removed);
        apply(&mut self.repositories, &put.repositories, removed);
        apply(&mut self.logs, &put.logs, removed);
        apply(&mut self.memory, &put.memory, removed);
        apply(&mut self.messages, &put.messages, removed);
    }

    /// The pinned plan, when the view carries one.
    #[must_use]
    pub fn plan(&self) -> Option<&Memory> {
        self.memory.iter().find(|entry| entry.pinned)
    }
}

impl Snapshot {
    /// Check the stream identity and the view.
    ///
    /// # Errors
    /// The first field out of bounds.
    pub fn validate(&self) -> Result<()> {
        stream_id(&self.stream)?;
        self.view.validate()
    }
}

impl Update {
    /// Check the stream identity, the order of sequences, what changed,
    /// and the encoded size.
    ///
    /// # Errors
    /// The first field out of bounds.
    pub fn validate(&self) -> Result<()> {
        stream_id(&self.stream)?;
        if self.from > self.sequence {
            return fail(Code::Malformed, "an update cannot go back");
        }
        self.put.validate()?;
        count(
            self.removed.len(),
            MAX_GOALS
                + MAX_SEATS * 2
                + MAX_TASKS
                + MAX_DECISIONS
                + MAX_REPOSITORIES
                + MAX_MEMORY
                + MAX_MESSAGES,
        )?;
        for gone in &self.removed {
            match gone.kind {
                Kind::Seat | Kind::Log => seat_name(&gone.id)?,
                Kind::Repository => line(&gone.id, 128)?,
                Kind::Message => message_id(&gone.id)?,
                _ => id(&gone.id)?,
            }
        }
        if encoded_len(self) > MAX_VIEW_BYTES + 1024 {
            return fail(Code::Bounds, "studio update exceeds its bound");
        }
        Ok(())
    }
}

impl MergeDecision {
    /// Check the task, the three revisions, the command ID, and that the
    /// text fits the verdict: required for **Request changes**, optional
    /// for **Reject**, and empty for **Merge**.
    ///
    /// # Errors
    /// The first field out of bounds.
    pub fn validate(&self) -> Result<()> {
        id(&self.task)?;
        revision(&self.base)?;
        revision(&self.head_commit)?;
        revision(&self.head)?;
        crate::protocol::identity(&self.command).map_err(Error::from)?;
        if self.issued_at > crate::protocol::MAX_SAFE {
            return fail(Code::Malformed, "integer exceeds the safe range");
        }
        match self.verdict {
            Verdict::Merge if !self.text.is_empty() => {
                fail(Code::Malformed, "a merge carries no text")
            }
            Verdict::Merge => Ok(()),
            Verdict::RequestChanges => text(&self.text, MAX_REVIEW_TEXT),
            Verdict::Reject => optional_text(&self.text, MAX_REVIEW_TEXT),
        }
    }
}

impl Merged {
    /// Check the revisions, and that a publication comes with a merge
    /// only and names the same task and revisions.
    ///
    /// # Errors
    /// The first field out of bounds.
    pub fn validate(&self) -> Result<()> {
        id(&self.task)?;
        revision(&self.base)?;
        revision(&self.head_commit)?;
        revision(&self.head)?;
        match (&self.verdict, &self.publication) {
            (Verdict::Merge, Some(publication)) => {
                publication.validate()?;
                if publication.task != self.task
                    || publication.base != self.base
                    || publication.head_commit != self.head_commit
                    || publication.head != self.head
                {
                    return fail(
                        Code::Malformed,
                        "the publication names another task or review",
                    );
                }
                Ok(())
            }
            (Verdict::Merge, None) => fail(Code::Malformed, "a merge carries its publication"),
            (_, Some(_)) => fail(Code::Malformed, "only a merge carries a publication"),
            (_, None) => Ok(()),
        }
    }

    /// Whether this answers `decision`.
    #[must_use]
    pub fn answers(&self, decision: &MergeDecision) -> bool {
        self.task == decision.task
            && self.base == decision.base
            && self.head_commit == decision.head_commit
            && self.head == decision.head
            && self.verdict == decision.verdict
    }
}

/// The host's side of the stream: the current sequence and the recent
/// views it computes updates from. One per host process; a new process is
/// a new stream, so a client's old sequence reads as stale.
#[derive(Clone, Debug)]
pub struct Stream {
    id: String,
    sequence: u64,
    history: VecDeque<(u64, View)>,
}

impl Stream {
    /// A new stream named `id` (1 to 64 lowercase hex characters).
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            sequence: 0,
            history: VecDeque::new(),
        }
    }

    /// The stream's identity.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The current sequence; zero before the first view.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Take `view` as the studio now, in key order and fitted to
    /// [`MAX_VIEW_BYTES`]. The sequence grows only when the view changed.
    fn advance(&mut self, mut view: View) {
        view.canonicalize();
        view.fit(MAX_VIEW_BYTES);
        if self
            .history
            .back()
            .is_none_or(|(_, current)| *current != view)
        {
            self.sequence += 1;
            self.history.push_back((self.sequence, view));
            while self.history.len() > MAX_HISTORY {
                self.history.pop_front();
            }
        }
    }

    /// The full view now (`studio.snapshot`).
    pub fn snapshot(&mut self, view: View) -> Snapshot {
        self.advance(view);
        let view = self
            .history
            .back()
            .map(|(_, view)| view.clone())
            .unwrap_or_default();
        Snapshot {
            stream: self.id.clone(),
            sequence: self.sequence,
            view,
        }
    }

    /// What changed since `since` in `stream` (`studio.update`).
    ///
    /// # Errors
    /// `stale` when `stream` is another stream, the stream no longer holds
    /// `since`, or what changed does not fit in one update: the client
    /// reads a fresh snapshot.
    pub fn update(
        &mut self,
        view: View,
        stream: &str,
        since: u64,
    ) -> std::result::Result<Update, Code> {
        self.advance(view);
        if stream != self.id {
            return Err(Code::Stale);
        }
        let old = self
            .history
            .iter()
            .find(|(sequence, _)| *sequence == since)
            .map(|(_, view)| view)
            .ok_or(Code::Stale)?;
        let current = &self.history.back().ok_or(Code::Stale)?.1;
        let (put, removed) = View::diff(old, current);
        let update = Update {
            stream: self.id.clone(),
            from: since,
            sequence: self.sequence,
            put,
            removed,
        };
        if encoded_len(&update) > MAX_VIEW_BYTES {
            return Err(Code::Stale);
        }
        Ok(update)
    }
}

/// A client's copy of the studio: a snapshot kept current with updates.
/// A missed update, a stale refusal, or a new stream forgets the copy, so
/// the next request is a fresh snapshot.
#[derive(Clone, Debug, Default)]
pub struct Mirror {
    snapshot: Option<Snapshot>,
}

impl Mirror {
    /// The studio as last read, when the mirror holds it.
    #[must_use]
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    /// The operation to send next: `studio.snapshot` without a copy, else
    /// `studio.update` from the copy's sequence.
    #[must_use]
    pub fn next(&self) -> Operation {
        match &self.snapshot {
            None => Operation::StudioSnapshot {},
            Some(snapshot) => Operation::StudioUpdate {
                stream: snapshot.stream.clone(),
                since: snapshot.sequence,
            },
        }
    }

    /// Take the host's answer to [`Mirror::next`]. Returns whether the
    /// studio changed.
    ///
    /// # Errors
    /// `stale` for an update that does not start where the copy is, which
    /// forgets the copy; an invalid snapshot or update, which leaves the
    /// copy as it was; `malformed` for any other outcome.
    pub fn accept(&mut self, outcome: &Outcome) -> Result<bool> {
        match outcome {
            Outcome::Studio { snapshot } => {
                snapshot.validate()?;
                let changed = self.snapshot.as_ref() != Some(&**snapshot);
                self.snapshot = Some((**snapshot).clone());
                Ok(changed)
            }
            Outcome::StudioUpdate { update } => {
                update.validate()?;
                let Some(current) = &mut self.snapshot else {
                    return fail(Code::Stale, "no studio snapshot to update");
                };
                if update.stream != current.stream || update.from != current.sequence {
                    self.snapshot = None;
                    return fail(Code::Stale, "missed a studio update; read a snapshot");
                }
                let mut view = current.view.clone();
                view.apply(&update.put, &update.removed);
                if view.validate().is_err() {
                    self.snapshot = None;
                    return fail(
                        Code::Stale,
                        "the studio update did not apply; read a snapshot",
                    );
                }
                let changed = view != current.view;
                current.view = view;
                current.sequence = update.sequence;
                Ok(changed)
            }
            _ => fail(Code::Malformed, "not a studio answer"),
        }
    }

    /// Take a refusal of [`Mirror::next`]: `stale` forgets the copy so the
    /// next request is a snapshot. Other refusals keep it.
    pub fn refused(&mut self, error: &Error) {
        if error.code == Code::Stale {
            self.snapshot = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(name: &str, activity: Activity) -> Seat {
        Seat {
            seat: name.into(),
            role: Role::Worker,
            route: "codex:gpt-6".into(),
            look: "amber".into(),
            desk: 0,
            activity,
            station: activity.station(),
            task: None,
            paused: false,
            spend: Spend::default(),
        }
    }

    fn task(id: &str, status: TaskStatus) -> Task {
        Task {
            task: id.into(),
            goal: "g1-0011aabb".into(),
            entry: "first".into(),
            position: 1,
            title: "First step".into(),
            seat: "builder".into(),
            depends_on: Vec::new(),
            status,
            spend: Spend::default(),
        }
    }

    fn view() -> View {
        let mut view = View {
            goals: vec![Goal {
                goal: "g1-0011aabb".into(),
                text: "Add a dark mode".into(),
                workspace: "app".into(),
                lead: "planner".into(),
                status: GoalStatus::Running,
                final_tasks: 0,
                total_tasks: 2,
                submitted_at: 1_790_000_000,
                spend: Spend::default(),
            }],
            seats: vec![
                seat("planner", Activity::Idle),
                seat("builder", Activity::Editing),
            ],
            tasks: vec![
                task("studio-g1-0011aabb-first", TaskStatus::Running),
                task("studio-g1-0011aabb-second", TaskStatus::Held),
            ],
            decisions: Vec::new(),
            repositories: vec![Repository {
                workspace: "app".into(),
                goals: 1,
                open_tasks: 1,
            }],
            logs: vec![Log {
                seat: "builder".into(),
                task: Some("studio-g1-0011aabb-first".into()),
                lines: vec![LogLine {
                    at: 1,
                    activity: Activity::Editing,
                    text: "editing: apply_patch".into(),
                }],
            }],
            memory: Vec::new(),
            messages: Vec::new(),
        };
        view.canonicalize();
        view
    }

    fn sent(sequence: u64, state: DeliveryState) -> SeatMessage {
        SeatMessage {
            message: message_key(sequence),
            seat: "builder".into(),
            from: None,
            at: 1_790_000_000,
            text: "Keep the palette to four colors.".into(),
            mode: DeliveryMode::MidTurn,
            state,
            task: Some("studio-g1-0011aabb-first".into()),
        }
    }

    #[test]
    fn a_message_carries_its_delivery_and_updates_when_the_engine_reads_it() {
        let mut accepted = view();
        accepted.messages.push(sent(12, DeliveryState::Accepted));
        accepted.messages.push(sent(9, DeliveryState::Consumed));
        accepted.canonicalize();
        accepted.validate().unwrap();
        // Key order is sending order, whatever the digit count.
        assert_eq!(accepted.messages[0].message, "00000000000000000009");
        let mut read = accepted.clone();
        read.messages[1].state = DeliveryState::Consumed;
        let (put, removed) = View::diff(&accepted, &read);
        assert_eq!(put.messages.len(), 1);
        assert!(removed.is_empty());
        let mut applied = accepted.clone();
        applied.apply(&put, &removed);
        assert_eq!(applied, read);
        // A waiting message names no task and waits for a turn boundary.
        let mut waiting = sent(13, DeliveryState::Waiting);
        assert!(Keyed::validate(&waiting).is_err());
        waiting.task = None;
        assert!(Keyed::validate(&waiting).is_err());
        waiting.mode = DeliveryMode::TurnBoundary;
        Keyed::validate(&waiting).unwrap();
        // A view a host encoded before messages were carried still reads.
        let old: View = serde_json::from_value(serde_json::to_value(view()).unwrap()).unwrap();
        assert!(old.messages.is_empty());
        // A removed message names its key.
        let (_, removed) = View::diff(&read, &old);
        assert_eq!(removed.len(), 2);
        assert!(removed.iter().all(|gone| gone.kind == Kind::Message));
    }

    #[test]
    fn spend_reads_as_dollars_and_old_views_read_without_it() {
        let label = |microusd, unpriced| Spend { microusd, unpriced }.label();
        assert_eq!(label(0, 0), "$0.00");
        assert_eq!(label(4_999, 0), "<$0.01");
        assert_eq!(label(1_250_000, 0), "$1.25");
        assert_eq!(label(12_345_678, 2), "$12.35+");
        // A view a host encoded before spend was kept still reads.
        let mut old = serde_json::to_value(view()).unwrap();
        assert!(old["goals"][0].get("spend").is_none());
        old["tasks"][0]["spend"] = serde_json::json!({"microusd": 7});
        let read: View = serde_json::from_value(old).unwrap();
        assert!(read.goals[0].spend.is_zero());
        assert_eq!(read.tasks[0].spend.microusd, 7);
        let total = read.tasks[0].spend.plus(Spend {
            microusd: 3,
            unpriced: 1,
        });
        assert_eq!(
            total,
            Spend {
                microusd: 10,
                unpriced: 1
            }
        );
    }

    #[test]
    fn an_update_applied_to_its_base_gives_the_newer_view() {
        let old = view();
        let mut new = old.clone();
        new.tasks[0].status = TaskStatus::Done;
        new.tasks.remove(1);
        new.seats[0].activity = Activity::Waiting;
        new.decisions.push(Decision {
            decision: "studio-g1-0011aabb-first".into(),
            goal: "g1-0011aabb".into(),
            task: Some("studio-g1-0011aabb-first".into()),
            seat: Some("builder".into()),
            kind: DecisionKind::Question,
            text: "Which palette?".into(),
            based_on: 4,
            approval: None,
        });
        new.validate().unwrap();
        let (put, removed) = View::diff(&old, &new);
        assert_eq!(put.tasks.len(), 1);
        assert_eq!(
            removed,
            vec![Removed {
                kind: Kind::Task,
                id: "studio-g1-0011aabb-second".into()
            }]
        );
        let mut applied = old.clone();
        applied.apply(&put, &removed);
        assert_eq!(applied, new);
        // Nothing changed is an empty update.
        let (put, removed) = View::diff(&new, &new);
        assert!(put.is_empty() && removed.is_empty());
    }

    #[test]
    fn a_missed_update_forces_a_fresh_snapshot() {
        let mut stream = Stream::new("00ff");
        let mut mirror = Mirror::default();
        assert_eq!(mirror.next(), Operation::StudioSnapshot {});
        let first = stream.snapshot(view());
        mirror
            .accept(&Outcome::Studio {
                snapshot: Box::new(first.clone()),
            })
            .unwrap();
        assert_eq!(
            mirror.next(),
            Operation::StudioUpdate {
                stream: "00ff".into(),
                since: first.sequence
            }
        );
        // The host moves on twice; the client hears only the second
        // update, which does not start where its copy is.
        let mut second = view();
        second.tasks[0].status = TaskStatus::Waiting;
        let skipped = stream
            .update(second.clone(), "00ff", first.sequence)
            .unwrap();
        let mut third = second.clone();
        third.tasks[0].status = TaskStatus::Done;
        let later = stream
            .update(third.clone(), "00ff", skipped.sequence)
            .unwrap();
        let error = mirror
            .accept(&Outcome::StudioUpdate {
                update: Box::new(later),
            })
            .unwrap_err();
        assert_eq!(error.code, Code::Stale);
        assert!(mirror.snapshot().is_none());
        assert_eq!(mirror.next(), Operation::StudioSnapshot {});
        // The fresh snapshot is the studio now.
        let fresh = stream.snapshot(third.clone());
        mirror
            .accept(&Outcome::Studio {
                snapshot: Box::new(fresh),
            })
            .unwrap();
        assert_eq!(mirror.snapshot().unwrap().view, third);
    }

    #[test]
    fn updates_in_order_keep_the_mirror_equal_to_a_fresh_snapshot() {
        let mut stream = Stream::new("0a");
        let mut mirror = Mirror::default();
        mirror
            .accept(&Outcome::Studio {
                snapshot: Box::new(stream.snapshot(view())),
            })
            .unwrap();
        let mut now = view();
        for status in [TaskStatus::Waiting, TaskStatus::Running, TaskStatus::Done] {
            now.tasks[0].status = status;
            let Operation::StudioUpdate { stream: id, since } = mirror.next() else {
                panic!("an update is next");
            };
            let update = stream.update(now.clone(), &id, since).unwrap();
            assert!(
                mirror
                    .accept(&Outcome::StudioUpdate {
                        update: Box::new(update)
                    })
                    .unwrap()
            );
        }
        // An unchanged studio is an empty update at the same sequence.
        let sequence = stream.sequence();
        let quiet = stream.update(now.clone(), "0a", sequence).unwrap();
        assert_eq!((quiet.from, quiet.sequence), (sequence, sequence));
        assert!(
            !mirror
                .accept(&Outcome::StudioUpdate {
                    update: Box::new(quiet)
                })
                .unwrap()
        );
        assert_eq!(mirror.snapshot().unwrap(), &stream.snapshot(now));
    }

    #[test]
    fn another_stream_or_a_forgotten_sequence_is_stale() {
        let mut stream = Stream::new("0b");
        let first = stream.snapshot(view());
        // A host that started again is another stream.
        assert_eq!(
            Stream::new("0c").update(view(), "0b", first.sequence),
            Err(Code::Stale)
        );
        // A sequence the stream never held, or no longer holds.
        assert_eq!(stream.update(view(), "0b", 0), Err(Code::Stale));
        let mut now = view();
        for n in 0..=MAX_HISTORY {
            now.goals[0].submitted_at = n as u64;
            stream.update(now.clone(), "0b", stream.sequence()).unwrap();
        }
        assert_eq!(stream.update(now, "0b", first.sequence), Err(Code::Stale));
        // A stale refusal forgets the mirror's copy; another keeps it.
        let mut mirror = Mirror::default();
        mirror
            .accept(&Outcome::Studio {
                snapshot: Box::new(first),
            })
            .unwrap();
        mirror.refused(&Error::new(Code::Unavailable, "busy"));
        assert!(mirror.snapshot().is_some());
        mirror.refused(&Error::new(Code::Stale, "gone"));
        assert!(mirror.snapshot().is_none());
    }

    #[test]
    fn a_view_drops_old_log_lines_then_finished_goals_to_fit() {
        let mut big = view();
        big.logs[0].lines = (0..MAX_LOG_LINES as u64)
            .map(|at| LogLine {
                at,
                activity: Activity::Reading,
                text: "r".repeat(MAX_LINE),
            })
            .collect();
        let whole = encoded_len(&big);
        assert!(big.clone().fit(whole));
        let mut fitted = big.clone();
        assert!(fitted.fit(whole - MAX_LINE));
        assert_eq!(fitted.logs[0].lines.len(), MAX_LOG_LINES - 1);
        assert_eq!(fitted.logs[0].lines[0].at, 1);
        // With no log lines left, a running goal is never dropped.
        let mut tight = view();
        tight.logs[0].lines.clear();
        assert!(!tight.clone().fit(10));
        tight.goals[0].status = GoalStatus::Done;
        let size = encoded_len(&tight);
        assert!(tight.fit(size - 1));
        assert!(tight.goals.is_empty() && tight.tasks.is_empty());
    }

    #[test]
    fn views_are_bounded_and_in_key_order() {
        let good = view();
        good.validate().unwrap();
        let mut unsorted = good.clone();
        unsorted.seats.reverse();
        assert_eq!(unsorted.validate().unwrap_err().code, Code::Malformed);
        let mut path = good.clone();
        path.tasks[0].task = "../escape".into();
        assert!(path.validate().is_err());
        let mut question = good.clone();
        question.decisions.push(Decision {
            decision: "g1-0011aabb".into(),
            goal: "g1-0011aabb".into(),
            task: None,
            seat: None,
            kind: DecisionKind::Question,
            text: "Which?".into(),
            based_on: 1,
            approval: None,
        });
        assert!(question.validate().is_err());
        let mut long = good;
        long.logs[0].lines[0].text = "x".repeat(MAX_LINE + 1);
        assert_eq!(long.validate().unwrap_err().code, Code::Bounds);
    }

    fn memory(sequence: u64, kind: MemoryKind, pinned: bool) -> Memory {
        Memory {
            entry: memory_key(sequence),
            kind,
            author: "@planner".into(),
            goal: Some("g1-0011aabb".into()),
            text: "Plan for goal g1-0011aabb:\n- first: First step [@builder]".into(),
            pinned,
        }
    }

    #[test]
    fn shared_memory_and_the_pinned_plan_travel_in_the_view() {
        let mut old = view();
        old.memory = vec![
            memory(1, MemoryKind::Plan, true),
            memory(2, MemoryKind::Decision, false),
        ];
        old.validate().unwrap();
        assert_eq!(
            old.plan().map(|plan| plan.entry.as_str()),
            Some("m000000000001")
        );
        assert!(
            memory_key(9) < memory_key(10),
            "key order is sequence order"
        );
        // A newer plan takes the pin; the update carries both entries.
        let mut new = old.clone();
        new.memory[0].pinned = false;
        new.memory.push(memory(3, MemoryKind::Plan, true));
        new.validate().unwrap();
        let (put, removed) = View::diff(&old, &new);
        assert_eq!(put.memory.len(), 2);
        assert!(removed.is_empty());
        let mut applied = old.clone();
        applied.apply(&put, &removed);
        assert_eq!(applied, new);
        assert_eq!(applied.plan().unwrap().entry, memory_key(3));
        // Two pins, a pinned note, or too much text are refused.
        let mut twice = new.clone();
        twice.memory[0].pinned = true;
        assert!(twice.validate().is_err());
        let mut note = old.clone();
        note.memory[1].pinned = true;
        note.memory[0].pinned = false;
        assert!(note.validate().is_err());
        let mut long = old.clone();
        long.memory[1].text = "x".repeat(MAX_MEMORY_TEXT + 1);
        assert_eq!(long.validate().unwrap_err().code, Code::Bounds);
        // A view from a host without shared memory still reads.
        let older: View = serde_json::from_str("{\"goals\":[]}").unwrap();
        assert!(older.memory.is_empty() && older.plan().is_none());
    }

    #[test]
    fn a_view_drops_unpinned_memory_before_any_goal() {
        let mut big = view();
        big.logs[0].lines.clear();
        big.goals[0].status = GoalStatus::Done;
        big.memory = vec![
            memory(1, MemoryKind::Plan, true),
            memory(2, MemoryKind::Note, false),
            memory(3, MemoryKind::Note, false),
        ];
        let whole = encoded_len(&big);
        let mut fitted = big.clone();
        assert!(fitted.fit(whole - 1));
        assert_eq!(fitted.memory.len(), 2);
        assert_eq!(
            fitted.memory[1].entry,
            memory_key(3),
            "the oldest goes first"
        );
        assert_eq!(fitted.goals.len(), 1, "the goal stays while memory can go");
        // The pinned plan goes only with nothing else left to drop.
        let mut pinned = big;
        pinned.memory.truncate(1);
        let size = encoded_len(&pinned);
        assert!(pinned.fit(size - 1));
        assert!(pinned.goals.is_empty());
        assert_eq!(pinned.memory.len(), 1);
    }

    #[test]
    fn a_merge_decision_carries_the_review_and_fitting_text() {
        let decision = |verdict, text: &str| MergeDecision {
            task: "studio-g1-0011aabb-first".into(),
            base: "a".repeat(40),
            head_commit: "a".repeat(40),
            head: "b".repeat(40),
            verdict,
            text: text.into(),
            command: "c".repeat(64),
            issued_at: 1_790_000_000,
        };
        decision(Verdict::Merge, "").validate().unwrap();
        decision(Verdict::Reject, "").validate().unwrap();
        decision(Verdict::RequestChanges, "Name the flag")
            .validate()
            .unwrap();
        assert!(decision(Verdict::Merge, "ship it").validate().is_err());
        assert!(decision(Verdict::RequestChanges, " ").validate().is_err());
        let mut short = decision(Verdict::Merge, "");
        short.head = "main".into();
        assert!(short.validate().is_err());
        let merged = Merged {
            task: "studio-g1-0011aabb-first".into(),
            base: "a".repeat(40),
            head_commit: "a".repeat(40),
            head: "b".repeat(40),
            verdict: Verdict::Reject,
            publication: None,
        };
        merged.validate().unwrap();
        assert!(merged.answers(&decision(Verdict::Reject, "")));
        assert!(!merged.answers(&decision(Verdict::Merge, "")));
        let mut unpublished = merged;
        unpublished.verdict = Verdict::Merge;
        assert!(unpublished.validate().is_err());
    }

    #[test]
    fn an_approval_names_its_step_and_a_high_risk_step_no_rule() {
        let step = Approval {
            tool: "shell".into(),
            command: "cargo test -p coder".into(),
            cwd: "/work/repo".into(),
            reason: "runs the crate's tests".into(),
            risk: Risk::Low,
            always: Some("builder may run shell `cargo test -p coder` in /work/repo".into()),
        };
        step.validate().unwrap();
        let mut approval = Decision {
            decision: "studio-g1-0011aabb-first".into(),
            goal: "g1-0011aabb".into(),
            task: Some("studio-g1-0011aabb-first".into()),
            seat: Some("builder".into()),
            kind: DecisionKind::Approval,
            text: "May I run the tests?".into(),
            based_on: 4,
            approval: Some(step.clone()),
        };
        approval.validate().unwrap();
        let json = serde_json::to_string(&approval).unwrap();
        assert_eq!(serde_json::from_str::<Decision>(&json).unwrap(), approval);
        // A view without the field still reads.
        approval.approval = None;
        let old = serde_json::to_string(&approval).unwrap();
        assert!(!old.contains("\"approval\":"));
        approval.kind = DecisionKind::Question;
        approval.approval = Some(step.clone());
        assert!(
            approval.validate().is_err(),
            "only an approval names a step"
        );
        let mut high = step.clone();
        high.risk = Risk::High;
        assert!(high.validate().is_err(), "a high-risk step has no rule");
        high.always = None;
        high.validate().unwrap();
        let mut relative = step.clone();
        relative.cwd = "repo".into();
        assert!(relative.validate().is_err());
        let mut long = step;
        long.command = "x".repeat(MAX_STEP_COMMAND + 1);
        assert_eq!(long.validate().unwrap_err().code, Code::Bounds);
        assert_eq!(Risk::Medium.label(), "Medium risk");
    }
}
