//! Submit a NIP-90 job to a DVM provider
//!
//! Usage: cargo run --example submit_job -- --prompt "Hello" --relay wss://relay.damus.io --provider <pubkey>

use nostr::{JobInput, JobRequest, KIND_JOB_TEXT_GENERATION};
use nostr_client::dvm::DvmClient;
use std::time::Duration;

// Default pylon provider pubkey (from pylon start logs)
const DEFAULT_PROVIDER: &str = "16dd3cf45416ae3de31264256d944539cd25077d104beb2ded078928010dbeb6";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info,nostr_client=debug")
        .init();

    let prompt = std::env::args()
        .skip_while(|a| a != "--prompt")
        .nth(1)
        .unwrap_or_else(|| "Say hello in exactly 5 words".to_string());

    let relay = std::env::args()
        .skip_while(|a| a != "--relay")
        .nth(1)
        .unwrap_or_else(|| "wss://relay.damus.io".to_string());

    let provider = std::env::args()
        .skip_while(|a| a != "--provider")
        .nth(1)
        .unwrap_or_else(|| DEFAULT_PROVIDER.to_string());

    println!("=== NIP-90 Job Submission Test ===");
    println!("Prompt: {}", prompt);
    println!("Relay: {}", relay);
    println!("Provider: {}", provider);

    // Generate a deterministic test key
    let mut private_key = [0u8; 32];
    private_key[31] = 42; // Non-zero for valid key

    let client = DvmClient::new(private_key)?;
    println!("Client pubkey: {}", client.pubkey());

    // Create job request with proper input and target provider
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
        .add_input(JobInput::text(&prompt))
        .add_param("model", "apple-foundation-model")
        .add_service_provider(&provider)
        .with_bid(1000);

    println!("\nSubmitting job...");
    let submission = client.submit_job(request, &[&relay]).await?;
    println!("Job submitted: {}", submission.event_id);
    println!("Waiting for result (60s timeout)...\n");

    // Subscribe to feedback
    let mut feedback_rx = client.subscribe_to_feedback(&submission.event_id).await?;

    // Spawn feedback listener
    let feedback_task = tokio::spawn(async move {
        while let Some(feedback) = feedback_rx.recv().await {
            println!("[Feedback] Status: {:?}", feedback.feedback.status);
            if !feedback.feedback.content.is_empty() {
                println!("[Feedback] Content: {}", feedback.feedback.content);
            }
            if let Some(amount) = feedback.feedback.amount {
                println!("[Feedback] Amount: {} msats", amount);
            }
        }
    });

    match client
        .await_result(&submission.event_id, Duration::from_secs(60))
        .await
    {
        Ok(result) => {
            println!("\n=== JOB RESULT ===");
            println!("Content: {}", result.content);
            if let Some(amount) = result.amount {
                println!("Amount: {} msats", amount);
            }
            if let Some(bolt11) = &result.bolt11 {
                let display = if bolt11.len() > 50 {
                    format!("{}...", &bolt11[..50])
                } else {
                    bolt11.clone()
                };
                println!("Invoice: {}", display);
            }
        }
        Err(e) => {
            println!("\n=== ERROR ===");
            println!("Failed: {}", e);
        }
    }

    feedback_task.abort();
    Ok(())
}
