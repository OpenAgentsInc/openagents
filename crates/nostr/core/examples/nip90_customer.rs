//! Example: NIP-90 Customer - Submitting Job Requests
//!
//! This example demonstrates how to:
//! 1. Create a job request for text generation
//! 2. Sign and publish the request to relays
//! 3. Listen for job results
//! 4. Handle payment

use nostr::nip90::{JobInput, JobRequest, KIND_JOB_TEXT_GENERATION};
use nostr::{EventTemplate, finalize_event, generate_secret_key};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== NIP-90 Customer Example ===\n");

    // 1. Generate customer keypair (in production, load from secure storage)
    let secret_key = generate_secret_key();
    println!("Customer secret key generated");

    // 2. Create a text generation job request
    let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
        .add_input(JobInput::text(
            "Write a haiku about decentralized protocols",
        ))
        .add_param("model", "llama3.2")
        .add_param("temperature", "0.7")
        .add_param("max_tokens", "100")
        .with_bid(1000) // Willing to pay 1000 millisats
        .add_relay("wss://relay.damus.io")
        .add_relay("wss://nos.lol");

    println!("\nJob Request Created:");
    println!("  Kind: {}", request.kind);
    println!("  Inputs: {} items", request.inputs.len());
    println!("  Parameters: {} items", request.params.len());
    println!("  Bid: {:?} millisats", request.bid);
    println!("  Relays: {:?}", request.relays);

    // 3. Convert to event and sign
    let template = EventTemplate {
        kind: request.kind,
        content: request.content.clone(),
        tags: request.to_tags(),
        created_at: chrono::Utc::now().timestamp() as u64,
    };

    let event = finalize_event(&template, &secret_key)?;
    println!("\nEvent signed:");
    println!("  ID: {}", event.id);
    println!("  Pubkey: {}", hex::encode(event.pubkey));

    // In a real application, you would:
    //
    // 4. Publish to relays
    // ```rust
    // let relay_client = RelayClient::new();
    // relay_client.publish(&event, &["wss://relay.damus.io"]).await?;
    // ```
    //
    // 5. Subscribe to results (kind 6050 for text generation)
    // ```rust
    // let filter = Filter::new()
    //     .kind(6050)
    //     .tag("e", &event.id);
    //
    // relay_client.subscribe(filter).await?;
    // ```
    //
    // 6. Handle incoming results
    // ```rust
    // while let Some(result_event) = relay_client.next_event().await? {
    //     let result = JobResult::from_event(&result_event)?;
    //
    //     println!("Result received: {}", result.content);
    //
    //     if let (Some(amount), Some(bolt11)) = (result.amount, result.bolt11) {
    //         println!("Payment required: {} millisats", amount);
    //         println!("Invoice: {}", bolt11);
    //
    //         // Pay the invoice
    //         lightning_client.pay_invoice(&bolt11).await?;
    //     }
    // }
    // ```

    println!("\n✓ Job request ready to publish!");
    println!("\nNext steps:");
    println!("  1. Connect to Nostr relays");
    println!("  2. Publish the job request event");
    println!(
        "  3. Subscribe to result events (kind {})",
        request.result_kind()
    );
    println!("  4. Wait for service provider responses");
    println!("  5. Pay bolt11 invoice when result arrives");

    Ok(())
}
