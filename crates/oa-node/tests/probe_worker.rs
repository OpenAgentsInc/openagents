use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn probe_attachment_enforces_scoped_no_raw_secret_policy() -> TestResult {
    let state_dir = unique_state_dir("probe-attachment");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json(&[
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let attachment = run_oa_node_json(&[
        "probe",
        "attach",
        "--file",
        fixture_path("workroom-probe.json")
            .to_str()
            .ok_or("fixture path utf8")?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        attachment
            .pointer("/schema_version")
            .and_then(Value::as_str),
        Some("openagents.probe_worker_attachment.v1")
    );
    assert_eq!(
        attachment
            .pointer("/raw_secret_access")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(attachment
        .pointer("/capability_names")
        .and_then(Value::as_array)
        .is_some_and(|capabilities| capabilities.len() >= 2));

    let stored = fs::read_to_string(state_dir.join("probe-worker.json"))?;
    for forbidden in ["secret-token", "bearer ", "api_key=", "password=", "sk-"] {
        assert!(
            !stored.to_ascii_lowercase().contains(forbidden),
            "probe worker attachment must not store raw secret marker {forbidden}"
        );
    }

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn probe_closeout_persists_artifacts_and_projects_receipt() -> TestResult {
    let state_dir = unique_state_dir("probe-closeout");
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
        "probe",
        "closeout",
        "append",
        "--workroom",
        "workroom.local.echo",
        "--worker",
        "probe.worker.local",
        "--artifact",
        "artifact://probe/transcript",
        "--artifact",
        "artifact://probe/summary",
        "--status",
        "succeeded",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        receipt.pointer("/status").and_then(Value::as_str),
        Some("succeeded")
    );
    assert_eq!(
        receipt
            .pointer("/artifact_refs")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert!(receipt
        .pointer("/receipt_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));

    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/evidence/artifact_receipts")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    let receipt_log = fs::read_to_string(state_dir.join("probe-closeout-receipts.jsonl"))?;
    assert!(receipt_log.contains("artifact://probe/transcript"));
    assert!(!receipt_log.to_ascii_lowercase().contains("secret-token"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/cloud/probe_worker_attachment_v1")
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
