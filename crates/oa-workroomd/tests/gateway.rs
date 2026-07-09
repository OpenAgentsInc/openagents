use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn gateway_access_is_policy_checked_and_logged() -> TestResult {
    let state_dir = unique_state_dir("gateway-access");
    fs::create_dir_all(&state_dir)?;
    run_workroomd_json(&[
        "gateway",
        "policy",
        "init",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let allowed = run_workroomd_json(&[
        "gateway",
        "access",
        "--gateway",
        "model",
        "--capability",
        "model.gateway",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        allowed.pointer("/decision").and_then(Value::as_str),
        Some("allowed")
    );

    let denied = run_workroomd_json(&[
        "gateway",
        "access",
        "--gateway",
        "model",
        "--capability",
        "email.send_receive",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        denied.pointer("/decision").and_then(Value::as_str),
        Some("denied")
    );
    assert_eq!(
        denied.pointer("/reason").and_then(Value::as_str),
        Some("capability_not_allowed_for_gateway")
    );

    let access_log = fs::read_to_string(state_dir.join("gateway-access.jsonl"))?;
    assert_eq!(access_log.lines().count(), 2);
    assert!(!access_log.to_ascii_lowercase().contains("secret-token"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn revoked_capability_stops_working_without_restart() -> TestResult {
    let state_dir = unique_state_dir("gateway-revoke");
    fs::create_dir_all(&state_dir)?;
    run_workroomd_json(&[
        "gateway",
        "access",
        "--gateway",
        "artifacts",
        "--capability",
        "artifact.write",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    run_workroomd_json(&[
        "gateway",
        "revoke",
        "--capability",
        "artifact.write",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    let denied = run_workroomd_json(&[
        "gateway",
        "access",
        "--gateway",
        "artifacts",
        "--capability",
        "artifact.write",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        denied.pointer("/decision").and_then(Value::as_str),
        Some("denied")
    );
    assert_eq!(
        denied.pointer("/reason").and_then(Value::as_str),
        Some("capability_revoked")
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
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
