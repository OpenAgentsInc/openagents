//! Integration tests documenting autonomous loop execution patterns
//!
//! These tests verify that the trajectory collector properly records
//! the autonomous loop workflow. See autonomous_loop_doc.md for details.

use autopilot::TrajectoryCollector;
use claude_agent_sdk::{
    SdkMessage, SdkAssistantMessage, SdkResultMessage, SdkSystemMessage,
    ResultSuccess, Usage,
    protocol::SystemInit,
};
use serde_json::json;
use std::collections::HashMap;

/// Helper to create a test trajectory collector
fn create_test_collector(session_id: &str) -> TrajectoryCollector {
    TrajectoryCollector::new(
        "test autonomous loop".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        session_id.to_string(),
        Some("main".to_string()),
    )
}

/// Helper to send init message
fn send_init(collector: &mut TrajectoryCollector, session_id: &str) {
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
            session_id: session_id.to_string(),
        }
    ));
    collector.process_message(&init_msg);
}

/// Helper to send success result
fn send_success(collector: &mut TrajectoryCollector, session_id: &str, num_turns: u32) {
    let success_msg = SdkMessage::Result(SdkResultMessage::Success(
        ResultSuccess {
            result: "completed".to_string(),
            is_error: false,
            duration_ms: 1000,
            duration_api_ms: 900,
            num_turns,
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
            uuid: "uuid-success".to_string(),
            session_id: session_id.to_string(),
        }
    ));
    collector.process_message(&success_msg);
}

#[test]
fn test_autonomous_loop_tool_sequence() {
    // Documents the typical tool call sequence in autonomous loop
    let session_id = "test-tool-sequence";
    let mut collector = create_test_collector(session_id);

    send_init(&mut collector, session_id);

    // issue_ready
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "tool_1",
                "name": "mcp__issues__issue_ready",
                "input": {}
            }]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-1".to_string(),
        session_id: session_id.to_string(),
    }));

    // issue_claim
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "tool_2",
                "name": "mcp__issues__issue_claim",
                "input": {"number": 1, "run_id": "test"}
            }]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-2".to_string(),
        session_id: session_id.to_string(),
    }));

    // Read tool (implementation)
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "tool_3",
                "name": "Read",
                "input": {"file_path": "/test/file.rs"}
            }]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-3".to_string(),
        session_id: session_id.to_string(),
    }));

    // Edit tool (implementation)
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "tool_4",
                "name": "Edit",
                "input": {
                    "file_path": "/test/file.rs",
                    "old_string": "old",
                    "new_string": "new"
                }
            }]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-4".to_string(),
        session_id: session_id.to_string(),
    }));

    // Git commit
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "tool_5",
                "name": "Bash",
                "input": {"command": "git commit -m 'test'"}
            }]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-5".to_string(),
        session_id: session_id.to_string(),
    }));

    // Git push
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "tool_6",
                "name": "Bash",
                "input": {"command": "git push origin main"}
            }]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-6".to_string(),
        session_id: session_id.to_string(),
    }));

    // issue_complete
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "tool_7",
                "name": "mcp__issues__issue_complete",
                "input": {"number": 1}
            }]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-7".to_string(),
        session_id: session_id.to_string(),
    }));

    send_success(&mut collector, session_id, 7);

    // Verify trajectory captured all steps
    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.result.as_ref().unwrap().num_turns, 7);
    assert!(trajectory.steps.len() >= 7);

    // Verify key tools are present
    assert!(trajectory.steps.iter().any(|s| {
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_ready")
    }));
    assert!(trajectory.steps.iter().any(|s| {
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_complete")
    }));
    assert!(trajectory.steps.iter().any(|s| {
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "Bash")
    }));
}

#[test]
fn test_no_ready_issues_workflow() {
    // Documents the workflow when no issues are ready
    let session_id = "test-no-issues";
    let mut collector = create_test_collector(session_id);

    send_init(&mut collector, session_id);

    // issue_ready (returns no issues)
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "tool_1",
                "name": "mcp__issues__issue_ready",
                "input": {}
            }]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-1".to_string(),
        session_id: session_id.to_string(),
    }));

    // directive_get (find work)
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "tool_2",
                "name": "mcp__issues__directive_get",
                "input": {"id": "d-001"}
            }]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-2".to_string(),
        session_id: session_id.to_string(),
    }));

    // issue_create (make new work)
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": "tool_3",
                "name": "mcp__issues__issue_create",
                "input": {
                    "title": "New task",
                    "directive_id": "d-001"
                }
            }]
        }),
        parent_tool_use_id: None,
        error: None,
        uuid: "uuid-3".to_string(),
        session_id: session_id.to_string(),
    }));

    send_success(&mut collector, session_id, 3);

    // Verify directive exploration is tracked
    let trajectory = collector.into_trajectory();
    assert!(trajectory.steps.iter().any(|s| {
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__directive_get")
    }));
    assert!(trajectory.steps.iter().any(|s| {
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_create")
    }));
}
