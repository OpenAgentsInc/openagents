//! Integration tests documenting autonomous loop execution patterns
//! Integration tests documenting autonomous loop execution patterns
//!
//!
//! These tests verify that the trajectory collector properly records
//! These tests verify that the trajectory collector properly records
//! the autonomous loop workflow. See autonomous_loop_doc.md for details.
//! the autonomous loop workflow. See autonomous_loop_doc.md for details.


use autopilot::TrajectoryCollector;
use autopilot::TrajectoryCollector;
use claude_agent_sdk::{
use claude_agent_sdk::{
    SdkMessage, SdkAssistantMessage, SdkResultMessage, SdkSystemMessage,
    SdkMessage, SdkAssistantMessage, SdkResultMessage, SdkSystemMessage,
    ResultSuccess, Usage,
    ResultSuccess, Usage,
    protocol::SystemInit,
    protocol::SystemInit,
};
};
use serde_json::json;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashMap;


/// Helper to create a test trajectory collector
/// Helper to create a test trajectory collector
fn create_test_collector(session_id: &str) -> TrajectoryCollector {
fn create_test_collector(session_id: &str) -> TrajectoryCollector {
    TrajectoryCollector::new(
    TrajectoryCollector::new(
        "test autonomous loop".to_string(),
        "test autonomous loop".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "/test/cwd".to_string(),
        session_id.to_string(),
        session_id.to_string(),
        Some("main".to_string()),
        Some("main".to_string()),
    )
    )
}
}


/// Helper to send init message
/// Helper to send init message
fn send_init(collector: &mut TrajectoryCollector, session_id: &str) {
fn send_init(collector: &mut TrajectoryCollector, session_id: &str) {
    let init_msg = SdkMessage::System(SdkSystemMessage::Init(
    let init_msg = SdkMessage::System(SdkSystemMessage::Init(
        SystemInit {
        SystemInit {
            agents: None,
            agents: None,
            api_key_source: "user".to_string(),
            api_key_source: "user".to_string(),
            betas: None,
            betas: None,
            claude_code_version: "1.0.0".to_string(),
            claude_code_version: "1.0.0".to_string(),
            cwd: "/test/cwd".to_string(),
            cwd: "/test/cwd".to_string(),
            tools: Vec::new(),
            tools: Vec::new(),
            mcp_servers: Vec::new(),
            mcp_servers: Vec::new(),
            model: "claude-sonnet-4".to_string(),
            model: "claude-sonnet-4".to_string(),
            permission_mode: "auto".to_string(),
            permission_mode: "auto".to_string(),
            slash_commands: Vec::new(),
            slash_commands: Vec::new(),
            output_style: "default".to_string(),
            output_style: "default".to_string(),
            skills: Vec::new(),
            skills: Vec::new(),
            plugins: Vec::new(),
            plugins: Vec::new(),
            uuid: "uuid-init".to_string(),
            uuid: "uuid-init".to_string(),
            session_id: session_id.to_string(),
            session_id: session_id.to_string(),
        }
        }
    ));
    ));
    collector.process_message(&init_msg);
    collector.process_message(&init_msg);
}
}


/// Helper to send success result
/// Helper to send success result
fn send_success(collector: &mut TrajectoryCollector, session_id: &str, num_turns: u32) {
fn send_success(collector: &mut TrajectoryCollector, session_id: &str, num_turns: u32) {
    let success_msg = SdkMessage::Result(SdkResultMessage::Success(
    let success_msg = SdkMessage::Result(SdkResultMessage::Success(
        ResultSuccess {
        ResultSuccess {
            result: "completed".to_string(),
            result: "completed".to_string(),
            is_error: false,
            is_error: false,
            duration_ms: 1000,
            duration_ms: 1000,
            duration_api_ms: 900,
            duration_api_ms: 900,
            num_turns,
            num_turns,
            usage: Usage {
            usage: Usage {
                input_tokens: 100,
                input_tokens: 100,
                output_tokens: 50,
                output_tokens: 50,
                cache_read_input_tokens: Some(20),
                cache_read_input_tokens: Some(20),
                cache_creation_input_tokens: Some(10),
                cache_creation_input_tokens: Some(10),
            },
            },
            total_cost_usd: 0.05,
            total_cost_usd: 0.05,
            model_usage: HashMap::new(),
            model_usage: HashMap::new(),
            permission_denials: Vec::new(),
            permission_denials: Vec::new(),
            structured_output: None,
            structured_output: None,
            uuid: "uuid-success".to_string(),
            uuid: "uuid-success".to_string(),
            session_id: session_id.to_string(),
            session_id: session_id.to_string(),
        }
        }
    ));
    ));
    collector.process_message(&success_msg);
    collector.process_message(&success_msg);
}
}


