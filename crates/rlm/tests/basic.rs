//! Basic integration tests for the RLM engine.
//!
//! These tests use the MockExecutor for isolated testing.

use rlm::{Command, MockExecutor, RlmConfig};

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
        ..Default::default()
    };

    assert_eq!(config.max_iterations, 5);
    assert!(config.allow_shell);
    assert!(config.verbose);
}

/// Test that prompts are constructed correctly.
#[test]
fn test_prompts() {
    use rlm::{GUIDED_SYSTEM_PROMPT, PromptTier, SYSTEM_PROMPT};

    // Basic system prompt should contain key instructions
    assert!(SYSTEM_PROMPT.contains("FINAL"));
    assert!(SYSTEM_PROMPT.contains("repl"));
    assert!(SYSTEM_PROMPT.contains("MUST use code"));

    // Guided prompt should NOT contain llm_query (Apple FM can't do meta-reasoning)
    assert!(!GUIDED_SYSTEM_PROMPT.contains("llm_query"));
    assert!(GUIDED_SYSTEM_PROMPT.contains("Do NOT import"));

    // PromptTier should default to Full
    assert_eq!(PromptTier::default(), PromptTier::Full);
}
