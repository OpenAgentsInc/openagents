#![cfg(feature = "host")]
use secp256k1::SecretKey;
use std::process::Command;

#[test]
fn local_pair_and_revoke_cli_retain_only_public_connection_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("synthetic-history");
    let state = temp.path().join("host");
    std::fs::create_dir(&root).unwrap();
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let public = coder_connect::protocol::pubkey(&secret);
    let output = Command::new(env!("CARGO_BIN_EXE_coder-connect"))
        .args([
            "pair",
            "--client",
            &public,
            "--relay",
            "wss://relay.example/",
        ])
        .arg("--codex-root")
        .arg(&root)
        .arg("--state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let code = coder_connect::ConnectionCode::parse(&output.stdout).unwrap();
    code.verify(
        &secret,
        coder_connect::unix_time().unwrap(),
        coder_connect::RelayPolicy::Production,
    )
    .unwrap();
    assert!(!String::from_utf8_lossy(&output.stdout).contains(root.to_str().unwrap()));
    let output = Command::new(env!("CARGO_BIN_EXE_coder-connect"))
        .args(["revoke", "--grant", &code.grant])
        .arg("--state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"revoked\n");
    assert!(
        coder_connect::host::Host::new(state, coder_connect::RelayPolicy::Production)
            .relays(coder_connect::unix_time().unwrap())
            .unwrap()
            .is_empty()
    );
}
