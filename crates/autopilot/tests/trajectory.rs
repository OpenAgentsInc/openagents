//! Unit tests for the trajectory module
//! Unit tests for the trajectory module


use autopilot::trajectory::{Step, StepType, TokenUsage, Trajectory, TrajectoryResult};
use autopilot::trajectory::{Step, StepType, TokenUsage, Trajectory, TrajectoryResult};
use serde_json::json;
use serde_json::json;


#[test]
#[test]
fn test_trajectory_creation() {
fn test_trajectory_creation() {
    let traj = Trajectory::new(
    let traj = Trajectory::new(
        "Test prompt".to_string(),
        "Test prompt".to_string(),
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


    assert_eq!(traj.prompt, "Test prompt");
    assert_eq!(traj.prompt, "Test prompt");
    assert_eq!(traj.model, "claude-sonnet-4");
    assert_eq!(traj.model, "claude-sonnet-4");
    assert_eq!(traj.cwd, "/test/cwd");
    assert_eq!(traj.cwd, "/test/cwd");
    assert_eq!(traj.repo_sha, "abc123");
    assert_eq!(traj.repo_sha, "abc123");
    assert_eq!(traj.branch, Some("main".to_string()));
    assert_eq!(traj.branch, Some("main".to_string()));
    assert_eq!(traj.steps.len(), 0);
    assert_eq!(traj.steps.len(), 0);
    assert!(traj.result.is_none());
    assert!(traj.result.is_none());
    assert!(traj.ended_at.is_none());
    assert!(traj.ended_at.is_none());
}
}


#[test]
#[test]
fn test_trajectory_without_branch() {
fn test_trajectory_without_branch() {
    let traj = Trajectory::new(
    let traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    assert_eq!(traj.branch, None);
    assert_eq!(traj.branch, None);
}
}


#[test]
#[test]
fn test_add_user_step() {
fn test_add_user_step() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.add_step(StepType::User {
    traj.add_step(StepType::User {
        content: "Hello".to_string(),
        content: "Hello".to_string(),
    });
    });


    assert_eq!(traj.steps.len(), 1);
    assert_eq!(traj.steps.len(), 1);
    assert_eq!(traj.steps[0].step_id, 1);
    assert_eq!(traj.steps[0].step_id, 1);


    match &traj.steps[0].step_type {
    match &traj.steps[0].step_type {
        StepType::User { content } => assert_eq!(content, "Hello"),
        StepType::User { content } => assert_eq!(content, "Hello"),
        _ => panic!("Expected User step"),
        _ => panic!("Expected User step"),
    }
    }
}
}


#[test]
#[test]
fn test_add_assistant_step() {
fn test_add_assistant_step() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.add_step(StepType::Assistant {
    traj.add_step(StepType::Assistant {
        content: "Hi there!".to_string(),
        content: "Hi there!".to_string(),
    });
    });


    assert_eq!(traj.steps.len(), 1);
    assert_eq!(traj.steps.len(), 1);


    match &traj.steps[0].step_type {
    match &traj.steps[0].step_type {
        StepType::Assistant { content } => assert_eq!(content, "Hi there!"),
        StepType::Assistant { content } => assert_eq!(content, "Hi there!"),
        _ => panic!("Expected Assistant step"),
        _ => panic!("Expected Assistant step"),
    }
    }
}
}


#[test]
#[test]
fn test_add_thinking_step() {
fn test_add_thinking_step() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.add_step(StepType::Thinking {
    traj.add_step(StepType::Thinking {
        content: "Let me think...".to_string(),
        content: "Let me think...".to_string(),
        signature: Some("sig_abc123".to_string()),
        signature: Some("sig_abc123".to_string()),
    });
    });


    match &traj.steps[0].step_type {
    match &traj.steps[0].step_type {
        StepType::Thinking { content, signature } => {
        StepType::Thinking { content, signature } => {
            assert_eq!(content, "Let me think...");
            assert_eq!(content, "Let me think...");
            assert_eq!(signature, &Some("sig_abc123".to_string()));
            assert_eq!(signature, &Some("sig_abc123".to_string()));
        }
        }
        _ => panic!("Expected Thinking step"),
        _ => panic!("Expected Thinking step"),
    }
    }
}
}


#[test]
#[test]
fn test_add_thinking_step_without_signature() {
fn test_add_thinking_step_without_signature() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.add_step(StepType::Thinking {
    traj.add_step(StepType::Thinking {
        content: "Thinking".to_string(),
        content: "Thinking".to_string(),
        signature: None,
        signature: None,
    });
    });


    match &traj.steps[0].step_type {
    match &traj.steps[0].step_type {
        StepType::Thinking { signature, .. } => assert_eq!(signature, &None),
        StepType::Thinking { signature, .. } => assert_eq!(signature, &None),
        _ => panic!("Expected Thinking step"),
        _ => panic!("Expected Thinking step"),
    }
    }
}
}


