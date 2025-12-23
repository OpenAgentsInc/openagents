//! Integration tests for autopilot trajectory tracking
//! Integration tests for autopilot trajectory tracking


use autopilot::TrajectoryCollector;
use autopilot::TrajectoryCollector;
use claude_agent_sdk::{
use claude_agent_sdk::{
    SdkMessage, SdkAssistantMessage, SdkResultMessage, SdkSystemMessage,
    SdkMessage, SdkAssistantMessage, SdkResultMessage, SdkSystemMessage,
    ResultSuccess, ResultError, Usage,
    ResultSuccess, ResultError, Usage,
    protocol::SystemInit,
    protocol::SystemInit,
};
};
use serde_json::json;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashMap;


#[test]
#[test]
fn test_issue_completion_tracking() {
fn test_issue_completion_tracking() {
    let mut collector = TrajectoryCollector::new(
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
        Some("main".to_string()),
    );
    );


    // Initialize trajectory result
    // Initialize trajectory result
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
            session_id: "test-session".to_string(),
            session_id: "test-session".to_string(),
        }
        }
    ));
    ));
    collector.process_message(&init_msg);
    collector.process_message(&init_msg);


    // Add a success result to initialize trajectory.result
    // Add a success result to initialize trajectory.result
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
            num_turns: 5,
            num_turns: 5,
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
            uuid: "uuid-1".to_string(),
            uuid: "uuid-1".to_string(),
            session_id: "test-session".to_string(),
            session_id: "test-session".to_string(),
        }
        }
    ));
    ));
    collector.process_message(&success_msg);
    collector.process_message(&success_msg);


    // Verify initial state
    // Verify initial state
    let trajectory = collector.into_trajectory();
    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.result.as_ref().unwrap().issues_completed, 0);
    assert_eq!(trajectory.result.as_ref().unwrap().issues_completed, 0);


    // Create a new collector and simulate issue_complete calls
    // Create a new collector and simulate issue_complete calls
    let mut collector = TrajectoryCollector::new(
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
        Some("main".to_string()),
    );
    );


    collector.process_message(&init_msg);
    collector.process_message(&init_msg);
    collector.process_message(&success_msg);
    collector.process_message(&success_msg);


    // Simulate first issue_complete tool call
    // Simulate first issue_complete tool call
    let assistant_msg = SdkMessage::Assistant(SdkAssistantMessage {
    let assistant_msg = SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [
            "content": [
                {
                {
                    "type": "tool_use",
                    "type": "tool_use",
                    "id": "tool_1",
                    "id": "tool_1",
                    "name": "mcp__issues__issue_complete",
                    "name": "mcp__issues__issue_complete",
                    "input": {"number": 1}
                    "input": {"number": 1}
                }
                }
            ]
            ]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-1".to_string(),
        uuid: "uuid-1".to_string(),
        session_id: "test-session".to_string(),
        session_id: "test-session".to_string(),
    });
    });
    collector.process_message(&assistant_msg);
    collector.process_message(&assistant_msg);


    // Simulate second issue_complete tool call
    // Simulate second issue_complete tool call
    let assistant_msg2 = SdkMessage::Assistant(SdkAssistantMessage {
    let assistant_msg2 = SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [
            "content": [
                {
                {
                    "type": "tool_use",
                    "type": "tool_use",
                    "id": "tool_2",
                    "id": "tool_2",
                    "name": "mcp__issues__issue_complete",
                    "name": "mcp__issues__issue_complete",
                    "input": {"number": 2}
                    "input": {"number": 2}
                }
                }
            ]
            ]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-2".to_string(),
        uuid: "uuid-2".to_string(),
        session_id: "test-session".to_string(),
        session_id: "test-session".to_string(),
    });
    });
    collector.process_message(&assistant_msg2);
    collector.process_message(&assistant_msg2);


    // Verify count is tracked
    // Verify count is tracked
    let trajectory = collector.into_trajectory();
    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.result.as_ref().unwrap().issues_completed, 2);
    assert_eq!(trajectory.result.as_ref().unwrap().issues_completed, 2);
}
}


