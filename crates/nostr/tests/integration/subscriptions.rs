//! Subscription behavior tests

use super::*;
use nostr::{
    EventTemplate, JobInput, JobRequest, KIND_JOB_TEXT_GENERATION, create_job_request_event,
    finalize_event, generate_secret_key,
};
use nostr_client::{RelayConnection, RelayMessage};
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn test_subscription_replacement() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // First subscription for kind 1
    let filters1 = vec![serde_json::json!({"kinds": [1]})];
    relay.subscribe("test-sub", &filters1).await.unwrap();

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

    // Replace with subscription for kind 2 (same ID)
    let filters2 = vec![serde_json::json!({"kinds": [2]})];
    relay.subscribe("test-sub", &filters2).await.unwrap();

    // Publish kind 1 event - should NOT be received
    let secret_key = generate_secret_key();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let template1 = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Kind 1 event".to_string(),
        created_at: now,
    };
    let event1 = finalize_event(&template1, &secret_key).unwrap();
    relay
        .publish_event(&event1, Duration::from_secs(5))
        .await
        .unwrap();

    // Publish kind 2 event - should be received
    let template2 = EventTemplate {
        kind: 2,
        tags: vec![],
        content: "Kind 2 event".to_string(),
        created_at: now + 1,
    };
    let event2 = finalize_event(&template2, &secret_key).unwrap();
    let _event2_id = event2.id.clone();
    relay
        .publish_event(&event2, Duration::from_secs(5))
        .await
        .unwrap();

    // Should only receive kind 2 event
    let result = timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay.recv().await
                && let RelayMessage::Event(_, evt) = msg
            {
                return evt.kind;
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should receive event");
    assert_eq!(result.unwrap(), 2, "Should only receive kind 2 event");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_multiple_concurrent_subscriptions() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Create 3 different subscriptions
    relay
        .subscribe("sub-kind-1", &[serde_json::json!({"kinds": [1]})])
        .await
        .unwrap();
    relay
        .subscribe("sub-kind-2", &[serde_json::json!({"kinds": [2]})])
        .await
        .unwrap();
    relay
        .subscribe("sub-kind-3", &[serde_json::json!({"kinds": [3]})])
        .await
        .unwrap();

    // Wait for all EOSE messages
    let mut eose_count = 0;
    timeout(Duration::from_secs(2), async {
        while eose_count < 3 {
            if let Ok(Some(RelayMessage::Eose(_))) = relay.recv().await {
                eose_count += 1;
            }
        }
    })
    .await
    .ok();

    assert_eq!(eose_count, 3, "Should receive 3 EOSE messages");

    // Publish kind 1 event
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
    let event_id = event.id.clone();
    relay
        .publish_event(&event, Duration::from_secs(5))
        .await
        .unwrap();

    // Should receive event on sub-kind-1
    let result = timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay.recv().await
                && let RelayMessage::Event(sub_id, evt) = msg
                && evt.id == event_id
            {
                return Some(sub_id);
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should receive event");
    assert_eq!(result.unwrap().unwrap(), "sub-kind-1");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_subscription_with_multiple_filters() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Subscribe with OR filters (kind 1 OR kind 2)
    let filters = vec![
        serde_json::json!({"kinds": [1]}),
        serde_json::json!({"kinds": [2]}),
    ];
    relay.subscribe("multi-filter", &filters).await.unwrap();

    sleep(Duration::from_millis(100)).await;

    // Publish both kinds
    let secret_key = generate_secret_key();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let template1 = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Kind 1".to_string(),
        created_at: now,
    };
    let event1 = finalize_event(&template1, &secret_key).unwrap();

    let template2 = EventTemplate {
        kind: 2,
        tags: vec![],
        content: "Kind 2".to_string(),
        created_at: now + 1,
    };
    let event2 = finalize_event(&template2, &secret_key).unwrap();

    relay
        .publish_event(&event1, Duration::from_secs(5))
        .await
        .unwrap();
    relay
        .publish_event(&event2, Duration::from_secs(5))
        .await
        .unwrap();

    // Should receive both events
    let mut received_kinds = Vec::new();
    timeout(Duration::from_secs(2), async {
        while received_kinds.len() < 2 {
            if let Ok(Some(msg)) = relay.recv().await
                && let RelayMessage::Event(_, evt) = msg
                && (evt.id == event1.id || evt.id == event2.id)
            {
                received_kinds.push(evt.kind);
            }
        }
    })
    .await
    .ok();

    assert_eq!(received_kinds.len(), 2, "Should receive both events");
    assert!(received_kinds.contains(&1), "Should receive kind 1");
    assert!(received_kinds.contains(&2), "Should receive kind 2");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_realtime_event_delivery() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay1 = RelayConnection::new(&url).unwrap();
    let relay2 = RelayConnection::new(&url).unwrap();

    relay1.connect().await.unwrap();
    relay2.connect().await.unwrap();

    // Subscribe on relay1
    relay1
        .subscribe("realtime", &[serde_json::json!({"kinds": [1]})])
        .await
        .unwrap();

    // Wait for EOSE
    timeout(Duration::from_secs(1), async {
        loop {
            if let Ok(Some(RelayMessage::Eose(_))) = relay1.recv().await {
                break;
            }
        }
    })
    .await
    .ok();

    // Publish event from relay2
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Realtime test".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();
    let event_id = event.id.clone();

    // Record time before publish
    let before = std::time::Instant::now();
    relay2
        .publish_event(&event, Duration::from_secs(5))
        .await
        .unwrap();

    // Wait for event on relay1
    let result = timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay1.recv().await
                && let RelayMessage::Event(_, evt) = msg
                && evt.id == event_id
            {
                return std::time::Instant::now();
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should receive event");
    let after = result.unwrap();
    let latency = after.duration_since(before);

    // Should be delivered quickly (< 500ms)
    assert!(
        latency < Duration::from_millis(500),
        "Event should be delivered quickly, took {:?}",
        latency
    );

    relay1.disconnect().await.ok();
    relay2.disconnect().await.ok();
}

#[tokio::test]
async fn test_subscription_receives_job_request_kind() {
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay_sub = RelayConnection::new(&url).unwrap();
    let relay_pub = RelayConnection::new(&url).unwrap();

    relay_sub.connect().await.unwrap();
    relay_pub.connect().await.unwrap();

    let filters = vec![serde_json::json!({"kinds": [KIND_JOB_TEXT_GENERATION]})];
    relay_sub.subscribe("job-sub", &filters).await.unwrap();

    timeout(Duration::from_secs(1), async {
        loop {
            if let Ok(Some(RelayMessage::Eose(_))) = relay_sub.recv().await {
                break;
            }
        }
    })
    .await
    .ok();

    let secret_key = generate_secret_key();
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .unwrap()
        .add_input(JobInput::text("Write a summary"));
    let template = create_job_request_event(&request);
    let event = finalize_event(&template, &secret_key).unwrap();
    let event_id = event.id.clone();

    relay_pub
        .publish_event(&event, Duration::from_secs(5))
        .await
        .unwrap();

    let result = timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay_sub.recv().await {
                if let RelayMessage::Event(sub_id, evt) = msg {
                    if evt.id == event_id {
                        return (sub_id, evt);
                    }
                }
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should receive job request event");
    let (sub_id, received_event) = result.unwrap();
    assert_eq!(sub_id, "job-sub");
    assert_eq!(received_event.kind, KIND_JOB_TEXT_GENERATION);

    relay_sub.disconnect().await.ok();
    relay_pub.disconnect().await.ok();
}
