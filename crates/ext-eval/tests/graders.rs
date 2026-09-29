//! Every grader type on the fixture trajectories, with fake doors: no
//! network. Subject-only graders, a case whose every grader is
//! subject-only, errored runs, and failed door calls.

mod common;

use std::collections::BTreeSet;

use ext_eval::door::fake::{FakeDecisionDoor, FakeJudgeDoor, FakeReplayer};
use ext_eval::grade::{JUDGE_SYSTEM, judge_prompt};
use ext_eval::{
    Arm, Case, CaseFiles, DecisionAnswer, Doors, GraderResult, ReplayVerdict, RunFailure,
    RunOutcome, RunRecord, grade_run,
};
use indexmap::IndexMap;

const V: &str = "v = \"openagents.eval-case.v1\"";

/// A case with the given grader files and the find-callers prompt.
fn case(graders: &[(&str, &str)]) -> Case {
    Case::parse(
        "c",
        "evals/c",
        CaseFiles {
            prompt: format!("+++\n{V}\n+++\n\nWhich functions call `parse_case`?\n").into_bytes(),
            case_toml: None,
            graders: graders
                .iter()
                .map(|(name, text)| ((*name).to_string(), text.as_bytes().to_vec()))
                .collect(),
            fixtures: Vec::new(),
        },
    )
    .expect("the case parses")
}

fn record(arm: Arm, trajectory: &str) -> RunRecord {
    let mut record = RunRecord::new("c", arm, 1, RunOutcome::Completed);
    record.trajectory = Some(common::trajectory(trajectory));
    record
}

fn operations() -> BTreeSet<String> {
    BTreeSet::from([common::MAP.to_string()])
}

fn grade(case: &Case, record: &RunRecord, doors: Doors<'_>) -> Vec<GraderResult> {
    grade_run(case, record, &operations(), doors)
}

fn one(case: &Case, record: &RunRecord, doors: Doors<'_>) -> GraderResult {
    let mut results = grade(case, record, doors);
    assert_eq!(results.len(), 1, "{results:#?}");
    results.remove(0)
}

#[test]
fn regex_reads_the_last_message_the_trajectory_and_the_files() {
    let answer = case(&[(
        "a.md",
        "+++\ntype = \"regex\"\n+++\n\\bload\\b.*\\bparse_all\\b\n",
    )]);
    assert!(
        one(
            &answer,
            &record(Arm::Subject, "callers-by-grep"),
            Doors::default()
        )
        .passed
    );
    let wrong = one(
        &answer,
        &record(Arm::Subject, "callers-wrong"),
        Doors::default(),
    );
    assert!(!wrong.passed);
    assert_eq!(
        wrong.explanation,
        "0 matches in last_message; wanted at least one match"
    );

    let in_trajectory = case(&[(
        "a.md",
        "+++\ntype = \"regex\"\ntarget = \"trajectory\"\nmatch = \"count:1\"\n+++\ngrep -rn parse_case\n",
    )]);
    assert!(
        one(
            &in_trajectory,
            &record(Arm::Subject, "callers-by-grep"),
            Doors::default()
        )
        .passed
    );

    let not_in = case(&[(
        "a.md",
        "+++\ntype = \"regex\"\nmatch = \"not_contains\"\n+++\ncould not\n",
    )]);
    assert!(
        !one(
            &not_in,
            &record(Arm::Subject, "callers-wrong"),
            Doors::default()
        )
        .passed
    );

    let files = case(&[(
        "a.md",
        "+++\ntype = \"regex\"\ntarget = \"files\"\n+++\n^SUMMARY\\.md$\n",
    )]);
    let mut made = record(Arm::Subject, "summary");
    made.created_files = vec!["notes.txt".into(), "SUMMARY.md".into()];
    let flags = case(&[(
        "a.md",
        "+++\ntype = \"regex\"\ntarget = \"files\"\nflags = \"m\"\n+++\n^SUMMARY\\.md$\n",
    )]);
    assert!(
        !one(&files, &made, Doors::default()).passed,
        "no m flag: ^ is the start of the text"
    );
    assert!(one(&flags, &made, Doors::default()).passed);
}

