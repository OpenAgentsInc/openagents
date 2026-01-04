//! Integration test for Nostr connectivity

use nostr::{generate_secret_key, get_public_key_hex, finalize_event, EventTemplate};
use nostr_client::RelayConnection;
use std::time::Duration;

#[tokio::test]
async fn test_nostr_relay_connection() {
    // Connect to local relay (port 7001 to avoid macOS AirPlay conflict)
    let relay = RelayConnection::new("ws://127.0.0.1:7001").expect("Failed to create relay");
    relay.connect().await.expect("Failed to connect to relay");

    assert!(relay.is_connected().await);
    println!("Connected to relay!");

    // Generate keypair
    let secret_key = generate_secret_key();
    let pubkey = get_public_key_hex(&secret_key).expect("Failed to derive pubkey");
    println!("Generated pubkey: {}", &pubkey[..16]);

    // Subscribe to kind 1 events
    let filter = serde_json::json!({
        "kinds": [1],
        "limit": 10
    });
    relay.subscribe("test", &[filter]).await.expect("Failed to subscribe");
    println!("Subscribed to kind:1 events");

    // Create and publish a test event
    let template = EventTemplate {
        kind: 1,
        content: "Hello from pylon-desktop test!".to_string(),
        tags: vec![],
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &secret_key).expect("Failed to finalize event");
    println!("Created event: {}", &event.id[..16]);

    let result = relay.publish_event(&event, Duration::from_secs(5)).await;
    match result {
        Ok(confirmation) => {
            println!("Published event - accepted: {}, message: {}", confirmation.accepted, confirmation.message);
            assert!(confirmation.accepted);
        }
        Err(e) => {
            panic!("Failed to publish: {}", e);
        }
    }

    // Subscribe to NIP-90 job requests
    let job_filter = serde_json::json!({
        "kinds": [5050],
        "limit": 10
    });
    relay.subscribe("jobs", &[job_filter]).await.expect("Failed to subscribe to jobs");
    println!("Subscribed to kind:5050 job requests");

    // Subscribe to NIP-28 chat messages
    let chat_filter = serde_json::json!({
        "kinds": [42],
        "limit": 10
    });
    relay.subscribe("chat", &[chat_filter]).await.expect("Failed to subscribe to chat");
    println!("Subscribed to kind:42 chat messages");

    // Publish a NIP-28 chat message
    let chat_template = EventTemplate {
        kind: 42,
        content: "Test chat message from pylon-desktop!".to_string(),
        tags: vec![
            vec!["e".to_string(), "test-channel-id".to_string(), String::new(), "root".to_string()],
        ],
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let chat_event = finalize_event(&chat_template, &secret_key).expect("Failed to finalize chat event");
    let chat_result = relay.publish_event(&chat_event, Duration::from_secs(5)).await;
    match chat_result {
        Ok(confirmation) => {
            println!("Published chat message - accepted: {}", confirmation.accepted);
            assert!(confirmation.accepted);
        }
        Err(e) => {
            panic!("Failed to publish chat: {}", e);
        }
    }

    // Publish a NIP-90 job request
    let job_template = EventTemplate {
        kind: 5050,
        content: String::new(),
        tags: vec![
            vec!["i".to_string(), "What is the capital of France?".to_string(), "text".to_string()],
            vec!["param".to_string(), "model".to_string(), "apple-fm".to_string()],
        ],
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let job_event = finalize_event(&job_template, &secret_key).expect("Failed to finalize job event");
    let job_result = relay.publish_event(&job_event, Duration::from_secs(5)).await;
    match job_result {
        Ok(confirmation) => {
            println!("Published job request - accepted: {}", confirmation.accepted);
            assert!(confirmation.accepted);
        }
        Err(e) => {
            panic!("Failed to publish job: {}", e);
        }
    }

    println!("\n=== All Nostr connectivity tests passed! ===");
}
