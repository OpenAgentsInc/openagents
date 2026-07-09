use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn artifacts_are_content_addressed_and_closeout_fails_closed() -> TestResult {
    let state_dir = unique_state_dir("artifacts-closeout");
    fs::create_dir_all(&state_dir)?;
    let transcript = state_dir.join("transcript.txt");
    let summary = state_dir.join("summary.md");
    fs::write(&transcript, "hello from the workroom\n")?;
    fs::write(&summary, "# Summary\n\nDone.\n")?;

    run_workroomd_json(&[
        "artifacts",
        "policy",
        "init",
        "--required",
        "transcript",
        "--required",
        "summary",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let uploaded = run_workroomd_json(&[
        "artifacts",
        "upload",
        "--name",
        "transcript",
        "--file",
        state_path(&transcript)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    let digest = uploaded
        .pointer("/artifact/content_digest")
        .and_then(Value::as_str)
        .ok_or("missing artifact digest")?;
    assert!(digest.starts_with("sha256:"));
    let object_path = uploaded
        .pointer("/artifact/object_path")
        .and_then(Value::as_str)
        .ok_or("missing object path")?;
    assert!(state_dir.join(object_path).exists());
    assert_eq!(
        uploaded
            .pointer("/receipt/event_kind")
            .and_then(Value::as_str),
        Some("artifact_uploaded")
    );
    assert!(uploaded
        .pointer("/receipt/receipt_digest")
        .and_then(Value::as_str)
        .is_some_and(|value| value.starts_with("sha256:")));

    let blocked = Command::new(env!("CARGO_BIN_EXE_oa-workroomd"))
        .args([
            "closeout",
            "submit",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("missing required artifacts"));
    assert!(!state_dir.join("closeout-manifest.json").exists());

    run_workroomd_json(&[
        "artifacts",
        "upload",
        "--name",
        "summary",
        "--file",
        state_path(&summary)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    let closeout = run_workroomd_json(&[
        "closeout",
        "submit",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        closeout.pointer("/manifest/status").and_then(Value::as_str),
        Some("submitted")
    );
    assert_eq!(
        closeout
            .pointer("/manifest/artifact_digests")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert!(closeout
        .pointer("/manifest/manifest_digest")
        .and_then(Value::as_str)
        .is_some_and(|value| value.starts_with("sha256:")));
    assert_eq!(
        closeout
            .pointer("/receipt/event_kind")
            .and_then(Value::as_str),
        Some("closeout_submitted")
    );
    assert!(state_dir.join("closeout-manifest.json").exists());

    let receipt_log = fs::read_to_string(state_dir.join("artifact-receipts.jsonl"))?;
    assert_eq!(receipt_log.lines().count(), 3);
    assert!(receipt_log.contains("artifact_uploaded"));
    assert!(receipt_log.contains("closeout_submitted"));
    assert!(!receipt_log.to_ascii_lowercase().contains("secret-token"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn identical_artifact_content_uses_the_same_digest() -> TestResult {
    let state_dir = unique_state_dir("artifacts-digest");
    fs::create_dir_all(&state_dir)?;
    let first = state_dir.join("first.txt");
    let second = state_dir.join("second.txt");
    fs::write(&first, "same bytes\n")?;
    fs::write(&second, "same bytes\n")?;

    let first_upload = run_workroomd_json(&[
        "artifacts",
        "upload",
        "--name",
        "first",
        "--file",
        state_path(&first)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    let second_upload = run_workroomd_json(&[
        "artifacts",
        "upload",
        "--name",
        "second",
        "--file",
        state_path(&second)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        first_upload.pointer("/artifact/content_digest"),
        second_upload.pointer("/artifact/content_digest")
    );
    assert_eq!(
        first_upload.pointer("/artifact/object_path"),
        second_upload.pointer("/artifact/object_path")
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn artifact_policy_rejects_path_names_and_secret_markers() -> TestResult {
    let state_dir = unique_state_dir("artifacts-invalid");
    fs::create_dir_all(&state_dir)?;

    let output = Command::new(env!("CARGO_BIN_EXE_oa-workroomd"))
        .args([
            "artifacts",
            "policy",
            "init",
            "--required",
            "../secret-token",
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("artifact name must be a bounded name")
            || stderr.contains("artifact name contains forbidden marker")
    );

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
