//! Agent Studio over NIP-HOST: the intents a view sends, the snapshot and
//! sequenced updates the host answers, and the mirror a client keeps
//! (`docs/verse/agent-studio.md`, "The client is a view").
//!
//! The host's studio coordinator is the source of truth; a view holds no
//! business logic. It sends intents (`studio.goal.submit`, `studio.seat.*`,
//! `studio.task.*`, `studio.decision.answer`, `studio.review.open`, and
//! `studio.merge.decide`), each one NIP-HOST operation with exactly one
//! required right, and it draws what the host answers:
//!
//! - `studio.snapshot` answers a full [`Snapshot`]: goals, seats, tasks,
//!   decisions, repository summaries, and a bounded log tail per seat.
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
//! A merge decision binds to the review's three revisions (base, `HEAD`
//! commit, and content tree). The host reads the review again and refuses
//! a decision whose revisions differ as `stale`, so the client reloads the
//! review rather than landing a change nobody read.
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
/// The longest answer a `studio.decision.answer` carries: a question's
/// answer, or a plan for a goal's plan decision.
pub const MAX_ANSWER: usize = 64 * 1024;
/// The longest text a **Request changes** or **Reject** carries.
pub const MAX_REVIEW_TEXT: usize = 16 * 1024;
/// The largest encoded view or update, inside one relay frame.
pub const MAX_VIEW_BYTES: usize = 48 * 1024;
/// How many earlier views a host keeps to compute updates from.
pub const MAX_HISTORY: usize = 64;

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
        if encoded_len(self) > MAX_VIEW_BYTES {
            return fail(Code::Bounds, "studio view exceeds its bound");
        }
        Ok(())
    }

    /// Shrink the view until it encodes in `max` bytes: drop the oldest
    /// line of the longest log tail, then the oldest finished goal with
    /// its tasks and decisions. Returns whether it fits.
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
            MAX_GOALS + MAX_SEATS * 2 + MAX_TASKS + MAX_DECISIONS + MAX_REPOSITORIES,
        )?;
        for gone in &self.removed {
            match gone.kind {
                Kind::Seat | Kind::Log => seat_name(&gone.id)?,
                Kind::Repository => line(&gone.id, 128)?,
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
        };
        view.canonicalize();
        view
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
        });
        assert!(question.validate().is_err());
        let mut long = good;
        long.logs[0].lines[0].text = "x".repeat(MAX_LINE + 1);
        assert_eq!(long.validate().unwrap_err().code, Code::Bounds);
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
}
