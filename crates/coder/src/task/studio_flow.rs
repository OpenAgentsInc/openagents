//! The path from a worker's finished task to the person's merge decision
//! (`docs/verse/agentcraft-parity.md`, "Orchestration"; the design follows
//! AgentCraft's Foreman, reimplemented here).
//!
//! A plan entry released with a worktree of its own
//! ([`Studio::with_worktrees`]) carries a [`Flow`]. Each coordinator pass
//! ([`Studio::reconcile`]) moves it on once its task's turn ended and the
//! turn's independent check ([`super::super::local_checks`]) settled:
//!
//! 1. **No change.** A task whose worktree holds no change from its base
//!    is done, with no merge decision.
//! 2. **Checks.** The check's verdict is the shared verification
//!    ([`Verification`]: passed, failed, unverifiable, or not run). A
//!    failure goes back to the same task once, as a follow-up turn that
//!    carries the failure; a second failure goes on to review with the
//!    failure noted.
//! 3. **Lead review.** Unless the person turned it off
//!    ([`Studio::set_lead_review`]), the goal's lead reviews the diff, the
//!    check result, and the task's history in a task of its own, and ends
//!    its reply with a verdict ([`REVIEW_SCHEMA`]): `approve` asks the
//!    person for the merge decision; `changes` goes back to the worker as a
//!    follow-up, then through the checks and the review again. After
//!    [`MAX_CHANGE_ROUNDS`] requests the change goes to the person with the
//!    lead's last notes.
//! 4. **Merge decision.** The change waits for the person
//!    ([`Progress::Merge`]); a passing check alone never means accepted. A
//!    merge that would conflict with the person's branch ([`git::merge`])
//!    sends the task back to its worker to merge that branch in and
//!    resolve it, then through the checks and the review again. A merge
//!    that lands ends the flow, and only then do the entry's dependents
//!    start.
//!
//! A follow-up is saved as exact command bytes before it is applied, as a
//! release is, so a restart applies the same bytes again, which the inbox
//! answers with the original receipt: no turn is lost or doubled.

use std::path::Path;

use coder_host::access::review::{Completeness, PublishState, TaskReview};
use nostr::contracts::parse_strict_bounded;
use serde::{Deserialize, Serialize};

use super::super::{
    Action, COMMAND_SCHEMA, Checks, Command, RequestedConfiguration, Task, TaskIntent, Workspace,
    autostart, local, publish, review,
};
use super::{
    DEVICE, DIR, Error, Inbox, MemoryKind, Party, PlanEntry, Progress, Released, Role, STATE_FILE,
    Seat, Slot, SlotState, State, Studio, cut, fenced_block, git, is_zero, newest_within, one_line,
    progress, progress_of, slot_progress,
};

/// The verdict block a lead's review reply ends with.
pub const REVIEW_SCHEMA: &str = "openagents.coder.studio-review.v1";
/// How many times a failed check goes back to the worker before review.
pub const MAX_FIX_ROUNDS: u32 = 1;
/// How many times the lead sends a change back before it goes to the
/// person with the lead's notes.
pub const MAX_CHANGE_ROUNDS: u32 = 2;
/// How many conflicting merges go back to the worker; after that the
/// person resolves the conflict.
pub const MAX_CONFLICT_ROUNDS: u32 = 3;
/// The most conflicting files a conflict names.
pub const MAX_CONFLICT_FILES: usize = 32;
/// The most notes a change carries to the merge decision; the oldest go
/// first.
const MAX_NOTES: usize = 8;
const NOTE_MAX: usize = 2 * 1024;
/// How much of the diff a review's prompt carries.
const DIFF_MAX: usize = 12 * 1024;
const FAILURE_MAX: usize = 2 * 1024;
/// How much of the task's follow-ups a review's prompt carries.
const HISTORY_MAX: usize = 3 * 1024;
/// How much of the notes a review's prompt carries.
const NOTES_IN_PROMPT: usize = 4 * 1024;
/// How much of one follow-up the history carries.
const HISTORY_LINE_MAX: usize = 600;
/// A prompt's bound, within the inbox's 32 KiB.
const PROMPT_MAX: usize = 30 * 1024;
const VERDICT_MAX: usize = 64 * 1024;
/// A conflict noted by a merge waits beside the studio document as
/// `conflict-<task>.json` until a pass reads it.
const CONFLICT_PREFIX: &str = "conflict-";
const MAX_CONFLICT_BYTES: u64 = 64 * 1024;

/// What a turn's independent check found: the shared verification
/// vocabulary of `docs/verse/networking.md`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    Passed,
    Failed,
    /// The check ran and could not decide, or its evidence was disputed.
    Unverifiable,
    /// No check ran: the run listed nothing to check.
    NotRun,
}

impl Verification {
    /// The verification of a task's checks state; `None` while they run.
    #[must_use]
    pub fn of(checks: Checks) -> Option<Self> {
        match checks {
            Checks::Running => None,
            Checks::NotRun => Some(Self::NotRun),
            Checks::Passed => Some(Self::Passed),
            Checks::Failed => Some(Self::Failed),
            Checks::Unavailable | Checks::Disputed => Some(Self::Unverifiable),
        }
    }