#[test]
#[test]
fn test_add_tool_call_step() {
fn test_add_tool_call_step() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.add_step(StepType::ToolCall {
    traj.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool: "Read".to_string(),
        tool_id: "toolu_abc123".to_string(),
        tool_id: "toolu_abc123".to_string(),
        input: json!({"file_path": "/test/file.rs"}),
        input: json!({"file_path": "/test/file.rs"}),
    });
    });


    match &traj.steps[0].step_type {
    match &traj.steps[0].step_type {
        StepType::ToolCall { tool, tool_id, input } => {
        StepType::ToolCall { tool, tool_id, input } => {
            assert_eq!(tool, "Read");
            assert_eq!(tool, "Read");
            assert_eq!(tool_id, "toolu_abc123");
            assert_eq!(tool_id, "toolu_abc123");
            assert_eq!(input["file_path"], "/test/file.rs");
            assert_eq!(input["file_path"], "/test/file.rs");
        }
        }
        _ => panic!("Expected ToolCall step"),
        _ => panic!("Expected ToolCall step"),
    }
    }
}
}


#[test]
#[test]
fn test_add_tool_result_success() {
fn test_add_tool_result_success() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.add_step(StepType::ToolResult {
    traj.add_step(StepType::ToolResult {
        tool_id: "toolu_abc123".to_string(),
        tool_id: "toolu_abc123".to_string(),
        success: true,
        success: true,
        output: Some("File contents".to_string()),
        output: Some("File contents".to_string()),
    });
    });


    match &traj.steps[0].step_type {
    match &traj.steps[0].step_type {
        StepType::ToolResult { tool_id, success, output } => {
        StepType::ToolResult { tool_id, success, output } => {
            assert_eq!(tool_id, "toolu_abc123");
            assert_eq!(tool_id, "toolu_abc123");
            assert_eq!(*success, true);
            assert_eq!(*success, true);
            assert_eq!(output, &Some("File contents".to_string()));
            assert_eq!(output, &Some("File contents".to_string()));
        }
        }
        _ => panic!("Expected ToolResult step"),
        _ => panic!("Expected ToolResult step"),
    }
    }
}
}


#[test]
#[test]
fn test_add_tool_result_failure() {
fn test_add_tool_result_failure() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.add_step(StepType::ToolResult {
    traj.add_step(StepType::ToolResult {
        tool_id: "toolu_xyz".to_string(),
        tool_id: "toolu_xyz".to_string(),
        success: false,
        success: false,
        output: Some("Error: file not found".to_string()),
        output: Some("Error: file not found".to_string()),
    });
    });


    match &traj.steps[0].step_type {
    match &traj.steps[0].step_type {
        StepType::ToolResult { success, output, .. } => {
        StepType::ToolResult { success, output, .. } => {
            assert_eq!(*success, false);
            assert_eq!(*success, false);
            assert!(output.as_ref().unwrap().contains("Error"));
            assert!(output.as_ref().unwrap().contains("Error"));
        }
        }
        _ => panic!("Expected ToolResult step"),
        _ => panic!("Expected ToolResult step"),
    }
    }
}
}


#[test]
#[test]
fn test_add_system_init_step() {
fn test_add_system_init_step() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.add_step(StepType::SystemInit {
    traj.add_step(StepType::SystemInit {
        model: "claude-sonnet-4-5".to_string(),
        model: "claude-sonnet-4-5".to_string(),
    });
    });


    match &traj.steps[0].step_type {
    match &traj.steps[0].step_type {
        StepType::SystemInit { model } => assert_eq!(model, "claude-sonnet-4-5"),
        StepType::SystemInit { model } => assert_eq!(model, "claude-sonnet-4-5"),
        _ => panic!("Expected SystemInit step"),
        _ => panic!("Expected SystemInit step"),
    }
    }
}
}


#[test]
#[test]
fn test_add_system_status_step() {
fn test_add_system_status_step() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.add_step(StepType::SystemStatus {
    traj.add_step(StepType::SystemStatus {
        status: "Processing request".to_string(),
        status: "Processing request".to_string(),
    });
    });


    match &traj.steps[0].step_type {
    match &traj.steps[0].step_type {
        StepType::SystemStatus { status } => assert_eq!(status, "Processing request"),
        StepType::SystemStatus { status } => assert_eq!(status, "Processing request"),
        _ => panic!("Expected SystemStatus step"),
        _ => panic!("Expected SystemStatus step"),
    }
    }
}
}


#[test]
#[test]
fn test_step_id_increments() {
fn test_step_id_increments() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.add_step(StepType::User { content: "1".to_string() });
    traj.add_step(StepType::User { content: "1".to_string() });
    traj.add_step(StepType::Assistant { content: "2".to_string() });
    traj.add_step(StepType::Assistant { content: "2".to_string() });
    traj.add_step(StepType::User { content: "3".to_string() });
    traj.add_step(StepType::User { content: "3".to_string() });


    assert_eq!(traj.steps[0].step_id, 1);
    assert_eq!(traj.steps[0].step_id, 1);
    assert_eq!(traj.steps[1].step_id, 2);
    assert_eq!(traj.steps[1].step_id, 2);
    assert_eq!(traj.steps[2].step_id, 3);
    assert_eq!(traj.steps[2].step_id, 3);
}
}


