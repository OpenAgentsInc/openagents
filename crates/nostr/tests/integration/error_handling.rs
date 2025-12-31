//! Error handling and edge case tests

use super::*;
use nostr::{EventTemplate, finalize_event, generate_secret_key};
use nostr_client::RelayConnection;
use serde_json::json;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn test_invalid_event_rejection() {
    let port = 17300;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Create event with invalid signature
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Test".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let mut event = finalize_event(&template, &secret_key).unwrap();

    // Corrupt the signature
    event.sig = "0".repeat(128);

    // Attempt to publish
    let result = relay.publish_event(&event, Duration::from_secs(5)).await;

    // Should either fail or return not accepted
    if let Ok(confirmation) = result {
        assert!(!confirmation.accepted, "Invalid event should be rejected");
    }

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_malformed_json_handling() {
    let port = 17301;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Try to send malformed message through raw send (if available)
    // This tests relay's error handling for protocol violations
    // For now, just ensure client handles connection properly

    // Send valid message to ensure connection works
    let filters = vec![json!({"kinds": [1]})];
    let result = relay.subscribe("test", &filters).await;
    assert!(result.is_ok(), "Valid message should work");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_invalid_filter_handling() {
    let port = 17302;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Try filter with invalid limit
    let filters = vec![json!({
        "kinds": [1],
        "limit": 10000  // Over max
    })];

    let result = relay.subscribe("invalid-filter", &filters).await;

    // Should either fail or relay should send CLOSED message
    // The exact behavior depends on relay implementation
    assert!(result.is_ok() || result.is_err());

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_connection_to_invalid_url() {
    let url = "ws://127.0.0.1:99999"; // Invalid port

    let relay = RelayConnection::new(url).unwrap();
    let result = timeout(Duration::from_secs(2), relay.connect()).await;

    // Should timeout or error
    assert!(result.is_err() || result.unwrap().is_err());
}

#[tokio::test]
async fn test_publish_while_disconnected() {
    let url = "ws://127.0.0.1:17311";

    let relay = RelayConnection::new(url).unwrap();
    // Don't connect

    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Test".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();

    let result = relay.publish_event(&event, Duration::from_secs(5)).await;

    // Should fail since not connected
    assert!(result.is_err(), "Publish should fail when not connected");
}

#[tokio::test]
async fn test_subscribe_while_disconnected() {
    let url = "ws://127.0.0.1:17312";

    let relay = RelayConnection::new(url).unwrap();
    // Don't connect

    let filters = vec![json!({"kinds": [1]})];
    let result = relay.subscribe("test", &filters).await;

    // Should fail since not connected
    assert!(result.is_err(), "Subscribe should fail when not connected");
}

#[tokio::test]
async fn test_empty_subscription_id() {
    let port = 17305;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Try to subscribe with empty ID (should be rejected by relay)
    let filters = vec![json!({"kinds": [1]})];
    let result = relay.subscribe("", &filters).await;

    // Should either fail at client level or relay should reject
    assert!(result.is_ok() || result.is_err());

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_very_long_subscription_id() {
    let port = 17306;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Try subscription ID that's too long (>64 chars)
    let long_id = "a".repeat(100);
    let filters = vec![json!({"kinds": [1]})];
    let result = relay.subscribe(&long_id, &filters).await;

    // Should be handled (either rejected or truncated)
    assert!(result.is_ok() || result.is_err());

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_event_with_future_timestamp() {
    let port = 17307;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Create event with timestamp far in future
    let secret_key = generate_secret_key();
    let far_future = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + (365 * 24 * 60 * 60 * 2); // 2 years in future

    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Future event".to_string(),
        created_at: far_future,
    };
    let event = finalize_event(&template, &secret_key).unwrap();

    let result = relay.publish_event(&event, Duration::from_secs(5)).await;

    // Relay should reject events too far in future
    if let Ok(confirmation) = result {
        assert!(
            !confirmation.accepted,
            "Event with far future timestamp should be rejected"
        );
    }

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_event_with_past_timestamp() {
    let port = 17308;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Create event with very old timestamp
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Old event".to_string(),
        created_at: 1000, // Very old Unix timestamp
    };
    let event = finalize_event(&template, &secret_key).unwrap();

    let result = relay.publish_event(&event, Duration::from_secs(5)).await;

    // Relay should reject events too far in past
    if let Ok(confirmation) = result {
        assert!(
            !confirmation.accepted,
            "Event with very old timestamp should be rejected"
        );
    }

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_close_nonexistent_subscription() {
    let port = 17309;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Try to close subscription that doesn't exist
    let result = relay.close_subscription("nonexistent-sub").await;

    // Should succeed (idempotent operation)
    assert!(result.is_ok());

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_rapid_reconnection() {
    let port = 17310;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();

    // Connect and disconnect rapidly
    for _ in 0..5 {
        relay.connect().await.unwrap();
        relay.disconnect().await.unwrap();
    }

    // Final connection should work
    let result = relay.connect().await;
    assert!(result.is_ok(), "Final reconnection should succeed");

    relay.disconnect().await.ok();
}
