//! The tuning series keeps an episode's repeated noise visible.

mod common;

use std::path::Path;

use coderbench::tune::{
    FaultRate, Persistence, Series, Shift, Sidecar, measure, measure_trace, traces_in,
};
use coderbench::{Ending, Workspace, observe};
use serde_json::Value;

use common::{authored_text, task};

fn variant(edit: impl Fn(&mut Value)) -> String {
    authored_text()
        .lines()
        .map(|line| {
            let mut value: Value = serde_json::from_str(line).unwrap();
            edit(&mut value);
            serde_json::to_string(&value).unwrap()
        })
        .map(|line| line + "\n")
        .collect()
}

fn measured(directory: &Path, name: &str, text: String) -> coderbench::tune::Run {
    let path = directory.join(format!("{name}.atif.jsonl"));
    std::fs::write(&path, text).unwrap();
    let mut observed = observe(&path).unwrap();
    observed.ending = Ending::Answered;
    observed.workspace = Some(Workspace::default());
    measure(&task(), &observed, &path)
}

fn clean(directory: &Path, name: &str) -> coderbench::tune::Run {
    measured(directory, name, authored_text())
}

fn series(runs: Vec<coderbench::tune::Run>) -> Series {
    Series {
        label: "test".to_string(),
        runs,
    }
}

#[test]
fn it_should_pass_eight_clean_runs_with_no_faults() {
    let directory = tempfile::tempdir().unwrap();
    let series = series(
        (0..8)
            .map(|index| clean(directory.path(), &format!("clean-{index}")))
            .collect(),
    );
    assert_eq!(series.faults(), Vec::<FaultRate>::new());
    assert_eq!(series.matches(), coderbench::Verdict::Passed);
    assert_eq!(series.floor().steps.unwrap().sd, 0.0);
    assert_eq!(series.floor().passed, 8);
}

#[test]
fn it_should_mark_one_run_unverifiable() {
    let directory = tempfile::tempdir().unwrap();
    let series = series(vec![clean(directory.path(), "one")]);
    assert_eq!(series.matches(), coderbench::Verdict::Unverifiable);
}

#[test]
fn it_should_classify_one_wrong_delegation_as_intermittent() {
    let directory = tempfile::tempdir().unwrap();
    let wrong = variant(|value| {
        if value.pointer("/step/call/name").and_then(Value::as_str) == Some("delegate") {
            let slot = value
                .pointer("/step/call/arguments/prompt")
                .and_then(Value::as_str)
                .map_or(0, |prompt| {
                    task()
                        .grade
                        .expects
                        .iter()
                        .position(|expected| expected.prompt == prompt)
                        .unwrap_or(0)
                });
            if slot == 2 {
                value["step"]["call"]["output"] = Value::String("wrong".to_string());
            }
        }
    });
    let mut runs = (0..8)
        .map(|index| clean(directory.path(), &format!("clean-{index}")))
        .collect::<Vec<_>>();
    runs[0] = measured(directory.path(), "wrong", wrong);
    let series = series(runs);
    let faults = series.faults();
    let wrong = faults
        .iter()
        .find(|fault| fault.signature.contains("delegation #3"))
        .unwrap();
    assert_eq!(wrong.persistence, Persistence::Intermittent);
    assert_eq!((wrong.seen, wrong.of), (1, 8));
    assert_eq!(wrong.verdict, coderbench::Verdict::Failed);
    assert!(
        faults
            .iter()
            .all(|fault| { fault.persistence == Persistence::Intermittent })
    );
    assert_eq!(series.matches(), coderbench::Verdict::Passed);
}

