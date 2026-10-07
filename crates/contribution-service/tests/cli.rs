//! CLI checks use isolated private state and never open a resident wallet.
use contribution_service::types::{Config, SCHEMA};
use gym::sales_evidence::Reference;
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn explicit_private_statement_has_no_funding_and_never_defaults_a_wallet() {
    let scratch = tempfile::tempdir_in(fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let root = scratch.path();
    let protected = root.join("protected");
    let worker = root.join("worker");
    for p in [&protected, &worker] {
        fs::create_dir(p).unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let reference = Reference {
        path: "absent.json".into(),
        sha256: "a".repeat(64),
    };
    let config = Config {
        schema: SCHEMA.into(),
        authority: "a".repeat(64),
        protected_root: protected,
        worker_root: worker,
        state: root.join("state"),
        ledger: root.join("central.sqlite"),
        frozen: reference.clone(),
        current: "current.json".into(),
        evaluation: reference.clone(),
        acceptance: reference,
    };
    drop(pay_ledger::Ledger::open(&config.ledger).unwrap());
    fs::set_permissions(&config.ledger, fs::Permissions::from_mode(0o600)).unwrap();
    let path = root.join("config.json");
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_contribution-service"))
        .env_clear()
        .env("HOME", root)
        .arg("--config")
        .arg(&path)
        .arg("statement")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value["funding"].is_null());
    assert!(value["settlement"].is_null());
    assert_eq!(value["independent_commercial_qualification"], "unverified");
    let refused = Command::new(env!("CARGO_BIN_EXE_contribution-service"))
        .env_clear()
        .env("HOME", root)
        .arg("--config")
        .arg(&path)
        .arg("prepare")
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("explicit absolute"));
    assert!(!root.join(".openagents").exists());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(contribution_service::load_config(&path).is_err());
}
