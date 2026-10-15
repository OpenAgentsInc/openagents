use serde_json::Value;
use std::process::Command;
const LOG: &[u8] = include_bytes!("../../nostr/tests/fixtures/atif/parent.atif.jsonl");
fn command() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_atif-nostr"));
    c.env_remove("OPENAGENTS_NOSTR_SECRET");
    c
}
fn key(path: &std::path::Path) {
    std::fs::write(path, "01".repeat(32)).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}
#[test]
fn help_needs_no_identity_and_pack_never_defaults_to_public() {
    assert!(command().arg("--help").output().unwrap().status.success());
    let home = tempfile::tempdir().unwrap();
    let log = home.path().join("input.jsonl");
    let keys = home.path().join("key");
    let bundle = home.path().join("bundle.json");
    key(&keys);
    std::fs::write(&log, LOG).unwrap();
    let output = command()
        .args([
            "pack",
            log.to_str().unwrap(),
            "--key-file",
            keys.to_str().unwrap(),
            "--bundle",
            bundle.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!bundle.exists());
    let secret = secp256k1::SecretKey::from_byte_array([2; 32]).unwrap();
    let pubkey =
        nostr::domain::RelaySigner::from_secret_hex(&secret.display_secret().to_string()).unwrap();
    let output = command()
        .args([
            "pack",
            log.to_str().unwrap(),
            "--key-file",
            keys.to_str().unwrap(),
            "--bundle",
            bundle.to_str().unwrap(),
            "--to",
            pubkey.pubkey(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["public"], false);
    assert_eq!(summary["published"], false);
    let events: Value = serde_json::from_slice(&std::fs::read(&bundle).unwrap()).unwrap();
    let event = serde_json::from_value(events["manifest"].clone()).unwrap();
    let chunks = serde_json::from_value::<Vec<_>>(events["chunks"].clone()).unwrap();
    assert_eq!(
        atif_nostr::carriage::receive(&event, &chunks, Some(&secret))
            .unwrap()
            .bytes,
        LOG
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&bundle).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // An existing destination is never replaced, even with explicit public consent.
    let before = std::fs::read(&bundle).unwrap();
    let output = command()
        .args([
            "pack",
            log.to_str().unwrap(),
            "--key-file",
            keys.to_str().unwrap(),
            "--bundle",
            bundle.to_str().unwrap(),
            "--publish-public",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read(&bundle).unwrap(), before);
}
#[test]
fn public_pack_requires_the_explicit_flag_and_keeps_exact_bytes() {
    let home = tempfile::tempdir().unwrap();
    let log = home.path().join("input.jsonl");
    let keys = home.path().join("key");
    let bundle = home.path().join("public.json");
    key(&keys);
    std::fs::write(&log, LOG).unwrap();
    let output = command()
        .args([
            "pack",
            log.to_str().unwrap(),
            "--key-file",
            keys.to_str().unwrap(),
            "--bundle",
            bundle.to_str().unwrap(),
            "--publish-public",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events: Value = serde_json::from_slice(&std::fs::read(&bundle).unwrap()).unwrap();
    let event = serde_json::from_value(events["manifest"].clone()).unwrap();
    let chunks = serde_json::from_value::<Vec<_>>(events["chunks"].clone()).unwrap();
    assert_eq!(
        atif_nostr::carriage::receive(&event, &chunks, None)
            .unwrap()
            .bytes,
        LOG
    );
}
