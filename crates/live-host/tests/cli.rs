use serde_json::{Value, json};
use std::process::Command;

fn run(args: &[&str]) -> (bool, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_live-host"))
        .args(args)
        .output()
        .unwrap();
    let value = if output.stdout.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&output.stdout).unwrap()
    };
    (output.status.success(), value)
}

#[test]
fn cli_initializes_and_refuses_unknown_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let key = temp.path().join("key");
    let secret = secp256k1::SecretKey::from_byte_array([41; 32]).unwrap();
    std::fs::write(&key, secret.display_secret().to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let policy = temp.path().join("policy.json");
    std::fs::write(
        &policy,
        serde_json::to_vec(&json!({"owners":[],"tasks":[]})).unwrap(),
    )
    .unwrap();
    let state = temp.path().join("state");
    let (s, k) = (state.to_str().unwrap(), key.to_str().unwrap());
    assert!(
        run(&[
            "init",
            "--state",
            s,
            "--key-file",
            k,
            "--policy",
            policy.to_str().unwrap()
        ])
        .0
    );
    assert!(
        !run(&[
            "init",
            "--state",
            s,
            "--key-file",
            k,
            "--policy",
            policy.to_str().unwrap()
        ])
        .0
    );
    assert!(
        !run(&[
            "status",
            "--state",
            s,
            "--key-file",
            k,
            "--session",
            &"ab".repeat(32)
        ])
        .0
    );
    let (ok, operation) = run(&["device-operation", "--state", s, "--key-file", k]);
    assert!(ok);
    assert!(
        operation["id"]
            .as_str()
            .unwrap()
            .ends_with(":live/device-input")
    );
    let grants = temp.path().join("grants.json");
    std::fs::write(&grants, b"[]").unwrap();
    let (ok, rules) = run(&[
        "input-policy",
        "--state",
        s,
        "--key-file",
        k,
        "--input",
        grants.to_str().unwrap(),
    ]);
    assert!(ok);
    assert_eq!(rules["rules"], json!([]));
    assert_eq!(
        run(&["--help"]).1["help"]
            .as_str()
            .map(|h| h.starts_with("usage")),
        Some(true)
    );
}
