use tokio::process::Command;
use std::process::Stdio;
use std::time::Duration;
use tokio::time::timeout;
use futures_util::{StreamExt, SinkExt};

#[tokio::test]
async fn forwards_claude_stderr_as_error_event() {
    // Build binaries
    assert!(Command::new("cargo").args(["build", "-p", "fake-claude"]).status().await.expect("cargo build fake-claude").success());
    assert!(Command::new("cargo").args(["build", "-p", "oa-bridge"]).status().await.expect("cargo build oa-bridge").success());

    // Resolve paths
    let workspace = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ws_root = std::path::Path::new(&workspace).ancestors().nth(2).unwrap().to_path_buf();
    let target_dir = ws_root.join("target").join("debug");
    let fake_claude = target_dir.join(if cfg!(windows) { "fake-claude.exe" } else { "fake-claude" });
    let bridge_bin = target_dir.join(if cfg!(windows) { "oa-bridge.exe" } else { "oa-bridge" });
    assert!(fake_claude.exists());
    assert!(bridge_bin.exists());

    // Allocate port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind temp port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Spawn bridge with fake Claude CLI in error mode
    let mut child = Command::new(bridge_bin)
        .arg("--bind").arg(format!("127.0.0.1:{}", port))
        .env("CLAUDE_BIN", fake_claude.to_string_lossy().to_string())
        .env("FAKE_CLAUDE_ERROR", "1")
        .env("OPENAGENTS_BRIDGE_TOKEN", "itest")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn oa-bridge");

    // Wait for server
    for _ in 0..40 { if std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_ok() { break; } tokio::time::sleep(Duration::from_millis(50)).await; }

    let url = format!("ws://127.0.0.1:{}/ws?token=itest", port);
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.expect("ws connect");
    let thread_doc_id = format!("ephemeral_claude_err_{}", port);
    let run = serde_json::json!({"control":"run.submit","threadDocId":thread_doc_id,"text":"hi","provider":"claude_code"}).to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(run.into())).await.unwrap();

    // Expect an error event to show up
    let mut saw_error = false;
    let fut = async {
        for _ in 0..200u32 {
            if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) = ws.next().await {
                if t.contains("\"type\":\"error\"") && t.to_lowercase().contains("claude") { saw_error = true; break; }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let _ = timeout(Duration::from_secs(6), fut).await;
    assert!(saw_error, "expected an error event forwarded from Claude stderr");
    let _ = child.start_kill();
}
