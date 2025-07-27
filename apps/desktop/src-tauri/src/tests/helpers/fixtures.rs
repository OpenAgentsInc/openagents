use crate::claude_code::models::*;
use chrono::Utc;
use uuid::Uuid;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Test fixture generators

// Helper function to create safe test paths
pub fn create_safe_test_path<P: AsRef<Path>>(base: P, file: &str) -> PathBuf {
    // Sanitize the filename to prevent path traversal
    let sanitized = file
        .replace("..", "")
        .replace("/", "_")
        .replace("\\", "_");
    
    base.as_ref().join(sanitized)
}

pub fn create_test_message(message_type: MessageType, content: &str) -> Message {
    Message {
        id: Uuid::new_v4(),
        message_type,
        content: content.to_string(),
        timestamp: Utc::now(),
        tool_info: None,
    }
}

pub fn create_test_message_with_tool(tool_name: &str, tool_use_id: &str) -> Message {
    let mut input = HashMap::new();
    input.insert("test_param".to_string(), serde_json::json!("test_value"));
    
    Message {
        id: Uuid::new_v4(),
        message_type: MessageType::ToolUse,
        content: format!("Using tool: {}", tool_name),
        timestamp: Utc::now(),
        tool_info: Some(ToolInfo {
            tool_name: tool_name.to_string(),
            tool_use_id: tool_use_id.to_string(),
            input,
            output: None,
        }),
    }
}

pub fn create_test_conversation(id: &str, project_name: &str) -> ClaudeConversation {
    // Use proper path construction with sanitization
    let test_base = PathBuf::from("/test");
    let project_base = PathBuf::from("/projects");
    
    ClaudeConversation {
        id: format!("TEST_{}", id), // Clear test prefix
        project_name: project_name.to_string(),
        timestamp: Utc::now(),
        first_message: "Test conversation".to_string(),
        message_count: 1,
        file_path: create_safe_test_path(&test_base, &format!("{}.jsonl", id)).to_string_lossy().to_string(),
        working_directory: create_safe_test_path(&project_base, project_name).to_string_lossy().to_string(),
        summary: None,
    }
}

pub fn create_test_apm_session(id: &str, apm: f64) -> crate::APMSession {
    crate::APMSession {
        id: format!("TEST_SESSION_{}", id), // Clear test prefix
        project: "TEST_PROJECT".to_string(), // Clear test prefix
        apm,
        duration: 60.0,
        message_count: 10.0,
        tool_count: 5.0,
        timestamp: Utc::now().to_rfc3339(),
    }
}

pub fn create_test_tool_usage(name: &str, count: u32) -> crate::ToolUsage {
    crate::ToolUsage {
        name: name.to_string(),
        count: count as f64,
        percentage: 0.0,
        category: crate::get_tool_category(name),
    }
}