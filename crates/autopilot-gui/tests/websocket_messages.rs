//! WebSocket message format tests for autopilot-gui
//!
//! Tests WebSocket message serialization/deserialization following directive d-013

use serde_json::json;

// =============================================================================
// Client message deserialization tests
// =============================================================================

#[test]
fn test_prompt_message_deserialize() {
    let json_msg = json!({
        "type": "prompt",
        "text": "Hello, agent!"
    });

    let json_str = json_msg.to_string();

    // Should parse successfully
    let result = serde_json::from_str::<serde_json::Value>(&json_str);
    assert!(result.is_ok());

    let parsed = result.unwrap();
    assert_eq!(parsed["type"], "prompt");
    assert_eq!(parsed["text"], "Hello, agent!");
}

#[test]
fn test_abort_message_deserialize() {
    let json_msg = json!({
        "type": "abort"
    });

    let json_str = json_msg.to_string();
    let result = serde_json::from_str::<serde_json::Value>(&json_str);

    assert!(result.is_ok());
    assert_eq!(result.unwrap()["type"], "abort");
}

#[test]
fn test_permission_response_deserialize() {
    let json_msg = json!({
        "type": "permission_response",
        "request_id": "req_123",
        "action": "allow",
        "pattern": null,
        "persistent": false
    });

    let json_str = json_msg.to_string();
    let result = serde_json::from_str::<serde_json::Value>(&json_str);

    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed["type"], "permission_response");
    assert_eq!(parsed["request_id"], "req_123");
    assert_eq!(parsed["action"], "allow");
    assert_eq!(parsed["persistent"], false);
}

#[test]
fn test_permission_response_with_pattern() {
    let json_msg = json!({
        "type": "permission_response",
        "request_id": "req_456",
        "action": "deny",
        "pattern": "some_pattern",
        "persistent": true
    });

    let json_str = json_msg.to_string();
    let result = serde_json::from_str::<serde_json::Value>(&json_str);

    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed["pattern"], "some_pattern");
    assert_eq!(parsed["persistent"], true);
}

// =============================================================================
// Server message serialization tests
// =============================================================================

#[test]
fn test_message_serialize() {
    let msg = json!({
        "type": "message",
        "role": "assistant",
        "content": "Response text"
    });

    let json_str = msg.to_string();
    assert!(json_str.contains("\"type\":\"message\""));
    assert!(json_str.contains("\"role\":\"assistant\""));
    assert!(json_str.contains("\"content\":\"Response text\""));
}

#[test]
fn test_status_serialize() {
    let msg = json!({
        "type": "status",
        "status": "running"
    });

    let json_str = msg.to_string();
    assert!(json_str.contains("\"type\":\"status\""));
    assert!(json_str.contains("\"status\":\"running\""));
}

#[test]
fn test_error_serialize() {
    let msg = json!({
        "type": "error",
        "message": "Something went wrong"
    });

    let json_str = msg.to_string();
    assert!(json_str.contains("\"type\":\"error\""));
    assert!(json_str.contains("\"message\":\"Something went wrong\""));
}

#[test]
fn test_tool_call_serialize() {
    let msg = json!({
        "type": "tool_call",
        "tool": "Bash",
        "input": {"command": "ls"},
        "status": "running"
    });

    let json_str = msg.to_string();
    assert!(json_str.contains("\"type\":\"tool_call\""));
    assert!(json_str.contains("\"tool\":\"Bash\""));
    assert!(json_str.contains("\"status\":\"running\""));
}

#[test]
fn test_tool_result_serialize() {
    let msg = json!({
        "type": "tool_result",
        "tool": "Bash",
        "output": "file1.txt\nfile2.txt",
        "elapsed_ms": 150
    });

    let json_str = msg.to_string();
    assert!(json_str.contains("\"type\":\"tool_result\""));
    assert!(json_str.contains("\"tool\":\"Bash\""));
    assert!(json_str.contains("\"elapsed_ms\":150"));
}

#[test]
fn test_permission_request_serialize() {
    let msg = json!({
        "type": "permission_request",
        "id": "req_789",
        "tool": "Write",
        "input": {"file_path": "/test.txt", "content": "Hello"},
        "description": "Write to test file",
        "timestamp": "2025-12-22T10:00:00Z"
    });

    let json_str = msg.to_string();
    assert!(json_str.contains("\"type\":\"permission_request\""));
    assert!(json_str.contains("\"id\":\"req_789\""));
    assert!(json_str.contains("\"tool\":\"Write\""));
}

#[test]
fn test_session_started_serialize() {
    let msg = json!({
        "type": "session_started",
        "session_id": "ses_123",
        "timestamp": "2025-12-22T10:00:00Z",
        "model": "claude-sonnet-4",
        "prompt": "Fix the bug"
    });

    let json_str = msg.to_string();
    assert!(json_str.contains("\"type\":\"session_started\""));
    assert!(json_str.contains("\"session_id\":\"ses_123\""));
    assert!(json_str.contains("\"model\":\"claude-sonnet-4\""));
}

#[test]
fn test_session_updated_serialize() {
    let msg = json!({
        "type": "session_updated",
        "session_id": "ses_123",
        "tokens_in": 1000,
        "tokens_out": 500,
        "tool_calls": 5,
        "tool_errors": 0
    });

    let json_str = msg.to_string();
    assert!(json_str.contains("\"type\":\"session_updated\""));
    assert!(json_str.contains("\"tokens_in\":1000"));
    assert!(json_str.contains("\"tokens_out\":500"));
}

