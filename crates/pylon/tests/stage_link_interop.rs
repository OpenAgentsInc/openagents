#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The fake stages' Python link (`fixtures/stage_link.py`) and the Rust
//! `stage_link` crate speak one wire: frames cross in both directions, and
//! each side refuses the other's wrong key or generation.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};
use stage_link::{Frame, Hop, Key, Kind, Limits, LinkError, Listener, Refusal, connect};

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stage_link.py")
}

fn hop(generation: u64) -> Hop {
    Hop {
        group: "sha256:interop".into(),
        generation,
        from: 1,
        to: 2,
    }
}

fn args(key: &Key, generation: u64) -> Vec<String> {
    [
        "--key",
        &key.to_hex(),
        "--group",
        "sha256:interop",
        "--generation",
        &generation.to_string(),
        "--from",
        "1",
        "--to",
        "2",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

/// Start `stage_link.py listen` and read the port it prints.
fn python_listener(
    key: &Key,
    generation: u64,
) -> (
    std::process::Child,
    std::io::Lines<BufReader<std::process::ChildStdout>>,
    u16,
) {
    let mut child = Command::new("python3")
        .arg(script())
        .arg("listen")
        .args(args(key, generation))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let first: serde_json::Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    let port = u16::try_from(first["port"].as_u64().unwrap()).unwrap();
    (child, lines, port)
}

fn frame(step: u64) -> Frame {
    Frame {
        kind: Kind::State,
        generation: 3,
        request_id: "rust".into(),
        attempt: 1,
        step,
        meta: br#"{"from":"rust"}"#.to_vec(),
        state: (0..44_300u32)
            .map(|i| (i * 7 + step as u32) as u8)
            .collect(),
    }
}

#[tokio::test]
async fn rust_sends_and_python_reads() {
    let key = Key::generate().unwrap();
    let (mut child, mut lines, port) = python_listener(&key, 3);
    let mut sender = connect(
        format!("127.0.0.1:{port}"),
        &key,
        &hop(3),
        &Limits::default(),
    )
    .await
    .unwrap();
    let mut sent = 0;
    for step in 0..3 {
        sent += sender.send(&frame(step)).await.unwrap();
    }
    sender
        .send(&Frame {
            kind: Kind::Cancel,
            meta: vec![],
            state: vec![],
            ..frame(3)
        })
        .await
        .unwrap();
    sender.close().await.unwrap();
    for step in 0..3 {
        let line: serde_json::Value =
            serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
        assert_eq!(line["step"], step);
        assert_eq!(line["kind"], 1);
        assert_eq!(line["request_id"], "rust");
        assert_eq!(line["meta"], r#"{"from":"rust"}"#);
        assert_eq!(line["len"], 44_300);
        let digest = format!("{:x}", Sha256::digest(&frame(step).state));
        assert_eq!(line["sha256"], digest);
    }
    let cancel: serde_json::Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(cancel["kind"], 2);
    let end: serde_json::Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(end["end"], "closed");
    assert!(end["bytes"].as_u64().unwrap() > sent as u64);
    child.wait().unwrap();
}

#[tokio::test]
async fn python_sends_and_rust_reads() {
    let key = Key::generate().unwrap();
    let listener = Listener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let child = Command::new("python3")
        .arg(script())
        .arg("send")
        .args([
            "--address",
            &address,
            "--frames",
            "4",
            "--state-bytes",
            "5000",
        ])
        .args(args(&key, 3))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let limits = Limits::default();
    let mut receiver = listener
        .accept(&limits)
        .await
        .unwrap()
        .accept(&key, &hop(3), &limits)
        .await
        .unwrap();
    for step in 0..4u64 {
        let frame = receiver.recv().await.unwrap();
        assert_eq!(frame.step, step);
        assert_eq!(frame.request_id, "py");
        assert_eq!(frame.meta, br#"{"from":"python"}"#);
        let want: Vec<u8> = (0..5000u64).map(|i| ((i + step) % 251) as u8).collect();
        assert_eq!(frame.state, want);
    }
    assert_eq!(receiver.recv().await, Err(LinkError::Closed));
    let out = child.wait_with_output().unwrap();
    let line: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(line["sent"], 4);
    assert_eq!(line["bytes"].as_u64().unwrap(), receiver.bytes());
}

#[tokio::test]
async fn each_side_refuses_the_others_wrong_key_or_generation() {
    let key = Key::generate().unwrap();
    // Rust with the wrong key, Python listening.
    let (mut child, mut lines, port) = python_listener(&key, 3);
    let sent = connect(
        format!("127.0.0.1:{port}"),
        &Key::generate().unwrap(),
        &hop(3),
        &Limits::default(),
    )
    .await;
    assert_eq!(sent.err(), Some(LinkError::Refused(Refusal::Auth)));
    let end: serde_json::Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(end["end"], "refused: auth");
    child.wait().unwrap();

    // Python from an old generation, Rust listening.
    let listener = Listener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let child = Command::new("python3")
        .arg(script())
        .arg("send")
        .args(["--address", &address])
        .args(args(&key, 2))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let limits = Limits::default();
    let offer = listener.accept(&limits).await.unwrap();
    assert_eq!(offer.hop().generation, 2);
    assert_eq!(
        offer.accept(&key, &hop(3), &limits).await.err(),
        Some(LinkError::Refused(Refusal::Generation))
    );
    let out = child.wait_with_output().unwrap();
    let line: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(line["error"], "refused: generation");
}