#[test]
#[test]
fn test_add_step_with_token_metadata() {
fn test_add_step_with_token_metadata() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    let step = traj.add_step(StepType::Assistant {
    let step = traj.add_step(StepType::Assistant {
        content: "Response".to_string(),
        content: "Response".to_string(),
    });
    });


    step.tokens_in = Some(1200);
    step.tokens_in = Some(1200);
    step.tokens_out = Some(450);
    step.tokens_out = Some(450);
    step.tokens_cached = Some(100);
    step.tokens_cached = Some(100);


    assert_eq!(traj.steps[0].tokens_in, Some(1200));
    assert_eq!(traj.steps[0].tokens_in, Some(1200));
    assert_eq!(traj.steps[0].tokens_out, Some(450));
    assert_eq!(traj.steps[0].tokens_out, Some(450));
    assert_eq!(traj.steps[0].tokens_cached, Some(100));
    assert_eq!(traj.steps[0].tokens_cached, Some(100));
}
}


#[test]
#[test]
fn test_step_content_user() {
fn test_step_content_user() {
    let step = Step {
    let step = Step {
        step_id: 1,
        step_id: 1,
        timestamp: chrono::Utc::now(),
        timestamp: chrono::Utc::now(),
        step_type: StepType::User {
        step_type: StepType::User {
            content: "Test content".to_string(),
            content: "Test content".to_string(),
        },
        },
        tokens_in: None,
        tokens_in: None,
        tokens_out: None,
        tokens_out: None,
        tokens_cached: None,
        tokens_cached: None,
    };
    };


    assert_eq!(step.content(), Some("Test content"));
    assert_eq!(step.content(), Some("Test content"));
}
}


#[test]
#[test]
fn test_step_content_assistant() {
fn test_step_content_assistant() {
    let step = Step {
    let step = Step {
        step_id: 1,
        step_id: 1,
        timestamp: chrono::Utc::now(),
        timestamp: chrono::Utc::now(),
        step_type: StepType::Assistant {
        step_type: StepType::Assistant {
            content: "Response".to_string(),
            content: "Response".to_string(),
        },
        },
        tokens_in: None,
        tokens_in: None,
        tokens_out: None,
        tokens_out: None,
        tokens_cached: None,
        tokens_cached: None,
    };
    };


    assert_eq!(step.content(), Some("Response"));
    assert_eq!(step.content(), Some("Response"));
}
}


#[test]
#[test]
fn test_step_content_thinking() {
fn test_step_content_thinking() {
    let step = Step {
    let step = Step {
        step_id: 1,
        step_id: 1,
        timestamp: chrono::Utc::now(),
        timestamp: chrono::Utc::now(),
        step_type: StepType::Thinking {
        step_type: StepType::Thinking {
            content: "Thinking...".to_string(),
            content: "Thinking...".to_string(),
            signature: None,
            signature: None,
        },
        },
        tokens_in: None,
        tokens_in: None,
        tokens_out: None,
        tokens_out: None,
        tokens_cached: None,
        tokens_cached: None,
    };
    };


    assert_eq!(step.content(), Some("Thinking..."));
    assert_eq!(step.content(), Some("Thinking..."));
}
}


#[test]
#[test]
fn test_step_content_tool_call() {
fn test_step_content_tool_call() {
    let step = Step {
    let step = Step {
        step_id: 1,
        step_id: 1,
        timestamp: chrono::Utc::now(),
        timestamp: chrono::Utc::now(),
        step_type: StepType::ToolCall {
        step_type: StepType::ToolCall {
            tool: "Read".to_string(),
            tool: "Read".to_string(),
            tool_id: "tool_1".to_string(),
            tool_id: "tool_1".to_string(),
            input: json!({}),
            input: json!({}),
        },
        },
        tokens_in: None,
        tokens_in: None,
        tokens_out: None,
        tokens_out: None,
        tokens_cached: None,
        tokens_cached: None,
    };
    };


    assert_eq!(step.content(), None);
    assert_eq!(step.content(), None);
}
}


#[test]
#[test]
fn test_step_content_tool_result() {
fn test_step_content_tool_result() {
    let step = Step {
    let step = Step {
        step_id: 1,
        step_id: 1,
        timestamp: chrono::Utc::now(),
        timestamp: chrono::Utc::now(),
        step_type: StepType::ToolResult {
        step_type: StepType::ToolResult {
            tool_id: "tool_1".to_string(),
            tool_id: "tool_1".to_string(),
            success: true,
            success: true,
            output: Some("Result".to_string()),
            output: Some("Result".to_string()),
        },
        },
        tokens_in: None,
        tokens_in: None,
        tokens_out: None,
        tokens_out: None,
        tokens_cached: None,
        tokens_cached: None,
    };
    };


    assert_eq!(step.content(), Some("Result"));
    assert_eq!(step.content(), Some("Result"));
}
}


#[test]
#[test]
fn test_step_content_system_init() {
fn test_step_content_system_init() {
    let step = Step {
    let step = Step {
        step_id: 1,
        step_id: 1,
        timestamp: chrono::Utc::now(),
        timestamp: chrono::Utc::now(),
        step_type: StepType::SystemInit {
        step_type: StepType::SystemInit {
            model: "claude".to_string(),
            model: "claude".to_string(),
        },
        },
        tokens_in: None,
        tokens_in: None,
        tokens_out: None,
        tokens_out: None,
        tokens_cached: None,
        tokens_cached: None,
    };
    };


    assert_eq!(step.content(), Some("claude"));
    assert_eq!(step.content(), Some("claude"));
}
}


