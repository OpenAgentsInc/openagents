use serde_json::{Value, json};
use std::process::Command;

fn run(args: &[&str]) -> (bool, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_sov-lifecycle"))
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
fn cli_initializes_and_reports_unknown_activations() {
    let temp = tempfile::tempdir().unwrap();
    let key = temp.path().join("key");
    let secret = secp256k1::SecretKey::from_byte_array([61; 32]).unwrap();
    std::fs::write(&key, secret.display_secret().to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let config = temp.path().join("config.json");
    std::fs::write(
        &config,
        serde_json::to_vec(&json!({"authorities":{},"profiles":{},"hosts":[]})).unwrap(),
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
            "--config",
            config.to_str().unwrap()
        ])
        .0
    );
    assert_eq!(
        run(&["expire", "--state", s, "--key-file", k]).1["expired"],
        json!([])
    );
    assert!(
        !run(&[
            "status",
            "--state",
            s,
            "--key-file",
            k,
            "--activation",
            &"ab".repeat(32)
        ])
        .0
    );
}