#[test]
fn regex_reads_one_file_inside_the_workspace() {
    let grader = case(&[(
        "a.md",
        "+++\ntype = \"regex\"\ntarget = { file = \"SUMMARY.md\" }\n+++\n(?i)parser\n",
    )]);
    let mut run = record(Arm::Subject, "summary");
    run.workspace = Some(common::fixtures().join("workspaces/summary-good"));
    assert!(one(&grader, &run, Doors::default()).passed);
    run.workspace = Some(common::fixtures().join("workspaces/summary-empty"));
    assert!(!one(&grader, &run, Doors::default()).passed);
    run.workspace = None;
    let result = one(&grader, &run, Doors::default());
    assert!(!result.passed);
    assert!(
        result.explanation.contains("no workspace"),
        "{}",
        result.explanation
    );
}

#[cfg(unix)]
#[test]
fn a_file_focus_never_leaves_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("ws")).unwrap();
    std::fs::write(dir.path().join("secret.txt"), "parser").unwrap();
    std::os::unix::fs::symlink(dir.path().join("secret.txt"), dir.path().join("ws/link.md"))
        .unwrap();
    let grader = case(&[(
        "a.md",
        "+++\ntype = \"regex\"\ntarget = { file = \"link.md\" }\n+++\nparser\n",
    )]);
    let mut run = record(Arm::Subject, "summary");
    run.workspace = Some(dir.path().join("ws"));
    let result = one(&grader, &run, Doors::default());
    assert!(!result.passed);
    assert!(
        result.explanation.contains("outside the workspace"),
        "{}",
        result.explanation
    );
}

#[test]
fn operation_used_counts_matching_calls() {
    let used = case(&[(
        "u.md",
        "+++\ntype = \"operation_used\"\noperation = \"shell\"\ninput_match = \"^grep \"\n+++\n",
    )]);
    assert!(
        one(
            &used,
            &record(Arm::Subject, "callers-by-grep"),
            Doors::default()
        )
        .passed
    );
    let result = one(
        &used,
        &record(Arm::Subject, "callers-with-map"),
        Doors::default(),
    );
    assert!(!result.passed);
    assert_eq!(result.explanation, "shell ran 0 times; wanted at least 1");

    let never = case(&[(
        "n.md",
        "+++\ntype = \"operation_used\"\noperation = \"shell\"\nmin = 0\nmax = 0\n+++\n",
    )]);
    let result = one(
        &never,
        &record(Arm::Subject, "callers-by-grep"),
        Doors::default(),
    );
    assert!(!result.passed);
    assert_eq!(result.explanation, "shell ran 1 times; wanted exactly 0");

    // A Wasm guest operation and a program step name both count.
    let by_operation = case(&[(
        "m.md",
        "+++\ntype = \"operation_used\"\noperation = \"repo-map.map\"\narm = \"both\"\n+++\n",
    )]);
    assert!(
        one(
            &by_operation,
            &record(Arm::Subject, "callers-with-map"),
            Doors::default()
        )
        .passed
    );
    let by_step = case(&[(
        "m.md",
        "+++\ntype = \"operation_used\"\noperation = \"map\"\n+++\n",
    )]);
    assert!(
        one(
            &by_step,
            &record(Arm::Subject, "callers-with-map"),
            Doors::default()
        )
        .passed
    );
}

#[test]
fn operation_order_compares_first_calls() {
    let order = case(&[(
        "o.md",
        "+++\ntype = \"operation_order\"\nbefore = \"repo-map.map\"\nafter = \"shell\"\n+++\n",
    )]);
    assert!(
        one(
            &order,
            &record(Arm::Subject, "typo-with-map"),
            Doors::default()
        )
        .passed
    );
    let result = one(
        &order,
        &record(Arm::Subject, "typo-direct"),
        Doors::default(),
    );
    assert!(!result.passed);
    assert_eq!(result.explanation, "repo-map.map never ran");
    let reversed = case(&[(
        "o.md",
        "+++\ntype = \"operation_order\"\nbefore = \"shell\"\nafter = \"repo-map.map\"\n+++\n",
    )]);
    let result = one(
        &reversed,
        &record(Arm::Subject, "typo-with-map"),
        Doors::default(),
    );
    assert!(!result.passed);
    assert!(
        result.explanation.contains("ran first"),
        "{}",
        result.explanation
    );
}

