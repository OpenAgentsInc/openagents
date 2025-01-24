use dotenvy::dotenv;
use openagents::server::services::deepseek::{DeepSeekService, ToolChoice};
use serde_json::json;
use std::env;

#[tokio::test]
async fn test_tool_selection() {
    // Load environment variables from .env file
    dotenv().ok();
    
    // Create a real DeepSeek service instance
    let api_key = env::var("DEEPSEEK_API_KEY")
        .expect("DEEPSEEK_API_KEY must be set in .env file");
    let service = DeepSeekService::new(api_key);

    // Create a tool for reading GitHub issues
    let read_issue_tool = DeepSeekService::create_tool(
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
    );

    // Test cases with expected tool usage
    let test_cases = vec![
        (
            "analyze https://github.com/OpenAgentsInc/openagents/issues/596",
            true,  // Should use tool
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
        let (response, _, tool_calls) = service
            .chat_with_tools(
                input.to_string(),
                vec![read_issue_tool.clone()],
                Some(ToolChoice::Auto("auto".to_string())),
                false,
            )
            .await
            .unwrap();

        if should_use_tool {
            assert!(tool_calls.is_some(), "Expected tool call for input: {}", input);
            let tool_calls = tool_calls.unwrap();
            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].function.name, expected_tool);
            
            // Parse the arguments JSON and compare
            let args: serde_json::Value = serde_json::from_str(&tool_calls[0].function.arguments).unwrap();
            assert_eq!(args, expected_args, "Tool arguments don't match for input: {}", input);
        } else {
            assert!(tool_calls.is_none(), "Did not expect tool call for input: {}", input);
            assert!(!response.is_empty(), "Expected non-empty response for input: {}", input);
        }
    }
}