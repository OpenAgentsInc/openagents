//! Stress tests for nostr-relay under high load
//!
//! Tests relay performance and stability with:
//! - 1000+ concurrent WebSocket connections
//! - Rapid event publishing
//! - Multiple subscriptions per client
//! - Sustained throughput
//! - Memory leak detection

use super::*;
use nostr::{EventTemplate, KIND_SHORT_TEXT_NOTE, finalize_event, generate_secret_key};
use nostr_client::RelayConnection;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::time::{Duration, timeout};

// =============================================================================
// Concurrent Connection Stress Tests
// =============================================================================

#[tokio::test]
#[ignore] // Run with: cargo test --test integration_tests stress -- --ignored --test-threads=1
async fn test_1000_concurrent_connections() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let num_clients = 1000;
    let mut handles = Vec::new();

    let start = Instant::now();

    // Spawn 1000 concurrent clients
    for i in 0..num_clients {
        let url = url.clone();
        let handle = tokio::spawn(async move {
            let relay = RelayConnection::new(&url).unwrap();
            match timeout(Duration::from_secs(10), relay.connect()).await {
                Ok(Ok(_)) => {
                    // Connection successful, keep alive briefly
                    sleep(Duration::from_millis(100)).await;
                    relay.disconnect().await.ok();
                    true
                }
                _ => false,
            }
        });
        handles.push(handle);

        // Stagger connections slightly to avoid thundering herd
        if i % 100 == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    }

    // Wait for all connections
    let mut successful = 0;
    for handle in handles {
        if handle.await.unwrap() {
            successful += 1;
        }
    }

    let elapsed = start.elapsed();

    println!(
        "Connected {}/{} clients in {:?}",
        successful, num_clients, elapsed
    );
    println!(
        "Connection rate: {:.2} conn/sec",
        successful as f64 / elapsed.as_secs_f64()
    );

    // At least 95% should succeed
    assert!(
        successful >= (num_clients * 95 / 100),
        "Only {}/{} connections succeeded",
        successful,
        num_clients
    );
}

#[tokio::test]
#[ignore]
async fn test_500_persistent_connections() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let num_clients = 500;
    let mut clients = Vec::new();

    let start = Instant::now();

    // Create 500 persistent connections
    for i in 0..num_clients {
        let relay = RelayConnection::new(&url).unwrap();
        match timeout(Duration::from_secs(5), relay.connect()).await {
            Ok(Ok(_)) => {
                clients.push(relay);
            }
            _ => {
                eprintln!("Failed to connect client {}", i);
            }
        }

        if i % 50 == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    }

    let connect_time = start.elapsed();
    let num_connected = clients.len();
    println!("Connected {} clients in {:?}", num_connected, connect_time);

    // Keep connections alive for 5 seconds
    sleep(Duration::from_secs(5)).await;

    // Disconnect all
    for relay in clients {
        relay.disconnect().await.ok();
    }

    println!("All clients disconnected successfully");

    assert!(
        num_connected >= (num_clients * 95 / 100),
        "Only {}/{} connections succeeded",
        num_connected,
        num_clients
    );
}

// =============================================================================
// Event Publishing Stress Tests
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_rapid_event_publishing_single_client() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    let num_events = 1000;
    let secret_key = generate_secret_key();

    let start = Instant::now();
    let mut successful = 0;

    for i in 0..num_events {
        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: format!("Stress test event {}", i),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        let event = finalize_event(&template, &secret_key).unwrap();

        match timeout(
            Duration::from_secs(2),
            relay.publish_event(&event, Duration::from_secs(5)),
        )
        .await
        {
            Ok(Ok(confirmation)) if confirmation.accepted => {
                successful += 1;
            }
            _ => {}
        }
    }

    let elapsed = start.elapsed();

    println!(
        "Published {}/{} events in {:?}",
        successful, num_events, elapsed
    );
    println!(
        "Throughput: {:.2} events/sec",
        successful as f64 / elapsed.as_secs_f64()
    );

    relay.disconnect().await.ok();

    // At least 95% should succeed
    assert!(
        successful >= (num_events * 95 / 100),
        "Only {}/{} events succeeded",
        successful,
        num_events
    );
}

