//! Test connection to live relay.openagents.com

use nostr::{finalize_event, EventTemplate};
use nostr_client::RelayConnection;
use std::time::Duration;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[tokio::test]
async fn test_live_relay_auth() {
    // Generate keypair
    let secret_key: [u8; 32] = rand::random();

    // Connect to live relay
    let relay_url = "wss://relay.openagents.com/";
    let conn = RelayConnection::new(relay_url).expect("create connection");
    conn.connect().await.expect("connect to relay");

    println!("Connected to {}", relay_url);

    // Wait for AUTH challenge
    let mut challenge = None;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(Some(msg)) = conn.recv().await {
            match msg {
                nostr_client::RelayMessage::Auth(c) => {
                    println!("Received AUTH challenge: {}", c);
                    challenge = Some(c);
                    break;
                }
                other => {
                    println!("Received: {:?}", other);
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let challenge = challenge.expect("should receive AUTH challenge");

    // Create NIP-42 AUTH event
    let template = EventTemplate {
        kind: 22242,
        content: String::new(),
        tags: vec![
            vec!["relay".to_string(), relay_url.to_string()],
            vec!["challenge".to_string(), challenge],
        ],
        created_at: now(),
    };

    let event = finalize_event(&template, &secret_key).expect("sign event");
    println!("Created AUTH event: {}", event.id);

    // Send AUTH
    let msg = serde_json::json!(["AUTH", event]);
    conn.send_message(&msg).await.expect("send AUTH");

    // Wait for OK response
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(Some(msg)) = conn.recv().await {
            match msg {
                nostr_client::RelayMessage::Ok(event_id, accepted, message) => {
                    println!("AUTH response: event_id={}, accepted={}, message={}", event_id, accepted, message);
                    assert!(accepted, "AUTH should be accepted: {}", message);

                    // Now try to subscribe
                    let filter = serde_json::json!({
                        "kinds": [5050],
                        "limit": 10
                    });
                    conn.subscribe("test", &[filter]).await.expect("subscribe");
                    println!("Subscribed to kind 5050");

                    // Wait for EOSE
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    while let Ok(Some(msg)) = conn.recv().await {
                        println!("Received: {:?}", msg);
                        if matches!(msg, nostr_client::RelayMessage::Eose(_)) {
                            println!("Got EOSE - subscription confirmed!");
                            return;
                        }
                    }
                    return;
                }
                other => {
                    println!("Received: {:?}", other);
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    panic!("Did not receive OK response for AUTH");
}
