use super::*;
use tracing::info;
use wiremock::{
    matchers::{body_string_contains, header, method, path},
    Mock, ResponseTemplate,
};

#[tokio::test]
async fn test_model_router_service() {
    // Initialize logging
    init_logging();

    // Load environment variables from .env file
    dotenv().ok();

    // Create mock router
    let (router, mock_server) = create_mock_router().await;

    // Test cases
    let test_cases = vec![
        (
            "Can you check issue #595?",
            true,
            Some("read_github_issue".to_string()),
            vec!["requesting", "github issue"],
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
            "What is 2 + 2?",
            true,
            Some("calculate".to_string()),
            vec!["calculation", "mathematical", "compute"],
            json!({
                "choices": [{
                    "message": {
                        "content": "{\"needs_tool\":true,\"reasoning\":\"requesting a calculation\",\"suggested_tool\":\"calculate\"}",
                        "role": "assistant"
                    }
                }]
            }),
        ),
        (
            "Hello, how are you today?",
            false,
            None,
            vec!["general chat", "conversation", "no tool"],
            json!({
                "choices": [{
                    "message": {
                        "content": "{\"needs_tool\":false,\"reasoning\":\"General chat message that doesn't require tools\",\"suggested_tool\":null}",
                        "role": "assistant"
                    }
                }]
            }),
        ),
        (
            "Tell me a joke",
            false,
            None,
            vec!["general", "chat", "no tool"],
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

    for (input, should_use_tool, expected_tool, expected_phrases, mock_response) in test_cases {
        info!("\n\nTesting routing for input: {}", input);

        // Set up mock for this test case
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("content-type", "application/json"))
            .and(body_string_contains(input))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
            .expect(1)
            .mount(&mock_server)
            .await;

        let (decision, tool_calls) = router.route_message(input.to_string()).await.unwrap();

        // Verify routing decision
        assert_eq!(
            decision.needs_tool, should_use_tool,
            "needs_tool mismatch for input: {}",
            input
        );

        // Verify reasoning is present and meaningful
        assert!(
            !decision.reasoning.is_empty(),
            "reasoning should not be empty"
        );
        let reasoning_lower = decision.reasoning.to_lowercase();
        let found_phrase = expected_phrases
            .iter()
            .any(|phrase| reasoning_lower.contains(&phrase.to_lowercase()));
        assert!(
            found_phrase,
            "reasoning '{}' should contain one of {:?}",
            reasoning_lower, expected_phrases
        );

        // Verify tool suggestion
        assert_eq!(
            decision.suggested_tool, expected_tool,
            "suggested_tool mismatch for input: {}",
            input
        );

        // If tools should be used, verify tool calls
        if should_use_tool {
            assert!(
                tool_calls.is_some(),
                "Expected tool calls for input: {}",
                input
            );
            let tool_calls = tool_calls.unwrap();
            assert_eq!(tool_calls.len(), 1, "Expected exactly one tool call");
            assert_eq!(
                tool_calls[0].function.name,
                expected_tool.unwrap(),
                "Tool name mismatch"
            );
        } else {
            assert!(
                tool_calls.is_none(),
                "Did not expect tool calls for input: {}",
                input
            );
        }
    }
}