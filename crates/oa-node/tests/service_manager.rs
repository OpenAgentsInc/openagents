use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn launchd_service_lifecycle_updates_node_health_without_secret_logs() -> TestResult {
    let state_dir = unique_state_dir("service-launchd");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;

    let installed = run_oa_node_json(&[
        "service",
        "install",
        "--manager",
        "launchd",
        "--service-name",
        "openagents-oa-node-test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        installed
            .pointer("/service/service_manager")
            .and_then(Value::as_str),
        Some("launchd")
    );
    assert_eq!(
        installed
            .pointer("/service/observed_status")
            .and_then(Value::as_str),
        Some("installed")
    );

    assert_service_action(&state_dir, "start", "running")?;
    let online = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        online
            .pointer("/lifecycle/observed_status")
            .and_then(Value::as_str),
        Some("online")
    );
    assert_eq!(
        online
            .pointer("/lifecycle/service_manager")
            .and_then(Value::as_str),
        Some("launchd")
    );

    assert_service_action(&state_dir, "stop", "stopped")?;
    let offline = run_oa_node_json(&["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        offline
            .pointer("/lifecycle/observed_status")
            .and_then(Value::as_str),
        Some("offline")
    );

    assert_service_action(&state_dir, "restart", "running")?;
    assert_service_action(&state_dir, "uninstall", "uninstalled")?;
    let status = run_oa_node_json(&[
        "service",
        "status",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        status
            .pointer("/service/service_manager")
            .and_then(Value::as_str),
        Some("manual")
    );

    let start_after_uninstall = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .args([
            "service",
            "start",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!start_after_uninstall.status.success());
    assert!(
        String::from_utf8_lossy(&start_after_uninstall.stderr).contains("service is not installed")
    );

    let events = fs::read_to_string(state_dir.join("service-events.jsonl"))?;
    assert_eq!(events.lines().count(), 5);
    assert!(events.contains("\"service_manager\":\"launchd\""));
    assert!(!events.to_ascii_lowercase().contains("secret-token"));
    assert!(!events.to_ascii_lowercase().contains("private_key"));

    let health = run_oa_node_json(&[
        "admin",
        "health",
        "list",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        health
            .pointer("/events")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(5)
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn systemd_service_install_is_supported() -> TestResult {
    let state_dir = unique_state_dir("service-systemd");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;

    let installed = run_oa_node_json(&[
        "service",
        "install",
        "--manager",
        "systemd",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        installed
            .pointer("/service/service_manager")
            .and_then(Value::as_str),
        Some("systemd")
    );
    assert_service_action(&state_dir, "start", "running")?;

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn service_install_rejects_secret_markers() -> TestResult {
    let state_dir = unique_state_dir("service-invalid");
    fs::create_dir_all(&state_dir)?;
    init_node(&state_dir)?;

    let output = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .args([
            "service",
            "install",
            "--manager",
            "launchd",
            "--service-name",
            "secret-token-service",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("service name"));
    assert!(!state_dir.join("service-events.jsonl").exists());

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn assert_service_action(
    state_dir: &Path,
    action: &str,
    expected_status: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let output = run_oa_node_json(&[
        "service",
        action,
        "--state-dir",
        state_path(state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        output
            .pointer("/service/observed_status")
            .and_then(Value::as_str),
        Some(expected_status)
    );
    assert_eq!(
        output.pointer("/action").and_then(Value::as_str),
        Some(action)
    );
    Ok(output)
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
