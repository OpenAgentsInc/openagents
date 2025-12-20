//! Integration tests for autopilot trajectory tracking

use autopilot::TrajectoryCollector;
use claude_agent_sdk::{
    SdkMessage, SdkAssistantMessage, SdkResultMessage, SdkSystemMessage,
    ResultSuccess, ResultError, Usage,
    protocol::SystemInit,
};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_issue_completion_tracking() {
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
    );

    // Initialize trajectory result
    let init_msg = SdkMessage::System(SdkSystemMessage::Init(
        SystemInit {
            agents: None,
            api_key_source: "user".to_string(),
            betas: None,
            claude_code_version: "1.0.0".to_string(),
            cwd: "/test/cwd".to_string(),
            tools: Vec::new(),
            mcp_servers: Vec::new(),
            model: "claude-sonnet-4".to_string(),
            permission_mode: "auto".to_string(),
            slash_commands: Vec::new(),
            output_style: "default".to_string(),
            skills: Vec::new(),
            plugins: Vec::new(),
            uuid: "uuid-init".to_string(),
            session_id: "test-session".to_string(),
        }
    ));
    collector.process_message(&init_msg);

    // Add a success result to initialize trajectory.result
    let success_msg = SdkMessage::Result(SdkResultMessage::Success(
        ResultSuccess {
            result: "completed".to_string(),
            is_error: false,
            duration_ms: 1000,
            duration_api_ms: 900,
            num_turns: 5,
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_input_tokens: Some(20),
                cache_creation_input_tokens: Some(10),
            },
            total_cost_usd: 0.05,
            model_usage: HashMap::new(),
            permission_denials: Vec::new(),
            structured_output: None,
            uuid: "uuid-1".to_string(),
            session_id: "test-session".to_string(),
        }
    ));
    collector.process_message(&success_msg);

    // Verify initial state
    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.result.as_ref().unwrap().issues_completed, 0);

    // Create a new collector and simulate issue_complete calls
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
    );

    collector.process_message(&init_msg);
    collector.process_message(&success_msg);

    // Simulate first issue_complete tool call
    let assistant_msg = SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "tool_1",
                    "name": "mcp__issues__issue_complete",
                    "input": {"number": 1}
                }
            ]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-1".to_string(),
        session_id: "test-session".to_string(),
    });
    collector.process_message(&assistant_msg);

    // Simulate second issue_complete tool call
    let assistant_msg2 = SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "tool_2",
                    "name": "mcp__issues__issue_complete",
                    "input": {"number": 2}
                }
            ]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-2".to_string(),
        session_id: "test-session".to_string(),
    });
    collector.process_message(&assistant_msg2);

    // Verify count is tracked
    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.result.as_ref().unwrap().issues_completed, 2);
}

#[test]
fn test_trajectory_serialization() {
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
    );

    // Add a system init message
    let init_msg = SdkMessage::System(SdkSystemMessage::Init(
        SystemInit {
            agents: None,
            api_key_source: "user".to_string(),
            betas: None,
            claude_code_version: "1.0.0".to_string(),
            cwd: "/test/cwd".to_string(),
            tools: Vec::new(),
            mcp_servers: Vec::new(),
            model: "claude-sonnet-4".to_string(),
            permission_mode: "auto".to_string(),
            slash_commands: Vec::new(),
            output_style: "default".to_string(),
            skills: Vec::new(),
            plugins: Vec::new(),
            uuid: "uuid-init".to_string(),
            session_id: "test-session".to_string(),
        }
    ));
    collector.process_message(&init_msg);

    // Add assistant message with tool call
    let assistant_msg = SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "Let me help with that."
                },
                {
                    "type": "tool_use",
                    "id": "tool_1",
                    "name": "Read",
                    "input": {"file_path": "/test/file.rs"}
                }
            ]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-1".to_string(),
        session_id: "test-session".to_string(),
    });
    collector.process_message(&assistant_msg);

    // Add result message
    let result_msg = SdkMessage::Result(SdkResultMessage::Success(
        ResultSuccess {
            result: "completed".to_string(),
            is_error: false,
            duration_ms: 1000,
            duration_api_ms: 900,
            num_turns: 3,
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_input_tokens: Some(20),
                cache_creation_input_tokens: Some(10),
            },
            total_cost_usd: 0.05,
            model_usage: HashMap::new(),
            permission_denials: Vec::new(),
            structured_output: None,
            uuid: "uuid-result".to_string(),
            session_id: "test-session".to_string(),
        }
    ));
    collector.process_message(&result_msg);

    let trajectory = collector.into_trajectory();

    // Verify serialization works
    let json = serde_json::to_string(&trajectory).expect("Failed to serialize trajectory");
    assert!(json.contains("test-session"));
    assert!(json.contains("claude-sonnet-4"));
    assert!(json.contains("test prompt"));

    // Verify deserialization works
    let deserialized: autopilot::trajectory::Trajectory =
        serde_json::from_str(&json).expect("Failed to deserialize trajectory");
    assert_eq!(deserialized.session_id, "test-session");
    assert_eq!(deserialized.prompt, "test prompt");
    assert_eq!(deserialized.steps.len(), 3); // init, text, tool_use
}

#[test]
fn test_token_usage_tracking() {
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        None,
    );

    // Add result message with token usage
    let result_msg = SdkMessage::Result(SdkResultMessage::Success(
        ResultSuccess {
            result: "completed".to_string(),
            is_error: false,
            duration_ms: 1000,
            duration_api_ms: 900,
            num_turns: 3,
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_input_tokens: Some(20),
                cache_creation_input_tokens: Some(10),
            },
            total_cost_usd: 0.05,
            model_usage: HashMap::new(),
            permission_denials: Vec::new(),
            structured_output: None,
            uuid: "uuid-1".to_string(),
            session_id: "test-session".to_string(),
        }
    ));
    collector.process_message(&result_msg);

    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.usage.input_tokens, 100);
    assert_eq!(trajectory.usage.output_tokens, 50);
    assert_eq!(trajectory.usage.cache_read_tokens, 20);
    assert_eq!(trajectory.usage.cache_creation_tokens, 10);
    assert_eq!(trajectory.usage.cost_usd, 0.05);
}

#[test]
fn test_error_result_tracking() {
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        None,
    );

    // Add error result
    let error_msg = SdkMessage::Result(SdkResultMessage::ErrorDuringExecution(
        ResultError {
            duration_ms: 500,
            duration_api_ms: 450,
            is_error: true,
            num_turns: 2,
            usage: Usage {
                input_tokens: 50,
                output_tokens: 25,
                cache_read_input_tokens: Some(10),
                cache_creation_input_tokens: Some(5),
            },
            total_cost_usd: 0.02,
            model_usage: HashMap::new(),
            permission_denials: Vec::new(),
            errors: vec!["Test error".to_string()],
            uuid: "uuid-1".to_string(),
            session_id: "test-session".to_string(),
        }
    ));
    collector.process_message(&error_msg);

    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.result.as_ref().unwrap().success, false);
    assert_eq!(trajectory.result.as_ref().unwrap().errors.len(), 1);
    assert_eq!(trajectory.result.as_ref().unwrap().issues_completed, 0);
    assert_eq!(trajectory.usage.cost_usd, 0.02);
}
