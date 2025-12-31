//! Example: NIP-90 Types - Input Types and Job Chaining
//!
//! This example demonstrates:
//! 1. Different input types (text, url, event, job)
//! 2. Using markers for multi-input jobs
//! 3. Chaining jobs together
//! 4. Working with parameters

use nostr::nip90::{JobInput, JobParam, JobRequest, KIND_JOB_TEXT_GENERATION};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== NIP-90 Types Example ===\n");

    // 1. Text Input - Direct string input
    println!("1. Text Input:");
    let text_input = JobInput::text("Translate this text to French");
    println!("   Type: {:?}", text_input.input_type);
    println!("   Data: {}", text_input.data);

    // 2. URL Input - Fetch data from URL
    println!("\n2. URL Input:");
    let url_input = JobInput::url("https://example.com/document.pdf");
    println!("   Type: {:?}", url_input.input_type);
    println!("   Data: {}", url_input.data);

    // 3. Event Input - Reference a Nostr event
    println!("\n3. Event Input:");
    let event_input = JobInput::event(
        "abcd1234...",                            // Event ID
        Some("wss://relay.damus.io".to_string()), // Relay hint
    );
    println!("   Type: {:?}", event_input.input_type);
    println!("   Data: {}", event_input.data);
    println!("   Relay: {:?}", event_input.relay);

    // 4. Job Input - Chain from another job's output
    println!("\n4. Job Input (chaining):");
    let job_input = JobInput::job("previous_job_event_id", Some("wss://relay.com".to_string()));
    println!("   Type: {:?}", job_input.input_type);
    println!("   Data: {}", job_input.data);

    // 5. Using Markers for Multi-Input Jobs
    println!("\n5. Multi-Input Job with Markers:");

    let request = JobRequest::new(5002)? // Translation
        .add_input(JobInput::text("Hello, world!").with_marker("source"))
        .add_input(JobInput::text("French").with_marker("target_language"))
        .add_param("formality", "formal");

    println!("   Inputs:");
    for (i, input) in request.inputs.iter().enumerate() {
        println!(
            "     [{}] {:?} (marker: {:?})",
            i, input.input_type, input.marker
        );
    }

    // 6. Complex Job Chaining Example
    println!("\n6. Job Chaining Example:");
    println!("   Job 1: Extract text from PDF");
    let _job1 = JobRequest::new(5000)? // Text extraction
        .add_input(JobInput::url("https://example.com/paper.pdf"));

    println!("   Job 2: Summarize extracted text (chains from Job 1)");
    let _job2 = JobRequest::new(5001)? // Summarization
        .add_input(JobInput::job("job1_result_event_id", None))
        .add_param("max_length", "500");

    println!("   Job 3: Translate summary (chains from Job 2)");
    let _job3 = JobRequest::new(5002)? // Translation
        .add_input(JobInput::job("job2_result_event_id", None))
        .add_param("target_language", "Spanish");

    println!("\n   Pipeline: PDF → Extract → Summarize → Translate");

    // 7. Parameters for Different Job Types
    println!("\n7. Job-Specific Parameters:");

    println!("\n   Text Generation Parameters:");
    let llm_params = vec![
        JobParam::new("model", "llama3.2"),
        JobParam::new("temperature", "0.7"),
        JobParam::new("max_tokens", "2048"),
        JobParam::new("top_p", "0.9"),
    ];
    for param in &llm_params {
        println!("     {}: {}", param.key, param.value);
    }

    println!("\n   Image Generation Parameters:");
    let image_params = vec![
        JobParam::new("model", "stable-diffusion-xl"),
        JobParam::new("width", "1024"),
        JobParam::new("height", "1024"),
        JobParam::new("steps", "30"),
        JobParam::new("guidance_scale", "7.5"),
    ];
    for param in &image_params {
        println!("     {}: {}", param.key, param.value);
    }

    println!("\n   Translation Parameters:");
    let translation_params = vec![
        JobParam::new("source_language", "auto"),
        JobParam::new("target_language", "French"),
        JobParam::new("formality", "formal"),
    ];
    for param in &translation_params {
        println!("     {}: {}", param.key, param.value);
    }

    // 8. Real-World Example: Multi-Source Document Analysis
    println!("\n8. Real-World Example: Multi-Source Document Analysis");

    let analysis_request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
        .add_input(JobInput::url("https://example.com/report1.pdf").with_marker("report_a"))
        .add_input(JobInput::url("https://example.com/report2.pdf").with_marker("report_b"))
        .add_input(JobInput::event("event_with_context", None).with_marker("context"))
        .add_input(
            JobInput::text("Compare the reports and summarize key differences")
                .with_marker("instruction"),
        )
        .add_param("output_format", "markdown")
        .add_param("max_length", "1000")
        .with_bid(5000);

    println!("   Created request with:");
    println!(
        "     - {} inputs (with markers)",
        analysis_request.inputs.len()
    );
    println!("     - {} parameters", analysis_request.params.len());
    println!("     - Bid: {} millisats", analysis_request.bid.unwrap());

    println!("\n✓ All input types and patterns demonstrated!");

    Ok(())
}
