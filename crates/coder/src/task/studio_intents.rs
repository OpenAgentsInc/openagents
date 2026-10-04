//! The studio's NIP-HOST intents and the view a device draws
//! (`docs/verse/agent-studio.md`, "The client is a view"; the wire form is
//! `coder_access::studio`).
//!
//! The host checks each intent's right before it reaches here. An intent
//! steers the coordinator through the paths it already has, never around
//! them:
//!
//! - **Pause** keeps a seat's task and holds back new ones: a paused
//!   seat's planned tasks and held lead stay held until it resumes. The
//!   paused set is a file of its own beside the studio document.
//! - **Stop** pauses the seat, cancels its queued or running task in the
//!   inbox, and returns that task to the board as planned, under a new
//!   task identity (`<task>.r2`, `.r3`, ...).
//! - **Reassign** and **prioritize** change a planned task that has not
//!   started. **Cancel** cancels a queued or running task in the inbox; a
//!   planned one goes into the inbox without being noted eligible, so no
//!   policy starts it, and is cancelled there, so its dependents see it
//!   cancelled. **Retry** plans a failed or cancelled task again under a
//!   new identity and releases it when its dependencies allow.
//! - **Answer** to a goal's plan decision delivers a plan
//!   ([`Studio::accept_plan`]). A task's question or approval is answered
//!   by the host through the existing `answer` command, not here.
//! - **Reject** records the person's decision in shared memory and
//!   cancels the task if it still runs; its worktree stays for inspection.
//!
//! Each intent answers once per NIP-HOST request ID: the reference it
//! answered is kept in a bounded ledger beside the studio document, so a
//! retry after an uncertain reply answers it again instead of repeating it.
//!
//! [`Studio::wire`] joins the coordinator with the inbox into the device's
//! view. A seat's activity and station come from its running task's newest
//! ATIF step through [`atif::classify`], the classifier a replay shares.
//! Log lines are display text: an activity and a tool's name with the
//! purpose the surface showed, or the first line of what the agent said.
//! A call's arguments and output never leave the host.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use coder_host::access::studio as wire;
use serde::{Deserialize, Serialize};

use super::super::{Action, COMMAND_SCHEMA, Command, interaction};
use super::{
    DecisionKind, Error, GoalStatus, Inbox, LOCK_FILE, MAX_MEMORY_BYTES, MemoryKind, Party,
    PlanOutcome, Progress, Released, Role, SlotState, Studio, cut, progress,
};

