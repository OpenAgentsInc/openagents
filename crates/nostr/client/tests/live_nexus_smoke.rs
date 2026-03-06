#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nostr::regenerate_identity;
use nostr_client::{RelayAuthIdentity, RelayConfig, RelayConnection, RelayMessage, Subscription};
use serde_json::json;

fn live_nexus_ws_url() -> String {
    std::env::var("OPENAGENTS_LIVE_NEXUS_WS_URL")
        .unwrap_or_else(|_| "wss://nexus.openagents.com/".to_string())
}

fn unique_smoke_suffix() -> String {
    format!(
        "{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_millis()
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "hits live nexus.openagents.com"]
async fn live_nexus_relay_auth_and_subscription_smoke() {
    let identity = regenerate_identity().expect("generate live smoke nostr identity");
    let relay = RelayConnection::with_config(
        live_nexus_ws_url().as_str(),
        RelayConfig {
            connect_timeout: Duration::from_secs(10),
            nip42_identity: Some(RelayAuthIdentity {
                private_key_hex: identity.private_key_hex.clone(),
            }),
        },
    )
    .expect("create live relay connection");

    relay.connect().await.expect("connect to live nexus relay");
    relay
        .subscribe(Subscription::new(
            format!("live-smoke-{}", unique_smoke_suffix()),
            vec![json!({
                "authors": [identity.public_key_hex.clone()],
                "limit": 1
            })],
        ))
        .await
        .expect("subscribe to live nexus relay");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut saw_auth = false;
    let mut saw_eose = false;

    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = tokio::time::timeout(remaining, relay.recv())
            .await
            .expect("wait for live relay message")
            .expect("relay recv result")
            .expect("relay message");
        match message {
            RelayMessage::Auth(_) => saw_auth = true,
            RelayMessage::Eose(_) => {
                saw_eose = true;
                break;
            }
            RelayMessage::Notice(notice) => {
                panic!("unexpected live relay notice: {notice}");
            }
            _ => {}
        }
    }

    let _ = relay.disconnect().await;

    assert!(saw_auth, "expected live nexus relay to issue a NIP-42 auth challenge");
    assert!(
        saw_eose,
        "expected authenticated live nexus relay subscription to terminate with EOSE"
    );
}
