use crate::claude_code::models::*;
use chrono::Utc;
use uuid::Uuid;
use std::collections::HashMap;

#[test]
fn test_message_serialization() {
    let message = Message {
        id: Uuid::new_v4(),
        message_type: MessageType::User,
        content: "Test message content".to_string(),
        timestamp: Utc::now(),
        tool_info: None,
    };
    
    // Serialize to JSON
    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains("user"));
    assert!(json.contains("Test message content"));
    
    // Deserialize back
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.content, message.content);
    assert_eq!(deserialized.message_type, MessageType::User);
}

#[test]
fn test_message_type_serialization() {
    // Test all message type variants
    let test_cases = vec![
        (MessageType::User, "\"user\""),
        (MessageType::Assistant, "\"assistant\""),
        (MessageType::ToolUse, "\"tool_use\""),
        (MessageType::ToolResult, "\"tool_result\""),
        (MessageType::Error, "\"error\""),
        (MessageType::Summary, "\"summary\""),
        (MessageType::Thinking, "\"thinking\""),
        (MessageType::System, "\"system\""),
    ];
    
    for (msg_type, expected) in test_cases {
        let json = serde_json::to_string(&msg_type).unwrap();
        assert_eq!(json, expected);
        
        let deserialized: MessageType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, msg_type);
    }
}

#[test]
fn test_tool_info_serialization() {
    let mut input = HashMap::new();
    input.insert("command".to_string(), serde_json::json!("ls -la"));
    
    let tool_info = ToolInfo {
        tool_name: "Bash".to_string(),
        tool_use_id: "test-tool-id".to_string(),
        input,
        output: Some("file1.txt\nfile2.txt".to_string()),
    };
    
    let json = serde_json::to_string(&tool_info).unwrap();
    assert!(json.contains("Bash"));
    assert!(json.contains("test-tool-id"));
    assert!(json.contains("ls -la"));
    
    let deserialized: ToolInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.tool_name, "Bash");
    assert_eq!(deserialized.output, Some("file1.txt\nfile2.txt".to_string()));
}

#[test]
fn test_message_with_tool_info() {
    let mut input = HashMap::new();
    input.insert("path".to_string(), serde_json::json!("/test/file.txt"));
    
    let tool_info = Some(ToolInfo {
        tool_name: "Read".to_string(),
        tool_use_id: "read-123".to_string(),
        input,
        output: None,
    });
    
    let message = Message {
        id: Uuid::new_v4(),
        message_type: MessageType::ToolUse,
        content: "Reading file...".to_string(),
        timestamp: Utc::now(),
        tool_info,
    };
    
    let json = serde_json::to_string(&message).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    
    assert!(deserialized.tool_info.is_some());
    let tool = deserialized.tool_info.unwrap();
    assert_eq!(tool.tool_name, "Read");
    assert_eq!(tool.tool_use_id, "read-123");
}

#[test]
fn test_claude_error_variants() {
    // Test error creation and display
    let errors = vec![
        ClaudeError::BinaryNotFound,
        ClaudeError::SessionNotFound("test-session".to_string()),
        ClaudeError::Other("custom error".to_string()),
    ];
    
    for error in errors {
        // Ensure errors can be converted to strings
        let error_string = error.to_string();
        assert!(!error_string.is_empty());
    }
}

#[test]
fn test_claude_conversation_serialization() {
    let conversation = ClaudeConversation {
        id: "conv-123".to_string(),
        project_name: "test-project".to_string(),
        timestamp: Utc::now(),
        first_message: "Hello Claude".to_string(),
        message_count: 5,
        file_path: "/path/to/conversation.jsonl".to_string(),
        working_directory: "/test/project".to_string(),
        summary: Some("Test conversation summary".to_string()),
    };
    
    let json = serde_json::to_string(&conversation).unwrap();
    let deserialized: ClaudeConversation = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.id, "conv-123");
    assert_eq!(deserialized.project_name, "test-project");
    assert_eq!(deserialized.message_count, 5);
    assert_eq!(deserialized.summary, Some("Test conversation summary".to_string()));
}

#[test]
fn test_outgoing_user_message_construction() {
    let content_item = ContentItem {
        item_type: "text".to_string(),
        text: "Hello, Claude!".to_string(),
    };
    
    let user_content = UserMessageContent {
        role: "user".to_string(),
        content: vec![content_item],
    };
    
    let message = OutgoingUserMessage {
        msg_type: "conversation.message".to_string(),
        message: user_content,
    };
    
    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains("conversation.message"));
    assert!(json.contains("Hello, Claude!"));
    assert!(json.contains("user"));
}

#[test]
fn test_unified_session_source_serialization() {
    let test_cases = vec![
        (SessionSource::Local, "\"local\""),
        (SessionSource::Convex, "\"convex\""),
    ];
    
    for (source, expected) in test_cases {
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, expected);
        
        let deserialized: SessionSource = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, source);
    }
}