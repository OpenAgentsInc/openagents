//! A standing subscription over the synthetic NIP-42 relay, with short
//! connection lifetimes so a test spans several renewals.

use super::*;
use nostr::domain::{RelaySigner, Tag};
use std::sync::Mutex;
use std::time::Instant;

#[path = "../../../../coder-control/src/tests/relay.rs"]
mod relay;

/// NIP-CJ's execution request kind: ephemeral, so a relay never replays it.
const EPHEMERAL: u16 = 25920;

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    crate::unix_time().unwrap()
}

/// A subscription to ephemeral events addressed to `host`.
fn ephemeral(relay: &str, host: &str, lifetime: Duration) -> Subscription {
    let host = host.to_owned();
    Subscription {
        relay: relay.to_owned(),
        subscription: "standing-test",
        lifetime,
        filter: Box::new(move |_| json!({"kinds": [EPHEMERAL], "#p": [host]})),
    }
}

/// Publish `count` ephemeral events to `host`, `every` apart, and return
/// their IDs.
async fn send(relay: &str, host: &str, count: usize, every: Duration) -> Vec<String> {
    let sender = key();
    let signer = RelaySigner::from_secret_hex(&sender.display_secret().to_string()).unwrap();
    let mut ids = Vec::new();
    let mut socket: Option<Connection> = None;
    for index in 0..count {
        let event = signer.sign(
            now(),
            EPHEMERAL,
            vec![Tag::new(vec!["p".into(), host.into()])],
            format!("request {index}"),
        );
        loop {
            if socket.is_none() {
                socket = Connection::connect(relay, &sender, Duration::from_secs(60))
                    .await
                    .ok();
            }
            let Some(connection) = socket.as_mut() else {
                continue;
            };
            if connection.send(json!(["EVENT", event])).await.is_ok()
                && connection.next().await.is_ok_and(|ok| ok[2] == true)
            {
                break;
            }
            socket = None;
        }
        ids.push(event.id.clone());
        tokio::time::sleep(every).await;
    }
    ids
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn renewals_overlap_so_an_ephemeral_event_is_never_missed_or_handled_twice() {
    let (relay, _task, _events) = relay::start().await;
    let host = key();
    let host_key = coder_reach::pubkey(&host);
    let publisher = Publisher::spawn(std::slice::from_ref(&relay), host);
    let handled: Arc<Mutex<Vec<String>>> = Arc::default();
    let connections = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (seen, counted) = (handled.clone(), connections.clone());
    // Two-second connections renew every 1.34 s, so a 5.5 s stream crosses
    // several renewals and connection ends.
    let serving = tokio::spawn(serve(
        host,
        publisher,
        ephemeral(&relay, &host_key, Duration::from_secs(2)),
        move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
        move |event, _reply| {
            let seen = seen.clone();
            async move { seen.lock().unwrap().push(event.id) }
        },
    ));
    tokio::time::sleep(Duration::from_millis(500)).await;
    let sent = send(&relay, &host_key, 55, Duration::from_millis(100)).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    serving.abort();
    let mut got = handled.lock().unwrap().clone();
    assert_eq!(got.len(), sent.len(), "each event handled exactly once");
    got.sort();
    let mut want = sent;
    want.sort();
    assert_eq!(got, want);
    assert!(connections.load(std::sync::atomic::Ordering::Relaxed) >= 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn events_are_handled_at_once_and_each_reply_reaches_the_relay() {
    let (relay, _task, events) = relay::start().await;
    let host = key();
    let host_key = coder_reach::pubkey(&host);
    let publisher = Publisher::spawn(std::slice::from_ref(&relay), host);
    let signer = RelaySigner::from_secret_hex(&host.display_secret().to_string()).unwrap();
    let serving = tokio::spawn(serve(
        host,
        publisher,
        ephemeral(&relay, &host_key, Duration::from_secs(30)),
        || {},
        move |event, reply| {
            let signer = signer.clone();
            async move {
                // A slow request, such as one that waits on the task store.
                tokio::time::sleep(Duration::from_millis(800)).await;
                let answer = signer.sign(
                    crate::unix_time().unwrap(),
                    26920,
                    vec![Tag::new(vec!["p".into(), event.pubkey.clone()])],
                    event.id.clone(),
                );
                reply.send(answer).await;
            }
        },
    ));
    tokio::time::sleep(Duration::from_millis(500)).await;
    let started = Instant::now();
    let sent = send(&relay, &host_key, 5, Duration::ZERO).await;
    // Five 800 ms requests, one after another, would take four seconds.
    let answered = |events: &std::collections::BTreeMap<String, Event>| {
        sent.iter()
            .all(|id| events.values().any(|e| e.kind == 26920 && &e.content == id))
    };
    while !answered(&*events.lock().await) {
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "answered one at a time"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    serving.abort();
}
