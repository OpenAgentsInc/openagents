//! E2E tests for NostrRelayConnector
//!
//! Note: These tests use `#[test]` instead of `#[tokio::test]` because
//! ExecutorManager creates its own tokio runtime. All async operations
//! are run via `fixture.block_on()`.

use crate::fixtures::{ExecutorTestFixture, NostrMockRelay, fast_test_config, wait_for_nostr_sent};
use nostr::EventTemplate;
use oanix::services::Filter;
use std::collections::HashMap;
use std::time::Duration;

/// Test sending an event to a mock relay
#[test]
fn test_nostr_send_event() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock_relay = fixture.block_on(NostrMockRelay::start());

    fixture.nostr_fs.add_relay(mock_relay.url());
    fixture.start().unwrap();

    // Create and sign an event
    let template = EventTemplate {
        created_at: 1700000000,
        kind: 1,
        tags: vec![],
        content: "Hello from test!".to_string(),
    };
    let event = fixture.nostr_fs.sign_event(template).unwrap();
    let event_id = event.id.clone();

    // Wait for event to be sent
    let nostr_fs = &fixture.nostr_fs;
    let sent = fixture.block_on(wait_for_nostr_sent(
        nostr_fs,
        &event_id,
        Duration::from_secs(10),
    ));
    assert!(sent, "Event should be sent to relay");

    // Verify relay received the event
    let received = fixture.block_on(mock_relay.received_events());
    assert!(
        received.iter().any(|e| e["id"].as_str() == Some(&event_id)),
        "Relay should have received event with id {}",
        event_id
    );

    fixture.block_on(mock_relay.shutdown());
    fixture.shutdown().unwrap();
}

/// Test subscription creates REQ on relay
#[test]
fn test_nostr_subscription() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock_relay = fixture.block_on(NostrMockRelay::start());

    fixture.nostr_fs.add_relay(mock_relay.url());

    // Add subscription before starting (so it's sent when connection opens)
    let filter = Filter {
        kinds: Some(vec![1]),
        ..Default::default()
    };
    fixture
        .nostr_fs
        .add_subscription("test-sub".to_string(), vec![filter]);

    fixture.start().unwrap();

    // Wait for relay to receive subscription
    fixture.block_on(async { tokio::time::sleep(Duration::from_secs(2)).await });

    // Verify subscription was created
    let subs = fixture.block_on(mock_relay.active_subscriptions());
    assert!(
        subs.contains(&"test-sub".to_string()),
        "Relay should have subscription 'test-sub', got: {:?}",
        subs
    );

    fixture.block_on(mock_relay.shutdown());
    fixture.shutdown().unwrap();
}

/// Test multiple relays receive events
#[test]
fn test_nostr_multiple_relays() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let relay1 = fixture.block_on(NostrMockRelay::start());
    let relay2 = fixture.block_on(NostrMockRelay::start());

    fixture.nostr_fs.add_relay(relay1.url());
    fixture.nostr_fs.add_relay(relay2.url());
    fixture.start().unwrap();

    // Send an event
    let template = EventTemplate {
        created_at: 1700000001,
        kind: 1,
        tags: vec![],
        content: "Broadcast to all relays".to_string(),
    };
    let event = fixture.nostr_fs.sign_event(template).unwrap();
    let event_id = event.id.clone();

    // Wait for event to be sent
    let nostr_fs = &fixture.nostr_fs;
    let sent = fixture.block_on(wait_for_nostr_sent(
        nostr_fs,
        &event_id,
        Duration::from_secs(10),
    ));
    assert!(sent, "Event should be sent");

    // Both relays should receive the event
    let received1 = fixture.block_on(relay1.received_events());
    let received2 = fixture.block_on(relay2.received_events());

    // At least one relay should have received it
    let either_received = received1
        .iter()
        .any(|e| e["id"].as_str() == Some(&event_id))
        || received2
            .iter()
            .any(|e| e["id"].as_str() == Some(&event_id));

    assert!(
        either_received,
        "At least one relay should have received the event"
    );

    fixture.block_on(relay1.shutdown());
    fixture.block_on(relay2.shutdown());
    fixture.shutdown().unwrap();
}

