use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

#[test]
fn init_is_idempotent_and_status_reports_identity() -> Result<(), Box<dyn std::error::Error>> {
    let state_dir = unique_state_dir("idempotent");
    fs::create_dir_all(&state_dir)?;

    let first = run_oa_node([
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_dir.to_str().ok_or("state dir is not utf-8")?,
        "--json",
    ])?;
    assert_eq!(
        first.pointer("/initialized").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        first.pointer("/existing").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        first.pointer("/identity/org_id").and_then(Value::as_str),
        Some("org.openagents.test")
    );
    assert_eq!(
        first
            .pointer("/identity/signing_key_ref")
            .and_then(Value::as_str),
        Some("configured")
    );
    let node_id = first
        .pointer("/identity/node_id")
        .and_then(Value::as_str)
        .ok_or("init output missing node id")?
        .to_string();

    let second = run_oa_node([
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_dir.to_str().ok_or("state dir is not utf-8")?,
        "--json",
    ])?;
    assert_eq!(
        second.pointer("/existing").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        second.pointer("/identity/node_id").and_then(Value::as_str),
        Some(node_id.as_str())
    );

    let status = run_oa_node([
        "status",
        "--state-dir",
        state_dir.to_str().ok_or("state dir is not utf-8")?,
        "--json",
    ])?;
    assert_eq!(
        status.pointer("/contract_version").and_then(Value::as_str),
        Some("openagents.cloud_node.v1")
    );
    assert_eq!(
        status.pointer("/identity/node_id").and_then(Value::as_str),
        Some(node_id.as_str())
    );
    assert_eq!(
        status
            .pointer("/identity/operator_identity")
            .and_then(Value::as_str),
        Some("org.openagents.test")
    );
    assert_eq!(
        status
            .pointer("/identity/account_or_org_binding")
            .and_then(Value::as_str),
        Some("org.openagents.test")
    );

    let state = fs::read_to_string(state_dir.join("node-state.json"))?;
    for forbidden in [
        "wallet_seed",
        "node_entropy",
        "private_key",
        "preimage",
        "bearer_token",
        "api_key",
    ] {
        assert!(
            !state.contains(forbidden),
            "local state must not contain {forbidden}"
        );
        assert!(
            !serde_json::to_string(&first)?.contains(forbidden),
            "init output must not contain {forbidden}"
        );
    }

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn run_oa_node<const N: usize>(args: [&str; N]) -> Result<Value, Box<dyn std::error::Error>> {
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