#[test]
fn file_exists_matches_created_paths_by_glob() {
    let made = case(&[(
        "f.md",
        "+++\ntype = \"file_exists\"\npath = \"**/*.md\"\n+++\n",
    )]);
    let absent = case(&[(
        "f.md",
        "+++\ntype = \"file_exists\"\npath = \"*.log\"\nexists = false\n+++\n",
    )]);
    let mut run = record(Arm::Subject, "summary");
    run.created_files = vec!["docs/SUMMARY.md".into()];
    assert!(one(&made, &run, Doors::default()).passed);
    assert!(one(&absent, &run, Doors::default()).passed);
    run.created_files = vec!["debug.log".into()];
    assert!(!one(&made, &run, Doors::default()).passed);
    let result = one(&absent, &run, Doors::default());
    assert!(!result.passed);
    assert_eq!(result.explanation, "created debug.log, which matches *.log");
}

const DECISION: &str = "+++\ntype = \"decision\"\nquestion = \"Did the run name the callers?\"\nthreshold = 0.7\n+++\nIt names load and parse_all.\n";

#[test]
fn decision_needs_two_of_three_answers_at_the_threshold() {
    let grader = case(&[("d.md", DECISION)]);
    let run = record(Arm::Subject, "callers-with-map");
    let door = FakeDecisionDoor::scripted(vec![
        Ok(DecisionAnswer::Noul(0.9)),
        Ok(DecisionAnswer::Noul(0.4)),
        Ok(DecisionAnswer::Noul(0.7)),
    ]);
    let result = one(
        &grader,
        &run,
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    );
    assert!(result.passed, "{result:#?}");
    assert_eq!(result.votes.len(), 3);
    assert_eq!(
        result.explanation,
        "2 of 3 answers at or above 0.7; 2 needed"
    );
    let state = &door.calls()[0];
    assert_eq!(state["run"], common::RIGHT);
    assert_eq!(state["focus"], "last_message");
    assert!(state["task"].as_str().unwrap().contains("parse_case"));

    // Two answers decide it; the third is never asked.
    let door = FakeDecisionDoor::scripted(vec![
        Ok(DecisionAnswer::Noul(0.1)),
        Ok(DecisionAnswer::Noul(0.2)),
    ]);
    let result = one(
        &grader,
        &run,
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    );
    assert!(!result.passed);
    assert_eq!(door.calls().len(), 2);
}

#[test]
fn a_failed_decision_call_fails_the_grader_with_its_reason() {
    let grader = case(&[("d.md", DECISION)]);
    let run = record(Arm::Subject, "callers-with-map");
    let door = FakeDecisionDoor::scripted(vec![
        Ok(DecisionAnswer::Noul(0.9)),
        Err("quota_exhausted: the daily budget is spent".into()),
    ]);
    let result = one(
        &grader,
        &run,
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    );
    assert!(!result.passed);
    assert_eq!(
        result.explanation,
        "the decision door failed on call 2: quota_exhausted: the daily budget is spent"
    );
    let door = FakeDecisionDoor::scripted(vec![Ok(DecisionAnswer::Noul(1.7))]);
    let result = one(
        &grader,
        &run,
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    );
    assert!(
        result.explanation.contains("not a probability"),
        "{}",
        result.explanation
    );
    let result = one(&grader, &run, Doors::default());
    assert_eq!(result.explanation, "no decision door is configured");
}

