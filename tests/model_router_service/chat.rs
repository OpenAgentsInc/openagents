use super::*;
use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

#[tokio::test]
async fn test_model_router_chat() {
    // Initialize logging
    init_logging();

    // Load environment variables from .env file
    dotenv().ok();

    // Create mock router
    let (router, mock_server) = create_mock_router().await;

    // Set up mock for general chat
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": {
                    "content": "Here's some information about the weather...",
                    "role": "assistant"
                }
            }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    // Test general chat
    let (response, reasoning) = router
        .chat("Tell me about the weather".to_string(), false)
        .await
        .unwrap();

    // Verify response
    assert!(!response.is_empty(), "Response should not be empty");
    assert!(
        reasoning.is_none(),
        "Reasoning should not be present when not requested"
    );

    // Set up mock for chat with reasoning
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": {
                    "content": "I'm doing well, thank you for asking!",
                    "role": "assistant"
                }
            }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    // Test with reasoning
    let (response, reasoning) = router.chat("How are you?".to_string(), true).await.unwrap();

    assert!(!response.is_empty(), "Response should not be empty");
    assert!(
        reasoning.is_some(),
        "Reasoning should be present when requested"
    );
}