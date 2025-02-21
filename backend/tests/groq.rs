use openagents::server::services::gateway::Gateway;
use openagents::server::services::groq::GroqService;

#[tokio::test]
async fn test_groq_metadata() {
    let service = GroqService::new("test-key".to_string());
    let metadata = service.metadata();
    assert_eq!(metadata.name, "Groq");
    assert!(metadata.openai_compatible);
    assert!(metadata.supported_features.contains(&"chat".to_string()));
    assert!(metadata
        .supported_features
        .contains(&"streaming".to_string()));
}

#[tokio::test]
async fn test_groq_chat() {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY must be set");
    let service = GroqService::new(api_key);

    let (response, _) = service.chat("Say hello".to_string(), false).await.unwrap();

    assert!(!response.is_empty());
}

// TODO: Add streaming test once implemented
// #[tokio::test]
// async fn test_groq_chat_stream() {
//     let service = GroqService::new("test-key".to_string());
//     let stream = service.chat_stream("Hello".to_string(), false).await.unwrap();
//     // Test streaming functionality
// }
