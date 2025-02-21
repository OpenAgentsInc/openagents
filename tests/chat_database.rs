use serde_json::json;
use tracing::info;
use crate::server::{
    models::chat::{CreateConversationRequest, CreateMessageRequest},
    services::chat_database::ChatDatabaseService,
};
use crate::test_utils::setup_test_db;

#[tokio::test]
async fn test_chat_database() {
    // Create chat database service
    let pool = setup_test_db().await;
    let chat_db = ChatDatabaseService::new(pool);

    // Test conversation creation
    info!("Testing conversation creation...");
    let create_conv_req = CreateConversationRequest {
        id: None,
        user_id: "test_user_1".to_string(),
        title: Some("Test Conversation".to_string()),
    };

    let conversation = chat_db
        .create_conversation(&create_conv_req)
        .await
        .expect("Failed to create conversation");

    assert_eq!(conversation.user_id, "test_user_1");
    assert_eq!(conversation.title, Some("Test Conversation".to_string()));
    assert!(conversation.created_at.is_some());
    assert!(conversation.updated_at.is_some());

    // Test message creation
    info!("Testing message creation...");
    let messages = vec![
        CreateMessageRequest {
            conversation_id: conversation.id,
            user_id: conversation.user_id.clone(),
            role: "user".to_string(),
            content: "Hello!".to_string(),
            reasoning: None,
            metadata: None,
            tool_calls: None,
        },
        CreateMessageRequest {
            conversation_id: conversation.id,
            user_id: conversation.user_id.clone(),
            role: "assistant".to_string(),
            content: "Hi there!".to_string(),
            reasoning: None,
            metadata: Some(json!({ "response_type": "greeting" })),
            tool_calls: Some(json!([{
                "name": "test_tool",
                "arguments": { "arg1": "value1" }
            }])),
        },
    ];
}
