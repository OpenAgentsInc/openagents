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
async fn test_routing_decision() {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Load environment variables from .env file
    dotenv().ok();

    // Create mock server
    let mock_server = MockServer::start().await;

    // System prompt for routing decisions
    let system_message = ChatMessage {
        role: "system".to_string(),
        content: r#"You are a routing assistant that determines whether a user message requires tool usage.
DO NOT USE ANY TOOLS DIRECTLY. Instead, analyze the user's message and respond with a JSON object containing:
1. "needs_tool": boolean - whether any tools are needed
2. "reasoning": string - brief explanation of your decision
3. "suggested_tool": string | null - name of suggested tool if applicable

IMPORTANT: Your response must be a valid JSON object and nothing else.

Example responses:
{
    "needs_tool": true,
    "reasoning": "User is requesting to view a GitHub issue",
    "suggested_tool": "read_github_issue"
}

{
    "needs_tool": false,
    "reasoning": "General chat message that doesn't require tools",
    "suggested_tool": null
}

Remember: Only respond with a JSON object, do not use any tools, and do not add any additional text."#.to_string(),
        tool_call_id: None,
        tool_calls: None,
    };

    // Create dummy tool
    let dummy_tool = DeepSeekService::create_tool(
        "dummy_tool".to_string(),
        Some("A dummy tool for testing routing decisions".to_string()),
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "A test message"
                }
            }
        }),
    );

    // Test cases with their corresponding mock responses
    let test_cases = vec![
        (
            "Can you check issue #595?",
            json!({
                "needs_tool": true,
                "reasoning": "User is requesting to view a GitHub issue",
                "suggested_tool": "read_github_issue"
            }),
            json!({
                "choices": [{
                    "message": {
                        "content": "{\"needs_tool\":true,\"reasoning\":\"User is requesting to view a GitHub issue\",\"suggested_tool\":\"read_github_issue\"}",
                        "role": "assistant"
                    }
                }]
            }),
        ),
        (
            "Hello, how are you today?",
            json!({
                "needs_tool": false,
                "reasoning": "General chat message that doesn't require tools",
                "suggested_tool": null
            }),
            json!({
                "choices": [{
                    "message": {
                        "content": "{\"needs_tool\":false,\"reasoning\":\"General chat message that doesn't require tools\",\"suggested_tool\":null}",
                        "role": "assistant"
                    }
                }]
            }),
        ),
    ];

    // Create service with mock server
    let service = DeepSeekService::with_base_url("test_key".to_string(), mock_server.uri());

    for (input, expected_decision, mock_response) in test_cases {
        info!("\n\nTesting routing for input: {}", input);
        info!("Expected decision: {}", expected_decision);

        // Set up mock for this specific test case
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("content-type", "application/json"))
            .and(body_string_contains(input)) // Match based on the input text
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Create messages with system context and user input
        let messages = vec![
            system_message.clone(),
            ChatMessage {
                role: "user".to_string(),
                content: input.to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        let (response, _, _) = service
            .chat_with_tools_messages(
                messages,
                vec![dummy_tool.clone()],
                Some(ToolChoice::Auto("auto".to_string())),
                false,
            )
            .await
            .unwrap();

        info!("Response: {}", response);

        // Parse the response as JSON
        let decision: serde_json::Value =
            serde_json::from_str(&response).expect("Response should be valid JSON");

        // Log the comparison
        info!("Expected decision: {}", expected_decision);
        info!("Actual decision: {}", decision);

        // Verify the decision structure
        assert!(
            decision.get("needs_tool").is_some(),
            "Missing needs_tool field"
        );
        assert!(
            decision.get("reasoning").is_some(),
            "Missing reasoning field"
        );
        assert!(
            decision.get("suggested_tool").is_some(),
            "Missing suggested_tool field"
        );

        // Verify the needs_tool field is a boolean
        assert!(
            decision["needs_tool"].is_boolean(),
            "needs_tool should be a boolean"
        );

        // Verify the reasoning field is a non-empty string
        assert!(
            decision["reasoning"].is_string(),
            "reasoning should be a string"
        );
        assert!(
            !decision["reasoning"].as_str().unwrap().is_empty(),
            "reasoning should not be empty"
        );

        // Verify the suggested_tool field is either a string or null
        assert!(
            decision["suggested_tool"].is_null() || decision["suggested_tool"].is_string(),
            "suggested_tool should be a string or null"
        );

        // Compare with expected decision
        assert_eq!(
            decision["needs_tool"], expected_decision["needs_tool"],
            "needs_tool mismatch for input '{}'. Expected {:?}, got {:?}",
            input, expected_decision["needs_tool"], decision["needs_tool"]
        );

        if decision["needs_tool"].as_bool().unwrap() {
            assert_eq!(
                decision["suggested_tool"], expected_decision["suggested_tool"],
                "suggested_tool mismatch for input '{}'. Expected {:?}, got {:?}",
                input, expected_decision["suggested_tool"], decision["suggested_tool"]
            );
        }
    }
}
