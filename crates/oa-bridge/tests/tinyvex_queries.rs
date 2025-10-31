use tokio::process::Command;
use std::process::Stdio;
use std::time::Duration;
use tokio::time::timeout;
use futures_util::{sink::SinkExt, StreamExt};

#[tokio::test]
async fn supports_new_tinyvex_queries() {
    // Build bridge
    assert!(Command::new("cargo").args(["build", "-p", "oa-bridge"]).status().await.unwrap().success());
    let workspace = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ws_root = std::path::Path::new(&workspace).ancestors().nth(2).unwrap().to_path_buf();
    let target_dir = ws_root.join("target").join("debug");
    let bridge_bin = target_dir.join(if cfg!(windows) { "oa-bridge.exe" } else { "oa-bridge" });
    assert!(bridge_bin.exists());

    // Pick free port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Temporary HOME
    let td = tempfile::tempdir().unwrap();
    let home = td.path().to_path_buf();

    // Launch bridge
    let mut child = Command::new(&bridge_bin)
        .arg("--bind").arg(format!("127.0.0.1:{}", port))
        .env("OPENAGENTS_BRIDGE_TOKEN", "itest")
        .env("HOME", home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn oa-bridge");
    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();
    let _t_out = tokio::spawn(async move { use tokio::io::AsyncReadExt; let mut s = String::new(); let _ = child_stdout.read_to_string(&mut s).await; s });
    let _t_err = tokio::spawn(async move { use tokio::io::AsyncReadExt; let mut s = String::new(); let _ = child_stderr.read_to_string(&mut s).await; s });

    // Wait for port
    for _ in 0..40 { if std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_ok() { break; } tokio::time::sleep(Duration::from_millis(50)).await; }

    let url = format!("ws://127.0.0.1:{}/ws?token=itest", port);
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.expect("ws connect");

    // Query aggregated bootstrap (may be empty but should have shape)
    let q = serde_json::json!({"control":"tvx.query","name":"threadsAndTails.list","args": {"limit": 10, "perThreadTail": 5}}).to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(q.into())).await.unwrap();
    let mut saw_bootstrap = false;
    let fut = async {
        for _ in 0..40u32 {
            if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) = ws.next().await {
                if t.contains("\"type\":\"tinyvex.query_result\"") && t.contains("threadsAndTails.list") {
                    let v: serde_json::Value = serde_json::from_str(&t).unwrap_or_default();
                    assert!(v.get("threads").is_some(), "expected threads array");
                    assert!(v.get("tails").is_some(), "expected tails array");
                    saw_bootstrap = true; break;
                }
            }
        }
    };
    let _ = timeout(Duration::from_secs(2), fut).await;
    assert!(saw_bootstrap, "expected threadsAndTails.list result");

    // Query tailMany with a fake id
    let q2 = serde_json::json!({"control":"tvx.query","name":"messages.tailMany","args": {"threadIds": ["t-abc"], "perThread": 3}}).to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(q2.into())).await.unwrap();
    let mut saw_tail_many = false;
    let fut2 = async {
        for _ in 0..40u32 {
            if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) = ws.next().await {
                if t.contains("\"type\":\"tinyvex.query_result\"") && t.contains("messages.tailMany") {
                    let v: serde_json::Value = serde_json::from_str(&t).unwrap_or_default();
                    assert!(v.get("rows").is_some(), "expected rows array");
                    saw_tail_many = true; break;
                }
            }
        }
    };
    let _ = timeout(Duration::from_secs(2), fut2).await;
    assert!(saw_tail_many, "expected messages.tailMany result");

    // Query threads.listSince with zero cursor returns something (maybe empty)
    let q3 = serde_json::json!({"control":"tvx.query","name":"threads.listSince","args": {"updatedAfter": 0, "limit": 10}}).to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(q3.into())).await.unwrap();
    let mut saw_since = false;
    let fut3 = async {
        for _ in 0..40u32 {
            if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) = ws.next().await {
                if t.contains("\"type\":\"tinyvex.query_result\"") && t.contains("threads.listSince") {
                    saw_since = true; break;
                }
            }
        }
    };
    let _ = timeout(Duration::from_secs(2), fut3).await;
    assert!(saw_since, "expected threads.listSince result");

    // Query messages.since on fake id (may be empty)
    let q4 = serde_json::json!({"control":"tvx.query","name":"messages.since","args": {"threadId": "t-abc", "afterSeq": 0, "limit": 5}}).to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(q4.into())).await.unwrap();
    let mut saw_msgs_since = false;
    let fut4 = async {
        for _ in 0..40u32 {
            if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) = ws.next().await {
                if t.contains("\"type\":\"tinyvex.query_result\"") && t.contains("messages.since") {
                    saw_msgs_since = true; break;
                }
            }
        }
    };
    let _ = timeout(Duration::from_secs(2), fut4).await;
    assert!(saw_msgs_since, "expected messages.since result");

    let _ = child.start_kill();
}

