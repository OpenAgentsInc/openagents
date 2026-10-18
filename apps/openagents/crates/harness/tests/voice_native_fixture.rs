#![cfg(unix)]

use serde_json::{Value, json};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command};

#[tokio::test]
async fn native_voice_fixture_accepts_relay_instructions_without_a_brand_phrase() {
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("bin/codex");
    std::fs::create_dir(binary.parent().unwrap()).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-codex-voice-native.py"),
        &binary,
    )
    .unwrap();
    // 83be482194 (#12061) changed the relay prompt without changing the protocol.
    let instructions = "Read get_voice_agent, send the full question with send_message, and wait_for_turn. Summarize the agent's reply.";
    let request = json!({
        "id": 1,
        "method": "thread/realtime/start",
        "params": {
            "threadId": "idle-thread",
            "transport": {"type": "webrtc", "sdp": "fixture-offer"},
            "version": "v3",
            "outputModality": "audio",
            "realtimeSessionId": "fixture-session",
            "clientManagedHandoffs": false,
            "includeStartupContext": true,
            "realtimeStartInstructions": instructions,
            "initialItems": [{"role": "developer", "text": instructions}]
        }
    });
    let mut child = Command::new("/usr/bin/python3")
        .arg(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input
        .write_all(format!("{request}\n").as_bytes())
        .await
        .unwrap();
    drop(input);
    let output = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .expect("the fake native voice server must exit after stdin closes")
        .unwrap();
    assert!(
        output.status.success(),
        "fake native voice server failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let frames: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(
        frames
            .iter()
            .any(|frame| frame["id"] == 1 && frame["result"] == json!({}))
    );
    assert!(frames.iter().any(|frame| {
        frame["method"] == "thread/realtime/sdp" && frame["params"]["sdp"] == "fixture-answer"
    }));
}