    /// The same value in the protocol contracts' type.
    #[must_use]
    pub fn shared(self) -> nostr::contracts::Verification {
        match self {
            Self::Passed => nostr::contracts::Verification::Passed,
            Self::Failed => nostr::contracts::Verification::Failed,
            Self::Unverifiable => nostr::contracts::Verification::Unverifiable,
            Self::NotRun => nostr::contracts::Verification::NotRun,
        }
    }

    /// Its wire word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Unverifiable => "unverifiable",
            Self::NotRun => "not_run",
        }
    }
}

/// Where a plan entry's change is on its way to the person.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The worker's turn runs, or its end is not looked at yet.
    #[default]
    Work,
    /// The lead reviews the change.
    Review,
    /// The change waits on the person's merge decision.
    Merge,
    /// The person's merge conflicted; the worker is sent to merge the
    /// branch in.
    Conflict,
    /// The task changed no files: done with no merge decision.
    Unchanged,
    /// The person's merge landed.
    Merged,
    /// The person rejected the change.
    Rejected,
}

/// A merge that would conflict, as the merge noted it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Conflict {
    /// The branch the change merges into.
    pub target: String,
    /// The conflicting files Git named, at most [`MAX_CONFLICT_FILES`].
    pub files: Vec<String>,
    /// The task's commit the merge was asked for.
    pub head_commit: String,
}

/// A plan entry's change on its way to the person's merge decision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    /// The task it follows; a retried entry starts a new flow.
    pub task_id: String,
    pub stage: Stage,
    /// The task's turn whose end it handled last; 0 before the first.
    #[serde(default)]
    pub turn: u64,
    /// Fix rounds sent after a failed check since the last review or
    /// conflict.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fixes: u32,
    /// Times the lead sent the change back.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub changes: u32,
    /// Conflicting merges sent back to the worker.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub conflicts: u32,
    /// Review tasks made for it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub reviews: u32,
    /// What the last handled turn's check found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<Verification>,
    /// What the person reads with the merge decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// The lead's review task while [`Stage::Review`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<Slot>,
    /// The conflict to send back while [`Stage::Conflict`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<Conflict>,
    /// A follow-up's exact command bytes while it may or may not be
    /// applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The computer the task runs on while it does, when it is not this
    /// host (#10930). A remote task has no local inbox entry: the agent
    /// that placed it moves the flow itself, and the coordinator leaves it
    /// alone until the slot submits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// The task's identity on `remote`'s host, kept so a restart resumes
    /// watching it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_task: Option<String>,
    /// `remote`'s checkout the task ran in, kept so the change comes back
    /// after a restart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_checkout: Option<String>,
}

impl Flow {
    /// A new flow for task `task_id`.
    #[must_use]
    pub fn new(task_id: &str) -> Self {
        Self {
            task_id: task_id.to_owned(),
            stage: Stage::Work,
            turn: 0,
            fixes: 0,
            changes: 0,
            conflicts: 0,
            reviews: 0,
            verification: None,
            notes: Vec::new(),
            review: None,
            conflict: None,
            command: None,
            remote: None,
            remote_task: None,
            remote_checkout: None,
        }
    }

    /// A new flow for a task `task_id` that runs on the computer `remote`,
    /// in its checkout `checkout`.
    #[must_use]
    pub fn remote(task_id: &str, remote: &str, checkout: &str) -> Self {
        Self {
            remote: Some(remote.to_owned()),
            remote_checkout: Some(checkout.to_owned()),
            ..Self::new(task_id)
        }
    }
}

/// What the lead decided about a change.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Ask the person for the merge decision.
    Approve,
    /// Send the notes back to the worker.
    Changes,
}

/// The verdict block a lead's review reply ends with.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewVerdict {
    pub schema: String,
    pub verdict: Verdict,
    #[serde(default)]
    pub notes: String,
}

/// The verdict in a lead's review reply: the last fenced block that holds
/// [`REVIEW_SCHEMA`], else the whole reply when it is that JSON object.
#[must_use]
pub fn review_in_reply(reply: &str) -> Option<ReviewVerdict> {
    let block = fenced_block(reply, REVIEW_SCHEMA)?;
    let value = parse_strict_bounded(block.as_bytes(), VERDICT_MAX).ok()?;
    let verdict: ReviewVerdict = serde_json::from_value(value).ok()?;
    (verdict.schema == REVIEW_SCHEMA).then_some(verdict)
}

