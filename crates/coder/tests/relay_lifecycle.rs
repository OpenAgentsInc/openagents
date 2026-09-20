//! The relay door's connection lifecycle, across many jobs.
//!
//! `relay_binding.rs` proves one job against a relay that lies. This file
//! proves what happens between jobs: that every job's subscription is
//! closed on every path that keeps the socket, that a relay ending a
//! subscription is a typed error rather than a silence, that a socket
//! which broke is not the one the next job runs over, and that opening a
//! socket is bounded even when the far end accepts and then says nothing.
//!
//! The loopback relay here is honest but bookkeeping: it tracks the
//! subscriptions open on each connection and counts connections, and the
//! tests read those counters. Everything runs on loopback with keys made
//! in the test.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use coder::generate::{Generate, GenerateError, Message, Role};
use coder::relay::{FEEDBACK_KIND, Identity, RESULT_KIND, RelayDoor};
use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, Tag};
use nostr::nip44;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite};

const CONTACT: Duration = Duration::from_secs(2);
const ANSWER: Duration = Duration::from_secs(5);
const WORKER: u8 = 0xaa;
const CLIENT: u8 = 0x0b;

type Server = WebSocketStream<TcpStream>;

/// What the relay does with the job it just took.
#[derive(Clone, Copy)]
enum Reply {
    /// Answer it, bound correctly.
    Answer,
    /// Refuse it with a typed status.
    Refuse,
    /// End the subscription with `CLOSED`.
    Close,
    /// Hang up the socket without a word.
    Drop,
}

/// What the tests read back.
#[derive(Default)]
struct Ledger {
    connections: AtomicUsize,
    /// The most subscriptions any one connection had open at once.
    peak_open: AtomicUsize,
    /// Subscriptions still open when a connection ended.
    leaked: AtomicUsize,
    /// How many `CLOSE` frames named a subscription that was open.
    closed: AtomicUsize,
}

fn identity(byte: u8) -> Identity {
    Identity::from_secret(SecretKey::from_byte_array([byte; 32]).unwrap()).unwrap()
}

fn xonly(pubkey: &str) -> XOnlyPublicKey {
    let mut bytes = [0u8; 32];
    for (index, pair) in pubkey.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16).unwrap();
        let low = (pair[1] as char).to_digit(16).unwrap();
        bytes[index] = ((high << 4) | low) as u8;
    }
    XOnlyPublicKey::from_byte_array(bytes).unwrap()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn send(socket: &mut Server, value: Value) -> bool {
    socket
        .send(tungstenite::Message::Text(value.to_string().into()))
        .await
        .is_ok()
}

