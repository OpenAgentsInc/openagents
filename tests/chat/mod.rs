use crate::server::services::deepseek::{
    self,
    types::{ChatMessage as DeepSeekMessage, StreamUpdate},
    DeepSeekService,
};
use crate::server::services::github_issue::GitHubService;
use crate::server::ws::{
    handlers::chat::ChatHandler,
    transport::WebSocketState,
    types::ChatMessage,
};
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_chat_history() {
    // Create test services
    let ws_state = Arc::new(WebSocketState::new());
    let deepseek_service = Arc::new(DeepSeekService::new("test_key".to_string()));
    let github_service = Arc::new(GitHubService::new("test_token".to_string()));

    // Create chat handler
    let handler = ChatHandler::new(ws_state, deepseek_service, github_service);

    // Test connection ID
    let conn_id = "test_conn";

    // Test adding messages to history
    let message1 = DeepSeekMessage {
        role: "user".to_string(),
        content: "Hello".to_string(),
        tool_call_id: None,
        tool_calls: None,
    };
    handler.add_to_history(conn_id, message1.clone()).await;

    let message2 = DeepSeekMessage {
        role: "assistant".to_string(),
        content: "Hi there!".to_string(),
        tool_call_id: None,
        tool_calls: None,
    };
    handler.add_to_history(conn_id, message2.clone()).await;

    // Test retrieving history
    let history = handler.get_history(conn_id).await;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].role, "user");
    assert_eq!(history[0].content, "Hello");
    assert_eq!(history[1].role, "assistant");
    assert_eq!(history[1].content, "Hi there!");

    // Test cleaning up history
    handler.cleanup_history(conn_id).await;
    let empty_history = handler.get_history(conn_id).await;
    assert_eq!(empty_history.len(), 0);
}

#[tokio::test]
async fn test_multiple_connections() {
    // Create test services
    let ws_state = Arc::new(WebSocketState::new());
    let deepseek_service = Arc::new(DeepSeekService::new("test_key".to_string()));
    let github_service = Arc::new(GitHubService::new("test_token".to_string()));

    // Create chat handler
    let handler = ChatHandler::new(ws_state, deepseek_service, github_service);

    // Test connection IDs
    let conn_id1 = "test_conn_1";
    let conn_id2 = "test_conn_2";

    // Add messages for first connection
    let message1 = DeepSeekMessage {
        role: "user".to_string(),
        content: "Hello from conn1".to_string(),
        tool_call_id: None,
        tool_calls: None,
    };
    handler.add_to_history(conn_id1, message1.clone()).await;

    // Add messages for second connection
    let message2 = DeepSeekMessage {
        role: "user".to_string(),
        content: "Hello from conn2".to_string(),
        tool_call_id: None,
        tool_calls: None,
    };
    handler.add_to_history(conn_id2, message2.clone()).await;

    // Test histories are separate
    let history1 = handler.get_history(conn_id1).await;
    let history2 = handler.get_history(conn_id2).await;

    assert_eq!(history1.len(), 1);
    assert_eq!(history2.len(), 1);
    assert_eq!(history1[0].content, "Hello from conn1");
    assert_eq!(history2[0].content, "Hello from conn2");

    // Clean up one connection
    handler.cleanup_history(conn_id1).await;

    // Verify only one history was cleaned
    let empty_history = handler.get_history(conn_id1).await;
    let remaining_history = handler.get_history(conn_id2).await;

    assert_eq!(empty_history.len(), 0);
    assert_eq!(remaining_history.len(), 1);
    assert_eq!(remaining_history[0].content, "Hello from conn2");
}

// Mock DeepSeekService for testing chat interactions
struct MockDeepSeekService {
    history: Vec<DeepSeekMessage>,
}

impl MockDeepSeekService {
    fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    async fn chat_stream_with_history(
        &self,
        history: Vec<DeepSeekMessage>,
        content: String,
        _use_reasoner: bool,
    ) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        let history_clone = history.clone();

        tokio::spawn(async move {
            // Send thinking update
            let _ = tx
                .send(StreamUpdate::Reasoning("Thinking...".to_string()))
                .await;

            // Generate response based on history
            let mut response = String::new();
            if !history_clone.is_empty() {
                response.push_str("Based on our conversation: ");
            }
            response.push_str(&content);

            // Send content update
            let _ = tx.send(StreamUpdate::Content(response)).await;

            // Send done update
            let _ = tx.send(StreamUpdate::Done).await;
        });

        rx
    }
}

#[tokio::test]
async fn test_chat_interaction() {
    // Create test services with mock DeepSeek
    let ws_state = Arc::new(WebSocketState::new());
    let mock_deepseek = Arc::new(MockDeepSeekService::new());
    let github_service = Arc::new(GitHubService::new("test_token".to_string()));

    // Create chat handler
    let handler = ChatHandler::new(ws_state.clone(), mock_deepseek, github_service);

    // Test connection ID
    let conn_id = "test_conn";

    // Send first message
    let message1 = ChatMessage::UserMessage {
        content: "Hello".to_string(),
    };
    handler
        .handle_message(message1, conn_id.to_string())
        .await
        .unwrap();

    // Verify history was updated
    let history = handler.get_history(conn_id).await;
    assert_eq!(history.len(), 2); // User message + AI response
    assert_eq!(history[0].role, "user");
    assert_eq!(history[0].content, "Hello");
    assert_eq!(history[1].role, "assistant");

    // Send second message
    let message2 = ChatMessage::UserMessage {
        content: "How are you?".to_string(),
    };
    handler
        .handle_message(message2, conn_id.to_string())
        .await
        .unwrap();

    // Verify history includes both interactions
    let history = handler.get_history(conn_id).await;
    assert_eq!(history.len(), 4); // 2 user messages + 2 AI responses
    assert_eq!(history[2].role, "user");
    assert_eq!(history[2].content, "How are you?");
    assert_eq!(history[3].role, "assistant");
}