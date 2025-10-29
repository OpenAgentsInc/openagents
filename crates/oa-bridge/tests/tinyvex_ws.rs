use tokio::process::Command;
use std::process::Stdio;
use std::time::Duration;
use tokio::time::timeout;
use futures_util::{sink::SinkExt, StreamExt};

#[tokio::test]
async fn tinyvex_query_and_subscribe() {
    // Build binaries
    assert!(Command::new("cargo").args(["build", "-p", "oa-bridge"]).status().await.unwrap().success());
    let workspace = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ws_root = std::path::Path::new(&workspace).ancestors().nth(2).unwrap().to_path_buf();
    let target_dir = ws_root.join("target").join("debug");
    let bridge_bin = target_dir.join(if cfg!(windows) { "oa-bridge.exe" } else { "oa-bridge" });
    assert!(bridge_bin.exists());

    // Allocate port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut child = Command::new(&bridge_bin)
        .arg("--bind").arg(format!("127.0.0.1:{}", port))
        .env("OPENAGENTS_BRIDGE_TOKEN", "itest")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn oa-bridge");
    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();
    let t_out = tokio::spawn(async move { use tokio::io::AsyncReadExt; let mut s = String::new(); let _ = child_stdout.read_to_string(&mut s).await; s });
    let t_err = tokio::spawn(async move { use tokio::io::AsyncReadExt; let mut s = String::new(); let _ = child_stderr.read_to_string(&mut s).await; s });

    for _ in 0..40 { if std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_ok() { break; } tokio::time::sleep(Duration::from_millis(50)).await; }

    let url = format!("ws://127.0.0.1:{}/ws?token=itest", port);
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.expect("ws connect");

    // Query threads.list (should be empty)
    let q = serde_json::json!({"control":"tvx.query","name":"threads.list","args": {"limit": 10}}).to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(q.into())).await.unwrap();

    let mut rows_empty = false;
    let fut = async {
        for _ in 0..40u32 {
            if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) = ws.next().await {
                if t.contains("\"type\":\"tinyvex.query_result\"") && t.contains("threads.list") {
                    let v: serde_json::Value = serde_json::from_str(&t).unwrap_or_default();
                    let n = v.get("rows").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
                    if n == 0 { rows_empty = true; break; }
                }
            }
        }
    };
    let _ = timeout(Duration::from_secs(2), fut).await;
    assert!(rows_empty, "expected empty threads list initially");

    // Subscribe to messages for a fake thread, then simulate a write by sending a tinyvex update indirectly via run.submit with fake codex is heavy; skip here.
    // Instead, just verify subscribe snapshot path returns quickly (even empty)
    let sub = serde_json::json!({"control":"tvx.subscribe","stream":"messages","threadId":"t_sub"}).to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(sub.into())).await.unwrap();
    let mut saw_snapshot = false;
    let fut2 = async {
        for _ in 0..40u32 {
            if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) = ws.next().await {
                if t.contains("\"type\":\"tinyvex.snapshot\"") && t.contains("\"stream\":\"messages\"") {
                    saw_snapshot = true; break;
                }
            }
        }
    };
    let _ = timeout(Duration::from_secs(2), fut2).await;
    assert!(saw_snapshot, "expected messages snapshot on subscribe");

    let _ = child.start_kill();
    let _ = t_out.await.unwrap_or_default();
    let _ = t_err.await.unwrap_or_default();
}