#[test]
fn decision_reads_score_and_choice_answers() {
    let score = case(&[(
        "d.md",
        "+++\ntype = \"decision\"\nthreshold = 0.5\nfocus = \"trajectory\"\nquestion = { instructions = \"How well?\", levels = [\"not\", \"partly\", \"fully\"] }\n+++\n",
    )]);
    let run = record(Arm::Subject, "callers-with-map");
    let door = FakeDecisionDoor::answering(|_, _| Ok(DecisionAnswer::Score(1.6)));
    let result = one(
        &score,
        &run,
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    );
    assert!(result.passed, "1.6 of 2 is 0.8");
    let view = &door.calls()[0]["run"];
    assert_eq!(view["final_message"], common::RIGHT);
    assert_eq!(view["first_steps"].as_array().unwrap().len(), 3);

    let choice = case(&[(
        "d.md",
        "+++\ntype = \"decision\"\nthreshold = 0.6\n[question]\ninstructions = \"What did the run do?\"\npass = [\"named\"]\n[question.options]\nnamed = \"It named the callers\"\nguessed = \"It guessed\"\nnone = \"It found none\"\n+++\n",
    )]);
    let door = FakeDecisionDoor::answering(|_, _| {
        Ok(DecisionAnswer::Choice {
            choice: "named".into(),
            probabilities: IndexMap::from([
                ("named".to_string(), 0.55),
                ("guessed".to_string(), 0.35),
                ("none".to_string(), 0.10),
            ]),
        })
    });
    let result = one(
        &choice,
        &run,
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    );
    assert!(!result.passed, "0.55 on the passing option is under 0.6");
    let door = FakeDecisionDoor::answering(|_, _| Ok(DecisionAnswer::Noul(0.9)));
    let result = one(
        &choice,
        &run,
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    );
    assert!(
        result.explanation.contains("different question type"),
        "{}",
        result.explanation
    );
}

const JUDGE: &str =
    "+++\ntype = \"judge\"\n+++\nThe answer names load and parse_all and nothing else.\n";

#[test]
fn judge_needs_two_passes_and_no_fail() {
    let grader = case(&[("j.md", JUDGE)]);
    let run = record(Arm::Subject, "callers-with-map");
    let door = FakeJudgeDoor::scripted(vec![
        Ok("PASS".into()),
        Ok("pass.".into()),
        Ok("PASS".into()),
    ]);
    let result = one(
        &grader,
        &run,
        Doors {
            judge: Some(&door),
            ..Doors::default()
        },
    );
    assert!(result.passed);
    let calls = door.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].0, JUDGE_SYSTEM);
    assert_eq!(
        calls[0].1,
        judge_prompt(
            common::RIGHT,
            "The answer names load and parse_all and nothing else."
        )
    );
    assert!(
        calls[0]
            .1
            .ends_with("Answer only with \"PASS\" or \"FAIL\".")
    );

    // One FAIL fails it at once.
    let door = FakeJudgeDoor::scripted(vec![Ok("PASS".into()), Ok("FAIL".into())]);
    let result = one(
        &grader,
        &run,
        Doors {
            judge: Some(&door),
            ..Doors::default()
        },
    );
    assert!(!result.passed);
    assert_eq!(door.calls().len(), 2);

    // An answer that is neither word counts as neither.
    let door = FakeJudgeDoor::scripted(vec![
        Ok("PASS".into()),
        Ok("Probably fine".into()),
        Ok("PASS".into()),
    ]);
    let result = one(
        &grader,
        &run,
        Doors {
            judge: Some(&door),
            ..Doors::default()
        },
    );
    assert!(result.passed);
    assert_eq!(result.votes[1].answer, "Probably fine");
    let door = FakeJudgeDoor::scripted(vec![Ok("unsure".into()), Ok("unsure".into())]);
    let result = one(
        &grader,
        &run,
        Doors {
            judge: Some(&door),
            ..Doors::default()
        },
    );
    assert!(!result.passed);
    assert_eq!(
        door.calls().len(),
        2,
        "two misses leave no way to two passes"
    );
}

