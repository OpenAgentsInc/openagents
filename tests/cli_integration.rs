//! Integration tests for unified openagents binary CLI
//!
//! These tests verify that all CLI subcommands parse correctly and delegate
//! to the appropriate crate handlers.
//!
//! Note: Some tests check commands that may block (like GUI launches),
//! so we only test --help for those commands.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn create_temp_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!("openagents-test-{}", Uuid::new_v4()));
    fs::create_dir_all(root.join(".openagents")).expect("create .openagents");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write Cargo.toml");
    root
}

#[cfg(unix)]
use std::path::Path;

#[cfg(unix)]
fn write_stub_script(dir: &Path, name: &str) -> PathBuf {
    let script_path = dir.join(name);
    let script = r#"#!/bin/sh
if [ -z "$LOG_PATH" ]; then
  echo "LOG_PATH not set" >&2
  exit 1
fi
printf '%s\n' "$@" > "$LOG_PATH"
"#;
    fs::write(&script_path, script).expect("write stub script");
    let mut perms = fs::metadata(&script_path)
        .expect("stub metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).expect("chmod stub");
    script_path
}

/// Test that the binary exists and shows help
#[test]
fn test_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OpenAgents"))
        .stdout(predicate::str::contains("wallet"))
        .stdout(predicate::str::contains("marketplace"))
        .stdout(predicate::str::contains("autopilot"))
        .stdout(predicate::str::contains("gitafter"))
        .stdout(predicate::str::contains("daemon"));
}

/// Test version flag
#[test]
fn test_version() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("openagents"));
}

#[test]
fn test_openagents_no_args_headless() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.env("OPENAGENTS_HEADLESS", "1");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OpenAgents GUI disabled"));
}

// Wallet commands

#[test]
fn test_wallet_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("wallet").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wallet commands"))
        .stdout(predicate::str::contains("gui"));
}

#[test]
fn test_wallet_receive_help_lists_expiry() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("wallet").arg("receive").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--expiry"));
}

#[test]
fn test_wallet_whoami_no_wallet() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("wallet").arg("whoami");
    // Should fail gracefully if no wallet exists
    cmd.assert().failure();
}

#[test]
fn test_wallet_balance_no_wallet() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("wallet").arg("balance");
    // Should fail gracefully if no wallet exists
    cmd.assert().failure();
}

#[test]
fn test_wallet_init_creates_mnemonic() {
    let workspace = create_temp_workspace();
    let keychain_path = workspace.join("keychain.txt");

    if keychain_path.exists() {
        fs::remove_file(&keychain_path).expect("clean keychain file");
    }

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.current_dir(&workspace)
        .env("OPENAGENTS_KEYCHAIN_FILE", &keychain_path)
        .arg("wallet")
        .arg("init")
        .arg("--show-mnemonic");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wallet initialized"));

    let mnemonic = fs::read_to_string(&keychain_path).expect("read keychain file");
    let word_count = mnemonic.split_whitespace().count();
    assert!(word_count == 12 || word_count == 24, "unexpected word count");

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

// Marketplace commands

#[test]
fn test_marketplace_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("marketplace").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Marketplace commands"));
}

#[test]
fn test_marketplace_compute_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("marketplace").arg("compute").arg("--help");
    cmd.assert().success();
}

#[test]
fn test_marketplace_skills_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("marketplace").arg("skills").arg("--help");
    cmd.assert().success();
}

#[test]
fn test_marketplace_data_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("marketplace").arg("data").arg("--help");
    cmd.assert().success();
}

// Autopilot commands

#[test]
fn test_autopilot_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("autopilot").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Autopilot commands"));
}

#[test]
fn test_autopilot_run_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("autopilot").arg("run").arg("--help");
    cmd.assert().success();
}

#[test]
fn test_autopilot_metrics_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("autopilot").arg("metrics").arg("--help");
    cmd.assert().success();
}

// GitAfter commands

#[test]
fn test_gitafter_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("gitafter").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("GitAfter commands"));
}

#[test]
fn test_gitafter_repos_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("gitafter").arg("repos").arg("--help");
    cmd.assert().success();
}

// Daemon commands

#[test]
fn test_daemon_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("daemon").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Daemon commands"));
}

#[test]
fn test_daemon_status_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("daemon").arg("status").arg("--help");
    cmd.assert().success();
}

#[test]
fn test_daemon_start_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("daemon").arg("start").arg("--help");
    cmd.assert().success();
}

// Verbose flag

#[test]
fn test_verbose_flag() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("--verbose").arg("--help");
    cmd.assert().success();
}

// Invalid commands

#[test]
fn test_invalid_command() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("nonexistent");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn test_invalid_subcommand_wallet() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("wallet").arg("nonexistent");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

// Command structure verification

#[test]
fn test_wallet_subcommands_listed() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("wallet").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("whoami"))
        .stdout(predicate::str::contains("balance"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("send"))
        .stdout(predicate::str::contains("receive"))
        .stdout(predicate::str::contains("notify"))
        .stdout(predicate::str::contains("history"))
        .stdout(predicate::str::contains("identity"))
        .stdout(predicate::str::contains("payee"));
}

