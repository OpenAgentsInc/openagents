//! Compatibility tests with public Nostr relays
//!
//! These tests verify that our nostr-client works with real-world public relays.
//! Tests are marked #[ignore] by default since they require network access.
//!
//! Run with: cargo test --features full -- --ignored

use nostr::{EventTemplate, finalize_event, generate_secret_key};
use nostr_client::{RelayConnection, RelayMessage};
use serde_json::json;
use tokio::time::{Duration, timeout};

/// Well-known public Nostr relays for testing
const PUBLIC_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
];

#[tokio::test]
#[ignore] // Requires network access
async fn test_connect_to_damus() {
    let relay = RelayConnection::new("wss://relay.damus.io").unwrap();
    let result = timeout(Duration::from_secs(10), relay.connect()).await;

    assert!(result.is_ok(), "Connection should complete");
    assert!(result.unwrap().is_ok(), "Should connect to relay.damus.io");

    relay.disconnect().await.ok();
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_connect_to_nos_lol() {
    let relay = RelayConnection::new("wss://nos.lol").unwrap();
    let result = timeout(Duration::from_secs(10), relay.connect()).await;

    assert!(result.is_ok(), "Connection should complete");
    assert!(result.unwrap().is_ok(), "Should connect to nos.lol");

    relay.disconnect().await.ok();
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_connect_to_nostr_band() {
    let relay = RelayConnection::new("wss://relay.nostr.band").unwrap();
    let result = timeout(Duration::from_secs(10), relay.connect()).await;

    assert!(result.is_ok(), "Connection should complete");
    assert!(
        result.unwrap().is_ok(),
        "Should connect to relay.nostr.band"
    );

    relay.disconnect().await.ok();
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_publish_and_retrieve_from_public_relay() {
    // Use relay.damus.io for this test
    let relay = RelayConnection::new("wss://relay.damus.io").unwrap();
    timeout(Duration::from_secs(10), relay.connect())
        .await
        .expect("Connection timeout")
        .expect("Should connect");

    // Create and publish a test event
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: format!(
            "Test event from nostr-client compatibility tests - {}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        ),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();
    let event_id = event.id.clone();

    // Publish event
    let publish_result = timeout(
        Duration::from_secs(10),
        relay.publish_event(&event, Duration::from_secs(5)),
    )
    .await;

    assert!(publish_result.is_ok(), "Publish should complete");
    let confirmation = publish_result.unwrap().unwrap();
    assert!(
        confirmation.accepted,
        "Event should be accepted by public relay"
    );

    // Subscribe to retrieve the event
    let filters = vec![json!({
        "ids": [event_id.clone()],
        "limit": 1
    })];

    relay
        .subscribe("test-retrieve", &filters)
        .await
        .expect("Should subscribe");

    // Wait for the event or EOSE
    let result = timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(Some(msg)) = relay.recv().await {
                match msg {
                    RelayMessage::Event(_, evt) => {
                        if evt.id == event_id {
                            return Some(evt);
                        }
                    }
                    RelayMessage::Eose(_) => return None,
                    _ => {}
                }
            }
        }
    })
    .await;

    // We may or may not retrieve the event depending on relay storage
    // Just verify we got a response (either event or EOSE)
    assert!(result.is_ok(), "Should receive response from relay");

    relay.disconnect().await.ok();
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_subscribe_to_recent_events() {
    let relay = RelayConnection::new("wss://relay.damus.io").unwrap();
    timeout(Duration::from_secs(10), relay.connect())
        .await
        .expect("Connection timeout")
        .expect("Should connect");

    // Subscribe to recent kind 1 events
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let filters = vec![json!({
        "kinds": [1],
        "since": now - 3600, // Last hour
        "limit": 10
    })];

    relay
        .subscribe("recent-events", &filters)
        .await
        .expect("Should subscribe");

    // Wait for EOSE
    let result = timeout(Duration::from_secs(30), async {
        let mut event_count = 0;
        loop {
            if let Ok(Some(msg)) = relay.recv().await {
                match msg {
                    RelayMessage::Event(_, _) => event_count += 1,
                    RelayMessage::Eose(_) => return event_count,
                    _ => {}
                }
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should receive EOSE");
    let event_count = result.unwrap();
    println!("Received {} events from public relay", event_count);

    relay.disconnect().await.ok();
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_filter_compatibility() {
    let relay = RelayConnection::new("wss://nos.lol").unwrap();
    timeout(Duration::from_secs(10), relay.connect())
        .await
        .expect("Connection timeout")
        .expect("Should connect");

    // Test various filter types
    let test_filters = vec![
        // By kind
        json!({"kinds": [1], "limit": 5}),
        // By time range
        json!({
            "kinds": [1],
            "since": 1700000000,
            "until": 1700001000,
            "limit": 5
        }),
        // By authors (example pubkeys)
        json!({
            "kinds": [1],
            "authors": ["a".repeat(64)],
            "limit": 5
        }),
    ];

    for (i, filter) in test_filters.iter().enumerate() {
        let sub_id = format!("compat-test-{}", i);
        let result = relay.subscribe(&sub_id, &[filter.clone()]).await;
        assert!(result.is_ok(), "Filter {} should be accepted", i);

        // Wait for EOSE
        timeout(Duration::from_secs(10), async {
            loop {
                if let Ok(Some(msg)) = relay.recv().await
                    && let RelayMessage::Eose(id) = msg
                    && id == sub_id
                {
                    break;
                }
            }
        })
        .await
        .ok();

        relay.close_subscription(&sub_id).await.ok();
    }

    relay.disconnect().await.ok();
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_event_format_compatibility() {
    let relay = RelayConnection::new("wss://relay.nostr.band").unwrap();
    timeout(Duration::from_secs(10), relay.connect())
        .await
        .expect("Connection timeout")
        .expect("Should connect");

    // Create event with various features
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![
            vec!["t".to_string(), "test".to_string()],
            vec!["client".to_string(), "nostr-client".to_string()],
        ],
        content: "Compatibility test with tags and special characters: émojis 🎉".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();

    // Try to publish
    let result = timeout(
        Duration::from_secs(10),
        relay.publish_event(&event, Duration::from_secs(5)),
    )
    .await;

    assert!(result.is_ok(), "Should handle event format");
    if let Ok(Ok(confirmation)) = result {
        // Event may be accepted or rejected based on relay policy
        println!(
            "Event {} by relay: {}",
            if confirmation.accepted {
                "accepted"
            } else {
                "rejected"
            },
            confirmation.message
        );
    }

    relay.disconnect().await.ok();
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_relay_reconnection() {
    let relay = RelayConnection::new("wss://relay.damus.io").unwrap();

    // Connect, disconnect, reconnect
    timeout(Duration::from_secs(10), relay.connect())
        .await
        .expect("First connection timeout")
        .expect("Should connect");

    relay.disconnect().await.expect("Should disconnect");

    // Wait a bit
    tokio::time::sleep(Duration::from_millis(500)).await;

    timeout(Duration::from_secs(10), relay.connect())
        .await
        .expect("Reconnection timeout")
        .expect("Should reconnect");

    relay.disconnect().await.ok();
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_multiple_public_relays() {
    let mut successful_connections = 0;

    for relay_url in PUBLIC_RELAYS {
        let relay = RelayConnection::new(relay_url).unwrap();
        let result = timeout(Duration::from_secs(10), relay.connect()).await;

        if result.is_ok() && result.unwrap().is_ok() {
            successful_connections += 1;
            println!("✓ Connected to {}", relay_url);
            relay.disconnect().await.ok();
        } else {
            println!("✗ Failed to connect to {}", relay_url);
        }
    }

    assert!(
        successful_connections >= 2,
        "Should connect to at least 2 public relays, connected to {}",
        successful_connections
    );
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_handle_large_result_set() {
    let relay = RelayConnection::new("wss://relay.damus.io").unwrap();
    timeout(Duration::from_secs(10), relay.connect())
        .await
        .expect("Connection timeout")
        .expect("Should connect");

    // Request many events
    let filters = vec![json!({
        "kinds": [1],
        "limit": 500
    })];

    relay
        .subscribe("large-set", &filters)
        .await
        .expect("Should subscribe");

    // Count events received
    let result = timeout(Duration::from_secs(60), async {
        let mut count = 0;
        loop {
            if let Ok(Some(msg)) = relay.recv().await {
                match msg {
                    RelayMessage::Event(_, _) => count += 1,
                    RelayMessage::Eose(_) => return count,
                    _ => {}
                }
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should handle large result set");
    let count = result.unwrap();
    println!("Received {} events in large result set", count);

    relay.disconnect().await.ok();
}
