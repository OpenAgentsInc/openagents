use tokio::process::Command;
use std::process::Stdio;
use std::time::Duration;
use tokio::time::timeout;
use futures_util::{sink::SinkExt, StreamExt};
use std::fs;

#[tokio::test]
async fn full_flow_with_fake_claude_emits_acp() {
    // Build binaries
    assert!(Command::new("cargo").args(["build", "-p", "fake-claude"]).status().await.expect("cargo build fake-claude").success());
    assert!(Command::new("cargo").args(["build", "-p", "oa-bridge"]).status().await.expect("cargo build oa-bridge").success());
    // Resolve paths
    let workspace = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ws_root = std::path::Path::new(&workspace).ancestors().nth(2).unwrap().to_path_buf();
    let target_dir = ws_root.join("target").join("debug");
    let fake_claude = target_dir.join(if cfg!(windows) { "fake-claude.exe" } else { "fake-claude" });
    let bridge_bin = target_dir.join(if cfg!(windows) { "oa-bridge.exe" } else { "oa-bridge" });
    assert!(fake_claude.exists(), "fake-claude binary not found at {:?}", fake_claude);
    assert!(bridge_bin.exists(), "oa-bridge binary not found at {:?}", bridge_bin);

    // Setup a temp OPENAGENTS_HOME with a Claude project
    let td = tempfile::tempdir().unwrap();
    let home = td.path().to_path_buf();
    let proj_dir = home.join("projects").join("itest");
    fs::create_dir_all(&proj_dir).unwrap();
    let project_md = format!("---\nname: ITest\nworkingDir: {}\nagentFile: claude_code\n---\n", ws_root.to_string_lossy());
    fs::write(proj_dir.join("PROJECT.md"), project_md).unwrap();

    // Allocate port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind temp port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Spawn bridge with fake Claude CLI and OPENAGENTS_HOME pointing to temp dir
    let mut child = Command::new(bridge_bin)
        .arg("--bind").arg(format!("127.0.0.1:{}", port))
        .env("CLAUDE_BIN", fake_claude.to_string_lossy().to_string())
        .env("OPENAGENTS_BRIDGE_TOKEN", "itest")
        .env("OPENAGENTS_MANAGE_CONVEX", "false")
        .env("OPENAGENTS_CONVEX_NOOP", "1")
        .env("OPENAGENTS_HOME", home.to_string_lossy().to_string())
        .env("BRIDGE_ACP_EMIT", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn oa-bridge");
    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();
    let t_out = tokio::spawn(async move { use tokio::io::AsyncReadExt; let mut s = String::new(); let _ = child_stdout.read_to_string(&mut s).await; s });
    let t_err = tokio::spawn(async move { use tokio::io::AsyncReadExt; let mut s = String::new(); let _ = child_stderr.read_to_string(&mut s).await; s });

    // Wait for server
    for _ in 0..40 {
        if std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_ok() { break; }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Connect to ws and submit with projectId
    let url = format!("ws://127.0.0.1:{}/ws?token=itest", port);
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.expect("ws connect");
    let thread_doc_id = format!("ephemeral_claude_{}", port);
    let run = serde_json::json!({"control":"run.submit","threadDocId":thread_doc_id,"text":"hello","projectId":"itest"}).to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(run.into())).await.unwrap();

    // Collect a few lines and assert ACP notifications arrive
    let mut saw_submit = false;
    let mut saw_acp = false;
    let mut lines = Vec::new();
    let fut = async {
        for _ in 0..200u32 {
            if let Some(msg) = ws.next().await { if let Ok(m) = msg { if let tokio_tungstenite::tungstenite::Message::Text(t) = m { lines.push(t.to_string()); } } }
            for l in &lines {
                if l.contains("\"type\":\"bridge.run_submit\"") && l.contains("\"provider\":\"claude_code\"") { saw_submit = true; }
                if l.contains("\"type\":\"bridge.acp_seen\"") { saw_acp = true; }
            }
            if saw_submit && saw_acp { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let _ = timeout(Duration::from_secs(8), fut).await;
    assert!(saw_submit, "missing expected run.submit logs: {:?}", lines);
    assert!(saw_acp, "missing expected Claude ACP events: {:?}", lines);
    let _ = child.start_kill();
    let _ = t_out.await.unwrap_or_default();
    let _ = t_err.await.unwrap_or_default();
}
