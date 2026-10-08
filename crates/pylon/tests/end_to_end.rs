#![allow(clippy::unwrap_used, clippy::expect_used)]
//! A pylon and a buyer against an in-process relay, with the echo engine:
//! discovery from the beacon, an encrypted free job, the receipt, the field
//! projection, and a pool aggregate that recomputes.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, EventClass, Filter};
use nostr::pylon::{PoolPolicy, parse_receipt};
use pylon::client::{self, Ask};
use pylon::engine::Echo;
use pylon::field::{Live, RelayField};
use pylon::identity::Identity;
use pylon::pool;
use pylon::provider::{Config, Provider};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};
use tokio_tungstenite::tungstenite::Message;

struct Sub {
    conn: u64,
    id: String,
    filters: Vec<Filter>,
    tx: mpsc::UnboundedSender<String>,
}

#[derive(Default)]
struct Hub {
    stored: Vec<Event>,
    subs: Vec<Sub>,
}

/// A minimal NIP-01/NIP-42 relay: stores regular and addressable events,
/// fans every accepted event out to matching subscriptions, and keeps no
/// ephemeral event.
async fn relay() -> (String, Arc<Mutex<Hub>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let hub = Arc::new(Mutex::new(Hub::default()));
    let shared = Arc::clone(&hub);
    tokio::spawn(async move {
        let mut next = 0_u64;
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            next += 1;
            let conn = next;
            let hub = Arc::clone(&shared);
            tokio::spawn(async move {
                let Ok(socket) = tokio_tungstenite::accept_async(stream).await else {
                    return;
                };
                let (mut sink, mut source) = socket.split();
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                tokio::spawn(async move {
                    while let Some(text) = rx.recv().await {
                        if sink.send(Message::Text(text.into())).await.is_err() {
                            return;
                        }
                    }
                });
                tx.send(json!(["AUTH", "challenge"]).to_string()).unwrap();
                while let Some(Ok(Message::Text(text))) = source.next().await {
                    let frame: Value = serde_json::from_str(&text).unwrap();
                    match frame[0].as_str().unwrap() {
                        "AUTH" => {
                            let _ = tx.send(json!(["OK", frame[1]["id"], true, ""]).to_string());
                        }
                        "EVENT" => {
                            let event: Event = serde_json::from_value(frame[1].clone()).unwrap();
                            let ok = event.validate_crypto().is_ok();
                            let _ = tx.send(json!(["OK", event.id, ok, ""]).to_string());
                            if !ok {
                                continue;
                            }
                            let mut hub = hub.lock().await;
                            match event.class() {
                                EventClass::Ephemeral => {}
                                EventClass::Addressable => {
                                    let d = event
                                        .tag_values("d")
                                        .next()
                                        .unwrap_or_default()
                                        .to_string();
                                    hub.stored.retain(|e| {
                                        !(e.kind == event.kind
                                            && e.pubkey == event.pubkey
                                            && e.tag_values("d").next().unwrap_or_default() == d)
                                    });
                                    hub.stored.push(event.clone());
                                }
                                _ => hub.stored.push(event.clone()),
                            }
                            for sub in &hub.subs {
                                if nostr::domain::matches_any(&sub.filters, &event) {
                                    let _ =
                                        sub.tx.send(json!(["EVENT", sub.id, event]).to_string());
                                }
                            }
                        }
                        "REQ" => {
                            let id = frame[1].as_str().unwrap().to_string();
                            let filters: Vec<Filter> = frame.as_array().unwrap()[2..]
                                .iter()
                                .map(|f| serde_json::from_value(f.clone()).unwrap())
                                .collect();
                            let mut hub = hub.lock().await;
                            for event in &hub.stored {
                                if nostr::domain::matches_any(&filters, event) {
                                    let _ = tx.send(json!(["EVENT", id, event]).to_string());
                                }
                            }
                            let _ = tx.send(json!(["EOSE", id]).to_string());
                            hub.subs.push(Sub {
                                conn,
                                id,
                                filters,
                                tx: tx.clone(),
                            });
                        }
                        "CLOSE" => {
                            let id = frame[1].as_str().unwrap();
                            hub.lock()
                                .await
                                .subs
                                .retain(|s| !(s.conn == conn && s.id == id));
                        }
                        _ => {}
                    }
                }
                hub.lock().await.subs.retain(|s| s.conn != conn);
            });
        }
    });
    (url, hub)
}

