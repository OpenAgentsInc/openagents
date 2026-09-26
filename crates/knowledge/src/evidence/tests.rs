//! Paired evidence from fixture run records.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::*;
use crate::Entry;

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
    // A run still going remains as incomplete intake.
    std::fs::create_dir_all(dir.join("task-a-1790000022")).unwrap();
    dir
}

#[test]
fn runs_are_read_from_their_summaries() {
    let runs = scan(&fixture("read"));
    assert_eq!(runs.len(), 15);
    let first = &runs[0];
    assert_eq!(first.task, "task-a");
    assert_eq!(first.started, 1_790_000_001);
    assert!(first.passed());
    assert_eq!(task_of("some-task-17"), "some-task");
    assert_eq!(task_of("some-task"), "some-task");
    assert_eq!(task_of("reference"), "reference");
    assert_eq!(count(1, "entry"), "1 entry");
    assert_eq!(count(2, "entry"), "2 entries");
    assert_eq!(count(0, "run"), "0 runs");
}

#[test]
fn historical_associations_never_authorize_admission() {
    let runs = scan(&fixture("rule"));
    let m = measure(&entry("e.one", "candidate", "reference"), &runs);
    let sides: Vec<(&str, Side)> = m.pairs.iter().map(|p| (p.task.as_str(), p.side)).collect();
    assert_eq!(
        sides,
        [("task-a", Side::Inconclusive), ("task-b", Side::Favors)]
    );
    assert_eq!(m.pairs[0].with.unknown, 1);
    assert_eq!(
        (m.favoring, m.opposing, m.verdict),
        (1, 0, Verdict::Inconclusive)
    );
    assert_eq!(m.runs_with, 5);
}

#[test]
fn a_task_the_entry_was_written_from_never_counts() {
    let runs = scan(&fixture("excluded"));
    let m = measure(&entry("e.two", "admitted", "task-a-1790000001"), &runs);
    assert_eq!(m.excluded_tasks, ["task-a", "task-a-1790000001"]);
    assert_eq!(m.excluded_runs, (2, 3));
    let sides: Vec<(&str, Side)> = m.pairs.iter().map(|p| (p.task.as_str(), p.side)).collect();
    assert_eq!(sides, [("task-d", Side::Opposes)]);
    assert_eq!(m.verdict, Verdict::Inconclusive);
}

#[test]
fn the_report_has_the_nip_eval_shape() {
    let runs = scan(&fixture("report"));
    let e = entry("e.one", "candidate", "reference");
    let m = measure(&e, &runs);
    let (report, artifacts) = report(&m, "the document", &runs, &Evaluator::local(), None);
    assert!(report["meta"]["kb"]["pairs"][0]["with"]["usd"].is_null());
    assert_eq!(report["meta"]["kb"]["pairs"][1]["with"]["usd"], 0.05);
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
    assert_eq!(report["verdict"], "inconclusive");
    assert_eq!(report["subject"]["definition"]["id"], "local:kb/e_one");
    // Every ArtifactRef resolves to stored bytes of its digest and size.
    for key in ["suite", "partition", "runs", "limitations"] {
        let reference = &report[key];
        let bytes = &artifacts[reference["digest"].as_str().unwrap()];
        assert_eq!(bytes.len() as u64, reference["size"].as_u64().unwrap());
    }
    let listed: Value =
        serde_json::from_slice(&artifacts[report["runs"]["digest"].as_str().unwrap()]).unwrap();
    // Every attempted intake remains, including unpaired and incomplete runs.
    assert_eq!(listed.as_array().unwrap().len(), 15);
    assert_eq!(
        report["meta"]["kb"]["denominators"]["unknown_membership"],
        1
    );
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
    assert_eq!(verdict, Verdict::Inconclusive);
    assert_eq!(digest, crate::digest(text.as_bytes()));
    assert_eq!(
        std::fs::read_dir(dir.join("artifacts")).unwrap().count(),
        artifacts.len()
    );
    assert!(line(&m, &digest).contains("2 paired tasks: 1 for it, 0 against it (inconclusive)"));
    assert!(
        recorded(&dir.join("absent.json"))
            .unwrap_err()
            .contains("kb evidence")
    );
}

#[test]
fn review_never_changes_admission_from_historical_associations() {
    let runs = scan(&fixture("review"));
    let entries = vec![
        entry("e.one", "candidate", "reference"),
        entry("e.two", "admitted", "task-a-1790000001"),
        entry("e.three", "admitted", "reference"),
    ];
    assert!(review(&entries, &runs).is_empty());
}