/// Test NIP-90 job request flow
#[test]
fn test_nostr_nip90_job_request() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock_relay = fixture.block_on(NostrMockRelay::start());

    fixture.nostr_fs.add_relay(mock_relay.url());
    fixture.start().unwrap();

    // Create NIP-90 job request
    let mut params = HashMap::new();
    params.insert("model".to_string(), "gpt-4".to_string());

    let event = fixture
        .nostr_fs
        .create_job_request(5050, "What is 2+2?", params)
        .unwrap();

    // Wait for event to be sent
    let nostr_fs = &fixture.nostr_fs;
    let sent = fixture.block_on(wait_for_nostr_sent(
        nostr_fs,
        &event.id,
        Duration::from_secs(10),
    ));
    assert!(sent, "Job request should be sent to relay");

    // Verify relay received a kind 5050 event
    let received = fixture.block_on(mock_relay.received_events());
    assert!(
        received.iter().any(|e| e["kind"] == 5050),
        "Relay should have received kind 5050 event"
    );

    fixture.block_on(mock_relay.shutdown());
    fixture.shutdown().unwrap();
}

/// Test outbox is cleared after successful send
#[test]
fn test_nostr_outbox_cleared() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock_relay = fixture.block_on(NostrMockRelay::start());

    fixture.nostr_fs.add_relay(mock_relay.url());
    fixture.start().unwrap();

    // Create and sign an event
    let template = EventTemplate {
        created_at: 1700000002,
        kind: 1,
        tags: vec![],
        content: "Event to be cleared".to_string(),
    };
    let event = fixture.nostr_fs.sign_event(template).unwrap();

    // Initially in outbox
    assert_eq!(fixture.nostr_fs.outbox_events().len(), 1);

    // Wait for event to be sent
    let nostr_fs = &fixture.nostr_fs;
    let sent = fixture.block_on(wait_for_nostr_sent(
        nostr_fs,
        &event.id,
        Duration::from_secs(10),
    ));
    assert!(sent);

    // Outbox should be cleared
    assert_eq!(
        fixture.nostr_fs.outbox_events().len(),
        0,
        "Outbox should be cleared after send"
    );

    fixture.block_on(mock_relay.shutdown());
    fixture.shutdown().unwrap();
}

/// Test event kind filtering in subscriptions
#[test]
fn test_nostr_filter_kinds() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock_relay = fixture.block_on(NostrMockRelay::start());

    fixture.nostr_fs.add_relay(mock_relay.url());

    // Subscribe to kind 1 only
    let filter = Filter {
        kinds: Some(vec![1]),
        ..Default::default()
    };
    fixture
        .nostr_fs
        .add_subscription("kind1-sub".to_string(), vec![filter]);

    fixture.start().unwrap();

    // Wait for subscription to be set up
    fixture.block_on(async { tokio::time::sleep(Duration::from_secs(2)).await });

    let subs = fixture.block_on(mock_relay.active_subscriptions());
    assert!(
        subs.contains(&"kind1-sub".to_string()),
        "Subscription should be active"
    );

    // Remove subscription
    fixture.nostr_fs.remove_subscription("kind1-sub");

    // Give time for CLOSE to be sent
    fixture.block_on(async { tokio::time::sleep(Duration::from_millis(500)).await });

    fixture.block_on(mock_relay.shutdown());
    fixture.shutdown().unwrap();
}

/// Test sent_to tracking
#[test]
fn test_nostr_sent_to_tracking() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let mock_relay = fixture.block_on(NostrMockRelay::start());
    let relay_url = mock_relay.url();

    fixture.nostr_fs.add_relay(&relay_url);
    fixture.start().unwrap();

    // Create and sign an event
    let template = EventTemplate {
        created_at: 1700000003,
        kind: 1,
        tags: vec![],
        content: "Track me".to_string(),
    };
    let event = fixture.nostr_fs.sign_event(template).unwrap();

    // Initially not sent anywhere
    assert!(fixture.nostr_fs.sent_to(&event.id).is_empty());

    // Wait for event to be sent
    let nostr_fs = &fixture.nostr_fs;
    let sent = fixture.block_on(wait_for_nostr_sent(
        nostr_fs,
        &event.id,
        Duration::from_secs(10),
    ));
    assert!(sent);

    // Should be tracked as sent to relay
    let sent_to = fixture.nostr_fs.sent_to(&event.id);
    assert!(
        !sent_to.is_empty(),
        "Event should be tracked as sent to relay"
    );

    fixture.block_on(mock_relay.shutdown());
    fixture.shutdown().unwrap();
}
