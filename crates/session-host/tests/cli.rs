use serde_json::{Value, json};
use std::{path::Path, process::Command};

fn run(args: &[&str]) -> (bool, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_session-host"))
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

fn write_key(path: &Path, byte: u8) {
    let secret = secp256k1::SecretKey::from_byte_array([byte; 32]).unwrap();
    std::fs::write(path, secret.display_secret().to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

#[test]
fn cli_admits_an_adapter_and_refuses_opening_without_an_environment() {
    let temp = tempfile::tempdir().unwrap();
    let key = temp.path().join("key");
    write_key(&key, 31);
    let state = temp.path().join("state");
    let policy = temp.path().join("policy.json");
    std::fs::write(
        &policy,
        serde_json::to_vec(&json!({"owners":[],"controllers":[]})).unwrap(),
    )
    .unwrap();
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
    let (ok, adapter) = run(&[
        "admit",
        "--state",
        s,
        "--key-file",
        k,
        "--engine",
        "loop-boundary",
    ]);
    assert!(ok, "{adapter}");
    assert_eq!(adapter["schema"], "openagents.session-adapter.v1");
    let (ok, _) = run(&[
        "admit",
        "--state",
        s,
        "--key-file",
        k,
        "--engine",
        "unknown",
    ]);
    assert!(!ok);
    let (ok, _) = run(&[
        "status",
        "--state",
        s,
        "--key-file",
        k,
        "--session",
        &"ab".repeat(32),
    ]);
    assert!(!ok);
}
