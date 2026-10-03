use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(home: &Path, args: &[&str], store: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
    command
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .env("OPENAGENTS_KEY_STORE", "file")
        .current_dir(home)
        .args(args);
    if let Some(store) = store {
        command.env("OPENAGENTS_SESSION_HOME", store);
    }
    command.output().unwrap()
}
fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}
fn fixture(path: &Path, grant: &str) -> Vec<u8> {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    // Listing parses public bootstrap data; grant verification happens when using a client.
    let bytes = serde_json::to_vec(&json!({
        "v": "openagents.history-observer-connection.v1", "requires": [],
        "host": "host-test", "client": "client-test", "relay": "wss://relay.example",
        "grant": grant, "sources": [], "expires_at": 2000000000,
        "authorization": {"id": "00".repeat(32), "pubkey": "00".repeat(32),
            "created_at": 1, "kind": 1, "tags": [], "content": "", "sig": "00".repeat(64)}
    }))
    .unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}
#[test]
fn fresh_home_explains_pairing_and_keeps_json_shape() {
    let home = tempfile::tempdir().unwrap();
    let out = run(home.path(), &["session", "connections"], None);
    assert!(out.status.success(), "{}", text(&out));
    let message = text(&out);
    for expected in [
        "No saved connections",
        "openagents pair",
        "other computer",
        "openagents session pair INVITATION",
    ] {
        assert!(message.contains(expected), "{message}");
    }
    let out = run(home.path(), &["--json", "session", "connections"], None);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        json!({"connections": []})
    );
}
#[test]
fn login_token_does_not_block_commands_or_get_changed() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".openagents");
    fs::create_dir(&root).unwrap();
    let token = "historical-login-token-fixture";
    fs::write(root.join("session"), token).unwrap();
    let out = run(home.path(), &["session", "connections"], None);
    assert!(out.status.success(), "{}", text(&out));
    for args in [
        vec!["session", "pair", "not-an-invitation"],
        vec!["session", "list"],
        vec!["session", "read", "chat"],
        vec!["session", "tail", "chat"],
        vec!["session", "steer", "chat", "hello"],
        vec!["session", "interrupt", "chat"],
        vec!["session", "forget", "missing"],
    ] {
        let out = run(home.path(), &args, None);
        assert_eq!(out.status.code(), Some(1), "{}", text(&out));
        assert!(!text(&out).contains("Not a directory"), "{}", text(&out));
    }
    assert_eq!(fs::read_to_string(root.join("session")).unwrap(), token);
}
#[test]
fn migrates_saved_connections_once_and_forgets_from_new_path() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".openagents");
    let bytes = fixture(
        &root.join("session/connections/grant-test.json"),
        "grant-test",
    );
    for _ in 0..2 {
        let out = run(home.path(), &["--json", "session", "connections"], None);
        assert!(out.status.success(), "{}", text(&out));
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["connections"][0]["grant"], "grant-test");
        assert_eq!(value["connections"][0]["host"], "host-test");
    }
    assert!(!root.join("session").exists());
    assert_eq!(
        fs::read(root.join("session-observer/connections/grant-test.json")).unwrap(),
        bytes
    );
    let out = run(home.path(), &["session", "forget", "grant-test"], None);
    assert!(out.status.success(), "{}", text(&out));
    assert!(
        !root
            .join("session-observer/connections/grant-test.json")
            .exists()
    );
}
#[test]
fn refuses_two_stores_without_overwriting_either() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".openagents");
    let old = root.join("session/connections/old.json");
    let new = root.join("session-observer/connections/new.json");
    let old_bytes = fixture(&old, "old");
    let new_bytes = fixture(&new, "new");
    let out = run(home.path(), &["session", "connections"], None);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out).contains("neither store was changed"),
        "{}",
        text(&out)
    );
    assert_eq!(fs::read(old).unwrap(), old_bytes);
    assert_eq!(fs::read(new).unwrap(), new_bytes);
}
#[test]
fn overrides_do_not_migrate_and_flag_wins_over_environment() {
    let home = tempfile::tempdir().unwrap();
    let legacy = home.path().join(".openagents/session/connections/old.json");
    fixture(&legacy, "old");
    let env_store = home.path().join("env-store");
    fixture(&env_store.join("connections/env.json"), "env");
    let flag_store = home.path().join("flag-store");
    fixture(&flag_store.join("connections/flag.json"), "flag");
    for (args, expected) in [
        (vec!["--json", "session", "connections"], "env"),
        (
            vec![
                "--json",
                "session",
                "connections",
                "--store",
                flag_store.to_str().unwrap(),
            ],
            "flag",
        ),
    ] {
        let out = run(home.path(), &args, Some(&env_store));
        assert!(out.status.success(), "{}", text(&out));
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["connections"][0]["grant"], expected);
        assert!(legacy.exists());
        assert!(!home.path().join(".openagents/session-observer").exists());
    }
}
#[test]
fn conflicting_files_have_actionable_errors_in_text_and_json() {
    for location in [
        ".openagents/session-observer",
        ".openagents/session-observer/connections",
        "custom",
    ] {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join(location);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "preserve-me").unwrap();
        for json in [false, true] {
            let mut args = vec!["session", "connections"];
            if location == "custom" {
                args.extend(["--store", file.to_str().unwrap()]);
            }
            if json {
                args.push("--json");
            }
            let out = run(home.path(), &args, None);
            assert_eq!(out.status.code(), Some(1));
            let message = if json {
                serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            } else {
                text(&out)
            };
            for expected in [
                "is a file",
                "requires a directory",
                "--store PATH",
                "OPENAGENTS_SESSION_HOME",
            ] {
                assert!(message.contains(expected), "{message}");
            }
            assert!(!message.contains("os error 20"));
            assert_eq!(fs::read_to_string(&file).unwrap(), "preserve-me");
        }
    }
}