#[test]
fn it_should_key_persistent_program_faults_independently_of_call_ids() {
    let directory = tempfile::tempdir().unwrap();
    let missing = variant(|value| {
        if value.pointer("/step/call/name").and_then(Value::as_str) == Some("program") {
            value["record"] = Value::String("message".to_string());
        }
        if value.pointer("/step/call/name").and_then(Value::as_str) == Some("delegate") {
            value["step"]["call"]["id"] = Value::String("renumbered".to_string());
        }
    });
    let runs = (0..8)
        .map(|index| {
            measured(
                directory.path(),
                &format!("missing-{index}"),
                missing.clone(),
            )
        })
        .collect::<Vec<_>>();
    let series = series(runs);
    let faults = series.faults();
    assert!(faults.iter().any(|fault| {
        fault.persistence == Persistence::Persistent
            && fault.seen == 8
            && fault.signature.contains("program")
    }));
    assert_eq!(series.matches(), coderbench::Verdict::Failed);
    assert_eq!(
        series
            .faults()
            .iter()
            .map(|fault| fault.signature.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        series.faults().len()
    );
}

#[test]
fn it_should_leave_one_run_comparison_unverifiable() {
    let directory = tempfile::tempdir().unwrap();
    let baseline = series(vec![clean(directory.path(), "baseline")]);
    let candidate = series(vec![clean(directory.path(), "candidate")]);
    let tuning = coderbench::tune::compare(&task(), &baseline, &candidate).unwrap();
    assert_eq!(tuning.verdict, coderbench::Verdict::Unverifiable);
    assert!(
        tuning
            .gate
            .deciding()
            .unwrap()
            .name
            .contains("scored_items")
    );
    assert!(tuning.shifts.iter().all(|shift| shift.detectable.is_none()));
}

#[test]
fn it_should_not_pass_eight_runs_below_the_gate_floor() {
    let directory = tempfile::tempdir().unwrap();
    let failing = variant(|value| {
        if value.pointer("/step/call/name").and_then(Value::as_str) == Some("program") {
            value["record"] = Value::String("message".to_string());
        }
    });
    let baseline = series(
        (0..8)
            .map(|index| {
                measured(
                    directory.path(),
                    &format!("baseline-{index}"),
                    failing.clone(),
                )
            })
            .collect(),
    );
    let candidate = series(
        (0..8)
            .map(|index| clean(directory.path(), &format!("candidate-{index}")))
            .collect(),
    );
    let tuning = coderbench::tune::compare(&task(), &baseline, &candidate).unwrap();
    assert!(!tuning.fixed.is_empty());
    assert_eq!(tuning.verdict, coderbench::Verdict::Unverifiable);
}

#[test]
fn it_should_use_the_gate_for_pass_rate_comparisons() {
    let directory = tempfile::tempdir().unwrap();
    let failing = variant(|value| {
        if value.pointer("/step/call/name").and_then(Value::as_str) == Some("program") {
            value["record"] = Value::String("message".to_string());
        }
    });
    let make = |name: &str, passed: usize, total: usize| {
        (0..total)
            .map(|index| {
                if index < passed {
                    clean(directory.path(), &format!("{name}-clean-{index}"))
                } else {
                    measured(
                        directory.path(),
                        &format!("{name}-failed-{index}"),
                        failing.clone(),
                    )
                }
            })
            .collect()
    };
    let baseline = series(make("five", 5, 20));
    assert_eq!(
        coderbench::tune::compare(&task(), &baseline, &series(make("fifteen", 15, 20)))
            .unwrap()
            .verdict,
        coderbench::Verdict::Passed
    );
    assert_eq!(
        coderbench::tune::compare(&task(), &baseline, &series(make("seven", 7, 20)))
            .unwrap()
            .verdict,
        coderbench::Verdict::Unverifiable
    );
    assert_eq!(
        coderbench::tune::compare(&task(), &series(make("ten", 10, 20)), &baseline)
            .unwrap()
            .verdict,
        coderbench::Verdict::Failed
    );
}

#[test]
fn it_should_describe_a_shift_inside_or_outside_the_spread() {
    let inside = Shift {
        metric: "seconds",
        before: 1.0,
        after: 1.1,
        detectable: Some(0.2),
    };
    let outside = Shift {
        after: 1.3,
        ..inside.clone()
    };
    assert_eq!(inside.inside_spread(), Some(true));
    assert_eq!(outside.inside_spread(), Some(false));
}

#[test]
fn it_should_find_sorted_traces_and_reject_an_empty_directory() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("b.atif.jsonl"), authored_text()).unwrap();
    std::fs::write(directory.path().join("a.atif.jsonl"), authored_text()).unwrap();
    std::fs::write(directory.path().join("stray.txt"), "stray").unwrap();
    let traces = traces_in(directory.path()).unwrap();
    assert_eq!(
        traces
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        vec!["a.atif.jsonl", "b.atif.jsonl"]
    );
    let empty = tempfile::tempdir().unwrap();
    assert!(traces_in(empty.path()).is_err());
}

#[test]
fn it_should_read_a_recorded_run_back_as_it_was_judged_live() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("live.atif.jsonl");
    std::fs::write(&path, authored_text()).unwrap();

    let bare = measure_trace(&task(), &path).unwrap();
    assert_eq!(bare.ending, Ending::Closed);
    assert!(
        bare.signatures
            .iter()
            .any(|fault| fault.contains("nothing compared the workspace")),
        "{:?}",
        bare.signatures
    );
    assert_ne!(bare.judgment.verdict, coderbench::Verdict::Passed);

    let mut observed = observe(&path).unwrap();
    observed.ending = Ending::Answered;
    observed.workspace = Some(Workspace::default());
    let sidecar = Sidecar::of(&observed);
    assert_eq!(sidecar.ending, "answered");
    assert_eq!(sidecar.changed, Some(Vec::new()));
    sidecar.write(&path).unwrap();
    assert!(Sidecar::path(&path).ends_with("live.atif.jsonl.observed.json"));

    let recorded = measure_trace(&task(), &path).unwrap();
    assert_eq!(recorded.ending, Ending::Answered);
    assert_eq!(recorded.signatures, Vec::<String>::new());
    assert_eq!(recorded.judgment.verdict, coderbench::Verdict::Passed);
    assert_eq!(traces_in(directory.path()).unwrap(), vec![path.clone()]);

    std::fs::write(Sidecar::path(&path), "not json").unwrap();
    assert!(measure_trace(&task(), &path).is_err());
}
