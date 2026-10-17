//! Every test uses an explicit disposable directory and synthetic accounts.
use coder_access::RelayPolicy;
use coder_sharing::{Client, Recipient};

#[test]
fn device_keys_and_saved_computers_are_bound_to_the_authenticated_account() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    let first = Client::open(&a, "account-a", "Alice", RelayPolicy::LoopbackTest).unwrap();
    let second = Client::open(&b, "account-b", "Bob", RelayPolicy::LoopbackTest).unwrap();
    assert_ne!(first.recipient().device, second.recipient().device);
    let again = Client::open(&a, "account-a", "Alice", RelayPolicy::LoopbackTest).unwrap();
    assert_eq!(first.recipient().device, again.recipient().device);
    assert!(Client::open(&a, "account-b", "Bob", RelayPolicy::LoopbackTest).is_err());
    let parsed = Recipient::parse(&first.recipient().encode()).unwrap();
    assert_eq!(parsed.account, "account-a");
    assert_eq!(parsed.device, first.recipient().device);
    assert!(first.computers().unwrap().is_empty());
    assert!(second.computers().unwrap().is_empty());
}

#[test]
fn malformed_recipient_codes_cannot_change_the_account_or_device_binding() {
    for code in [
        "",
        "coder-host:old-host-code",
        "openagents-share-to:not-base64",
        &"x".repeat(3000),
    ] {
        assert!(Recipient::parse(code).is_err());
    }
    let mut recipient = Recipient {
        account: "alice".into(),
        label: "Alice".into(),
        device: "ab".repeat(32),
    };
    assert!(Recipient::parse(&recipient.encode()).is_ok());
    recipient.device = "arbitrary-endpoint".into();
    assert!(Recipient::parse(&recipient.encode()).is_err());
    recipient.device = "ab".repeat(32);
    recipient.account = "alice\nother".into();
    assert!(Recipient::parse(&recipient.encode()).is_err());
}

#[cfg(unix)]
#[test]
fn links_hardlinks_and_widened_files_never_open_a_private_profile() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("profile");
    let client = Client::open(&path, "alice", "Alice", RelayPolicy::LoopbackTest).unwrap();
    let before = client.recipient().device;
    drop(client);
    let key = path.join("device.key");
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Client::open(&path, "alice", "Alice", RelayPolicy::LoopbackTest).is_err());
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let linked = temp.path().join("linked-key");
    std::fs::hard_link(&key, &linked).unwrap();
    assert!(Client::open(&path, "alice", "Alice", RelayPolicy::LoopbackTest).is_err());
    std::fs::remove_file(linked).unwrap();
    let record = path.join("computers.json");
    let saved = temp.path().join("saved.json");
    std::fs::rename(&record, &saved).unwrap();
    symlink(&saved, &record).unwrap();
    assert!(Client::open(&path, "alice", "Alice", RelayPolicy::LoopbackTest).is_err());
    std::fs::remove_file(&record).unwrap();
    std::fs::rename(saved, record).unwrap();
    assert_eq!(
        Client::open(&path, "alice", "Alice", RelayPolicy::LoopbackTest)
            .unwrap()
            .recipient()
            .device,
        before
    );
}

#[test]
fn deleting_the_list_cannot_reassign_a_device_key_and_missing_keys_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("private");
    let first = Client::open(&path, "alice", "Alice", RelayPolicy::LoopbackTest).unwrap();
    let device = first.recipient().device;
    drop(first);
    std::fs::remove_file(path.join("computers.json")).unwrap();
    assert!(Client::open(&path, "bob", "Bob", RelayPolicy::LoopbackTest).is_err());
    let first = Client::open(&path, "alice", "Alice", RelayPolicy::LoopbackTest).unwrap();
    assert_eq!(first.recipient().device, device);
    drop(first);
    std::fs::remove_file(path.join("device.key")).unwrap();
    assert!(Client::open(&path, "alice", "Alice", RelayPolicy::LoopbackTest).is_err());
    assert!(!path.join("device.key").exists());
}
