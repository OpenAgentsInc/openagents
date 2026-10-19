//! What a recovered `unknown` mark may become.
//!
//! Recovery marks unfinished records `unknown`; reconciliation is the
//! next question: may the work start again? A mark says what happened —
//! this module says what may happen next, and the answer is an
//! authority question, not a convenience. A step that only reads is
//! replayable: running it again produces the same observable world. A
//! step that may have written, spent, or delegated is not: the record
//! cannot say whether the effect already happened, and running again
//! may produce it twice. The acceptance's own words — never
//! automatically replay an ambiguous write, payment, merge, or other
//! non-idempotent effect — are the whole rule: an `unknown` mark is
//! evidence, and a replay is a decision the operator makes with it,
//! not something the store does to itself.
//!
//! Three rulings, and nothing between them:
//!
//! - [`Reconciliation::Replayable`] — the record's declared effects
//!   cannot have produced an external change, and the recorded
//!   authority covers what replaying would do.
//! - [`Reconciliation::NeedsDecision`] — the record could have
//!   produced a non-idempotent effect, or nobody recorded what it would
//!   have done at all. Either way a new attempt is the operator's call,
//!   made against the evidence the ruling names.
//! - [`Reconciliation::OutsideAuthority`] — replaying would exceed the
//!   grant the run was claimed under, whatever the mark means. That is
//!   refused outright: a crash never widens a grant.

use std::collections::BTreeMap;

use crate::program_authority::Effects;
use crate::runstate::{Run, State};

/// What one unfinished record may do next.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reconciliation {
    /// Re-running it produces the same observable world — a read is
    /// its own idempotence, and the authority already covers it.
    Replayable,
    /// The record may have produced an effect nobody can rule out, or
    /// its effects were never recorded at all. Reconciliation is the
    /// operator's answer to the evidence, never an automatic replay.
    NeedsDecision,
    /// Replaying would exceed the recorded grant. Refused, whatever
    /// the mark means — a crash does not widen authority.
    OutsideAuthority,
}

impl Reconciliation {
    /// The word a report or ATIF record carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Replayable => "replayable",
            Self::NeedsDecision => "needs-decision",
            Self::OutsideAuthority => "outside-authority",
        }
    }
}

/// One record's ruling, with the evidence a reconciler reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ruling {
    /// Which record: `run`, `step:<name>`, or `task:<name>#<attempt>`.
    pub subject: String,
    /// The mark as it stands — `unknown` for everything a ruling
    /// covers, since a written end takes no ruling.
    pub state: State,
    /// What the record may do next.
    pub ruling: Reconciliation,
    /// The declared effects the ruling read. `None` is the honest
    /// answer when nobody recorded them — and an unrecorded footprint
    /// is never `Replayable`.
    pub effects: Option<Effects>,
    /// What a reconciler looks at: the worktree the record retained,
    /// the result reference it holds. Empty when there is nothing to
    /// show — which is itself a fact the caller sees.
    pub evidence: Vec<String>,
}

/// Rule every unfinished record in a recovered run.
///
/// `declared` maps a subject to the effects its work was declared to
/// have — `step:<name>` for a step, `task:<name>#<attempt>` for a task
/// attempt, `run` for the run itself. A subject nobody declared effects
/// for is unrecorded, and unrecorded is never replayable: the run could
/// have done anything. `authority` is the effect ceiling the run's
/// grant held — replay is refused where it would exceed it.
///
/// Only `unknown` records are ruled. A settled, answered, refused, or
/// cancelled record is an end someone wrote; reconciliation has nothing
/// to add to it.
#[must_use]
pub fn reconcile(
    run: &Run,
    declared: &BTreeMap<String, Effects>,
    authority: Effects,
) -> Vec<Ruling> {
    let mut rulings = Vec::new();
    if run.state == State::Unknown {
        rulings.push(rule(
            "run".to_string(),
            run.state,
            declared.get("run").copied(),
            authority,
            evidence(run.worktree.as_ref(), run.result.as_ref()),
        ));
    }
    for step in &run.steps {
        if step.state != State::Unknown {
            continue;
        }
        let subject = format!("step:{}", step.step);
        rulings.push(rule(
            subject.clone(),
            step.state,
            declared.get(&subject).copied(),
            authority,
            evidence(step.worktree.as_ref(), step.result.as_ref()),
        ));
    }
    for task in &run.tasks {
        if task.state != State::Unknown {
            continue;
        }
        let subject = format!("task:{}#{}", task.task, task.attempt);
        rulings.push(rule(
            subject.clone(),
            task.state,
            declared.get(&subject).copied(),
            authority,
            evidence(task.worktree.as_ref(), task.result.as_ref()),
        ));
    }
    rulings
}

