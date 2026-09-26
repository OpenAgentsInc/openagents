//! A synced entry measured from run records: only its exact digest counts
//! as shown, and the tasks it was written from never count.

use std::path::PathBuf;

use nostr::domain::RelaySigner;
use serde_json::json;

use super::*;
use crate::evidence::{Verdict, scan};
use crate::remote::{self, Accepted, Sync};

const DOCUMENT: &str = "---\nid: git.reflog-recovery\nversion: 1\nkind: method\ntitle: Recover commits from the reflog\nsummary: >-\n  Lost commits stay reachable from the reflog.\ntags: [git]\napplies_when: >-\n  A branch lost commits.\nstatus: candidate\nauthor: someone\nprovenance:\n  written_from: [in-sample-1790000000]\n  cites: [\"Pro Git, 2014\"]\nevidence: []\n---\n\n## Details\n\nBody.\n";

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("knowledge-transfer-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The author's signed `3190`, cached in `dir` as `kb sync` would.
fn cache(dir: &Path, author: &RelaySigner) -> Event {
    let parts = remote::entry_event(DOCUMENT).unwrap();
    let event = author.sign(1_790_000_000, parts.kind, parts.tags, parts.content);
    let version = kb::parse_entry(&event).unwrap();
    let sync = Sync {
        accepted: vec![Accepted {
            author: author.pubkey().to_string(),
            version,
            event: event.clone(),
        }],
        ..Sync::default()
    };
    remote::write_cache(dir, &sync).unwrap();
    event
}

/// A run record that showed `shown`: each entry ID with its digest, or
/// with none.
fn run(dir: &Path, name: &str, reward: f64, shown: &[(&str, Option<&str>)]) {
    let at = dir.join(name);
    std::fs::create_dir_all(&at).unwrap();
    let knowledge: Vec<Value> = shown
        .iter()
        .map(|(id, digest)| match digest {
            Some(digest) => json!({"id": id, "digest": digest}),
            None => json!({"id": id}),
        })
        .collect();
    let summary = json!({"task": evidence::task_of(name), "model": "m", "reward": reward,
        "outcome": {"model_usd": 0.1, "jev_usd": 0.0, "embedding_usd": 0.0, "knowledge": knowledge}});
    std::fs::write(at.join("summary.json"), summary.to_string()).unwrap();
}

#[test]
fn a_synced_version_is_measured_by_its_exact_digest() {
    let author = RelaySigner::from_secret_hex(&format!("{:064x}", 7)).unwrap();
    let remote_dir = scratch("cache");
    let event = cache(&remote_dir, &author);
    let found = synced(&remote_dir, author.pubkey(), &[]).unwrap();
    assert_eq!(found.len(), 1);
    let synced = &found[0];
    assert_eq!(synced.event.id, event.id);
    assert_eq!(synced.document, DOCUMENT);
    let digest = synced.entry.digest.clone();
    let other_digest = crate::digest(b"another version");
    assert_eq!(synced.hex_digest(), kb::parse_entry(&event).unwrap().digest);

    let id = "git.reflog-recovery";
    let runs_dir = scratch("runs");
    for (name, reward, shown) in [
        ("task-a-1790000001", 1.0, vec![(id, Some(digest.as_str()))]),
        ("task-a-1790000002", 0.0, vec![]),
        ("task-b-1790000003", 1.0, vec![(id, Some(digest.as_str()))]),
        ("task-b-1790000004", 0.0, vec![]),
        // Another version, or another author's entry with the same ID.
        (
            "task-c-1790000005",
            1.0,
            vec![(id, Some(other_digest.as_str()))],
        ),
        ("task-c-1790000006", 0.0, vec![]),
        // No digest recorded.
        ("task-d-1790000007", 1.0, vec![(id, None)]),
        ("task-d-1790000008", 0.0, vec![]),
        // The task the entry was written from.
        (
            "in-sample-1790000009",
            1.0,
            vec![(id, Some(digest.as_str()))],
        ),
        ("in-sample-1790000010", 0.0, vec![]),
    ] {
        run(&runs_dir, name, reward, &shown);
    }
    let runs = scan(&runs_dir);
    assert_eq!(runs.len(), 10);
    let (measured, selection) = measure(synced, &runs, &runs_dir);
    assert_eq!(selection.other_versions, 1);
    assert_eq!(selection.unpinned, 1);
    assert_eq!(selection.runs.len(), 8);
    let tasks: Vec<&str> = measured.pairs.iter().map(|p| p.task.as_str()).collect();
    assert_eq!(tasks, ["task-a", "task-b"]);
    assert_eq!(
        measured.excluded_tasks,
        ["in-sample", "in-sample-1790000000"]
    );
    assert_eq!(measured.excluded_runs, (1, 1));
    assert_eq!(measured.verdict, Verdict::Inconclusive);
    assert_eq!(measured.intake_records, 10);
    assert_eq!(measured.changed_entry_runs, 1);
    assert_eq!(measured.unpinned_entry_runs, 1);
    assert!(left_out(&selection).contains("1 run showed another version"));

    let runner = RelaySigner::from_secret_hex(&format!("{:064x}", 8)).unwrap();
    let evaluator = Evaluator {
        id: runner.pubkey().to_string(),
        namespace: runner.pubkey().to_string(),
    };
    let (report, _) = report(synced, &measured, &selection, &evaluator);
    let definition = &report["subject"]["definition"];
    assert_eq!(definition["id"], kb::qualified_id(author.pubkey(), id));
    assert_eq!(definition["event"]["id"], event.id);
    assert_eq!(definition["event"]["pubkey"], author.pubkey());
    assert_eq!(definition["artifact"], kb::document_artifact(DOCUMENT));
    assert_eq!(report["evaluator"], runner.pubkey());
    assert_eq!(report["meta"]["kb"]["author"], author.pubkey());
    assert_eq!(report["meta"]["kb"]["digest"], digest);
    assert_eq!(report["meta"]["kb"]["other_version_runs"], 1);
    let path = report_path(Path::new("/e"), synced);
    assert_eq!(
        path,
        PathBuf::from(format!("/e/remote/{}/{id}.v1.json", author.pubkey()))
    );
}

#[test]
fn transfer_keeps_failed_intake_and_never_rereads_summary_identity() {
    let author = RelaySigner::from_secret_hex(&format!("{:064x}", 7)).unwrap();
    let remote_dir = scratch("retained-cache");
    cache(&remote_dir, &author);
    let found = synced(&remote_dir, author.pubkey(), &[]).unwrap();
    let synced = &found[0];
    let runs_dir = scratch("retained-runs");
    run(
        &runs_dir,
        "task-1790000001",
        1.0,
        &[("git.reflog-recovery", Some(&synced.entry.digest))],
    );
    run(&runs_dir, "task-1790000002", 0.0, &[]);
    let corrupt = runs_dir.join("task-1790000003");
    std::fs::create_dir_all(&corrupt).unwrap();
    std::fs::write(corrupt.join("summary.json"), b"{broken").unwrap();
    std::fs::create_dir_all(runs_dir.join("task-1790000004")).unwrap();
    let runs = scan(&runs_dir);
    std::fs::write(runs_dir.join("task-1790000001/summary.json"), b"{replaced").unwrap();
    let (measured, selection) = measure(synced, &runs, &runs_dir);
    assert_eq!(measured.intake_records, 4);
    assert_eq!(measured.intake_faults, 2);
    assert_eq!(measured.unknown_membership, 2);
    assert_eq!(measured.pairs[0].with.runs, 1);
    assert_eq!(selection.intake.len(), 4);
    let (_, artifacts) = report(synced, &measured, &selection, &Evaluator::local());
    assert_eq!(artifacts[&crate::digest(b"{broken")], b"{broken");
    assert!(artifacts.contains_key(runs[0].summary_digest.as_ref().unwrap()));
}

#[test]
fn an_entry_that_isnt_synced_is_named() {
    let author = RelaySigner::from_secret_hex(&format!("{:064x}", 7)).unwrap();
    let other = RelaySigner::from_secret_hex(&format!("{:064x}", 9)).unwrap();
    let remote_dir = scratch("missing");
    cache(&remote_dir, &author);
    let error = synced(&remote_dir, author.pubkey(), &["git.other".to_string()]).unwrap_err();
    assert!(error.contains("no entry git.other"), "{error}");
    let error = synced(&remote_dir, other.pubkey(), &[]).unwrap_err();
    assert!(error.contains("kb sync --author"), "{error}");
}
