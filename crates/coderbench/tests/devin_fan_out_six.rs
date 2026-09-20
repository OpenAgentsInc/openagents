//! The golden reads back, and the run it records took the path the task
//! expects as far as a trace alone can show it.
//!
//! This is the test that keeps the golden honest. It parses the file with
//! the same `atif` code a live session writes with, so a golden that drifts
//! from the format fails here rather than the first time somebody tries to
//! read one.

mod common;

use coderbench::{Fault, Observed, Task, Verdict, goldens_dir, observe};

use common::{authored_run, authored_text, task};

fn golden() -> Observed {
    observe(&goldens_dir().join("devin-fan-out-six.atif.jsonl")).expect("golden observes")
}

#[test]
fn the_golden_reads_back_as_atif() {
    let path = goldens_dir().join("devin-fan-out-six.atif.jsonl");
    let recording = atif::log::read(&path).expect("golden parses as an ATIF log");
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(goldens_dir().join("devin-fan-out-six.meta.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(recording.session.id, meta["session_id"].as_str().unwrap());
    assert_eq!(
        recording.steps.len(),
        15,
        "every step of the path is recorded"
    );
    assert!(
        recording.whole(),
        "the golden is whole: {:?}",
        recording.faults
    );
    atif::log::read_whole(&path).expect("the golden reads as evidence");
    let document = recording.document();
    assert!(document.get("steps").is_some(), "the document renders");
}

/// A trace on its own cannot pass. It holds the path, and it does not hold
/// the exit code or the workspace, so the two faults left are about the
/// evidence rather than about the run. The trace here is the authored
/// fixture — the golden rewritten to the calls a sentence-driven run is
/// expected to make — so the delegation answers are the manifest's check
/// rather than a claim the recording makes about itself.
#[test]
fn a_trace_alone_is_unverifiable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authored.atif.jsonl");
    std::fs::write(&path, authored_text()).unwrap();
    let run = observe(&path).expect("the rewritten golden still reads");
    let task = task();
    for (delegation, want) in run.delegations.iter().zip(&task.grade.expects) {
        assert_eq!(delegation.prompt, want.prompt);
        assert_eq!(delegation.correct, None);
    }
    let judgment = task.judge(&run);
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

/// The observed trace carries the manifest's exact questions and answers.
/// The live driver's separate report establishes its successful exit and
/// unchanged workspace; reading the trace alone cannot re-observe either.
#[test]
fn the_observed_recording_matches_the_manifest_answers() {
    let run = golden();
    let task = task();
    assert_eq!(run.program.as_deref(), Some("delegate-fan-out"));
    assert_eq!(run.delegations.len(), 6);
    for (delegation, expected) in run.delegations.iter().zip(&task.grade.expects) {
        assert!(delegation.verified_against(expected), "{}", delegation.id);
    }
    assert!(run.writes.is_empty(), "no delegate reported writing");
    let judgment = task.judge(&run);
    assert_eq!(judgment.verdict, Verdict::Unverifiable);
    assert_eq!(judgment.faults.len(), 2, "{:?}", judgment.faults);
}

/// The authored fixture, judged end to end: the calls ask the request's
/// questions in order and answer them the way the manifest independently
/// holds, and nothing asserts its own correctness — the grade passes on
/// the check, not the claim.
///
/// This proves the grader against the shape a sentence-driven run is
/// expected to record; the fixture's synthetic driver facts are not live evidence.
#[test]
fn the_authored_run_passes_on_the_manifests_check() {
    let run = authored_run();
    for (delegation, want) in run.delegations.iter().zip(&task().grade.expects) {
        assert!(
            delegation.verified_against(want),
            "{} is checked against the task's answer",
            delegation.id
        );
    }
    let judgment = task().judge(&run);
    assert!(
        judgment.passed(),
        "independently verified answers on the expected path: {:?}",
        judgment.faults
    );
}

/// Whitespace at an answer's edges does not change it — a delegate that
/// answers `  3\n` answered 3 — while the answer's case and interior
/// spacing stay part of the value.
#[test]
fn whitespace_at_an_answers_edges_is_the_same_answer() {
    let task = task();
    let mut run = authored_run();
    run.delegations[5].output = "  3\n".to_string();
    assert!(run.delegations[5].verified_against(&task.grade.expects[5]));
    let judgment = task.judge(&run);
    assert!(judgment.passed(), "{:?}", judgment.faults);
}

#[test]
fn a_wrong_delegation_is_a_fault() {
    let mut run = authored_run();
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
    let mut run = authored_run();
    run.decisions.remove("independence");
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert_eq!(
        judgment.faults,
        vec![Fault::DecisionMissing {
            name: "independence".to_string()
        }],
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
        Provenance::Observed,
        "Coder produced the retained episode"
    );
    assert_eq!(
        meta.orchestrator, "coder",
        "the observed golden was driven by Coder"
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
    let task: Task = task();
    let judgment = task.judge(&Observed::default());
    let said: Vec<String> = judgment.faults.iter().map(ToString::to_string).collect();
    let mut wanted: Vec<String> = vec![
        "never ran the capability_probe check".to_string(),
        "never ran the program_registry check".to_string(),
        "selected program none, expected delegate-fan-out".to_string(),
        "never asked the program decision".to_string(),
        "never asked the independence decision".to_string(),
        "never ran the admission_check check".to_string(),
        "started 0 delegations, expected 6".to_string(),
    ];
    wanted.extend(
        task.grade
            .expects
            .iter()
            .map(|want| format!("no delegation asked {}", want.prompt)),
    );
    wanted.extend([
        "0 delegations are recorded correct, expected 6; 0 recorded nothing either way".to_string(),
        "nothing compared the workspace, so writing nothing is unobserved rather than shown"
            .to_string(),
        "the trace has no end record, so the session never closed".to_string(),
        "nothing observed how the episode ended; the task allows answered".to_string(),
    ]);
    assert_eq!(said, wanted);
    assert_eq!(judgment.verdict, Verdict::Failed);
}
