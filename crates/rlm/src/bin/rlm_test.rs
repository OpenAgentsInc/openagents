//! Simple CLI to test RLM with FM Bridge.
//!
//! Usage: cargo run -p rlm --bin rlm_test "What is 15 * 23?"

use fm_bridge::FMClient;
use rlm::{MockExecutor, RlmConfig, RlmEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get query from args or use default
    let query = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "What is 15 * 23?".to_string());

    println!("=== RLM Test ===\n");
    println!("Query: {}\n", query);

    // Check if FM Bridge is available (pylon runs it on 11435)
    println!("Connecting to FM Bridge...");
    let client = FMClient::with_base_url("http://localhost:11435")?;

    match client.health().await {
        Ok(true) => println!("FM Bridge: OK\n"),
        Ok(false) => {
            println!("FM Bridge: NOT HEALTHY");
            println!("\nPlease start FM Bridge first:");
            println!("  pylon-desktop --cli");
            return Ok(());
        }
        Err(e) => {
            println!("FM Bridge: CONNECTION FAILED - {}", e);
            println!("\nPlease start FM Bridge first:");
            println!("  pylon-desktop --cli");
            return Ok(());
        }
    }

    // Create mock executor that simulates Python execution
    let executor = MockExecutor::new()
        .expect("15 * 23", "345")
        .expect("15*23", "345")
        .expect("2 + 2", "4")
        .expect("2+2", "4")
        .expect("sqrt", "12.0")
        .expect("print", "[Code executed - output captured]")
        .expect("summary", "Summary: OpenAgents builds infrastructure for autonomous AI agents with identity, payments, and marketplaces.")
        .expect("openagents", "OpenAgents: AI agent economy OS - identity, payments, markets on Bitcoin/Nostr")
        .default_response("[Executed] No explicit output");

    // Create engine with config
    let config = RlmConfig {
        max_iterations: 5,
        allow_shell: false,
        verbose: true,
        ..Default::default()
    };

    let engine = RlmEngine::with_config(client.clone(), executor, config);

    println!("Starting RLM loop...\n");
    println!("{}", "=".repeat(60));

    // First, let's see what the LLM actually responds with
    println!("\n--- Testing raw LLM response ---\n");
    let test_prompt = format!("You are an AI that can execute code. Answer this: {}\n\nRespond with FINAL <answer> when done.", query);
    match client.complete(&test_prompt, None).await {
        Ok(resp) => {
            let text = resp.choices.first().map(|c| &c.message.content).unwrap();
            println!("Raw LLM response:\n{}\n", text);
        }
        Err(e) => println!("LLM error: {}", e),
    }
    println!("{}", "=".repeat(60));

    match engine.run(&query).await {
        Ok(result) => {
            println!("{}", "=".repeat(60));
            println!("\n=== RESULT ===\n");
            println!("Output: {}", result.output);
            println!("Iterations: {}", result.iterations);

            if !result.execution_log.is_empty() {
                println!("\n=== Execution Log ===\n");
                for entry in &result.execution_log {
                    println!("--- Iteration {} ---", entry.iteration);
                    println!("Command: {}", entry.command_type);
                    if !entry.executed.is_empty() {
                        println!("Executed: {}", entry.executed.chars().take(100).collect::<String>());
                    }
                    println!("Result: {}", entry.result.chars().take(200).collect::<String>());
                    println!();
                }
            }
        }
        Err(e) => {
            println!("{}", "=".repeat(60));
            println!("\n=== ERROR ===\n");
            println!("{}", e);
        }
    }

    Ok(())
}
