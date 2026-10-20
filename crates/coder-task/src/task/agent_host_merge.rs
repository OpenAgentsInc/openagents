//! The Merge station from her side: where it is, and her own merge when
//! the owner asks for it and confirms it.
//!
//! The owner reviews and merges at the Merge station; when the owner asks
//! her directly to merge her own change, she does it for them. Whether a
//! request asks that is Jev's typed answer (`agent_route`), never the
//! words in it, and even then she merges only after the owner confirms at
//! her lectern. She merges only a change of hers that reached the station,
//! which means its checks passed, through the same landing path the
//! station's **Merge** takes: it merges into the checkout's branch and
//! pushes nothing.

use std::sync::atomic::AtomicBool;

use super::super::agent_route::Route;
use super::*;
use coder_host::access::review::PublishState;

/// Her answer to where the Merge station is.
pub(crate) const WHERE: &str = "The Merge station is the strongroom in the Everglade \
     workshop yard: the metal crate behind the metal fence. From a terminal, `openagents studio \
     review TASK --diff` shows a change and `openagents studio merge TASK` merges it. Or ask me \
     to merge my own change.";

/// What she holds at her lectern before she merges.
pub(crate) const CONFIRM_MERGE: &str = "merge my change at the Merge station";

impl Agents {
    /// Answers a request `route` reads as one about the Merge station, or
    /// `None` when it is not one. Her own merge waits for the owner's
    /// CONFIRM at her lectern; a REJECT, a stop, or no answer merges
    /// nothing.
    pub(super) fn merge_station(
        &self,
        store: &Store,
        record: &Record,
        queued: &Queued,
        route: Route,
        stop: &AtomicBool,
    ) -> Option<Report> {
        if record.job_role.is_some() || !matches!(route, Route::MergeStationWhere | Route::MergeOwn)
        {
            return None;
        }
        let now = (self.clock)();
        let _ = store.append(&request_entry(now, queued));
        let name = &record.name;
        let (outcome, reply, headline) = if route == Route::MergeStationWhere {
            self.set_status(name, "Checking where the Merge station is");
            (Outcome::Done, WHERE.to_string(), "answered".to_string())
        } else {
            self.set_status(name, "Waiting for you to confirm my merge");
            let decision = self.propose(
                name,
                CONFIRM_MERGE,
                "You asked me to merge my change. Confirm and I merge it into your branch; \
                 reject and I leave it at the Merge station.",
                stop,
            );
            if decision == Decision::Confirm {
                self.set_status(name, "Merging my change at the Merge station");
                match self.merge_own(store, name) {
                    Ok(line) => (Outcome::Done, line, "merged".to_string()),
                    Err(line) => (Outcome::Failed, line, "not merged".to_string()),
                }
            } else {
                (
                    Outcome::Stopped,
                    "I didn't merge; my change still waits at the Merge station.".to_string(),
                    "not merged".to_string(),
                )
            }
        };
        let _ = store.append(&Entry::new((self.clock)(), Kind::Report, &reply));
        Some(Report {
            outcome,
            reply,
            headline,
        })
    }

    /// Merges her newest change that waits at the Merge station, at its
    /// current reviewed revisions.
    fn merge_own(&self, store: &Store, name: &str) -> Result<String, String> {
        use crate::task::studio::Studio;
        use crate::task::studio::flow::Stage;
        let task = Studio::open(&self.tasks)
            .ok()
            .and_then(|studio| {
                studio
                    .state()
                    .goals
                    .iter()
                    .filter_map(|goal| {
                        let entry = goal.plan.first()?;
                        (entry.slot.seat == name
                            && entry.flow.as_ref().map(|flow| flow.stage) == Some(Stage::Merge))
                        .then(|| (goal.submitted_at, entry.slot.task_id.clone()))
                    })
                    .max()
                    .map(|(_, task)| task)
            })
            .ok_or_else(|| {
                "I have no change waiting at the Merge station, so there is nothing for me to \
                 merge."
                    .to_string()
            })?;
        let short = short_task(&task).to_string();
        let local = crate::task::local::record(&self.tasks, &task).ok_or_else(|| {
            format!("I can't find the worktree of task {short}, so I didn't merge.")
        })?;
        let review = crate::task::review::read_for_wire(
            &self.tasks,
            &task,
            Path::new(&local.worktree),
            &local.base,
        )
        .map_err(|why| {
            format!("I can't read the review of task {short} ({why}), so I didn't merge.")
        })?;
        let reviewed = crate::task::publish::Reviewed {
            base: review.base,
            head_commit: review.head_commit,
            head: review.head,
        };
        let publication = crate::task::studio::git::merge(&self.tasks, &task, &reviewed)
            .map_err(|_| format!("The Merge station refused task {short}, so I didn't merge."))?;
        if publication.state != PublishState::Published {
            return Err(format!(
                "The merge of task {short} didn't land: {}",
                agent::plain(&publication.note)
            ));
        }
        if let Ok(mut studio) = Studio::open(&self.tasks) {
            let _ = studio.note_merged(&task);
        }
        if let Some(sweep) = &self.sweep {
            sweep();
        }
        self.with_live(name, |live| live.change = None);
        let _ = store.append(&Entry::new(
            (self.clock)(),
            Kind::Task,
            &format!("merged task {short} at the Merge station at the owner's request"),
        ));
        let into = publication
            .branch
            .as_deref()
            .map_or_else(String::new, |branch| format!(" into {branch}"));
        let at = publication
            .commit
            .as_deref()
            .map_or_else(String::new, |commit| {
                format!(" at {}", commit.get(..10).unwrap_or(commit))
            });
        Ok(format!(
            "I merged my change (task {short}){into}{at}, as you asked; its checks passed before \
             it reached the Merge station."
        ))
    }
}
