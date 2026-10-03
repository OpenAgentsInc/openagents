//! Post hoc quality checks, separate from the frozen five-case acceptance checker.
//! Prepared after candidate review; not part of the registered trial scores.
//! Execution status is recorded in heldout-audit.json.

use gym::runs::{Catalog, Run, Sources};
use gym::runs_microcoder::{Knowledge, read_all};
use serde_json::json;
use std::path::{Path, PathBuf};

const NAME: &str = "posthoc-order-1700000001";

fn write_run(root: &Path, reward: f64) {
    let dir = root.join(NAME);
    std::fs::create_dir_all(&dir).unwrap();
    let summary = json!({
        "task": "posthoc-order",
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
    std::fs::write(dir.join("summary.json"), summary.to_string()).unwrap();
}

fn manifest(root: &Path, entry: serde_json::Value) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("MANIFEST.json"),
        json!({
            "schema": "openagents.gym.microcoder-retained.v1",
            "commit_rule": "posthoc attribution",
            "runs": [entry]
        })
        .to_string(),
    )
    .unwrap();
}

fn readers(dirs: &[PathBuf]) -> [Vec<Run>; 2] {
    [
        read_all(dirs, &Knowledge::default(), 0),
        Catalog::load(Sources {
            jobs: None,
            traces: None,
            tasks: Vec::new(),
            index: None,
            microcoder: dirs.to_vec(),
            knowledge: None,
        })
        .runs,
    ]
}

#[test]
fn equal_rank_divergent_copies_choose_the_same_record_in_both_orders() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("a-retained");
    let second = temp.path().join("z-retained");
    write_run(&first, 0.0);
    write_run(&second, 1.0);
    for root in [&first, &second] {
        manifest(root, json!({"name": NAME, "files": {}}));
    }
    let forward = readers(&[first.clone(), second.clone()]);
    let reverse = readers(&[second, first]);
    for (left, right) in forward.iter().zip(&reverse) {
        assert_eq!(left.len(), 1);
        assert_eq!(right.len(), 1);
        assert_eq!(left[0].reward, right[0].reward);
        assert_eq!(left[0].files.dir, right[0].files.dir);
    }
}

#[test]
fn conflicting_manifest_marks_resolve_the_same_way_in_both_orders() {
    let temp = tempfile::tempdir().unwrap();
    let live = temp.path().join("live");
    let first = temp.path().join("a-marks");
    let second = temp.path().join("z-marks");
    write_run(&live, 1.0);
    for (root, commit, reason) in [
        (&first, "a".repeat(40), "posthoc marker a"),
        (&second, "b".repeat(40), "posthoc marker z"),
    ] {
        manifest(root, json!({"name": NAME, "commit": commit, "mixed": reason}));
    }
    let forward = readers(&[live.clone(), first.clone(), second.clone()]);
    let reverse = readers(&[second, first, live]);
    for (left, right) in forward.iter().zip(&reverse) {
        assert_eq!(left.len(), 1);
        assert_eq!(right.len(), 1);
        let a = left[0].microcoder.as_ref().unwrap();
        let b = right[0].microcoder.as_ref().unwrap();
        assert!(a.mixed && b.mixed);
        assert_eq!(a.commit, b.commit);
        assert_eq!(a.commit_source, b.commit_source);
        let reasons = |run: &Run| {
            run.notes
                .iter()
                .filter(|note| note.contains("posthoc marker"))
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(reasons(&left[0]), reasons(&right[0]));
    }
}
