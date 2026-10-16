use serde_json::json;
use std::process::Command;

#[test]
fn cli_initializes_a_treasury_once() {
    let temp = tempfile::tempdir().unwrap();
    let key = temp.path().join("key");
    let secret = secp256k1::SecretKey::from_byte_array([71; 32]).unwrap();
    std::fs::write(&key, secret.display_secret().to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let config = temp.path().join("config.json");
    std::fs::write(
        &config,
        serde_json::to_vec(&json!({"authorities":{}})).unwrap(),
    )
    .unwrap();
    let state = temp.path().join("state");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_sov-treasury"))
            .args([
                "init",
                "--state",
                state.to_str().unwrap(),
                "--key-file",
                key.to_str().unwrap(),
                "--config",
                config.to_str().unwrap(),
            ])
            .output()
            .unwrap()
            .status
            .success()
    };
    assert!(run());
    assert!(!run());
}
