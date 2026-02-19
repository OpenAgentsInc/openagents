//! Example: NIP-90 Service Provider - Processing Job Requests
//!
//! This example demonstrates how to:
//! 1. Advertise DVM capabilities
//! 2. Listen for job requests
//! 3. Process jobs and publish results
//! 4. Request payment via bolt11 invoices

use nostr::nip90::{JobFeedback, JobResult, JobStatus, KIND_JOB_TEXT_GENERATION};
use nostr::{EventTemplate, finalize_event, generate_secret_key};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== NIP-90 Service Provider Example ===\n");

    // 1. Generate provider keypair
    let provider_sk = generate_secret_key();
    println!("Provider secret key generated");

    // In a real application, you would:
    //
    // 2. Advertise capabilities via NIP-89
    // ```rust
    // let handler = ApplicationHandler::new(KIND_JOB_TEXT_GENERATION)
    //     .with_relay("wss://relay.damus.io")
    //     .with_metadata("name", "My LLM Service")
    //     .with_metadata("about", "Fast text generation powered by Llama 3.2");
    //
    // relay_client.publish_handler(&handler).await?;
    // ```
    //
    // 3. Subscribe to job requests
    // ```rust
    // let filter = Filter::new()
    //     .kind(KIND_JOB_TEXT_GENERATION);
    //
    // relay_client.subscribe(filter).await?;
    // ```

    println!(
        "\nListening for job requests (kind {})...",
        KIND_JOB_TEXT_GENERATION
    );

    // 4. Process incoming job requests
    //
    // Simulate receiving a job request
    let example_request_id = "request_event_id_123";
    let example_customer_pubkey = "customer_pubkey_abc";

    println!("\n✓ Job request received:");
    println!("  Request ID: {}", example_request_id);
    println!("  Customer: {}", example_customer_pubkey);

    // 5. Optionally send feedback (payment required)
    let _feedback = JobFeedback::new(
        JobStatus::PaymentRequired,
        example_request_id,
        example_customer_pubkey,
    )
    .with_status_extra("Please pay 1000 millisats before processing")
    .with_amount(1000, Some("lnbc1000n...".to_string()));

    println!("\nSending feedback (kind 7000):");
    println!("  Status: payment-required");
    println!("  Amount: 1000 millisats");

    // In production:
    // ```rust
    // let feedback_template = EventTemplate {
    //     kind: KIND_JOB_FEEDBACK,
    //     content: feedback.content,
    //     tags: feedback.to_tags(),
    //     created_at: chrono::Utc::now().timestamp() as u64,
    // };
    // let feedback_event = finalize_event(&feedback_template, &provider_sk)?;
    // relay_client.publish(&feedback_event).await?;
    //
    // // Wait for payment confirmation
    // wait_for_payment(&bolt11_invoice).await?;
    // ```

    // 6. Perform the actual computation
    println!("\nPerforming computation...");
    let result_content = simulate_llm_generation("Write a haiku about decentralized protocols");
    println!("  Result: {}", result_content);

    // 7. Create and publish result
    let result = JobResult::new(
        KIND_JOB_TEXT_GENERATION,
        example_request_id,
        example_customer_pubkey,
        &result_content,
    )?
    .with_amount(1000, Some("lnbc1000n...".to_string()));

    let result_template = EventTemplate {
        kind: result.kind,
        content: result.content.clone(),
        tags: result.to_tags(),
        created_at: chrono::Utc::now().timestamp() as u64,
    };

    let result_event = finalize_event(&result_template, &provider_sk)?;

    println!("\nJob result published:");
    println!("  Event ID: {}", result_event.id);
    println!("  Kind: {} (result)", result_event.kind);
    println!("  Payment: {} millisats", result.amount.unwrap());

    // 8. Publish to relays
    // ```rust
    // relay_client.publish(&result_event, &relays).await?;
    // ```

    println!("\n✓ Job completed successfully!");
    println!("\nProvider workflow:");
    println!("  1. Advertise capabilities (NIP-89)");
    println!("  2. Subscribe to job requests");
    println!("  3. Send payment-required feedback (optional)");
    println!("  4. Wait for payment");
    println!("  5. Perform computation");
    println!("  6. Publish result with bolt11 invoice");
    println!("  7. Receive payment");

    Ok(())
}

/// Simulate LLM text generation
fn simulate_llm_generation(prompt: &str) -> String {
    println!("  Processing prompt: \"{}\"", prompt);

    // In a real provider, this would call an actual LLM
    "Protocols bloom,\n\
     Decentralized minds connect,\n\
     Freedom flows freely."
        .to_string()
}
