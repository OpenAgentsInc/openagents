use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn psionic_attachment_projects_readiness_and_crash_per_product() -> TestResult {
    let state_dir = unique_state_dir("psionic-attachment");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json(&[
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let attached = run_oa_node_json(&[
        "psionic",
        "attach",
        "--file",
        fixture_path("mixed-readiness.json")
            .to_str()
            .ok_or("fixture path utf8")?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        attached.pointer("/schema_version").and_then(Value::as_str),
        Some("openagents.psionic_worker_attachment.v1")
    );
    assert_eq!(
        attached
            .pointer("/workers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3)
    );

    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/capabilities/inference_products/0/product_id")
            .and_then(Value::as_str),
        Some("psionic.managed.inference")
    );
    assert_eq!(
        status
            .pointer("/capabilities/inference_products/0/backend_ready")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        status
            .pointer("/capabilities/training_products/0/product_id")
            .and_then(Value::as_str),
        Some("psionic.managed.training")
    );
    assert_eq!(
        status
            .pointer("/capabilities/training_products/0/backend_ready")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(status
        .pointer("/capabilities/training_products/0/capability_summary")
        .and_then(Value::as_str)
        .is_some_and(|summary| summary.contains("worker_crashed")));
    assert_eq!(
        status
            .pointer("/lifecycle/observed_status")
            .and_then(Value::as_str),
        Some("offline")
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn psionic_execution_receipt_cites_evidence_digest() -> TestResult {
    let state_dir = unique_state_dir("psionic-receipt");
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
        "psionic",
        "receipt",
        "append",
        "--product",
        "psionic.managed.inference",
        "--worker",
        "psionic.inference.local",
        "--assignment",
        "forge.assignment.workroom.echo",
        "--evidence-digest",
        "sha256:psionic-execution-evidence",
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
            .pointer("/psionic_evidence_digest")
            .and_then(Value::as_str),
        Some("sha256:psionic-execution-evidence")
    );
    assert!(receipt
        .pointer("/receipt_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));

    let receipt_log = fs::read_to_string(state_dir.join("psionic-execution-receipts.jsonl"))?;
    let logged: Value = serde_json::from_str(receipt_log.lines().next().ok_or("missing receipt")?)?;
    assert_eq!(
        logged
            .pointer("/psionic_evidence_digest")
            .and_then(Value::as_str),
        Some("sha256:psionic-execution-evidence")
    );

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

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/cloud/psionic_worker_attachment_v1")
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
