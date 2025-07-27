use crate::claude_code::{ClaudeManager, models::ClaudeError};
use std::path::PathBuf;

#[tokio::test]
async fn test_manager_creation() {
    let manager = ClaudeManager::new();
    
    // Manager should be created without binary path initially
    // We can't directly test private fields, but we can test behavior
    let sessions = manager.get_active_sessions().await;
    assert_eq!(sessions.len(), 0);
}

#[tokio::test]
async fn test_create_session_without_binary() {
    let manager = ClaudeManager::new();
    
    // Attempt to create session without setting binary path
    let result = manager.create_session("/test/project".to_string()).await;
    
    match result {
        Err(ClaudeError::BinaryNotFound) => {
            // Expected error
        }
        _ => panic!("Expected BinaryNotFound error"),
    }
}

#[tokio::test]
async fn test_send_message_invalid_session() {
    let manager = ClaudeManager::new();
    
    // Try to send message to non-existent session
    let result = manager.send_message("invalid-session-id", "Test message".to_string()).await;
    
    match result {
        Err(ClaudeError::SessionNotFound(id)) => {
            assert_eq!(id, "invalid-session-id");
        }
        _ => panic!("Expected SessionNotFound error"),
    }
}

#[tokio::test]
async fn test_trigger_response_invalid_session() {
    let manager = ClaudeManager::new();
    
    // Try to trigger response for non-existent session
    let result = manager.trigger_response("invalid-session-id", "Test message".to_string()).await;
    
    match result {
        Err(ClaudeError::SessionNotFound(id)) => {
            assert_eq!(id, "invalid-session-id");
        }
        _ => panic!("Expected SessionNotFound error"),
    }
}

#[tokio::test]
async fn test_get_messages_invalid_session() {
    let manager = ClaudeManager::new();
    
    // Try to get messages from non-existent session
    let result = manager.get_messages("invalid-session-id").await;
    
    match result {
        Err(ClaudeError::SessionNotFound(id)) => {
            assert_eq!(id, "invalid-session-id");
        }
        _ => panic!("Expected SessionNotFound error"),
    }
}

#[tokio::test]
async fn test_stop_session_invalid_session() {
    let manager = ClaudeManager::new();
    
    // Try to stop non-existent session - should succeed silently
    let result = manager.stop_session("invalid-session-id").await;
    
    // The implementation returns Ok(()) even if session doesn't exist
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_active_sessions_empty() {
    let manager = ClaudeManager::new();
    
    // Should return empty list when no sessions exist
    let sessions = manager.get_active_sessions().await;
    assert_eq!(sessions.len(), 0);
}

// Test for binary path setting
#[tokio::test]
async fn test_set_binary_path() {
    let mut manager = ClaudeManager::new();
    let test_path = PathBuf::from("/test/binary/path");
    
    manager.set_binary_path(test_path.clone());
    
    // We can't directly verify the path was set, but we can test
    // that creating a session now fails differently (not BinaryNotFound)
    // In a real test environment with mocks, we would verify this better
}