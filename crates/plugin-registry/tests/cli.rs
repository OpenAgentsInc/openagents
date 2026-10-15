//! All CLI state and keys are disposable; no ambient account is consulted.
use serde_json::{Value, json};
use std::process::Command;
#[test]
fn publish_enroll_refresh_browse_retry_and_remove() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let state = root.join("state");
    let mirror = root.join("mirror");
    let key = root.join("curator.key");
    let secret = "01".repeat(32);
    std::fs::write(&key, &secret).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let signer = nostr::domain::RelaySigner::from_secret_hex(&secret).unwrap();
    let id = format!("{}:catalog", signer.pubkey());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let catalog = root.join("catalog.json");
    std::fs::write(&catalog,json!({"v":"openagents.registry.v1","requires":[],"registry":id,"revision":1,"as_of":now,"valid_until":now+3600,"title":"Catalog\u{001b}[2J","description":"","entries":[]}).to_string()).unwrap();
    let run = |words: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_plugin-registry"))
            .args(words)
            .env_remove("OPENAGENTS_NOSTR_SECRET")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.contains(&27));
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let publish = [
        "publish",
        catalog.to_str().unwrap(),
        "--mirror",
        mirror.to_str().unwrap(),
        "--license",
        "MIT",
        "--key-file",
        key.to_str().unwrap(),
    ];
    let first = run(&publish);
    let second = run(&publish);
    assert_eq!(first["listing"], second["listing"]);
    let sources = root.join("sources.json");
    std::fs::write(
        &sources,
        json!([{"type":"directory","path":mirror}]).to_string(),
    )
    .unwrap();
    let path = state.to_str().unwrap();
    run(&[
        "add",
        &id,
        "--sources",
        sources.to_str().unwrap(),
        "--state",
        path,
    ]);
    run(&["refresh", &id, "--state", path]);
    assert_eq!(run(&["browse", &id, "--state", path])["fresh"], true);
    assert_eq!(run(&["list", "--state", path]).as_array().unwrap().len(), 1);
    run(&["remove", &id, "--state", path]);
    assert_eq!(run(&["list", "--state", path])[0]["enabled"], false);
    let bad = Command::new(env!("CARGO_BIN_EXE_plugin-registry"))
        .args(["browse", &id, "--release", "garbage", "--state", path])
        .output()
        .unwrap();
    assert!(!bad.status.success());
}
