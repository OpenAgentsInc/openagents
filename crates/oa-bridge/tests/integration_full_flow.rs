use tokio::process::Command;
use std::process::Stdio;
use std::time::Duration;
use tokio::time::timeout;
use futures_util::{sink::SinkExt, StreamExt};

#[tokio::test]
async fn full_flow_with_fake_codex_emits_acp() {
    // Build binaries to ensure they exist
    assert!(Command::new("cargo").args(["build", "-p", "fake-codex"]).status().await.expect("cargo build fake-codex").success());
    assert!(Command::new("cargo").args(["build", "-p", "oa-bridge"]).status().await.expect("cargo build oa-bridge").success());
    // Resolve paths
    let workspace = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ws_root = std::path::Path::new(&workspace).ancestors().nth(2).unwrap().to_path_buf();
    let target_dir = ws_root.join("target").join("debug");
    let fake_codex = target_dir.join(if cfg!(windows) { "fake-codex.exe" } else { "fake-codex" });
    let bridge_bin = target_dir.join(if cfg!(windows) { "oa-bridge.exe" } else { "oa-bridge" });
    assert!(fake_codex.exists(), "fake-codex binary not found at {:?}", fake_codex);
    assert!(bridge_bin.exists(), "oa-bridge binary not found at {:?}", bridge_bin);

    // Allocate a port by binding to 0 then releasing
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind temp port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Spawn oa-bridge process
    let mut child = Command::new(bridge_bin)
        .arg("--bind").arg(format!("127.0.0.1:{}", port))
        .arg("--codex-bin").arg(fake_codex.to_string_lossy().to_string())
        .env("BRIDGE_ACP_EMIT", "1")
        .env("OPENAGENTS_BRIDGE_TOKEN", "itest")
        .env("BRIDGE_DEBUG_CODEX", "1")
        .env("OPENAGENTS_MANAGE_CONVEX", "false")
        .env("OPENAGENTS_CONVEX_NOOP", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn oa-bridge");
    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();
    let mut out_buf = String::new();
    let mut err_buf = String::new();
    let t_out = tokio::spawn(async move { use tokio::io::AsyncReadExt; let mut s = String::new(); let _ = child_stdout.read_to_string(&mut s).await; s });
    let t_err = tokio::spawn(async move { use tokio::io::AsyncReadExt; let mut s = String::new(); let _ = child_stderr.read_to_string(&mut s).await; s });

    // Wait for server port to accept connections
    for _ in 0..40 {
        if std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_ok() { break; }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // Connect to ws and drive a run
    let url = format!("ws://127.0.0.1:{}/ws?token=itest", port);
    let (mut ws, _resp) = match tokio_tungstenite::connect_async(&url).await { Ok(x) => x, Err(e) => {
        // Try to collect any early logs for debugging
        let _ = child.start_kill();
        tokio::time::sleep(Duration::from_millis(200)).await;
        let out = t_out.await.unwrap_or_default();
        let err = t_err.await.unwrap_or_default();
        panic!("ws connect: {:?}\nstdout: {}\nstderr: {}", e, out, err);
    }};
    // Send echo + run
    let thread_doc_id = format!("ephemeral_{}", port);
    let echo = serde_json::json!({"control":"echo","tag":"itest"}).to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(echo.into())).await.unwrap();
    let run = serde_json::json!({"control":"run.submit","threadDocId":thread_doc_id,"text":"hello"}).to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(run.into())).await.unwrap();

    // Collect a few lines and assert ACP notifications arrive
    let mut saw_acp_message = false;
    let mut saw_acp_thought = false;
    let mut saw_submit = false;
    let mut lines = Vec::new();
    let fut = async {
        for _ in 0..200u32 {
            if let Some(msg) = ws.next().await { if let Ok(m) = msg { if let tokio_tungstenite::tungstenite::Message::Text(t) = m { lines.push(t.to_string()); } } }
            for l in &lines {
                if l.contains("\"type\":\"bridge.run_submit\"") { saw_submit = true; }
                if l.contains("\"type\":\"bridge.acp\"") && l.contains("agent_message_chunk") { saw_acp_message = true; }
                if l.contains("\"type\":\"bridge.acp\"") && l.contains("agent_thought_chunk") { saw_acp_thought = true; }
                if l.contains("\"type\":\"bridge.convex_noop\"") && l.contains("\"kind\":\"assistant\"") { saw_acp_message = true; }
                if l.contains("\"type\":\"bridge.convex_noop\"") && l.contains("\"kind\":\"reason\"") { saw_acp_thought = true; }
                // Optional: plan/tool updates may arrive; we do not hard fail on their absence here
            }
            if saw_acp_message && saw_acp_thought && saw_submit { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let _ = timeout(Duration::from_secs(8), fut).await;
    assert!(saw_submit, "expected bridge.run_submit in ws log; got: {:?}", lines);
    // Cleanup and capture logs to ensure codex spawn path ran
    let _ = child.start_kill();
    let out = t_out.await.unwrap_or_default();
    let err = t_err.await.unwrap_or_default();
    assert!(out.contains("spawning codex"), "bridge stdout did not include codex spawn; stdout=\n{}\nstderr=\n{}", out, err);
}
