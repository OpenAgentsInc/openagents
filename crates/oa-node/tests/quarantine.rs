use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn quarantine_blocks_new_work_and_records_drain_policy() -> TestResult {
    let state_dir = unique_state_dir("quarantine-enter");
    fs::create_dir_all(&state_dir)?;
    init_online_node(&state_dir)?;

    let entered = run_oa_node_json(&[
        "quarantine",
        "enter",
        "--reason",
        "policy_violation",
        "--workroom-policy",
        "pause",
        "--workroom",
        "workroom.local.echo",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        entered
            .pointer("/quarantine/quarantined")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        entered
            .pointer("/quarantine/workroom_policy")
            .and_then(Value::as_str),
        Some("pause")
    );
    assert_eq!(
        entered.pointer("/receipt/result").and_then(Value::as_str),
        Some("new_work_blocked")
    );
    assert!(entered
        .pointer("/receipt/receipt_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));

    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/lifecycle/observed_status")
            .and_then(Value::as_str),
        Some("quarantined")
    );

    let assignment = run_oa_node_json(&[
        "forge",
        "assignment",
        "receive",
        "--file",
        fixture_path("workroom-assignment.json")?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        assignment.pointer("/decision").and_then(Value::as_str),
        Some("refused")
    );
    assert!(assignment
        .pointer("/reason")
        .and_then(Value::as_str)
        .is_some_and(|reason| reason.contains("node_not_available: quarantined")));

    let receipts = fs::read_to_string(state_dir.join("quarantine-receipts.jsonl"))?;
    assert_eq!(receipts.lines().count(), 1);
    assert!(receipts.contains("workroom.local.echo"));
    assert!(!receipts.to_ascii_lowercase().contains("secret-token"));

    let health = run_oa_node_json(&[
        "admin",
        "health",
        "list",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert!(serde_json::to_string(&health)?.contains("quarantine_entered"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn quarantine_exit_releases_node_to_offline() -> TestResult {
    let state_dir = unique_state_dir("quarantine-exit");
    fs::create_dir_all(&state_dir)?;
    init_online_node(&state_dir)?;
    run_oa_node_json(&[
        "quarantine",
        "enter",
        "--reason",
        "suspicious_health",
        "--workroom-policy",
        "migrate",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let exited = run_oa_node_json(&[
        "quarantine",
        "exit",
        "--reason",
        "operator_release",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        exited
            .pointer("/quarantine/quarantined")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        exited.pointer("/receipt/result").and_then(Value::as_str),
        Some("released")
    );
    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/lifecycle/observed_status")
            .and_then(Value::as_str),
        Some("offline")
    );
    let receipts = fs::read_to_string(state_dir.join("quarantine-receipts.jsonl"))?;
    assert_eq!(receipts.lines().count(), 2);
    assert!(receipts.contains("released"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn quarantine_rejects_invalid_policy_and_secret_markers() -> TestResult {
    let state_dir = unique_state_dir("quarantine-invalid");
    fs::create_dir_all(&state_dir)?;
    init_online_node(&state_dir)?;

    let invalid_policy = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .args([
            "quarantine",
            "enter",
            "--reason",
            "policy_violation",
            "--workroom-policy",
            "ignore",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!invalid_policy.status.success());
    assert!(String::from_utf8_lossy(&invalid_policy.stderr).contains("unsupported"));

    let secret_reason = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .args([
            "quarantine",
            "enter",
            "--reason",
            "secret-token",
            "--workroom-policy",
            "close",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!secret_reason.status.success());
    assert!(String::from_utf8_lossy(&secret_reason.stderr).contains("forbidden marker"));
    assert!(!state_dir.join("quarantine-receipts.jsonl").exists());

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn init_online_node(state_dir: &Path) -> TestResult {
    run_oa_node_json(&[
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(state_dir)?,
        "--json",
    ])?;
    run_oa_node_json(&[
        "admin",
        "desired-mode",
        "set",
        "online",
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

fn fixture_path(name: &str) -> Result<&'static str, Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/cloud/forge_assignment_v1")
        .join(name);
    let leaked = Box::leak(path.to_string_lossy().into_owned().into_boxed_str());
    Ok(leaked)
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
