//! Launch contract for Grok ACP: `grok agent stdio` answers `initialize`.
//!
//! The authenticate round-trip is the next slice. This only proves the argv
//! this crate produces is ACP mode, not a trailing `acp`.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

fn grok_on_path() -> bool {
    std::process::Command::new("which")
        .arg("grok")
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false)
}

#[tokio::test]
async fn grok_agent_stdio_answers_initialize() {
    if !grok_on_path() {
        return;
    }

    let mut child = Command::new("grok")
        .args(["agent", "stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("grok agent stdio starts");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut lines = BufReader::new(stdout).lines();

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": 1,
            "clientCapabilities": {"fs": {"readTextFile": false, "writeTextFile": false}}
        }
    });
    let mut payload = serde_json::to_string(&request).unwrap();
    payload.push('\n');
    stdin.write_all(payload.as_bytes()).await.unwrap();
    stdin.flush().await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut reply = None;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, lines.next_line()).await {
            Ok(Ok(Some(line))) => {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                if value.get("id").and_then(|id| id.as_u64()) == Some(1) {
                    reply = Some(value);
                    break;
                }
            }
            _ => break,
        }
    }

    let _ = child.kill().await;
    let _ = child.wait().await;

    let reply = reply.expect("grok agent stdio answered initialize within 20s");
    assert!(reply.get("error").is_none(), "initialize refused: {reply}");
    let result = reply.get("result").expect("initialize result");
    assert_eq!(
        result.get("protocolVersion").and_then(|v| v.as_u64()),
        Some(1)
    );
    assert!(
        result.get("authMethods").is_some(),
        "initialize advertises authMethods: {result}"
    );
}
