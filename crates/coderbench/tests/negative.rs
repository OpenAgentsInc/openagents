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
//! describes a trace nothing writes any more. Cases about the delegation
//! answers use [`common::authored_text`], an authored fixture: the golden
//! rewritten to carry the request's list items as prompts and no
//! self-asserted correctness, so the manifest's `expects` are the only
//! check. It is not a recording — the run it stands in for happens after
//! openagents#9427.

mod common;

use std::path::{Path, PathBuf};

use coderbench::{
    Asked, Check, Delegation, Ending, Fault, Observed, Task, Verdict, Workspace, goldens_dir,
    observe,
};
use serde_json::Value;

use common::{authored_run, authored_text, task};

/// The same task without the answers it owns, which is the shape a task
/// takes when the request cannot say what correct is. The trace's own
/// `correct` flag is the only correctness evidence there is, and the
/// stricter evidence rule still applies.
fn task_without_expectations() -> Task {
    let mut task = task();
    task.grade.expects.clear();
    task
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
                prompt: String::new(),
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

/// Six delegations that asked nothing the task expects are six delegations
/// that are not the task's, and a task requiring six correct answers does
/// not get them.
#[test]
fn six_ungraded_delegations_are_not_six_correct_ones() {
    let judgment = task().judge(&the_audits_run());
    assert!(
        !judgment.passed(),
        "the run the audit constructed still grades clean: {:?}",
        judgment.faults
    );
    let misattributed = judgment
        .faults
        .iter()
        .filter(|fault| matches!(fault, Fault::DelegationMisattributed { .. }))
        .count();
    assert_eq!(
        misattributed, 6,
        "each delegation that asked nothing is named"
    );
    assert!(
        judgment.faults.iter().any(|fault| matches!(
            fault,
            Fault::DelegationsCorrect {
                expected: 6,
                verified: 0,
                unverified: 0
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

/// A task that states no answers of its own still needs the trace's own
/// correctness evidence: six delegations that recorded nothing are six
/// nobody checked, which is `unverifiable` rather than a pass and rather
/// than a failure — nobody looked, and nothing measured them wrong.
#[test]
fn an_unchecked_run_is_unverifiable_rather_than_clean() {
    let mut run = the_audits_run();
    run.closed = true;
    run.ending = Ending::Answered;
    run.workspace = Some(Workspace::default());
    let judgment = task_without_expectations().judge(&run);
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
    let text: String = authored_text()
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
    let lines: Vec<String> = authored_text().lines().map(str::to_string).collect();
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
    let lines: Vec<String> = authored_text().lines().map(str::to_string).collect();
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
    let mut run = authored_run();
    run.ending = Ending::TimedOut;
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
    let mut run = authored_run();
    run.ending = Ending::Declined;
    assert_eq!(task().judge(&run).verdict, Verdict::Failed);
}

/// The required steps in the wrong order are the required steps in the
/// wrong order. A run that admitted a delegation before it probed for the
/// executor did not establish what the admission claims.
#[test]
fn reordered_steps_are_a_fault() {
    let directory = tempfile::tempdir().unwrap();
    let mut lines: Vec<String> = authored_text().lines().map(str::to_string).collect();
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
    let text = authored_text().replace("0.93", "0.53");
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
/// says — on a task that owns no answers of its own, where the flag is the
/// only correctness evidence there is.
#[test]
fn a_correct_flag_on_an_empty_answer_is_not_a_check() {
    let mut run = observe(&goldens_dir().join("devin-fan-out-six.atif.jsonl")).unwrap();
    run.ending = Ending::Answered;
    run.workspace = Some(Workspace::default());
    run.delegations[0].output = String::new();
    let judgment = task_without_expectations().judge(&run);
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
    let mut run = authored_run();
    run.delegations[3].outcome = atif::Outcome::Failed;
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert!(
        judgment
            .faults
            .iter()
            .any(|fault| matches!(fault, Fault::DelegationFailed { .. })),
        "{:?}",
        judgment.faults
    );
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

/// The trace calling a right answer wrong contradicts the manifest's own
/// copy, and a record that contradicts it is a fault rather than a pass.
#[test]
fn a_delegation_the_trace_calls_wrong_is_a_fault() {
    let mut run = authored_run();
    run.delegations[4].correct = Some(false);
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert!(
        judgment
            .faults
            .iter()
            .any(|fault| matches!(fault, Fault::DelegationWrong { .. })),
        "{:?}",
        judgment.faults
    );
}

/// A self-asserted `correct` flag cannot count an answer the task's own
/// expectation disagrees with. The flag is the run's claim; the manifest
/// is the check.
#[test]
fn a_self_asserted_correct_flag_is_not_the_check() {
    let mut run = authored_run();
    run.delegations[1].output = "calibration, locked".to_string();
    run.delegations[1].correct = Some(true);
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert!(
        judgment.faults.iter().any(|fault| matches!(
            fault,
            Fault::DelegationAnswered { wanted, .. }
                if wanted == "calibration, development, locked"
        )),
        "the wrong answer is named against the task's own: {:?}",
        judgment.faults
    );
}

/// A recorded answer that is not the task's answer is measured and wrong —
/// an empty one included, because the task holding the answer is what makes
/// the difference between a wrong answer and an unchecked one.
#[test]
fn a_wrong_or_empty_answer_is_a_fault() {
    for output in ["6", ""] {
        let mut run = authored_run();
        run.delegations[0].output = output.to_string();
        let judgment = task().judge(&run);
        assert_eq!(judgment.verdict, Verdict::Failed, "output {output:?}");
        assert!(
            judgment.faults.iter().any(|fault| matches!(
                fault,
                Fault::DelegationAnswered { wanted, .. } if wanted == "5"
            )),
            "{:?}",
            judgment.faults
        );
    }
}

/// A run short a delegation is missing the question that delegation owed,
/// not six-fifths of a fan-out.
#[test]
fn a_missing_delegation_is_named() {
    let task = task();
    let mut run = authored_run();
    run.delegations.pop();
    let judgment = task.judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert!(
        judgment.faults.iter().any(|fault| matches!(
            fault,
            Fault::DelegationCount {
                expected: 6,
                found: 5
            }
        )),
        "{:?}",
        judgment.faults
    );
    assert!(
        judgment.faults.iter().any(|fault| matches!(
            fault,
            Fault::DelegationMissing { prompt } if prompt == &task.grade.expects[5].prompt
        )),
        "the question that went unasked is named: {:?}",
        judgment.faults
    );
}

/// The same prompt twice leaves another question unasked, and a duplicated
/// delegation is not the one the task expects in its place.
#[test]
fn a_duplicated_delegation_is_not_two_answers() {
    let task = task();
    let mut run = authored_run();
    run.delegations[3] = run.delegations[0].clone();
    let judgment = task.judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert!(
        judgment.faults.iter().any(|fault| matches!(
            fault,
            Fault::DelegationMisattributed { wanted, found, .. }
                if wanted == &task.grade.expects[3].prompt
                    && found == &task.grade.expects[0].prompt
        )),
        "the duplicate is named against the question it displaced: {:?}",
        judgment.faults
    );
}

/// The same six answers in another order are the wrong delegations in the
/// places that do not happen to match, because the task pins the order.
#[test]
fn reordered_delegations_are_not_the_task() {
    let task = task();
    let mut run = authored_run();
    run.delegations.swap(0, 1);
    let judgment = task.judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert_eq!(
        judgment
            .faults
            .iter()
            .filter(|fault| matches!(fault, Fault::DelegationMisattributed { .. }))
            .count(),
        2,
        "each displaced delegation is named: {:?}",
        judgment.faults
    );
}

/// The prompt is the delegation's identity, and the identity is exact:
/// a recorded call that asked the question with a letter's case or a
/// space changed asked a different question, however it answered —
/// `pub struct` and `Pub Struct` do not name the same declaration.
#[test]
fn a_prompt_that_differs_in_case_or_spacing_is_a_different_question() {
    let task = task();
    let expected = &task.grade.expects[0].prompt;
    for prompt in [
        expected.replace("pub struct", "Pub Struct"),
        expected.replacen("? Answer", "?  Answer", 1),
    ] {
        assert_ne!(&prompt, expected, "the case changed something");
        let mut run = authored_run();
        run.delegations[0].prompt = prompt.clone();
        let judgment = task.judge(&run);
        assert_eq!(judgment.verdict, Verdict::Failed, "prompt {prompt:?}");
        assert!(
            judgment.faults.iter().any(|fault| matches!(
                fault,
                Fault::DelegationMisattributed { found, .. } if found == &prompt
            )),
            "the near-miss prompt is named as a different question: {:?}",
            judgment.faults
        );
    }
}

/// Answers compare with their case intact: `l1, l2, l3` is not
/// `L1, L2, L3` — variant names change meaning with their letters.
#[test]
fn an_answer_that_differs_only_in_case_is_a_different_answer() {
    let mut run = authored_run();
    run.delegations[2].output = "l1, l2, l3".to_string();
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert!(
        judgment.faults.iter().any(|fault| matches!(
            fault,
            Fault::DelegationAnswered { wanted, found, .. }
                if wanted == "L1, L2, L3" && found == "l1, l2, l3"
        )),
        "{:?}",
        judgment.faults
    );
}

/// An `expects` list that cannot check a run cannot pass one, wherever
/// the task came from: a blank prompt is no identity, a blank answer owes
/// nothing — and compared to an empty output would "verify" a delegation
/// that said nothing — a repeated prompt leaves a question unasked, and a
/// count that disagrees with the pinned list states different work.
#[test]
fn malformed_expectations_fault_rather_than_pass() {
    let cases: Vec<(&str, Task, &str)> = {
        let mut blank_prompt = task();
        blank_prompt.grade.expects[0].prompt = "   ".to_string();
        let mut blank_answer = task();
        blank_answer.grade.expects[2].answer = String::new();
        let mut repeated_prompt = task();
        repeated_prompt.grade.expects[3] = repeated_prompt.grade.expects[0].clone();
        let mut short_list = task();
        short_list.grade.expects.pop();
        let mut partial_credit = task();
        partial_credit.grade.delegations_correct = 4;
        vec![
            ("a blank prompt", blank_prompt, "expects[0] pins no prompt"),
            ("a blank answer", blank_answer, "expects[2] pins no answer"),
            (
                "a repeated prompt",
                repeated_prompt,
                "expects[3] repeats a prompt an earlier entry pins",
            ),
            (
                "fewer pins than delegations",
                short_list,
                "expects pins 5 answers for 6 delegations; it owes one per delegation",
            ),
            (
                "asking for fewer than are pinned",
                partial_credit,
                "delegations_correct is 4, but 6 pinned answers means all 6 must verify",
            ),
        ]
    };
    for (name, task, why) in cases {
        // The run is the clean authored one, so the only thing wrong is
        // the manifest itself.
        let judgment = task.judge(&authored_run());
        assert_eq!(judgment.verdict, Verdict::Failed, "{name}");
        assert!(
            judgment.faults.iter().any(|fault| matches!(
                fault,
                Fault::ExpectationMalformed { why: said } if said == why
            )),
            "{name} is named: {:?}",
            judgment.faults
        );
    }
}

/// A manifest file with the same problems never reaches `judge` at all:
/// `Task::load` refuses it rather than grade against half an expectation.
#[test]
fn a_malformed_manifest_does_not_load() {
    let directory = tempfile::tempdir().unwrap();
    let mut manifest = serde_json::to_value(task()).unwrap();
    manifest["grade"]["expects"][2]["answer"] = Value::String(String::new());
    let path = directory.path().join("malformed.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    let why = Task::load(&path).expect_err("a blank expected answer refuses to load");
    assert!(why.contains("pins no answer"), "{why}");
}

/// Underneath the manifest checks: an expectation with a blank prompt or
/// a blank answer verifies nothing even when a delegation hands back
/// nothing, so the pair cannot silently count each other.
#[test]
fn an_empty_expectation_verifies_nothing() {
    let run = authored_run();
    let mut blank = run.delegations[0].clone();
    blank.output = String::new();
    for want in [
        coderbench::ExpectedAnswer {
            prompt: String::new(),
            answer: "5".to_string(),
        },
        coderbench::ExpectedAnswer {
            prompt: blank.prompt.clone(),
            answer: " ".to_string(),
        },
        coderbench::ExpectedAnswer {
            prompt: String::new(),
            answer: String::new(),
        },
    ] {
        assert!(
            !blank.verified_against(&want),
            "{want:?} cannot verify an empty output"
        );
    }
}

/// A write nobody mentioned is still a write, and a workspace nobody read
/// is not a workspace that holds still.
#[test]
fn writes_are_judged_against_the_workspace() {
    let mut run = authored_run();
    assert!(
        run.writes.is_empty(),
        "no delegate in the golden reported writing"
    );

    // Nobody looked: unknown, and unknown is not a pass.
    run.workspace = None;
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