#[test]
fn a_failed_judge_call_fails_the_grader_with_its_reason() {
    let grader = case(&[("j.md", JUDGE)]);
    let run = record(Arm::Subject, "callers-with-map");
    let door = FakeJudgeDoor::scripted(vec![Err("auth_failed: the door refused the key".into())]);
    let result = one(
        &grader,
        &run,
        Doors {
            judge: Some(&door),
            ..Doors::default()
        },
    );
    assert!(!result.passed);
    assert_eq!(
        result.explanation,
        "the judge door failed on call 1: auth_failed: the door refused the key"
    );
    let result = one(&grader, &run, Doors::default());
    assert_eq!(result.explanation, "no judge door is configured");
}

#[test]
fn receipt_replays_every_recorded_invocation() {
    let grader = case(&[(
        "r.md",
        "+++\ntype = \"receipt\"\noperation = \"repo-map.map\"\narm = \"both\"\n+++\n",
    )]);
    let run = record(Arm::Subject, "callers-with-map");
    let key = |replayer: FakeReplayer| replayer;
    let passing = key(FakeReplayer::default().with(
        "c",
        Arm::Subject,
        1,
        common::MAP,
        Ok(vec![ReplayVerdict::Passed, ReplayVerdict::Passed]),
    ));
    let result = one(
        &grader,
        &run,
        Doors {
            replayer: Some(&passing),
            ..Doors::default()
        },
    );
    assert!(result.passed);
    assert_eq!(
        result.explanation,
        "2 receipts of repo-map.map replayed exactly"
    );

    let diverged = FakeReplayer::default().with(
        "c",
        Arm::Subject,
        1,
        common::MAP,
        Ok(vec![
            ReplayVerdict::Passed,
            ReplayVerdict::Failed {
                field: "fuel_consumed".into(),
                expected: "100".into(),
                actual: "120".into(),
            },
        ]),
    );
    let result = one(
        &grader,
        &run,
        Doors {
            replayer: Some(&diverged),
            ..Doors::default()
        },
    );
    assert!(!result.passed);
    assert!(
        result.explanation.contains("fuel_consumed was 100"),
        "{}",
        result.explanation
    );

    let unverifiable = FakeReplayer::default().with(
        "c",
        Arm::Subject,
        1,
        common::MAP,
        Ok(vec![ReplayVerdict::Unverifiable {
            reason: "engine differs".into(),
        }]),
    );
    let result = one(
        &grader,
        &run,
        Doors {
            replayer: Some(&unverifiable),
            ..Doors::default()
        },
    );
    assert!(!result.passed);
    assert!(result.explanation.contains("engine differs"));

    let none = FakeReplayer::default();
    let result = one(
        &grader,
        &run,
        Doors {
            replayer: Some(&none),
            ..Doors::default()
        },
    );
    assert_eq!(
        result.explanation,
        "the run recorded no receipt for repo-map.map"
    );
    let result = one(&grader, &run, Doors::default());
    assert_eq!(result.explanation, "no receipt replayer is configured");
}

#[test]
fn subject_only_graders_are_dropped_from_the_baseline_and_the_score() {
    let mixed = case(&[
        ("answer.md", "+++\ntype = \"regex\"\n+++\nparse_all\n"),
        // No explicit arm, on the extension's operation: subject-only.
        (
            "used-map.md",
            "+++\ntype = \"operation_used\"\noperation = \"repo-map.map\"\n+++\n",
        ),
        (
            "marked.md",
            "+++\ntype = \"regex\"\narm = \"subject-only\"\n+++\nload\n",
        ),
        // The same operation with arm = "both" runs in both arms.
        (
            "never-map.md",
            "+++\ntype = \"operation_used\"\noperation = \"repo-map.map\"\nmin = 0\nmax = 0\narm = \"both\"\n+++\n",
        ),
    ]);
    assert!(!mixed.subject_only(&operations()));
    let subject = grade(
        &mixed,
        &record(Arm::Subject, "callers-with-map"),
        Doors::default(),
    );
    let scored: Vec<(&str, bool)> = subject
        .iter()
        .map(|r| (r.name.as_str(), r.scored))
        .collect();
    assert_eq!(
        scored,
        [
            ("answer", true),
            ("marked", false),
            ("never-map", true),
            ("used-map", false)
        ]
    );
    let baseline = grade(
        &mixed,
        &record(Arm::Baseline, "callers-by-grep"),
        Doors::default(),
    );
    let names: Vec<&str> = baseline.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        ["answer", "never-map"],
        "subject-only graders don't run without the tool"
    );
}

