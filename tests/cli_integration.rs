//! Integration tests for unified openagents binary CLI
//!
//! These tests verify that all CLI subcommands parse correctly and delegate
//! to the appropriate crate handlers.
//!
//! Note: Some tests check commands that may block (like GUI launches),
//! so we only test --help for those commands.

use assert_cmd::Command;
use predicates::prelude::*;

/// Test that the binary exists and shows help
#[test]
fn test_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
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
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("openagents"));
}

// Wallet commands

#[test]
fn test_wallet_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("wallet").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wallet commands"));
}

#[test]
fn test_wallet_whoami_no_wallet() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("wallet").arg("whoami");
    // Should fail gracefully if no wallet exists
    cmd.assert().failure();
}

#[test]
fn test_wallet_balance_no_wallet() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("wallet").arg("balance");
    // Should fail gracefully if no wallet exists
    cmd.assert().failure();
}

// Marketplace commands

#[test]
fn test_marketplace_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("marketplace").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Marketplace commands"));
}

#[test]
fn test_marketplace_compute_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("marketplace").arg("compute").arg("--help");
    cmd.assert().success();
}

#[test]
fn test_marketplace_skills_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("marketplace").arg("skills").arg("--help");
    cmd.assert().success();
}

#[test]
fn test_marketplace_data_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("marketplace").arg("data").arg("--help");
    cmd.assert().success();
}

// Autopilot commands

#[test]
fn test_autopilot_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("autopilot").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Autopilot commands"));
}

#[test]
fn test_autopilot_run_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("autopilot").arg("run").arg("--help");
    cmd.assert().success();
}

#[test]
fn test_autopilot_metrics_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("autopilot").arg("metrics").arg("--help");
    cmd.assert().success();
}

// GitAfter commands

#[test]
fn test_gitafter_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("gitafter").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("GitAfter commands"));
}

#[test]
fn test_gitafter_gui() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("gitafter").arg("gui");
    // GUI commands would normally block, so we just test help
    let mut help_cmd = Command::cargo_bin("openagents").unwrap();
    help_cmd.arg("gitafter").arg("gui").arg("--help");
    help_cmd.assert().success();
}

// Daemon commands

#[test]
fn test_daemon_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("daemon").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Daemon commands"));
}

#[test]
fn test_daemon_status_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("daemon").arg("status").arg("--help");
    cmd.assert().success();
}

#[test]
fn test_daemon_start_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("daemon").arg("start").arg("--help");
    cmd.assert().success();
}

// Verbose flag

#[test]
fn test_verbose_flag() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("--verbose").arg("--help");
    cmd.assert().success();
}

// Invalid commands

#[test]
fn test_invalid_command() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("nonexistent");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn test_invalid_subcommand_wallet() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("wallet").arg("nonexistent");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

// Command structure verification

#[test]
fn test_wallet_subcommands_listed() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("wallet").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("whoami"))
        .stdout(predicate::str::contains("balance"));
}

#[test]
fn test_marketplace_subcommands_listed() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("marketplace").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("compute"))
        .stdout(predicate::str::contains("skills"))
        .stdout(predicate::str::contains("data"));
}

#[test]
fn test_autopilot_subcommands_listed() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("autopilot").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("dashboard"))
        .stdout(predicate::str::contains("metrics"));
}

#[test]
fn test_daemon_subcommands_listed() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("daemon").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("status"));
}
