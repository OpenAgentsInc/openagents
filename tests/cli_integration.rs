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

fn create_temp_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!("openagents-test-{}", Uuid::new_v4()));
    fs::create_dir_all(root.join(".openagents")).expect("create .openagents");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write Cargo.toml");
    root
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
        .stdout(predicate::str::contains("agent"))
        .stdout(predicate::str::contains("gitafter"))
        .stdout(predicate::str::contains("pylon"))
        .stdout(predicate::str::contains("auth"));
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
        .stdout(predicate::str::contains("OpenAgents CLI"));
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
