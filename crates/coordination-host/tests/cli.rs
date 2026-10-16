use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::{path::Path, process::Command};
fn run(root: &Path, args: &[&str]) -> Value {
    let result = Command::new(env!("CARGO_BIN_EXE_coordination-host"))
        .args(args)
        .args([
            "--state",
            root.join("state").to_str().unwrap(),
            "--key-file",
            root.join("owner.key").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}
fn input(root: &Path, body: &Value) -> String {
    let path = root.join("input.json");
    std::fs::write(&path, serde_json::to_vec(body).unwrap()).unwrap();
    path.to_str().unwrap().into()
}
fn key(n: u8) -> String {
    nostr::domain::RelaySigner::from_secret_hex(
        &SecretKey::from_byte_array([n; 32])
            .unwrap()
            .display_secret()
            .to_string(),
    )
    .unwrap()
    .pubkey()
    .into()
}
#[test]
fn standalone_host_creates_admitted_scope_and_accepts_original_signed_control() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let keyfile = root.join("owner.key");
    std::fs::write(
        &keyfile,
        SecretKey::from_byte_array([1; 32])
            .unwrap()
            .display_secret()
            .to_string(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&keyfile, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert_eq!(run(root, &["init"])["created"], true);
    let file = input(root, &json!({"binding":"data"}));
    let target = run(root, &["operation", "--input", &file]);
    assert_eq!(target["artifact"]["schema"], "openagents.cap.v1");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let file = input(
        root,
        &json!({"policy":{"recipients":[key(1),key(2),key(3)],"checkers":[key(3)],"calls":4,"concurrency":2,"max_claim_seconds":60,"max_depth":4,"until":now+300,"effects":{"reads":["artifact"],"writes":[],"network":[],"process":false,"delegates":false,"spend":false},"parent":null},"resources":[],"targets":[target]}),
    );
    let scope = run(root, &["scope", "--input", &file]);
    let file = input(root, &scope);
    let status = run(root, &["inspect", "--input", &file]);
    assert_eq!(status["revision"], 0);
    assert_eq!(
        status["transport"]["target"]["artifact"]["schema"],
        "openagents.cap.v1"
    );
    let file = input(
        root,
        &json!({"v":"openagents.coordination-native-control.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":0,"actor":key(2),"roles":["read","claim"],"resources":[],"targets":[],"all":true,"until":now+200,"revoke":false,"scope_revoke":false}),
    );
    let signed = run(
        root,
        &[
            "sign",
            "--input",
            &file,
            "--participant-key",
            keyfile.to_str().unwrap(),
        ],
    );
    let file = input(root, &signed);
    assert_eq!(
        run(root, &["grant", "--input", &file])["status"],
        "accepted"
    );
    let file = input(root, &scope);
    assert_eq!(run(root, &["inspect", "--input", &file])["revision"], 1);
    let host = coordination_host::book::Coordinator::open(
        root.join("state"),
        SecretKey::from_byte_array([1; 32]).unwrap(),
    )
    .unwrap();
    let target = host.data_operation().unwrap();
    let mut card =
        nostr::contracts::parse_strict(&host.get_local_bytes(&target["artifact"]).unwrap())
            .unwrap();
    let id = format!("{}:session/cli-runtime", key(2));
    card["id"] = json!(id);
    card["binding_contract"]["operation"] = json!("session-control");
    let bytes = nostr::contracts::jcs(&card).unwrap();
    let artifact =
        workspace_host::artifact::reference(&bytes, "application/json", Some("openagents.cap.v1"));
    let definition = json!({"id":id,"artifact":artifact});
    let file = input(
        root,
        &json!({"v":"openagents.coordination-native-executor.v1",
        "requires":[],"definition":definition,"worker":key(2),
        "artifacts":{nostr::contracts::digest_bytes(&bytes):bytes}}),
    );
    assert_eq!(
        run(root, &["register-executor", "--input", &file]),
        definition
    );
    let file = input(
        root,
        &json!({"v":"openagents.coordination-native-executor-revoke.v1",
        "requires":[],"definition":definition}),
    );
    assert_eq!(
        run(root, &["revoke-executor", "--input", &file])["revoked"],
        true
    );
}