#[test]
#[test]
fn test_step_content_system_status() {
fn test_step_content_system_status() {
    let step = Step {
    let step = Step {
        step_id: 1,
        step_id: 1,
        timestamp: chrono::Utc::now(),
        timestamp: chrono::Utc::now(),
        step_type: StepType::SystemStatus {
        step_type: StepType::SystemStatus {
            status: "Running".to_string(),
            status: "Running".to_string(),
        },
        },
        tokens_in: None,
        tokens_in: None,
        tokens_out: None,
        tokens_out: None,
        tokens_cached: None,
        tokens_cached: None,
    };
    };


    assert_eq!(step.content(), Some("Running"));
    assert_eq!(step.content(), Some("Running"));
}
}


#[test]
#[test]
fn test_token_usage_default() {
fn test_token_usage_default() {
    let usage = TokenUsage::default();
    let usage = TokenUsage::default();


    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.output_tokens, 0);
    assert_eq!(usage.output_tokens, 0);
    assert_eq!(usage.cache_read_tokens, 0);
    assert_eq!(usage.cache_read_tokens, 0);
    assert_eq!(usage.cache_creation_tokens, 0);
    assert_eq!(usage.cache_creation_tokens, 0);
    assert_eq!(usage.cost_usd, 0.0);
    assert_eq!(usage.cost_usd, 0.0);
}
}


#[test]
#[test]
fn test_token_usage_creation() {
fn test_token_usage_creation() {
    let usage = TokenUsage {
    let usage = TokenUsage {
        input_tokens: 1000,
        input_tokens: 1000,
        output_tokens: 500,
        output_tokens: 500,
        cache_read_tokens: 200,
        cache_read_tokens: 200,
        cache_creation_tokens: 100,
        cache_creation_tokens: 100,
        cost_usd: 0.05,
        cost_usd: 0.05,
    };
    };


    assert_eq!(usage.input_tokens, 1000);
    assert_eq!(usage.input_tokens, 1000);
    assert_eq!(usage.output_tokens, 500);
    assert_eq!(usage.output_tokens, 500);
    assert_eq!(usage.cache_read_tokens, 200);
    assert_eq!(usage.cache_read_tokens, 200);
    assert_eq!(usage.cache_creation_tokens, 100);
    assert_eq!(usage.cache_creation_tokens, 100);
    assert_eq!(usage.cost_usd, 0.05);
    assert_eq!(usage.cost_usd, 0.05);
}
}


#[test]
#[test]
fn test_trajectory_result_success() {
fn test_trajectory_result_success() {
    let result = TrajectoryResult {
    let result = TrajectoryResult {
        success: true,
        success: true,
        duration_ms: 5000,
        duration_ms: 5000,
        num_turns: 3,
        num_turns: 3,
        result_text: Some("Task completed".to_string()),
        result_text: Some("Task completed".to_string()),
        errors: Vec::new(),
        errors: Vec::new(),
        issues_completed: 2,
        issues_completed: 2,
                apm: Some(20.0),
                apm: Some(20.0),
    };
    };


    assert_eq!(result.success, true);
    assert_eq!(result.success, true);
    assert_eq!(result.duration_ms, 5000);
    assert_eq!(result.duration_ms, 5000);
    assert_eq!(result.num_turns, 3);
    assert_eq!(result.num_turns, 3);
    assert_eq!(result.result_text, Some("Task completed".to_string()));
    assert_eq!(result.result_text, Some("Task completed".to_string()));
    assert_eq!(result.errors.len(), 0);
    assert_eq!(result.errors.len(), 0);
    assert_eq!(result.issues_completed, 2);
    assert_eq!(result.issues_completed, 2);
}
}


#[test]
#[test]
fn test_trajectory_result_failure() {
fn test_trajectory_result_failure() {
    let result = TrajectoryResult {
    let result = TrajectoryResult {
        success: false,
        success: false,
        duration_ms: 2000,
        duration_ms: 2000,
        num_turns: 1,
        num_turns: 1,
        result_text: None,
        result_text: None,
        errors: vec!["Error 1".to_string(), "Error 2".to_string()],
        errors: vec!["Error 1".to_string(), "Error 2".to_string()],
        issues_completed: 0,
        issues_completed: 0,
                apm: Some(20.0),
                apm: Some(20.0),
    };
    };


    assert_eq!(result.success, false);
    assert_eq!(result.success, false);
    assert_eq!(result.errors.len(), 2);
    assert_eq!(result.errors.len(), 2);
    assert_eq!(result.errors[0], "Error 1");
    assert_eq!(result.errors[0], "Error 1");
    assert_eq!(result.errors[1], "Error 2");
    assert_eq!(result.errors[1], "Error 2");
}
}


#[test]
#[test]
fn test_trajectory_serialization() {
fn test_trajectory_serialization() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test prompt".to_string(),
        "Test prompt".to_string(),
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


    traj.add_step(StepType::User {
    traj.add_step(StepType::User {
        content: "Hello".to_string(),
        content: "Hello".to_string(),
    });
    });


    let json = traj.to_json();
    let json = traj.to_json();
    assert!(json.contains("Test prompt"));
    assert!(json.contains("Test prompt"));
    assert!(json.contains("claude-sonnet-4"));
    assert!(json.contains("claude-sonnet-4"));
    assert!(json.contains("Hello"));
    assert!(json.contains("Hello"));
}
}


