use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn signed_update_apply_records_versions_signer_and_receipt() -> TestResult {
    let state_dir = unique_state_dir("update-apply");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;

    let receipt = run_oa_node_json(&[
        "update",
        "apply",
        "--target-version",
        "0.2.0",
        "--signer",
        "local-keychain://openagents/cloud/release",
        "--signature-digest",
        "sha256:abc123",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        receipt.pointer("/action").and_then(Value::as_str),
        Some("apply")
    );
    assert_eq!(
        receipt.pointer("/target_version").and_then(Value::as_str),
        Some("0.2.0")
    );
    assert_eq!(
        receipt.pointer("/signer").and_then(Value::as_str),
        Some("local-keychain://openagents/cloud/release")
    );
    assert_eq!(
        receipt.pointer("/result").and_then(Value::as_str),
        Some("succeeded")
    );
    assert!(receipt
        .pointer("/previous_version")
        .and_then(Value::as_str)
        .is_some());
    assert!(receipt
        .pointer("/receipt_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));

    let status = update_status(&state_dir)?;
    assert_eq!(
        status
            .pointer("/updates/current_version")
            .and_then(Value::as_str),
        Some("0.2.0")
    );
    let log = fs::read_to_string(state_dir.join("update-receipts.jsonl"))?;
    assert_eq!(log.lines().count(), 1);
    assert!(!log.to_ascii_lowercase().contains("private_key"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn failed_update_rolls_back_and_records_health() -> TestResult {
    let state_dir = unique_state_dir("update-rollback");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;
    run_update_apply(&state_dir, "0.2.0", "succeeded")?;

    let receipt = run_update_apply(&state_dir, "0.3.0", "failed")?;
    assert_eq!(
        receipt.pointer("/previous_version").and_then(Value::as_str),
        Some("0.2.0")
    );
    assert_eq!(
        receipt.pointer("/target_version").and_then(Value::as_str),
        Some("0.3.0")
    );
    assert_eq!(
        receipt.pointer("/result").and_then(Value::as_str),
        Some("rolled_back")
    );
    let status = update_status(&state_dir)?;
    assert_eq!(
        status
            .pointer("/updates/current_version")
            .and_then(Value::as_str),
        Some("0.2.0")
    );
    let health = run_oa_node_json(&[
        "admin",
        "health",
        "list",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert!(serde_json::to_string(&health)?.contains("update_failed_rolled_back"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn update_policy_can_pin_and_defer() -> TestResult {
    let state_dir = unique_state_dir("update-policy");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;

    let policy = run_oa_node_json(&[
        "update",
        "policy",
        "set",
        "--channel",
        "stable",
        "--pin",
        "0.2.0",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        policy
            .pointer("/updates/pinned_version")
            .and_then(Value::as_str),
        Some("0.2.0")
    );
    let deferred_pinned = run_update_apply(&state_dir, "0.3.0", "succeeded")?;
    assert_eq!(
        deferred_pinned.pointer("/result").and_then(Value::as_str),
        Some("deferred_pinned")
    );

    run_oa_node_json(&[
        "update",
        "policy",
        "set",
        "--channel",
        "stable",
        "--pin",
        "0.2.0",
        "--defer",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    let deferred = run_update_apply(&state_dir, "0.2.0", "succeeded")?;
    assert_eq!(
        deferred.pointer("/result").and_then(Value::as_str),
        Some("deferred")
    );
    let status = update_status(&state_dir)?;
    assert_eq!(
        status
            .pointer("/updates/pending_update")
            .and_then(Value::as_str),
        Some("0.2.0")
    );
    assert_eq!(
        status.pointer("/updates/deferred").and_then(Value::as_bool),
        Some(true)
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn update_rejects_secret_markers_in_signer() -> TestResult {
    let state_dir = unique_state_dir("update-invalid");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;

    let output = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .args([
            "update",
            "apply",
            "--target-version",
            "0.2.0",
            "--signer",
            "private_key://release",
            "--signature-digest",
            "sha256:abc123",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("forbidden marker"));
    assert!(!state_dir.join("update-receipts.jsonl").exists());

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn run_update_apply(
    state_dir: &Path,
    target_version: &str,
    result: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    run_oa_node_json(&[
        "update",
        "apply",
        "--target-version",
        target_version,
        "--signer",
        "local-keychain://openagents/cloud/release",
        "--signature-digest",
        "sha256:abc123",
        "--result",
        result,
        "--state-dir",
        state_path(state_dir)?,
        "--json",
    ])
}

fn update_status(state_dir: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    run_oa_node_json(&[
        "update",
        "status",
        "--state-dir",
        state_path(state_dir)?,
        "--json",
    ])
}

fn init_node(state_dir: &Path) -> TestResult {
    run_oa_node_json(&[
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(state_dir)?,
        "--json",
    ])?;
    Ok(())
}

fn run_oa_node_json(args: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_oa-node"))
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