#[test]
fn a_case_whose_every_grader_is_subject_only_scores_the_subject_arm_alone() {
    let only = case(&[(
        "used.md",
        "+++\ntype = \"operation_used\"\noperation = \"repo-map.map\"\n+++\n",
    )]);
    assert!(only.subject_only(&operations()));
    let subject = grade(
        &only,
        &record(Arm::Subject, "overview-with-map"),
        Doors::default(),
    );
    assert_eq!(subject.len(), 1);
    assert!(subject[0].scored, "scored in the subject arm");
    assert!(subject[0].passed);
    assert!(
        grade(
            &only,
            &record(Arm::Baseline, "overview-by-ls"),
            Doors::default()
        )
        .is_empty()
    );

    // In the fixture suite, map-only is planned in the subject arm alone and
    // left out of the change.
    let suite = common::suite();
    let plan = common::plan(None);
    let map_only = suite.case("map-only").unwrap();
    assert!(!plan.baseline_runs(map_only));
    let attempts = plan.attempts(&suite);
    assert!(
        attempts
            .iter()
            .any(|(case, arm, _)| case == "map-only" && *arm == Arm::Subject)
    );
    assert!(
        !attempts
            .iter()
            .any(|(case, arm, _)| case == "map-only" && *arm == Arm::Baseline)
    );
}

#[test]
fn an_errored_run_fails_every_grader_with_its_reason_and_calls_no_door() {
    let grader = case(&[
        ("d.md", DECISION),
        ("a.md", "+++\ntype = \"regex\"\n+++\nload\n"),
    ]);
    let door = FakeDecisionDoor::answering(|_, _| Ok(DecisionAnswer::Noul(1.0)));
    for failure in [
        RunFailure::Timeout,
        RunFailure::Refused,
        RunFailure::CostCeiling,
        RunFailure::AuthFailed,
        RunFailure::EnvVarRejected,
        RunFailure::UnconfinedHost,
    ] {
        let run = RunRecord::new("c", Arm::Subject, 1, RunOutcome::Errored(failure));
        let results = grade(
            &grader,
            &run,
            Doors {
                decision: Some(&door),
                ..Doors::default()
            },
        );
        assert_eq!(results.len(), 2);
        for result in results {
            assert!(!result.passed);
            assert_eq!(
                result.explanation,
                format!("the run ended with {}", failure.word())
            );
        }
    }
    assert!(door.calls().is_empty());
    for outcome in [RunOutcome::Cancelled, RunOutcome::Unknown] {
        let run = RunRecord::new("c", Arm::Subject, 1, outcome);
        assert!(
            grade(&grader, &run, Doors::default()).is_empty(),
            "not graded"
        );
    }
}

#[test]
fn a_run_without_a_trajectory_fails_the_graders_that_read_one() {
    let grader = case(&[
        ("a.md", "+++\ntype = \"regex\"\n+++\nload\n"),
        (
            "u.md",
            "+++\ntype = \"operation_used\"\noperation = \"shell\"\n+++\n",
        ),
        (
            "o.md",
            "+++\ntype = \"operation_order\"\nbefore = \"a\"\nafter = \"b\"\n+++\n",
        ),
    ]);
    let run = RunRecord::new("c", Arm::Subject, 1, RunOutcome::Completed);
    for result in grade(&grader, &run, Doors::default()) {
        assert!(!result.passed);
        assert_eq!(
            result.explanation, "the run wrote no trajectory",
            "{}",
            result.name
        );
    }
}
