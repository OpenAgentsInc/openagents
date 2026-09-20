//! The golden reads back, and the run it records took the path the task
//! expects.
//!
//! This is the test that keeps the golden honest. It parses the file with
//! the same `atif` code a live session writes with, so a golden that drifts
//! from the format fails here rather than the first time somebody tries to
//! read one.

use coderbench::{Fault, Task, goldens_dir, observe, tasks_dir};

#[test]
fn the_golden_reads_back_as_atif() {
    let path = goldens_dir().join("devin-fan-out-six.atif.jsonl");
    let recording = atif::log::read(&path).expect("golden parses as an ATIF log");
    assert_eq!(recording.session.id, "devin-fan-out-six");
    assert_eq!(recording.steps.len(), 13, "every step of the path is recorded");
    let document = recording.document();
    assert!(document.get("steps").is_some(), "the document renders");
}

#[test]
fn the_recorded_run_took_the_path_the_task_expects() {
    let task = Task::load(&tasks_dir().join("devin-fan-out-six").join("task.json"))
        .expect("task manifest loads");
    let run = observe(&goldens_dir().join("devin-fan-out-six.atif.jsonl")).expect("golden observes");

    assert_eq!(run.program.as_deref(), Some("delegate-fan-out"));
    assert_eq!(run.delegations.len(), 6);
    assert!(run.writes.is_empty(), "a read-only task writes nothing");

    let faults = task.judge(&run);
    assert!(faults.is_empty(), "the recorded run is clean: {faults:?}");
}

#[test]
fn a_wrong_delegation_is_a_fault() {
    let task = Task::load(&tasks_dir().join("devin-fan-out-six").join("task.json")).unwrap();
    let mut run = observe(&goldens_dir().join("devin-fan-out-six.atif.jsonl")).unwrap();
    run.delegations[2].correct = Some(false);
    let faults = task.judge(&run);
    assert!(
        faults.iter().any(|f| matches!(f, Fault::DelegationWrong { .. })),
        "judging catches a delegate that answered wrongly"
    );
}

#[test]
fn a_missing_decision_is_a_fault() {
    let task = Task::load(&tasks_dir().join("devin-fan-out-six").join("task.json")).unwrap();
    let mut run = observe(&goldens_dir().join("devin-fan-out-six.atif.jsonl")).unwrap();
    run.decisions.remove("independence");
    let faults = task.judge(&run);
    assert!(
        faults.iter().any(
            |f| matches!(f, Fault::DecisionMissing { name } if name == "independence")
        ),
        "a run that skipped the independence decision is not on the path"
    );
}

#[test]
fn the_golden_says_what_it_rests_on() {
    use coderbench::{GoldenMeta, Provenance};
    let meta = GoldenMeta::load(&goldens_dir().join("devin-fan-out-six.meta.json"))
        .expect("the golden declares its provenance");
    assert_eq!(meta.task, "devin-fan-out-six");
    assert!(!meta.repository_commit.is_empty(), "a golden names the commit it ran at");
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
