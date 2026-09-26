//! The foreground Rust app reads only generated fixtures through a real local
//! authenticated WebSocket. This is not a device or production-relay test.
use super::{App, Config, Packet, Request};
use base64::Engine;
use coder_connect::{RelayPolicy, host::Host, transport::Receiver};
use coder_history::TranscriptPage;
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

struct Server {
    relay: String,
    start: Option<tokio::sync::oneshot::Sender<()>>,
    ready: mpsc::Receiver<()>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Server {
    fn new(state: PathBuf) -> Self {
        let (address_tx, address_rx) = mpsc::channel();
        let (ready_tx, ready) = mpsc::channel();
        let (start, start_rx) = tokio::sync::oneshot::channel();
        let (stop, mut stop_rx) = tokio::sync::oneshot::channel();
        let thread = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let (url, relay_task, _) = relay::start().await;
                address_tx.send(url.clone()).unwrap();
                tokio::select! {
                    _ = &mut stop_rx => { relay_task.abort(); return; },
                    begun = start_rx => { if begun.is_err() { relay_task.abort(); return; } },
                }
                let host = Host::new(state, RelayPolicy::LoopbackTest);
                let mut receiver =
                    Receiver::connect(&url, &host.key().unwrap(), RelayPolicy::LoopbackTest)
                        .await
                        .unwrap();
                ready_tx.send(()).unwrap();
                loop {
                    tokio::select! {
                        _ = &mut stop_rx => break,
                        incoming = receiver.next_request() => {
                            let event = incoming.unwrap();
                            let reply = host.handle_current(&event, &url).unwrap();
                            receiver.publish(&reply).await.unwrap();
                        },
                    }
                }
                relay_task.abort();
                let _ = relay_task.await;
            });
        });
        Self {
            relay: address_rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            start: Some(start),
            ready,
            stop: Some(stop),
            thread: Some(thread),
        }
    }

    fn start(&mut self) {
        self.start.take().unwrap().send(()).unwrap();
        self.ready.recv_timeout(Duration::from_secs(10)).unwrap();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let outcome = thread.join();
            if !thread::panicking() {
                outcome.unwrap();
            }
        }
    }
}

fn config(cache: &Path, secret: &SecretKey) -> Config {
    Config {
        cache_dir: cache.into(),
        secret_hex: secret.display_secret().to_string(),
        synthetic: true,
    }
}

fn checked(packet: Packet) -> Packet {
    assert!(packet.error.is_none(), "{:?}", packet.error);
    packet
}

fn button(value: &Value, title: &str) -> Option<String> {
    if value["element"]["kind"] == "button"
        && value["element"]["props"]["label"]
            .as_str()
            .is_some_and(|label| label.starts_with(title))
    {
        return value["key"].as_str().map(str::to_owned);
    }
    value["element"]["props"]["children"]
        .as_array()
        .and_then(|children| children.iter().find_map(|child| button(child, title)))
}

fn open_chat(app: &mut App) {
    let view = checked(app.call(Request::Snapshot)).view.unwrap();
    let node = button(&view["root"], "Synthetic connected chat").unwrap();
    checked(app.call(Request::Activate {
        instance: view["instance"].as_str().unwrap().into(),
        revision: view["revision"].as_u64().unwrap(),
        node,
    }));
}

fn retained_bytes(app: &App) -> Vec<u8> {
    let mut bytes = Vec::new();
    for key in app.page_keys().unwrap() {
        let page: TranscriptPage = app.cache.read(&key).unwrap().unwrap();
        for chunk in page.chunks {
            assert_eq!(chunk.offset, bytes.len() as u64);
            bytes.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(&chunk.raw_base64)
                    .unwrap(),
            );
            assert_eq!(chunk.end_offset, bytes.len() as u64);
        }
    }
    bytes
}

fn message(text: &str) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({"type":"response_item","payload":{
        "type":"message","role":"assistant","content":[{"type":"output_text","text":text}]
    }}))
    .unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn app_redeems_qr_invitation_pages_refreshes_restores_and_erases_revoked_history() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic-codex");
    let state = temp.path().join("host");
    let cache = temp.path().join("phone-cache");
    let log = source.join("sessions/2026/01/01/fixture.jsonl");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    std::fs::write(
        source.join("session_index.jsonl"),
        b"{\"id\":\"fixture\",\"thread_name\":\"Synthetic connected chat\"}\n",
    )
    .unwrap();
    let mut original = b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"fixture\"}}\n".to_vec();
    for index in 0..48 {
        original.extend(message(&format!(
            "Synthetic message {index}: {}",
            "safe fixture ".repeat(130)
        )));
    }
    assert!(original.len() > coder_history::MAX_PAGE_BYTES as usize * 2);
    std::fs::write(&log, &original).unwrap();
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let mut server = Server::new(state.clone());
    let host = Host::new(&state, RelayPolicy::LoopbackTest);
    let now = coder_connect::unix_time().unwrap();
    let invitation = host
        .invite(
            &server.relay,
            coder_history::Config {
                codex: Some(source),
                claude: None,
            },
            now,
            now + 3600,
        )
        .unwrap();
    server.start();
    let mut app = App::new(config(&cache, &secret)).unwrap();
    checked(app.call(Request::Connect { code: invitation }));
    let code = app.code.clone().unwrap();
    assert_eq!(code.client, coder_connect::protocol::pubkey(&secret));
    assert!(app.call(Request::Snapshot).paired);
    checked(app.call(Request::Refresh));
    assert_eq!(app.catalog.len(), 1);
    assert_eq!(app.catalog[0].title, "Synthetic connected chat");
    open_chat(&mut app);
    for _ in 0..8 {
        checked(app.call(Request::Refresh));
        if !app.transcript.has_more {
            break;
        }
    }
    assert!(!app.transcript.has_more);
    assert!(app.page_keys().unwrap().len() >= 3);
    assert_eq!(retained_bytes(&app), original);

    let appended = message("Appended while the phone was connected: café 日本語.");
    std::fs::OpenOptions::new()
        .append(true)
        .open(&log)
        .unwrap()
        .write_all(&appended)
        .unwrap();
    original.extend(appended);
    let refreshed = checked(app.call(Request::Refresh));
    assert!(
        refreshed
            .view
            .unwrap()
            .to_string()
            .contains("Appended while")
    );
    assert_eq!(retained_bytes(&app), original);
    drop(app);

    // A fresh App has no in-memory catalog, socket, transcript, or projection.
    // It must authenticate the encrypted cache before reconstructing the view.
    let mut restored = App::new(config(&cache, &secret)).unwrap();
    assert!(restored.status.starts_with("Cached"));
    open_chat(&mut restored);
    assert_eq!(retained_bytes(&restored), original);
    assert!(
        checked(restored.call(Request::Snapshot))
            .view
            .unwrap()
            .to_string()
            .contains("Appended while")
    );
    host.revoke(&code.grant, None, coder_connect::unix_time().unwrap())
        .unwrap();
    let revoked = restored.call(Request::Refresh);
    assert!(revoked.error.unwrap().contains("Access ended"));
    assert!(restored.code.is_none());
    assert!(restored.catalog.is_empty());
    assert!(restored.cache.keys("").unwrap().is_empty());
    assert!(restored.selected.is_none());
}
