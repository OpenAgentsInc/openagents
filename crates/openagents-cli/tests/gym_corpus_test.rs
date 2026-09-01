//! Corpus inventory and qualification tests.
//!
//! Uses fixture trees in temp dirs, never the real home stores. The determinism
//! property is asserted: two runs over the same fixture tree produce byte-identical
//! output apart from mtimes, which the inventory does not include.

use openagents_cli::gym::corpus::{
    INVENTORY_BOUNDS, corpus_status, inventory, prepare_import, qualify, read_ledger,
    record_import, refuse_batch_visibility, tripwire_findings, verify_ledger,
};
use openagents_cli::trace::DiscoveryBounds;
use std::fs;

fn atif_with_steps(steps: usize, repo: &str) -> String {
    let step_json: String = (1..=steps)
        .map(|i| {
            format!(
                r#"{{"step_id":{i},"timestamp":"2026-08-26T{:02}:00:00.000Z","source":"user","message":"step {i}"}}"#,
                i % 24
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{
  "schema_version": "ATIF-v1.7",
  "session_id": "test-session",
  "agent": {{ "name": "test", "version": "1", "model_name": "openai/gpt-4" }},
  "steps": [{step_json}],
  "notes": "Working in {repo}."
}}"#
    )
}

#[test]
fn inventory_counts_three_stores_and_extra_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let exports = home.join(".openagents/exports");
    let codex = home.join(".codex/sessions");
    let claude = home.join(".claude/projects");
    let extra = tmp.path().join("extra");
    fs::create_dir_all(&exports).unwrap();
    fs::create_dir_all(&codex).unwrap();
    fs::create_dir_all(&claude).unwrap();
    fs::create_dir_all(&extra).unwrap();

    fs::write(
        exports.join("valid.json"),
        atif_with_steps(12, "github.com/OpenAgentsInc/openagents"),
    )
    .unwrap();
    fs::write(
        exports.join("short.json"),
        atif_with_steps(5, "github.com/OpenAgentsInc/openagents"),
    )
    .unwrap();
    fs::write(exports.join("bad.json"), "not json").unwrap();
    fs::write(
        codex.join("rollout.jsonl"),
        r#"{"role":"user","content":"hi"}"#,
    )
    .unwrap();
    fs::write(claude.join("session.jsonl"), "\n").unwrap();
    fs::write(
        extra.join("extra.json"),
        atif_with_steps(15, "github.com/OpenAgentsInc/openagents"),
    )
    .unwrap();

    let out = tmp.path().join("inventory.json");
    let doc = inventory(&home, &[extra.clone()], &out, INVENTORY_BOUNDS).unwrap();

    let by_source: std::collections::BTreeMap<String, usize> = doc
        .stores
        .iter()
        .map(|s| (s.source.clone(), s.listed))
        .collect();
    assert_eq!(
        by_source.get("openagents_export"),
        Some(&3),
        "exports: three .json files"
    );
    assert_eq!(
        by_source.get("codex_session"),
        Some(&1),
        "codex: one .jsonl file"
    );
    assert_eq!(
        by_source.get("claude_session"),
        Some(&1),
        "claude: one .jsonl file"
    );
    assert_eq!(
        by_source.get("trace_path"),
        Some(&1),
        "extra: one .json file"
    );

    assert_eq!(doc.rows.len(), 6, "one row per listed file");

    let valid = doc
        .rows
        .iter()
        .find(|r| r.path.ends_with("valid.json"))
        .unwrap();
    assert!(
        valid.qualifies,
        "valid ATIF with repo hint and steps qualifies"
    );
    assert!(valid.digest.as_ref().unwrap().starts_with("sha256:"));
    assert_eq!(valid.steps, Some(12));
    assert_eq!(valid.repo_hint.as_deref(), Some("OpenAgentsInc/openagents"));

    let short = doc
        .rows
        .iter()
        .find(|r| r.path.ends_with("short.json"))
        .unwrap();
    assert!(!short.qualifies);
    assert!(
        short
            .excluded_because
            .as_ref()
            .unwrap()
            .contains(&"insufficient_substance".to_string())
    );

    let bad = doc
        .rows
        .iter()
        .find(|r| r.path.ends_with("bad.json"))
        .unwrap();
    assert!(!bad.qualifies);
    assert!(
        bad.excluded_because
            .as_ref()
            .unwrap()
            .contains(&"not_redactable".to_string())
    );

    // A parseable Codex line with no typed records converts to an empty ATIF
    // document: redactable now, but with nothing in it.
    let codex_row = doc
        .rows
        .iter()
        .find(|r| r.path.ends_with("rollout.jsonl"))
        .unwrap();
    assert!(!codex_row.qualifies);
    assert!(
        codex_row
            .excluded_because
            .as_ref()
            .unwrap()
            .contains(&"insufficient_substance".to_string()),
        "{:?}",
        codex_row.excluded_because
    );
    assert!(codex_row.digest.is_some(), "converted rows carry a digest");

    // A Claude file with no parseable records at all stays not redactable.
    let claude_row = doc
        .rows
        .iter()
        .find(|r| r.path.ends_with("session.jsonl"))
        .unwrap();
    assert!(!claude_row.qualifies);
    assert!(
        claude_row
            .excluded_because
            .as_ref()
            .unwrap()
            .contains(&"not_redactable".to_string()),
        "{:?}",
        claude_row.excluded_because
    );
    assert!(claude_row.digest.is_none());
}

