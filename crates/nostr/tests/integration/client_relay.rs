//! Basic client-relay communication tests

use super::*;
use nostr::{EventTemplate, finalize_event, generate_secret_key};
use nostr_client::{RelayConnection, RelayMessage};
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn test_client_connects_to_relay() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    let result = timeout(Duration::from_secs(10), relay.connect()).await;

    assert!(result.is_ok(), "Connection should succeed");
    if let Err(err) = result.unwrap() {
        panic!("Connection should not error: {}", err);
    }

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_client_publishes_event() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Create and publish event
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Hello from integration test!".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();

    let result = timeout(
        Duration::from_secs(2),
        relay.publish_event(&event, Duration::from_secs(5)),
    )
    .await;
    assert!(result.is_ok(), "Publish should complete");

    let confirmation = result.unwrap().unwrap();
    assert!(confirmation.accepted, "Event should be accepted");
    assert_eq!(confirmation.event_id, event.id);

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_client_subscribes_and_receives_event() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    // Connect first client to subscribe
    let relay1 = RelayConnection::new(&url).unwrap();
    let connect_result = timeout(Duration::from_secs(2), relay1.connect()).await;
    assert!(connect_result.is_ok(), "Relay1 connect timed out");
    assert!(connect_result.unwrap().is_ok(), "Relay1 connect failed");

    // Subscribe to kind 1 events
    let filters = vec![serde_json::json!({
        "kinds": [1]
    })];
    let sub_result = timeout(
        Duration::from_secs(2),
        relay1.subscribe_with_channel("test-sub", &filters),
    )
    .await;
    assert!(sub_result.is_ok(), "Relay1 subscribe timed out");
    let mut rx = sub_result.unwrap().unwrap();

    // Give subscription time to register
    sleep(Duration::from_millis(50)).await;

    // Connect second client to publish
    let relay2 = RelayConnection::new(&url).unwrap();
    let connect_result = timeout(Duration::from_secs(2), relay2.connect()).await;
    assert!(connect_result.is_ok(), "Relay2 connect timed out");
    assert!(connect_result.unwrap().is_ok(), "Relay2 connect failed");

    // Publish event
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Test event".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();
    let event_id = event.id.clone();

    let publish_result = timeout(
        Duration::from_secs(5),
        relay2.publish_event(&event, Duration::from_secs(5)),
    )
    .await;
    assert!(publish_result.is_ok(), "Relay2 publish timed out");
    publish_result.unwrap().unwrap();

    // Wait for event on first client
    let result = timeout(Duration::from_secs(2), rx.recv()).await;

    assert!(result.is_ok(), "Should receive event");
    let received_event = result.unwrap().unwrap();
    assert_eq!(received_event.id, event_id);

    relay1.disconnect().await.ok();
    relay2.disconnect().await.ok();
}

#[tokio::test]
async fn test_subscription_receives_eose() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Subscribe
    let filters = vec![serde_json::json!({
        "kinds": [1],
        "limit": 10
    })];
    relay.subscribe("eose-test", &filters).await.unwrap();

    // Wait for EOSE
    let result = timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay.recv().await
                && let RelayMessage::Eose(sub_id) = msg
            {
                return Some(sub_id);
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should receive EOSE");
    assert_eq!(result.unwrap().unwrap(), "eose-test");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_close_subscription() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Subscribe
    let filters = vec![serde_json::json!({"kinds": [1]})];
    relay.subscribe("close-test", &filters).await.unwrap();

    // Wait for EOSE
    timeout(Duration::from_secs(1), async {
        loop {
            if let Ok(Some(RelayMessage::Eose(_))) = relay.recv().await {
                break;
            }
        }
    })
    .await
    .ok();

    // Close subscription
    let result = relay.close_subscription("close-test").await;
    assert!(result.is_ok(), "Close should succeed");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_multiple_clients_receive_broadcast() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    // Connect 3 clients
    let relay1 = RelayConnection::new(&url).unwrap();
    let relay2 = RelayConnection::new(&url).unwrap();
    let relay3 = RelayConnection::new(&url).unwrap();

    relay1.connect().await.unwrap();
    relay2.connect().await.unwrap();
    relay3.connect().await.unwrap();

    // All subscribe to kind 1
    let filters = vec![serde_json::json!({"kinds": [1]})];
    relay1.subscribe("sub1", &filters).await.unwrap();
    relay2.subscribe("sub2", &filters).await.unwrap();
    relay3.subscribe("sub3", &filters).await.unwrap();

    // Wait for subscriptions to register
    sleep(Duration::from_millis(100)).await;

    // Publish event from relay1
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Broadcast test".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();
    let event_id = event.id.clone();

    relay1
        .publish_event(&event, Duration::from_secs(5))
        .await
        .unwrap();

    // All clients should receive the event
    let mut received_count = 0;
    let timeout_duration = Duration::from_secs(2);

    for relay in [&relay1, &relay2, &relay3] {
        let result = timeout(timeout_duration, async {
            loop {
                if let Ok(Some(msg)) = relay.recv().await
                    && let RelayMessage::Event(_, evt) = msg
                    && evt.id == event_id
                {
                    return true;
                }
            }
        })
        .await;

        if result.is_ok() && result.unwrap() {
            received_count += 1;
        }
    }

    assert_eq!(received_count, 3, "All 3 clients should receive the event");

    relay1.disconnect().await.ok();
    relay2.disconnect().await.ok();
    relay3.disconnect().await.ok();
}

#[tokio::test]
async fn test_reconnection() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Disconnect
    relay.disconnect().await.unwrap();

    // Reconnect
    let result = timeout(Duration::from_secs(2), relay.connect()).await;
    assert!(result.is_ok(), "Reconnection should succeed");
    assert!(result.unwrap().is_ok(), "Reconnection should not error");

    relay.disconnect().await.ok();
}
