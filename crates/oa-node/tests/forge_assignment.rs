use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn forge_assignment_refuses_open_ended_labor_and_persists_receipt() -> TestResult {
    let state_dir = unique_state_dir("forge-labor");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json(&[
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let receipt = run_oa_node_json(&[
        "forge",
        "assignment",
        "receive",
        "--file",
        fixture_path("open-ended-labor-assignment.json")
            .to_str()
            .ok_or("fixture path utf8")?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        receipt.pointer("/decision").and_then(Value::as_str),
        Some("refused")
    );
    assert!(receipt
        .pointer("/reason")
        .and_then(Value::as_str)
        .is_some_and(|reason| reason.contains("open_ended_labor")));
    assert!(receipt
        .pointer("/assignment_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));
    assert!(receipt
        .pointer("/receipt_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));

    let receipt_log = fs::read_to_string(state_dir.join("forge-assignment-receipts.jsonl"))?;
    assert_eq!(receipt_log.lines().count(), 1);
    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/evidence/job_receipts")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn forge_assignment_accepts_workroom_scaffold_when_node_online() -> TestResult {
    let state_dir = unique_state_dir("forge-workroom");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json(&[
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    run_oa_node_json(&[
        "admin",
        "desired-mode",
        "set",
        "online",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let receipt = run_oa_node_json(&[
        "forge",
        "assignment",
        "receive",
        "--file",
        fixture_path("workroom-assignment.json")
            .to_str()
            .ok_or("fixture path utf8")?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        receipt.pointer("/decision").and_then(Value::as_str),
        Some("accepted")
    );
    assert_eq!(
        receipt.pointer("/reason").and_then(Value::as_str),
        Some("accepted_for_workroom_scaffold")
    );

    let admin_store: Value =
        serde_json::from_str(&fs::read_to_string(state_dir.join("admin-store.json"))?)?;
    assert!(admin_store
        .pointer("/receipt_cursors/job_receipt_cursor")
        .and_then(Value::as_str)
        .is_some_and(|cursor| cursor.starts_with("sha256:")));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/cloud/forge_assignment_v1")
        .join(name)
}

fn run_oa_node_json(args: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .env_remove("OPENAGENTS_PSIONIC_ENDPOINT")
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "oa-node failed: status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn unique_state_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa-node-{label}-{}-{}",
        std::process::id(),
        unique_suffix()
    ))
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn state_path(path: &Path) -> Result<&str, Box<dyn std::error::Error>> {
    path.to_str().ok_or_else(|| "state dir is not utf-8".into())
}
