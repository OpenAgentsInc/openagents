use dotenvy::dotenv;
use openagents::server::services::deepseek::{ChatMessage, DeepSeekService, ToolChoice};
use serde_json::json;
use std::env;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::test]
async fn test_tool_selection() {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Load environment variables from .env file
    dotenv().ok();

    // Create a real DeepSeek service instance
    let api_key = env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set in .env file");
    let service = DeepSeekService::new(api_key);

    // Create a tool for reading GitHub issues
    let read_issue_tool = DeepSeekService::create_tool(
        "read_github_issue".to_string(),
        Some("Read a GitHub issue by number. Note: The repository owner is 'OpenAgentsInc' (case-sensitive) and the repository name is 'openagents' (lowercase).".to_string()),
        json!({
            "type": "object",
            "properties": {
                "owner": {
                    "type": "string",
                    "description": "The owner of the repository (must be 'OpenAgentsInc')"
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository (must be 'openagents' in lowercase)"
                },
                "issue_number": {
                    "type": "integer",
                    "description": "The issue number"
                }
            },
            "required": ["owner", "repo", "issue_number"]
        }),
    );

    info!("Tool definition: {:?}", read_issue_tool);

    // Test cases with expected tool usage
    let test_cases = vec![
        (
            "analyze https://github.com/OpenAgentsInc/openagents/issues/596",
            true, // Should use tool
            "read_github_issue",
            json!({
                "owner": "OpenAgentsInc",
                "repo": "openagents",
                "issue_number": 596
            }),
        ),
        (
            "Hello, how are you?",
            false, // Should not use tool
            "",
            json!({}),
        ),
        (
            "Can you check issue #595 in the OpenAgents repo?",
            true, // Should use tool
            "read_github_issue",
            json!({
                "owner": "OpenAgentsInc",
                "repo": "openagents",
                "issue_number": 595
            }),
        ),
    ];

    for (input, should_use_tool, expected_tool, expected_args) in test_cases {
        info!("\n\nTesting input: {}", input);
        info!("Expected tool usage: {}", should_use_tool);
        if should_use_tool {
            info!("Expected tool: {}", expected_tool);
            info!("Expected args: {}", expected_args);
        }

        // Create messages with system context
        let _messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a helpful assistant that reads GitHub issues. When referring to the repository, always use 'OpenAgentsInc' as the owner and 'openagents' (lowercase) as the repository name.".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: input.to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        let (response, _, tool_calls) = service
            .chat_with_tools(
                input.to_string(),
                vec![read_issue_tool.clone()],
                Some(ToolChoice::Auto("auto".to_string())),
                false,
            )
            .await
            .unwrap();

        info!("Response: {}", response);

        if let Some(ref calls) = tool_calls {
            info!("Tool calls received: {:#?}", calls);
            for call in calls {
                info!("Tool call:");
                info!("  Name: {}", call.function.name);
                info!("  Arguments: {}", call.function.arguments);
                if should_use_tool {
                    let args: serde_json::Value =
                        serde_json::from_str(&call.function.arguments).unwrap();
                    info!("  Parsed arguments: {:#?}", args);
                    info!("  Expected arguments: {:#?}", expected_args);
                }
            }
        } else {
            info!("No tool calls received");
        }

        if should_use_tool {
            assert!(
                tool_calls.is_some(),
                "Expected tool call for input: {}",
                input
            );
            let tool_calls = tool_calls.unwrap();
            assert_eq!(tool_calls.len(), 1, "Expected exactly one tool call");
            assert_eq!(
                tool_calls[0].function.name, expected_tool,
                "Tool name mismatch"
            );

            // Parse the arguments JSON and compare
            let args: serde_json::Value =
                serde_json::from_str(&tool_calls[0].function.arguments).unwrap();
            assert_eq!(
                args, expected_args,
                "Tool arguments don't match for input: {}\nReceived: {:#?}\nExpected: {:#?}",
                input, args, expected_args
            );
        } else {
            assert!(
                tool_calls.is_none(),
                "Did not expect tool call for input: {}",
                input
            );
            assert!(
                !response.is_empty(),
                "Expected non-empty response for input: {}",
                input
            );
        }
    }
}

#[tokio::test]
async fn test_routing_decision() {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Load environment variables from .env file
    dotenv().ok();

    // Create a real DeepSeek service instance
    let api_key = env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set in .env file");
    let service = DeepSeekService::new(api_key);

    // System prompt for routing decisions
    let system_prompt = r#"You are a routing assistant that determines whether a user message requires tool usage.
Analyze the user's message and respond with a JSON object containing:
1. "needs_tool": boolean - whether any tools are needed
2. "reasoning": string - brief explanation of your decision
3. "suggested_tool": string | null - name of suggested tool if applicable

Always respond with valid JSON matching this format."#;

    // Test cases for routing decisions
    let test_cases = vec![
        (
            "Can you check issue #595?",
            json!({
                "needs_tool": true,
                "reasoning": "User is requesting to view a GitHub issue",
                "suggested_tool": "read_github_issue"
            }),
        ),
        (
            "Hello, how are you today?",
            json!({
                "needs_tool": false,
                "reasoning": "General chat message that doesn't require tools",
                "suggested_tool": null
            }),
        ),
    ];

    for (input, expected_decision) in test_cases {
        info!("\n\nTesting routing for input: {}", input);
        info!("Expected decision: {}", expected_decision);

        let (response, _, _) = service
            .chat_with_tools(
                input.to_string(),
                vec![], // No tools needed for routing decision
                None,
                false,
            )
            .await
            .unwrap();

        info!("Response: {}", response);

        // Parse the response as JSON
        let decision: serde_json::Value = serde_json::from_str(&response)
            .expect("Response should be valid JSON");

        // Verify the decision structure
        assert!(decision.get("needs_tool").is_some(), "Missing needs_tool field");
        assert!(decision.get("reasoning").is_some(), "Missing reasoning field");
        assert!(decision.get("suggested_tool").is_some(), "Missing suggested_tool field");

        // Verify the needs_tool field is a boolean
        assert!(decision["needs_tool"].is_boolean(), "needs_tool should be a boolean");

        // Verify the reasoning field is a non-empty string
        assert!(decision["reasoning"].is_string(), "reasoning should be a string");
        assert!(!decision["reasoning"].as_str().unwrap().is_empty(), "reasoning should not be empty");

        // Verify the suggested_tool field is either a string or null
        assert!(
            decision["suggested_tool"].is_null() || decision["suggested_tool"].is_string(),
            "suggested_tool should be a string or null"
        );

        // Compare with expected decision
        assert_eq!(
            decision["needs_tool"], expected_decision["needs_tool"],
            "needs_tool mismatch"
        );
        
        if decision["needs_tool"].as_bool().unwrap() {
            assert_eq!(
                decision["suggested_tool"], expected_decision["suggested_tool"],
                "suggested_tool mismatch for tool-requiring message"
            );
        }
    }
}