#[test]
#[test]
fn test_trajectory_serialization() {
fn test_trajectory_serialization() {
    let mut collector = TrajectoryCollector::new(
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
        Some("main".to_string()),
    );
    );


    // Add a system init message
    // Add a system init message
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
            session_id: "test-session".to_string(),
            session_id: "test-session".to_string(),
        }
        }
    ));
    ));
    collector.process_message(&init_msg);
    collector.process_message(&init_msg);


    // Add assistant message with tool call
    // Add assistant message with tool call
    let assistant_msg = SdkMessage::Assistant(SdkAssistantMessage {
    let assistant_msg = SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [
            "content": [
                {
                {
                    "type": "text",
                    "type": "text",
                    "text": "Let me help with that."
                    "text": "Let me help with that."
                },
                },
                {
                {
                    "type": "tool_use",
                    "type": "tool_use",
                    "id": "tool_1",
                    "id": "tool_1",
                    "name": "Read",
                    "name": "Read",
                    "input": {"file_path": "/test/file.rs"}
                    "input": {"file_path": "/test/file.rs"}
                }
                }
            ]
            ]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-1".to_string(),
        uuid: "uuid-1".to_string(),
        session_id: "test-session".to_string(),
        session_id: "test-session".to_string(),
    });
    });
    collector.process_message(&assistant_msg);
    collector.process_message(&assistant_msg);


    // Add result message
    // Add result message
    let result_msg = SdkMessage::Result(SdkResultMessage::Success(
    let result_msg = SdkMessage::Result(SdkResultMessage::Success(
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
            num_turns: 3,
            num_turns: 3,
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
            uuid: "uuid-result".to_string(),
            uuid: "uuid-result".to_string(),
            session_id: "test-session".to_string(),
            session_id: "test-session".to_string(),
        }
        }
    ));
    ));
    collector.process_message(&result_msg);
    collector.process_message(&result_msg);


    let trajectory = collector.into_trajectory();
    let trajectory = collector.into_trajectory();


    // Verify serialization works
    // Verify serialization works
    let json = serde_json::to_string(&trajectory).expect("Failed to serialize trajectory");
    let json = serde_json::to_string(&trajectory).expect("Failed to serialize trajectory");
    assert!(json.contains("test-session"));
    assert!(json.contains("test-session"));
    assert!(json.contains("claude-sonnet-4"));
    assert!(json.contains("claude-sonnet-4"));
    assert!(json.contains("test prompt"));
    assert!(json.contains("test prompt"));


    // Verify deserialization works
    // Verify deserialization works
    let deserialized: autopilot::trajectory::Trajectory =
    let deserialized: autopilot::trajectory::Trajectory =
        serde_json::from_str(&json).expect("Failed to deserialize trajectory");
        serde_json::from_str(&json).expect("Failed to deserialize trajectory");
    assert_eq!(deserialized.session_id, "test-session");
    assert_eq!(deserialized.session_id, "test-session");
    assert_eq!(deserialized.prompt, "test prompt");
    assert_eq!(deserialized.prompt, "test prompt");
    assert_eq!(deserialized.steps.len(), 3); // init, text, tool_use
    assert_eq!(deserialized.steps.len(), 3); // init, text, tool_use
}
}


#[test]
#[test]
fn test_token_usage_tracking() {
fn test_token_usage_tracking() {
    let mut collector = TrajectoryCollector::new(
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        "abc123".to_string(),
        None,
        None,
    );
    );


    // Add result message with token usage
    // Add result message with token usage
    let result_msg = SdkMessage::Result(SdkResultMessage::Success(
    let result_msg = SdkMessage::Result(SdkResultMessage::Success(
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
            num_turns: 3,
            num_turns: 3,
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
            uuid: "uuid-1".to_string(),
            uuid: "uuid-1".to_string(),
            session_id: "test-session".to_string(),
            session_id: "test-session".to_string(),
        }
        }
    ));
    ));
    collector.process_message(&result_msg);
    collector.process_message(&result_msg);


    let trajectory = collector.into_trajectory();
    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.usage.input_tokens, 100);
    assert_eq!(trajectory.usage.input_tokens, 100);
    assert_eq!(trajectory.usage.output_tokens, 50);
    assert_eq!(trajectory.usage.output_tokens, 50);
    assert_eq!(trajectory.usage.cache_read_tokens, 20);
    assert_eq!(trajectory.usage.cache_read_tokens, 20);
    assert_eq!(trajectory.usage.cache_creation_tokens, 10);
    assert_eq!(trajectory.usage.cache_creation_tokens, 10);
    assert_eq!(trajectory.usage.cost_usd, 0.05);
    assert_eq!(trajectory.usage.cost_usd, 0.05);
}
}


