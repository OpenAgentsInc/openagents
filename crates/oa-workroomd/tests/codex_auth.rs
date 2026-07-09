use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn codex_auth_materialize_status_and_scrub_are_redacted() -> TestResult {
    let state_dir = unique_state_dir("codex-auth");
    fs::create_dir_all(&state_dir)?;
    let grant_file = state_dir.join("grant.json");
    let auth_file = state_dir.join("broker-auth-cache.json");
    let fake_codex = state_dir.join("fake-codex.sh");
    let now = now_ms();
    let fake_auth_cache = "{\"kind\":\"test-codex-auth-cache\",\"value\":\"do-not-log-this\"}\n";

    fs::write(
        &grant_file,
        format!(
            r#"{{
  "contract_version": "openagents.codex_auth_grant.v1",
  "workroom_id": "workroom.codex.test",
  "user_ref": "user.test",
  "organization_ref": "org.test",
  "project_ref": "project.test",
  "provider_account_ref": "provider-account_codex_test",
  "grant_ref": "codex-auth-grant_test",
  "provider_secret_ref": "secret://codex/account/test",
  "requested_mode": "exec",
  "issued_at_ms": {now},
  "expires_at_ms": {},
  "audit_context": "vortex.issue.84"
}}"#,
            now + 1000 * 60 * 30,
        ),
    )?;
    fs::write(&auth_file, fake_auth_cache)?;
    write_fake_codex(&fake_codex)?;

    let materialized = run_workroomd_json(&[
        "codex",
        "auth",
        "materialize",
        "--grant-file",
        state_path(&grant_file)?,
        "--auth-json-file",
        state_path(&auth_file)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    let auth_json_path = materialized
        .pointer("/state/auth_json_path")
        .and_then(Value::as_str)
        .ok_or("missing auth_json_path")?;
    assert!(Path::new(auth_json_path).exists());
    assert_eq!(
        materialized
            .pointer("/receipt/event_kind")
            .and_then(Value::as_str),
        Some("grant_materialized")
    );
    assert_output_redacted(&materialized, fake_auth_cache);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(auth_json_path)?.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    let status = run_workroomd_json(&[
        "codex",
        "auth",
        "status",
        "--codex-bin",
        state_path(&fake_codex)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        status
            .pointer("/receipt/event_kind")
            .and_then(Value::as_str),
        Some("login_status_checked")
    );
    assert_eq!(
        status.pointer("/receipt/decision").and_then(Value::as_str),
        Some("accepted")
    );

    let scrubbed = run_workroomd_json(&[
        "codex",
        "auth",
        "scrub",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        scrubbed
            .pointer("/receipt/event_kind")
            .and_then(Value::as_str),
        Some("auth_material_scrubbed")
    );
    assert!(!Path::new(auth_json_path).exists());

    let receipt_log = fs::read_to_string(state_dir.join("codex-auth-receipts.jsonl"))?;
    assert_eq!(receipt_log.lines().count(), 3);
    assert!(!receipt_log.contains(fake_auth_cache.trim()));
    assert!(!receipt_log.to_ascii_lowercase().contains("do-not-log-this"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn expired_codex_auth_grant_is_refused_before_materialization() -> TestResult {
    let state_dir = unique_state_dir("codex-auth-expired");
    fs::create_dir_all(&state_dir)?;
    let grant_file = state_dir.join("expired-grant.json");
    let auth_file = state_dir.join("auth-cache.json");
    let now = now_ms();

    fs::write(
        &grant_file,
        format!(
            r#"{{
  "contract_version": "openagents.codex_auth_grant.v1",
  "workroom_id": "workroom.codex.expired",
  "user_ref": "user.test",
  "organization_ref": null,
  "project_ref": null,
  "provider_account_ref": "provider-account_codex_test",
  "grant_ref": "codex-auth-grant_expired",
  "provider_secret_ref": "secret://codex/account/test",
  "requested_mode": "exec",
  "issued_at_ms": {},
  "expires_at_ms": {},
  "audit_context": "expired-test"
}}"#,
            now - 1000 * 60 * 60,
            now - 1000,
        ),
    )?;
    fs::write(&auth_file, "{}\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_oa-workroomd"))
        .args([
            "codex",
            "auth",
            "materialize",
            "--grant-file",
            state_path(&grant_file)?,
            "--auth-json-file",
            state_path(&auth_file)?,
            "--state-dir",
            state_path(&state_dir)?,
            "--json",
        ])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("expired"));
    assert!(!state_dir.join("codex-auth-state.json").exists());

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

fn write_fake_codex(path: &Path) -> TestResult {
    fs::write(
        path,
        "#!/bin/sh\nif [ -z \"$CODEX_HOME\" ]; then exit 9; fi\nif [ \"$1\" = \"login\" ] && [ \"$2\" = \"status\" ]; then exit 0; fi\nexit 2\n",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn assert_output_redacted(output: &Value, secret: &str) {
    let rendered = serde_json::to_string(output).expect("json output should render");
    assert!(!rendered.contains(secret.trim()));
    assert!(!rendered.to_ascii_lowercase().contains("do-not-log-this"));
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

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn state_path(path: &Path) -> Result<&str, Box<dyn std::error::Error>> {
    path.to_str().ok_or_else(|| "path is not utf-8".into())
}
