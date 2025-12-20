//! Unit tests for the trajectory module

use autopilot::trajectory::{Step, StepType, TokenUsage, Trajectory, TrajectoryResult};
use serde_json::json;

#[test]
fn test_trajectory_creation() {
    let traj = Trajectory::new(
        "Test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
    );

    assert_eq!(traj.prompt, "Test prompt");
    assert_eq!(traj.model, "claude-sonnet-4");
    assert_eq!(traj.cwd, "/test/cwd");
    assert_eq!(traj.repo_sha, "abc123");
    assert_eq!(traj.branch, Some("main".to_string()));
    assert_eq!(traj.steps.len(), 0);
    assert!(traj.result.is_none());
    assert!(traj.ended_at.is_none());
}

#[test]
fn test_trajectory_without_branch() {
    let traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    assert_eq!(traj.branch, None);
}

#[test]
fn test_add_user_step() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::User {
        content: "Hello".to_string(),
    });

    assert_eq!(traj.steps.len(), 1);
    assert_eq!(traj.steps[0].step_id, 1);

    match &traj.steps[0].step_type {
        StepType::User { content } => assert_eq!(content, "Hello"),
        _ => panic!("Expected User step"),
    }
}

#[test]
fn test_add_assistant_step() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::Assistant {
        content: "Hi there!".to_string(),
    });

    assert_eq!(traj.steps.len(), 1);

    match &traj.steps[0].step_type {
        StepType::Assistant { content } => assert_eq!(content, "Hi there!"),
        _ => panic!("Expected Assistant step"),
    }
}

#[test]
fn test_add_thinking_step() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::Thinking {
        content: "Let me think...".to_string(),
        signature: Some("sig_abc123".to_string()),
    });

    match &traj.steps[0].step_type {
        StepType::Thinking { content, signature } => {
            assert_eq!(content, "Let me think...");
            assert_eq!(signature, &Some("sig_abc123".to_string()));
        }
        _ => panic!("Expected Thinking step"),
    }
}

#[test]
fn test_add_thinking_step_without_signature() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::Thinking {
        content: "Thinking".to_string(),
        signature: None,
    });

    match &traj.steps[0].step_type {
        StepType::Thinking { signature, .. } => assert_eq!(signature, &None),
        _ => panic!("Expected Thinking step"),
    }
}

#[test]
fn test_add_tool_call_step() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool_id: "toolu_abc123".to_string(),
        input: json!({"file_path": "/test/file.rs"}),
    });

    match &traj.steps[0].step_type {
        StepType::ToolCall { tool, tool_id, input } => {
            assert_eq!(tool, "Read");
            assert_eq!(tool_id, "toolu_abc123");
            assert_eq!(input["file_path"], "/test/file.rs");
        }
        _ => panic!("Expected ToolCall step"),
    }
}

#[test]
fn test_add_tool_result_success() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::ToolResult {
        tool_id: "toolu_abc123".to_string(),
        success: true,
        output: Some("File contents".to_string()),
    });

    match &traj.steps[0].step_type {
        StepType::ToolResult { tool_id, success, output } => {
            assert_eq!(tool_id, "toolu_abc123");
            assert_eq!(*success, true);
            assert_eq!(output, &Some("File contents".to_string()));
        }
        _ => panic!("Expected ToolResult step"),
    }
}

#[test]
fn test_add_tool_result_failure() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::ToolResult {
        tool_id: "toolu_xyz".to_string(),
        success: false,
        output: Some("Error: file not found".to_string()),
    });

    match &traj.steps[0].step_type {
        StepType::ToolResult { success, output, .. } => {
            assert_eq!(*success, false);
            assert!(output.as_ref().unwrap().contains("Error"));
        }
        _ => panic!("Expected ToolResult step"),
    }
}

#[test]
fn test_add_system_init_step() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::SystemInit {
        model: "claude-sonnet-4-5".to_string(),
    });

    match &traj.steps[0].step_type {
        StepType::SystemInit { model } => assert_eq!(model, "claude-sonnet-4-5"),
        _ => panic!("Expected SystemInit step"),
    }
}

#[test]
fn test_add_system_status_step() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::SystemStatus {
        status: "Processing request".to_string(),
    });

    match &traj.steps[0].step_type {
        StepType::SystemStatus { status } => assert_eq!(status, "Processing request"),
        _ => panic!("Expected SystemStatus step"),
    }
}

