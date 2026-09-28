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

#[test]
fn x402_refuses_bad_arguments_with_json() {
    let home = std::env::temp_dir().join(format!("openagents-x402-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_openagents"))
            .env("OPENAGENTS_WALLET_HOME", home.join("wallet"))
            .env("OPENAGENTS_X402_HOME", home.join("x402"))
            .arg("--json")
            .arg("x402")
            .args(args)
            .output()
            .unwrap()
    };
    let no_command = run(&[
        "serve",
        "--url",
        "https://example.com/run",
        "--msat",
        "1000",
    ]);
    assert_eq!(no_command.status.code(), Some(64));

    let bad_url = run(&[
        "serve",
        "--url",
        "example.com/run",
        "--msat",
        "1000",
        "--",
        "cat",
    ]);
    assert_eq!(bad_url.status.code(), Some(64), "{bad_url:?}");

    let no_wallet = run(&[
        "serve",
        "--url",
        "https://example.com/run",
        "--msat",
        "1000",
        "--",
        "cat",
    ]);
    assert_eq!(no_wallet.status.code(), Some(1), "{no_wallet:?}");
    let doc: serde_json::Value = serde_json::from_slice(&no_wallet.stdout).unwrap();
    assert!(
        doc["error"].as_str().unwrap().contains("wallet init"),
        "{doc}"
    );

    let no_max = run(&["fetch", "https://example.com/run"]);
    assert_eq!(no_max.status.code(), Some(64));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn x402_policy_and_ledger_round_trip() {
    let home = std::env::temp_dir().join(format!("openagents-x402-policy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let run = |args: &[&str]| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_openagents"))
            .env("OPENAGENTS_WALLET_HOME", home.join("wallet"))
            .env("OPENAGENTS_X402_HOME", home.join("x402"))
            .arg("--json")
            .arg("x402")
            .args(args)
            .output()
            .unwrap();
        let doc: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
        (out.status.code(), doc)
    };
    let (code, doc) = run(&["policy"]);
    assert_eq!(code, Some(0));
    assert_eq!(doc["present"], false);

    let (code, _) = run(&["policy", "set"]);
    assert_eq!(code, Some(64));
    let (code, _) = run(&["policy", "set", "--max-msat", "x"]);
    assert_eq!(code, Some(64));

    let provider = format!("02{}", "ab".repeat(32));
    let (code, doc) = run(&[
        "policy",
        "set",
        "--max-msat",
        "5000",
        "--daily-cap-msat",
        "100000",
    ]);
    assert_eq!(code, Some(0), "{doc}");
    let (code, doc) = run(&[
        "policy",
        "set",
        "--max-msat",
        "20000",
        "--max-fee-msat",
        "40",
        "--provider",
        &provider,
    ]);
    assert_eq!(code, Some(0), "{doc}");
    let (code, doc) = run(&["policy", "allow", &provider]);
    assert_eq!(code, Some(0), "{doc}");
    assert_eq!(doc["policy"]["default"]["max_msat"], 5000);
    assert_eq!(doc["policy"]["daily_cap_msat"], 100000);
    assert_eq!(doc["policy"]["providers"][&provider]["max_fee_msat"], 40);
    assert_eq!(doc["policy"]["allow"][0], provider);

    let (code, doc) = run(&["policy", "deny", &provider]);
    assert_eq!(code, Some(0));
    assert!(doc["policy"]["allow"].is_null(), "{doc}");
    let (code, doc) = run(&["policy", "set", "--max-msat", "-", "--provider", &provider]);
    assert_eq!(code, Some(0));
    assert_eq!(doc["policy"]["providers"][&provider]["max_fee_msat"], 40);

    // With a policy present, a buyer no longer needs --max-msat to be
    // admitted past argument parsing (it then fails on the network).
    let (code, doc) = run(&["fetch", "http://127.0.0.1:9/run"]);
    assert_eq!(code, Some(1), "{doc}");

    let (code, doc) = run(&["ledger"]);
    assert_eq!(code, Some(0), "{doc}");
    assert_eq!(doc["count"], 0);
    let (code, _) = run(&["ledger", "--since", "x"]);
    assert_eq!(code, Some(64));
    let _ = std::fs::remove_dir_all(&home);
}
