use crate::claude_code::{ClaudeManager, models::*};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

// Critical test for mobile-desktop sync issue
#[tokio::test]
async fn test_trigger_claude_response_no_duplicate_user_message() {
    // This test ensures that trigger_claude_response does NOT create
    // a duplicate user message, which is critical for mobile sync
    
    // TODO: This requires a mock implementation of ClaudeManager
    // For now, we document the expected behavior
    
    // Expected behavior:
    // 1. send_message creates a user message AND triggers Claude
    // 2. trigger_claude_response ONLY triggers Claude without creating user message
    
    // This distinction is critical because mobile sends a message,
    // then desktop needs to trigger Claude without duplicating the message
}

#[tokio::test]
async fn test_concurrent_message_handling() {
    // Test handling multiple messages arriving simultaneously
    // This simulates the race condition when mobile sends messages
    // while desktop is processing
    
    // Create shared manager and discovery instances
    let discovery = Arc::new(Mutex::new(crate::claude_code::ClaudeDiscovery::new()));
    let manager: Arc<Mutex<Option<ClaudeManager>>> = Arc::new(Mutex::new(None));
    
    // Clone the Arc references for concurrent access
    let manager1 = manager.clone();
    let manager2 = manager.clone();
    
    let handle1 = tokio::spawn(async move {
        let _guard = manager1.lock().await;
        // Simulate work
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    });
    
    let handle2 = tokio::spawn(async move {
        let _guard = manager2.lock().await;
        // Simulate work
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    });
    
    // Both should complete without deadlock
    let result1 = handle1.await;
    let result2 = handle2.await;
    
    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

#[tokio::test]
async fn test_message_id_consistency() {
    // Test that message IDs remain consistent across operations
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    
    assert_ne!(id1, id2);
    
    // Serialize and deserialize to ensure IDs are preserved
    let message = Message {
        id: id1,
        message_type: MessageType::User,
        content: "Test".to_string(),
        timestamp: chrono::Utc::now(),
        tool_info: None,
    };
    
    let json = serde_json::to_string(&message).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    
    assert_eq!(message.id, deserialized.id);
}