/// A plan entry's progress with its change's flow: a finished task whose
/// change is still in its checks or the lead's review is
/// [`Progress::Review`], one that waits on the person is
/// [`Progress::Merge`], a rejected one is cancelled, and only a merged or
/// unchanged one is done. A merged change is done whatever its task does
/// after: what landed is what its dependents build on.
#[must_use]
pub fn entry_progress(entry: &PlanEntry, tasks: &dyn Inbox) -> Progress {
    let raw = slot_progress(&entry.slot, tasks);
    let Some(flow) = entry
        .flow
        .as_ref()
        .filter(|flow| flow.task_id == entry.slot.task_id)
    else {
        return raw;
    };
    if flow.stage == Stage::Merged && entry.slot.state == SlotState::Submitted {
        return Progress::Done;
    }
    if raw != Progress::Done {
        return if flow.stage == Stage::Rejected && raw.is_final() {
            Progress::Cancelled
        } else {
            raw
        };
    }
    match flow.stage {
        Stage::Merged | Stage::Unchanged => Progress::Done,
        Stage::Merge => Progress::Merge,
        Stage::Rejected => Progress::Cancelled,
        Stage::Work | Stage::Review | Stage::Conflict => Progress::Review,
    }
}

/// The flow stage of studio task `task`, read from the studio document of
/// the task store at `store` without taking its lock, as
/// [`git::seat_of`] reads it.
#[must_use]
pub fn stage_of(store: &Path, task: &str) -> Option<Stage> {
    let bytes = std::fs::read(store.join(DIR).join(STATE_FILE)).ok()?;
    let state: State = serde_json::from_slice(&bytes).ok()?;
    state
        .goals
        .iter()
        .flat_map(|goal| goal.plan.iter())
        .find(|entry| entry.slot.task_id == task)?
        .flow
        .as_ref()
        .filter(|flow| flow.task_id == task)
        .map(|flow| flow.stage)
}

fn conflict_file(task: &str) -> String {
    format!("{CONFLICT_PREFIX}{task}.json")
}

/// Note for the studio of the task store at `store` that merging task
/// `task` conflicted. The coordinator's next pass reads it.
///
/// # Errors
/// A plain sentence when the note cannot be written.
pub(super) fn note_conflict(store: &Path, task: &str, conflict: &Conflict) -> Result<(), String> {
    if !super::super::identifier(task, false) {
        return Err("the task identity cannot name a file".into());
    }
    let dir = store.join(DIR);
    if !dir.is_dir() {
        return Err("the task store holds no studio".into());
    }
    let bytes = serde_json::to_vec(conflict).map_err(|error| error.to_string())?;
    super::super::replace_file(&dir, &conflict_file(task), &bytes)
        .map_err(|error| error.to_string())
}

fn take_conflict(dir: &Path, task: &str) -> Option<Conflict> {
    let path = dir.join(conflict_file(task));
    let meta = std::fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_CONFLICT_BYTES {
        return None;
    }
    serde_json::from_slice(&std::fs::read(&path).ok()?).ok()
}

fn forget_conflict(dir: &Path, task: &str) {
    let _ = std::fs::remove_file(dir.join(conflict_file(task)));
}

/// Whether the person's merge of `task` landed.
fn published(store: &Path, task: &str) -> bool {
    publish::last(store, task).is_some_and(|kept| kept.state == PublishState::Published)
}

/// Display text: control characters other than line breaks and tabs
/// dropped, at most `max` bytes.
fn clean(text: &str, max: usize) -> String {
    let kept: String = text
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
        .collect();
    cut(kept.trim(), max)
}

/// Why `task`'s check failed: the check report's reason and each failed
/// check's own, or a plain sentence when none is recorded.
fn failure_of(task: &Task) -> String {
    let Some(report) = task.run.as_ref().and_then(|run| run.check_report.as_ref()) else {
        return "The checks failed; no reason was recorded.".into();
    };
    let mut parts: Vec<String> = report.reason.iter().cloned().collect();
    if let Some(checks) = report
        .evidence
        .as_ref()
        .and_then(|evidence| evidence.get("checks"))
        .and_then(serde_json::Value::as_array)
    {
        for check in checks {
            if check.get("verdict").and_then(serde_json::Value::as_str) == Some("passed") {
                continue;
            }
            if let Some(reason) = check.get("reason").and_then(serde_json::Value::as_str) {
                parts.push(reason.to_owned());
            }
        }
    }
    let text = clean(&parts.join("\n"), FAILURE_MAX);
    if text.is_empty() {
        "The checks failed; no reason was recorded.".into()
    } else {
        text
    }
}

/// The identity of the `n`th review of task `task`: a digest, so it is
/// the same across restarts and lower-case 32-byte hex as host access
/// names tasks.
fn review_task_id(task: &str, n: u32) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::new()
        .chain_update(b"openagents.studio.review.v1\0")
        .chain_update(task.as_bytes())
        .chain_update(b"\0")
        .chain_update(n.to_string().as_bytes())
        .finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn io(message: String) -> Error {
    Error::Tasks(super::super::Error::Io(std::io::Error::other(message)))
}

impl Studio {
    /// Whether a goal's lead reviews each finished change before the
    /// person's merge decision.
    #[must_use]
    pub fn lead_review(&self) -> bool {
        self.state.lead_review
    }

