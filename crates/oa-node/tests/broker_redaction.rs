use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn broker_redacts_supported_payload_kinds_without_leaking_artifacts_or_receipts() -> TestResult {
    let state_dir = unique_state_dir("broker-redaction");
    fs::create_dir_all(&state_dir)?;

    let samples = [
        (
            "headers",
            "OPENAGENTS_FAKE_SECRET_OK\nauthorization: Bearer secret-token\nx-safe: ok\n",
        ),
        (
            "url",
            "OPENAGENTS_FAKE_SECRET_OK\nhttps://example.invalid/path?api_key=secret-token\n",
        ),
        (
            "env",
            "OPENAGENTS_FAKE_SECRET_OK\nPASSWORD=secret-token\nSAFE=value\n",
        ),
        (
            "config",
            "OPENAGENTS_FAKE_SECRET_OK\nprivate_key=secret-token\nmode=test\n",
        ),
        (
            "log",
            "OPENAGENTS_FAKE_SECRET_OK\nmodel call used sk-secret-token\nok\n",
        ),
        (
            "receipt",
            "OPENAGENTS_FAKE_SECRET_OK\nwallet_seed=secret-token\nreceipt_digest=sha256:test\n",
        ),
    ];

    for (kind, body) in samples {
        let input = state_dir.join(format!("{kind}.txt"));
        fs::write(&input, body)?;
        let output = run_oa_node_json(&[
            "broker",
            "redact",
            "--kind",
            kind,
            "--input",
            state_path(&input)?,
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])?;
        assert_eq!(
            output.pointer("/receipt/kind").and_then(Value::as_str),
            Some(kind)
        );
        let artifact = output
            .pointer("/receipt/redacted_artifact_path")
            .and_then(Value::as_str)
            .ok_or("missing artifact path")?;
        let artifact_body = fs::read_to_string(state_dir.join(artifact))?;
        assert!(artifact_body.contains("[REDACTED]"));
        for forbidden in [
            "secret-token",
            "bearer ",
            "api_key",
            "password",
            "private_key",
            "wallet_seed",
            "sk-",
        ] {
            assert!(
                !artifact_body.to_ascii_lowercase().contains(forbidden),
                "redacted artifact leaked {forbidden} for kind {kind}"
            );
        }
    }

    let receipts = fs::read_to_string(state_dir.join("broker-redaction-receipts.jsonl"))?;
    assert_eq!(receipts.lines().count(), 6);
    for forbidden in [
        "secret-token",
        "bearer ",
        "api_key",
        "password",
        "private_key",
        "wallet_seed",
        "sk-",
    ] {
        assert!(
            !receipts.to_ascii_lowercase().contains(forbidden),
            "broker receipt log leaked {forbidden}"
        );
    }

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn broker_rejects_unmarked_secret_looking_fixtures() -> TestResult {
    let state_dir = unique_state_dir("broker-secret");
    fs::create_dir_all(&state_dir)?;
    let input = state_dir.join("headers.txt");
    fs::write(&input, "authorization: Bearer secret-token\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .args([
            "broker",
            "redact",
            "--kind",
            "headers",
            "--input",
            state_path(&input)?,
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("fake-secret marker"));
    assert!(!state_dir.join("broker-redaction-receipts.jsonl").exists());
    assert!(!state_dir.join("broker-redacted-artifacts").exists());

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn broker_rejects_unknown_redaction_kind() -> TestResult {
    let state_dir = unique_state_dir("broker-kind");
    fs::create_dir_all(&state_dir)?;
    let input = state_dir.join("input.txt");
    fs::write(&input, "safe\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .args([
            "broker",
            "redact",
            "--kind",
            "screenshot",
            "--input",
            state_path(&input)?,
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported"));

    fs::remove_dir_all(&state_dir)?;
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
    path.to_str().ok_or_else(|| "path is not utf-8".into())
}