/// The paused seats, beside the studio document.
const PAUSED_FILE: &str = "paused.json";
const PAUSED_SCHEMA: &str = "openagents.coder.studio-paused.v1";
/// The intents already answered, by NIP-HOST request ID.
const INTENTS_FILE: &str = "intents.json";
const INTENTS_SCHEMA: &str = "openagents.coder.studio-intents.v1";
/// The most answered intents the ledger keeps; the oldest go first. A
/// request lives at most 60 seconds, so this is far more than a retry
/// needs.
const MAX_INTENTS: usize = 256;
/// The most times one plan entry is returned to the board or retried.
const MAX_ATTEMPTS: u32 = 99;
/// The largest sidecar file the coordinator reads.
const MAX_SIDECAR_BYTES: u64 = 256 * 1024;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Paused {
    schema: String,
    seats: BTreeSet<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Answered {
    schema: String,
    /// `(request ID, reference)`, oldest first.
    intents: Vec<(String, String)>,
}

/// Read a sidecar document, or its default when it is absent or does not
/// read: a damaged sidecar loses a pause or a retry's memo, never a task.
fn read<T: Default + for<'de> Deserialize<'de>>(dir: &Path, name: &str) -> T {
    let path = dir.join(name);
    let fits = std::fs::metadata(&path).is_ok_and(|meta| meta.len() <= MAX_SIDECAR_BYTES);
    if !fits || !super::super::regular_or_absent(&path).unwrap_or(false) {
        return T::default();
    }
    std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// The identity and attempt number a returned or retried task takes after
/// `slot`. Host access names tasks by lower-case 32-byte hex, so the next
/// identity is a digest of the current one and the next attempt number,
/// which keeps it the same across restarts and distinct from every other
/// slot's.
fn next_id(slot: &super::Slot) -> Result<(String, u32), Error> {
    use sha2::{Digest, Sha256};
    let attempt = slot.attempt.max(1);
    if attempt >= MAX_ATTEMPTS {
        return Err(Error::LimitExceeded("attempts at one task"));
    }
    let next = attempt + 1;
    let digest = Sha256::new()
        .chain_update(b"openagents.studio.retry.v1\0")
        .chain_update(slot.task_id.as_bytes())
        .chain_update(b"\0")
        .chain_update(next.to_string().as_bytes())
        .finalize();
    let id = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok((id, next))
}

/// Cancel `task_id` in the inbox at its current revision.
fn cancel(tasks: &mut dyn Inbox, task_id: &str, reason: &str) -> Result<(), Error> {
    let task = tasks
        .task(task_id)
        .ok_or(Error::Tasks(super::super::Error::NotFound))?;
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: format!("studio-cancel-{task_id}"),
        task_id: task_id.into(),
        expected_revision: Some(task.revision),
        action: Action::Cancel {
            reason: reason.into(),
        },
    };
    let bytes = serde_json::to_vec(&command)
        .map_err(|_| Error::Corrupt("a studio command could not be encoded"))?;
    tasks.apply(&bytes)?;
    Ok(())
}

impl Studio {
    /// The seats a person paused.
    #[must_use]
    pub fn paused(&self) -> BTreeSet<String> {
        read::<Paused>(&self.dir, PAUSED_FILE).seats
    }

    /// Whether seat `name` is paused.
    pub(super) fn paused_seat(&self, name: &str) -> bool {
        self.paused().contains(name)
    }

    fn write_sidecar(&self, name: &str, value: &impl Serialize) -> Result<(), Error> {
        super::super::verify_same_file(&self.dir.join(LOCK_FILE), &self.lock)?;
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|_| Error::Corrupt("a studio file could not be encoded"))?;
        super::super::replace_file(&self.dir, name, &bytes)?;
        Ok(())
    }

    fn set_paused(&mut self, name: &str, paused: bool) -> Result<(), Error> {
        if self.state.seat(name).is_none() {
            return Err(Error::UnknownSeat(name.into()));
        }
        let mut seats = self.paused();
        let changed = if paused {
            seats.insert(name.to_owned())
        } else {
            seats.remove(name)
        };
        if changed {
            self.write_sidecar(
                PAUSED_FILE,
                &Paused {
                    schema: PAUSED_SCHEMA.into(),
                    seats,
                },
            )?;
            self.save()?;
        }
        Ok(())
    }

    /// The reference the intent under request ID `key` answered, if it
    /// was answered.
    #[must_use]
    pub fn answered(&self, key: &str) -> Option<String> {
        read::<Answered>(&self.dir, INTENTS_FILE)
            .intents
            .into_iter()
            .find(|(request, _)| request == key)
            .map(|(_, reference)| reference)
    }

    /// Keep that the intent under request ID `key` answered `reference`.
    ///
    /// # Errors
    /// The ledger cannot be written.
    pub fn record_answer(&mut self, key: &str, reference: &str) -> Result<(), Error> {
        let mut ledger = read::<Answered>(&self.dir, INTENTS_FILE);
        ledger.schema = INTENTS_SCHEMA.into();
        ledger.intents.retain(|(request, _)| request != key);
        ledger.intents.push((key.to_owned(), reference.to_owned()));
        let excess = ledger.intents.len().saturating_sub(MAX_INTENTS);
        ledger.intents.drain(..excess);
        self.write_sidecar(INTENTS_FILE, &ledger)
    }

    /// Where the studio task `task_id` is: its goal, and its plan entry
    /// (`None` for the lead).
    fn locate(&self, task_id: &str) -> Result<(usize, Option<usize>), Error> {
        for (index, goal) in self.state.goals.iter().enumerate() {
            if goal.lead.task_id == task_id {
                return Ok((index, None));
            }
            if let Some(entry) = goal
                .plan
                .iter()
                .position(|entry| entry.slot.task_id == task_id)
            {
                return Ok((index, Some(entry)));
            }
        }
        Err(Error::Tasks(super::super::Error::NotFound))
    }

    /// Whether `task_id` is one of the studio's tasks.
    #[must_use]
    pub fn holds_task(&self, task_id: &str) -> bool {
        self.locate(task_id).is_ok()
    }

    /// Pause seat `name`: it keeps its task and takes no new one.
    ///
    /// # Errors
    /// No such seat, or the studio cannot be written.
    pub fn pause_seat(&mut self, name: &str) -> Result<(), Error> {
        self.set_paused(name, true)
    }

    /// Resume seat `name` and release what its pause held back.
    ///
    /// # Errors
    /// No such seat, or the inbox or the studio cannot be written.
    pub fn resume_seat(
        &mut self,
        tasks: &mut dyn Inbox,
        name: &str,
        now: u64,
    ) -> Result<Vec<Released>, Error> {
        self.set_paused(name, false)?;
        self.release_waiting(tasks, now)
    }

    /// Release every held lead and ready plan entry whose seat is not
    /// paused, as [`Studio::reconcile`] does without reading replies.
    fn release_waiting(&mut self, tasks: &mut dyn Inbox, now: u64) -> Result<Vec<Released>, Error> {
        let mut released = Vec::new();
        for index in 0..self.state.goals.len() {
            let goal = &self.state.goals[index];
            if goal.lead.state != SlotState::Submitted {
                if !self.paused_seat(&goal.lead.seat) {
                    released.push(self.release(tasks, index, None, now)?);
                }
                continue;
            }
            if goal.planned {
                released.extend(self.release_ready(tasks, index, now)?);
            }
        }
        Ok(released)
    }

    /// Stop seat `name`: pause it, cancel its queued or running tasks,
    /// and return each of its active tasks to the board as planned under
    /// a new identity. Returns the new identities.
    ///
    /// # Errors
    /// No such seat, or the inbox or the studio cannot be written.
    pub fn stop_seat(&mut self, tasks: &mut dyn Inbox, name: &str) -> Result<Vec<String>, Error> {
        self.set_paused(name, true)?;
        let mut slots = Vec::new();
        for (index, goal) in self.state.goals.iter().enumerate() {
            let entries = std::iter::once(None).chain((0..goal.plan.len()).map(Some));
            for entry in entries {
                let slot = self.slot(index, entry);
                if slot.seat == name && slot.state == SlotState::Submitted {
                    slots.push((index, entry, slot.task_id.clone()));
                }
            }
        }
        let mut returned = Vec::new();
        for (index, entry, task_id) in slots {
            let now = progress(tasks, &task_id);
            if !now.is_active() {
                continue;
            }
            if matches!(now, Progress::Queued | Progress::Running) {
                cancel(tasks, &task_id, "Stopped in the studio")?;
            }
            let (fresh, attempt) = next_id(self.slot(index, entry))?;
            let slot = self.slot_mut(index, entry);
            slot.task_id = fresh.clone();
            slot.attempt = attempt;
            slot.state = SlotState::Held;
            slot.command = None;
            returned.push(fresh);
        }
        self.save()?;
        Ok(returned)
    }

    /// Give the planned task `task_id`, which has not started, to seat
    /// `seat`. A lead goes to a lead seat only.
    ///
    /// # Errors
    /// No such task or seat, the task started, or a lead's seat is not a
    /// lead.
    pub fn reassign(&mut self, task_id: &str, seat: &str) -> Result<(), Error> {
        let (index, entry) = self.locate(task_id)?;
        let target = self
            .state
            .seat(seat)
            .ok_or_else(|| Error::UnknownSeat(seat.into()))?;
        if entry.is_none() && target.role != Role::Lead {
            return Err(Error::Invalid(format!("seat `{seat}` is not a lead")));
        }
        if self.slot(index, entry).state != SlotState::Held {
            return Err(Error::State(format!(
                "task `{task_id}` has started; cancel it, retry it, then reassign it"
            )));
        }
        self.slot_mut(index, entry).seat = seat.into();
        self.save()
    }

    /// Move the planned task `task_id`, which has not started, ahead of
    /// its goal's other plan entries: it is released first when several
    /// become ready together, and drawn first on the board.
    ///
    /// # Errors
    /// No such task, it is a lead, or it started.
    pub fn prioritize(&mut self, task_id: &str) -> Result<(), Error> {
        let (index, entry) = self.locate(task_id)?;
        let Some(entry) = entry else {
            return Err(Error::Invalid("a goal's lead is not on its board".into()));
        };
        if self.state.goals[index].plan[entry].slot.state != SlotState::Held {
            return Err(Error::State(format!("task `{task_id}` has started")));
        }
        let item = self.state.goals[index].plan.remove(entry);
        self.state.goals[index].plan.insert(0, item);
        self.save()
    }

    /// Cancel the studio task `task_id`: a queued or running one in the
    /// inbox, and a planned one by putting it in the inbox without noting
    /// it eligible, so no policy starts it, and cancelling it there.
    ///
    /// # Errors
    /// No such task, it waits on a decision or is over, or the inbox
    /// refuses.
    pub fn cancel_task(
        &mut self,
        tasks: &mut dyn Inbox,
        task_id: &str,
        now: u64,
    ) -> Result<(), Error> {
        let (index, entry) = self.locate(task_id)?;
        let slot = self.slot(index, entry).clone();
        match slot.state {
            SlotState::Held | SlotState::Releasing => {
                let root = self.host_root.take();
                let released = self.release(tasks, index, entry, now);
                self.host_root = root;
                released?;
            }
            SlotState::Submitted => match progress(tasks, &slot.task_id) {
                Progress::Queued | Progress::Running => {}
                Progress::Waiting => {
                    return Err(Error::State(format!(
                        "task `{task_id}` waits on a decision; answer it, or reject its change"
                    )));
                }
                _ => return Err(Error::State(format!("task `{task_id}` is already over"))),
            },
        }
        cancel(tasks, &slot.task_id, "Cancelled in the studio")
    }

    /// Plan the failed or cancelled studio task `task_id` again under a
    /// new identity, clear the decision its failure opened, and release
    /// what is ready. Returns the new identity.
    ///
    /// # Errors
    /// No such task, it did not fail, or the inbox or the studio cannot be
    /// written.
    pub fn retry_task(
        &mut self,
        tasks: &mut dyn Inbox,
        task_id: &str,
        now: u64,
    ) -> Result<String, Error> {
        let (index, entry) = self.locate(task_id)?;
        let slot = self.slot(index, entry).clone();
        let over = slot.state == SlotState::Submitted
            && matches!(
                progress(tasks, &slot.task_id),
                Progress::Failed | Progress::Cancelled | Progress::Missing
            );
        if !over {
            return Err(Error::State(format!(
                "only a failed or cancelled task is retried; `{task_id}` is not"
            )));
        }
        let (fresh, attempt) = next_id(&slot)?;
        let target = self.slot_mut(index, entry);
        target.task_id = fresh.clone();
        target.attempt = attempt;
        target.state = SlotState::Held;
        target.command = None;
        let goal = &mut self.state.goals[index];
        let opened = goal.decision.as_ref().is_some_and(|decision| match entry {
            None => decision.kind == DecisionKind::LeadFailed,
            Some(_) => decision.kind == DecisionKind::DependencyFailed,
        });
        if opened {
            goal.decision = None;
        }
        self.save()?;
        self.release_waiting(tasks, now)?;
        Ok(fresh)
    }

    /// Answer goal `goal_id`'s plan decision with `plan`. `based_on` is
    /// the decision's sequence; another refuses as stale.
    ///
    /// # Errors
    /// No such goal, no plan decision waits, the decision moved on, or the
    /// inbox or the studio cannot be written.
    pub fn answer_goal(
        &mut self,
        tasks: &mut dyn Inbox,
        goal_id: &str,
        based_on: u64,
        plan: &str,
        now: u64,
    ) -> Result<PlanOutcome, Error> {
        let index = self.goal_index(goal_id)?;
        let goal = &self.state.goals[index];
        let Some(decision) = &goal.decision else {
            return Err(Error::State(format!(
                "goal `{goal_id}` waits on no decision"
            )));
        };
        if decision.sequence != based_on {
            return Err(Error::Tasks(super::super::Error::RevisionMismatch));
        }
        if goal.planned || decision.kind == DecisionKind::DependencyFailed {
            return Err(Error::State(
                "retry or cancel the task that did not finish".into(),
            ));
        }
        self.accept_plan(tasks, goal_id, plan.as_bytes(), now)
    }

    /// Record that the person rejected the studio task `task_id`'s change
    /// at content tree `head`, with an optional `reason`, and cancel the
    /// task if it still runs. Its worktree stays until it is archived.
    /// Returns the memory entry's sequence.
    ///
    /// # Errors
    /// No such task, or the inbox or the studio cannot be written.
    pub fn reject(
        &mut self,
        tasks: &mut dyn Inbox,
        task_id: &str,
        head: &str,
        reason: &str,
    ) -> Result<u64, Error> {
        let (index, _) = self.locate(task_id)?;
        if matches!(
            progress(tasks, task_id),
            Progress::Queued | Progress::Running
        ) {
            cancel(tasks, task_id, "Rejected in the studio")?;
        }
        let goal_id = self.state.goals[index].goal_id.clone();
        let mut text = format!(
            "The person rejected the change of task {task_id} at tree {}.",
            cut(head, 12)
        );
        if !reason.trim().is_empty() {
            text.push_str(" Reason: ");
            text.push_str(reason.trim());
        }
        self.remember(
            MemoryKind::Decision,
            Party::Person,
            Some(&goal_id),
            &cut(&text, MAX_MEMORY_BYTES),
        )
    }

    /// The studio as a device draws it: goals, seats with their activity,
    /// tasks, open decisions, repository summaries, and each seat's log
    /// tail. `store` is the task store, where runs keep their traces.
    #[must_use]
    pub fn wire(&self, tasks: &dyn Inbox, store: &Path) -> wire::View {
        let view = self.view(tasks);
        let paused = self.paused();
        // The newest goals that fit the bounds.
        let mut budget = wire::MAX_TASKS;
        let goals: Vec<&super::GoalView> = view
            .goals
            .iter()
            .rev()
            .take(wire::MAX_GOALS)
            .take_while(|goal| {
                let needed = 1 + goal.entries.len();
                let fits = needed <= budget;
                budget = budget.saturating_sub(needed);
                fits
            })
            .collect();
        let mut out = wire::View::default();
        for goal in goals {
            let submitted_at = self
                .state
                .goal(&goal.goal_id)
                .map_or(0, |found| found.submitted_at);
            out.goals.push(wire::Goal {
                goal: goal.goal_id.clone(),
                text: clean(&goal.text, wire::MAX_GOAL_TEXT, &goal.goal_id),
                workspace: goal.workspace.clone(),
                lead: goal.lead_seat.clone(),
                status: match goal.status {
                    GoalStatus::Planning => wire::GoalStatus::Planning,
                    GoalStatus::Decision => wire::GoalStatus::Decision,
                    GoalStatus::Running => wire::GoalStatus::Running,
                    GoalStatus::Done => wire::GoalStatus::Done,
                },
                final_tasks: u32::try_from(goal.final_tasks).unwrap_or(u32::MAX),
                total_tasks: u32::try_from(goal.total_tasks).unwrap_or(u32::MAX),
                submitted_at,
            });
            out.tasks.push(wire::Task {
                task: goal.lead_task_id.clone(),
                goal: goal.goal_id.clone(),
                entry: "lead".into(),
                position: 0,
                title: title(&format!("Plan: {}", goal.text), "Plan"),
                seat: goal.lead_seat.clone(),
                depends_on: Vec::new(),
                status: status(goal.lead_progress),
            });
            for (position, entry) in goal.entries.iter().enumerate() {
                out.tasks.push(wire::Task {
                    task: entry.task_id.clone(),
                    goal: goal.goal_id.clone(),
                    entry: entry.id.clone(),
                    position: u32::try_from(position + 1).unwrap_or(u32::MAX),
                    title: title(&entry.title, &entry.id),
                    seat: entry.seat.clone(),
                    depends_on: entry.depends_on.clone(),
                    status: status(entry.progress),
                });
            }
            if let Some(decision) = &goal.decision {
                let kind = match decision.kind {
                    DecisionKind::InvalidPlan => wire::DecisionKind::InvalidPlan,
                    DecisionKind::NoPlan => wire::DecisionKind::NoPlan,
                    DecisionKind::LeadFailed => wire::DecisionKind::LeadFailed,
                    DecisionKind::DependencyFailed => wire::DecisionKind::DependencyFailed,
                };
                out.decisions.push(wire::Decision {
                    decision: goal.goal_id.clone(),
                    goal: goal.goal_id.clone(),
                    task: None,
                    seat: Some(goal.lead_seat.clone()),
                    kind,
                    text: clean(
                        &decision.reasons.join("\n"),
                        wire::MAX_DECISION_TEXT,
                        "The goal needs a decision.",
                    ),
                    based_on: decision.sequence,
                });
            }
        }
        // A waiting task's question or approval: its turn's reply, which a
        // device with `observe` reads in the task's transcript anyway.
        for task in &out.tasks {
            if task.status != wire::TaskStatus::Waiting {
                continue;
            }
            let Some(record) = tasks.task(&task.task) else {
                continue;
            };
            let (kind, fallback) = match interaction::pending(&record) {
                Some(interaction::Kind::Approval) => (
                    wire::DecisionKind::Approval,
                    "The task asks to approve a step.",
                ),
                _ => (wire::DecisionKind::Question, "The task asks a question."),
            };
            let asked = super::super::local::result_in(Some(store), &task.task)
                .map(|run| run.summary)
                .unwrap_or_default();
            out.decisions.push(wire::Decision {
                decision: task.task.clone(),
                goal: task.goal.clone(),
                task: Some(task.task.clone()),
                seat: Some(task.seat.clone()),
                kind,
                text: clean(&asked, wire::MAX_DECISION_TEXT, fallback),
                based_on: record.revision,
            });
        }
        out.decisions.truncate(wire::MAX_DECISIONS);
        for seat in view.seats.iter().take(wire::MAX_SEATS) {
            let name = &seat.seat.name;
            let shown = seat.task_id.clone().or_else(|| self.latest_task(name));
            let steps = shown
                .as_deref()
                .map(|task| trace(tasks, store, task))
                .unwrap_or_default();
            let (activity, station) = if paused.contains(name) {
                (wire::Activity::Paused, wire::Station::Lounge)
            } else {
                match seat.progress {
                    Some(Progress::Running) => steps.last().map_or(
                        (wire::Activity::Thinking, wire::Station::Desk),
                        |step| {
                            let classified = atif::classify(step);
                            (activity(classified.activity), place(classified.station))
                        },
                    ),
                    Some(Progress::Waiting) => (wire::Activity::Waiting, wire::Station::Podium),
                    _ => (wire::Activity::Idle, wire::Station::Desk),
                }
            };
            out.seats.push(wire::Seat {
                seat: name.clone(),
                role: match seat.seat.role {
                    Role::Lead => wire::Role::Lead,
                    Role::Worker => wire::Role::Worker,
                },
                route: seat.seat.route.to_string(),
                look: seat.seat.look.clone(),
                desk: seat.seat.desk,
                activity,
                station,
                task: seat.task_id.clone(),
                paused: paused.contains(name),
            });
            if let Some(task) = shown {
                let first = steps.len().saturating_sub(wire::MAX_LOG_LINES * 4);
                let mut lines: Vec<wire::LogLine> =
                    steps[first..].iter().filter_map(disclose).collect();
                let excess = lines.len().saturating_sub(wire::MAX_LOG_LINES);
                lines.drain(..excess);
                out.logs.push(wire::Log {
                    seat: name.clone(),
                    task: Some(task),
                    lines,
                });
            }
        }
        let mut repositories: BTreeMap<String, (u32, u32)> = BTreeMap::new();
        for goal in &out.goals {
            repositories.entry(goal.workspace.clone()).or_default().0 += 1;
        }
        for task in &out.tasks {
            let open = matches!(
                task.status,
                wire::TaskStatus::Queued | wire::TaskStatus::Running | wire::TaskStatus::Waiting
            );
            if open && let Some(goal) = out.goals.iter().find(|goal| goal.goal == task.goal) {
                repositories.entry(goal.workspace.clone()).or_default().1 += 1;
            }
        }
        out.repositories = repositories
            .into_iter()
            .map(|(workspace, (goals, open_tasks))| wire::Repository {
                workspace,
                goals,
                open_tasks,
            })
            .collect();
        out.canonicalize();
        out
    }

    /// The seat's newest task in the inbox, active or not.
    fn latest_task(&self, seat: &str) -> Option<String> {
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
            .find(|slot| slot.seat == seat && slot.state == SlotState::Submitted)
            .map(|slot| slot.task_id.clone())
    }
}

