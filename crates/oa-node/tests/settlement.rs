use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn managed_nodes_default_to_no_wallet_settlement() -> TestResult {
    let state_dir = unique_state_dir("settlement-default");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;

    let settlement = run_oa_node_json(&[
        "settlement",
        "status",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        settlement
            .pointer("/settlement/mode")
            .and_then(Value::as_str),
        Some("no-wallet")
    );
    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/policy/settlement_policy")
            .and_then(Value::as_str),
        Some("no_wallet")
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn internal_accounting_receipts_reconcile_with_treasury_and_nexus_refs() -> TestResult {
    let state_dir = unique_state_dir("settlement-internal");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;

    let mode = run_oa_node_json(&[
        "settlement",
        "mode",
        "set",
        "internal-accounting",
        "--treasury-ref",
        "treasury://batch/local-1",
        "--nexus-ref",
        "nexus://settlement/local-1",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        mode.pointer("/settlement/mode").and_then(Value::as_str),
        Some("internal-accounting")
    );

    let receipt = run_oa_node_json(&[
        "settlement",
        "receipt",
        "append",
        "--amount-microusd",
        "12345",
        "--treasury-ref",
        "treasury://batch/local-1",
        "--nexus-ref",
        "nexus://settlement/local-1",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        receipt.pointer("/receipt/result").and_then(Value::as_str),
        Some("reconciled")
    );
    assert_eq!(
        receipt
            .pointer("/receipt/treasury_ref")
            .and_then(Value::as_str),
        Some("treasury://batch/local-1")
    );
    assert_eq!(
        receipt
            .pointer("/receipt/nexus_ref")
            .and_then(Value::as_str),
        Some("nexus://settlement/local-1")
    );
    assert!(receipt
        .pointer("/receipt/receipt_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));

    let status = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/policy/settlement_policy")
            .and_then(Value::as_str),
        Some("internal_accounting")
    );
    assert_eq!(
        status
            .pointer("/evidence/payout_or_accounting_receipts")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    let log = fs::read_to_string(state_dir.join("settlement-receipts.jsonl"))?;
    assert!(log.contains("treasury://batch/local-1"));
    assert!(log.contains("nexus://settlement/local-1"));
    assert!(!log.to_ascii_lowercase().contains("wallet_seed"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn contributor_wallet_mode_is_refused_in_cloud() -> TestResult {
    let state_dir = unique_state_dir("settlement-contributor");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;

    let output = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .args([
            "settlement",
            "mode",
            "set",
            "contributor-wallet",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("public Pylon"),
        "cloud must keep contributor-wallet behavior outside this repo"
    );
    assert!(!state_dir.join("settlement-receipts.jsonl").exists());

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn settlement_receipts_require_internal_accounting_mode() -> TestResult {
    let state_dir = unique_state_dir("settlement-requires-mode");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;

    let output = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .args([
            "settlement",
            "receipt",
            "append",
            "--amount-microusd",
            "100",
            "--treasury-ref",
            "treasury://batch/local-1",
            "--nexus-ref",
            "nexus://settlement/local-1",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("internal-accounting"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
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
