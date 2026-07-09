use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn sandbox_profile_registration_projects_into_status_and_accepts_matching_assignment() -> TestResult
{
    let state_dir = unique_state_dir("sandbox-profile-accept");
    fs::create_dir_all(&state_dir)?;
    init_online_node(&state_dir)?;
    register_posix_profile(&state_dir)?;

    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/policy/sandbox_policy")
            .and_then(Value::as_str),
        Some("profile_enforced")
    );
    assert_eq!(
        status
            .pointer("/capabilities/sandbox_profiles/0/profile_id")
            .and_then(Value::as_str),
        Some("sandbox.posix.local")
    );
    assert_eq!(
        status
            .pointer("/capabilities/sandbox_profiles/0/profile_digest")
            .and_then(Value::as_str),
        Some("sha256:sandbox-posix-local-profile")
    );

    let receipt = run_oa_node_json(&[
        "forge",
        "assignment",
        "receive",
        "--file",
        fixture_path("sandbox-worker-assignment.json")
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
        Some("accepted_for_sandbox_profile")
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn sandbox_assignment_with_undeclared_network_policy_is_refused_and_receipted() -> TestResult {
    let state_dir = unique_state_dir("sandbox-profile-network-refusal");
    fs::create_dir_all(&state_dir)?;
    init_online_node(&state_dir)?;
    register_posix_profile(&state_dir)?;

    let mut assignment: Value = serde_json::from_str(&fs::read_to_string(fixture_path(
        "sandbox-worker-assignment.json",
    ))?)?;
    assignment["sandbox"]["network_policy"] = Value::String("host_inherit".to_string());
    let assignment_path = state_dir.join("network-mismatch-assignment.json");
    fs::write(&assignment_path, serde_json::to_string_pretty(&assignment)?)?;

    let receipt = run_oa_node_json(&[
        "forge",
        "assignment",
        "receive",
        "--file",
        assignment_path.to_str().ok_or("assignment path utf8")?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        receipt.pointer("/decision").and_then(Value::as_str),
        Some("refused")
    );
    assert_eq!(
        receipt.pointer("/reason").and_then(Value::as_str),
        Some("sandbox_network_policy_mismatch")
    );
    assert!(receipt
        .pointer("/receipt_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn sandbox_psionic_receipts_require_profile_digest() -> TestResult {
    let state_dir = unique_state_dir("sandbox-psionic-receipt");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json(&[
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let missing_profile = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .env_remove("OPENAGENTS_PSIONIC_ENDPOINT")
        .args([
            "psionic",
            "receipt",
            "append",
            "--product",
            "sandbox.posix.exec",
            "--worker",
            "sandbox.worker.local",
            "--assignment",
            "forge.assignment.sandbox.echo",
            "--evidence-digest",
            "sha256:psionic-sandbox-evidence",
            "--status",
            "succeeded",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(
        !missing_profile.status.success(),
        "sandbox psionic receipt without profile digest should fail"
    );

    let receipt = run_oa_node_json(&[
        "psionic",
        "receipt",
        "append",
        "--product",
        "sandbox.posix.exec",
        "--worker",
        "sandbox.worker.local",
        "--assignment",
        "forge.assignment.sandbox.echo",
        "--evidence-digest",
        "sha256:psionic-sandbox-evidence",
        "--profile-digest",
        "sha256:sandbox-posix-local-profile",
        "--status",
        "succeeded",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        receipt.pointer("/profile_digest").and_then(Value::as_str),
        Some("sha256:sandbox-posix-local-profile")
    );
    assert_eq!(
        receipt
            .pointer("/psionic_evidence_digest")
            .and_then(Value::as_str),
        Some("sha256:psionic-sandbox-evidence")
    );

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

fn register_posix_profile(state_dir: &Path) -> TestResult {
    run_oa_node_json(&[
        "sandbox",
        "profile",
        "register",
        "--profile-id",
        "sandbox.posix.local",
        "--profile-digest",
        "sha256:sandbox-posix-local-profile",
        "--execution-class",
        "sandbox.posix.exec",
        "--network-policy",
        "none",
        "--filesystem-policy",
        "workspace_only",
        "--timeout-ms",
        "60000",
        "--max-artifact-bytes",
        "10485760",
        "--secret-policy",
        "brokered_no_raw_secrets",
        "--state-dir",
        state_path(state_dir)?,
        "--json",
    ])?;
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
