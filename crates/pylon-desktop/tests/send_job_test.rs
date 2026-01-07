//! Test sending a job request to the relay

use nostr::{
    generate_secret_key, get_public_key_hex, finalize_event, EventTemplate,
    nip90::{KIND_JOB_TEXT_GENERATION, JobRequest, JobInput},
};
use nostr_client::{RelayConnection, RelayMessage};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Job Request Test ===\n");

    // Generate test keypair
    let secret_key = generate_secret_key();
    let pubkey = get_public_key_hex(&secret_key)?;
    println!("Test pubkey: {}", &pubkey[..16]);

    // Connect to relay
    let relay_url = "wss://nexus.openagents.com/";
    println!("Connecting to {}...", relay_url);
    let relay = RelayConnection::new(relay_url)?;
    relay.connect().await?;
    println!("Connected!");

    // Wait for and handle AUTH
    println!("Waiting for auth challenge...");
    let mut authenticated = false;
    while !authenticated {
        if let Ok(Some(msg)) = tokio::time::timeout(
            Duration::from_secs(5),
            relay.recv()
        ).await? {
            match msg {
                RelayMessage::Auth(challenge) => {
                    println!("Got challenge: {}...", &challenge[..20.min(challenge.len())]);

                    // Create NIP-42 AUTH event
                    let auth_template = EventTemplate {
                        kind: 22242,
                        content: String::new(),
                        tags: vec![
                            vec!["relay".to_string(), relay_url.to_string()],
                            vec!["challenge".to_string(), challenge],
                        ],
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)?
                            .as_secs(),
                    };

                    let auth_event = finalize_event(&auth_template, &secret_key)?;
                    let auth_msg = serde_json::json!(["AUTH", auth_event]);
                    relay.send_message(&auth_msg).await?;
                    println!("Sent auth response");
                }
                RelayMessage::Ok(_event_id, accepted, message) => {
                    if accepted {
                        println!("Auth accepted: {}", message);
                        authenticated = true;
                    } else {
                        println!("Auth rejected: {}", message);
                    }
                }
                _ => {}
            }
        }
    }

    // Create job request
    println!("\nCreating job request...");
    let job_request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
        .add_input(JobInput::text("What is 2+2? Reply with just the number."))
        .add_param("model", "apple-fm")
        .add_param("max_tokens", "100");

    let template = EventTemplate {
        kind: KIND_JOB_TEXT_GENERATION,
        content: String::new(),
        tags: job_request.to_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
    };

    let event = finalize_event(&template, &secret_key)?;
    let event_id = event.id.clone();
    println!("Job ID: {}", &event_id[..16]);

    // Publish
    println!("Publishing job request...");
    match relay.publish_event(&event, Duration::from_secs(10)).await {
        Ok(conf) => {
            if conf.accepted {
                println!("Job published successfully!");
            } else {
                println!("Job rejected: {}", conf.message);
                return Ok(());
            }
        }
        Err(e) => {
            println!("Publish failed: {}", e);
            return Ok(());
        }
    }

    // Wait for result
    println!("\nWaiting for job result (30s timeout)...");
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        if let Ok(Ok(Some(msg))) = tokio::time::timeout(
            Duration::from_millis(100),
            relay.recv()
        ).await {
            if let RelayMessage::Event(_, event) = msg {
                if event.kind == KIND_JOB_TEXT_GENERATION + 1000 {
                    // Check if it's for our job
                    let is_for_us = event.tags.iter()
                        .any(|t| t.first().map(|s| s.as_str()) == Some("e")
                            && t.get(1).map(|s| s.as_str()) == Some(&event_id));

                    if is_for_us {
                        println!("\n=== GOT RESULT ===");
                        println!("From: {}...", &event.pubkey[..16]);
                        println!("Content: {}", event.content);
                        break;
                    }
                }
            }
        }
    }

    relay.disconnect().await?;
    println!("\nTest complete!");
    Ok(())
}
