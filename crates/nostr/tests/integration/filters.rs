//! Filter matching tests

use super::*;
use nostr::{EventTemplate, finalize_event, generate_secret_key};
use nostr_client::{RelayConnection, RelayMessage};
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn test_filter_by_kinds() {
    let port = 17200;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Subscribe to kinds 1 and 3
    relay
        .subscribe("kind-filter", &[serde_json::json!({"kinds": [1, 3]})])
        .await
        .unwrap();

    sleep(Duration::from_millis(100)).await;

    // Publish events of different kinds
    let secret_key = generate_secret_key();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    for kind in [1, 2, 3, 4] {
        let template = EventTemplate {
            kind,
            tags: vec![],
            content: format!("Kind {} event", kind),
            created_at: now + kind as u64,
        };
        let event = finalize_event(&template, &secret_key).unwrap();
        relay
            .publish_event(&event, Duration::from_secs(5))
            .await
            .unwrap();
    }

    // Should only receive kinds 1 and 3
    let mut received_kinds = Vec::new();
    timeout(Duration::from_secs(2), async {
        while received_kinds.len() < 2 {
            if let Ok(Some(msg)) = relay.recv().await
                && let RelayMessage::Event(_, evt) = msg
            {
                received_kinds.push(evt.kind);
            }
        }
    })
    .await
    .ok();

    assert_eq!(received_kinds.len(), 2);
    assert!(received_kinds.contains(&1));
    assert!(received_kinds.contains(&3));
    assert!(!received_kinds.contains(&2));
    assert!(!received_kinds.contains(&4));

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_filter_by_authors() {
    let port = 17201;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Create two different keys
    let secret_key1 = generate_secret_key();
    let secret_key2 = generate_secret_key();
    let pubkey1 = nostr::get_public_key_hex(&secret_key1).unwrap();

    // Subscribe to events from pubkey1 only
    relay
        .subscribe(
            "author-filter",
            &[serde_json::json!({"authors": [pubkey1]})],
        )
        .await
        .unwrap();

    sleep(Duration::from_millis(100)).await;

    // Publish events from both keys
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let template1 = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "From key 1".to_string(),
        created_at: now,
    };
    let event1 = finalize_event(&template1, &secret_key1).unwrap();
    let event1_id = event1.id.clone();

    let template2 = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "From key 2".to_string(),
        created_at: now + 1,
    };
    let event2 = finalize_event(&template2, &secret_key2).unwrap();

    relay
        .publish_event(&event1, Duration::from_secs(5))
        .await
        .unwrap();
    relay
        .publish_event(&event2, Duration::from_secs(5))
        .await
        .unwrap();

    // Should only receive event from key 1
    let result = timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay.recv().await
                && let RelayMessage::Event(_, evt) = msg
            {
                return evt.id;
            }
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), event1_id);

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_filter_by_event_ids() {
    let port = 17202;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Publish some events first
    let secret_key = generate_secret_key();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut event_ids = Vec::new();
    for i in 0..3 {
        let template = EventTemplate {
            kind: 1,
            tags: vec![],
            content: format!("Event {}", i),
            created_at: now + i,
        };
        let event = finalize_event(&template, &secret_key).unwrap();
        event_ids.push(event.id.clone());
        relay
            .publish_event(&event, Duration::from_secs(5))
            .await
            .unwrap();
    }

    sleep(Duration::from_millis(100)).await;

    // Subscribe to specific event IDs (first and third)
    relay
        .subscribe(
            "id-filter",
            &[serde_json::json!({
                "ids": [event_ids[0].clone(), event_ids[2].clone()]
            })],
        )
        .await
        .unwrap();

    // Should receive only those two events
    let mut received_ids = Vec::new();
    timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay.recv().await {
                match msg {
                    RelayMessage::Event(_, evt) => {
                        received_ids.push(evt.id);
                        if received_ids.len() == 2 {
                            break;
                        }
                    }
                    RelayMessage::Eose(_) => break,
                    _ => {}
                }
            }
        }
    })
    .await
    .ok();

    assert_eq!(received_ids.len(), 2);
    assert!(received_ids.contains(&event_ids[0]));
    assert!(received_ids.contains(&event_ids[2]));
    assert!(!received_ids.contains(&event_ids[1]));

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_filter_by_tags() {
    let port = 17203;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Create event with e tag
    let secret_key = generate_secret_key();
    let referenced_event_id = "a".repeat(64);

    let template = EventTemplate {
        kind: 1,
        tags: vec![
            vec!["e".to_string(), referenced_event_id.clone()],
            vec!["p".to_string(), "b".repeat(64)],
        ],
        content: "Reply event".to_string(),
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

    sleep(Duration::from_millis(100)).await;

    // Subscribe to events with this e tag
    relay
        .subscribe(
            "tag-filter",
            &[serde_json::json!({
                "#e": [referenced_event_id]
            })],
        )
        .await
        .unwrap();

    // Should receive the event
    let result = timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay.recv().await
                && let RelayMessage::Event(_, evt) = msg
            {
                return evt.id;
            }
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), event_id);

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_filter_by_since_until() {
    let port = 17204;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Publish events with different timestamps
    let secret_key = generate_secret_key();
    let base_time = 1700000000u64;

    let timestamps = [base_time, base_time + 100, base_time + 200, base_time + 300];
    let mut event_ids = Vec::new();

    for &ts in &timestamps {
        let template = EventTemplate {
            kind: 1,
            tags: vec![],
            content: format!("Event at {}", ts),
            created_at: ts,
        };
        let event = finalize_event(&template, &secret_key).unwrap();
        event_ids.push(event.id.clone());
        relay
            .publish_event(&event, Duration::from_secs(5))
            .await
            .unwrap();
    }

    sleep(Duration::from_millis(100)).await;

    // Subscribe to events between base_time + 100 and base_time + 200
    relay
        .subscribe(
            "time-filter",
            &[serde_json::json!({
                "since": base_time + 100,
                "until": base_time + 200
            })],
        )
        .await
        .unwrap();

    // Should receive events at timestamps 1 and 2 (indices 1 and 2)
    let mut received_ids = Vec::new();
    timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay.recv().await {
                match msg {
                    RelayMessage::Event(_, evt) => {
                        received_ids.push(evt.id);
                    }
                    RelayMessage::Eose(_) => break,
                    _ => {}
                }
            }
        }
    })
    .await
    .ok();

    assert_eq!(received_ids.len(), 2);
    assert!(received_ids.contains(&event_ids[1]));
    assert!(received_ids.contains(&event_ids[2]));

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_filter_limit() {
    let port = 17205;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let url = test_relay_url(port);

    let relay = RelayConnection::new(&url).unwrap();
    relay.connect().await.unwrap();

    // Publish 10 events
    let secret_key = generate_secret_key();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    for i in 0..10 {
        let template = EventTemplate {
            kind: 1,
            tags: vec![],
            content: format!("Event {}", i),
            created_at: now + i,
        };
        let event = finalize_event(&template, &secret_key).unwrap();
        relay
            .publish_event(&event, Duration::from_secs(5))
            .await
            .unwrap();
    }

    sleep(Duration::from_millis(100)).await;

    // Subscribe with limit 3
    relay
        .subscribe(
            "limit-filter",
            &[serde_json::json!({"kinds": [1], "limit": 3})],
        )
        .await
        .unwrap();

    // Should receive at most 3 events
    let mut count = 0;
    timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Some(msg)) = relay.recv().await {
                match msg {
                    RelayMessage::Event(_, _) => count += 1,
                    RelayMessage::Eose(_) => break,
                    _ => {}
                }
            }
        }
    })
    .await
    .ok();

    assert!(count <= 3, "Should receive at most 3 events, got {}", count);

    relay.disconnect().await.ok();
}