#[test]
#[test]
fn test_autonomous_loop_tool_sequence() {
fn test_autonomous_loop_tool_sequence() {
    // Documents the typical tool call sequence in autonomous loop
    // Documents the typical tool call sequence in autonomous loop
    let session_id = "test-tool-sequence";
    let session_id = "test-tool-sequence";
    let mut collector = create_test_collector(session_id);
    let mut collector = create_test_collector(session_id);


    send_init(&mut collector, session_id);
    send_init(&mut collector, session_id);


    // issue_ready
    // issue_ready
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [{
            "content": [{
                "type": "tool_use",
                "type": "tool_use",
                "id": "tool_1",
                "id": "tool_1",
                "name": "mcp__issues__issue_ready",
                "name": "mcp__issues__issue_ready",
                "input": {}
                "input": {}
            }]
            }]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-1".to_string(),
        uuid: "uuid-1".to_string(),
        session_id: session_id.to_string(),
        session_id: session_id.to_string(),
    }));
    }));


    // issue_claim
    // issue_claim
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [{
            "content": [{
                "type": "tool_use",
                "type": "tool_use",
                "id": "tool_2",
                "id": "tool_2",
                "name": "mcp__issues__issue_claim",
                "name": "mcp__issues__issue_claim",
                "input": {"number": 1, "run_id": "test"}
                "input": {"number": 1, "run_id": "test"}
            }]
            }]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-2".to_string(),
        uuid: "uuid-2".to_string(),
        session_id: session_id.to_string(),
        session_id: session_id.to_string(),
    }));
    }));


    // Read tool (implementation)
    // Read tool (implementation)
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [{
            "content": [{
                "type": "tool_use",
                "type": "tool_use",
                "id": "tool_3",
                "id": "tool_3",
                "name": "Read",
                "name": "Read",
                "input": {"file_path": "/test/file.rs"}
                "input": {"file_path": "/test/file.rs"}
            }]
            }]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-3".to_string(),
        uuid: "uuid-3".to_string(),
        session_id: session_id.to_string(),
        session_id: session_id.to_string(),
    }));
    }));


    // Edit tool (implementation)
    // Edit tool (implementation)
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [{
            "content": [{
                "type": "tool_use",
                "type": "tool_use",
                "id": "tool_4",
                "id": "tool_4",
                "name": "Edit",
                "name": "Edit",
                "input": {
                "input": {
                    "file_path": "/test/file.rs",
                    "file_path": "/test/file.rs",
                    "old_string": "old",
                    "old_string": "old",
                    "new_string": "new"
                    "new_string": "new"
                }
                }
            }]
            }]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-4".to_string(),
        uuid: "uuid-4".to_string(),
        session_id: session_id.to_string(),
        session_id: session_id.to_string(),
    }));
    }));


    // Git commit
    // Git commit
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [{
            "content": [{
                "type": "tool_use",
                "type": "tool_use",
                "id": "tool_5",
                "id": "tool_5",
                "name": "Bash",
                "name": "Bash",
                "input": {"command": "git commit -m 'test'"}
                "input": {"command": "git commit -m 'test'"}
            }]
            }]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-5".to_string(),
        uuid: "uuid-5".to_string(),
        session_id: session_id.to_string(),
        session_id: session_id.to_string(),
    }));
    }));


    // Git push
    // Git push
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [{
            "content": [{
                "type": "tool_use",
                "type": "tool_use",
                "id": "tool_6",
                "id": "tool_6",
                "name": "Bash",
                "name": "Bash",
                "input": {"command": "git push origin main"}
                "input": {"command": "git push origin main"}
            }]
            }]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-6".to_string(),
        uuid: "uuid-6".to_string(),
        session_id: session_id.to_string(),
        session_id: session_id.to_string(),
    }));
    }));


    // issue_complete
    // issue_complete
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [{
            "content": [{
                "type": "tool_use",
                "type": "tool_use",
                "id": "tool_7",
                "id": "tool_7",
                "name": "mcp__issues__issue_complete",
                "name": "mcp__issues__issue_complete",
                "input": {"number": 1}
                "input": {"number": 1}
            }]
            }]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-7".to_string(),
        uuid: "uuid-7".to_string(),
        session_id: session_id.to_string(),
        session_id: session_id.to_string(),
    }));
    }));


    send_success(&mut collector, session_id, 7);
    send_success(&mut collector, session_id, 7);


    // Verify trajectory captured all steps
    // Verify trajectory captured all steps
    let trajectory = collector.into_trajectory();
    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.result.as_ref().unwrap().num_turns, 7);
    assert_eq!(trajectory.result.as_ref().unwrap().num_turns, 7);
    assert!(trajectory.steps.len() >= 7);
    assert!(trajectory.steps.len() >= 7);


    // Verify key tools are present
    // Verify key tools are present
    assert!(trajectory.steps.iter().any(|s| {
    assert!(trajectory.steps.iter().any(|s| {
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_ready")
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_ready")
    }));
    }));
    assert!(trajectory.steps.iter().any(|s| {
    assert!(trajectory.steps.iter().any(|s| {
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_complete")
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_complete")
    }));
    }));
    assert!(trajectory.steps.iter().any(|s| {
    assert!(trajectory.steps.iter().any(|s| {
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "Bash")
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "Bash")
    }));
    }));
}
}


