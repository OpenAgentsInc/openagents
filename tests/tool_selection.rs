use dotenvy::dotenv;
use openagents::server::services::deepseek::{ChatMessage, DeepSeekService, ToolChoice};
use serde_json::json;
use tracing::{info, Level};
use tracing_subscriber;
use wiremock::{
    matchers::{body_string_contains, header, method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn test_tool_selection() {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Load environment variables from .env file
    dotenv().ok();

    // Create mock server
    let mock_server = MockServer::start().await;

    // Create DeepSeek service with mock server
    let service = DeepSeekService::with_base_url("test_key".to_string(), mock_server.uri());

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

    // Test cases with expected tool usage and mock responses
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
            json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "",
                        "tool_calls": [{
                            "id": "call_123",
                            "type": "function",
                            "function": {
                                "name": "read_github_issue",
                                "arguments": "{\"owner\":\"OpenAgentsInc\",\"repo\":\"openagents\",\"issue_number\":596}"
                            }
                        }]
                    }
                }]
            }),
        ),
        (
            "Hello, how are you?",
            false, // Should not use tool
            "",
            json!({}),
            json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "Hello! I'm doing well, thank you for asking. How can I help you today?"
                    }
                }]
            }),
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
            json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "",
                        "tool_calls": [{
                            "id": "call_124",
                            "type": "function",
                            "function": {
                                "name": "read_github_issue",
                                "arguments": "{\"owner\":\"OpenAgentsInc\",\"repo\":\"openagents\",\"issue_number\":595}"
                            }
                        }]
                    }
                }]
            }),
        ),
    ];

    for (input, should_use_tool, expected_tool, expected_args, mock_response) in test_cases {
        info!("\n\nTesting input: {}", input);
        info!("Expected tool usage: {}", should_use_tool);
        if should_use_tool {
            info!("Expected tool: {}", expected_tool);
            info!("Expected args: {}", expected_args);
        }

        // Set up mock for this test case
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("content-type", "application/json"))
            .and(body_string_contains(input))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
            .expect(1)
            .mount(&mock_server)
            .await;

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