/// The steps of `task_id`'s current run's trace, oldest first; none when
/// it has no run or the trace does not read.
fn trace(tasks: &dyn Inbox, store: &Path, task_id: &str) -> Vec<atif::Step> {
    let Some(run) = tasks.task(task_id).and_then(|task| task.run) else {
        return Vec::new();
    };
    atif::log::read(&store.join(&run.admission.trace_file))
        .map(|recording| recording.steps)
        .unwrap_or_default()
}

/// A step as a log line, under the disclosure rule: a call shows its
/// activity, its tool's name, and the purpose the surface showed beside
/// it; an agent's message shows its first line. A call's arguments and
/// output, and the person's and host's messages, are never shown.
fn disclose(step: &atif::Step) -> Option<wire::LogLine> {
    let classified = atif::classify(step);
    let text = match &step.call {
        Some(call) => {
            let name = wire::first_line(&call.name, 48);
            let name = if name.is_empty() {
                "a tool".into()
            } else {
                name
            };
            let mut text = format!("{}: {name}", classified.activity.word());
            if let Some(purpose) = call.purpose.as_deref() {
                let purpose = wire::first_line(purpose, wire::MAX_LINE);
                if !purpose.is_empty() {
                    text.push_str(" — ");
                    text.push_str(&purpose);
                }
            }
            text
        }
        None if step.source == atif::Source::Agent => {
            wire::first_line(&step.message, wire::MAX_LINE)
        }
        None => String::new(),
    };
    let text = wire::first_line(&text, wire::MAX_LINE);
    (!text.is_empty()).then(|| wire::LogLine {
        at: step.at,
        activity: activity(classified.activity),
        text,
    })
}

