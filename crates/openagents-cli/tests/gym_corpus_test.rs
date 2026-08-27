//! Corpus inventory and qualification tests.
//!
//! Uses fixture trees in temp dirs, never the real home stores. The determinism
//! property is asserted: two runs over the same fixture tree produce byte-identical
//! output apart from mtimes, which the inventory does not include.

use openagents_cli::gym::corpus::{INVENTORY_BOUNDS, inventory, qualify};
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
            .contains(&"not_redactable".to_string())
    );
    assert!(codex_row.digest.is_none());
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
