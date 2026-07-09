use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn metadata_get_returns_non_secret_context_and_logs_access() -> TestResult {
    let state_dir = unique_state_dir("metadata");
    fs::create_dir_all(&state_dir)?;
    let init = run_workroomd_json(&[
        "metadata",
        "init",
        "--workroom",
        "workroom.local.echo",
        "--program",
        "program.local.smoke",
        "--repo",
        "repo.openagents.echo",
        "--template",
        "template.posix.echo",
        "--budget",
        "runtime_ms=60000,cost_microusd=1000000",
        "--deadline",
        "2026-05-25T12:00:00Z",
        "--trust-tier",
        "internal_test",
        "--capability",
        "repo.read",
        "--capability",
        "artifact.write",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        init.pointer("/workroom_id").and_then(Value::as_str),
        Some("workroom.local.echo")
    );

    let metadata = run_workroomd_json(&[
        "metadata",
        "get",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        metadata.pointer("/repo").and_then(Value::as_str),
        Some("repo.openagents.echo")
    );
    assert_eq!(
        metadata
            .pointer("/capability_names")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    let serialized = serde_json::to_string(&metadata)?;
    for forbidden in [
        "secret-token",
        "bearer ",
        "api_key",
        "wallet_seed",
        "private_key",
        "tailnet",
    ] {
        assert!(
            !serialized.to_ascii_lowercase().contains(forbidden),
            "metadata must not contain {forbidden}"
        );
    }

    let access_log = fs::read_to_string(state_dir.join("metadata-access.jsonl"))?;
    assert_eq!(access_log.lines().count(), 1);
    assert!(access_log.contains("metadata_get"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn metadata_init_rejects_secret_markers() -> TestResult {
    let state_dir = unique_state_dir("metadata-secret");
    fs::create_dir_all(&state_dir)?;
    let output = Command::new(env!("CARGO_BIN_EXE_oa-workroomd"))
        .args([
            "metadata",
            "init",
            "--workroom",
            "workroom.local.echo",
            "--program",
            "program.local.smoke",
            "--repo",
            "repo.openagents.echo",
            "--template",
            "template.posix.echo",
            "--budget",
            "api_key=secret-token",
            "--deadline",
            "2026-05-25T12:00:00Z",
            "--trust-tier",
            "internal_test",
            "--capability",
            "repo.read",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("metadata contains secret"));

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
