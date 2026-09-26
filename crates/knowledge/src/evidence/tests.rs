//! Paired evidence from fixture run records.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::*;
use crate::{Entry, Status};

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("knowledge-evidence-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes one run's `summary.json`, shaped like Microcoder's.
fn run(dir: &Path, name: &str, reward: Option<f64>, usd: f64, used: &[&str]) {
    let at = dir.join(name);
    std::fs::create_dir_all(&at).unwrap();
    let task = task_of(name);
    let summary = json!({
        "task": task, "model": "openai/gpt-6-luna", "reward": reward,
        "outcome": {
            "model_usd": usd, "jev_usd": 0.0, "embedding_usd": 0.0, "steps": 3,
            "knowledge": used.iter().map(|id| json!({"id": id, "digest": "sha256:0", "kept_steps": 1, "expanded_steps": 0})).collect::<Vec<_>>(),
        },
    });
    std::fs::write(at.join("summary.json"), summary.to_string()).unwrap();
}

fn entry(id: &str, status: &str, written_from: &str) -> Entry {
    Entry::parse(&format!(
        "---\nid: {id}\nversion: 1\nkind: slip\ntitle: T\nsummary: S.\ntags: [x]\napplies_when: A.\nstatus: {status}\nauthor: me\nprovenance:\n  written_from: [{written_from}]\n  cites: [\"Book\"]\nevidence: []\n---\n\n## Details\n\nBody.\n"
    ))
    .unwrap()
}

fn fixture(name: &str) -> PathBuf {
    let dir = scratch(name);
    // task-a: the runs with both entries pass, the runs without fail.
    run(
        &dir,
        "task-a-1790000001",
        Some(1.0),
        0.10,
        &["e.one", "e.two"],
    );
    run(
        &dir,
        "task-a-1790000002",
        Some(1.0),
        0.10,
        &["e.one", "e.two"],
    );
    run(&dir, "task-a-1790000003", Some(0.0), 0.30, &[]);
    run(&dir, "task-a-1790000004", Some(0.0), 0.30, &[]);
    // task-b: both pass; the run with e.one costs a quarter as much.
    run(&dir, "task-b-1790000005", Some(1.0), 0.05, &["e.one"]);
    run(&dir, "task-b-1790000006", Some(1.0), 0.20, &[]);
    // task-c: only with e.one, so no pair.
    run(&dir, "task-c-1790000007", Some(1.0), 0.05, &["e.one"]);
    // task-d: e.two and e.three fail where the run without passes.
    for n in 0..5 {
        run(
            &dir,
            &format!("task-d-179000001{n}"),
            Some(0.0),
            0.10,
            &["e.two", "e.three"],
        );
    }
    run(&dir, "task-d-1790000020", Some(1.0), 0.10, &[]);
    // Grading failed: counted as unknown, never as a pass or a failure.
    run(&dir, "task-a-1790000021", None, 0.10, &["e.one"]);
    // A run still going has no summary and is skipped.
    std::fs::create_dir_all(dir.join("task-a-1790000022")).unwrap();
    dir
}

#[test]
fn runs_are_read_from_their_summaries() {
    let runs = scan(&fixture("read"));
    assert_eq!(runs.len(), 14);
    let first = &runs[0];
    assert_eq!(first.task, "task-a");
    assert_eq!(first.started, 1_790_000_001);
    assert!(first.passed());
    assert_eq!(task_of("some-task-17"), "some-task");
    assert_eq!(task_of("some-task"), "some-task");
    assert_eq!(task_of("reference"), "reference");
}

#[test]
fn an_entry_that_passes_more_or_costs_less_passes_the_rule() {
    let runs = scan(&fixture("rule"));
    let m = measure(&entry("e.one", "candidate", "reference"), &runs);
    let sides: Vec<(&str, Side)> = m.pairs.iter().map(|p| (p.task.as_str(), p.side)).collect();
    assert_eq!(sides, [("task-a", Side::Favors), ("task-b", Side::Favors)]);
    assert_eq!(m.pairs[0].with.unknown, 1);
    assert_eq!((m.favoring, m.opposing, m.verdict), (2, 0, Verdict::Pass));
    assert_eq!(m.runs_with, 5);
}

#[test]
fn a_task_the_entry_was_written_from_never_counts() {
    let runs = scan(&fixture("excluded"));
    let m = measure(&entry("e.two", "admitted", "task-a-1790000001"), &runs);
    assert_eq!(m.excluded_tasks, ["task-a"]);
    assert_eq!(m.excluded_runs, (2, 3));
    let sides: Vec<(&str, Side)> = m.pairs.iter().map(|p| (p.task.as_str(), p.side)).collect();
    assert_eq!(sides, [("task-d", Side::Opposes)]);
    assert_eq!(m.verdict, Verdict::Fail);
}

#[test]
fn the_report_has_the_nip_eval_shape() {
    let runs = scan(&fixture("report"));
    let e = entry("e.one", "candidate", "reference");
    let m = measure(&e, &runs);
    let (report, artifacts) = report(&m, "the document", &runs, &Evaluator::local(), None);
    for key in [
        "suite",
        "partition",
        "subject",
        "baseline",
        "evaluator",
        "started_at",
        "ended_at",
        "runs",
        "coverage",
        "measurements",
        "verdict",
        "limitations",
    ] {
        assert!(!report[key].is_null(), "{key} is missing");
    }
    assert_eq!(report["v"], "openagents.eval-report.v1");
    assert_eq!(report["verdict"], "pass");
    assert_eq!(report["subject"]["definition"]["id"], "local:kb/e_one");
    // Every ArtifactRef resolves to stored bytes of its digest and size.
    for key in ["suite", "partition", "runs", "limitations"] {
        let reference = &report[key];
        let bytes = &artifacts[reference["digest"].as_str().unwrap()];
        assert_eq!(bytes.len() as u64, reference["size"].as_u64().unwrap());
    }
    let listed: Value =
        serde_json::from_slice(&artifacts[report["runs"]["digest"].as_str().unwrap()]).unwrap();
    // task-a and task-b: 3 + 1 runs with the entry and 2 + 1 without.
    assert_eq!(listed.as_array().unwrap().len(), 7);
    let coverage = &report["coverage"]["subject"];
    let terminal: u64 = ["completed", "refused", "failed", "cancelled", "unknown"]
        .iter()
        .map(|k| coverage[k].as_u64().unwrap())
        .sum();
    assert_eq!(terminal, coverage["attempted"].as_u64().unwrap());
    assert_eq!(coverage["unknown"], 1);

    let dir = scratch("reports");
    let path = report_path(&dir, "e.one", 1);
    let text = write(&path, &report, &artifacts).unwrap();
    let (verdict, digest) = recorded(&path).unwrap();
    assert_eq!(verdict, Verdict::Pass);
    assert_eq!(digest, crate::digest(text.as_bytes()));
    assert_eq!(
        std::fs::read_dir(dir.join("artifacts")).unwrap().count(),
        artifacts.len()
    );
    assert!(line(&m, &digest).contains("2 of 2 paired tasks favor it, 0 oppose it (pass)"));
    assert!(
        recorded(&dir.join("absent.json"))
            .unwrap_err()
            .contains("kb evidence")
    );
}

#[test]
fn review_demotes_a_shown_entry_that_never_helps_and_names_a_candidate_to_admit() {
    let runs = scan(&fixture("review"));
    let entries = vec![
        entry("e.one", "candidate", "reference"),
        entry("e.two", "admitted", "task-a-1790000001"),
        entry("e.three", "admitted", "reference"),
    ];
    let proposals = review(&entries, &runs);
    let actions: Vec<(&str, &str)> = proposals
        .iter()
        .map(|p| (p.id.as_str(), p.action))
        .collect();
    // e.two was shown in 5 out-of-sample runs too, and never helped.
    assert_eq!(
        actions,
        [
            ("e.one", "admit"),
            ("e.two", "demote"),
            ("e.three", "demote")
        ]
    );
    assert_eq!(entries[1].status, Status::Admitted);
}
