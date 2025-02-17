use dotenvy::dotenv;
use openagents::server::{
    models::{chat::*, timestamp::Timestamp},
    services::chat_database::ChatDatabaseService,
};
use serde_json::json;
use sqlx::PgPool;
use tracing::{info, Level};
use tracing_subscriber::fmt::format::FmtSpan;

#[tokio::test]
async fn test_chat_persistence() {
    // Initialize logging with custom format
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_test_writer()
        .with_span_events(FmtSpan::NONE)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    // Load environment variables
    dotenv().ok();

    // Set up database connection
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Clean up any existing test data
    sqlx::query!("DELETE FROM conversations WHERE user_id LIKE 'test_user_%'")
        .execute(&pool)
        .await
        .expect("Failed to clean up existing test data");

    // Create ChatDatabase instance
    let chat_db = ChatDatabaseService::new(pool.clone());

    // Test conversation creation
    info!("Testing conversation creation...");
    let create_conv_req = CreateConversationRequest {
        user_id: "test_user_1".to_string(),
        title: Some("Test Conversation".to_string()),
    };

    let conversation = chat_db
        .create_conversation(&create_conv_req)
        .await
        .expect("Failed to create conversation");

    assert_eq!(conversation.user_id, "test_user_1");
    assert_eq!(
        conversation.title.as_deref(),
        Some("Test Conversation"),
        "Title mismatch"
    );
    assert!(
        conversation.created_at.is_some(),
        "created_at should be set"
    );
    assert!(
        conversation.updated_at.is_some(),
        "updated_at should be set"
    );

    // Test message creation
    info!("Testing message creation...");
    let messages = vec![
        CreateMessageRequest {
            conversation_id: conversation.id,
            role: "user".to_string(),
            content: "Hello!".to_string(),
            metadata: None,
            tool_calls: None,
        },
        CreateMessageRequest {
            conversation_id: conversation.id,
            role: "assistant".to_string(),
            content: "Hi there!".to_string(),
            metadata: Some(json!({ "response_type": "greeting" })),
            tool_calls: Some(json!([{
                "name": "test_tool",
                "arguments": { "arg1": "value1" }
            }])),
        },
    ];

    for msg_req in messages {
        let message = chat_db
            .create_message(&msg_req)
            .await
            .expect("Failed to create message");

        assert_eq!(message.conversation_id, conversation.id);
        assert_eq!(message.role, msg_req.role);
        assert_eq!(message.content, msg_req.content);
        assert_eq!(message.metadata, msg_req.metadata);
        assert_eq!(message.tool_calls, msg_req.tool_calls);
        assert!(message.created_at.is_some());
    }

    // Test conversation retrieval
    info!("Testing conversation retrieval...");
    let retrieved_conv = chat_db
        .get_conversation(conversation.id)
        .await
        .expect("Failed to retrieve conversation");

    assert_eq!(retrieved_conv.id, conversation.id);
    assert_eq!(retrieved_conv.user_id, conversation.user_id);
    assert_eq!(retrieved_conv.title, conversation.title);

    // Test message retrieval
    info!("Testing message retrieval...");
    let retrieved_msgs = chat_db
        .get_conversation_messages(conversation.id)
        .await
        .expect("Failed to retrieve messages");

    assert_eq!(retrieved_msgs.len(), 2, "Should have 2 messages");
    assert_eq!(retrieved_msgs[0].role, "user");
    assert_eq!(retrieved_msgs[0].content, "Hello!");
    assert_eq!(retrieved_msgs[1].role, "assistant");
    assert_eq!(retrieved_msgs[1].content, "Hi there!");

    // Test user conversations listing
    info!("Testing user conversations listing...");
    let user_convs = chat_db
        .list_user_conversations("test_user_1")
        .await
        .expect("Failed to list user conversations");

    assert_eq!(user_convs.len(), 1);
    assert_eq!(user_convs[0].id, conversation.id);

    // Test conversation deletion
    info!("Testing conversation deletion...");
    chat_db
        .delete_conversation(conversation.id)
        .await
        .expect("Failed to delete conversation");

    // Verify conversation is deleted
    let result = chat_db.get_conversation(conversation.id).await;
    assert!(result.is_err(), "Conversation should be deleted");

    // Verify messages are cascade deleted
    let msgs = chat_db
        .get_conversation_messages(conversation.id)
        .await
        .expect("Should return empty vec for deleted conversation");
    assert!(msgs.is_empty(), "Messages should be deleted");

    info!("All tests passed!");
}
