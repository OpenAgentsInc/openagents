//! Runs that must not grade clean.
//!
//! A grader with only a passing fixture is a grader nobody has tested. Each
//! case here is a run that holds the names the task asks for and is missing
//! something the grade depends on, and each one names the fault it owes and
//! the [`Verdict`] that fault carries.
//!
//! The first case is the audit's: a constructed run with six ungraded
//! delegations, null decision answers, and the required check names, which
//! the grader accepted with no faults at all. Audit A04, openagents#9418.
//!
//! The trace cases are built by changing one thing in the golden, so they
//! stay in step with the format rather than drifting into a fixture that
//! describes a trace nothing writes any more.

use std::path::{Path, PathBuf};

use coderbench::{
    Asked, Check, Delegation, Ending, Fault, Observed, Task, Verdict, Workspace, goldens_dir,
    observe, tasks_dir,
};
use serde_json::Value;

fn task() -> Task {
    Task::load(&tasks_dir().join("devin-fan-out-six").join("task.json"))
        .expect("task manifest loads")
}

fn golden_text() -> String {
    std::fs::read_to_string(goldens_dir().join("devin-fan-out-six.atif.jsonl"))
        .expect("the golden reads")
}

/// Writes a changed golden and reads it back the way a run would, with the
/// two facts only a driver sees supplied: the turn answered, and the
/// checkout is unchanged. What is left wrong is what the case changed.
fn observed(directory: &Path, name: &str, text: &str) -> Observed {
    let path: PathBuf = directory.join(format!("{name}.atif.jsonl"));
    std::fs::write(&path, text).unwrap();
    let mut run = observe(&path).expect("the changed golden still reads");
    run.ending = Ending::Answered;
    run.workspace = Some(Workspace::default());
    run
}

/// The audit's constructed run: six delegations that started and recorded
/// nothing about whether they were right, decisions with null answers, and
/// the required check names.
///
/// Every name the task asks for is present. Nothing about the run was
/// checked. This returned an empty fault list.
fn the_audits_run() -> Observed {
    let task = task();
    Observed {
        program: Some(task.grade.program.clone()),
        delegations: (0..task.grade.delegations)
            .map(|i| Delegation {
                id: i.to_string(),
                output: String::new(),
                milliseconds: 0,
                outcome: atif::Outcome::Completed,
                correct: None,
            })
            .collect(),
        decisions: task
            .grade
            .decisions
            .iter()
            .map(|name| {
                (
                    name.clone(),
                    Asked {
                        answers: Value::Null,
                        outcome: atif::Outcome::Completed,
                    },
                )
            })
            .collect(),
        checks: task
            .grade
            .checks
            .iter()
            .map(|name| Check {
                name: name.clone(),
                outcome: atif::Outcome::Completed,
            })
            .collect(),
        ..Observed::default()
    }
}

/// Six delegations that recorded nothing are six delegations nobody
/// checked, and a task requiring six correct answers does not get them.
#[test]
fn six_ungraded_delegations_are_not_six_correct_ones() {
    let judgment = task().judge(&the_audits_run());
    assert!(
        !judgment.passed(),
        "the run the audit constructed still grades clean: {:?}",
        judgment.faults
    );
    let unverified = judgment
        .faults
        .iter()
        .filter(|fault| matches!(fault, Fault::DelegationUnverified { .. }))
        .count();
    assert_eq!(unverified, 6, "each unchecked delegation is named");
    assert!(
        judgment.faults.iter().any(|fault| matches!(
            fault,
            Fault::DelegationsCorrect {
                expected: 6,
                verified: 0,
                unverified: 6
            }
        )),
        "the shortfall against delegations_correct is stated: {:?}",
        judgment.faults
    );
    assert!(
        judgment
            .faults
            .iter()
            .any(|fault| matches!(fault, Fault::DecisionUnanswered { .. })),
        "a decision that answered null did not answer: {:?}",
        judgment.faults
    );
}

/// The same run with a whole trace behind it and a turn that answered. What
/// is left is only the evidence nobody has, so the verdict is
/// `unverifiable` rather than `passed`.
#[test]
fn an_unchecked_run_is_unverifiable_rather_than_clean() {
    let mut run = the_audits_run();
    run.closed = true;
    run.ending = Ending::Answered;
    run.workspace = Some(Workspace::default());
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Unverifiable);
    assert!(
        judgment
            .faults
            .iter()
            .all(|fault| fault.verdict() == Verdict::Unverifiable),
        "nothing here was measured and wrong: {:?}",
        judgment.faults
    );
}

