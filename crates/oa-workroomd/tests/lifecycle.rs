use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn lifecycle_transitions_are_validated_receipted_and_restart_safe() -> TestResult {
    let state_dir = unique_state_dir("lifecycle-transitions");
    fs::create_dir_all(&state_dir)?;

    let status = run_workroomd_json(&[
        "lifecycle",
        "status",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        status.pointer("/state").and_then(Value::as_str),
        Some("not_created")
    );

    let invalid = Command::new(env!("CARGO_BIN_EXE_oa-workroomd"))
        .args([
            "lifecycle",
            "start",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid lifecycle transition"));

    assert_lifecycle_action(&state_dir, "create", "created")?;
    assert_lifecycle_action(&state_dir, "start", "running")?;
    assert_lifecycle_action(&state_dir, "pause", "paused")?;
    assert_lifecycle_action(&state_dir, "resume", "running")?;
    assert_lifecycle_action(&state_dir, "expose", "exposed")?;

    let persisted = run_workroomd_json(&[
        "lifecycle",
        "status",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        persisted.pointer("/state").and_then(Value::as_str),
        Some("exposed")
    );
    assert_eq!(
        persisted
            .pointer("/receipts")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(5)
    );

    let receipt_log = fs::read_to_string(state_dir.join("lifecycle-receipts.jsonl"))?;
    assert_eq!(receipt_log.lines().count(), 5);
    assert!(receipt_log.contains("\"action\":\"expose\""));
    assert!(!receipt_log.to_ascii_lowercase().contains("secret-token"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn lifecycle_closeout_and_destroy_require_satisfied_artifact_policy() -> TestResult {
    let state_dir = unique_state_dir("lifecycle-closeout");
    fs::create_dir_all(&state_dir)?;
    let summary = state_dir.join("summary.md");
    fs::write(&summary, "# Summary\n\nReady.\n")?;

    assert_lifecycle_action(&state_dir, "create", "created")?;
    assert_lifecycle_action(&state_dir, "start", "running")?;
    run_workroomd_json(&[
        "artifacts",
        "policy",
        "init",
        "--required",
        "summary",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let blocked = Command::new(env!("CARGO_BIN_EXE_oa-workroomd"))
        .args([
            "lifecycle",
            "closeout",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("closeout policy is not satisfied"));

    run_workroomd_json(&[
        "artifacts",
        "upload",
        "--name",
        "summary",
        "--file",
        state_path(&summary)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    run_workroomd_json(&[
        "closeout",
        "submit",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_lifecycle_action(&state_dir, "closeout", "closed_out")?;
    assert_lifecycle_action(&state_dir, "destroy", "destroyed")?;

    let terminal = Command::new(env!("CARGO_BIN_EXE_oa-workroomd"))
        .args([
            "lifecycle",
            "start",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!terminal.status.success());
    assert!(String::from_utf8_lossy(&terminal.stderr).contains("destroyed is terminal"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn lifecycle_archive_then_destroy_is_explicit() -> TestResult {
    let state_dir = unique_state_dir("lifecycle-archive");
    fs::create_dir_all(&state_dir)?;

    assert_lifecycle_action(&state_dir, "create", "created")?;
    assert_lifecycle_action(&state_dir, "start", "running")?;
    assert_lifecycle_action(&state_dir, "closeout", "closed_out")?;
    assert_lifecycle_action(&state_dir, "archive", "archived")?;
    let destroyed = assert_lifecycle_action(&state_dir, "destroy", "destroyed")?;
    assert_eq!(
        destroyed
            .pointer("/receipt/from_state")
            .and_then(Value::as_str),
        Some("archived")
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn assert_lifecycle_action(
    state_dir: &Path,
    action: &str,
    expected_state: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let output = run_workroomd_json(&[
        "lifecycle",
        action,
        "--state-dir",
        state_path(state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        output.pointer("/state/state").and_then(Value::as_str),
        Some(expected_state)
    );
    assert_eq!(
        output.pointer("/receipt/action").and_then(Value::as_str),
        Some(action)
    );
    assert!(output
        .pointer("/receipt/receipt_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));
    Ok(output)
}

fn run_workroomd_json(args: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_oa-workroomd"))
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "oa-workroomd failed: status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn unique_state_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa-workroomd-{label}-{}-{}",
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
