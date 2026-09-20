//! The golden reads back, and the run it records took the path the task
//! expects as far as a trace alone can show it.
//!
//! This is the test that keeps the golden honest. It parses the file with
//! the same `atif` code a live session writes with, so a golden that drifts
//! from the format fails here rather than the first time somebody tries to
//! read one.

use coderbench::{Fault, Observed, Task, Verdict, Workspace, goldens_dir, observe, tasks_dir};

fn task() -> Task {
    Task::load(&tasks_dir().join("devin-fan-out-six").join("task.json"))
        .expect("task manifest loads")
}

fn golden() -> Observed {
    observe(&goldens_dir().join("devin-fan-out-six.atif.jsonl")).expect("golden observes")
}

/// The golden with the two facts a trace cannot carry supplied: the exit
/// code the driver saw, and a reading of the checkout before and after.
fn as_run(mut run: Observed) -> Observed {
    run.ending = coderbench::Ending::Answered;
    run.workspace = Some(Workspace::default());
    run
}

#[test]
fn the_golden_reads_back_as_atif() {
    let path = goldens_dir().join("devin-fan-out-six.atif.jsonl");
    let recording = atif::log::read(&path).expect("golden parses as an ATIF log");
    assert_eq!(recording.session.id, "devin-fan-out-six");
    assert_eq!(
        recording.steps.len(),
        13,
        "every step of the path is recorded"
    );
    assert_eq!(recording.unreadable_lines, 0, "the golden is whole");
    assert!(recording.ended(), "the golden closed itself");
    let document = recording.document();
    assert!(document.get("steps").is_some(), "the document renders");
}

/// A trace on its own cannot pass. It holds the path, and it does not hold
/// the exit code or the workspace, so the two faults left are about the
/// evidence rather than about the run.
#[test]
fn a_trace_alone_is_unverifiable() {
    let judgment = task().judge(&golden());
    assert_eq!(judgment.verdict, Verdict::Unverifiable);
    let said: Vec<String> = judgment.faults.iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        vec![
            "nothing compared the workspace, so writing nothing is unobserved rather than shown"
                .to_string(),
            "the trace closed without saying how the episode ended; the task allows answered"
                .to_string(),
        ],
        "a trace is missing exactly the two things only a driver sees"
    );
}

#[test]
fn the_recorded_run_took_the_path_the_task_expects() {
    let run = golden();
    assert_eq!(run.program.as_deref(), Some("delegate-fan-out"));
    assert_eq!(run.delegations.len(), 6);
    assert!(
        run.delegations.iter().all(coderbench::Delegation::verified),
        "every delegate completed, answered, and was checked"
    );
    assert!(run.writes.is_empty(), "no delegate reported writing");

    let judgment = task().judge(&as_run(run));
    assert!(
        judgment.passed(),
        "the recorded run is clean once the driver's two facts are supplied: {:?}",
        judgment.faults
    );
}

#[test]
fn a_wrong_delegation_is_a_fault() {
    let mut run = as_run(golden());
    run.delegations[2].correct = Some(false);
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert!(
        judgment
            .faults
            .iter()
            .any(|f| matches!(f, Fault::DelegationWrong { .. })),
        "judging catches a delegate that answered wrongly"
    );
}

#[test]
fn a_missing_decision_is_a_fault() {
    let mut run = as_run(golden());
    run.decisions.remove("independence");
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert!(
        judgment
            .faults
            .iter()
            .any(|f| matches!(f, Fault::DecisionMissing { name } if name == "independence")),
        "a run that skipped the independence decision is not on the path"
    );
}

#[test]
fn the_golden_says_what_it_rests_on() {
    use coderbench::{GoldenMeta, Provenance};
    let meta = GoldenMeta::load(&goldens_dir().join("devin-fan-out-six.meta.json"))
        .expect("the golden declares its provenance");
    assert_eq!(meta.task, "devin-fan-out-six");
    assert!(
        !meta.repository_commit.is_empty(),
        "a golden names the commit it ran at"
    );
    assert_eq!(
        meta.provenance,
        Provenance::Staged,
        "Coder has not produced this path yet; see openagents#9412"
    );
    assert_ne!(
        meta.orchestrator, "coder",
        "a staged golden did not have Coder driving it, and must not claim to"
    );
}

/// A run that took none of the path faults on every step of it, and the
/// faults come back in the order the path takes rather than the order the
/// checker tests them.
///
/// The order is the whole point of `grade.path`. The first fault a reader
/// sees should be the earliest thing that went wrong, so a run with no
/// delegations reads as "it never probed" rather than as "it never
/// delegated", which is the consequence. Faults about the record sort after
/// every step, because they are about the trace rather than about a place
/// in it.
#[test]
fn faults_come_back_in_path_order() {
    let judgment = task().judge(&Observed::default());
    let said: Vec<String> = judgment.faults.iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        vec![
            "never ran the capability_probe check",
            "never ran the program_registry check",
            "selected program none, expected delegate-fan-out",
            "never asked the program decision",
            "never asked the independence decision",
            "never ran the admission_check check",
            "started 0 delegations, expected 6",
            "0 delegations are recorded correct, expected 6; 0 recorded nothing either way",
            "nothing compared the workspace, so writing nothing is unobserved rather than shown",
            "the trace has no end record, so the session never closed",
            "nothing observed how the episode ended; the task allows answered",
        ]
    );
    assert_eq!(judgment.verdict, Verdict::Failed);
}