/// A check that ran under the expected name and failed is not a check that
/// ran.
#[test]
fn a_named_check_that_failed_is_a_fault() {
    let directory = tempfile::tempdir().unwrap();
    let text: String = golden_text()
        .lines()
        .map(|line| {
            if line.contains(r#""name": "capability_probe""#) {
                line.replacen(r#""outcome": "Completed""#, r#""outcome": "Failed""#, 1)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let judgment = task().judge(&observed(directory.path(), "failed-check", &text));
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert_eq!(
        judgment.faults,
        vec![Fault::CheckFailed {
            name: "capability_probe".to_string(),
            outcome: "failed".to_string(),
        }],
        "the probe is named, and nothing else changed"
    );
}

/// A trace with a line that did not read back is a trace with a hole in it,
/// and a hole is unknown rather than fine.
#[test]
fn a_torn_trace_is_unverifiable() {
    let directory = tempfile::tempdir().unwrap();
    let lines: Vec<String> = golden_text().lines().map(str::to_string).collect();
    // A line the writer did not finish, which is what a session killed
    // mid-write leaves behind.
    let torn = r#"{"record": "step", "step": {"at": 1789869709018, "source": "Agent", "mess"#;
    let mut with_hole = lines.clone();
    with_hole.insert(lines.len() - 1, torn.to_string());
    let judgment = task().judge(&observed(directory.path(), "torn", &with_hole.join("\n")));
    assert_eq!(judgment.verdict, Verdict::Unverifiable);
    assert_eq!(judgment.faults, vec![Fault::TornTrace { lines: 1 }]);
}

/// A trace with no end record is a session that never closed, whatever the
/// steps in it say.
#[test]
fn a_trace_without_an_end_record_is_a_fault() {
    let directory = tempfile::tempdir().unwrap();
    let lines: Vec<String> = golden_text().lines().map(str::to_string).collect();
    let stopped = lines[..lines.len() - 1].join("\n");
    let run = observed(directory.path(), "stopped", &stopped);
    assert!(!run.closed);
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert_eq!(judgment.faults, vec![Fault::Unfinished]);
}

/// A run that ran past the timeout ended a way the task does not allow,
/// however complete its steps look. The trace here is the whole golden.
#[test]
fn a_timed_out_ending_is_a_fault() {
    let mut run = observe(&goldens_dir().join("devin-fan-out-six.atif.jsonl")).unwrap();
    run.ending = Ending::TimedOut;
    run.workspace = Some(Workspace::default());
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert_eq!(
        judgment.faults,
        vec![Fault::Ended {
            found: "timed_out".to_string(),
            allowed: vec!["answered".to_string()],
        }]
    );
}

/// A declined turn is a result rather than a failure, and it is still not
/// this task's result.
#[test]
fn a_declined_ending_is_a_fault_for_a_task_that_does_not_allow_it() {
    let mut run = observe(&goldens_dir().join("devin-fan-out-six.atif.jsonl")).unwrap();
    run.ending = Ending::Declined;
    run.workspace = Some(Workspace::default());
    assert_eq!(task().judge(&run).verdict, Verdict::Failed);
}

/// The required steps in the wrong order are the required steps in the
/// wrong order. A run that admitted a delegation before it probed for the
/// executor did not establish what the admission claims.
#[test]
fn reordered_steps_are_a_fault() {
    let directory = tempfile::tempdir().unwrap();
    let mut lines: Vec<String> = golden_text().lines().map(str::to_string).collect();
    let probe = lines
        .iter()
        .position(|line| line.contains(r#""name": "capability_probe""#))
        .unwrap();
    let admit = lines
        .iter()
        .position(|line| line.contains(r#""name": "admission_check""#))
        .unwrap();
    lines.swap(probe, admit);
    let judgment = task().judge(&observed(directory.path(), "reordered", &lines.join("\n")));
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert_eq!(
        judgment
            .faults
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec![
            "ran program_registry before capability_probe, which the path puts first",
            "ran admission_check before independence, which the path puts first",
        ]
    );
}

/// A decision that was asked, completed, and answered the wrong way is a
/// fault about the answer rather than about the call.
#[test]
fn a_decision_that_went_the_wrong_way_is_a_fault() {
    let directory = tempfile::tempdir().unwrap();
    let text = golden_text().replace("0.93", "0.53");
    let judgment = task().judge(&observed(directory.path(), "unsure", &text));
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert_eq!(
        judgment
            .faults
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec!["the independence decision answered independent 0.53, expected at least 0.7"]
    );
}

/// A delegation the trace says completed with nothing to show for it is not
/// a delegation anybody checked, whatever the `correct` flag beside it
/// says.
#[test]
fn a_correct_flag_on_an_empty_answer_is_not_a_check() {
    let mut run = observe(&goldens_dir().join("devin-fan-out-six.atif.jsonl")).unwrap();
    run.ending = Ending::Answered;
    run.workspace = Some(Workspace::default());
    run.delegations[0].output = String::new();
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Unverifiable);
    assert!(
        judgment
            .faults
            .iter()
            .any(|fault| matches!(fault, Fault::DelegationUnverified { .. })),
        "{:?}",
        judgment.faults
    );
}

/// A delegation that failed is a failure rather than an unknown, and it
/// does not count toward the correct answers the task requires.
#[test]
fn a_delegation_that_failed_is_a_fault() {
    let mut run = observe(&goldens_dir().join("devin-fan-out-six.atif.jsonl")).unwrap();
    run.ending = Ending::Answered;
    run.workspace = Some(Workspace::default());
    run.delegations[3].outcome = atif::Outcome::Failed;
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert!(
        judgment.faults.iter().any(|fault| matches!(
            fault,
            Fault::DelegationsCorrect {
                expected: 6,
                verified: 5,
                unverified: 0
            }
        )),
        "{:?}",
        judgment.faults
    );
}

/// A write nobody mentioned is still a write, and a workspace nobody read
/// is not a workspace that holds still.
#[test]
fn writes_are_judged_against_the_workspace() {
    let path = goldens_dir().join("devin-fan-out-six.atif.jsonl");
    let mut run = observe(&path).unwrap();
    run.ending = Ending::Answered;
    assert!(
        run.writes.is_empty(),
        "no delegate in the golden reported writing"
    );

    // Nobody looked: unknown, and unknown is not a pass.
    assert_eq!(task().judge(&run).verdict, Verdict::Unverifiable);

    // Somebody looked and found a file: measured, and wrong.
    run.workspace = Some(Workspace {
        changed: vec!["souvenir.txt".to_string()],
    });
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert_eq!(
        judgment.faults,
        vec![Fault::UnexpectedWrite {
            path: "souvenir.txt".to_string(),
        }]
    );
}
