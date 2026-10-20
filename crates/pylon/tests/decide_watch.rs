#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `check decide --watch` against an in-process relay: the continuous
//! checker checks a new decision pylon at once and again after the
//! interval, leaves an offline pylon alone, draws its items from the
//! checker-private seed, and sends each round from a new buyer key.

use std::sync::Arc;
use std::time::Duration;

use nostr::pylon::{Status, Verdict, beacon_event, parse_beacon, parse_receipt, sha256_hex};
use pylon::check::Checker;
use pylon::check_decide::{Items, WatchConfig, Watcher};
use pylon::decide::{Fixed, Identity as Served};
use pylon::identity::Identity;
use pylon::items;
use pylon::lease::Dedicated;
use pylon::provider::{Config, Provider};
use pylon::verify;
use serde_json::json;

fn served() -> Served {
    Served {
        model: "clef-flash".into(),
        artifact_digest: Some(format!("sha256:{}", "cd".repeat(32))),
        model_id: None,
    }
}

/// The digest a round's receipt names for `batch`.
fn request_digest(batch: &[items::Item]) -> String {
    let body = json!(
        batch
            .iter()
            .map(|i| i.body("clef-flash"))
            .collect::<Vec<_>>()
    );
    sha256_hex(body.to_string().as_bytes())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_watcher_checks_new_pylons_privately_and_on_schedule() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let pylon_key = Identity::generate();
    let checker = Checker {
        relay: url.clone(),
        checker: Identity::generate(),
        buyer: Identity::generate(),
        home: home.path().join("checker"),
        wait: Duration::from_secs(10),
    };
    // A community pylon admits any key, so a fresh buyer key per round
    // reaches it.
    let mut config = Config::new(&url, "test-watch", home.path().join("pylon"));
    config.allow = None;
    config.rate_per_minute = 600;
    let provider = Provider::deciding(
        config,
        pylon_key.clone(),
        None,
        Arc::new(Fixed { identity: served() }),
        Arc::new(Dedicated),
    )
    .unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(Arc::clone(&provider).run(async {
        let _ = stop_rx.await;
    }));
    let mut online = None;
    for _ in 0..100 {
        online = hub
            .lock()
            .await
            .stored
            .iter()
            .find(|e| e.kind == 30_200)
            .cloned();
        if online.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // A second pylon serving the same model whose beacon says it is
    // offline: the watcher must leave it alone.
    let gone_key = Identity::generate();
    let mut gone = parse_beacon(&online.expect("the pylon's beacon")).unwrap();
    gone.provider = gone_key.pubkey().to_string();
    gone.pylon = "gone".into();
    gone.label = "gone".into();
    gone.status = Status::Offline;
    let event = beacon_event(gone_key.signer(), &gone).unwrap();
    let mut conn = pylon::relay::connect(&url, &gone_key, Duration::from_secs(10))
        .await
        .unwrap();
    pylon::relay::publish(&mut conn, &event).await.unwrap();
    let _ = conn.close().await;

    let secret = items::checker_seed(&checker.home).unwrap();
    let mut watcher = Watcher::new(
        checker.clone(),
        Arc::new(Fixed { identity: served() }),
        verify::CLEF_FLASH,
        WatchConfig::default(),
        Items::Private(secret),
    );
    let t0 = pylon::now();

    // First sight: one round, on the online pylon only.
    let first = watcher.pass(t0).await.unwrap();
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0].pylon, pylon_key.pubkey());
    let one = first[0].check.clone().expect("a round");
    assert_eq!(one.verdict, Verdict::Pass, "{one:?}");
    assert_eq!(one.items_from, "private");
    assert_eq!(one.private_round, Some(0));
    assert!(one.seed.is_none(), "a private round's seed stays private");
    assert_eq!(first[0].next, t0 + 900);

    // Not again before the interval.
    assert!(watcher.pass(t0 + 60).await.unwrap().is_empty());

    // Again after it.
    let second = watcher.pass(t0 + 900).await.unwrap();
    assert_eq!(second.len(), 1, "{second:?}");
    let two = second[0].check.clone().expect("a round");
    assert_eq!(two.verdict, Verdict::Pass, "{two:?}");
    assert_eq!(two.private_round, Some(1));

    // Each round came from its own new buyer key, never the stored one.
    assert_ne!(one.buyer, two.buyer);
    assert_ne!(one.buyer, checker.buyer.pubkey());
    assert_ne!(two.buyer, checker.buyer.pubkey());

    // The receipts are published under those keys, and their items are
    // the private rounds' items, not the public generator's for the same
    // round numbers.
    let stored = hub.lock().await.stored.clone();
    for (check, round) in [(&one, 0_u64), (&two, 1_u64)] {
        let receipt = stored
            .iter()
            .find(|e| Some(&e.id) == check.receipt.as_ref())
            .expect("the round's receipt");
        assert_eq!(receipt.pubkey, check.buyer);
        let body = parse_receipt(receipt, None).unwrap();
        assert_eq!(body.buyer, check.buyer);
        assert_eq!(
            body.request_digest,
            request_digest(&items::private_items(&secret, round, 8))
        );
        assert_ne!(body.request_digest, request_digest(&items::items(round, 8)));
    }
    // The round counter survives a restart: the next private round is 2.
    assert_eq!(pylon::check_decide::next_round(&checker.home).unwrap(), 2);

    let _ = stop_tx.send(());
    let _ = running.await;
}