#[test]
#[test]
fn test_trajectory_deserialization() {
fn test_trajectory_deserialization() {
    let json = r#"{
    let json = r#"{
        "session_id": "test-123",
        "session_id": "test-123",
        "prompt": "Test prompt",
        "prompt": "Test prompt",
        "model": "claude",
        "model": "claude",
        "cwd": "/test",
        "cwd": "/test",
        "repo_sha": "abc",
        "repo_sha": "abc",
        "branch": "main",
        "branch": "main",
        "started_at": "2024-01-01T00:00:00Z",
        "started_at": "2024-01-01T00:00:00Z",
        "steps": [],
        "steps": [],
        "usage": {
        "usage": {
            "input_tokens": 100,
            "input_tokens": 100,
            "output_tokens": 50,
            "output_tokens": 50,
            "cache_read_tokens": 10,
            "cache_read_tokens": 10,
            "cache_creation_tokens": 5,
            "cache_creation_tokens": 5,
            "cost_usd": 0.01
            "cost_usd": 0.01
        }
        }
    }"#;
    }"#;


    let traj: Trajectory = serde_json::from_str(json).unwrap();
    let traj: Trajectory = serde_json::from_str(json).unwrap();
    assert_eq!(traj.session_id, "test-123");
    assert_eq!(traj.session_id, "test-123");
    assert_eq!(traj.prompt, "Test prompt");
    assert_eq!(traj.prompt, "Test prompt");
    assert_eq!(traj.model, "claude");
    assert_eq!(traj.model, "claude");
    assert_eq!(traj.usage.input_tokens, 100);
    assert_eq!(traj.usage.input_tokens, 100);
}
}


#[test]
#[test]
fn test_step_serialization() {
fn test_step_serialization() {
    let step = Step {
    let step = Step {
        step_id: 1,
        step_id: 1,
        timestamp: chrono::Utc::now(),
        timestamp: chrono::Utc::now(),
        step_type: StepType::User {
        step_type: StepType::User {
            content: "Test".to_string(),
            content: "Test".to_string(),
        },
        },
        tokens_in: Some(100),
        tokens_in: Some(100),
        tokens_out: Some(50),
        tokens_out: Some(50),
        tokens_cached: None,
        tokens_cached: None,
    };
    };


    let json = serde_json::to_string(&step).unwrap();
    let json = serde_json::to_string(&step).unwrap();
    assert!(json.contains("\"type\":\"user\""));
    assert!(json.contains("\"type\":\"user\""));
    assert!(json.contains("Test"));
    assert!(json.contains("Test"));
    assert!(json.contains("tokens_in"));
    assert!(json.contains("tokens_in"));
}
}


#[test]
#[test]
fn test_trajectory_with_result() {
fn test_trajectory_with_result() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.result = Some(TrajectoryResult {
    traj.result = Some(TrajectoryResult {
        success: true,
        success: true,
        duration_ms: 1000,
        duration_ms: 1000,
        num_turns: 2,
        num_turns: 2,
        result_text: Some("Done".to_string()),
        result_text: Some("Done".to_string()),
        errors: Vec::new(),
        errors: Vec::new(),
        issues_completed: 1,
        issues_completed: 1,
                apm: Some(20.0),
                apm: Some(20.0),
    });
    });


    assert!(traj.result.is_some());
    assert!(traj.result.is_some());
    assert_eq!(traj.result.as_ref().unwrap().success, true);
    assert_eq!(traj.result.as_ref().unwrap().success, true);
    assert_eq!(traj.result.as_ref().unwrap().issues_completed, 1);
    assert_eq!(traj.result.as_ref().unwrap().issues_completed, 1);
}
}


#[test]
#[test]
fn test_multiple_steps_complete_workflow() {
fn test_multiple_steps_complete_workflow() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Fix a bug".to_string(),
        "Fix a bug".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-sonnet-4".to_string(),
        "/project".to_string(),
        "/project".to_string(),
        "commit-sha".to_string(),
        "commit-sha".to_string(),
        Some("feature-branch".to_string()),
        Some("feature-branch".to_string()),
    );
    );


    // User request
    // User request
    traj.add_step(StepType::User {
    traj.add_step(StepType::User {
        content: "Fix the authentication bug".to_string(),
        content: "Fix the authentication bug".to_string(),
    });
    });


    // Assistant thinking
    // Assistant thinking
    traj.add_step(StepType::Thinking {
    traj.add_step(StepType::Thinking {
        content: "I need to check the auth module".to_string(),
        content: "I need to check the auth module".to_string(),
        signature: None,
        signature: None,
    });
    });


    // Tool call
    // Tool call
    traj.add_step(StepType::ToolCall {
    traj.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool: "Read".to_string(),
        tool_id: "tool_1".to_string(),
        tool_id: "tool_1".to_string(),
        input: json!({"file_path": "src/auth.rs"}),
        input: json!({"file_path": "src/auth.rs"}),
    });
    });


    // Tool result
    // Tool result
    traj.add_step(StepType::ToolResult {
    traj.add_step(StepType::ToolResult {
        tool_id: "tool_1".to_string(),
        tool_id: "tool_1".to_string(),
        success: true,
        success: true,
        output: Some("File contents...".to_string()),
        output: Some("File contents...".to_string()),
    });
    });


    // Assistant response
    // Assistant response
    traj.add_step(StepType::Assistant {
    traj.add_step(StepType::Assistant {
        content: "Found the issue on line 42".to_string(),
        content: "Found the issue on line 42".to_string(),
    });
    });


    assert_eq!(traj.steps.len(), 5);
    assert_eq!(traj.steps.len(), 5);
    assert_eq!(traj.steps[0].step_id, 1);
    assert_eq!(traj.steps[0].step_id, 1);
    assert_eq!(traj.steps[4].step_id, 5);
    assert_eq!(traj.steps[4].step_id, 5);
}
}