#[test]
fn inventory_is_deterministic_modulo_mtimes() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let exports = home.join(".openagents/exports");
    let codex = home.join(".codex/sessions");
    let claude = home.join(".claude/projects");
    fs::create_dir_all(&exports).unwrap();
    fs::create_dir_all(&codex).unwrap();
    fs::create_dir_all(&claude).unwrap();

    fs::write(
        exports.join("valid.json"),
        atif_with_steps(12, "github.com/OpenAgentsInc/openagents"),
    )
    .unwrap();
    fs::write(codex.join("rollout.jsonl"), "not atif").unwrap();
    fs::write(claude.join("session.jsonl"), "not atif").unwrap();

    let out1 = tmp.path().join("inventory1.json");
    let out2 = tmp.path().join("inventory2.json");
    inventory(&home, &[], &out1, DiscoveryBounds::default()).unwrap();
    inventory(&home, &[], &out2, DiscoveryBounds::default()).unwrap();

    let t1 = fs::read_to_string(&out1).unwrap();
    let t2 = fs::read_to_string(&out2).unwrap();
    assert_eq!(t1, t2, "two runs over the same tree are byte-identical");
}

#[test]
fn qualify_reports_counted_exclusions() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let exports = home.join(".openagents/exports");
    let codex = home.join(".codex/sessions");
    fs::create_dir_all(&exports).unwrap();
    fs::create_dir_all(&codex).unwrap();

    fs::write(
        exports.join("valid.json"),
        atif_with_steps(12, "github.com/OpenAgentsInc/openagents"),
    )
    .unwrap();
    fs::write(
        exports.join("short.json"),
        atif_with_steps(5, "github.com/OpenAgentsInc/openagents"),
    )
    .unwrap();
    fs::write(codex.join("rollout.jsonl"), "not atif").unwrap();

    let out = tmp.path().join("inventory.json");
    inventory(&home, &[], &out, DiscoveryBounds::default()).unwrap();
    let report = qualify(&out).unwrap();

    assert_eq!(report.total_rows, 3);
    assert_eq!(report.qualified_rows, 1);
    assert_eq!(report.excluded_rows, 2);
    assert!(report.by_reason.contains_key("not_redactable"));
    assert!(report.by_reason.contains_key("insufficient_substance"));
}

#[test]
fn dark_and_glass_batch_visibility_are_refused() {
    let dark = refuse_batch_visibility("dark").expect("dark refused");
    assert!(dark.contains("dark"), "{dark}");
    let glass = refuse_batch_visibility("glass").expect("glass refused");
    assert!(glass.contains("glass"), "{glass}");
    assert!(refuse_batch_visibility("ledger").is_none());
}

#[test]
fn tripwire_halts_on_a_leftover_provider_key() {
    let findings = tripwire_findings("the key is sk-abcdefghijklmnopqrstuvwxyz");
    assert!(
        findings.contains(&"provider_key".to_string()),
        "{findings:?}"
    );
}

#[test]
fn import_is_idempotent_and_verify_names_a_missing_digest() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let exports = home.join(".openagents/exports");
    fs::create_dir_all(&exports).unwrap();
    fs::write(
        exports.join("valid.json"),
        atif_with_steps(12, "github.com/OpenAgentsInc/openagents"),
    )
    .unwrap();
    let inventory_path = tmp.path().join("inventory.json");
    inventory(&home, &[], &inventory_path, DiscoveryBounds::default()).unwrap();
    let ledger = tmp.path().join("corpus.jsonl");

    let (prepared, skipped, _) =
        prepare_import(&inventory_path, "ledger", &ledger, "/Users/test").unwrap();
    assert_eq!(skipped, 0);
    assert_eq!(prepared.len(), 1);
    record_import(
        &ledger,
        "ledger",
        &prepared[0],
        "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
        prepared[0].digest.clone(),
    )
    .unwrap();

    let (again, skipped_again, _) =
        prepare_import(&inventory_path, "ledger", &ledger, "/Users/test").unwrap();
    assert!(again.is_empty(), "re-import must skip the same digest");
    assert_eq!(skipped_again, 1);

    let (imported, pending) = corpus_status(&ledger, Some(&inventory_path)).unwrap();
    assert_eq!(imported, 1);
    assert_eq!(pending, 0);

    let rows = read_ledger(&ledger).unwrap();
    assert_eq!(rows.len(), 1);

    let report = verify_ledger(&ledger, &["sha256:deadbeef".to_string()]).unwrap();
    assert!(
        report.drifts.iter().any(|d| d.contains("sha256:deadbeef")),
        "{:?}",
        report.drifts
    );

    // The recorded row itself verifies against its untouched source.
    let full = verify_ledger(&ledger, &[]).unwrap();
    assert!(full.drifts.is_empty(), "{:?}", full.drifts);
    assert_eq!(full.verified, 1);
    assert_eq!(full.unverifiable, 0);
}

#[test]
fn prepare_import_refuses_dark() {
    let tmp = tempfile::tempdir().unwrap();
    let inventory_path = tmp.path().join("inventory.json");
    fs::write(
        &inventory_path,
        r#"{"schema":"openagents.gym.corpus_inventory.v1","stores":[],"rows":[]}"#,
    )
    .unwrap();
    let err = prepare_import(
        &inventory_path,
        "dark",
        &tmp.path().join("ledger.jsonl"),
        "/Users/test",
    )
    .unwrap_err();
    assert!(err.to_string().contains("dark"), "{err}");
}

/// A redacted assignment keeps its name; only a surviving value is a leak.
#[test]
fn tripwire_spares_a_redacted_assignment_and_halts_on_a_live_one() {
    use openagents_cli::gym::corpus::tripwire_findings;
    let redacted = "the run set OPENAGENTS_TOKEN=[REDACTED_TOKEN] and went on";
    assert!(
        tripwire_findings(redacted).is_empty(),
        "a masked value is not a leak"
    );
    let live = "the run printed OPENAGENTS_TOKEN=oa_live_abc123def456 by mistake";
    assert_eq!(tripwire_findings(live), vec!["env_secret".to_string()]);
}
