//! NIP-01 protocol message compliance tests
//!
//! Tests for client-relay communication protocol messages:
//! - CLIENT -> RELAY: EVENT, REQ, CLOSE
//! - RELAY -> CLIENT: EVENT, OK, EOSE, CLOSED, NOTICE
//!
//! Validates message structure, formats, and error handling

use super::*;
use nostr::{EventTemplate, KIND_SHORT_TEXT_NOTE, finalize_event, generate_secret_key};
use nostr_client::{RelayConnection, RelayMessage};
use serde_json::json;
use tokio::time::{Duration, timeout};

// =============================================================================
// CLIENT -> RELAY Message Tests
// =============================================================================

#[tokio::test]
async fn test_event_message_structure() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Send valid EVENT message
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Test event message".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();

    let result = relay.publish_event(&event, Duration::from_secs(5)).await;
    assert!(result.is_ok(), "Valid EVENT message should succeed");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_req_message_structure() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Send valid REQ message
    let filters = vec![json!({"kinds": [1]})];
    let result = relay.subscribe("test-sub", &filters).await;

    assert!(result.is_ok(), "Valid REQ message should succeed");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_req_with_multiple_filters() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Multiple filters (OR condition)
    let filters = vec![json!({"kinds": [1]}), json!({"kinds": [2]})];

    let result = relay.subscribe("multi-filter", &filters).await;
    assert!(result.is_ok(), "REQ with multiple filters should succeed");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_close_message_structure() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Create subscription first
    let filters = vec![json!({"kinds": [1]})];
    relay.subscribe("close-test", &filters).await.unwrap();

    // Send CLOSE message
    let result = relay.close_subscription("close-test").await;
    assert!(result.is_ok(), "Valid CLOSE message should succeed");

    relay.disconnect().await.ok();
}

// =============================================================================
// RELAY -> CLIENT Message Tests
// =============================================================================

#[tokio::test]
async fn test_ok_message_format() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Publish event and wait for OK response
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Test OK message".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();
    let event_id = event.id.clone();

    let confirmation = relay
        .publish_event(&event, Duration::from_secs(5))
        .await
        .unwrap();

    // OK message should have correct event ID
    assert_eq!(confirmation.event_id, event_id);
    assert!(confirmation.accepted, "Event should be accepted");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_ok_message_rejection() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Create event with invalid signature
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Invalid event".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let mut event = finalize_event(&template, &secret_key).unwrap();

    // Corrupt signature
    event.sig = "0".repeat(128);

    let result = relay.publish_event(&event, Duration::from_secs(5)).await;

    // Should either error or return not accepted
    if let Ok(confirmation) = result {
        assert!(!confirmation.accepted, "Invalid event should be rejected");
        assert!(
            !confirmation.message.is_empty(),
            "Should have error message"
        );
    }

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_eose_message_received() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Subscribe
    let filters = vec![json!({"kinds": [1], "limit": 5})];
    relay.subscribe("eose-test", &filters).await.unwrap();

    // Wait for EOSE
    let result = timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay.recv().await
                && let RelayMessage::Eose(sub_id) = msg
            {
                return sub_id;
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should receive EOSE message");
    assert_eq!(
        result.unwrap(),
        "eose-test",
        "EOSE should have correct sub ID"
    );

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_event_message_with_subscription_id() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay1 = RelayConnection::new(&url).unwrap();
    let relay2 = RelayConnection::new(&url).unwrap();

    relay1.connect().await.unwrap();
    relay2.connect().await.unwrap();

    // Subscribe on relay1
    let filters = vec![json!({"kinds": [1]})];
    relay1.subscribe("my-sub", &filters).await.unwrap();

    sleep(Duration::from_millis(100)).await;

    // Publish from relay2
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Event with sub ID".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();
    let event_id = event.id.clone();

    relay2
        .publish_event(&event, Duration::from_secs(5))
        .await
        .unwrap();

    // Receive on relay1 - should have subscription ID
    let result = timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay1.recv().await
                && let RelayMessage::Event(sub_id, evt) = msg
                && evt.id == event_id
            {
                return Some((sub_id, evt));
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should receive EVENT message");
    let (sub_id, _) = result.unwrap().unwrap();
    assert_eq!(
        sub_id, "my-sub",
        "EVENT should have correct subscription ID"
    );

    relay1.disconnect().await.ok();
    relay2.disconnect().await.ok();
}

#[tokio::test]
async fn test_notice_message_handling() {
    // NOTICE messages are optional and implementation-specific
    // This test just ensures they don't break the connection if received
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Connection should remain stable even if NOTICE is sent
    let filters = vec![json!({"kinds": [1]})];
    let result = relay.subscribe("notice-test", &filters).await;

    assert!(result.is_ok(), "Should handle connection properly");

    relay.disconnect().await.ok();
}

// =============================================================================
// Filter Validation Tests
// =============================================================================

#[tokio::test]
async fn test_filter_ids_64_char_hex() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Valid 64-char hex
    let filters = vec![json!({
        "ids": ["a".repeat(64)]
    })];

    let result = relay.subscribe("hex-test", &filters).await;
    assert!(result.is_ok(), "Valid 64-char hex ID should work");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_filter_authors_64_char_hex() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Valid 64-char hex
    let filters = vec![json!({
        "authors": ["b".repeat(64)]
    })];

    let result = relay.subscribe("author-test", &filters).await;
    assert!(result.is_ok(), "Valid 64-char hex author should work");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_filter_tag_queries() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Single-letter tag filters
    let filters = vec![json!({
        "#e": ["event_id_here"],
        "#p": ["pubkey_here"],
        "#t": ["nostr"]
    })];

    let result = relay.subscribe("tag-test", &filters).await;
    assert!(result.is_ok(), "Tag filters should work");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_filter_since_until() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let filters = vec![json!({
        "since": now - 3600,  // Last hour
        "until": now
    })];

    let result = relay.subscribe("time-test", &filters).await;
    assert!(result.is_ok(), "Time range filters should work");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_filter_limit_positive() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    let filters = vec![json!({
        "kinds": [1],
        "limit": 10
    })];

    let result = relay.subscribe("limit-test", &filters).await;
    assert!(result.is_ok(), "Positive limit should work");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_filter_empty() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Empty filter (matches everything)
    let filters = vec![json!({})];

    let result = relay.subscribe("empty-filter", &filters).await;
    assert!(result.is_ok(), "Empty filter should work");

    relay.disconnect().await.ok();
}