    /// Turn the lead's review of finished changes on or off. Off, a
    /// finished change goes from its checks to the person's merge
    /// decision; a review already under way finishes.
    ///
    /// # Errors
    /// The studio document cannot be written.
    pub fn set_lead_review(&mut self, on: bool) -> Result<(), Error> {
        if self.state.lead_review != on {
            self.state.lead_review = on;
            self.save()?;
        }
        Ok(())
    }

    /// Mark the change of studio task `task` merged once the person's
    /// merge landed: its plan entry is done, so the entries that wait on it
    /// start at the next pass ([`Studio::reconcile`]), and its flow sends
    /// nothing more to its worker. Returns whether this changed the studio;
    /// a task no plan entry holds changes nothing.
    ///
    /// # Errors
    /// The studio document cannot be written.
    pub fn note_merged(&mut self, task: &str) -> Result<bool, Error> {
        let Some((index, entry)) = self
            .state
            .goals
            .iter()
            .enumerate()
            .find_map(|(index, goal)| {
                goal.plan
                    .iter()
                    .position(|entry| {
                        entry.slot.task_id == task && entry.slot.state == SlotState::Submitted
                    })
                    .map(|entry| (index, entry))
            })
        else {
            return Ok(false);
        };
        let item = &mut self.state.goals[index].plan[entry];
        // A flow left from an earlier attempt of a retried entry is not
        // this task's.
        if item.flow.as_ref().is_some_and(|flow| flow.task_id != task) {
            item.flow = None;
        }
        let flow = self.flow_mut(index, entry);
        if flow.stage == Stage::Merged {
            return Ok(false);
        }
        flow.stage = Stage::Merged;
        flow.review = None;
        flow.conflict = None;
        flow.command = None;
        forget_conflict(&self.dir, task);
        self.save()?;
        Ok(true)
    }

    fn flow_mut(&mut self, index: usize, entry: usize) -> &mut Flow {
        let item = &mut self.state.goals[index].plan[entry];
        let task_id = &item.slot.task_id;
        item.flow.get_or_insert_with(|| Flow::new(task_id))
    }

    /// Add `text` to what the person reads with entry `entry`'s merge
    /// decision.
    fn note(&mut self, index: usize, entry: usize, text: String) {
        let flow = self.flow_mut(index, entry);
        if flow.notes.len() >= MAX_NOTES {
            flow.notes.remove(0);
        }
        flow.notes.push(clean(&text, NOTE_MAX));
    }

    fn set_stage(&mut self, index: usize, entry: usize, stage: Stage) -> Result<(), Error> {
        self.flow_mut(index, entry).stage = stage;
        self.save()
    }

