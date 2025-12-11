//! Live Nostr smoke tests against public relays
//!
//! Run with: `cargo test --features "net-executor,nostr" -p oanix -- --ignored nostr_live`

use crate::fixtures::{wait_for_nostr_sent, ExecutorTestFixture};
use nostr::EventTemplate;
use oanix::executor::ExecutorConfig;
use std::time::Duration;

fn live_test_config() -> ExecutorConfig {
    ExecutorConfig::builder()
        .poll_interval(Duration::from_millis(50))
        .ws_connect_timeout(Duration::from_secs(10))
        .ws_ping_interval(Duration::from_secs(30))
        .build()
}

/// Public Nostr relays to test against
const RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
];

/// Live test: Connect to real Nostr relays and send ephemeral event
#[tokio::test]
#[ignore]
async fn test_nostr_live_relay_connection() {
    let mut fixture = ExecutorTestFixture::new(live_test_config());

    // Add multiple relays
    for relay in RELAYS {
        fixture.nostr_fs.add_relay(*relay);
        println!("Added relay: {}", relay);
    }

    fixture.start().unwrap();

    // Wait for at least one relay connection
    println!("Waiting for relay connections...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Send a test event using ephemeral kind 20001 to avoid polluting relays
    // Ephemeral events (20000-29999) are not stored by relays
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let template = EventTemplate {
        created_at,
        kind: 20001, // Ephemeral kind - won't be stored
        tags: vec![],
        content: "OANIX E2E test - please ignore (ephemeral)".to_string(),
    };

    let event = fixture.nostr_fs.sign_event(template).unwrap();
    println!("Created event: {}", event.id);

    // Wait for event to be sent (30 second timeout for slow relays)
    if wait_for_nostr_sent(&fixture.nostr_fs, &event.id, Duration::from_secs(30)).await {
        let sent_to = fixture.nostr_fs.sent_to(&event.id);
        println!("Event sent to {} relays: {:?}", sent_to.len(), sent_to);
        assert!(
            !sent_to.is_empty(),
            "Event should be sent to at least one relay"
        );
        println!("Live Nostr test passed!");
    } else {
        // Don't fail hard - network issues are expected in CI
        println!("WARNING: Could not confirm event delivery to any relay");
        println!("This may be due to network issues or relay rate limiting");
    }

    fixture.shutdown().unwrap();
}

/// Live test: Nostr subscription to real relay
#[tokio::test]
#[ignore]
async fn test_nostr_live_subscription() {
    use oanix::services::Filter;

    let mut fixture = ExecutorTestFixture::new(live_test_config());

    // Just use one relay for subscription test
    fixture.nostr_fs.add_relay("wss://relay.damus.io");

    // Subscribe to recent kind 1 (text notes)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let filter = Filter {
        kinds: Some(vec![1]),
        since: Some(now - 300), // Last 5 minutes
        limit: Some(5),         // Just 5 events
        ..Default::default()
    };
    fixture
        .nostr_fs
        .add_subscription("live-test".to_string(), vec![filter]);

    fixture.start().unwrap();

    // Wait for subscription to be set up and events to arrive
    println!("Waiting for events from relay...");
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Check inbox for received events
    let inbox = fixture.nostr_fs.inbox_events();
    println!("Received {} events in inbox", inbox.len());

    if !inbox.is_empty() {
        println!("Live Nostr subscription test passed!");
        for event in inbox.iter().take(3) {
            println!("  - Event {}: {} chars", event.id, event.content.len());
        }
    } else {
        println!("No events received - relay may be slow or have no recent traffic");
        // Don't fail - this is expected in some scenarios
    }

    fixture.shutdown().unwrap();
}

/// Live test: Connect to multiple relays simultaneously
#[tokio::test]
#[ignore]
async fn test_nostr_live_multi_relay() {
    let mut fixture = ExecutorTestFixture::new(live_test_config());

    // Add all relays
    for relay in RELAYS {
        fixture.nostr_fs.add_relay(*relay);
    }

    fixture.start().unwrap();

    // Wait for connections
    println!("Connecting to {} relays...", RELAYS.len());
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Send a test event
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let template = EventTemplate {
        created_at,
        kind: 20001, // Ephemeral
        tags: vec![],
        content: "OANIX multi-relay test".to_string(),
    };

    let event = fixture.nostr_fs.sign_event(template).unwrap();

    // Wait for sends
    tokio::time::sleep(Duration::from_secs(10)).await;

    let sent_to = fixture.nostr_fs.sent_to(&event.id);
    println!(
        "Event sent to {} of {} relays: {:?}",
        sent_to.len(),
        RELAYS.len(),
        sent_to
    );

    if sent_to.len() > 1 {
        println!("Live multi-relay test passed!");
    } else {
        println!("Only {} relays confirmed - network may be slow", sent_to.len());
    }

    fixture.shutdown().unwrap();
}