fn rewrite(dir: &Path, name: &str, edit: impl FnOnce(&mut Value)) {
    let path = dir.join(name).join("summary.json");
    let mut value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    edit(&mut value);
    std::fs::write(path, value.to_string()).unwrap();
}

#[test]
fn missing_invalid_and_additional_required_costs_are_unknown() {
    let dir = scratch("costs");
    let name = "task-a-1";
    run(&dir, name, Some(1.0), 0.2, &["e.one"]);
    rewrite(&dir, name, |v| {
        v["outcome"].as_object_mut().unwrap().remove("jev_usd");
    });
    let missing = read_run(&dir.join(name));
    assert_eq!(missing.cost.total_usd, None);
    assert_eq!(missing.cost.known_lower_bound_usd, 0.2);
    assert_eq!(missing.cost.missing, ["jev_usd"]);
    rewrite(&dir, name, |v| {
        v["outcome"]["jev_usd"] = json!(-1.0);
        v["required_cost_components"] = json!(["host_usd"]);
    });
    let invalid = read_run(&dir.join(name));
    assert_eq!(invalid.cost.missing, ["host_usd", "jev_usd"]);
    rewrite(&dir, name, |v| {
        v["outcome"]["jev_usd"] = json!(0.0);
        v["outcome"]["host_usd"] = json!(0.03);
    });
    let complete = read_run(&dir.join(name));
    assert_eq!(complete.cost.total_usd, Some(0.23));
}

#[test]
fn unknown_cost_cannot_look_like_a_cheaper_success() {
    let dir = scratch("unknown-cheap");
    run(&dir, "task-a-1", Some(1.0), 0.01, &["e.one"]);
    run(&dir, "task-a-2", Some(1.0), 1.0, &[]);
    rewrite(&dir, "task-a-1", |v| {
        v["outcome"]
            .as_object_mut()
            .unwrap()
            .remove("embedding_usd");
    });
    let measured = measure(&entry("e.one", "candidate", "reference"), &scan(&dir));
    assert_eq!(measured.pairs[0].side, Side::Inconclusive);
    assert_eq!(measured.pairs[0].with.usd_per_run(), None);
    assert_eq!(measured.favoring, 0);
}