#[test]
#[test]
fn test_error_result_tracking() {
fn test_error_result_tracking() {
    let mut collector = TrajectoryCollector::new(
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        "abc123".to_string(),
        None,
        None,
    );
    );


    // Add error result
    // Add error result
    let error_msg = SdkMessage::Result(SdkResultMessage::ErrorDuringExecution(
    let error_msg = SdkMessage::Result(SdkResultMessage::ErrorDuringExecution(
        ResultError {
        ResultError {
            duration_ms: 500,
            duration_ms: 500,
            duration_api_ms: 450,
            duration_api_ms: 450,
            is_error: true,
            is_error: true,
            num_turns: 2,
            num_turns: 2,
            usage: Usage {
            usage: Usage {
                input_tokens: 50,
                input_tokens: 50,
                output_tokens: 25,
                output_tokens: 25,
                cache_read_input_tokens: Some(10),
                cache_read_input_tokens: Some(10),
                cache_creation_input_tokens: Some(5),
                cache_creation_input_tokens: Some(5),
            },
            },
            total_cost_usd: 0.02,
            total_cost_usd: 0.02,
            model_usage: HashMap::new(),
            model_usage: HashMap::new(),
            permission_denials: Vec::new(),
            permission_denials: Vec::new(),
            errors: vec!["Test error".to_string()],
            errors: vec!["Test error".to_string()],
            uuid: "uuid-1".to_string(),
            uuid: "uuid-1".to_string(),
            session_id: "test-session".to_string(),
            session_id: "test-session".to_string(),
        }
        }
    ));
    ));
    collector.process_message(&error_msg);
    collector.process_message(&error_msg);


    let trajectory = collector.into_trajectory();
    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.result.as_ref().unwrap().success, false);
    assert_eq!(trajectory.result.as_ref().unwrap().success, false);
    assert_eq!(trajectory.result.as_ref().unwrap().errors.len(), 1);
    assert_eq!(trajectory.result.as_ref().unwrap().errors.len(), 1);
    assert_eq!(trajectory.result.as_ref().unwrap().issues_completed, 0);
    assert_eq!(trajectory.result.as_ref().unwrap().issues_completed, 0);
    assert_eq!(trajectory.usage.cost_usd, 0.02);
    assert_eq!(trajectory.usage.cost_usd, 0.02);
}
}


#[test]
#[test]
fn test_session_id_callback() {
fn test_session_id_callback() {
    use std::sync::{Arc, Mutex};
    use std::sync::{Arc, Mutex};


    let mut collector = TrajectoryCollector::new(
    let mut collector = TrajectoryCollector::new(
        "test prompt".to_string(),
        "test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        "abc123".to_string(),
        None,
        None,
    );
    );


    // Setup callback to capture session_id
    // Setup callback to capture session_id
    let captured_id = Arc::new(Mutex::new(None));
    let captured_id = Arc::new(Mutex::new(None));
    let captured_id_clone = captured_id.clone();
    let captured_id_clone = captured_id.clone();


    collector.on_session_id(move |session_id: &str| {
    collector.on_session_id(move |session_id: &str| {
        *captured_id_clone.lock().unwrap() = Some(session_id.to_string());
        *captured_id_clone.lock().unwrap() = Some(session_id.to_string());
    });
    });


    // Process Init message
    // Process Init message
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
            session_id: "callback-test-session".to_string(),
            session_id: "callback-test-session".to_string(),
        }
        }
    ));
    ));


    collector.process_message(&init_msg);
    collector.process_message(&init_msg);


    // Verify callback was invoked with correct session_id
    // Verify callback was invoked with correct session_id
    let captured = captured_id.lock().unwrap();
    let captured = captured_id.lock().unwrap();
    assert_eq!(captured.as_ref().unwrap(), "callback-test-session");
    assert_eq!(captured.as_ref().unwrap(), "callback-test-session");


    // Verify trajectory also has the session_id
    // Verify trajectory also has the session_id
    let trajectory = collector.into_trajectory();
    let trajectory = collector.into_trajectory();
    assert_eq!(trajectory.session_id, "callback-test-session");
    assert_eq!(trajectory.session_id, "callback-test-session");
}
}