// =============================================================================
// Subscription ID Validation Tests
// =============================================================================

#[tokio::test]
async fn test_subscription_id_max_length() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Exactly 64 characters (max allowed per NIP-01)
    let sub_id = "a".repeat(64);
    let filters = vec![json!({"kinds": [1]})];

    let result = relay.subscribe(&sub_id, &filters).await;
    assert!(result.is_ok(), "64-char subscription ID should work");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_subscription_id_special_characters() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Subscription IDs can contain various characters
    let sub_ids = vec!["test-sub-123", "test_sub_456", "test.sub.789", "TestSub"];

    for sub_id in sub_ids {
        let filters = vec![json!({"kinds": [1]})];
        let result = relay.subscribe(sub_id, &filters).await;
        assert!(result.is_ok(), "Subscription ID '{}' should work", sub_id);
    }

    relay.disconnect().await.ok();
}

// =============================================================================
// Message Ordering and Timing Tests
// =============================================================================

#[tokio::test]
async fn test_eose_before_new_events() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay1 = RelayConnection::new(&url).unwrap();
    let relay2 = RelayConnection::new(&url).unwrap();

    relay1.connect().await.unwrap();
    relay2.connect().await.unwrap();

    // Subscribe
    let filters = vec![json!({"kinds": [1]})];
    relay1.subscribe("order-test", &filters).await.unwrap();

    // Wait for EOSE
    let mut received_eose = false;
    timeout(Duration::from_secs(2), async {
        while !received_eose {
            if let Ok(Some(msg)) = relay1.recv().await
                && let RelayMessage::Eose(_) = msg
            {
                received_eose = true;
                break;
            }
        }
    })
    .await
    .ok();

    assert!(received_eose, "Should receive EOSE before new events");

    // Now publish new event
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "New event after EOSE".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();
    relay2
        .publish_event(&event, Duration::from_secs(5))
        .await
        .unwrap();

    // Should receive event after EOSE
    let result = timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay1.recv().await
                && let RelayMessage::Event(_, evt) = msg
            {
                return Some(evt);
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should receive new event after EOSE");

    relay1.disconnect().await.ok();
    relay2.disconnect().await.ok();
}

#[tokio::test]
async fn test_duplicate_event_handling() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Publish same event twice
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Duplicate test".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();

    // First publish
    let result1 = relay
        .publish_event(&event, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(result1.accepted, "First publish should be accepted");

    // Second publish (duplicate)
    let result2 = relay
        .publish_event(&event, Duration::from_secs(5))
        .await
        .unwrap();

    // Should either be accepted (idempotent) or provide duplicate message
    // Both behaviors are acceptable per NIP-01
    assert!(
        result2.accepted || result2.message.contains("duplicate"),
        "Duplicate should be handled gracefully"
    );

    relay.disconnect().await.ok();
}

// =============================================================================
// Connection State Tests
// =============================================================================

#[tokio::test]
async fn test_multiple_subscriptions_per_connection() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Create multiple subscriptions on same connection
    for i in 0..10 {
        let sub_id = format!("sub-{}", i);
        let filters = vec![json!({"kinds": [1]})];

        let result = relay.subscribe(&sub_id, &filters).await;
        assert!(
            result.is_ok(),
            "Should support multiple subscriptions on one connection"
        );
    }

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_subscription_replacement_same_id() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // First subscription
    let filters1 = vec![json!({"kinds": [1]})];
    relay.subscribe("replace-test", &filters1).await.unwrap();

    // Replace with different filter
    let filters2 = vec![json!({"kinds": [2]})];
    relay.subscribe("replace-test", &filters2).await.unwrap();

    // Old subscription should be replaced
    // (Verified by behavior in subscriptions.rs tests)

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_subscription_independent_per_connection() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay1 = RelayConnection::new(&url).unwrap();
    let relay2 = RelayConnection::new(&url).unwrap();

    relay1.connect().await.unwrap();
    relay2.connect().await.unwrap();

    // Same subscription ID on different connections should be independent
    let filters = vec![json!({"kinds": [1]})];

    relay1.subscribe("same-id", &filters).await.unwrap();
    relay2.subscribe("same-id", &filters).await.unwrap();

    // Both should work independently
    // (Each connection manages its own subscriptions)

    relay1.disconnect().await.ok();
    relay2.disconnect().await.ok();
}