/// Display text of several lines: control characters other than line
/// breaks and tabs dropped, at most `max` bytes, or `fallback` when
/// nothing is left.
fn clean(text: &str, max: usize, fallback: &str) -> String {
    let kept: String = text
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
        .collect();
    let kept = wire::cut(kept.trim(), max);
    if kept.trim().is_empty() {
        wire::cut(fallback, max)
    } else {
        kept
    }
}

/// A one-line title of at most [`wire::MAX_TITLE`] bytes, or `fallback`.
fn title(text: &str, fallback: &str) -> String {
    let line = wire::first_line(text, wire::MAX_TITLE);
    if line.is_empty() {
        wire::first_line(fallback, wire::MAX_TITLE)
    } else {
        line
    }
}

fn status(progress: Progress) -> wire::TaskStatus {
    match progress {
        Progress::Held => wire::TaskStatus::Held,
        Progress::Blocked => wire::TaskStatus::Blocked,
        Progress::Queued => wire::TaskStatus::Queued,
        Progress::Running => wire::TaskStatus::Running,
        Progress::Waiting => wire::TaskStatus::Waiting,
        Progress::Done => wire::TaskStatus::Done,
        Progress::Failed => wire::TaskStatus::Failed,
        Progress::Cancelled => wire::TaskStatus::Cancelled,
        Progress::Missing => wire::TaskStatus::Missing,
    }
}