#[test]
#[test]
fn test_trajectory_lifecycle() {
fn test_trajectory_lifecycle() {
    let mut collector = TrajectoryCollector::new(
    let mut collector = TrajectoryCollector::new(
        "test prompt for lifecycle".to_string(),
        "test prompt for lifecycle".to_string(),
        "claude-sonnet-4.5".to_string(),
        "claude-sonnet-4.5".to_string(),
        "/home/test".to_string(),
        "/home/test".to_string(),
        "commit-abc123".to_string(),
        "commit-abc123".to_string(),
        Some("feature-branch".to_string()),
        Some("feature-branch".to_string()),
    );
    );


    // Verify initial state
    // Verify initial state
    assert_eq!(collector.trajectory().session_id, "");
    assert_eq!(collector.trajectory().session_id, "");
    assert_eq!(collector.trajectory().prompt, "test prompt for lifecycle");
    assert_eq!(collector.trajectory().prompt, "test prompt for lifecycle");
    assert_eq!(collector.trajectory().model, "claude-sonnet-4.5");
    assert_eq!(collector.trajectory().model, "claude-sonnet-4.5");
    assert_eq!(collector.trajectory().cwd, "/home/test");
    assert_eq!(collector.trajectory().cwd, "/home/test");
    assert_eq!(collector.trajectory().repo_sha, "commit-abc123");
    assert_eq!(collector.trajectory().repo_sha, "commit-abc123");
    assert_eq!(collector.trajectory().branch, Some("feature-branch".to_string()));
    assert_eq!(collector.trajectory().branch, Some("feature-branch".to_string()));
    assert_eq!(collector.trajectory().steps.len(), 0);
    assert_eq!(collector.trajectory().steps.len(), 0);


    // Process init
    // Process init
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
            cwd: "/home/test".to_string(),
            cwd: "/home/test".to_string(),
            tools: Vec::new(),
            tools: Vec::new(),
            mcp_servers: Vec::new(),
            mcp_servers: Vec::new(),
            model: "claude-sonnet-4.5".to_string(),
            model: "claude-sonnet-4.5".to_string(),
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
            session_id: "lifecycle-session-123".to_string(),
            session_id: "lifecycle-session-123".to_string(),
        }
        }
    ));
    ));
    collector.process_message(&init_msg);
    collector.process_message(&init_msg);


    // Verify session_id is set
    // Verify session_id is set
    assert_eq!(collector.trajectory().session_id, "lifecycle-session-123");
    assert_eq!(collector.trajectory().session_id, "lifecycle-session-123");
    assert_eq!(collector.trajectory().steps.len(), 1);
    assert_eq!(collector.trajectory().steps.len(), 1);


    // Add some steps
    // Add some steps
    let assistant_msg = SdkMessage::Assistant(SdkAssistantMessage {
    let assistant_msg = SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [
            "content": [
                {
                {
                    "type": "text",
                    "type": "text",
                    "text": "Processing request..."
                    "text": "Processing request..."
                }
                }
            ]
            ]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-1".to_string(),
        uuid: "uuid-1".to_string(),
        session_id: "lifecycle-session-123".to_string(),
        session_id: "lifecycle-session-123".to_string(),
    });
    });
    collector.process_message(&assistant_msg);
    collector.process_message(&assistant_msg);


    assert_eq!(collector.trajectory().steps.len(), 2);
    assert_eq!(collector.trajectory().steps.len(), 2);


    // Add result
    // Add result
    let result_msg = SdkMessage::Result(SdkResultMessage::Success(
    let result_msg = SdkMessage::Result(SdkResultMessage::Success(
        ResultSuccess {
        ResultSuccess {
            result: "done".to_string(),
            result: "done".to_string(),
            is_error: false,
            is_error: false,
            duration_ms: 2000,
            duration_ms: 2000,
            duration_api_ms: 1800,
            duration_api_ms: 1800,
            num_turns: 1,
            num_turns: 1,
            usage: Usage {
            usage: Usage {
                input_tokens: 200,
                input_tokens: 200,
                output_tokens: 100,
                output_tokens: 100,
                cache_read_input_tokens: Some(50),
                cache_read_input_tokens: Some(50),
                cache_creation_input_tokens: Some(25),
                cache_creation_input_tokens: Some(25),
            },
            },
            total_cost_usd: 0.10,
            total_cost_usd: 0.10,
            model_usage: HashMap::new(),
            model_usage: HashMap::new(),
            permission_denials: Vec::new(),
            permission_denials: Vec::new(),
            structured_output: None,
            structured_output: None,
            uuid: "uuid-result".to_string(),
            uuid: "uuid-result".to_string(),
            session_id: "lifecycle-session-123".to_string(),
            session_id: "lifecycle-session-123".to_string(),
        }
        }
    ));
    ));
    collector.process_message(&result_msg);
    collector.process_message(&result_msg);


    // Verify ended_at is set and result is populated
    // Verify ended_at is set and result is populated
    assert!(collector.trajectory().ended_at.is_some());
    assert!(collector.trajectory().ended_at.is_some());
    assert!(collector.trajectory().result.is_some());
    assert!(collector.trajectory().result.is_some());
    assert_eq!(collector.trajectory().result.as_ref().unwrap().success, true);
    assert_eq!(collector.trajectory().result.as_ref().unwrap().success, true);
    assert_eq!(collector.trajectory().result.as_ref().unwrap().duration_ms, 2000);
    assert_eq!(collector.trajectory().result.as_ref().unwrap().duration_ms, 2000);
    assert_eq!(collector.trajectory().usage.input_tokens, 200);
    assert_eq!(collector.trajectory().usage.input_tokens, 200);
    assert_eq!(collector.trajectory().usage.output_tokens, 100);
    assert_eq!(collector.trajectory().usage.output_tokens, 100);


    // Finish trajectory
    // Finish trajectory
    let trajectory = collector.finish();
    let trajectory = collector.finish();
    assert_eq!(trajectory.steps.len(), 2);
    assert_eq!(trajectory.steps.len(), 2);
    assert_eq!(trajectory.session_id, "lifecycle-session-123");
    assert_eq!(trajectory.session_id, "lifecycle-session-123");
}
}