/// One record's ruling from its declared effects and the grant.
fn rule(
    subject: String,
    state: State,
    effects: Option<Effects>,
    authority: Effects,
    evidence: Vec<String>,
) -> Ruling {
    let ruling = match effects {
        // Nobody recorded what the record would have done — it could
        // have done anything, and anything could be non-idempotent.
        None => Reconciliation::NeedsDecision,
        // A replay that would exceed the grant is refused before the
        // ambiguity question is ever reached.
        Some(declared) if !declared.missing(authority).is_empty() => {
            Reconciliation::OutsideAuthority
        }
        // Reads alone cannot have changed the world. Everything else —
        // writes, spend, delegation, network, subprocesses — could have
        // produced an effect the record cannot rule out.
        Some(declared) => {
            let mut effects = declared;
            effects.reads = false;
            if effects == Effects::none() {
                Reconciliation::Replayable
            } else {
                Reconciliation::NeedsDecision
            }
        }
    };
    Ruling {
        subject,
        state,
        ruling,
        effects,
        evidence,
    }
}

/// The evidence a record carries, as the references a reconciler reads —
/// paths and digests, never contents.
fn evidence(worktree: Option<&std::path::PathBuf>, result: Option<&String>) -> Vec<String> {
    let mut evidence = Vec::new();
    if let Some(path) = worktree {
        evidence.push(format!("worktree:{}", path.display()));
    }
    if let Some(result) = result {
        evidence.push(format!("result:{result}"));
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runstate::{Outcome, Step, Task};

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
            owner: None,
            unix: 0,
            steps: Vec::new(),
            tasks: Vec::new(),
        }
    }

    fn step(name: &str, state: State) -> Step {
        Step {
            schema: "runstate/v1".to_string(),
            step: name.to_string(),
            state,
            worktree: None,
            result: None,
            unix: 0,
        }
    }

    fn task(name: &str, attempt: u32, state: State) -> Task {
        Task {
            schema: "runstate/v1".to_string(),
            task: name.to_string(),
            attempt,
            state,
            worktree: None,
            result: None,
            unix: 0,
        }
    }

    fn effects(words: &[&str]) -> Effects {
        let mut effects = Effects::none();
        for word in words {
            effects = effects.union(Effects::named(word).expect("a named effect"));
        }
        effects
    }

    #[test]
    fn a_reads_only_record_is_replayable_under_its_grant() {
        let mut run = run(State::Unknown);
        run.steps.push(step("look-up", State::Unknown));
        let declared = BTreeMap::from([
            ("run".to_string(), effects(&["reads"])),
            ("step:look-up".to_string(), effects(&["reads"])),
        ]);
        let rulings = reconcile(&run, &declared, effects(&["reads", "writes"]));
        assert_eq!(rulings.len(), 2);
        assert!(
            rulings
                .iter()
                .all(|r| r.ruling == Reconciliation::Replayable)
        );
    }

    #[test]
    fn a_writing_record_needs_a_decision_never_a_replay() {
        let mut run = run(State::Unknown);
        run.tasks.push(task("issue-9", 1, State::Unknown));
        let declared =
            BTreeMap::from([("task:issue-9#1".to_string(), effects(&["reads", "writes"]))]);
        let rulings = reconcile(&run, &declared, Effects::all());
        let task_ruling = rulings
            .iter()
            .find(|r| r.subject == "task:issue-9#1")
            .expect("the task was ruled");
        assert_eq!(task_ruling.ruling, Reconciliation::NeedsDecision);
        // The run itself, never declared, needs a decision too.
        let run_ruling = rulings.iter().find(|r| r.subject == "run").expect("ruled");
        assert_eq!(run_ruling.ruling, Reconciliation::NeedsDecision);
    }

    #[test]
    fn a_spend_is_a_payment_and_payments_are_ambiguous() {
        let mut run = run(State::Unknown);
        run.steps.push(step("decide", State::Unknown));
        let declared = BTreeMap::from([(
            "step:decide".to_string(),
            effects(&["reads", "network", "spend"]),
        )]);
        let rulings = reconcile(&run, &declared, Effects::all());
        let ruling = rulings
            .iter()
            .find(|r| r.subject == "step:decide")
            .expect("ruled");
        assert_eq!(ruling.ruling, Reconciliation::NeedsDecision);
    }

    #[test]
    fn replay_past_the_grant_is_refused_not_decided() {
        let mut run = run(State::Unknown);
        run.steps.push(step("build", State::Unknown));
        let declared = BTreeMap::from([("step:build".to_string(), effects(&["reads", "writes"]))]);
        // The grant covered reads only — the record may have written,
        // and replaying would exceed what the run was allowed.
        let rulings = reconcile(&run, &declared, effects(&["reads"]));
        let ruling = rulings
            .iter()
            .find(|r| r.subject == "step:build")
            .expect("ruled");
        assert_eq!(ruling.ruling, Reconciliation::OutsideAuthority);
    }

    #[test]
    fn written_ends_take_no_ruling() {
        let mut run = run(State::Settled);
        run.outcome = Some(Outcome::Answered);
        run.steps.push(step("done", State::Answered));
        run.steps.push(step("ended", State::Cancelled));
        run.steps.push(step("lost", State::Unknown));
        let declared = BTreeMap::from([("step:lost".to_string(), effects(&["reads"]))]);
        let rulings = reconcile(&run, &declared, Effects::all());
        // Only the unknown step is ruled — the settled run and its
        // finished steps are ends already written.
        assert_eq!(rulings.len(), 1);
        assert_eq!(rulings[0].subject, "step:lost");
    }

    #[test]
    fn a_retained_worktree_and_result_ride_the_ruling_as_evidence() {
        let mut run = run(State::Unknown);
        let mut kept = task("issue-4", 2, State::Unknown);
        kept.worktree = Some(std::path::PathBuf::from("/tmp/wt-issue-4"));
        kept.result = Some("atif:session-3".to_string());
        run.tasks.push(kept);
        let declared =
            BTreeMap::from([("task:issue-4#2".to_string(), effects(&["reads", "writes"]))]);
        let rulings = reconcile(&run, &declared, Effects::all());
        let ruling = rulings
            .iter()
            .find(|r| r.subject == "task:issue-4#2")
            .expect("ruled");
        assert_eq!(ruling.ruling, Reconciliation::NeedsDecision);
        assert!(
            ruling
                .evidence
                .iter()
                .any(|e| e == "worktree:/tmp/wt-issue-4")
        );
        assert!(ruling.evidence.iter().any(|e| e == "result:atif:session-3"));
        assert_eq!(ruling.evidence.len(), 2);
    }

    #[test]
    fn an_undeclared_record_is_never_replayable() {
        let mut run = run(State::Unknown);
        run.steps.push(step("mystery", State::Unknown));
        let rulings = reconcile(&run, &BTreeMap::new(), Effects::all());
        assert!(
            rulings
                .iter()
                .all(|r| r.ruling == Reconciliation::NeedsDecision)
        );
        assert!(rulings.iter().all(|r| r.effects.is_none()));
    }

    #[test]
    fn a_recovered_run_with_no_unfinished_records_rules_nothing() {
        let run = run(State::Settled);
        let rulings = reconcile(&run, &BTreeMap::new(), Effects::all());
        assert!(rulings.is_empty());
    }
}