#[test]
#[test]
fn test_no_ready_issues_workflow() {
fn test_no_ready_issues_workflow() {
    // Documents the workflow when no issues are ready
    // Documents the workflow when no issues are ready
    let session_id = "test-no-issues";
    let session_id = "test-no-issues";
    let mut collector = create_test_collector(session_id);
    let mut collector = create_test_collector(session_id);


    send_init(&mut collector, session_id);
    send_init(&mut collector, session_id);


    // issue_ready (returns no issues)
    // issue_ready (returns no issues)
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [{
            "content": [{
                "type": "tool_use",
                "type": "tool_use",
                "id": "tool_1",
                "id": "tool_1",
                "name": "mcp__issues__issue_ready",
                "name": "mcp__issues__issue_ready",
                "input": {}
                "input": {}
            }]
            }]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-1".to_string(),
        uuid: "uuid-1".to_string(),
        session_id: session_id.to_string(),
        session_id: session_id.to_string(),
    }));
    }));


    // directive_get (find work)
    // directive_get (find work)
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [{
            "content": [{
                "type": "tool_use",
                "type": "tool_use",
                "id": "tool_2",
                "id": "tool_2",
                "name": "mcp__issues__directive_get",
                "name": "mcp__issues__directive_get",
                "input": {"id": "d-001"}
                "input": {"id": "d-001"}
            }]
            }]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-2".to_string(),
        uuid: "uuid-2".to_string(),
        session_id: session_id.to_string(),
        session_id: session_id.to_string(),
    }));
    }));


    // issue_create (make new work)
    // issue_create (make new work)
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
    collector.process_message(&SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [{
            "content": [{
                "type": "tool_use",
                "type": "tool_use",
                "id": "tool_3",
                "id": "tool_3",
                "name": "mcp__issues__issue_create",
                "name": "mcp__issues__issue_create",
                "input": {
                "input": {
                    "title": "New task",
                    "title": "New task",
                    "directive_id": "d-001"
                    "directive_id": "d-001"
                }
                }
            }]
            }]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-3".to_string(),
        uuid: "uuid-3".to_string(),
        session_id: session_id.to_string(),
        session_id: session_id.to_string(),
    }));
    }));


    send_success(&mut collector, session_id, 3);
    send_success(&mut collector, session_id, 3);


    // Verify directive exploration is tracked
    // Verify directive exploration is tracked
    let trajectory = collector.into_trajectory();
    let trajectory = collector.into_trajectory();
    assert!(trajectory.steps.iter().any(|s| {
    assert!(trajectory.steps.iter().any(|s| {
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__directive_get")
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__directive_get")
    }));
    }));
    assert!(trajectory.steps.iter().any(|s| {
    assert!(trajectory.steps.iter().any(|s| {
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_create")
        matches!(&s.step_type, autopilot::trajectory::StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_create")
    }));
    }));
}
}
