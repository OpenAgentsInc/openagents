//! Whether the work a crash left in flight is still running.
//!
//! Recovery marks an unfinished record `unknown`; [`reconcile`] rules
//! whether the work may start again. Reattachment is the question that
//! comes first: a `dispatched` record holds a reference to the job it
//! started — a decision request, a delegate session, a subprocess —
//! and when the coordinator comes back the job may still be running
//! under it. Watching that job to its end is the honest continuation;
//! starting a second attempt while the first still runs is the
//! double-dispatch the claim exists to prevent.
//!
//! An executor answers one of three observations:
//!
//! - [`Observation::Live`] — a job answers the reference and carries
//!   the pins the record claimed. The ruling is
//!   [`Reattachment::Observe`]: follow it, and settle what it reports
//!   as the record's end.
//! - [`Observation::Ended`] — a job under the reference already came
//!   to an end while nobody watched. The ruling is
//!   [`Reattachment::SettleOnEvidence`]: read the end as evidence and
//!   reconcile against it — nothing runs again.
//! - [`Observation::Gone`] and [`Observation::Unsupported`] — nothing
//!   answers, or the executor cannot resume at all: a session that
//!   ended with the coordinator, a one-shot request with no job API.
//!   The ruling is [`Reattachment::NewAttempt`], and it is honest
//!   about what it is — a deliberate new attempt under its own
//!   attempt number, never a claim that the first ran exactly once.
//!
//! A live job whose pins do not match the record's is none of these:
//! the reference is stale or collides with another run's work, and
//! [`Reattachment::Refused`] keeps one run from adopting another's.

use crate::runstate::Run;

/// What the host observed for one record's dispatch reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Observation<'a> {
    /// A live job answers the reference. `pins_match` is whether the
    /// job's claimed inputs are the record's — a job running under the
    /// reference for different inputs is a different run's work.
    Live {
        /// The reference the record dispatched under.
        reference: &'a str,
        /// Whether the job's pinned inputs match the record's.
        pins_match: bool,
    },
    /// A job under the reference ended while unwatched — its end is
    /// evidence already written.
    Ended {
        /// The reference the record dispatched under.
        reference: &'a str,
    },
    /// The executor answers for jobs but nothing lives under the
    /// reference.
    Gone,
    /// The executor cannot resume — a session that ended with the
    /// coordinator, a one-shot request. The limitation is part of the
    /// answer, not a failure to hide.
    Unsupported,
}

/// What one record's in-flight work may become.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reattachment {
    /// Watch the live job to its end and settle what it reports.
    /// The reference names what to observe.
    Observe {
        /// The dispatch reference the live job answers.
        reference: String,
    },
    /// The job's end already exists — reconcile against it as
    /// evidence. Nothing dispatches again.
    SettleOnEvidence {
        /// The reference the ended job ran under.
        reference: String,
    },
    /// Nothing is live and nothing can resume. A new attempt is the
    /// deliberate move — its own attempt number, its own record — and
    /// never the claim that the first attempt ran exactly once.
    NewAttempt {
        /// Why reattachment was not possible — stated, not hidden.
        limitation: &'static str,
    },
    /// A job answers the reference but is not this record's job —
    /// adopting it would attribute another run's work.
    Refused {
        /// Why the observation was refused.
        reason: &'static str,
    },
}

impl Reattachment {
    /// The word a report or ATIF record carries.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Self::Observe { .. } => "observe",
            Self::SettleOnEvidence { .. } => "settle-on-evidence",
            Self::NewAttempt { .. } => "new-attempt",
            Self::Refused { .. } => "refused",
        }
    }
}

/// Rule whether one record's in-flight work may be reattached, from
/// what the host observed for its dispatch reference.
///
/// The ruling never executes anything itself: `Observe` and
/// `SettleOnEvidence` tell the caller what to watch or read, and
/// `NewAttempt` says only that a new attempt is the honest move —
/// launching it is the operator's or reconciler's deliberate act,
/// recorded under its own attempt number.
#[must_use]
pub fn reattach(observation: Observation<'_>) -> Reattachment {
    match observation {
        Observation::Live {
            reference,
            pins_match: true,
        } => Reattachment::Observe {
            reference: reference.to_string(),
        },
        Observation::Live {
            pins_match: false, ..
        } => Reattachment::Refused {
            reason: "a job answers the reference but its pins are another run's",
        },
        Observation::Ended { reference } => Reattachment::SettleOnEvidence {
            reference: reference.to_string(),
        },
        Observation::Gone => Reattachment::NewAttempt {
            limitation: "no job answers the dispatch reference",
        },
        Observation::Unsupported => Reattachment::NewAttempt {
            limitation: "the executor cannot resume dispatched work",
        },
    }
}

/// Which unfinished records of a run may be reattached at all: the
/// ones still `dispatched` or recovered to `unknown` while holding a
/// dispatch reference — a restarting coordinator sees the first, a
/// recovery the second, and both ask the same question. A record
/// never dispatched has no job to find, and a written end has nothing
/// to reattach to.
///
/// `observe` answers for one record's dispatch reference — the host's
/// probe of the executor that ran it. The subject is `run`,
/// `step:<name>`, or `task:<name>#<attempt>` as in [`crate::reconcile`].
#[must_use]
pub fn reattachable(
    run: &Run,
    observe: impl for<'a> Fn(&str, &'a str) -> Observation<'a>,
) -> Vec<(String, Reattachment)> {
    let mut rulings = Vec::new();
    for (subject, reference) in references(run) {
        rulings.push((subject.clone(), reattach(observe(&subject, &reference))));
    }
    rulings
}

