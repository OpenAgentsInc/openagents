use serde_json::{Value, json};
use std::process::Command;

fn run(args: &[&str]) -> (bool, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_sov-custody"))
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
fn cli_initializes_revokes_and_advances_generations() {
    let temp = tempfile::tempdir().unwrap();
    let key = temp.path().join("key");
    let secret = secp256k1::SecretKey::from_byte_array([51; 32]).unwrap();
    std::fs::write(&key, secret.display_secret().to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let agent = "aa".repeat(32);
    let config = temp.path().join("config.json");
    std::fs::write(
        &config,
        serde_json::to_vec(&json!({"authorities":{agent.clone():"bb".repeat(32)},"profiles":{}}))
            .unwrap(),
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
    assert!(
        run(&[
            "revoke",
            "--state",
            s,
            "--key-file",
            k,
            "--caller",
            &"cc".repeat(32)
        ])
        .0
    );
    assert_eq!(
        run(&[
            "generation",
            "--state",
            s,
            "--key-file",
            k,
            "--agent",
            &agent
        ])
        .1["generation"],
        2
    );
}