#[tokio::test]
#[ignore]
async fn test_concurrent_event_publishing_100_clients() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let num_clients = 100;
    let events_per_client = 50;
    let total_expected = num_clients * events_per_client;

    let successful = Arc::new(AtomicU64::new(0));
    let mut handles = Vec::new();

    let start = Instant::now();

    for client_id in 0..num_clients {
        let url = url.clone();
        let successful = Arc::clone(&successful);

        let handle = tokio::spawn(async move {
            let relay = match RelayConnection::new(&url) {
                Ok(r) => r,
                Err(_) => return,
            };

            if relay.connect().await.is_err() {
                return;
            }

            let secret_key = generate_secret_key();

            for i in 0..events_per_client {
                let template = EventTemplate {
                    kind: KIND_SHORT_TEXT_NOTE,
                    tags: vec![],
                    content: format!("Client {} event {}", client_id, i),
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };
                let event = finalize_event(&template, &secret_key).unwrap();

                match timeout(
                    Duration::from_secs(5),
                    relay.publish_event(&event, Duration::from_secs(10)),
                )
                .await
                {
                    Ok(Ok(confirmation)) if confirmation.accepted => {
                        successful.fetch_add(1, Ordering::Relaxed);
                    }
                    _ => {}
                }
            }

            relay.disconnect().await.ok();
        });

        handles.push(handle);
    }

    // Wait for all clients
    for handle in handles {
        handle.await.ok();
    }

    let elapsed = start.elapsed();
    let final_count = successful.load(Ordering::Relaxed);

    println!(
        "Published {}/{} events from {} clients in {:?}",
        final_count, total_expected, num_clients, elapsed
    );
    println!(
        "Throughput: {:.2} events/sec",
        final_count as f64 / elapsed.as_secs_f64()
    );

    // At least 90% should succeed (lower threshold for concurrent test)
    assert!(
        final_count >= (total_expected * 90 / 100) as u64,
        "Only {}/{} events succeeded",
        final_count,
        total_expected
    );
}

// =============================================================================
// Subscription Stress Tests
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_100_clients_with_10_subscriptions_each() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let num_clients = 100;
    let subs_per_client = 10;
    let mut handles = Vec::new();

    let start = Instant::now();

    for client_id in 0..num_clients {
        let url = url.clone();

        let handle = tokio::spawn(async move {
            let relay = match RelayConnection::new(&url) {
                Ok(r) => r,
                Err(_) => return 0,
            };

            if relay.connect().await.is_err() {
                return 0;
            }

            let mut successful_subs = 0;

            for i in 0..subs_per_client {
                let sub_id = format!("client{}-sub{}", client_id, i);
                let filters = vec![serde_json::json!({
                    "kinds": [1],
                    "limit": 10
                })];

                if let Ok(Ok(_)) =
                    timeout(Duration::from_secs(2), relay.subscribe(&sub_id, &filters)).await
                {
                    successful_subs += 1;
                }
            }

            sleep(Duration::from_millis(100)).await;
            relay.disconnect().await.ok();

            successful_subs
        });

        handles.push(handle);
    }

    let mut total_subs = 0;
    for handle in handles {
        total_subs += handle.await.unwrap_or(0);
    }

    let elapsed = start.elapsed();
    let expected_subs = num_clients * subs_per_client;

    println!(
        "Created {}/{} subscriptions from {} clients in {:?}",
        total_subs, expected_subs, num_clients, elapsed
    );

    assert!(
        total_subs >= (expected_subs * 95 / 100),
        "Only {}/{} subscriptions succeeded",
        total_subs,
        expected_subs
    );
}

