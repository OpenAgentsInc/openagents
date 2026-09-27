//! `openagents mcp serve` end to end: the built binary, the MCP lifecycle
//! over its stdin and stdout, and a tool call that runs a real group.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

fn round_trip(messages: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(["mcp", "serve", "--timeout", "60"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("openagents starts");
    let mut stdin = child.stdin.take().unwrap();
    for message in messages {
        writeln!(stdin, "{message}").unwrap();
    }
    drop(stdin);
    let reader = BufReader::new(child.stdout.take().unwrap());
    let replies = reader
        .lines()
        .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
        .collect();
    assert!(child.wait().unwrap().success());
    replies
}

#[test]
fn lists_groups_and_runs_version() {
    let replies = round_trip(&[
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "version", "arguments": { "args": [] } } }),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "sov", "arguments": { "args": ["nonsense"] } } }),
    ]);
    assert_eq!(replies.len(), 4, "{replies:?}");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "openagents");
    let names: Vec<&str> = replies[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for expected in ["verse", "zone", "sov", "computer", "key", "version"] {
        assert!(names.contains(&expected), "{names:?}");
    }
    assert!(
        !names.contains(&"host") && !names.contains(&"mcp"),
        "{names:?}"
    );
    let version = &replies[2]["result"];
    assert_eq!(version["isError"], false, "{version}");
    assert_eq!(version["structuredContent"]["exit_code"], 0);
    assert!(
        version["structuredContent"]["document"]["version"].is_string(),
        "{version}"
    );
    let usage = &replies[3]["result"];
    assert_eq!(usage["isError"], true);
    assert_eq!(usage["structuredContent"]["exit_code"], 64, "{usage}");
}

#[test]
fn completions_name_the_groups() {
    for shell in ["bash", "zsh", "fish"] {
        let out = Command::new(env!("CARGO_BIN_EXE_openagents"))
            .args(["completions", shell])
            .output()
            .unwrap();
        assert!(out.status.success());
        let script = String::from_utf8(out.stdout).unwrap();
        assert!(
            script.contains("verse") && script.contains("sov"),
            "{script}"
        );
    }
}

#[test]
fn wallet_refuses_offline_use_with_json() {
    let home = std::env::temp_dir().join(format!("openagents-wallet-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_openagents"))
            .env("OPENAGENTS_WALLET_HOME", &home)
            .arg("--json")
            .arg("wallet")
            .args(args)
            .output()
            .unwrap()
    };
    let missing = run(&["info"]);
    assert_eq!(missing.status.code(), Some(1));
    let doc: serde_json::Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert!(doc["error"].as_str().unwrap().contains("wallet init"));

    let regtest = run(&["init", "--network", "regtest"]);
    assert_eq!(regtest.status.code(), Some(64));

    let bad_hash = run(&["invoice", "--msat", "1000", "--request-hash", "zz"]);
    assert_eq!(bad_hash.status.code(), Some(64), "{bad_hash:?}");
    let _ = std::fs::remove_dir_all(&home);
}
