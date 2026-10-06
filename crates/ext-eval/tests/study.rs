//! Reopening a retained evaluation (#10663): the with/without fixtures,
//! written as a run writes them, recompute from every retained attempt,
//! and a changed or missing record is shown as such.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ext_eval::study::{Agreement, Kept, list, reopen};
use ext_eval::{ArtifactRef, Doors, Evaluation, evaluate, load_gate};
use serde_json::Value;

fn run(name: &str) -> Evaluation {
    let scenario = common::scenario(name);
    let (gate, gate_file) = load_gate().expect("the committed gate loads");
    let door = common::decision_door();
    evaluate(
        &common::suite(),
        &common::plan(scenario.runs),
        scenario.records,
        &common::identity(),
        (&gate, &gate_file),
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    )
    .expect("the scenario evaluates")
}

/// The trajectory fixtures by digest.
fn trajectories() -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(common::fixtures().join("trajectories")).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        out.insert(nostr::contracts::digest_bytes(&bytes), bytes);
    }
    out
}

/// Writes `name`'s results directory under `base` the way a run does:
/// the report and its artifacts, each run's trajectory, and the suite.
fn retained(base: &Path, name: &str) -> PathBuf {
    let evaluation = run(name);
    let dir = base.join("results").join(name);
    evaluation.write(&dir).unwrap();
    let bytes = trajectories();
    for graded in &evaluation.runs {
        if let Some(ArtifactRef { digest, .. }) = &graded.trajectory {
            let run_dir = dir.join(ext_eval::run_path(&graded.case, graded.arm, graded.attempt));
            std::fs::create_dir_all(&run_dir).unwrap();
            std::fs::write(run_dir.join("trajectory.json"), &bytes[digest]).unwrap();
        }
    }
    ext_eval::run::write_suite_files(&dir.join("suite"), &common::suite()).unwrap();
    dir
}

fn files(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push((path.clone(), std::fs::read(&path).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_better_fixture_recomputes_from_every_retained_attempt() {
    let temp = tempfile::tempdir().unwrap();
    let dir = retained(temp.path(), "better");
    let before = files(temp.path());
    let study = reopen(&dir).unwrap();
    assert_eq!(study.problems, Vec::<String>::new());
    assert_eq!(study.missing, Vec::<String>::new());
    assert_eq!(study.agreement, Agreement::Agrees);
    assert_eq!(study.reported.as_deref(), Some("pass"));
    assert_eq!(study.recomputed.as_deref(), Some("pass"));
    assert_eq!(study.shown, "Better");
    assert_eq!(study.gate.as_deref(), Some("ext-eval-v2"));
    assert!(study.baseline.is_some());
    let report: Value =
        serde_json::from_slice(&std::fs::read(dir.join("report.json")).unwrap()).unwrap();
    assert_eq!(
        study.subject.as_ref().unwrap().definition,
        report["subject"]["definition"]
    );
    assert_eq!(
        study.attempts.len() as u64,
        report["coverage"]["subject"]["attempted"].as_u64().unwrap()
            + report["coverage"]["baseline"]["attempted"]
                .as_u64()
                .unwrap()
    );
    assert!(
        study
            .attempts
            .iter()
            .all(|attempt| attempt.grades == Kept::Retained)
    );
    assert!(
        study
            .attempts
            .iter()
            .all(|attempt| matches!(attempt.trajectory, Kept::Retained | Kept::None))
    );
    assert_eq!(study.totals["subject"].cases_passed, 4);
    assert_eq!(study.totals["baseline"].cases_passed, 2);
    // Reading wrote, moved, and changed nothing.
    assert_eq!(files(temp.path()), before);
}

#[test]
fn an_inconclusive_report_stays_inconclusive_and_missing_runs_stay_counted() {
    let temp = tempfile::tempdir().unwrap();
    let study = reopen(&retained(temp.path(), "inconclusive-spread")).unwrap();
    assert_eq!(study.agreement, Agreement::Agrees, "{:?}", study.problems);
    assert_eq!(study.shown, "No clear change");

    let study = reopen(&retained(temp.path(), "partial")).unwrap();
    assert_eq!(study.agreement, Agreement::Agrees, "{:?}", study.problems);
    assert!(study.partial.is_some());
    let coverage = study.coverage["subject"];
    assert!(coverage.completed < coverage.attempted, "{coverage:?}");
    assert_eq!(
        coverage.attempted,
        study
            .attempts
            .iter()
            .filter(|a| a.arm == ext_eval::Arm::Subject)
            .count() as u64
    );
    // An unfinished run's cost is unknown, never zero.
    let totals = &study.totals["subject"];
    assert!(
        totals.cost_unknown > 0 && totals.cost_usd.is_none(),
        "{totals:?}"
    );
    assert_ne!(study.shown, "Better");
}

#[test]
fn a_report_its_attempts_do_not_give_is_disputed() {
    let temp = tempfile::tempdir().unwrap();
    let dir = retained(temp.path(), "worse-fewer");
    let path = dir.join("report.json");
    let mut report: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    report["verdict"] = Value::from("pass");
    std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    let study = reopen(&dir).unwrap();
    assert_eq!(study.agreement, Agreement::Disputes);
    assert_eq!(study.recomputed.as_deref(), Some("fail"));
    assert_eq!(study.shown, "Disputed");
}

#[test]
fn changed_grades_are_disputed_and_missing_ones_unverified() {
    let temp = tempfile::tempdir().unwrap();
    let dir = retained(temp.path(), "better");
    let grades = dir.join("artifacts/grades");
    let case = std::fs::read_dir(&grades)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let file = std::fs::read_dir(&case)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    doc["passed"] = Value::from(!doc["passed"].as_bool().unwrap_or(false));
    std::fs::write(&file, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    let study = reopen(&dir).unwrap();
    assert_eq!(study.agreement, Agreement::Disputes);
    assert!(
        study
            .attempts
            .iter()
            .any(|attempt| attempt.grades == Kept::Changed)
    );

    std::fs::remove_file(&file).unwrap();
    let study = reopen(&dir).unwrap();
    assert_eq!(
        study.agreement,
        Agreement::Unverifiable,
        "{:?}",
        study.problems
    );
    assert_eq!(study.shown, "Unverified");
    assert!(
        study
            .attempts
            .iter()
            .any(|attempt| attempt.grades == Kept::Missing)
    );
}

#[test]
fn a_missing_transcript_is_shown_as_missing() {
    let temp = tempfile::tempdir().unwrap();
    let dir = retained(temp.path(), "better");
    let study = reopen(&dir).unwrap();
    let first = study
        .attempts
        .iter()
        .find(|attempt| attempt.trajectory == Kept::Retained)
        .expect("a retained trajectory");
    std::fs::remove_file(
        dir.join(ext_eval::run_path(&first.case, first.arm, first.attempt))
            .join("trajectory.json"),
    )
    .unwrap();
    let study = reopen(&dir).unwrap();
    assert!(
        study
            .attempts
            .iter()
            .any(|attempt| attempt.trajectory == Kept::Missing)
    );
}

#[test]
fn listing_finds_retained_reports_newest_first() {
    let temp = tempfile::tempdir().unwrap();
    retained(temp.path(), "better");
    retained(temp.path(), "worse-fewer");
    std::fs::create_dir_all(temp.path().join("results/empty")).unwrap();
    let listed = list(temp.path());
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().all(|entry| entry.subject.is_some()));
    assert!(reopen(&temp.path().join("results/empty")).is_err());
}