fn activity(activity: atif::Activity) -> wire::Activity {
    match activity {
        atif::Activity::Reading => wire::Activity::Reading,
        atif::Activity::Editing => wire::Activity::Editing,
        atif::Activity::Running => wire::Activity::Running,
        atif::Activity::Testing => wire::Activity::Testing,
        atif::Activity::Judging => wire::Activity::Judging,
        atif::Activity::Thinking => wire::Activity::Thinking,
        atif::Activity::Waiting => wire::Activity::Waiting,
        atif::Activity::Blocked => wire::Activity::Blocked,
        atif::Activity::Done => wire::Activity::Done,
        atif::Activity::Failed => wire::Activity::Failed,
    }
}

fn place(station: atif::Station) -> wire::Station {
    match station {
        atif::Station::Library => wire::Station::Library,
        atif::Station::Desk => wire::Station::Desk,
        atif::Station::Workbench => wire::Station::Workbench,
        atif::Station::ProvingGround => wire::Station::ProvingGround,
        atif::Station::Oracle => wire::Station::Oracle,
        atif::Station::Podium => wire::Station::Podium,
        atif::Station::Lounge => wire::Station::Lounge,
        atif::Station::TaskWall => wire::Station::TaskWall,
    }
}

#[cfg(test)]
#[path = "studio_intents_tests.rs"]
mod tests;
