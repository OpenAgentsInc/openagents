//! Integration tests for unified openagents binary CLI
//!
//! These tests verify that all CLI subcommands parse correctly and delegate
//! to the appropriate crate handlers.
//!
//! Note: Some tests check commands that may block (like GUI launches),
//! so we only test --help for those commands.

use assert_cmd::Command;
use autopilot::apm::{APMSnapshot, APMSource, APMWindow};
use autopilot::apm_storage;
use chrono::Utc;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn create_temp_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!("openagents-test-{}", Uuid::new_v4()));
    fs::create_dir_all(root.join(".openagents")).expect("create .openagents");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write Cargo.toml");
    root
}

fn seed_apm_snapshot(db_path: &Path) {
    let conn = rusqlite::Connection::open(db_path).expect("open apm db");
    apm_storage::init_apm_tables(&conn).expect("init apm tables");
    let snapshot = APMSnapshot {
        timestamp: Utc::now(),
        source: APMSource::Autopilot,
        window: APMWindow::Lifetime,
        apm: 12.5,
        actions: 50,
        duration_minutes: 4.0,
        messages: 20,
        tool_calls: 30,
    };
    apm_storage::save_snapshot(&conn, &snapshot).expect("save snapshot");
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

// Wallet commands

#[test]
fn test_wallet_help() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.arg("wallet").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wallet commands"));
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
        .stdout(predicate::str::contains("balance"));
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
fn test_autopilot_apm_stats_shows_current_apm() {
    let workspace = create_temp_workspace();
    let db_path = workspace.join(".openagents").join("autopilot.db");
    seed_apm_snapshot(&db_path);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.current_dir(&workspace)
        .arg("autopilot")
        .arg("apm")
        .arg("stats")
        .arg("--source")
        .arg("autopilot");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("APM Statistics"))
        .stdout(predicate::str::contains("Autopilot"))
        .stdout(predicate::str::contains("12.5"));

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}
