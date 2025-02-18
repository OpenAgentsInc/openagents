use super::*;
use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

#[tokio::test]
async fn test_model_router_tool_execution() {
    // Initialize logging
    init_logging();

    // Load environment variables from .env file
    dotenv().ok();

    // Create mock router
    let (router, mock_server) = create_mock_router().await;

    // Create test tool
    let test_tool = DeepSeekService::create_tool(
        "test_tool".to_string(),
        Some("A test tool that takes a message and returns it".to_string()),
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "A test message to echo back"
                }
            },
            "required": ["message"]
        }),
    );

    // Set up mock for tool execution
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": {
                    "content": "",
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {
                            "name": "test_tool",
                            "arguments": "{\"message\":\"Hello World\"}"
                        }
                    }]
                }
            }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    // Test tool execution with a clear instruction
    let (response, _, tool_calls) = router
        .execute_tool_call(
            "Please echo back the message 'Hello World'".to_string(),
            test_tool,
        )
        .await
        .unwrap();

    // Verify response
    assert!(!response.is_empty(), "Response should not be empty");
    assert!(
        tool_calls.is_some(),
        "Tool calls should be present for tool execution"
    );

    // Verify tool call
    let tool_calls = tool_calls.unwrap();
    assert_eq!(tool_calls.len(), 1, "Expected exactly one tool call");
    assert_eq!(tool_calls[0].function.name, "test_tool");

    // Parse tool call arguments
    let args: serde_json::Value = serde_json::from_str(&tool_calls[0].function.arguments).unwrap();
    assert!(
        args.get("message").is_some(),
        "Tool call should include message parameter"
    );
}