/// Every unfinished record's dispatch reference — `result` is where a
/// dispatched mark keeps the job it named.
fn references(run: &Run) -> Vec<(String, String)> {
    use crate::runstate::State;
    let unfinished = |state| matches!(state, State::Dispatched | State::Unknown);
    let mut references = Vec::new();
    if unfinished(run.state) {
        if let Some(reference) = &run.result {
            references.push(("run".to_string(), reference.clone()));
        }
    }
    for step in &run.steps {
        if let (true, Some(reference)) = (unfinished(step.state), step.result.as_ref()) {
            references.push((format!("step:{}", step.step), reference.clone()));
        }
    }
    for task in &run.tasks {
        if let (true, Some(reference)) = (unfinished(task.state), task.result.as_ref()) {
            references.push((
                format!("task:{}#{}", task.task, task.attempt),
                reference.clone(),
            ));
        }
    }
    references
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runstate::{Outcome, State, Step, Task};

    fn run(state: State) -> Run {
        Run {
            schema: "runstate/v1".to_string(),
            run: "run-1".to_string(),
            base: "base".to_string(),
            program: "digest".to_string(),
            questions: Vec::new(),
            sources: Vec::new(),
            state,
            outcome: None,
            result: None,
            worktree: None,
            unix: 0,
            steps: Vec::new(),
            tasks: Vec::new(),
        }
    }

    fn step(name: &str, state: State, reference: Option<&str>) -> Step {
        Step {
            schema: "runstate/v1".to_string(),
            step: name.to_string(),
            state,
            worktree: None,
            result: reference.map(str::to_string),
            unix: 0,
        }
    }

    fn task(name: &str, attempt: u32, state: State, reference: Option<&str>) -> Task {
        Task {
            schema: "runstate/v1".to_string(),
            task: name.to_string(),
            attempt,
            state,
            worktree: None,
            result: reference.map(str::to_string),
            unix: 0,
        }
    }

    #[test]
    fn a_live_job_under_matching_pins_is_observed_not_redone() {
        let ruling = reattach(Observation::Live {
            reference: "job-77",
            pins_match: true,
        });
        assert_eq!(
            ruling,
            Reattachment::Observe {
                reference: "job-77".to_string()
            }
        );
        assert_eq!(ruling.word(), "observe");
    }

    #[test]
    fn a_live_job_under_other_pins_is_refused_not_adopted() {
        let ruling = reattach(Observation::Live {
            reference: "job-77",
            pins_match: false,
        });
        assert!(matches!(ruling, Reattachment::Refused { .. }));
    }

    #[test]
    fn an_ended_job_is_evidence_to_settle_not_work_to_redo() {
        let ruling = reattach(Observation::Ended {
            reference: "job-12",
        });
        assert_eq!(
            ruling,
            Reattachment::SettleOnEvidence {
                reference: "job-12".to_string()
            }
        );
    }

    #[test]
    fn nothing_live_and_no_resume_both_mean_a_deliberate_new_attempt() {
        for observation in [Observation::Gone, Observation::Unsupported] {
            let ruling = reattach(observation);
            assert!(matches!(ruling, Reattachment::NewAttempt { .. }));
        }
        // The limitation is stated differently for each — the honest
        // part of the answer, not a detail to hide.
        let gone = reattach(Observation::Gone);
        let unsupported = reattach(Observation::Unsupported);
        match (gone, unsupported) {
            (
                Reattachment::NewAttempt { limitation: g },
                Reattachment::NewAttempt { limitation: u },
            ) => assert_ne!(g, u),
            _ => panic!("both are new-attempt rulings"),
        }
    }

    #[test]
    fn only_unfinished_records_with_references_reattach() {
        let mut run = run(State::Unknown);
        run.result = Some("job-run".to_string());
        run.steps.push(step("watch", State::Unknown, Some("job-3")));
        run.steps
            .push(step("in-flight", State::Dispatched, Some("job-5")));
        run.steps.push(step("never-went", State::Unknown, None));
        run.steps.push(step("done", State::Answered, Some("job-4")));
        run.tasks
            .push(task("issue-2", 1, State::Unknown, Some("session-9")));
        run.tasks
            .push(task("issue-2", 2, State::Cancelled, Some("session-10")));
        let rulings = reattachable(&run, |_, reference| Observation::Live {
            reference,
            pins_match: true,
        });
        let subjects: Vec<&str> = rulings.iter().map(|(s, _)| s.as_str()).collect();
        // The run's own reference, the unknown and in-flight steps',
        // and the unknown task attempt's. A reference-less record has
        // nothing to find; a written end takes no ruling at all.
        assert_eq!(
            subjects,
            ["run", "step:watch", "step:in-flight", "task:issue-2#1"]
        );
        assert!(
            rulings
                .iter()
                .all(|(_, r)| matches!(r, Reattachment::Observe { .. }))
        );
    }

    #[test]
    fn a_settled_run_reattaches_nothing() {
        let mut run = run(State::Settled);
        run.outcome = Some(Outcome::Answered);
        run.result = Some("job-final".to_string());
        let rulings = reattachable(&run, |_, reference| Observation::Live {
            reference,
            pins_match: true,
        });
        assert!(rulings.is_empty());
    }
}