async fn wait_for_beacon(hub: &Arc<Mutex<Hub>>) {
    for _ in 0..100 {
        if hub.lock().await.stored.iter().any(|e| e.kind == 30_200) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("no beacon arrived");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_free_job_runs_end_to_end_and_the_pool_counts_it() {
    let (url, hub) = relay().await;
    let home = tempfile::tempdir().unwrap();
    let provider_key = Identity::generate();
    let buyer = Identity::generate();
    let stranger = Identity::generate();

    let mut config = Config::new(&url, "test-4080", home.path().to_path_buf());
    config.allow = Some(BTreeSet::from([buyer.pubkey().to_string()]));
    config.rate_per_minute = 2;
    let provider = Provider::new(config, provider_key.clone(), Arc::new(Echo)).unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(Arc::clone(&provider).run(async {
        let _ = stop_rx.await;
    }));
    wait_for_beacon(&hub).await;

    // The buyer finds the pylon from its beacon and gets an answer.
    let ask = |prompt: &str| Ask {
        relay: url.clone(),
        pylon: None,
        prompt: prompt.into(),
        wait: Duration::from_secs(10),
        publish_receipt: true,
        home: home.path().to_path_buf(),
    };
    let answer = client::ask(&buyer, &ask("hello pylon")).await.unwrap();
    assert_eq!(answer.text.as_deref(), Some("echo: nolyp olleh"));
    assert_eq!(answer.outcome, "accepted");
    assert_eq!(answer.pylon_npub, provider_key.npub());
    assert!(answer.contact_ms.is_some() && answer.answer_ms.is_some());

    // The receipt is on the relay, verifies, names the request, and is free.
    let receipt_id = answer.receipt.clone().unwrap();
    let stored = hub
        .lock()
        .await
        .stored
        .iter()
        .find(|e| e.id == receipt_id)
        .cloned()
        .unwrap();
    let receipt = parse_receipt(&stored, None).unwrap();
    assert_eq!(receipt.provider, provider_key.pubkey());
    assert_eq!(receipt.request, answer.request);
    assert!(receipt.payment.is_none());
    assert!(home.path().join("receipts.jsonl").exists());
    // No job content reached the relay in the clear.
    for event in &hub.lock().await.stored {
        assert!(!event.content.contains("hello pylon"));
    }

    // A key that is not on the allowlist is refused before any model work.
    let refused = client::ask(&stranger, &ask("let me in")).await.unwrap();
    assert!(refused.text.is_none());
    assert!(refused.error.unwrap().starts_with("not_admitted"));

    // The admitted buyer's rate limit (2 per minute) refuses a third job.
    let second = client::ask(&buyer, &ask("again")).await.unwrap();
    assert!(second.text.is_some());
    let third = client::ask(&buyer, &ask("and again")).await.unwrap();
    assert!(third.error.unwrap().starts_with("rate_limited"));
    assert_eq!(provider.counters().await.served, 2);

    // The field shows the pylon online with its receipt-backed jobs.
    let field = RelayField::new(&url, Some("everglade"), Identity::generate());
    let states = field.poll().await.unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].status, "online");
    assert_eq!(states[0].jobs, 2);

    // The aggregator counts the accepted jobs, and a reader recomputes it.
    let aggregator = Identity::generate();
    let policy = PoolPolicy::open("everglade", pool::SLICES);
    let (aggregate, event) = pool::aggregate(&aggregator, &url, &policy, 60, true)
        .await
        .unwrap();
    assert!(event.is_some());
    assert_eq!(aggregate.totals.pylons_online, 1);
    assert_eq!(aggregate.totals.jobs.accepted, 2);
    assert_eq!(aggregate.totals.jobs.failed, 2);
    let verified = pool::verify(&Identity::generate(), &url, aggregator.pubkey(), &policy)
        .await
        .unwrap();
    assert_eq!(verified, aggregate);
    // Every job's in-flight mark is gone once it ended.
    assert!(pylon::inflight::read(home.path(), pylon::now()).is_empty());

    // A live subscription shows the same pylon and jobs, and recomputes
    // the aggregate.
    let live = Arc::new(std::sync::Mutex::new(Live::new(Some("everglade"))));
    let stop = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (live, stop, url) = (Arc::clone(&live), Arc::clone(&stop), url.clone());
        tokio::spawn(async move {
            RelayField::new(&url, Some("everglade"), Identity::generate())
                .watch(&live, &stop)
                .await;
        })
    };
    let seen = |check: fn(&Live) -> bool| {
        let live = Arc::clone(&live);
        async move {
            for _ in 0..100 {
                if check(&live.lock().unwrap()) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            panic!(
                "the live field never showed it: {:?}",
                live.lock().unwrap().error
            );
        }
    };
    seen(|l| l.synced).await;
    {
        let l = live.lock().unwrap();
        let pylons = l.pylons(pylon::now());
        assert_eq!(pylons.len(), 1);
        assert_eq!(pylons[0].state.status, "online");
        assert_eq!(pylons[0].state.jobs, 2);
        let (held, recomputed) = l.aggregate(pylon::now()).unwrap();
        assert_eq!(held.totals.jobs.accepted, 2);
        assert!(recomputed);
    }

    // Stopping publishes an offline beacon, which the live field sees
    // without polling. A beacon sampled in the same second as the one held
    // doesn't replace it, so the stop waits for the clock to turn.
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    stop_tx.send(()).unwrap();
    running.await.unwrap().unwrap();
    let states = field.poll().await.unwrap();
    assert_eq!(states[0].status, "offline");
    seen(|l| l.pylons(pylon::now())[0].state.status == "offline").await;
    stop.store(true, Ordering::Relaxed);
    watcher.await.unwrap();
}
