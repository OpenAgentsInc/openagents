//! Integration tests for wallet CLI flows via the unified openagents binary.

use assert_cmd::Command;
use bip39::Mnemonic;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use wallet::core::identity::UnifiedIdentity;

fn create_temp_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!("openagents-wallet-test-{}", Uuid::new_v4()));
    fs::create_dir_all(root.join(".openagents")).expect("create .openagents");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write Cargo.toml");
    root
}

fn keychain_path(workspace: &Path) -> PathBuf {
    workspace.join(".openagents").join("keychain.txt")
}

fn write_wallet_config(workspace: &Path, relays: &[&str]) {
    let config_dir = workspace.join(".openagents");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let relays_list = relays
        .iter()
        .map(|relay| format!("\"{}\"", relay))
        .collect::<Vec<_>>()
        .join(", ");

    let contents = format!(
        "[network]\nbitcoin = \"mainnet\"\n\n[nostr]\nrelays = [{}]\n\n[storage]\ndb_path = \"~/.openagents/wallet.db\"\nbackup_enabled = false\n",
        relays_list
    );

    fs::write(config_dir.join("wallet.toml"), contents).expect("write wallet config");
}

fn openagents_cmd(workspace: &Path, keychain: &Path) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("openagents"));
    cmd.current_dir(workspace)
        .env("HOME", workspace)
        .env("OPENAGENTS_KEYCHAIN_FILE", keychain);
    cmd
}

fn init_wallet(workspace: &Path, keychain: &Path) {
    let mut cmd = openagents_cmd(workspace, keychain);
    cmd.arg("wallet").arg("init");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wallet initialized"));
}