#[test]
#[test]
fn test_trajectory_usage_updates() {
fn test_trajectory_usage_updates() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj.usage.input_tokens = 1500;
    traj.usage.input_tokens = 1500;
    traj.usage.output_tokens = 750;
    traj.usage.output_tokens = 750;
    traj.usage.cost_usd = 0.0234;
    traj.usage.cost_usd = 0.0234;


    assert_eq!(traj.usage.input_tokens, 1500);
    assert_eq!(traj.usage.input_tokens, 1500);
    assert_eq!(traj.usage.output_tokens, 750);
    assert_eq!(traj.usage.output_tokens, 750);
    assert_eq!(traj.usage.cost_usd, 0.0234);
    assert_eq!(traj.usage.cost_usd, 0.0234);
}
}


#[test]
#[test]
fn test_empty_trajectory_json() {
fn test_empty_trajectory_json() {
    let traj = Trajectory::new(
    let traj = Trajectory::new(
        "Empty".to_string(),
        "Empty".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    let json = traj.to_json();
    let json = traj.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();


    assert_eq!(parsed["steps"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["steps"].as_array().unwrap().len(), 0);
    assert!(parsed["result"].is_null());
    assert!(parsed["result"].is_null());
}
}


// ============================================================================
// ============================================================================
// Error Handling Tests
// Error Handling Tests
// ============================================================================
// ============================================================================


#[test]
#[test]
fn test_trajectory_with_empty_session_id() {
fn test_trajectory_with_empty_session_id() {
    let traj = Trajectory::new(
    let traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    // Session ID starts empty
    // Session ID starts empty
    assert_eq!(traj.session_id, "");
    assert_eq!(traj.session_id, "");


    // Should be able to serialize even with empty session_id
    // Should be able to serialize even with empty session_id
    let json = traj.to_json();
    let json = traj.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["session_id"], "");
    assert_eq!(parsed["session_id"], "");
}
}


#[test]
#[test]
fn test_trajectory_with_very_long_session_id() {
fn test_trajectory_with_very_long_session_id() {
    use autopilot::TrajectoryCollector;
    use autopilot::TrajectoryCollector;


    let mut collector = TrajectoryCollector::new(
    let mut collector = TrajectoryCollector::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    // Set an extremely long session ID (simulating corruption or attack)
    // Set an extremely long session ID (simulating corruption or attack)
    let long_id = "x".repeat(10000);
    let long_id = "x".repeat(10000);
    collector.set_session_id(long_id.clone());
    collector.set_session_id(long_id.clone());


    let traj = collector.into_trajectory();
    let traj = collector.into_trajectory();
    assert_eq!(traj.session_id, long_id);
    assert_eq!(traj.session_id, long_id);


    // Should still serialize successfully
    // Should still serialize successfully
    let json = traj.to_json();
    let json = traj.to_json();
    assert!(json.contains(&long_id));
    assert!(json.contains(&long_id));
}
}


#[test]
#[test]
fn test_corrupted_trajectory_json_parsing() {
fn test_corrupted_trajectory_json_parsing() {
    // Test parsing various corrupted JSON inputs
    // Test parsing various corrupted JSON inputs
    let test_cases = vec![
    let test_cases = vec![
        (r#"{}"#, "empty object"),
        (r#"{}"#, "empty object"),
        (r#"{"steps": null}"#, "null steps"),
        (r#"{"steps": null}"#, "null steps"),
        (r#"{"steps": "invalid"}"#, "invalid steps type"),
        (r#"{"steps": "invalid"}"#, "invalid steps type"),
        (r#"{"usage": "invalid"}"#, "invalid usage"),
        (r#"{"usage": "invalid"}"#, "invalid usage"),
        (r#"{"started_at": "not a date"}"#, "invalid date"),
        (r#"{"started_at": "not a date"}"#, "invalid date"),
    ];
    ];


    for (json_str, desc) in test_cases {
    for (json_str, desc) in test_cases {
        let result: Result<Trajectory, _> = serde_json::from_str(json_str);
        let result: Result<Trajectory, _> = serde_json::from_str(json_str);
        // All of these should fail to parse
        // All of these should fail to parse
        assert!(result.is_err(), "Should fail to parse: {}", desc);
        assert!(result.is_err(), "Should fail to parse: {}", desc);
    }
    }
}
}


#[test]
#[test]
fn test_trajectory_with_extremely_large_step_count() {
fn test_trajectory_with_extremely_large_step_count() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Large test".to_string(),
        "Large test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    // Add 10,000 steps to test memory handling
    // Add 10,000 steps to test memory handling
    for i in 0..10000 {
    for i in 0..10000 {
        traj.add_step(StepType::User {
        traj.add_step(StepType::User {
            content: format!("Step {}", i),
            content: format!("Step {}", i),
        });
        });
    }
    }


    assert_eq!(traj.steps.len(), 10000);
    assert_eq!(traj.steps.len(), 10000);


    // Should be able to serialize (though it will be large)
    // Should be able to serialize (though it will be large)
    let json = traj.to_json();
    let json = traj.to_json();
    assert!(json.len() > 100000); // At least 100KB
    assert!(json.len() > 100000); // At least 100KB


    // Should be able to deserialize
    // Should be able to deserialize
    let parsed: Trajectory = serde_json::from_str(&json).unwrap();
    let parsed: Trajectory = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.steps.len(), 10000);
    assert_eq!(parsed.steps.len(), 10000);
}
}


#[test]
#[test]
fn test_trajectory_with_large_step_content() {
fn test_trajectory_with_large_step_content() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Large content test".to_string(),
        "Large content test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    // Add a step with 1MB of content
    // Add a step with 1MB of content
    let large_content = "x".repeat(1024 * 1024);
    let large_content = "x".repeat(1024 * 1024);
    traj.add_step(StepType::User {
    traj.add_step(StepType::User {
        content: large_content,
        content: large_content,
    });
    });


    assert_eq!(traj.steps.len(), 1);
    assert_eq!(traj.steps.len(), 1);


    // Verify the content matches (can't use assert_eq due to size)
    // Verify the content matches (can't use assert_eq due to size)
    if let StepType::User { content } = &traj.steps[0].step_type {
    if let StepType::User { content } = &traj.steps[0].step_type {
        assert_eq!(content.len(), 1024 * 1024);
        assert_eq!(content.len(), 1024 * 1024);
    } else {
    } else {
        panic!("Expected User step type");
        panic!("Expected User step type");
    }
    }


    // Should be able to serialize
    // Should be able to serialize
    let json = traj.to_json();
    let json = traj.to_json();
    assert!(json.len() > 1024 * 1024);
    assert!(json.len() > 1024 * 1024);
}
}


#[test]
#[test]
fn test_concurrent_step_additions() {
fn test_concurrent_step_additions() {
    use std::sync::{Arc, Mutex};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::thread;


    let traj = Arc::new(Mutex::new(Trajectory::new(
    let traj = Arc::new(Mutex::new(Trajectory::new(
        "Concurrent test".to_string(),
        "Concurrent test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    )));
    )));


    let mut handles = vec![];
    let mut handles = vec![];


    // Spawn 10 threads each adding 100 steps
    // Spawn 10 threads each adding 100 steps
    for thread_id in 0..10 {
    for thread_id in 0..10 {
        let traj_clone = Arc::clone(&traj);
        let traj_clone = Arc::clone(&traj);
        let handle = thread::spawn(move || {
        let handle = thread::spawn(move || {
            for i in 0..100 {
            for i in 0..100 {
                let mut traj = traj_clone.lock().unwrap();
                let mut traj = traj_clone.lock().unwrap();
                traj.add_step(StepType::User {
                traj.add_step(StepType::User {
                    content: format!("Thread {} step {}", thread_id, i),
                    content: format!("Thread {} step {}", thread_id, i),
                });
                });
            }
            }
        });
        });
        handles.push(handle);
        handles.push(handle);
    }
    }


    // Wait for all threads to complete
    // Wait for all threads to complete
    for handle in handles {
    for handle in handles {
        handle.join().unwrap();
        handle.join().unwrap();
    }
    }


    // Should have 1000 total steps
    // Should have 1000 total steps
    let traj = traj.lock().unwrap();
    let traj = traj.lock().unwrap();
    assert_eq!(traj.steps.len(), 1000);
    assert_eq!(traj.steps.len(), 1000);


    // All step IDs should be unique and sequential
    // All step IDs should be unique and sequential
    for (i, step) in traj.steps.iter().enumerate() {
    for (i, step) in traj.steps.iter().enumerate() {
        assert_eq!(step.step_id as usize, i + 1);
        assert_eq!(step.step_id as usize, i + 1);
    }
    }
}
}


#[test]
#[test]
fn test_trajectory_with_invalid_utf8_sequences() {
fn test_trajectory_with_invalid_utf8_sequences() {
    // Test handling of potentially invalid UTF-8 in content
    // Test handling of potentially invalid UTF-8 in content
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "UTF-8 test".to_string(),
        "UTF-8 test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    // Add various unicode edge cases
    // Add various unicode edge cases
    let test_strings = vec![
    let test_strings = vec![
        "\u{0000}",              // Null character
        "\u{0000}",              // Null character
        "\u{FFFD}",              // Replacement character
        "\u{FFFD}",              // Replacement character
        "🚀🎉💻",                 // Emojis
        "🚀🎉💻",                 // Emojis
        "日本語",                 // Japanese
        "日本語",                 // Japanese
        "🏴󠁧󠁢󠁥󠁮󠁧󠁿",                  // Flag emoji
        "🏴󠁧󠁢󠁥󠁮󠁧󠁿",                  // Flag emoji
        "\n\r\t",                // Control characters
        "\n\r\t",                // Control characters
    ];
    ];


    let expected_len = test_strings.len();
    let expected_len = test_strings.len();


    for s in test_strings {
    for s in test_strings {
        traj.add_step(StepType::User {
        traj.add_step(StepType::User {
            content: s.to_string(),
            content: s.to_string(),
        });
        });
    }
    }


    // Should serialize without panicking
    // Should serialize without panicking
    let json = traj.to_json();
    let json = traj.to_json();
    let parsed: Trajectory = serde_json::from_str(&json).unwrap();
    let parsed: Trajectory = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.steps.len(), expected_len);
    assert_eq!(parsed.steps.len(), expected_len);
}
}


#[test]
#[test]
fn test_trajectory_serialization_with_special_characters() {
fn test_trajectory_serialization_with_special_characters() {
    let mut traj = Trajectory::new(
    let mut traj = Trajectory::new(
        "Special chars test".to_string(),
        "Special chars test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    // Add content with JSON-problematic characters
    // Add content with JSON-problematic characters
    traj.add_step(StepType::User {
    traj.add_step(StepType::User {
        content: r#"{"key": "value", "quote": "\"", "newline": "\n"}"#.to_string(),
        content: r#"{"key": "value", "quote": "\"", "newline": "\n"}"#.to_string(),
    });
    });


    traj.add_step(StepType::ToolCall {
    traj.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool: "Read".to_string(),
        tool_id: "test-id".to_string(),
        tool_id: "test-id".to_string(),
        input: json!({
        input: json!({
            "path": "/path/with\"quotes/and\nnewlines",
            "path": "/path/with\"quotes/and\nnewlines",
            "special": "</script><script>alert('xss')</script>"
            "special": "</script><script>alert('xss')</script>"
        }),
        }),
    });
    });


    // Should serialize and deserialize correctly
    // Should serialize and deserialize correctly
    let json_str = traj.to_json();
    let json_str = traj.to_json();
    let parsed: Trajectory = serde_json::from_str(&json_str).unwrap();
    let parsed: Trajectory = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.steps.len(), 2);
    assert_eq!(parsed.steps.len(), 2);
}
}


#[test]
#[test]
fn test_trajectory_with_missing_optional_fields() {
fn test_trajectory_with_missing_optional_fields() {
    // Test deserialization with minimal JSON (only required fields)
    // Test deserialization with minimal JSON (only required fields)
    let minimal_json = r#"{
    let minimal_json = r#"{
        "session_id": "",
        "session_id": "",
        "prompt": "Test",
        "prompt": "Test",
        "model": "claude",
        "model": "claude",
        "cwd": "/test",
        "cwd": "/test",
        "repo_sha": "sha",
        "repo_sha": "sha",
        "started_at": "2024-01-01T00:00:00Z",
        "started_at": "2024-01-01T00:00:00Z",
        "steps": [],
        "steps": [],
        "usage": {
        "usage": {
            "input_tokens": 0,
            "input_tokens": 0,
            "output_tokens": 0,
            "output_tokens": 0,
            "cache_read_tokens": 0,
            "cache_read_tokens": 0,
            "cache_creation_tokens": 0,
            "cache_creation_tokens": 0,
            "cost_usd": 0.0
            "cost_usd": 0.0
        }
        }
    }"#;
    }"#;


    let traj: Trajectory = serde_json::from_str(minimal_json).unwrap();
    let traj: Trajectory = serde_json::from_str(minimal_json).unwrap();
    assert_eq!(traj.prompt, "Test");
    assert_eq!(traj.prompt, "Test");
    assert_eq!(traj.branch, None);
    assert_eq!(traj.branch, None);
    assert_eq!(traj.ended_at, None);
    assert_eq!(traj.ended_at, None);
    assert!(traj.result.is_none());
    assert!(traj.result.is_none());
}
}


#[test]
#[test]
fn test_trajectory_token_usage_overflow() {
fn test_trajectory_token_usage_overflow() {
    use autopilot::TrajectoryCollector;
    use autopilot::TrajectoryCollector;


    let collector = TrajectoryCollector::new(
    let collector = TrajectoryCollector::new(
        "Overflow test".to_string(),
        "Overflow test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    let traj = collector.into_trajectory();
    let traj = collector.into_trajectory();


    // Token counts should handle large numbers
    // Token counts should handle large numbers
    assert_eq!(traj.usage.input_tokens, 0);
    assert_eq!(traj.usage.input_tokens, 0);
    assert_eq!(traj.usage.output_tokens, 0);
    assert_eq!(traj.usage.output_tokens, 0);


    // Max u64 should be representable
    // Max u64 should be representable
    let max_tokens = u64::MAX;
    let max_tokens = u64::MAX;
    let mut traj_with_max = Trajectory::new(
    let mut traj_with_max = Trajectory::new(
        "Max test".to_string(),
        "Max test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None,
        None,
    );
    );


    traj_with_max.usage.input_tokens = max_tokens;
    traj_with_max.usage.input_tokens = max_tokens;


    // Should serialize without overflow
    // Should serialize without overflow
    let json = traj_with_max.to_json();
    let json = traj_with_max.to_json();
    let parsed: Trajectory = serde_json::from_str(&json).unwrap();
    let parsed: Trajectory = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.usage.input_tokens, max_tokens);
    assert_eq!(parsed.usage.input_tokens, max_tokens);
}
}
