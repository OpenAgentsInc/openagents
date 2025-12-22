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

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_trajectory_with_empty_session_id() {
    let traj = Trajectory::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    // Session ID starts empty
    assert_eq!(traj.session_id, "");

    // Should be able to serialize even with empty session_id
    let json = traj.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["session_id"], "");
}

#[test]
fn test_trajectory_with_very_long_session_id() {
    use autopilot::TrajectoryCollector;

    let mut collector = TrajectoryCollector::new(
        "Test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    // Set an extremely long session ID (simulating corruption or attack)
    let long_id = "x".repeat(10000);
    collector.set_session_id(long_id.clone());

    let traj = collector.into_trajectory();
    assert_eq!(traj.session_id, long_id);

    // Should still serialize successfully
    let json = traj.to_json();
    assert!(json.contains(&long_id));
}

#[test]
fn test_corrupted_trajectory_json_parsing() {
    // Test parsing various corrupted JSON inputs
    let test_cases = vec![
        (r#"{}"#, "empty object"),
        (r#"{"steps": null}"#, "null steps"),
        (r#"{"steps": "invalid"}"#, "invalid steps type"),
        (r#"{"usage": "invalid"}"#, "invalid usage"),
        (r#"{"started_at": "not a date"}"#, "invalid date"),
    ];

    for (json_str, desc) in test_cases {
        let result: Result<Trajectory, _> = serde_json::from_str(json_str);
        // All of these should fail to parse
        assert!(result.is_err(), "Should fail to parse: {}", desc);
    }
}

#[test]
fn test_trajectory_with_extremely_large_step_count() {
    let mut traj = Trajectory::new(
        "Large test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    // Add 10,000 steps to test memory handling
    for i in 0..10000 {
        traj.add_step(StepType::User {
            content: format!("Step {}", i),
        });
    }

    assert_eq!(traj.steps.len(), 10000);

    // Should be able to serialize (though it will be large)
    let json = traj.to_json();
    assert!(json.len() > 100000); // At least 100KB

    // Should be able to deserialize
    let parsed: Trajectory = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.steps.len(), 10000);
}

#[test]
fn test_trajectory_with_large_step_content() {
    let mut traj = Trajectory::new(
        "Large content test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    // Add a step with 1MB of content
    let large_content = "x".repeat(1024 * 1024);
    traj.add_step(StepType::User {
        content: large_content,
    });

    assert_eq!(traj.steps.len(), 1);

    // Verify the content matches (can't use assert_eq due to size)
    if let StepType::User { content } = &traj.steps[0].step_type {
        assert_eq!(content.len(), 1024 * 1024);
    } else {
        panic!("Expected User step type");
    }

    // Should be able to serialize
    let json = traj.to_json();
    assert!(json.len() > 1024 * 1024);
}

#[test]
fn test_concurrent_step_additions() {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let traj = Arc::new(Mutex::new(Trajectory::new(
        "Concurrent test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    )));

    let mut handles = vec![];

    // Spawn 10 threads each adding 100 steps
    for thread_id in 0..10 {
        let traj_clone = Arc::clone(&traj);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let mut traj = traj_clone.lock().unwrap();
                traj.add_step(StepType::User {
                    content: format!("Thread {} step {}", thread_id, i),
                });
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Should have 1000 total steps
    let traj = traj.lock().unwrap();
    assert_eq!(traj.steps.len(), 1000);

    // All step IDs should be unique and sequential
    for (i, step) in traj.steps.iter().enumerate() {
        assert_eq!(step.step_id as usize, i + 1);
    }
}

#[test]
fn test_trajectory_with_invalid_utf8_sequences() {
    // Test handling of potentially invalid UTF-8 in content
    let mut traj = Trajectory::new(
        "UTF-8 test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    // Add various unicode edge cases
    let test_strings = vec![
        "\u{0000}",              // Null character
        "\u{FFFD}",              // Replacement character
        "🚀🎉💻",                 // Emojis
        "日本語",                 // Japanese
        "🏴󠁧󠁢󠁥󠁮󠁧󠁿",                  // Flag emoji
        "\n\r\t",                // Control characters
    ];

    let expected_len = test_strings.len();

    for s in test_strings {
        traj.add_step(StepType::User {
            content: s.to_string(),
        });
    }

    // Should serialize without panicking
    let json = traj.to_json();
    let parsed: Trajectory = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.steps.len(), expected_len);
}

#[test]
fn test_trajectory_serialization_with_special_characters() {
    let mut traj = Trajectory::new(
        "Special chars test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    // Add content with JSON-problematic characters
    traj.add_step(StepType::User {
        content: r#"{"key": "value", "quote": "\"", "newline": "\n"}"#.to_string(),
    });

    traj.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool_id: "test-id".to_string(),
        input: json!({
            "path": "/path/with\"quotes/and\nnewlines",
            "special": "</script><script>alert('xss')</script>"
        }),
    });

    // Should serialize and deserialize correctly
    let json_str = traj.to_json();
    let parsed: Trajectory = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.steps.len(), 2);
}

#[test]
fn test_trajectory_with_missing_optional_fields() {
    // Test deserialization with minimal JSON (only required fields)
    let minimal_json = r#"{
        "session_id": "",
        "prompt": "Test",
        "model": "claude",
        "cwd": "/test",
        "repo_sha": "sha",
        "started_at": "2024-01-01T00:00:00Z",
        "steps": [],
        "usage": {
            "input_tokens": 0,
            "output_tokens": 0,
            "cache_read_tokens": 0,
            "cache_creation_tokens": 0,
            "cost_usd": 0.0
        }
    }"#;

    let traj: Trajectory = serde_json::from_str(minimal_json).unwrap();
    assert_eq!(traj.prompt, "Test");
    assert_eq!(traj.branch, None);
    assert_eq!(traj.ended_at, None);
    assert!(traj.result.is_none());
}

#[test]
fn test_trajectory_token_usage_overflow() {
    use autopilot::TrajectoryCollector;

    let collector = TrajectoryCollector::new(
        "Overflow test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    let traj = collector.into_trajectory();

    // Token counts should handle large numbers
    assert_eq!(traj.usage.input_tokens, 0);
    assert_eq!(traj.usage.output_tokens, 0);

    // Max u64 should be representable
    let max_tokens = u64::MAX;
    let mut traj_with_max = Trajectory::new(
        "Max test".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj_with_max.usage.input_tokens = max_tokens;

    // Should serialize without overflow
    let json = traj_with_max.to_json();
    let parsed: Trajectory = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.usage.input_tokens, max_tokens);
}
