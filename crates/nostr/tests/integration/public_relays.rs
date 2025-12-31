//! Public relay compatibility tests (ignored by default).

use super::*;
use nostr_client::{PoolConfig, RelayConnection, RelayMessage, RelayPool};
use serde_json::json;
use tokio::time::{Duration, timeout};

const PUBLIC_RELAYS: [&str; 2] = ["wss://relay.damus.io", "wss://nos.lol"];

async fn wait_for_eose(relay: &RelayConnection, timeout_secs: u64) -> Option<String> {
    let result = timeout(Duration::from_secs(timeout_secs), async {
        loop {
            match relay.recv().await {
                Ok(Some(RelayMessage::Eose(sub_id))) => return Some(sub_id),
                Ok(Some(_)) => continue,
                Ok(None) => return None,
                Err(_) => return None,
            }
        }
    })
    .await;

    result.ok().flatten()
}

#[tokio::test]
#[ignore]
async fn test_public_relays_eose() {
    for url in PUBLIC_RELAYS {
        let relay = RelayConnection::new(url).expect("relay connection");
        relay.connect().await.expect("connect");

        let filters = vec![json!({"kinds": [1], "limit": 1})];
        relay
            .subscribe("public-eose", &filters)
            .await
            .expect("subscribe");

        let eose = wait_for_eose(&relay, 6).await;
        assert!(eose.is_some(), "{} should send EOSE", url);

        relay.disconnect().await.ok();
    }
}

#[tokio::test]
#[ignore]
async fn test_public_relay_pool_subscription() {
    let pool = RelayPool::new(PoolConfig::default());
    for url in PUBLIC_RELAYS {
        pool.add_relay(url).await.expect("add relay");
    }
    pool.connect_all().await.expect("connect pool");

    let filters = vec![json!({"kinds": [1], "limit": 1})];
    let mut rx = pool
        .subscribe("public-pool", &filters)
        .await
        .expect("subscribe");

    let event = timeout(Duration::from_secs(6), rx.recv())
        .await
        .ok()
        .flatten();
    assert!(event.is_some(), "should receive at least one event");

    pool.unsubscribe("public-pool").await.ok();
}
