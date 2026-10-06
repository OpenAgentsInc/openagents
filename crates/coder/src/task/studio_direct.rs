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
    slot_task_id,
};

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
        let index = self.state.goals.len() - 1;
        let released = self.release_ready(tasks, index, now)?;
        Ok((goal_id, task_id, released))
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
