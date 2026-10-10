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
    for expected in ["verse", "zone", "sov", "computer", "version"] {
        assert!(names.contains(&expected), "{names:?}");
    }
    for never in [
        "host", "mcp", "wallet", "pay", "x402", "key", "ssh", "service",
    ] {
        assert!(!names.contains(&never), "{never} is served: {names:?}");
    }
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

/// Audit CLI-01: no MCP caller spends, signs in a shell, installs a
/// service, or passes a consent switch. Each call is refused before any
/// command runs (`unknown tool` for a group never served, an error result
/// for a command or switch that is not read-only).
#[test]
fn mcp_serve_refuses_money_shells_services_and_consent_switches() {
    let call = |id: u64, name: &str, args: &[&str]| json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": name, "arguments": { "args": args } } });
    let mut messages = vec![
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ];
    let never: [(&str, &[&str]); 7] = [
        ("wallet", &["send", "lnbc1fake", "--yes"]),
        ("wallet", &["balance"]),
        ("pay", &["--help"]),
        ("ssh", &["somehost"]),
        ("service", &["install"]),
        (
            "x402",
            &["call", "version", "--", "openagents", "mcp", "serve"],
        ),
        ("key", &["--help"]),
    ];
    for (index, (name, args)) in never.iter().enumerate() {
        messages.push(call(10 + index as u64, name, args));
    }
    // Served groups still refuse a spend, a consent switch, and a command
    // word smuggled after a read-only one.
    messages.push(call(30, "computer", &["exec", "laptop", "rm", "-rf", "x"]));
    messages.push(call(31, "version", &["--yes"]));
    messages.push(call(32, "version", &["--show-words"]));
    messages.push(call(33, "version", &["--replace=1"]));
    messages.push(call(34, "computer", &["link"]));
    messages.push(call(35, "computer", &["list", "--", "exec"]));
    let replies = round_trip(&messages);
    assert_eq!(replies.len(), 1 + never.len() + 6, "{replies:?}");
    for reply in &replies[1..=never.len()] {
        assert!(
            reply["error"]["message"]
                .as_str()
                .is_some_and(|m| m.starts_with("unknown tool")),
            "{reply}"
        );
    }
    for reply in &replies[1 + never.len()..] {
        let result = &reply["result"];
        assert_eq!(result["isError"], true, "{reply}");
        assert_eq!(result["structuredContent"]["exit_code"], 1, "{reply}");
        assert!(
            result["content"][0]["text"]
                .as_str()
                .is_some_and(|t| t.starts_with("refused")),
            "{reply}"
        );
    }
}

/// Audit CLI-01: `x402 mcp-serve` sells only named groups, and never one
/// that moves money, holds keys, or opens shells. Refused at argument
/// parsing, before any wallet opens.
#[test]
fn x402_mcp_serve_requires_tools_and_refuses_spend_class_groups() {
    let home = std::env::temp_dir().join(format!("openagents-x402-mcp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let run = |tools: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
        command
            .env("OPENAGENTS_WALLET_HOME", home.join("wallet"))
            .env("OPENAGENTS_X402_HOME", home.join("x402"))
            .args([
                "--json",
                "x402",
                "mcp-serve",
                "--server",
                "https://example.com/mcp",
                "--msat",
                "1000",
            ]);
        for tool in tools {
            command.args(["--tool", tool]);
        }
        command.stdin(Stdio::null()).output().unwrap()
    };
    let none = run(&[]);
    assert_eq!(none.status.code(), Some(64), "{none:?}");
    assert!(String::from_utf8_lossy(&none.stdout).contains("--tool"));
    for group in ["wallet", "pay", "x402", "key", "ssh", "service", "host"] {
        let out = run(&["version", group]);
        assert_eq!(out.status.code(), Some(64), "{group}: {out:?}");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains(group), "{group}: {text}");
    }
    let _ = std::fs::remove_dir_all(&home);
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
fn x402_node_refuses_offline_use_with_json() {
    let home = std::env::temp_dir().join(format!("openagents-wallet-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_openagents"))
            .env("OPENAGENTS_WALLET_HOME", &home)
            .arg("--json")
            .arg("x402")
            .arg("node")
            .args(args)
            .output()
            .unwrap()
    };
    let missing = run(&["info"]);
    assert_eq!(missing.status.code(), Some(1));
    let doc: serde_json::Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert!(doc["error"].as_str().unwrap().contains("x402 node init"));

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
        doc["error"].as_str().unwrap().contains("x402 node init"),
        "{doc}"
    );

    let no_max = run(&["fetch", "https://example.com/run"]);
    assert_eq!(no_max.status.code(), Some(64));

    let provider = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_openagents"))
            .env("OPENAGENTS_WALLET_HOME", home.join("wallet"))
            .env("OPENAGENTS_X402_HOME", home.join("x402"))
            .env("VERSE_HOME", home.join("verse"))
            .arg("--json")
            .arg("x402")
            .arg("status")
            .args(args)
            .output()
            .unwrap()
    };
    let empty = provider(&["--list"]);
    assert_eq!(empty.status.code(), Some(0), "{empty:?}");
    let doc: serde_json::Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert_eq!(doc["count"], 0, "{doc}");

    let bad_target = provider(&["--finish", "nocolon", "--cause", "operator_cancelled"]);
    assert_eq!(bad_target.status.code(), Some(64), "{bad_target:?}");
    let bad_cause = provider(&["--finish", "aa:bb", "--cause", "because"]);
    assert_eq!(bad_cause.status.code(), Some(64), "{bad_cause:?}");
    let unknown = provider(&["--finish", "aa:bb", "--cause", "operator_cancelled"]);
    assert_eq!(unknown.status.code(), Some(1), "{unknown:?}");
    let doc: serde_json::Value = serde_json::from_slice(&unknown.stdout).unwrap();
    assert!(
        doc["error"].as_str().unwrap().contains("unknown_purchase"),
        "{doc}"
    );
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