#[test]
fn test_step_id_increments() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::User { content: "1".to_string() });
    traj.add_step(StepType::Assistant { content: "2".to_string() });
    traj.add_step(StepType::User { content: "3".to_string() });

    assert_eq!(traj.steps[0].step_id, 1);
    assert_eq!(traj.steps[1].step_id, 2);
    assert_eq!(traj.steps[2].step_id, 3);
}

#[test]
fn test_add_step_with_token_metadata() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    let step = traj.add_step(StepType::Assistant {
        content: "Response".to_string(),
    });

    step.tokens_in = Some(1200);
    step.tokens_out = Some(450);
    step.tokens_cached = Some(100);

    assert_eq!(traj.steps[0].tokens_in, Some(1200));
    assert_eq!(traj.steps[0].tokens_out, Some(450));
    assert_eq!(traj.steps[0].tokens_cached, Some(100));
}

#[test]
fn test_step_content_user() {
    let step = Step {
        step_id: 1,
        timestamp: chrono::Utc::now(),
        step_type: StepType::User {
            content: "Test content".to_string(),
        },
        tokens_in: None,
        tokens_out: None,
        tokens_cached: None,
    };

    assert_eq!(step.content(), Some("Test content"));
}

#[test]
fn test_step_content_assistant() {
    let step = Step {
        step_id: 1,
        timestamp: chrono::Utc::now(),
        step_type: StepType::Assistant {
            content: "Response".to_string(),
        },
        tokens_in: None,
        tokens_out: None,
        tokens_cached: None,
    };

    assert_eq!(step.content(), Some("Response"));
}

#[test]
fn test_step_content_thinking() {
    let step = Step {
        step_id: 1,
        timestamp: chrono::Utc::now(),
        step_type: StepType::Thinking {
            content: "Thinking...".to_string(),
            signature: None,
        },
        tokens_in: None,
        tokens_out: None,
        tokens_cached: None,
    };

    assert_eq!(step.content(), Some("Thinking..."));
}

#[test]
fn test_step_content_tool_call() {
    let step = Step {
        step_id: 1,
        timestamp: chrono::Utc::now(),
        step_type: StepType::ToolCall {
            tool: "Read".to_string(),
            tool_id: "tool_1".to_string(),
            input: json!({}),
        },
        tokens_in: None,
        tokens_out: None,
        tokens_cached: None,
    };

    assert_eq!(step.content(), None);
}

#[test]
fn test_step_content_tool_result() {
    let step = Step {
        step_id: 1,
        timestamp: chrono::Utc::now(),
        step_type: StepType::ToolResult {
            tool_id: "tool_1".to_string(),
            success: true,
            output: Some("Result".to_string()),
        },
        tokens_in: None,
        tokens_out: None,
        tokens_cached: None,
    };

    assert_eq!(step.content(), Some("Result"));
}

#[test]
fn test_step_content_system_init() {
    let step = Step {
        step_id: 1,
        timestamp: chrono::Utc::now(),
        step_type: StepType::SystemInit {
            model: "claude".to_string(),
        },
        tokens_in: None,
        tokens_out: None,
        tokens_cached: None,
    };

    assert_eq!(step.content(), Some("claude"));
}

#[test]
fn test_step_content_system_status() {
    let step = Step {
        step_id: 1,
        timestamp: chrono::Utc::now(),
        step_type: StepType::SystemStatus {
            status: "Running".to_string(),
        },
        tokens_in: None,
        tokens_out: None,
        tokens_cached: None,
    };

    assert_eq!(step.content(), Some("Running"));
}

#[test]
fn test_token_usage_default() {
    let usage = TokenUsage::default();

    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.output_tokens, 0);
    assert_eq!(usage.cache_read_tokens, 0);
    assert_eq!(usage.cache_creation_tokens, 0);
    assert_eq!(usage.cost_usd, 0.0);
}

#[test]
fn test_token_usage_creation() {
    let usage = TokenUsage {
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_tokens: 200,
        cache_creation_tokens: 100,
        cost_usd: 0.05,
    };

    assert_eq!(usage.input_tokens, 1000);
    assert_eq!(usage.output_tokens, 500);
    assert_eq!(usage.cache_read_tokens, 200);
    assert_eq!(usage.cache_creation_tokens, 100);
    assert_eq!(usage.cost_usd, 0.05);
}

#[test]
fn test_trajectory_result_success() {
    let result = TrajectoryResult {
        success: true,
        duration_ms: 5000,
        num_turns: 3,
        result_text: Some("Task completed".to_string()),
        errors: Vec::new(),
        issues_completed: 2,
    };

    assert_eq!(result.success, true);
    assert_eq!(result.duration_ms, 5000);
    assert_eq!(result.num_turns, 3);
    assert_eq!(result.result_text, Some("Task completed".to_string()));
    assert_eq!(result.errors.len(), 0);
    assert_eq!(result.issues_completed, 2);
}

