//! Independent acceptance for the historical directory-order task.
//! Fixtures use the documented record shape and APIs that existed at the base.

use gym::runs::{Catalog, Run, Sources};
use gym::runs_beats_winner::beats_winner;
use gym::runs_highlights::Inputs;
use gym::runs_marks::Marks;
use gym::runs_microcoder::{Knowledge, read_all};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const TASK: &str = "holdout-fixture";
const MIXED: &str = "holdout-fixture-1700000001";
const PASS: &str = "holdout-fixture-1700000002";
const FAIL: &str = "holdout-fixture-1700000003";
const ATTRIBUTED: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const RECORDED: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn summary(reward: f64, commit: Option<&str>) -> Value {
    let mut value = json!({
        "task": TASK,
        "model": "openai/fixture-model",
        "provider": "openrouter",
        "cost_basis": "billed",
        "kb": "off",
        "knowledge_assisted": false,
        "reward": reward,
        "outcome": {
            "ending": {"reason": "finished"},
            "steps": 1,
            "seconds": 2.0,
            "model_usd": 0.1,
            "jev_usd": 0.0,
            "embedding_usd": 0.0,
            "knowledge": [],
            "knowledge_assisted": false
        }
    });
    if let Some(commit) = commit {
        value["commit"] = json!(commit);
    }
    value
}

fn write_run(root: &Path, name: &str, reward: f64, commit: Option<&str>) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("summary.json"),
        summary(reward, commit).to_string(),
    )
    .unwrap();
}

fn write_manifest(root: &Path, entries: Value) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("MANIFEST.json"),
        json!({
            "schema": "openagents.gym.microcoder-retained.v1",
            "commit_rule": "fixture attribution",
            "runs": entries
        })
        .to_string(),
    )
    .unwrap();
}

fn catalog(dirs: &[PathBuf]) -> Vec<Run> {
    Catalog::load(Sources {
        jobs: None,
        traces: None,
        tasks: Vec::new(),
        index: None,
        microcoder: dirs.to_vec(),
        knowledge: None,
    })
    .runs
}

fn readers(dirs: &[PathBuf]) -> [Vec<Run>; 2] {
    [read_all(dirs, &Knowledge::default(), 0), catalog(dirs)]
}

fn claims(runs: &[Run]) -> Vec<Value> {
    let reference = json!({"trials": [{
        "id": "fixture-reference",
        "task": TASK,
        "model": "Fable 5.1",
        "effort": "low",
        "reward": 1.0,
        "cost_usd": 1.0,
        "started_at": "2026-09-01T00:00:00Z",
        "finished_at": "2026-09-01T00:00:10Z"
    }]});
    beats_winner(&Inputs {
        runs,
        answers: &HashMap::new(),
        reference: None,
        marks: &Marks::default(),
        fable: Some(&reference),
    })
    .iter()
    .map(|claim| claim.to_json())
    .collect()
}

fn number(claim: &Value, label: &str) -> f64 {
    claim["numbers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|number| number["label"] == label)
        .unwrap()["value"]
        .as_f64()
        .unwrap()
}

#[test]
fn a_manifest_without_a_local_copy_marks_the_run_in_both_readers() {
    let tmp = tempfile::tempdir().unwrap();
    let live = tmp.path().join("live");
    let retained = tmp.path().join("retained");
    write_run(&live, MIXED, 1.0, None);
    write_manifest(
        &retained,
        json!([{"name": MIXED, "mixed": "independently reported collision", "commit": ATTRIBUTED}]),
    );
    for dirs in [[live.clone(), retained.clone()], [retained, live]] {
        for runs in readers(&dirs) {
            assert_eq!(runs.len(), 1);
            let record = runs[0].microcoder.as_ref().unwrap();
            assert!(record.mixed, "metadata must follow the run identity");
            assert_eq!(record.commit.as_deref(), Some(ATTRIBUTED));
            assert_eq!(record.commit_source.as_deref(), Some("fixture attribution"));
        }
    }
}

#[test]
fn retained_records_win_over_divergent_live_copies_in_both_orders() {
    let tmp = tempfile::tempdir().unwrap();
    let live = tmp.path().join("a-live");
    let retained = tmp.path().join("z-retained");
    write_run(&live, PASS, 1.0, None);
    write_run(&retained, PASS, 0.0, None);
    write_manifest(&retained, json!([{"name": PASS, "files": {}}]));
    for dirs in [[live.clone(), retained.clone()], [retained.clone(), live]] {
        for runs in readers(&dirs) {
            assert_eq!(runs.len(), 1, "copies represent one run");
            assert_eq!(runs[0].reward, Some(0.0), "read the retained record");
            assert_eq!(runs[0].files.dir, retained.join(PASS));
        }
    }
}

#[test]
fn directory_order_preserves_the_claim_and_its_denominator() {
    let tmp = tempfile::tempdir().unwrap();
    let live = tmp.path().join("live");
    let retained = tmp.path().join("retained");
    for root in [&live, &retained] {
        write_run(root, MIXED, 1.0, None);
    }
    write_run(&live, PASS, 1.0, None);
    write_run(&live, FAIL, 0.0, None);
    write_manifest(
        &retained,
        json!([{"name": MIXED, "files": {}, "mixed": "collision"}]),
    );
    let mut baseline = None;
    for dirs in [[live.clone(), retained.clone()], [retained, live]] {
        for runs in readers(&dirs) {
            assert_eq!(runs.len(), 3);
            let result = claims(&runs);
            assert_eq!(result.len(), 1);
            assert_eq!(number(&result[0], "passes"), 1.0);
            assert_eq!(number(&result[0], "graded"), 2.0);
            if let Some(expected) = &baseline {
                assert_eq!(&result, expected, "both readers must make identical claims");
            } else {
                baseline = Some(result);
            }
        }
    }
}

#[test]
fn duplicate_inputs_do_not_duplicate_runs_or_merge_distinct_identities() {
    let tmp = tempfile::tempdir().unwrap();
    let first = tmp.path().join("first");
    let second = tmp.path().join("second");
    for root in [&first, &second] {
        write_run(root, PASS, 1.0, None);
        write_run(root, FAIL, 1.0, None);
    }
    for runs in readers(&[second, first.clone(), first]) {
        assert_eq!(runs.len(), 2);
        let mut ids: Vec<_> = runs.iter().map(|run| run.trial.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec![PASS, FAIL]);
    }
}

#[test]
fn recorded_commit_keeps_its_provenance_when_a_manifest_adds_a_mark() {
    let tmp = tempfile::tempdir().unwrap();
    let live = tmp.path().join("live");
    let retained = tmp.path().join("retained");
    write_run(&live, MIXED, 1.0, Some(RECORDED));
    write_manifest(
        &retained,
        json!([{"name": MIXED, "mixed": "collision", "commit": ATTRIBUTED}]),
    );
    for runs in readers(&[retained, live]) {
        let record = runs[0].microcoder.as_ref().unwrap();
        assert!(record.mixed);
        assert_eq!(record.commit.as_deref(), Some(RECORDED));
        assert_eq!(record.commit_source.as_deref(), Some("record"));
    }
}
