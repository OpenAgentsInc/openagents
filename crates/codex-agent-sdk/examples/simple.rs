use codex_agent_sdk::{Codex, SandboxMode, ThreadOptions, TurnOptions};

#[tokio::main]
async fn main() -> Result<(), codex_agent_sdk::Error> {
    // Create a new Codex client
    let codex = Codex::new();

    // Configure the thread with read-only sandbox mode
    let options = ThreadOptions {
        sandbox_mode: Some(SandboxMode::ReadOnly),
        ..Default::default()
    };

    // Start a new conversation thread
    let mut thread = codex.start_thread(options);

    // Run a simple query
    let turn = thread.run("What is 2 + 2?", TurnOptions::default()).await?;

    // Print the response
    println!("Response: {}", turn.final_response);

    // Print usage statistics if available
    if let Some(usage) = turn.usage {
        println!("\nUsage:");
        println!("  Input tokens: {}", usage.input_tokens);
        println!("  Cached input tokens: {}", usage.cached_input_tokens);
        println!("  Output tokens: {}", usage.output_tokens);
    }

    // Print the thread ID for potential resumption
    if let Some(id) = thread.id() {
        println!("\nThread ID: {}", id);
    }

    Ok(())
}
