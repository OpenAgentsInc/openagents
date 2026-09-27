//! Temporary read-only NIP-MV relay admission probe. Only AUTH is signed.
use nostr::domain::{RelaySigner, Tag};
use serde_json::json;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use verse::net::{In, Link, Out};

fn main() {
    let url = "wss://relay.openagents.com";
    let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string()).unwrap();
    let started = Instant::now();
    let mut link = Link::start(url);
    link.send(Out::Subscribe { id: "probe-mv-live".into(),
        filters: vec![json!({"kinds": [23300,23301], "#w": ["verse-plaza"]})], live: true });
    link.send(Out::Subscribe { id: "probe-mv-state".into(),
        filters: vec![json!({"kinds": [33301], "#w": ["verse-plaza"], "limit": 500})], live: true });
    let mut auth_id = None;
    let mut auth_accepted = None;
    let mut connected_ms = None;
    let mut live_ms = None;
    let mut state_ms = None;
    let mut failures = Vec::new();
    while started.elapsed() < Duration::from_secs(15) {
        for message in link.drain() {
            let ms = started.elapsed().as_millis();
            match message {
                In::Connected => { connected_ms = Some(ms); live_ms = None; state_ms = None; auth_accepted = None; }
                In::Auth(challenge) => {
                    live_ms = None; state_ms = None;
                    let event = signer.sign(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        22242, vec![Tag::new(vec!["relay".into(),url.into()]),Tag::new(vec!["challenge".into(),challenge])], String::new());
                    auth_id = Some(event.id.clone());
                    link.send(Out::Auth(event));
                }
                In::Ok { id, accepted, message } if auth_id.as_deref() == Some(&id) => {
                    auth_accepted = Some(accepted);
                    live_ms = None; state_ms = None;
                    if !accepted { failures.push(json!({"operation":"auth", "prefix": prefix(&message), "ms":ms})); }
                }
                In::Eose(id) if id == "probe-mv-live" => live_ms = Some(ms),
                In::Eose(id) if id == "probe-mv-state" => state_ms = Some(ms),
                In::Closed(id, message) => failures.push(json!({"operation":id,"prefix":prefix(&message),"ms":ms})),
                In::Disconnected(_) => failures.push(json!({"operation":"socket","prefix":"disconnected","ms":ms})),
                // Discard received payloads without inspecting, logging, or saving them.
                _ => {},
            }
        }
        if live_ms.is_some() && state_ms.is_some() && (auth_id.is_none() || auth_accepted == Some(true)) { break; }
        std::thread::sleep(Duration::from_millis(10));
    }
    link.send(Out::Close("probe-mv-live".into()));
    link.send(Out::Close("probe-mv-state".into()));
    let shutdown = link.shutdown(Duration::from_millis(100));
    println!("{}", json!({"relay":url,"world":"verse-plaza", "socket_connected_ms":connected_ms,
        "auth_challenged":auth_id.is_some(),"auth_accepted":auth_accepted,
        "live_subscription_eose_ms":live_ms,"state_subscription_eose_ms":state_ms,
        "elapsed_ms":started.elapsed().as_millis(),"failures":failures,"worker_stopped":shutdown,
        "published_events":0,"auth_only":true,"received_payloads_retained":false}));
}
fn prefix(message: &str) -> &str {
    let prefix = message.split(':').next().unwrap_or("");
    match prefix { "restricted" | "auth-required" | "invalid" | "error" | "blocked" | "rate-limited" => prefix, _ => "other" }
}