async fn read(socket: &mut Server) -> Option<Value> {
    loop {
        match socket.next().await? {
            Ok(tungstenite::Message::Text(text)) => {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    return Some(value);
                }
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
}

fn bound(worker: &Identity, label: &str, request: &Event, kind: u16, payload: &Value) -> Value {
    let customer = xonly(&request.pubkey);
    let conversation = nip44::conversation_key(worker.secret(), &customer);
    let ciphertext = nip44::encrypt(
        &payload.to_string(),
        &conversation,
        secp256k1::rand::random::<[u8; 32]>(),
    )
    .unwrap();
    let event = worker.signer().sign(
        unix_now(),
        kind,
        vec![
            Tag::new(vec!["e".into(), request.id.clone()]),
            Tag::new(vec!["p".into(), request.pubkey.clone()]),
        ],
        ciphertext,
    );
    json!(["EVENT", label, event])
}

/// One connection: challenge, then serve jobs until the client hangs up,
/// consulting the shared script for what to do with each.
async fn serve(tcp: TcpStream, script: Arc<Mutex<Vec<Reply>>>, ledger: Arc<Ledger>) {
    ledger.connections.fetch_add(1, Ordering::SeqCst);
    let mut socket = accept_async(tcp).await.unwrap();
    send(&mut socket, json!(["AUTH", "lifecycle-challenge"])).await;
    let worker = identity(WORKER);
    let mut open: HashSet<String> = HashSet::new();
    // The label of the subscription the next request will answer under.
    let mut label = String::new();
    loop {
        let Some(frame) = read(&mut socket).await else {
            break;
        };
        match frame[0].as_str().unwrap_or_default() {
            "AUTH" => {
                let id = frame[1]["id"].as_str().unwrap_or_default().to_string();
                send(&mut socket, json!(["OK", id, true, ""])).await;
            }
            "REQ" => {
                label = frame[1].as_str().unwrap_or_default().to_string();
                open.insert(label.clone());
                ledger.peak_open.fetch_max(open.len(), Ordering::SeqCst);
            }
            "CLOSE" => {
                let name = frame[1].as_str().unwrap_or_default();
                if open.remove(name) {
                    ledger.closed.fetch_add(1, Ordering::SeqCst);
                }
            }
            "EVENT" => {
                let request: Event = serde_json::from_value(frame[1].clone()).unwrap();
                let reply = {
                    let mut script = script.lock().await;
                    if script.is_empty() {
                        Reply::Answer
                    } else {
                        script.remove(0)
                    }
                };
                if matches!(reply, Reply::Drop) {
                    break;
                }
                send(&mut socket, json!(["OK", request.id, true, ""])).await;
                let frames = match reply {
                    Reply::Answer => vec![bound(
                        &worker,
                        &label,
                        &request,
                        RESULT_KIND,
                        &json!({"v": 2, "type": "result", "text": "ok"}),
                    )],
                    Reply::Refuse => vec![bound(
                        &worker,
                        &label,
                        &request,
                        FEEDBACK_KIND,
                        &json!({
                            "v": 2, "type": "status", "status": "error",
                            "code": "quota_exhausted", "message": "no"
                        }),
                    )],
                    Reply::Close => {
                        open.remove(&label);
                        vec![json!(["CLOSED", label, "error: too many subscriptions"])]
                    }
                    Reply::Drop => unreachable!(),
                };
                for frame in frames {
                    if !send(&mut socket, frame).await {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    ledger.leaked.fetch_add(open.len(), Ordering::SeqCst);
}

/// A relay on loopback that serves every connection it is offered.
async fn spawn_relay(script: Vec<Reply>) -> (String, Arc<Ledger>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let ledger = Arc::new(Ledger::default());
    let script = Arc::new(Mutex::new(script));
    let served = Arc::clone(&ledger);
    tokio::spawn(async move {
        loop {
            let (tcp, _) = listener.accept().await.unwrap();
            tokio::spawn(serve(tcp, Arc::clone(&script), Arc::clone(&served)));
        }
    });
    (url, ledger)
}

fn door(url: String) -> RelayDoor {
    RelayDoor::new(url, xonly(identity(WORKER).pubkey()), identity(CLIENT))
        .waiting(CONTACT, ANSWER)
        .connecting(Duration::from_millis(500))
}

async fn turn(door: &RelayDoor) -> Result<String, GenerateError> {
    let input = vec![Message {
        role: Role::User,
        text: "ping".to_string(),
    }];
    door.generate("be terse", &input, &mut |_| {}, &mut |_| {})
        .await
        .map(|(text, _)| text)
}

/// Lets the relay task observe the client's last frame before a counter
/// is read.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(100)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forty_jobs_share_one_socket_and_leave_no_subscription_open() {
    let (url, ledger) = spawn_relay(Vec::new()).await;
    let door = door(url);
    for _ in 0..40 {
        assert_eq!(turn(&door).await.unwrap(), "ok");
    }
    settle().await;
    assert_eq!(ledger.connections.load(Ordering::SeqCst), 1);
    assert_eq!(ledger.peak_open.load(Ordering::SeqCst), 1);
    assert_eq!(ledger.closed.load(Ordering::SeqCst), 40);
    drop(door);
    settle().await;
    assert_eq!(ledger.leaked.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refusal_closes_its_subscription_and_keeps_the_socket() {
    let (url, ledger) = spawn_relay(vec![Reply::Refuse]).await;
    let door = door(url);
    let refused = turn(&door).await;
    assert!(
        matches!(&refused, Err(GenerateError::Refused { code, .. }) if code == "quota_exhausted"),
        "{refused:?}"
    );
    assert_eq!(turn(&door).await.unwrap(), "ok");
    settle().await;
    assert_eq!(ledger.connections.load(Ordering::SeqCst), 1);
    assert_eq!(ledger.closed.load(Ordering::SeqCst), 2);
    assert_eq!(ledger.peak_open.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_closed_subscription_is_a_relay_error_not_a_silence() {
    let (url, ledger) = spawn_relay(vec![Reply::Close]).await;
    let door = door(url);
    let started = std::time::Instant::now();
    let closed = turn(&door).await;
    assert!(
        matches!(&closed, Err(GenerateError::Relay(why)) if why.contains("closed the job subscription") && why.contains("too many subscriptions")),
        "{closed:?}"
    );
    assert!(
        started.elapsed() < CONTACT,
        "CLOSED must end the job at once, not after the contact wait"
    );
    // The socket is healthy and the next job runs over it.
    assert_eq!(turn(&door).await.unwrap(), "ok");
    settle().await;
    assert_eq!(ledger.connections.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_socket_the_relay_dropped_is_replaced_for_the_next_job() {
    let (url, ledger) = spawn_relay(vec![Reply::Drop]).await;
    let door = door(url);
    let dropped = turn(&door).await;
    // A clean close is a silence; a reset is a stream failure. Either
    // way the socket is gone and no other error would be honest.
    assert!(
        matches!(
            &dropped,
            Err(GenerateError::Silent { heard: false, .. } | GenerateError::Stream(_))
        ),
        "{dropped:?}"
    );
    assert_eq!(turn(&door).await.unwrap(), "ok");
    settle().await;
    assert_eq!(ledger.connections.load(Ordering::SeqCst), 2);
    assert_eq!(ledger.closed.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wait_that_ran_out_evicts_the_socket() {
    // A relay that takes the job and answers nothing: the first turn ends
    // on the contact wait, and the second turn opens a fresh socket
    // rather than reading the first job's late answer.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let connections = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&connections);
    tokio::spawn(async move {
        loop {
            let (tcp, _) = listener.accept().await.unwrap();
            counted.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let mut socket = accept_async(tcp).await.unwrap();
                send(&mut socket, json!(["AUTH", "silent-challenge"])).await;
                while let Some(frame) = read(&mut socket).await {
                    match frame[0].as_str().unwrap_or_default() {
                        "AUTH" | "EVENT" => {
                            let id = frame[1]["id"].as_str().unwrap_or_default().to_string();
                            send(&mut socket, json!(["OK", id, true, ""])).await;
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    let door = RelayDoor::new(url, xonly(identity(WORKER).pubkey()), identity(CLIENT))
        .waiting(Duration::from_millis(300), ANSWER);
    let silent = turn(&door).await;
    assert!(
        matches!(&silent, Err(GenerateError::Silent { heard: false, .. })),
        "{silent:?}"
    );
    let silent = turn(&door).await;
    assert!(matches!(&silent, Err(GenerateError::Silent { .. })));
    settle().await;
    assert_eq!(connections.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_relay_that_accepts_and_says_nothing_is_bounded() {
    // A TCP listener that never completes the WebSocket handshake. The
    // door's connect bound, not the operating system's, ends the wait.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let mut held = Vec::new();
        loop {
            let (tcp, _) = listener.accept().await.unwrap();
            held.push(tcp);
        }
    });
    let door = door(url);
    let started = std::time::Instant::now();
    let refused = turn(&door).await;
    assert!(
        matches!(&refused, Err(GenerateError::Relay(why)) if why.contains("no WebSocket handshake")),
        "{refused:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}
