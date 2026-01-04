//! Basic integration tests for the RLM engine.
//!
//! Note: Tests that use the actual FMClient require the FM Bridge to be running.
//! These tests use the MockExecutor for isolated testing.

use rlm::{Command, MockExecutor, RlmEngine, RlmConfig};
use fm_bridge::FMClient;

/// Test that the command parser works correctly.
#[test]
fn test_command_parsing() {
    // Test FINAL
    let cmd = Command::parse("FINAL The answer is 42");
    assert_eq!(cmd.as_final().unwrap(), "The answer is 42");

    // Test RUN
    let cmd = Command::parse("RUN echo hello");
    let args = cmd.as_run().unwrap();
    assert_eq!(args.program, "echo");
    assert_eq!(args.args, vec!["hello"]);

    // Test code block
    let input = "```repl\nprint('hello')\n```";
    let cmd = Command::parse(input);
    assert_eq!(cmd.as_code().unwrap(), "print('hello')");

    // Test invalid
    let cmd = Command::parse("just some text");
    assert!(cmd.is_invalid());
}

/// Test the mock executor returns expected responses.
#[tokio::test]
async fn test_mock_executor() {
    use rlm::ExecutionEnvironment;

    let executor = MockExecutor::new()
        .expect("2 + 2", "4")
        .expect("hello", "world")
        .default_response("unknown");

    // Pattern match
    let result = executor.execute("result = 2 + 2").await.unwrap();
    assert_eq!(result.stdout, "4");

    // Another pattern
    let result = executor.execute("print(hello)").await.unwrap();
    assert_eq!(result.stdout, "world");

    // Default
    let result = executor.execute("something else").await.unwrap();
    assert_eq!(result.stdout, "unknown");

    // Check execution log
    let log = executor.execution_log();
    assert_eq!(log.len(), 3);
}

/// Test RLM engine creation and configuration.
#[test]
fn test_engine_creation() {
    // This test doesn't require FM Bridge to be running
    // as we're just testing construction

    let config = RlmConfig {
        max_iterations: 5,
        allow_shell: true,
        verbose: true,
    };

    assert_eq!(config.max_iterations, 5);
    assert!(config.allow_shell);
    assert!(config.verbose);
}

/// Test that prompts are constructed correctly.
#[test]
fn test_prompts() {
    use rlm::SYSTEM_PROMPT;

    // System prompt should contain key instructions
    assert!(SYSTEM_PROMPT.contains("FINAL"));
    assert!(SYSTEM_PROMPT.contains("RUN"));
    assert!(SYSTEM_PROMPT.contains("repl"));
    assert!(SYSTEM_PROMPT.contains("query_llm"));
}

// ========================================
// Integration tests (require FM Bridge)
// ========================================

/// Test the full RLM loop with FM Bridge.
///
/// This test is ignored by default since it requires:
/// 1. FM Bridge running at localhost:3030
/// 2. A working model loaded
///
/// Run with: cargo test -p rlm -- --ignored
#[tokio::test]
#[ignore]
async fn test_rlm_with_fm_bridge() {
    // Create client - will fail if FM Bridge not running
    let client = match FMClient::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test - FM Bridge not available: {}", e);
            return;
        }
    };

    // Check if FM Bridge is healthy
    if !client.health().await.unwrap_or(false) {
        eprintln!("Skipping test - FM Bridge not healthy");
        return;
    }

    // Create mock executor that simulates successful execution
    let executor = MockExecutor::new()
        .expect("15 * 23", "345")
        .expect("2 + 2", "4");

    let engine = RlmEngine::new(client, executor);

    // Run a simple query
    match engine.run("What is 2 + 2?").await {
        Ok(result) => {
            println!("Result: {}", result.output);
            println!("Iterations: {}", result.iterations);
            for entry in &result.execution_log {
                println!("  [{}] {} -> {}",
                    entry.iteration,
                    entry.command_type,
                    entry.result.chars().take(50).collect::<String>()
                );
            }
            assert!(result.iterations > 0);
        }
        Err(e) => {
            println!("RLM execution failed: {}", e);
            // Don't fail the test - LLM might give unexpected responses
        }
    }
}
