use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

#[test]
fn admin_store_persists_desired_mode_and_health_events() -> Result<(), Box<dyn std::error::Error>> {
    let state_dir = unique_state_dir("admin-store");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json([
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let set = run_oa_node_json([
        "admin",
        "desired-mode",
        "set",
        "online",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        set.pointer("/desired_mode").and_then(Value::as_str),
        Some("online")
    );

    let get = run_oa_node_json([
        "admin",
        "desired-mode",
        "get",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        get.pointer("/desired_mode").and_then(Value::as_str),
        Some("online")
    );

    run_oa_node_json([
        "admin",
        "health",
        "append",
        "--severity",
        "warn",
        "--code",
        "disk_low",
        "--detail",
        "test disk warning",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    run_oa_node_json([
        "admin",
        "health",
        "append",
        "--severity",
        "info",
        "--code",
        "operator_note",
        "--detail",
        "test operator note",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let listed = run_oa_node_json([
        "admin",
        "health",
        "list",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        listed
            .pointer("/events")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );

    let status = run_oa_node_json(["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/lifecycle/desired_mode")
            .and_then(Value::as_str),
        Some("online")
    );
    assert_eq!(
        status
            .pointer("/evidence/health_events")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );

    let event_log = fs::read_to_string(state_dir.join("health-events.jsonl"))?;
    assert_eq!(event_log.lines().count(), 2);
    let admin_store: Value =
        serde_json::from_str(&fs::read_to_string(state_dir.join("admin-store.json"))?)?;
    assert_eq!(
        admin_store
            .pointer("/receipt_cursors/health_event_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        admin_store
            .pointer("/observed_status")
            .and_then(Value::as_str),
        Some("offline")
    );
    assert_eq!(
        admin_store
            .pointer("/inventory/items")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        admin_store
            .pointer("/updates/current_version")
            .and_then(Value::as_str),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        admin_store
            .pointer("/quarantine/quarantined")
            .and_then(Value::as_bool),
        Some(false)
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn corrupt_admin_store_degrades_status_safely() -> Result<(), Box<dyn std::error::Error>> {
    let state_dir = unique_state_dir("corrupt-admin-store");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json([
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    fs::write(state_dir.join("admin-store.json"), "{not valid json\n")?;

    let status = run_oa_node_json(["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status
            .pointer("/lifecycle/observed_status")
            .and_then(Value::as_str),
        Some("degraded")
    );
    assert!(status
        .pointer("/lifecycle/degradation_reason")
        .and_then(Value::as_str)
        .is_some_and(|reason| reason.contains("admin_store_corrupt")));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn run_oa_node_json<const N: usize>(args: [&str; N]) -> Result<Value, Box<dyn std::error::Error>> {
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
