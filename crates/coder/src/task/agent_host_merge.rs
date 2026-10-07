//! The Merge station from her side: where it is, and her own merge when
//! the owner asks for it.
//!
//! The owner reviews and merges at the Merge station; when the owner asks
//! her directly to merge her own change, she does it for them. She merges
//! only a change of hers that reached the station, which means its checks
//! passed, through the same landing path the station's **Merge** takes: it
//! merges into the checkout's branch and pushes nothing. Neither a
//! question about the station nor her merge calls a model.

use super::*;
use coder_host::access::review::PublishState;

/// Her answer to where the Merge station is.
pub(crate) const WHERE: &str = "The Merge station is the strongroom in the Everglade \
     workshop yard: the metal crate behind the metal fence. From a terminal, `openagents studio \
     review TASK --diff` shows a change and `openagents studio merge TASK` merges it. Or ask me \
     to merge my own change.";

fn lower(text: &str) -> String {
    text.to_lowercase().replace(['\u{2019}', '`'], "'")
}

/// Whether `text` asks where the Merge station is.
#[must_use]
pub(crate) fn asks_where(text: &str) -> bool {
    let text = lower(text);
    text.contains("merge station")
        && ["where", "how do i", "how to"]
            .iter()
            .any(|word| text.contains(word))
}

/// Whether `text` asks her to merge her own change: it names merging and
/// asks her to do it, and does not tell her not to.
#[must_use]
pub(crate) fn asks_to_merge(text: &str) -> bool {
    let text = lower(text);
    if !text.contains("merge") {
        return false;
    }
    if ["don't merge", "do not merge", "never merge", "not merge"]
        .iter()
        .any(|no| text.contains(no))
    {
        return false;
    }
    [
        "can you merge",
        "could you merge",
        "please merge",
        "merge it",
        "merge that",
        "merge your",
        "merge the change",
        "merge my",
        "go ahead and merge",
        "you merge",
        "instead of me",
        "for me",
        "do that",
        "do it",
    ]
    .iter()
    .any(|ask| text.contains(ask))
}

impl Agents {
    /// Answers a request about the Merge station, or `None` when `queued`
    /// is not one.
    pub(super) fn merge_station(
        &self,
        store: &Store,
        record: &Record,
        queued: &Queued,
    ) -> Option<Report> {
        let (place, merge) = (asks_where(&queued.text), asks_to_merge(&queued.text));
        if !place && !merge {
            return None;
        }
        let now = (self.clock)();
        let _ = store.append(&request_entry(now, queued));
        let name = &record.name;
        let mut said = Vec::new();
        let mut outcome = Outcome::Done;
        let mut headline = "answered".to_string();
        if place {
            self.set_status(name, "Checking where the Merge station is");
            said.push(WHERE.to_string());
        }
        if merge {
            self.set_status(name, "Merging my change at the Merge station");
            match self.merge_own(store, name) {
                Ok(line) => {
                    headline = "merged".into();
                    said.push(line);
                }
                Err(line) => {
                    outcome = Outcome::Failed;
                    headline = "not merged".into();
                    said.push(line);
                }
            }
        }
        let reply = said.join(" ");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn she_reads_where_and_merge_requests_plainly() {
        let owner = "where's the merge station and can you do that instead of me";
        assert!(asks_where(owner) && asks_to_merge(owner));
        assert!(asks_to_merge("Please merge your change for #10893"));
        assert!(asks_to_merge("merge it"));
        assert!(asks_where("How do I get to the Merge station?"));
        assert!(!asks_where("merge the change"));
        assert!(!asks_to_merge("don't merge it yet"));
        assert!(!asks_to_merge("what is a merge conflict?"));
        assert!(!asks_to_merge("fix the typo in the readme"));
    }
}