    /// Move goal `index`'s plan entries along their flows; `reply`
    /// answers a task identity with its reply. Returns the review tasks it
    /// released.
    pub(super) fn advance(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        now: u64,
        reply: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Vec<Released>, Error> {
        if !self.state.goals[index]
            .plan
            .iter()
            .any(|entry| entry.flow.is_some())
        {
            return Ok(Vec::new());
        }
        // A review's spend is kept before a pass lets go of its task.
        self.record_spend(&*tasks)?;
        let mut released = Vec::new();
        for entry in 0..self.state.goals[index].plan.len() {
            released.extend(self.advance_entry(tasks, index, entry, now, reply)?);
        }
        Ok(released)
    }

    fn advance_entry(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        entry: usize,
        now: u64,
        reply: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Option<Released>, Error> {
        let item = &self.state.goals[index].plan[entry];
        let Some(flow) = item.flow.clone() else {
            return Ok(None);
        };
        let task_id = item.slot.task_id.clone();
        if item.slot.state != SlotState::Submitted || flow.task_id != task_id {
            return Ok(None);
        }
        // A follow-up a restart interrupted.
        if let Some(bytes) = flow.command {
            self.send_follow_up(tasks, index, entry, &bytes, now)?;
            return Ok(None);
        }
        if matches!(
            flow.stage,
            Stage::Merged | Stage::Unchanged | Stage::Rejected
        ) {
            forget_conflict(&self.dir, &task_id);
            return Ok(None);
        }
        let Some(task) = tasks.task(&task_id) else {
            return Ok(None);
        };
        // A merge the person made ends the flow.
        if published(&self.store, &task_id) {
            forget_conflict(&self.dir, &task_id);
            let flow = self.flow_mut(index, entry);
            flow.stage = Stage::Merged;
            flow.review = None;
            flow.conflict = None;
            self.save()?;
            return Ok(None);
        }
        let mut stage = flow.stage;
        if stage == Stage::Merge {
            if let Some(conflict) = take_conflict(&self.dir, &task_id) {
                stage = self.on_conflict(index, entry, conflict)?;
            }
        } else {
            // Only a change that waits on the merge decision is merged.
            forget_conflict(&self.dir, &task_id);
        }
        let turn = task.turn() as u64;
        let settled = progress_of(&task) == Progress::Done
            && task.checks != Checks::Running
            && !(task.checks == Checks::NotRun && tasks.checks_pending(&task_id));
        match stage {
            Stage::Conflict => {
                if settled {
                    self.conflict_follow_up(tasks, index, entry, &task, now)?;
                }
                return Ok(None);
            }
            Stage::Review if turn == flow.turn => {
                return self.advance_review(tasks, index, entry, now, reply);
            }
            Stage::Review => {
                // The worker's task took another turn during the review:
                // that turn is what counts.
                let flow = self.flow_mut(index, entry);
                flow.stage = Stage::Work;
                flow.review = None;
                self.save()?;
            }
            _ => {}
        }
        if !settled || turn == flow.turn {
            return Ok(None);
        }
        self.handle_end(tasks, index, entry, &task, now)
    }

    /// The change of studio task `task_id` at its worktree's revisions now,
    /// when it has a worktree here that Git can read.
    fn change_of(&self, task_id: &str) -> Option<TaskReview> {
        let record = local::record(&self.store, task_id)?;
        review::read(task_id, Path::new(&record.worktree), &record.base, DIFF_MAX).ok()
    }

    /// Handle the end of entry `entry`'s task's turn: close it when it
    /// changed nothing, send a failed check back once, else go to review.
    fn handle_end(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        entry: usize,
        task: &Task,
        now: u64,
    ) -> Result<Option<Released>, Error> {
        let verification = Verification::of(task.checks).unwrap_or(Verification::NotRun);
        let change = self.change_of(&task.task_id);
        let goal_id = self.state.goals[index].goal_id.clone();
        let item = &self.state.goals[index].plan[entry];
        let (id, seat) = (item.id.clone(), item.slot.seat.clone());
        let flow = self.flow_mut(index, entry);
        flow.turn = task.turn() as u64;
        flow.verification = Some(verification);
        flow.review = None;
        let unchanged = change.as_ref().is_some_and(|change| {
            change.files_total == 0 && change.completeness == Completeness::Complete
        });
        if unchanged {
            flow.stage = Stage::Unchanged;
            self.push_memory(
                MemoryKind::Note,
                Party::Seat { name: seat },
                Some(goal_id),
                format!("Task `{id}` changed no files, so it is done with no merge decision."),
            );
            self.save()?;
            return Ok(None);
        }
        if verification == Verification::Failed {
            let failure = failure_of(task);
            if flow.fixes < MAX_FIX_ROUNDS {
                let prompt = format!(
                    "The checks on your change failed:\n\n{failure}\n\nFix the change in this \
                     worktree so the checks pass, then commit it. Do not push it."
                );
                self.follow_up(tasks, index, entry, task, "fix", &prompt, now, |flow| {
                    flow.fixes += 1;
                })?;
                return Ok(None);
            }
            self.note(
                index,
                entry,
                format!("The checks still failed after a fix round: {failure}"),
            );
        }
        self.send_to_review(tasks, index, entry, now)
    }

    /// Hand entry `entry`'s change to the lead's review, or straight to
    /// the person when the review is off or the goal has no lead seat.
    fn send_to_review(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        entry: usize,
        now: u64,
    ) -> Result<Option<Released>, Error> {
        let lead = self.state.goals[index].lead.seat.clone();
        let reviewed = self.state.lead_review
            && self
                .state
                .seat(&lead)
                .is_some_and(|seat| seat.role == Role::Lead);
        if !reviewed {
            self.set_stage(index, entry, Stage::Merge)?;
            return Ok(None);
        }
        let flow = self.flow_mut(index, entry);
        flow.reviews += 1;
        flow.review = Some(Slot {
            task_id: review_task_id(&flow.task_id, flow.reviews),
            seat: lead.clone(),
            state: SlotState::Held,
            command: None,
            attempt: 1,
        });
        flow.stage = Stage::Review;
        self.save()?;
        if self.paused_seat(&lead) {
            return Ok(None);
        }
        self.release_review(tasks, index, entry, now).map(Some)
    }

    /// Put entry `entry`'s review task in the inbox: save the exact
    /// command, apply it, note it eligible, and mark it submitted, as
    /// [`Studio::reconcile`] releases a plan entry.
    fn release_review(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        entry: usize,
        now: u64,
    ) -> Result<Released, Error> {
        let review = self.state.goals[index].plan[entry]
            .flow
            .as_ref()
            .and_then(|flow| flow.review.clone())
            .ok_or_else(|| Error::State("the change has no review to release".into()))?;
        let seat = self
            .state
            .seat(&review.seat)
            .cloned()
            .ok_or_else(|| Error::UnknownSeat(review.seat.clone()))?;
        let bytes = match (&review.state, &review.command) {
            (SlotState::Releasing, Some(command)) => command.clone(),
            _ => {
                let prompt = self.review_prompt(&*tasks, index, entry, &seat);
                let goal = &self.state.goals[index];
                let title = one_line(&format!("Review: {}", goal.plan[entry].title), 256);
                let path = match &self.worktrees {
                    Some(dir) => git::prepare(
                        dir,
                        &self.store,
                        Path::new(&goal.repository.path),
                        &seat.name,
                        &review.task_id,
                        &title,
                        Some(seat.route.provider.as_str()),
                        None,
                    )
                    .map_err(io)?
                    .to_string_lossy()
                    .into_owned(),
                    None => goal.repository.path.clone(),
                };
                let command = Command {
                    schema: COMMAND_SCHEMA.into(),
                    command_id: format!("studio-{}", review.task_id),
                    task_id: review.task_id.clone(),
                    expected_revision: None,
                    action: Action::Submit {
                        intent: TaskIntent {
                            title,
                            prompt,
                            workspace: Workspace {
                                path,
                                source_revision: None,
                            },
                            configuration: RequestedConfiguration {
                                adapter: super::super::adapter::NAME.into(),
                                model: Some(seat.route.model.clone()),
                            },
                            images: Vec::new(),
                        },
                    },
                };
                let bytes = serde_json::to_string(&command)
                    .map_err(|_| Error::Corrupt("a studio command could not be encoded"))?;
                if let Some(slot) = self.flow_mut(index, entry).review.as_mut() {
                    slot.state = SlotState::Releasing;
                    slot.command = Some(bytes.clone());
                }
                self.save()?;
                bytes
            }
        };
        tasks.apply(bytes.as_bytes())?;
        let workspace = self.state.goals[index].repository.label.clone();
        if let Some(root) = &self.host_root {
            autostart::note_eligible(
                root,
                now,
                &review.task_id,
                DEVICE,
                &workspace,
                1,
                Some(seat.route.provider),
            )
            .map_err(io)?;
        }
        if let Some(slot) = self.flow_mut(index, entry).review.as_mut() {
            slot.state = SlotState::Submitted;
            slot.command = None;
        }
        self.save()?;
        Ok(Released {
            goal_id: self.state.goals[index].goal_id.clone(),
            task_id: review.task_id,
            seat: seat.name,
            workspace,
            provider: seat.route.provider,
        })
    }

    /// The lead's review prompt for entry `entry`: the goal and task, the
    /// check result and notes, the diff, and the task's history, within
    /// [`PROMPT_MAX`].
    fn review_prompt(&self, tasks: &dyn Inbox, index: usize, entry: usize, seat: &Seat) -> String {
        let goal = &self.state.goals[index];
        let item = &goal.plan[entry];
        let worker = &item.slot.seat;
        let mut prompt = format!(
            "You lead an Agent Studio team as seat `{}`. Review the change seat `{worker}` made \
             for one task of the goal below, then ask the person for the merge decision or \
             send changes back. Do not change files.\n\nGoal: {}\n\nTask `{}`: {}\n",
            seat.name, goal.text, item.id, item.title
        );
        if !item.description.is_empty() {
            prompt.push_str(&format!("{}\n", item.description));
        }
        if let Some(flow) = &item.flow {
            let checks = flow.verification.unwrap_or(Verification::NotRun);
            prompt.push_str(&format!("\nChecks: {}\n", checks.word()));
            let notes: Vec<String> = flow
                .notes
                .iter()
                .map(|note| format!("- {note}\n"))
                .collect();
            let notes = newest_within(&notes, NOTES_IN_PROMPT);
            if !notes.is_empty() {
                prompt.push_str("Notes:\n");
                prompt.push_str(&notes.concat());
            }
        }
        match self.change_of(&item.slot.task_id) {
            Some(change) => {
                let cut_short = if change.completeness == Completeness::Complete {
                    ""
                } else {
                    " The diff below is cut short."
                };
                prompt.push_str(&format!(
                    "\nThe change from {} to {}: {} file(s), +{} -{}. Read it with `git diff {} {}`; \
                     work it left uncommitted shows only below.{cut_short}\n```diff\n{}\n```\n",
                    cut(&change.base, 12),
                    cut(&change.head_commit, 12),
                    change.files_total,
                    change.added,
                    change.removed,
                    change.base,
                    change.head_commit,
                    cut(&change.diff, DIFF_MAX),
                ));
            }
            None => prompt.push_str(
                "\nThe change could not be read here; read the task's worktree before you \
                 decide.\n",
            ),
        }
        if let Some(task) = tasks.task(&item.slot.task_id) {
            prompt.push_str(&format!(
                "\nHistory: seat `{worker}` took {} turn(s) on it.\n",
                task.turn()
            ));
            let lines: Vec<String> = task
                .follow_ups
                .iter()
                .map(|follow_up| {
                    let words: Vec<&str> = follow_up.prompt.split_whitespace().collect();
                    format!("- {}\n", cut(&words.join(" "), HISTORY_LINE_MAX))
                })
                .collect();
            let kept = newest_within(&lines, HISTORY_MAX);
            if !kept.is_empty() {
                prompt.push_str("What it was told after its first turn, oldest first:\n");
                prompt.push_str(&kept.concat());
            }
        }
        let instructions = format!(
            "\nEnd your reply with your verdict as one fenced JSON block:\n```json\n\
             {{\"schema\":\"{REVIEW_SCHEMA}\",\"verdict\":\"approve\",\"notes\":\"What the \
             person should know\"}}\n```\nverdict is approve to ask the person for the merge \
             decision, or changes to send your notes back to seat `{worker}` as what to change."
        );
        // The verdict's instructions always fit; the body gives way.
        let body = clean(&prompt, PROMPT_MAX - instructions.len());
        format!("{body}\n{instructions}\n")
    }

    /// Read the verdict of entry `entry`'s review task once it ended.
    fn advance_review(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        entry: usize,
        now: u64,
        reply: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Option<Released>, Error> {
        let item = &self.state.goals[index].plan[entry];
        let goal_id = self.state.goals[index].goal_id.clone();
        let id = item.id.clone();
        let Some(flow) = item.flow.clone() else {
            return Ok(None);
        };
        let Some(review) = flow.review.clone() else {
            self.set_stage(index, entry, Stage::Merge)?;
            return Ok(None);
        };
        if review.state != SlotState::Submitted {
            if review.state == SlotState::Held && self.paused_seat(&review.seat) {
                return Ok(None);
            }
            return self.release_review(tasks, index, entry, now).map(Some);
        }
        let verdict = match progress(&*tasks, &review.task_id) {
            Progress::Done => reply(&review.task_id).as_deref().and_then(review_in_reply),
            Progress::Waiting | Progress::Failed | Progress::Cancelled | Progress::Missing => None,
            _ => return Ok(None),
        };
        let lead = Party::Seat {
            name: review.seat.clone(),
        };
        match verdict {
            Some(ReviewVerdict {
                verdict: Verdict::Changes,
                notes,
                ..
            }) if flow.changes < MAX_CHANGE_ROUNDS => {
                let notes = match clean(&notes, NOTE_MAX) {
                    empty if empty.is_empty() => "The lead asked for changes without naming them; \
                                                  read the review and improve the change."
                        .to_owned(),
                    notes => notes,
                };
                let Some(task) = tasks.task(&flow.task_id) else {
                    self.note(index, entry, format!("The lead asked for changes: {notes}"));
                    self.set_stage(index, entry, Stage::Merge)?;
                    return Ok(None);
                };
                self.push_memory(
                    MemoryKind::Decision,
                    lead,
                    Some(goal_id),
                    format!("The lead asked for changes to task `{id}`: {notes}"),
                );
                let prompt = format!(
                    "The lead reviewed your change and asks for changes:\n\n{notes}\n\nMake them \
                     in this worktree, then commit. Do not push."
                );
                self.follow_up(
                    tasks,
                    index,
                    entry,
                    &task,
                    "changes",
                    &prompt,
                    now,
                    |flow| {
                        flow.changes += 1;
                        flow.fixes = 0;
                        flow.stage = Stage::Work;
                        flow.review = None;
                        flow.notes.clear();
                        flow.verification = None;
                    },
                )?;
            }
            Some(ReviewVerdict {
                verdict: Verdict::Changes,
                notes,
                ..
            }) => {
                self.note(
                    index,
                    entry,
                    format!(
                        "The lead still asks for changes after {MAX_CHANGE_ROUNDS} rounds: {notes}"
                    ),
                );
                self.set_stage(index, entry, Stage::Merge)?;
            }
            Some(ReviewVerdict {
                verdict: Verdict::Approve,
                notes,
                ..
            }) => {
                let notes = clean(&notes, NOTE_MAX);
                self.note(
                    index,
                    entry,
                    if notes.is_empty() {
                        "The lead approved the change.".to_owned()
                    } else {
                        format!("The lead approved the change: {notes}")
                    },
                );
                self.push_memory(
                    MemoryKind::Decision,
                    lead,
                    Some(goal_id),
                    format!("The lead approved task `{id}`'s change for the merge decision."),
                );
                self.set_stage(index, entry, Stage::Merge)?;
            }
            None => {
                self.note(
                    index,
                    entry,
                    "The lead's review ended without a verdict.".to_owned(),
                );
                self.set_stage(index, entry, Stage::Merge)?;
            }
        }
        Ok(None)
    }

    /// A merge of entry `entry`'s change conflicted: send it back to its
    /// worker, unless it went back [`MAX_CONFLICT_ROUNDS`] times already.
    /// Returns the stage it is at now.
    fn on_conflict(
        &mut self,
        index: usize,
        entry: usize,
        conflict: Conflict,
    ) -> Result<Stage, Error> {
        let task_id = self.state.goals[index].plan[entry].slot.task_id.clone();
        let flow = self.flow_mut(index, entry);
        let stage = if flow.conflicts >= MAX_CONFLICT_ROUNDS {
            self.note(
                index,
                entry,
                format!(
                    "Merging into {} still conflicts after {MAX_CONFLICT_ROUNDS} rounds; resolve \
                     it by hand.",
                    conflict.target
                ),
            );
            Stage::Merge
        } else {
            flow.conflicts += 1;
            flow.conflict = Some(conflict);
            flow.stage = Stage::Conflict;
            Stage::Conflict
        };
        self.save()?;
        forget_conflict(&self.dir, &task_id);
        Ok(stage)
    }

    /// Send entry `entry`'s worker to merge the branch its change
    /// conflicted with into its own and resolve it.
    fn conflict_follow_up(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        entry: usize,
        task: &Task,
        now: u64,
    ) -> Result<(), Error> {
        let goal_id = self.state.goals[index].goal_id.clone();
        let item = &self.state.goals[index].plan[entry];
        let (id, seat) = (item.id.clone(), item.slot.seat.clone());
        let conflict = item.flow.as_ref().and_then(|flow| flow.conflict.clone());
        let (target, files) = match &conflict {
            Some(conflict) => (conflict.target.clone(), conflict.files.join(", ")),
            None => ("the branch it merges into".to_owned(), String::new()),
        };
        let place = if files.is_empty() {
            String::new()
        } else {
            format!(" in {files}")
        };
        // The host starts the merge: the seat's run cannot write the common
        // Git directory. The seat resolves the markers by editing files.
        let prompt = match git::resolve_conflict(&self.store, &task.task_id, &target) {
            Ok(files) => {
                let place = if files.is_empty() {
                    place.clone()
                } else {
                    format!(" in {}", files.join(", "))
                };
                format!(
                    "Merging your change into `{target}` would conflict. The host has merged \
                     `{target}` into this worktree, and Git left conflict markers{place}. Resolve \
                     them by editing the files, run the checks, and leave the result in the \
                     working tree: the host commits the merge. Do not run Git commands that write."
                )
            }
            Err(why) => format!(
                "Merging your change into `{target}` would conflict{place}, and the host could \
                 not start the merge in this worktree ({why}). Explain what you see; do not run \
                 Git commands that write."
            ),
        };
        self.push_memory(
            MemoryKind::Note,
            Party::Seat { name: seat.clone() },
            Some(goal_id),
            format!(
                "Merging task `{id}` into {target} conflicted{place}; it went back to @{seat} to \
                 merge {target} in."
            ),
        );
        self.follow_up(
            tasks,
            index,
            entry,
            task,
            "conflict",
            &prompt,
            now,
            |flow| {
                flow.stage = Stage::Work;
                flow.conflict = None;
                flow.fixes = 0;
                flow.notes.clear();
                flow.verification = None;
            },
        )
    }

    /// Start the next turn of entry `entry`'s task with `prompt`, as a
    /// person's follow-up does: change the flow with `update` and save the
    /// exact command in one write, then apply it.
    #[allow(clippy::too_many_arguments)]
    fn follow_up(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        entry: usize,
        task: &Task,
        kind: &str,
        prompt: &str,
        now: u64,
        update: impl FnOnce(&mut Flow),
    ) -> Result<(), Error> {
        let command = Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: format!("studio-{kind}-{}-{}", task.task_id, task.revision),
            task_id: task.task_id.clone(),
            expected_revision: Some(task.revision),
            action: Action::Continue {
                prompt: clean(prompt, PROMPT_MAX),
            },
        };
        let bytes = serde_json::to_string(&command)
            .map_err(|_| Error::Corrupt("a studio command could not be encoded"))?;
        let flow = self.flow_mut(index, entry);
        update(&mut *flow);
        flow.command = Some(bytes.clone());
        self.save()?;
        self.send_follow_up(tasks, index, entry, &bytes, now)
    }

    /// Apply entry `entry`'s saved follow-up and note the turn eligible. A
    /// follow-up the inbox refuses for good, because the task moved on,
    /// sends the change to the person's merge decision with the reason.
    fn send_follow_up(
        &mut self,
        tasks: &mut dyn Inbox,
        index: usize,
        entry: usize,
        bytes: &str,
        now: u64,
    ) -> Result<(), Error> {
        use super::super::Error as Refused;
        let slot = self.state.goals[index].plan[entry].slot.clone();
        match tasks.apply(bytes.as_bytes()) {
            Ok(()) => {
                if let Some(root) = &self.host_root
                    && let Some(task) = tasks.task(&slot.task_id)
                {
                    let provider = self.state.seat(&slot.seat).map(|seat| seat.route.provider);
                    autostart::note_eligible(
                        root,
                        now,
                        &slot.task_id,
                        DEVICE,
                        &self.state.goals[index].repository.label,
                        task.turn_started(),
                        provider,
                    )
                    .map_err(io)?;
                }
            }
            Err(
                error @ (Refused::Conflict
                | Refused::RevisionMismatch
                | Refused::NotFound
                | Refused::InvalidTransition
                | Refused::LimitExceeded
                | Refused::InvalidCommand(_)),
            ) => {
                self.note(
                    index,
                    entry,
                    format!("A follow-up to the worker could not be sent: {error}"),
                );
                self.flow_mut(index, entry).stage = Stage::Merge;
            }
            Err(error) => return Err(error.into()),
        }
        self.flow_mut(index, entry).command = None;
        self.save()
    }
}

#[cfg(test)]
#[path = "studio_flow_tests.rs"]
mod tests;
