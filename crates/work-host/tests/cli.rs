use serde_json::{Value, json};
use std::process::Command;
fn run(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_work-host"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn cli_plans_applies_and_reads_the_same_issue_history() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("work");
    let keyfile = temp.path().join("key");
    let secret = secp256k1::SecretKey::from_byte_array([19; 32]).unwrap();
    std::fs::write(&keyfile, secret.display_secret().to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&keyfile, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let root = root.to_str().unwrap();
    let keyfile = keyfile.to_str().unwrap();
    run(&["init", "--state", root, "--key-file", keyfile]);
    let scope = run(&["scope", "--state", root, "--key-file", keyfile]);
    let store = workspace_host::artifact::Artifacts::new(root.into()).unwrap();
    let descriptor = store.json(&scope).unwrap();
    let work = json!({"authority":descriptor["authority"],"scope":descriptor["scope"],"item":"ab".repeat(32)});
    let snapshot = json!({"v":"openagents.work-item.v1","requires":[],"kind":"project","title":"CLI project","body":null,"accountable":descriptor["owner"],"state":"open","frame":null,"acceptance":null,"delegation":null,"relations":[],"resources":[],"evidence":[],"disposition":null,"labels":[],"due_at":null,"timebox":null});
    let input = temp.path().join("input.json");
    std::fs::write(&input,serde_json::to_vec(&json!({"scope":scope,"work":work,"snapshot":snapshot,"expected":null,"source":null,"reason":null})).unwrap()).unwrap();
    let bundle = run(&[
        "plan",
        "--state",
        root,
        "--key-file",
        keyfile,
        "--input",
        input.to_str().unwrap(),
    ]);
    std::fs::write(&input, serde_json::to_vec(&bundle).unwrap()).unwrap();
    let saved = run(&[
        "apply",
        "--state",
        root,
        "--key-file",
        keyfile,
        "--input",
        input.to_str().unwrap(),
    ]);
    assert_eq!(saved["status"], "accepted");
    assert_eq!(
        run(&[
            "apply",
            "--state",
            root,
            "--key-file",
            keyfile,
            "--input",
            input.to_str().unwrap()
        ]),
        saved
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let event=workspace_host::host::request(&secret,descriptor["authority"].as_str().unwrap(),json!({"v":"openagents.work-history-read.v1","requires":[],"issuer":descriptor["owner"],"work":work}),now).unwrap();
    std::fs::write(&input, serde_json::to_vec(&json!({"event":event})).unwrap()).unwrap();
    let history = run(&[
        "issue",
        "history",
        "--state",
        root,
        "--key-file",
        keyfile,
        "--input",
        input.to_str().unwrap(),
    ]);
    assert_eq!(history["records"][0]["snapshot"]["title"], "CLI project");
    assert_eq!(history["records"][0]["revision"], saved["revision"]);
}
