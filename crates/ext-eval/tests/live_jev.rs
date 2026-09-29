//! One live call path through the Jev decision door. Ignored by default: it
//! spends quota and needs a key.
//!
//! ```sh
//! TYPESAFE_ENV_FILE=/path/to/typesafe.env \
//!   cargo test -p ext-eval --test live_jev -- --ignored
//! ```
//!
//! The key comes from `TYPESAFE_API_KEY`, or from a `TYPESAFE_API_KEY=` line
//! in the file `TYPESAFE_ENV_FILE` names. It is never printed.

mod common;

use std::collections::BTreeSet;

use ext_eval::{Arm, Case, CaseFiles, Doors, JevDoor, RunOutcome, RunRecord, grade_run};

fn key() -> String {
    if let Some(key) = std::env::var("TYPESAFE_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
    {
        return key;
    }
    let path = std::env::var("TYPESAFE_ENV_FILE")
        .expect("set TYPESAFE_API_KEY or TYPESAFE_ENV_FILE to run the live test");
    let text = std::fs::read_to_string(&path).expect("the env file reads");
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("TYPESAFE_API_KEY="))
        .map(|value| value.trim().trim_matches('"').to_string())
        .next()
        .expect("the env file has a TYPESAFE_API_KEY line")
}

#[test]
#[ignore = "calls the live Jev door and spends quota"]
fn a_live_decision_grader_tells_a_right_answer_from_a_wrong_one() {
    let door = JevDoor::new(jev::Config::new().api_key(key()), None).expect("the client builds");
    let case = Case::parse(
        "find-callers",
        "evals/find-callers",
        CaseFiles {
            prompt: b"+++\nv = \"openagents.eval-case.v1\"\n+++\n\nWhich functions call `parse_case` in this repository? Answer with their names.\n".to_vec(),
            graders: vec![(
                "judged.md".into(),
                b"+++\ntype = \"decision\"\nquestion = \"Did the run's answer name both `load` and `parse_all` as the functions that call `parse_case`?\"\nthreshold = 0.5\n+++\n\nThe answer names `load` and `parse_all` as the callers.\n".to_vec(),
            )],
            ..CaseFiles::default()
        },
    )
    .expect("the case parses");
    let doors = Doors {
        decision: Some(&door),
        ..Doors::default()
    };
    let grade = |trajectory: &str| {
        let mut record = RunRecord::new("find-callers", Arm::Subject, 1, RunOutcome::Completed);
        record.trajectory = Some(common::trajectory(trajectory));
        grade_run(&case, &record, &BTreeSet::new(), doors).remove(0)
    };
    let right = grade("callers-with-map");
    assert!(right.passed, "{right:#?}");
    let wrong = grade("callers-wrong");
    assert!(!wrong.passed, "{wrong:#?}");
}
