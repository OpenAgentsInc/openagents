//! A direct request to a resident seat, such as the workshop agent's task
//! mode (`docs/verse/workshop-agent.md`, "Task mode"): a goal of exactly
//! one task, already planned for that seat, with no lead's planning turn.
//!
//! The goal's lead is the seat itself, and its lead slot names the same
//! task as its one plan entry, so the goal's progress is that task's. The
//! entry is released at once into a worktree of its own, when the studio
//! has a worktrees directory, and follows the ordinary path to the
//! person's merge decision: checks, one fix round, and the Merge station.
//! A worker seat as the goal's lead means no lead review; the person
//! reviews the change at the station.

use std::path::Path;

use super::{
    Error, Goal, Inbox, PlanEntry, Released, Repository, Slot, SlotState, Studio, digest_bytes,
    git, slot_task_id,
};

/// One remote task's studio entry, as [`Studio::remote_tasks`] reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteTask {
    /// The local task identity.
    pub task: String,
    /// The computer it runs on.
    pub computer: String,
    /// Its identity on the computer's host, when attached.
    pub remote_task: Option<String>,
    /// Its change's stage.
    pub stage: super::flow::Stage,
}

/// A direct request to submit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Direct {
    /// What the seat is asked to do.
    pub text: String,
    /// The task's title, at most 120 bytes.
    pub title: String,
    pub repository: Repository,
    /// The seat that does it.
    pub seat: String,
}

impl Studio {
    /// Submit `direct`: record a one-task goal for its seat and release the
    /// task. Returns the goal and the task identity.
    ///
    /// # Errors
    /// The text or repository is malformed, the seat is unknown or paused,
    /// the studio holds its most goals, or the inbox refuses.
    pub fn submit_direct(
        &mut self,
        tasks: &mut dyn Inbox,
        direct: Direct,
        now: u64,
    ) -> Result<(String, String, Vec<Released>), Error> {
        let (goal_id, task_id, index) = self.record_direct(direct, now)?;
        let released = self.release_ready(tasks, index, now)?;
        Ok((goal_id, task_id, released))
    }

    /// Submit `direct` for a task that runs on the computer `computer`
    /// (#10930): the same one-task goal, its worktree and run record made
    /// here like a release's, but no command reaches the inbox, so no
    /// local run starts. The seat that placed it moves its flow when the
    /// remote task ends: [`Studio::land_remote`] applies the patch the
    /// computer returns, and `openagents studio review`/`merge` work on it
    /// like a local task's change.
    ///
    /// # Errors
    /// As [`Studio::submit_direct`], plus a studio without a worktrees
    /// directory, or a worktree the repository cannot make.
    pub fn submit_remote(
        &mut self,
        direct: Direct,
        computer: &str,
        base: &str,
        now: u64,
    ) -> Result<(String, String), Error> {
        let Some(worktrees) = self.worktrees.clone() else {
            return Err(Error::State(
                "remote placement needs the studio's worktrees directory".into(),
            ));
        };
        let (goal_id, task_id, index) = self.record_direct(direct, now)?;
        let (repository, seat, title) = {
            let goal = &self.state.goals[index];
            let entry = &goal.plan[0];
            (
                goal.repository.path.clone(),
                entry.slot.seat.clone(),
                entry.title.clone(),
            )
        };
        git::prepare(
            &worktrees,
            &self.store,
            Path::new(&repository),
            &seat,
            &task_id,
            &title,
            Some("devin"),
            Some(base),
        )
        .map_err(|message| Error::Tasks(super::super::Error::Io(std::io::Error::other(message))))?;
        let item = &mut self.state.goals[index].plan[0];
        item.flow = Some(super::flow::Flow::remote(&task_id, computer));
        self.save()?;
        Ok((goal_id, task_id))
    }

    /// Where remote task `task`'s studio entry is: its goal and plan
    /// indexes, when it is one `submit_remote` made.
    fn remote_entry(&self, task: &str) -> Option<(usize, usize)> {
        for (index, goal) in self.state.goals.iter().enumerate() {
            for (entry, item) in goal.plan.iter().enumerate() {
                if item.slot.task_id == task
                    && item.flow.as_ref().is_some_and(|flow| flow.remote.is_some())
                {
                    return Some((index, entry));
                }
            }
        }
        None
    }

    /// Every remote task `seat` placed: the local task, the computer, the
    /// remote task's identity when attached, and the stage (#10930).
    #[must_use]
    pub fn remote_tasks(&self, seat: &str) -> Vec<RemoteTask> {
        self.state
            .goals
            .iter()
            .flat_map(|goal| goal.plan.iter())
            .filter(|entry| entry.slot.seat == seat)
            .filter_map(|entry| {
                let flow = entry.flow.as_ref()?;
                Some(RemoteTask {
                    task: entry.slot.task_id.clone(),
                    computer: flow.remote.clone()?,
                    remote_task: flow.remote_task.clone(),
                    stage: flow.stage,
                })
            })
            .collect()
    }

    /// Keep remote task `task`'s identity on its computer in the flow, so
    /// a restart resumes watching it (#10930).
    ///
    /// # Errors
    /// The task is not a remote one.
    pub fn attach_remote_task(&mut self, task: &str, remote_task: &str) -> Result<(), Error> {
        let Some((index, entry)) = self.remote_entry(task) else {
            return Err(Error::State(format!("task `{task}` is not a remote one")));
        };
        self.state.goals[index].plan[entry]
            .flow
            .as_mut()
            .expect("a remote entry has a flow")
            .remote_task = Some(remote_task.to_owned());
        self.save()
    }