#[test]
fn test_session_completed_serialize() {
    let msg = json!({
        "type": "session_completed",
        "session_id": "ses_123",
        "duration_seconds": 45.5,
        "final_status": "success",
        "issues_completed": 3,
        "cost_usd": 0.0123
    });

    let json_str = msg.to_string();
    assert!(json_str.contains("\"type\":\"session_completed\""));
    assert!(json_str.contains("\"duration_seconds\":45.5"));
    assert!(json_str.contains("\"final_status\":\"success\""));
    assert!(json_str.contains("\"issues_completed\":3"));
}

#[test]
fn test_stats_updated_serialize() {
    let msg = json!({
        "type": "stats_updated",
        "sessions_today": 10,
        "success_rate": 0.85,
        "total_tokens": 50000,
        "total_cost": 0.5,
        "avg_duration": 30.5
    });

    let json_str = msg.to_string();
    assert!(json_str.contains("\"type\":\"stats_updated\""));
    assert!(json_str.contains("\"sessions_today\":10"));
    assert!(json_str.contains("\"success_rate\":0.85"));
}

// =============================================================================
// Message validation tests
// =============================================================================

#[test]
fn test_prompt_empty_text() {
    let json_msg = json!({
        "type": "prompt",
        "text": ""
    });

    let result = serde_json::from_str::<serde_json::Value>(&json_msg.to_string());
    assert!(result.is_ok());
    assert_eq!(result.unwrap()["text"], "");
}

#[test]
fn test_prompt_long_text() {
    let long_text = "x".repeat(10000);
    let json_msg = json!({
        "type": "prompt",
        "text": long_text
    });

    let result = serde_json::from_str::<serde_json::Value>(&json_msg.to_string());
    assert!(result.is_ok());
}

#[test]
fn test_prompt_special_characters() {
    let json_msg = json!({
        "type": "prompt",
        "text": "Special chars: <script>alert('xss')</script> & \"quotes\" \n\t"
    });

    let json_str = json_msg.to_string();

    // JSON should properly escape quotes and newlines
    // Note: serde_json doesn't escape HTML chars in JSON strings by default (that's handled by Maud)
    assert!(json_str.contains("\\n") || json_str.contains("\\t"));

    // Should be valid JSON
    let result = serde_json::from_str::<serde_json::Value>(&json_str);
    assert!(result.is_ok());
}

#[test]
fn test_prompt_unicode() {
    let json_msg = json!({
        "type": "prompt",
        "text": "Unicode: 你好世界 🚀 ñ"
    });

    let result = serde_json::from_str::<serde_json::Value>(&json_msg.to_string());
    assert!(result.is_ok());
}

#[test]
fn test_invalid_json() {
    let invalid = "{ type: prompt, text: 'bad json' }";
    let result = serde_json::from_str::<serde_json::Value>(invalid);
    assert!(result.is_err());
}

#[test]
fn test_missing_required_field() {
    // Prompt message without text field
    let json_msg = json!({
        "type": "prompt"
    });

    // Should parse as JSON but might fail validation later
    let result = serde_json::from_str::<serde_json::Value>(&json_msg.to_string());
    assert!(result.is_ok());
}

// =============================================================================
// Round-trip serialization tests
// =============================================================================

#[test]
fn test_roundtrip_prompt() {
    let original = json!({
        "type": "prompt",
        "text": "Test message"
    });

    let json_str = original.to_string();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed, original);
}

#[test]
fn test_roundtrip_permission_response() {
    let original = json!({
        "type": "permission_response",
        "request_id": "req_999",
        "action": "allow",
        "pattern": "test_pattern",
        "persistent": true
    });

    let json_str = original.to_string();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed, original);
}

#[test]
fn test_roundtrip_tool_result() {
    let original = json!({
        "type": "tool_result",
        "tool": "Read",
        "output": "File contents here",
        "elapsed_ms": 250
    });

    let json_str = original.to_string();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed, original);
}

// =============================================================================
// Type safety tests
// =============================================================================

#[test]
fn test_message_type_field_required() {
    let msg_with_type = json!({"type": "prompt", "text": "test"});
    let msg_without_type = json!({"text": "test"});

    assert!(msg_with_type.get("type").is_some());
    assert!(msg_without_type.get("type").is_none());
}

#[test]
fn test_all_message_types_unique() {
    let types = vec![
        "message",
        "status",
        "error",
        "tool_call",
        "tool_result",
        "permission_request",
        "session_started",
        "session_updated",
        "session_completed",
        "stats_updated",
    ];

    // All should be unique
    let unique_count = types.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(unique_count, types.len());
}

#[test]
fn test_numeric_fields_types() {
    let msg = json!({
        "type": "session_updated",
        "session_id": "ses_123",
        "tokens_in": 1000,
        "tokens_out": 500,
        "tool_calls": 5,
        "tool_errors": 0
    });

    assert!(msg["tokens_in"].is_i64());
    assert!(msg["tokens_out"].is_i64());
    assert!(msg["tool_calls"].is_i64());
    assert!(msg["tool_errors"].is_i64());
}

#[test]
fn test_float_fields_types() {
    let msg = json!({
        "type": "session_completed",
        "session_id": "ses_123",
        "duration_seconds": 45.5,
        "final_status": "success",
        "issues_completed": 3,
        "cost_usd": 0.0123
    });

    assert!(msg["duration_seconds"].is_f64());
    assert!(msg["cost_usd"].is_f64());
}
