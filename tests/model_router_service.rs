use dotenvy::dotenv;
use openagents::server::services::{
    deepseek::{DeepSeekService, Tool, ToolChoice},
    model_router::ModelRouter,
};
use serde_json::json;
use std::{env, sync::Arc};
use tracing::{info, Level};
use tracing_subscriber;

// Helper function to initialize logging once
fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .try_init();
}

// Helper function to create test tools
fn create_test_tools() -> Vec<Tool> {
    vec![
        // GitHub issue tool
        DeepSeekService::create_tool(
            "read_github_issue".to_string(),
            Some("Read a GitHub issue by number".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "owner": {
                        "type": "string",
                        "description": "The owner of the repository"
                    },
                    "repo": {
                        "type": "string",
                        "description": "The name of the repository"
                    },
                    "issue_number": {
                        "type": "integer",
                        "description": "The issue number"
                    }
                },
                "required": ["owner", "repo", "issue_number"]
            }),
        ),
        // Calculator tool
        DeepSeekService::create_tool(
            "calculate".to_string(),
            Some("Perform a calculation".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            }),
        ),
    ]
}

#[tokio::test]
async fn test_model_router_service() {
    // Initialize logging
    init_logging();

    // Load environment variables from .env file
    dotenv().ok();

    // Create DeepSeek services
    let api_key = env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set in .env file");
    let tool_model = Arc::new(DeepSeekService::new(api_key.clone()));
    let chat_model = Arc::new(DeepSeekService::new(api_key));

    // Create available tools
    let tools = create_test_tools();

    // Create model router
    let router = ModelRouter::new(tool_model, chat_model, tools);

    // Test cases
    let test_cases = vec![
        (
            "Can you check issue #595?",
            true,
            Some("read_github_issue".to_string()),
            "requesting to view a GitHub issue",
        ),
        (
            "What is 2 + 2?",
            true,
            Some("calculate".to_string()),
            "requesting a calculation",
        ),
        (
            "Hello, how are you today?",
            false,
            None,
            "general chat message",
        ),
        (
            "Tell me a joke",
            false,
            None,
            "general chat request",
        ),
    ];

    for (input, should_use_tool, expected_tool, expected_reasoning) in test_cases {
        info!("\n\nTesting routing for input: {}", input);

        let (decision, tool_calls) = router.route_message(input.to_string()).await.unwrap();

        // Verify routing decision
        assert_eq!(
            decision.needs_tool, should_use_tool,
            "needs_tool mismatch for input: {}",
            input
        );

        // Verify reasoning is present and meaningful
        assert!(!decision.reasoning.is_empty(), "reasoning should not be empty");
        assert!(
            decision.reasoning.to_lowercase().contains(expected_reasoning),
            "reasoning should contain '{}', got: {}",
            expected_reasoning,
            decision.reasoning
        );

        // Verify tool suggestion
        assert_eq!(
            decision.suggested_tool, expected_tool,
            "suggested_tool mismatch for input: {}",
            input
        );

        // If tools should be used, verify tool calls
        if should_use_tool {
            assert!(tool_calls.is_some(), "Expected tool calls for input: {}", input);
            let tool_calls = tool_calls.unwrap();
            assert_eq!(tool_calls.len(), 1, "Expected exactly one tool call");
            assert_eq!(
                tool_calls[0].function.name,
                expected_tool.unwrap(),
                "Tool name mismatch"
            );
        } else {
            assert!(tool_calls.is_none(), "Did not expect tool calls for input: {}", input);
        }
    }
}

#[tokio::test]
async fn test_model_router_chat() {
    // Initialize logging
    init_logging();

    // Load environment variables from .env file
    dotenv().ok();

    // Create DeepSeek services
    let api_key = env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set in .env file");
    let tool_model = Arc::new(DeepSeekService::new(api_key.clone()));
    let chat_model = Arc::new(DeepSeekService::new(api_key));

    // Create model router with empty tools
    let router = ModelRouter::new(tool_model, chat_model, vec![]);

    // Test general chat
    let (response, reasoning) = router
        .chat("Tell me about the weather".to_string(), true)
        .await
        .unwrap();

    // Verify response
    assert!(!response.is_empty(), "Response should not be empty");
    assert!(
        reasoning.is_some(),
        "Reasoning should be present when requested"
    );

    // Test without reasoning
    let (response, reasoning) = router
        .chat("How are you?".to_string(), false)
        .await
        .unwrap();

    assert!(!response.is_empty(), "Response should not be empty");
    assert!(reasoning.is_none(), "Reasoning should not be present when not requested");
}

#[tokio::test]
async fn test_model_router_tool_execution() {
    // Initialize logging
    init_logging();

    // Load environment variables from .env file
    dotenv().ok();

    // Create DeepSeek services
    let api_key = env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set in .env file");
    let tool_model = Arc::new(DeepSeekService::new(api_key.clone()));
    let chat_model = Arc::new(DeepSeekService::new(api_key));

    // Create test tool
    let test_tool = DeepSeekService::create_tool(
        "test_tool".to_string(),
        Some("A test tool".to_string()),
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "A test message"
                }
            },
            "required": ["message"]
        }),
    );

    // Create model router
    let router = ModelRouter::new(tool_model, chat_model, vec![test_tool.clone()]);

    // Test tool execution
    let (response, _, tool_calls) = router
        .execute_tool_call("Run a test".to_string(), test_tool)
        .await
        .unwrap();

    // Verify response
    assert!(!response.is_empty(), "Response should not be empty");
    assert!(
        tool_calls.is_some(),
        "Tool calls should be present for tool execution"
    );
}