#[test]
fn failed_intake_and_original_corrupt_bytes_are_retained() {
    let dir = scratch("intake-faults");
    run(&dir, "task-a-1", Some(1.0), 0.1, &[]);
    std::fs::create_dir_all(dir.join("task-b-2")).unwrap();
    std::fs::write(dir.join("task-b-2/summary.json"), b"{broken").unwrap();
    std::fs::create_dir_all(dir.join("task-c-3/summary.json")).unwrap();
    std::fs::create_dir_all(dir.join("task-d-4")).unwrap();
    let runs = scan(&dir);
    assert_eq!(runs.len(), 4);
    assert_eq!(
        runs.iter().map(|r| r.intake).collect::<Vec<_>>(),
        [
            IntakeStatus::Complete,
            IntakeStatus::Malformed,
            IntakeStatus::Unreadable,
            IntakeStatus::Incomplete,
        ]
    );
    let e = entry("e.one", "candidate", "reference");
    let m = measure(&e, &runs);
    assert_eq!(m.intake_faults, 3);
    let (report, artifacts) = report(&m, "doc", &runs, &Evaluator::local(), None);
    assert_eq!(artifacts[&crate::digest(b"{broken")], b"{broken");
    let records: Value =
        serde_json::from_slice(&artifacts[report["runs"]["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(records.as_array().unwrap().len(), 4);
    let root_failure = scan(&dir.join("absent"));
    assert_eq!(root_failure.len(), 1);
    assert_eq!(root_failure[0].intake, IntakeStatus::Unreadable);
}

#[test]
fn duplicate_json_fields_are_not_silently_accepted() {
    let dir = scratch("duplicate-fields");
    std::fs::write(dir.join("summary.json"), br#"{"task":"a","task":"b"}"#).unwrap();
    let run = read_run(&dir);
    assert_eq!(run.intake, IntakeStatus::Malformed);
}

#[test]
fn changed_entry_digest_never_counts_for_the_current_version() {
    let dir = scratch("entry-pin");
    let e = entry("e.one", "candidate", "reference");
    run(&dir, "task-a-1", Some(1.0), 0.1, &["e.one"]);
    run(&dir, "task-a-2", Some(0.0), 0.1, &[]);
    rewrite(&dir, "task-a-1", |v| {
        v["outcome"]["knowledge"][0]["digest"] = json!(crate::digest(b"old version"));
        v["outcome"]["knowledge"][0]["version"] = json!(1);
    });
    let m = measure(&e, &scan(&dir));
    assert_eq!(m.changed_entry_runs, 1);
    assert!(m.pairs.is_empty());
    rewrite(&dir, "task-a-1", |v| {
        v["outcome"]["knowledge"][0]["digest"] = json!(e.digest);
    });
    let m = measure(&e, &scan(&dir));
    assert_eq!(m.changed_entry_runs, 0);
    assert_eq!(m.unpinned_entry_runs, 0);
    assert_eq!(m.pairs.len(), 1);
    assert_eq!(m.verdict, Verdict::Inconclusive);
}

fn identity() -> Value {
    json!({
        "schema": "openagents.kb-evidence-identity.v1",
        "harness_digest": crate::digest(b"harness"),
        "configuration_digest": crate::digest(b"configuration"),
        "environment_digest": crate::digest(b"environment"),
        "workload_digest": crate::digest(b"workload"),
        "context_digest": crate::digest(b"other-context-and-entries"),
        "model": "openai/gpt-6-luna", "effort": "high",
        "budget": {"steps": 100, "tokens": 100000, "seconds": 600, "usd": 10.0},
        "partition": "held_out", "group": "independent-source-family",
    })
}

#[test]
fn different_budgets_effort_environment_or_partition_never_pair() {
    for field in [
        "budget",
        "effort",
        "environment_digest",
        "partition",
        "context_digest",
    ] {
        let dir = scratch(&format!("identity-{field}"));
        run(&dir, "task-a-1", Some(1.0), 0.1, &["e.one"]);
        run(&dir, "task-a-2", Some(0.0), 0.1, &[]);
        for name in ["task-a-1", "task-a-2"] {
            rewrite(&dir, name, |v| {
                v["evidence_identity"] = identity();
            });
        }
        rewrite(&dir, "task-a-2", |v| {
            v["evidence_identity"][field] = match field {
                "budget" => json!({"steps": 200, "tokens": 100000, "seconds": 600, "usd": 10.0}),
                "environment_digest" | "context_digest" => json!(crate::digest(b"different")),
                _ => json!("different"),
            };
        });
        let m = measure(&entry("e.one", "candidate", "reference"), &scan(&dir));
        assert!(m.pairs.is_empty(), "{field}");
        assert_eq!(m.noncomparable_runs, 0);
    }
}

#[test]
fn prospective_declaration_is_not_a_verified_study() {
    let dir = scratch("unverified-study");
    run(&dir, "task-a-1", Some(1.0), 0.1, &["e.one"]);
    rewrite(&dir, "task-a-1", |v| {
        v["evidence_identity"] = identity();
        v["evidence_identity"]["assignment"] =
            json!({"study_digest": crate::digest(b"claimed"), "arm": "subject"});
    });
    let m = measure(&entry("e.one", "candidate", "reference"), &scan(&dir));
    assert_eq!(m.prospective_unverified_runs, 1);
    assert!(!m.promotion_eligible);
}

#[test]
fn legacy_positive_report_is_readable_but_cannot_admit_and_is_archived() {
    let dir = scratch("legacy-report");
    let path = report_path(&dir, "e.one", 1);
    let old = b"{\n  \"verdict\": \"pass\", \"legacy\": true\n}";
    std::fs::write(&path, old).unwrap();
    assert_eq!(recorded(&path).unwrap().0, Verdict::Pass);
    let e = entry("e.one", "candidate", "reference");
    assert!(
        recorded_for_admission(&path, &e)
            .unwrap_err()
            .contains("verified prospective study")
    );
    let runs = scan(&fixture("legacy-replacement"));
    let m = measure(&e, &runs);
    let (r, artifacts) = report(&m, "doc", &runs, &Evaluator::local(), None);
    write(&path, &r, &artifacts).unwrap();
    let old_path = dir.join("history").join(format!(
        "{}.json",
        crate::digest(old).trim_start_matches("sha256:")
    ));
    assert_eq!(std::fs::read(old_path).unwrap(), old);
    assert_eq!(recorded(&path).unwrap().0, Verdict::Inconclusive);
}

#[test]
fn unknown_outcomes_keep_spend_and_do_not_improve_a_pair() {
    let dir = scratch("unknown-outcome");
    run(&dir, "task-a-1", Some(1.0), 0.1, &["e.one"]);
    run(&dir, "task-a-2", None, 0.5, &["e.one"]);
    run(&dir, "task-a-3", Some(0.0), 0.1, &[]);
    let m = measure(&entry("e.one", "candidate", "reference"), &scan(&dir));
    assert_eq!(m.pairs[0].side, Side::Inconclusive);
    assert_eq!(m.pairs[0].with.known_lower_bound_usd, 0.6);
    assert_eq!(m.pairs[0].with.usd_per_run(), None);
}

#[tokio::test]
async fn evidence_refusal_does_not_promote_a_waiting_version() {
    let dir = scratch("refused-admission");
    let head = entry("e.one", "admitted", "reference").render();
    let mut next = entry("e.one", "candidate", "reference");
    next.version = 2;
    let candidate = next.render();
    std::fs::create_dir_all(dir.join("versions")).unwrap();
    let head_path = dir.join("e.one.md");
    let candidate_path = crate::version_path(&dir, "e.one", 2);
    std::fs::write(&head_path, &head).unwrap();
    std::fs::write(&candidate_path, &candidate).unwrap();
    let reports = dir.join("reports");
    std::fs::create_dir_all(&reports).unwrap();
    std::fs::write(report_path(&reports, "e.one", 2), br#"{"verdict":"pass"}"#).unwrap();
    let args = [
        "admit",
        "e.one",
        "--evidence",
        "--dir",
        dir.to_str().unwrap(),
        "--evidence-dir",
        reports.to_str().unwrap(),
    ]
    .map(str::to_string);
    assert_ne!(crate::cli::main(&args).await, 0);
    assert_eq!(std::fs::read_to_string(head_path).unwrap(), head);
    assert_eq!(std::fs::read_to_string(candidate_path).unwrap(), candidate);
    assert!(!crate::version_path(&dir, "e.one", 1).exists());
}

#[test]
fn unreadable_previous_report_refuses_replacement() {
    let dir = scratch("unreadable-previous");
    let path = dir.join("report.json");
    // A directory is an existing path that cannot be read as report bytes.
    std::fs::create_dir(&path).unwrap();
    let error = write(&path, &json!({"verdict":"inconclusive"}), &Artifacts::new()).unwrap_err();
    assert!(error.contains("read previous report for retention"));
    assert!(path.is_dir());
}

#[test]
fn millisecond_run_names_retain_name_and_report_seconds() {
    let dir = scratch("millisecond-name");
    run(&dir, "task-a-1790000001123", Some(1.0), 0.1, &[]);
    let rows = scan(&dir);
    assert_eq!(rows[0].started, 1_790_000_001);
    assert_eq!(rows[0].name, "task-a-1790000001123");
}

#[test]
fn configuration_identity_uses_canonical_json() {
    let dir = scratch("canonical-identity");
    run(&dir, "task-a-1", Some(1.0), 0.1, &[]);
    let identity = identity();
    rewrite(&dir, "task-a-1", |v| {
        v["evidence_identity"] = identity.clone()
    });
    let rows = scan(&dir);
    assert_eq!(
        rows[0].identity.comparison_digest.as_ref().unwrap(),
        &nostr::contracts::digest_value(&identity).unwrap()
    );
}

#[test]
fn source_task_with_numeric_suffix_is_excluded_exactly() {
    let dir = scratch("numeric-source");
    run(&dir, "task-2-100", Some(1.0), 0.1, &["e.one"]);
    run(&dir, "task-2-101", Some(0.0), 0.2, &[]);
    let measured = measure(&entry("e.one", "candidate", "task-2"), &scan(&dir));
    assert!(measured.pairs.is_empty());
    assert_eq!(measured.excluded_runs, (1, 1));
}

#[test]
fn aggregate_cost_overflow_never_becomes_a_comparable_total() {
    let dir = scratch("aggregate-overflow");
    run(&dir, "task-a-1", Some(1.0), f64::MAX, &["e.one"]);
    run(&dir, "task-a-2", Some(1.0), f64::MAX, &["e.one"]);
    run(&dir, "task-a-3", Some(1.0), 1.0, &[]);
    let measured = measure(&entry("e.one", "candidate", "reference"), &scan(&dir));
    assert_eq!(measured.pairs[0].with.usd_per_run(), None);
    assert!(measured.pairs[0].with.known_lower_bound_usd.is_finite());
    assert_eq!(measured.pairs[0].side, Side::Inconclusive);
}

#[test]
fn partial_call_charges_from_new_producer_survive_missing_component_total() {
    let dir = scratch("partial-charge");
    run(&dir, "task-a-1", Some(1.0), 0.1, &[]);
    rewrite(&dir, "task-a-1", |v| {
        v["outcome"]["model_usd"] = Value::Null;
        v["outcome"]["known_usd"] = json!(0.25);
        v["outcome"]["cost_unknown"] = json!(["one unpriced model call"]);
    });
    let runs = scan(&dir);
    assert_eq!(runs[0].cost.total_usd, None);
    assert_eq!(runs[0].cost.known_lower_bound_usd, 0.25);
}
