use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn ingress_defaults_private_and_public_changes_emit_receipts() -> TestResult {
    let state_dir = unique_state_dir("ingress-public");
    fs::create_dir_all(&state_dir)?;

    let status = run_workroomd_json(&[
        "ingress",
        "status",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        status.pointer("/visibility").and_then(Value::as_str),
        Some("private")
    );
    assert_eq!(
        status
            .pointer("/receipts")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let public = run_workroomd_json(&[
        "ingress",
        "set",
        "--visibility",
        "public",
        "--preview-url",
        "https://preview.example.invalid/workroom.local.echo",
        "--custom-domain",
        "preview.example.invalid",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        public.pointer("/visibility").and_then(Value::as_str),
        Some("public")
    );
    assert_eq!(
        public.pointer("/preview_url").and_then(Value::as_str),
        Some("https://preview.example.invalid/workroom.local.echo")
    );
    assert_eq!(
        public.pointer("/custom_domain").and_then(Value::as_str),
        Some("preview.example.invalid")
    );
    assert_eq!(
        receipt_event(&public, 0),
        Some("preview_exposed"),
        "public exposure must emit a preview receipt"
    );
    assert_digest(&public, 0);

    let revoked = run_workroomd_json(&[
        "ingress",
        "revoke",
        "--target",
        "public",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        revoked.pointer("/visibility").and_then(Value::as_str),
        Some("private")
    );
    assert!(revoked.pointer("/preview_url").is_some_and(Value::is_null));
    assert_eq!(receipt_event(&revoked, 1), Some("ingress_revoked"));
    assert_digest(&revoked, 1);

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn collaborator_grants_and_revocations_are_receipted() -> TestResult {
    let state_dir = unique_state_dir("ingress-collaborator");
    fs::create_dir_all(&state_dir)?;

    let granted = run_workroomd_json(&[
        "ingress",
        "collaborator",
        "grant",
        "--identity",
        "github:OpenAgentsInc/alice",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        granted.pointer("/visibility").and_then(Value::as_str),
        Some("collaborators")
    );
    assert_eq!(
        granted
            .pointer("/collaborator_grants/0")
            .and_then(Value::as_str),
        Some("github:OpenAgentsInc/alice")
    );
    assert_eq!(receipt_event(&granted, 0), Some("collaborator_granted"));
    assert_eq!(receipt_event(&granted, 1), Some("preview_exposed"));
    assert_digest(&granted, 0);
    assert_digest(&granted, 1);

    let revoked = run_workroomd_json(&[
        "ingress",
        "revoke",
        "--target",
        "github:OpenAgentsInc/alice",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        revoked
            .pointer("/collaborator_grants")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(receipt_event(&revoked, 2), Some("ingress_revoked"));
    assert_digest(&revoked, 2);

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn endpoint_tokens_are_digested_and_revocable() -> TestResult {
    let state_dir = unique_state_dir("ingress-token");
    fs::create_dir_all(&state_dir)?;

    let minted = run_workroomd_json(&[
        "ingress",
        "token",
        "mint",
        "--label",
        "preview-collaborator",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    let digest = minted
        .pointer("/endpoint_token_digests/0")
        .and_then(Value::as_str)
        .ok_or("missing endpoint token digest")?;
    assert!(digest.starts_with("sha256:"));
    assert_eq!(receipt_event(&minted, 0), Some("endpoint_token_minted"));
    assert_digest(&minted, 0);
    let serialized = serde_json::to_string(&minted)?;
    for forbidden in ["secret-token", "bearer ", "api_key", "private_key"] {
        assert!(!serialized.to_ascii_lowercase().contains(forbidden));
    }

    let revoked = run_workroomd_json(&[
        "ingress",
        "revoke",
        "--target",
        digest,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        revoked
            .pointer("/endpoint_token_digests")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(receipt_event(&revoked, 1), Some("ingress_revoked"));
    assert_digest(&revoked, 1);

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn ingress_rejects_invalid_visibility() -> TestResult {
    let state_dir = unique_state_dir("ingress-invalid");
    fs::create_dir_all(&state_dir)?;

    let output = Command::new(env!("CARGO_BIN_EXE_oa-workroomd"))
        .args([
            "ingress",
            "set",
            "--visibility",
            "internet",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported ingress visibility"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn receipt_event(value: &Value, index: usize) -> Option<&str> {
    value
        .pointer(&format!("/receipts/{index}/event_kind"))
        .and_then(Value::as_str)
}

fn assert_digest(value: &Value, index: usize) {
    assert!(
        value
            .pointer(&format!("/receipts/{index}/digest"))
            .and_then(Value::as_str)
            .is_some_and(|digest| digest.starts_with("sha256:")),
        "receipt {index} should contain a sha256 digest"
    );
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
