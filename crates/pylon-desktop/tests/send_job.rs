//! Send a NIP-90 job request to test pylon-desktop integration

use nostr::{generate_secret_key, get_public_key_hex, finalize_event, EventTemplate};
use nostr_client::RelayConnection;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to local relay
    let relay = RelayConnection::new("ws://127.0.0.1:7001")?;
    relay.connect().await?;
    println!("Connected to relay");

    // Generate keypair
    let secret_key = generate_secret_key();
    let pubkey = get_public_key_hex(&secret_key)?;
    println!("Test client pubkey: {}", &pubkey[..16]);

    // Subscribe to job results for our pubkey
    let result_filter = serde_json::json!({
        "kinds": [6050],
        "#p": [pubkey],
        "limit": 10
    });
    relay.subscribe("results", &[result_filter]).await?;
    println!("Subscribed to job results");

    // Create a NIP-90 job request (kind 5050)
    let job_template = EventTemplate {
        kind: 5050,
        content: String::new(),
        tags: vec![
            vec!["i".to_string(), "What is 2 + 2? Answer with just the number.".to_string(), "text".to_string()],
            vec!["param".to_string(), "model".to_string(), "apple-fm".to_string()],
        ],
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let job_event = finalize_event(&job_template, &secret_key)?;
    println!("Created job request: {}", &job_event.id[..16]);

    let result = relay.publish_event(&job_event, Duration::from_secs(5)).await?;
    if result.accepted {
        println!("Job request published successfully!");
    } else {
        println!("Job request rejected: {}", result.message);
        return Ok(());
    }

    println!("Waiting for job result from pylon-desktop...");
    println!("(Note: FM Bridge must be running for inference to complete)");

    // Wait for response (with timeout)
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        if let Ok(Some(msg)) = relay.recv().await {
            match msg {
                nostr_client::RelayMessage::Event(sub_id, event) => {
                    if sub_id == "results" && event.kind == 6050 {
                        println!("\n=== Received Job Result ===");
                        println!("Event ID: {}", event.id);
                        println!("Content: {}", event.content);
                        return Ok(());
                    }
                }
                nostr_client::RelayMessage::Eose(sub_id) => {
                    println!("End of stored events for: {}", sub_id);
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("Timeout waiting for job result");
    println!("(This is expected if FM Bridge is not running)");
    Ok(())
}
