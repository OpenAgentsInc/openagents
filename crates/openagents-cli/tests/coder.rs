//! Coder's command surface with isolated settings and no model calls.

use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn run(home: &std::path::Path, arguments: &[&str], input: Option<&Value>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
    command
        .env_clear()
        .env("HOME", home)
        .env("USERPROFILE", home)
        .current_dir(home)
        .arg("--json")
        .arg("coder")
        .args(arguments)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(root) = std::env::var_os("SYSTEMROOT") {
        command.env("SYSTEMROOT", root);
    }
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

fn value(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn coder_routes_plugin_model_and_atif_commands_without_exposing_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let configured = value(run(
        home,
        &["plugins", "configure", "openrouter-byok", "--stdin"],
        Some(&json!({"api_key":"fake-test-key","enabled":true})),
    ));
    assert!(!configured.to_string().contains("fake-test-key"));
    let model = value(run(
        home,
        &["models", "set", "openai/gpt-6-luna:low:max-tokens=4096"],
        None,
    ));
    assert_eq!(model["model"], "openai/gpt-6-luna:low:max-tokens=4096");
    value(run(
        home,
        &["plugins", "configure", "openrouter-byok", "--stdin"],
        Some(&json!({"api_key":null})),
    ));
    let status = value(run(home, &["status"], None));
    assert_eq!(status["openrouter_connected"], false);
    value(run(
        home,
        &["chat", "-p", "hello", "--session", "cli-demo", "--demo"],
        None,
    ));
    let exported = value(run(home, &["export", "cli-demo"], None));
    assert_eq!(exported["schema_version"], "ATIF-v1.8");
    assert_eq!(exported["steps"][0]["message"], "hello");
    assert_eq!(
        value(run(home, &["sessions", "list"], None))["sessions"][0]["id"],
        "cli-demo"
    );
    let refused = run(home, &["sessions", "read", "../escape"], None);
    assert_eq!(refused.status.code(), Some(64));
}