    /// Apply the patch `computer`'s task `task` sent back into the task's
    /// local worktree, commit it on the task's branch, and move its change
    /// to the person's merge decision. Returns the worktree it landed in.
    ///
    /// # Errors
    /// The task is not a remote one, it is not waiting on a remote
    /// change, its worktree is unreadable, or Git cannot apply or commit
    /// the patch.
    pub fn land_remote(&mut self, task: &str, patch: &str) -> Result<std::path::PathBuf, Error> {
        let Some((index, entry)) = self.remote_entry(task) else {
            return Err(Error::State(format!("task `{task}` is not a remote one")));
        };
        // Two looks can race a landing; only the first applies the patch.
        if self.state.goals[index].plan[entry]
            .flow
            .as_ref()
            .is_some_and(|flow| flow.stage != super::flow::Stage::Work)
        {
            return Err(Error::State(format!(
                "task `{task}` is not waiting on a remote change"
            )));
        }
        let record = super::super::local::record(&self.store, task)
            .ok_or_else(|| Error::Corrupt("the remote task lost its run record"))?;
        let seat = self.state.goals[index].plan[entry].slot.seat.clone();
        let worktree = Path::new(&record.worktree);
        git::apply_remote(worktree, &seat, patch).map_err(|message| {
            Error::Tasks(super::super::Error::Io(std::io::Error::other(message)))
        })?;
        let flow = self.state.goals[index].plan[entry]
            .flow
            .as_mut()
            .expect("a remote entry has a flow");
        flow.stage = super::flow::Stage::Merge;
        self.save()?;
        Ok(worktree.to_path_buf())
    }

    /// Mark remote task `task`'s change ended without a merge decision and
    /// say why: the remote run failed or was cancelled, or its patch could
    /// not land.
    ///
    /// # Errors
    /// The task is not a remote one, or it already ended.
    pub fn fail_remote(&mut self, task: &str, note: &str) -> Result<(), Error> {
        let Some((index, entry)) = self.remote_entry(task) else {
            return Err(Error::State(format!("task `{task}` is not a remote one")));
        };
        if self.state.goals[index].plan[entry]
            .flow
            .as_ref()
            .is_some_and(|flow| flow.stage != super::flow::Stage::Work)
        {
            return Err(Error::State(format!("task `{task}` already ended")));
        }
        let flow = self.state.goals[index].plan[entry]
            .flow
            .as_mut()
            .expect("a remote entry has a flow");
        flow.notes.push(note.to_owned());
        flow.stage = super::flow::Stage::Rejected;
        self.save()
    }

    /// Record `direct`'s goal: the shared part of [`Studio::submit_direct`]
    /// and [`Studio::submit_remote`]. Returns the goal, the task identity,
    /// and the goal's index.
    fn record_direct(
        &mut self,
        direct: Direct,
        now: u64,
    ) -> Result<(String, String, usize), Error> {
        let text = direct.text.trim();
        if !super::super::text(text, super::MAX_GOAL_BYTES, true) {
            return Err(Error::Invalid(format!(
                "a request is 1 to {} bytes of text",
                super::MAX_GOAL_BYTES
            )));
        }
        let path = Path::new(&direct.repository.path);
        if !super::super::identifier(&direct.repository.label, false) || !path.is_absolute() {
            return Err(Error::Invalid(
                "a repository is a workspace label and an absolute path".into(),
            ));
        }
        if self.state.seat(&direct.seat).is_none() {
            return Err(Error::UnknownSeat(direct.seat));
        }
        if self.paused_seat(&direct.seat) {
            return Err(Error::State(format!("seat `{}` is paused", direct.seat)));
        }
        if self.state.goals.len() >= super::MAX_GOALS {
            return Err(Error::LimitExceeded("goals"));
        }
        let digest = digest_bytes(format!("{now}\n{}\n{text}", direct.seat).as_bytes());
        let goal_id = format!(
            "d{}-{}",
            self.state.goals.len() + 1,
            &digest["sha256:".len().."sha256:".len() + 8]
        );
        let task_id = slot_task_id(&goal_id, "work");
        let title = super::one_line(&direct.title, 120);
        self.state.goals.push(Goal {
            goal_id: goal_id.clone(),
            text: text.to_owned(),
            repository: direct.repository,
            lead: Slot {
                task_id: task_id.clone(),
                seat: direct.seat.clone(),
                state: SlotState::Submitted,
                command: None,
                attempt: 1,
            },
            planned: true,
            plan: vec![PlanEntry {
                id: "work".into(),
                title,
                description: text.to_owned(),
                depends_on: Vec::new(),
                slot: Slot {
                    task_id: task_id.clone(),
                    seat: direct.seat,
                    state: SlotState::Held,
                    command: None,
                    attempt: 1,
                },
                flow: None,
            }],
            decision: None,
            submitted_at: now,
        });
        self.save()?;
        Ok((goal_id, task_id, self.state.goals.len() - 1))
    }

    /// Where the direct goal `goal_id`'s one task is: its current task
    /// identity, which a fix or a retry changes, and its flow's stage.
    #[must_use]
    pub fn direct_task(&self, goal_id: &str) -> Option<(String, Option<super::flow::Stage>)> {
        let goal = self.state.goal(goal_id)?;
        let entry = goal.plan.first()?;
        Some((
            entry.slot.task_id.clone(),
            entry.flow.as_ref().map(|flow| flow.stage),
        ))
    }
}