#[test]
#[test]
fn test_all_step_types() {
fn test_all_step_types() {
    let mut collector = TrajectoryCollector::new(
    let mut collector = TrajectoryCollector::new(
        "test".to_string(),
        "test".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-sonnet-4".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "abc".to_string(),
        "abc".to_string(),
        None,
        None,
    );
    );


    // SystemInit
    // SystemInit
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
            cwd: "/test".to_string(),
            cwd: "/test".to_string(),
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
            session_id: "test-session".to_string(),
            session_id: "test-session".to_string(),
        }
        }
    ));
    ));
    collector.process_message(&init_msg);
    collector.process_message(&init_msg);


    // Text message (Assistant)
    // Text message (Assistant)
    let text_msg = SdkMessage::Assistant(SdkAssistantMessage {
    let text_msg = SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [
            "content": [
                {
                {
                    "type": "text",
                    "type": "text",
                    "text": "Let me help."
                    "text": "Let me help."
                }
                }
            ]
            ]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-1".to_string(),
        uuid: "uuid-1".to_string(),
        session_id: "test-session".to_string(),
        session_id: "test-session".to_string(),
    });
    });
    collector.process_message(&text_msg);
    collector.process_message(&text_msg);


    // Thinking message
    // Thinking message
    let thinking_msg = SdkMessage::Assistant(SdkAssistantMessage {
    let thinking_msg = SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [
            "content": [
                {
                {
                    "type": "thinking",
                    "type": "thinking",
                    "thinking": "I need to analyze this...",
                    "thinking": "I need to analyze this...",
                    "signature": "sig-abc123"
                    "signature": "sig-abc123"
                }
                }
            ]
            ]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-2".to_string(),
        uuid: "uuid-2".to_string(),
        session_id: "test-session".to_string(),
        session_id: "test-session".to_string(),
    });
    });
    collector.process_message(&thinking_msg);
    collector.process_message(&thinking_msg);


    // ToolCall
    // ToolCall
    let tool_call_msg = SdkMessage::Assistant(SdkAssistantMessage {
    let tool_call_msg = SdkMessage::Assistant(SdkAssistantMessage {
        message: json!({
        message: json!({
            "role": "assistant",
            "role": "assistant",
            "content": [
            "content": [
                {
                {
                    "type": "tool_use",
                    "type": "tool_use",
                    "id": "tool-1",
                    "id": "tool-1",
                    "name": "Read",
                    "name": "Read",
                    "input": {"file_path": "/test.rs"}
                    "input": {"file_path": "/test.rs"}
                }
                }
            ]
            ]
        }),
        }),
        parent_tool_use_id: None,
        parent_tool_use_id: None,
        error: None,
        error: None,
        uuid: "uuid-3".to_string(),
        uuid: "uuid-3".to_string(),
        session_id: "test-session".to_string(),
        session_id: "test-session".to_string(),
    });
    });
    collector.process_message(&tool_call_msg);
    collector.process_message(&tool_call_msg);


    let trajectory = collector.into_trajectory();
    let trajectory = collector.into_trajectory();


    // Verify all step types are present
    // Verify all step types are present
    assert_eq!(trajectory.steps.len(), 4);
    assert_eq!(trajectory.steps.len(), 4);


    // Check step types
    // Check step types
    use autopilot::trajectory::StepType;
    use autopilot::trajectory::StepType;
    assert!(matches!(trajectory.steps[0].step_type, StepType::SystemInit { .. }));
    assert!(matches!(trajectory.steps[0].step_type, StepType::SystemInit { .. }));
    assert!(matches!(trajectory.steps[1].step_type, StepType::Assistant { .. }));
    assert!(matches!(trajectory.steps[1].step_type, StepType::Assistant { .. }));
    assert!(matches!(trajectory.steps[2].step_type, StepType::Thinking { .. }));
    assert!(matches!(trajectory.steps[2].step_type, StepType::Thinking { .. }));
    assert!(matches!(trajectory.steps[3].step_type, StepType::ToolCall { .. }));
    assert!(matches!(trajectory.steps[3].step_type, StepType::ToolCall { .. }));


    // Verify thinking has signature
    // Verify thinking has signature
    if let StepType::Thinking { signature, .. } = &trajectory.steps[2].step_type {
    if let StepType::Thinking { signature, .. } = &trajectory.steps[2].step_type {
        assert_eq!(signature.as_ref().unwrap(), "sig-abc123");
        assert_eq!(signature.as_ref().unwrap(), "sig-abc123");
    } else {
    } else {
        panic!("Expected Thinking step");
        panic!("Expected Thinking step");
    }
    }


    // Verify tool call has correct details
    // Verify tool call has correct details
    if let StepType::ToolCall { tool, tool_id, input } = &trajectory.steps[3].step_type {
    if let StepType::ToolCall { tool, tool_id, input } = &trajectory.steps[3].step_type {
        assert_eq!(tool, "Read");
        assert_eq!(tool, "Read");
        assert_eq!(tool_id, "tool-1");
        assert_eq!(tool_id, "tool-1");
        assert_eq!(input.get("file_path").and_then(|v| v.as_str()), Some("/test.rs"));
        assert_eq!(input.get("file_path").and_then(|v| v.as_str()), Some("/test.rs"));
    } else {
    } else {
        panic!("Expected ToolCall step");
        panic!("Expected ToolCall step");
    }
    }
}
}