#[test]
fn test_trajectory_result_failure() {
    let result = TrajectoryResult {
        success: false,
        duration_ms: 2000,
        num_turns: 1,
        result_text: None,
        errors: vec!["Error 1".to_string(), "Error 2".to_string()],
        issues_completed: 0,
    };

    assert_eq!(result.success, false);
    assert_eq!(result.errors.len(), 2);
    assert_eq!(result.errors[0], "Error 1");
    assert_eq!(result.errors[1], "Error 2");
}

#[test]
fn test_trajectory_serialization() {
    let mut traj = Trajectory::new(
        "Test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
    );

    traj.add_step(StepType::User {
        content: "Hello".to_string(),
    });

    let json = traj.to_json();
    assert!(json.contains("Test prompt"));
    assert!(json.contains("claude-sonnet-4"));
    assert!(json.contains("Hello"));
}

#[test]
fn test_trajectory_deserialization() {
    let json = r#"{
        "session_id": "test-123",
        "prompt": "Test prompt",
        "model": "claude",
        "cwd": "/test",
        "repo_sha": "abc",
        "branch": "main",
        "started_at": "2024-01-01T00:00:00Z",
        "steps": [],
        "usage": {
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_read_tokens": 10,
            "cache_creation_tokens": 5,
            "cost_usd": 0.01
        }
    }"#;

    let traj: Trajectory = serde_json::from_str(json).unwrap();
    assert_eq!(traj.session_id, "test-123");
    assert_eq!(traj.prompt, "Test prompt");
    assert_eq!(traj.model, "claude");
    assert_eq!(traj.usage.input_tokens, 100);
}

#[test]
fn test_step_serialization() {
    let step = Step {
        step_id: 1,
        timestamp: chrono::Utc::now(),
        step_type: StepType::User {
            content: "Test".to_string(),
        },
        tokens_in: Some(100),
        tokens_out: Some(50),
        tokens_cached: None,
    };

    let json = serde_json::to_string(&step).unwrap();
    assert!(json.contains("\"type\":\"user\""));
    assert!(json.contains("Test"));
    assert!(json.contains("tokens_in"));
}

#[test]
fn test_trajectory_with_result() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.result = Some(TrajectoryResult {
        success: true,
        duration_ms: 1000,
        num_turns: 2,
        result_text: Some("Done".to_string()),
        errors: Vec::new(),
        issues_completed: 1,
    });

    assert!(traj.result.is_some());
    assert_eq!(traj.result.as_ref().unwrap().success, true);
    assert_eq!(traj.result.as_ref().unwrap().issues_completed, 1);
}

#[test]
fn test_multiple_steps_complete_workflow() {
    let mut traj = Trajectory::new(
        "Fix a bug".to_string(),
        "claude-sonnet-4".to_string(),
        "/project".to_string(),
        "commit-sha".to_string(),
        Some("feature-branch".to_string()),
    );

    // User request
    traj.add_step(StepType::User {
        content: "Fix the authentication bug".to_string(),
    });

    // Assistant thinking
    traj.add_step(StepType::Thinking {
        content: "I need to check the auth module".to_string(),
        signature: None,
    });

    // Tool call
    traj.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool_id: "tool_1".to_string(),
        input: json!({"file_path": "src/auth.rs"}),
    });

    // Tool result
    traj.add_step(StepType::ToolResult {
        tool_id: "tool_1".to_string(),
        success: true,
        output: Some("File contents...".to_string()),
    });

    // Assistant response
    traj.add_step(StepType::Assistant {
        content: "Found the issue on line 42".to_string(),
    });

    assert_eq!(traj.steps.len(), 5);
    assert_eq!(traj.steps[0].step_id, 1);
    assert_eq!(traj.steps[4].step_id, 5);
}

#[test]
fn test_trajectory_usage_updates() {
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.usage.input_tokens = 1500;
    traj.usage.output_tokens = 750;
    traj.usage.cost_usd = 0.0234;

    assert_eq!(traj.usage.input_tokens, 1500);
    assert_eq!(traj.usage.output_tokens, 750);
    assert_eq!(traj.usage.cost_usd, 0.0234);
}

#[test]
fn test_empty_trajectory_json() {
    let traj = Trajectory::new(
        "Empty".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    let json = traj.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["steps"].as_array().unwrap().len(), 0);
    assert!(parsed["result"].is_null());
}
