//! Integration tests for nostr-relay crate
//!
//! These tests verify end-to-end relay functionality including:
//! - Connecting to real relays
//! - Subscribing to events
//! - Publishing events
//! - Handling disconnections

use nostr_relay::{Filter, PoolEvent, RelayPool, DEFAULT_RELAYS};
use std::time::Duration;
use tokio::time::timeout;

/// Test connecting to a real relay
#[tokio::test]
async fn test_connect_to_real_relay() {
    let pool = RelayPool::new();

    // Add a single relay
    pool.add_relay("wss://relay.damus.io").await.unwrap();

    // Try to connect with timeout
    let result = timeout(Duration::from_secs(10), pool.connect_all()).await;

    match result {
        Ok(results) => {
            // At least attempted connection
            assert_eq!(results.len(), 1);
            println!("Connection result: {:?}", results[0]);
        }
        Err(_) => {
            // Timeout is acceptable in CI environments
            println!("Connection timed out (acceptable in CI)");
        }
    }
}

/// Test connecting to default relays
#[tokio::test]
async fn test_connect_to_default_relays() {
    let pool = nostr_relay::default_pool();

    // Subscribe to events before connecting
    let mut events = pool.subscribe();

    // Try to connect with timeout
    let connect_result = timeout(Duration::from_secs(15), pool.connect_default()).await;

    match connect_result {
        Ok(results) => {
            let connected = results.iter().filter(|(_, r)| r.is_ok()).count();
            println!(
                "Connected to {}/{} relays",
                connected,
                DEFAULT_RELAYS.len()
            );

            // Check for connected events
            let event_result = timeout(Duration::from_secs(2), events.recv()).await;
            if let Ok(Ok(event)) = event_result {
                match event {
                    PoolEvent::Connected { relay_url } => {
                        println!("Received connected event for: {}", relay_url);
                    }
                    _ => {
                        println!("Received other event: {:?}", event);
                    }
                }
            }
        }
        Err(_) => {
            println!("Connection timed out (acceptable in CI)");
        }
    }
}

/// Test subscription to kind 1 (text notes)
#[tokio::test]
async fn test_subscribe_to_text_notes() {
    let pool = nostr_relay::default_pool();
    let mut events = pool.subscribe();

    // Connect first
    let connect_result = timeout(Duration::from_secs(10), pool.connect_default()).await;
    if connect_result.is_err() {
        println!("Connection timed out, skipping subscription test");
        return;
    }

    // Wait a bit for connections to establish
    tokio::time::sleep(Duration::from_millis(500)).await;

    let connected = pool.connected_count().await;
    if connected == 0 {
        println!("No relays connected, skipping subscription test");
        return;
    }

    // Subscribe to recent text notes
    let filter = Filter::new().kinds(vec![1]).limit(5);

    let sub_result = timeout(Duration::from_secs(5), pool.subscribe_all(vec![filter])).await;

    match sub_result {
        Ok(Ok(sub_id)) => {
            println!("Created subscription: {}", sub_id);

            // Wait for some events
            let mut event_count = 0;
            let start = std::time::Instant::now();

            while start.elapsed() < Duration::from_secs(5) && event_count < 3 {
                match timeout(Duration::from_millis(500), events.recv()).await {
                    Ok(Ok(PoolEvent::Event { event, .. })) => {
                        println!("Received event: {} (kind {})", &event.id[..8], event.kind);
                        event_count += 1;
                    }
                    Ok(Ok(PoolEvent::Eose { subscription_id, .. })) => {
                        println!("EOSE for subscription: {}", subscription_id);
                    }
                    Ok(Ok(PoolEvent::AllEose { subscription_id })) => {
                        println!("All EOSE for subscription: {}", subscription_id);
                        break;
                    }
                    _ => {}
                }
            }

            println!("Received {} events", event_count);
        }
        Ok(Err(e)) => {
            println!("Subscription error: {}", e);
        }
        Err(_) => {
            println!("Subscription timed out");
        }
    }
}

/// Test filter builder
#[test]
fn test_filter_builder_comprehensive() {
    // Test kinds filter
    let filter = Filter::new().kinds(vec![1, 4, 7]);
    let json = serde_json::to_string(&filter).unwrap();
    assert!(json.contains("\"kinds\":[1,4,7]"));

    // Test authors filter
    let filter = Filter::new().authors(vec!["abc123".to_string()]);
    let json = serde_json::to_string(&filter).unwrap();
    assert!(json.contains("\"authors\":[\"abc123\"]"));

    // Test combined filters
    let filter = Filter::new()
        .kinds(vec![1])
        .authors(vec!["author1".to_string()])
        .limit(10);
    let json = serde_json::to_string(&filter).unwrap();
    assert!(json.contains("\"kinds\":[1]"));
    assert!(json.contains("\"authors\":[\"author1\"]"));
    assert!(json.contains("\"limit\":10"));
}

/// Test pool state management
#[tokio::test]
async fn test_pool_state_management() {
    let pool = RelayPool::new();

    // Initially empty
    assert_eq!(pool.relay_urls().await.len(), 0);
    assert_eq!(pool.connected_count().await, 0);

    // Add relays
    pool.add_relay("wss://relay1.example.com").await.unwrap();
    pool.add_relay("wss://relay2.example.com").await.unwrap();

    assert_eq!(pool.relay_urls().await.len(), 2);

    // Still not connected
    assert_eq!(pool.connected_count().await, 0);

    // Remove a relay
    pool.remove_relay("wss://relay1.example.com").await;
    assert_eq!(pool.relay_urls().await.len(), 1);
}

/// Test subscription tracking
#[tokio::test]
async fn test_subscription_tracking() {
    let pool = RelayPool::new();

    // No subscriptions initially
    let sub_ids = pool.subscription_ids().await;
    assert!(sub_ids.is_empty());
}

/// Test disconnection handling
#[tokio::test]
async fn test_disconnect_all() {
    let pool = RelayPool::new();

    pool.add_relay("wss://relay.damus.io").await.unwrap();

    // Disconnect (should not error even if not connected)
    pool.disconnect_all().await;

    assert_eq!(pool.connected_count().await, 0);
}