#[test]
fn test_marketplace_subcommands_listed() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("marketplace").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("compute"))
        .stdout(predicate::str::contains("skills"))
        .stdout(predicate::str::contains("data"));
}

#[test]
fn test_autopilot_subcommands_listed() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("autopilot").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("resume"))
        .stdout(predicate::str::contains("metrics"));
}

#[test]
#[cfg(unix)]
fn test_autopilot_run_delegates_to_autopilot_bin() {
    let workspace = create_temp_workspace();
    let log_path = workspace.join("autopilot.log");
    let stub = write_stub_script(&workspace, "autopilot-stub.sh");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.current_dir(&workspace)
        .env("OPENAGENTS_AUTOPILOT_BIN", &stub)
        .env("LOG_PATH", &log_path)
        .arg("autopilot")
        .arg("run")
        .arg("ship-it");

    cmd.assert().success();

    let output = fs::read_to_string(&log_path).expect("read stub log");
    assert!(output.contains("run"), "expected run arg");
    assert!(output.contains("ship-it"), "expected prompt arg");

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
#[cfg(unix)]
fn test_autopilot_run_delegates_gpt_oss_agent() {
    let workspace = create_temp_workspace();
    let log_path = workspace.join("autopilot-gpt-oss.log");
    let stub = write_stub_script(&workspace, "autopilot-stub.sh");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.current_dir(&workspace)
        .env("OPENAGENTS_AUTOPILOT_BIN", &stub)
        .env("LOG_PATH", &log_path)
        .arg("autopilot")
        .arg("run")
        .arg("local-run")
        .arg("--agent")
        .arg("gpt-oss")
        .arg("--model")
        .arg("gpt-oss-20b");

    cmd.assert().success();

    let output = fs::read_to_string(&log_path).expect("read stub log");
    assert!(output.contains("--agent"), "expected agent flag");
    assert!(output.contains("gpt-oss"), "expected gpt-oss agent");
    assert!(output.contains("--model"), "expected model flag");
    assert!(output.contains("gpt-oss-20b"), "expected gpt-oss model");

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
#[cfg(unix)]
fn test_autopilot_run_delegates_fm_bridge_agent() {
    let workspace = create_temp_workspace();
    let log_path = workspace.join("autopilot-fm-bridge.log");
    let stub = write_stub_script(&workspace, "autopilot-stub.sh");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.current_dir(&workspace)
        .env("OPENAGENTS_AUTOPILOT_BIN", &stub)
        .env("LOG_PATH", &log_path)
        .arg("autopilot")
        .arg("run")
        .arg("local-run")
        .arg("--agent")
        .arg("fm-bridge")
        .arg("--model")
        .arg("gpt-4o-mini-2024-07-18");

    cmd.assert().success();

    let output = fs::read_to_string(&log_path).expect("read stub log");
    assert!(output.contains("--agent"), "expected agent flag");
    assert!(output.contains("fm-bridge"), "expected fm-bridge agent");
    assert!(output.contains("--model"), "expected model flag");
    assert!(output.contains("gpt-4o-mini-2024-07-18"), "expected fm-bridge model");

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
#[cfg(unix)]
fn test_autopilot_resume_delegates_to_autopilot_bin() {
    let workspace = create_temp_workspace();
    let log_path = workspace.join("autopilot-resume.log");
    let stub = write_stub_script(&workspace, "autopilot-stub.sh");
    let trajectory = workspace.join("trajectory.json");

    fs::write(&trajectory, "{}").expect("write trajectory placeholder");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.current_dir(&workspace)
        .env("OPENAGENTS_AUTOPILOT_BIN", &stub)
        .env("LOG_PATH", &log_path)
        .arg("autopilot")
        .arg("resume")
        .arg(&trajectory)
        .arg("--continue-last")
        .arg("--prompt")
        .arg("pick-up");

    cmd.assert().success();

    let output = fs::read_to_string(&log_path).expect("read stub log");
    let trajectory_arg = trajectory.display().to_string();
    assert!(output.contains("resume"), "expected resume arg");
    assert!(output.contains(&trajectory_arg), "expected trajectory arg");
    assert!(
        output.contains("--continue-last"),
        "expected continue flag"
    );
    assert!(output.contains("--prompt"), "expected prompt flag");
    assert!(output.contains("pick-up"), "expected prompt text");

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

// TODO: test_autopilot_replay_trajectory removed - Trajectory type was refactored

#[test]
fn test_daemon_subcommands_listed() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("daemon").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("status"));
}

#[test]
#[cfg(unix)]
fn test_daemon_start_delegates_to_autopilotd_bin() {
    let workspace = create_temp_workspace();
    let log_path = workspace.join("autopilotd.log");
    let stub = write_stub_script(&workspace, "autopilotd-stub.sh");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.current_dir(&workspace)
        .env("OPENAGENTS_AUTOPILOTD_BIN", &stub)
        .env("LOG_PATH", &log_path)
        .arg("daemon")
        .arg("start")
        .arg("--workdir")
        .arg(&workspace);

    cmd.assert().success();

    let output = fs::read_to_string(&log_path).expect("read stub log");
    assert!(output.contains("start"), "expected start arg");
    assert!(output.contains("--workdir"), "expected workdir flag");

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

// TODO: test_autopilot_apm_stats_shows_current_apm removed - APM types were refactored