#[test]
fn test_wallet_import_from_mnemonic() {
    let workspace = create_temp_workspace();
    let keychain = keychain_path(&workspace);
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    let mut cmd = openagents_cmd(&workspace, &keychain);
    cmd.arg("wallet")
        .arg("import")
        .arg("--mnemonic")
        .arg(mnemonic);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wallet imported"));

    let stored = fs::read_to_string(&keychain).expect("read keychain file");
    assert_eq!(stored.trim(), mnemonic);

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
fn test_wallet_whoami_shows_identity() {
    let workspace = create_temp_workspace();
    let keychain = keychain_path(&workspace);

    init_wallet(&workspace, &keychain);

    let mnemonic = fs::read_to_string(&keychain).expect("read keychain file");
    let parsed = Mnemonic::parse(mnemonic.trim()).expect("parse mnemonic");
    let identity = UnifiedIdentity::from_mnemonic(parsed).expect("derive identity");
    let npub = identity.npub().expect("encode npub");

    let mut cmd = openagents_cmd(&workspace, &keychain);
    cmd.arg("wallet").arg("whoami");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wallet Information"))
        .stdout(predicate::str::contains("Nostr npub"))
        .stdout(predicate::str::contains(&npub))
        .stdout(predicate::str::contains("Spark Address"));

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
fn test_wallet_password_set_requires_unlock() {
    let workspace = create_temp_workspace();
    let keychain = keychain_path(&workspace);

    init_wallet(&workspace, &keychain);

    let original = fs::read_to_string(&keychain).expect("read keychain file");

    let mut cmd = openagents_cmd(&workspace, &keychain);
    cmd.arg("wallet")
        .arg("password")
        .arg("set")
        .arg("--password")
        .arg("hunter2");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wallet password set"));

    let encrypted = fs::read_to_string(&keychain).expect("read keychain file");
    assert_ne!(encrypted.trim(), original.trim());
    assert!(encrypted.contains("\"ciphertext\""));

    let mut locked_cmd = openagents_cmd(&workspace, &keychain);
    locked_cmd.arg("wallet").arg("whoami");
    locked_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("password protected"));

    let mut unlocked_cmd = openagents_cmd(&workspace, &keychain);
    unlocked_cmd
        .env("OPENAGENTS_WALLET_PASSWORD", "hunter2")
        .arg("wallet")
        .arg("whoami");
    unlocked_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Wallet Information"));

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
fn test_wallet_profile_set_and_show() {
    let workspace = create_temp_workspace();
    let keychain = keychain_path(&workspace);

    init_wallet(&workspace, &keychain);
    write_wallet_config(&workspace, &[]);

    let mut cmd = openagents_cmd(&workspace, &keychain);
    cmd.arg("wallet")
        .arg("profile")
        .arg("set")
        .arg("--name")
        .arg("Alice")
        .arg("--about")
        .arg("Test user");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Profile updated locally"));

    let mut show_cmd = openagents_cmd(&workspace, &keychain);
    show_cmd.arg("wallet").arg("profile").arg("show");
    show_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("Test user"));

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
fn test_wallet_contacts_add_list_remove() {
    let workspace = create_temp_workspace();
    let keychain = keychain_path(&workspace);

    init_wallet(&workspace, &keychain);
    write_wallet_config(&workspace, &[]);

    let contact_one = "a".repeat(64);
    let contact_two = "b".repeat(64);

    let mut add_cmd = openagents_cmd(&workspace, &keychain);
    add_cmd
        .arg("wallet")
        .arg("contacts")
        .arg("add")
        .arg(&contact_one)
        .arg("--name")
        .arg("Alice");
    add_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Followed"));

    let mut add_cmd_two = openagents_cmd(&workspace, &keychain);
    add_cmd_two
        .arg("wallet")
        .arg("contacts")
        .arg("add")
        .arg(&contact_two);
    add_cmd_two
        .assert()
        .success()
        .stdout(predicate::str::contains("Followed"));

    let mut list_cmd = openagents_cmd(&workspace, &keychain);
    list_cmd.arg("wallet").arg("contacts").arg("list");
    list_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Following: 2"))
        .stdout(predicate::str::contains(&contact_one))
        .stdout(predicate::str::contains(&contact_two));

    let mut remove_cmd = openagents_cmd(&workspace, &keychain);
    remove_cmd
        .arg("wallet")
        .arg("contacts")
        .arg("remove")
        .arg(&contact_one);
    remove_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Unfollowed"));

    let mut list_after = openagents_cmd(&workspace, &keychain);
    list_after.arg("wallet").arg("contacts").arg("list");
    list_after
        .assert()
        .success()
        .stdout(predicate::str::contains("Following: 1"))
        .stdout(predicate::str::contains(&contact_two));

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
fn test_wallet_post_offline() {
    let workspace = create_temp_workspace();
    let keychain = keychain_path(&workspace);

    init_wallet(&workspace, &keychain);
    write_wallet_config(&workspace, &[]);

    let mut cmd = openagents_cmd(&workspace, &keychain);
    cmd.arg("wallet").arg("post").arg("Hello Nostr");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Publishing note"))
        .stdout(predicate::str::contains("No relays configured"))
        .stdout(predicate::str::contains("Event ID"));

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
fn test_wallet_dm_send_offline() {
    let workspace = create_temp_workspace();
    let keychain = keychain_path(&workspace);

    init_wallet(&workspace, &keychain);
    write_wallet_config(&workspace, &[]);

    let recipient_mnemonic =
        "legal winner thank year wave sausage worth useful legal winner thank yellow";
    let parsed = Mnemonic::parse(recipient_mnemonic).expect("parse recipient mnemonic");
    let recipient = UnifiedIdentity::from_mnemonic(parsed)
        .expect("derive recipient identity")
        .npub()
        .expect("encode npub");

    let mut cmd = openagents_cmd(&workspace, &keychain);
    cmd.arg("wallet")
        .arg("dm")
        .arg("send")
        .arg(recipient)
        .arg("Hello there");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Sending encrypted DM"))
        .stdout(predicate::str::contains("No relays configured"))
        .stdout(predicate::str::contains("Event ID"));

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}

#[test]
fn test_wallet_export_shows_mnemonic() {
    let workspace = create_temp_workspace();
    let keychain = keychain_path(&workspace);

    init_wallet(&workspace, &keychain);

    let mnemonic = fs::read_to_string(&keychain).expect("read keychain file");

    let mut cmd = openagents_cmd(&workspace, &keychain);
    cmd.arg("wallet").arg("export");
    cmd.write_stdin("yes\n");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Your recovery phrase"))
        .stdout(predicate::str::contains(mnemonic.trim()));

    fs::remove_dir_all(&workspace).expect("cleanup workspace");
}