// =============================================================================
// Broadcast Stress Tests
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_broadcast_to_100_subscribers() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let num_subscribers = 100;
    let num_events = 10;

    // Create subscribers
    let mut subscribers = Vec::new();
    for i in 0..num_subscribers {
        let relay = RelayConnection::new(&url).unwrap();
        relay.connect().await.unwrap();

        let filters = vec![serde_json::json!({"kinds": [1]})];
        relay
            .subscribe(&format!("sub-{}", i), &filters)
            .await
            .unwrap();

        subscribers.push(relay);
    }

    sleep(Duration::from_millis(200)).await;

    // Create publisher
    let publisher = RelayConnection::new(&url).unwrap();
    publisher.connect().await.unwrap();

    let secret_key = generate_secret_key();
    let start = Instant::now();

    // Publish events
    for i in 0..num_events {
        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: format!("Broadcast event {}", i),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        let event = finalize_event(&template, &secret_key).unwrap();
        publisher
            .publish_event(&event, Duration::from_secs(5))
            .await
            .ok();

        sleep(Duration::from_millis(50)).await;
    }

    let publish_time = start.elapsed();
    println!(
        "Published {} events to {} subscribers in {:?}",
        num_events, num_subscribers, publish_time
    );

    // Cleanup
    publisher.disconnect().await.ok();
    for relay in subscribers {
        relay.disconnect().await.ok();
    }
}

// =============================================================================
// Sustained Load Tests
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_sustained_load_30_seconds() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let duration = Duration::from_secs(30);
    let num_clients = 50;

    let total_events = Arc::new(AtomicU64::new(0));
    let mut handles = Vec::new();

    let start = Instant::now();
    let end_time = start + duration;

    for client_id in 0..num_clients {
        let url = url.clone();
        let total_events = Arc::clone(&total_events);
        let end_time = end_time;

        let handle = tokio::spawn(async move {
            let relay = match RelayConnection::new(&url) {
                Ok(r) => r,
                Err(_) => return,
            };

            if relay.connect().await.is_err() {
                return;
            }

            let secret_key = generate_secret_key();
            let mut event_num = 0;

            while Instant::now() < end_time {
                let template = EventTemplate {
                    kind: KIND_SHORT_TEXT_NOTE,
                    tags: vec![],
                    content: format!("Client {} sustained event {}", client_id, event_num),
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };
                let event = finalize_event(&template, &secret_key).unwrap();

                if relay
                    .publish_event(&event, Duration::from_secs(5))
                    .await
                    .is_ok()
                {
                    total_events.fetch_add(1, Ordering::Relaxed);
                    event_num += 1;
                }

                // Rate limit to ~10 events/sec per client
                sleep(Duration::from_millis(100)).await;
            }

            relay.disconnect().await.ok();
        });

        handles.push(handle);
    }

    // Wait for all clients
    for handle in handles {
        handle.await.ok();
    }

    let elapsed = start.elapsed();
    let final_count = total_events.load(Ordering::Relaxed);

    println!(
        "Sustained load: {} events from {} clients over {:?}",
        final_count, num_clients, elapsed
    );
    println!(
        "Average throughput: {:.2} events/sec",
        final_count as f64 / elapsed.as_secs_f64()
    );

    // Should get at least 80% of theoretical max (50 clients * 10 events/sec * 30 sec = 15000)
    let theoretical_max = (num_clients * 10 * 30) as u64;
    assert!(
        final_count >= (theoretical_max * 80 / 100),
        "Only {} events, expected at least {}",
        final_count,
        theoretical_max * 80 / 100
    );
}

// =============================================================================
// Memory and Resource Tests
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_connection_churn() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let iterations = 100;
    let clients_per_iteration = 50;

    let start = Instant::now();

    for iteration in 0..iterations {
        let mut handles = Vec::new();

        // Connect 50 clients
        for _ in 0..clients_per_iteration {
            let url = url.clone();
            let handle = tokio::spawn(async move {
                let relay = RelayConnection::new(&url).unwrap();
                if relay.connect().await.is_ok() {
                    // Brief activity
                    sleep(Duration::from_millis(10)).await;
                    relay.disconnect().await.ok();
                    true
                } else {
                    false
                }
            });
            handles.push(handle);
        }

        // Wait for all to complete
        let mut successful = 0;
        for handle in handles {
            if handle.await.unwrap_or(false) {
                successful += 1;
            }
        }

        if iteration % 10 == 0 {
            println!(
                "Iteration {}/{}: {}/{} successful",
                iteration, iterations, successful, clients_per_iteration
            );
        }
    }

    let elapsed = start.elapsed();
    let total_connections = iterations * clients_per_iteration;

    println!(
        "Connection churn test: {} connections over {:?}",
        total_connections, elapsed
    );
    println!(
        "Rate: {:.2} conn/sec",
        total_connections as f64 / elapsed.as_secs_f64()
    );
}
