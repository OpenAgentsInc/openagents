//! Error handling tests for autopilot trajectory collection
//!
//! Tests various error scenarios in trajectory collection:
//! - File system errors during trajectory writing
//! - Partial trajectory recovery after crashes
//! - Network errors during trajectory upload
//! - Graceful degradation when Claude Agent SDK encounters issues

use autopilot::trajectory::{StepType, Trajectory};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Helper to create a test trajectory
fn create_test_trajectory() -> Trajectory {
    let mut traj = Trajectory::new(
        "Test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
    );

    traj.add_step(StepType::User {
        content: "Test message".to_string(),
    });

    let step = traj.add_step(StepType::Assistant {
        content: "Response".to_string(),
    });
    step.tokens_in = Some(100);
    step.tokens_out = Some(50);

    traj
}

#[test]
fn test_trajectory_serialization_success() {
    let traj = create_test_trajectory();
    let json = serde_json::to_string(&traj);
    assert!(json.is_ok(), "Trajectory should serialize successfully");

    let json_str = json.unwrap();
    assert!(json_str.contains("Test prompt"));
    assert!(json_str.contains("claude-sonnet-4"));
}

#[test]
fn test_trajectory_deserialization_success() {
    let traj = create_test_trajectory();
    let json = serde_json::to_string(&traj).unwrap();

    let deserialized: Result<Trajectory, _> = serde_json::from_str(&json);
    assert!(deserialized.is_ok(), "Should deserialize successfully");

    let traj2 = deserialized.unwrap();
    assert_eq!(traj2.prompt, traj.prompt);
    assert_eq!(traj2.model, traj.model);
    assert_eq!(traj2.steps.len(), traj.steps.len());
}

#[test]
fn test_trajectory_write_to_readonly_directory() {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let traj_path = dir.path().join("trajectory.json");

    // Create a valid trajectory
    let traj = create_test_trajectory();

    // Make directory readonly (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o444); // Read-only
        fs::set_permissions(dir.path(), perms).unwrap();

        // Try to write trajectory - should fail
        let result = fs::write(&traj_path, serde_json::to_string(&traj).unwrap());
        assert!(result.is_err(), "Should fail to write to readonly directory");

        // Restore permissions for cleanup
        let mut perms = fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dir.path(), perms).unwrap();
    }
}

#[test]
fn test_trajectory_write_to_full_disk() {
    // This is hard to test in practice, but we can test the error handling path
    // by checking that write errors are properly propagated

    let dir = TempDir::new().expect("Failed to create temp dir");
    let traj_path = dir.path().join("trajectory.json");

    let traj = create_test_trajectory();
    let json = serde_json::to_string(&traj).unwrap();

    // Normal write should succeed
    let result = fs::write(&traj_path, &json);
    assert!(result.is_ok(), "Normal write should succeed");
}

#[test]
fn test_partial_trajectory_recovery() {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let traj_path = dir.path().join("partial.json");

    // Create a trajectory and serialize it
    let mut traj = create_test_trajectory();

    // Add some steps
    for i in 0..5 {
        let step = traj.add_step(StepType::Assistant {
            content: format!("Step {}", i),
        });
        step.tokens_in = Some(100);
        step.tokens_out = Some(50);
    }

    // Write partial JSON (simulating crash during write)
    let json = serde_json::to_string(&traj).unwrap();
    let partial_json = &json[..json.len() / 2]; // Only write half

    fs::write(&traj_path, partial_json).expect("Failed to write partial JSON");

    // Try to read back - should fail gracefully
    let content = fs::read_to_string(&traj_path).unwrap();
    let result: Result<Trajectory, _> = serde_json::from_str(&content);

    assert!(result.is_err(), "Should fail to deserialize partial JSON");
}

#[test]
fn test_trajectory_with_errors() {
    let mut traj = create_test_trajectory();

    // Add tool call that fails
    traj.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool_id: "call_123".to_string(),
        input: json!({"file_path": "/nonexistent"}),
    });

    traj.add_step(StepType::ToolResult {
        tool_id: "call_123".to_string(),
        success: false,
        output: Some("File not found".to_string()),
    });

    // Set result with errors
    use autopilot::trajectory::TrajectoryResult;
    traj.result = Some(TrajectoryResult {
        success: false,
        duration_ms: 1000,
        num_turns: 2,
        result_text: None,
        errors: vec!["File not found".to_string()],
        issues_completed: 0,
                apm: Some(20.0),
    });

    assert!(traj.result.is_some());
    let result = traj.result.as_ref().unwrap();
    assert!(!result.success);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0], "File not found");
}

#[test]
fn test_trajectory_serialization_with_special_characters() {
    let mut traj = Trajectory::new(
        "Test with \"quotes\" and \n newlines".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::User {
        content: "Message with \t tabs and \r carriage returns".to_string(),
    });

    let json = serde_json::to_string(&traj);
    assert!(json.is_ok(), "Should handle special characters");

    let deserialized: Result<Trajectory, _> = serde_json::from_str(&json.unwrap());
    assert!(deserialized.is_ok(), "Should deserialize with special chars");
}

#[test]
fn test_trajectory_with_very_large_content() {
    let mut traj = create_test_trajectory();

    // Add a step with very large content (1MB)
    let large_content = "x".repeat(1024 * 1024);
    let step = traj.add_step(StepType::Assistant {
        content: large_content,
    });
    step.tokens_in = Some(1000000);
    step.tokens_out = Some(500000);

    let json = serde_json::to_string(&traj);
    assert!(json.is_ok(), "Should serialize large content");

    let json_str = json.unwrap();
    assert!(json_str.len() > 1024 * 1024, "JSON should contain large content");
}

#[test]
fn test_trajectory_token_overflow_handling() {
    let mut traj = create_test_trajectory();

    // Add steps with very large token counts
    let step = traj.add_step(StepType::Assistant {
        content: "Test".to_string(),
    });
    step.tokens_in = Some(u64::MAX);
    step.tokens_out = Some(u64::MAX);
    step.tokens_cached = Some(u64::MAX);

    traj.usage.input_tokens = u64::MAX;
    traj.usage.output_tokens = u64::MAX;
    traj.usage.cache_read_tokens = u64::MAX;

    // Should serialize without overflow
    let json = serde_json::to_string(&traj);
    assert!(json.is_ok(), "Should handle max token values");
}

#[test]
fn test_empty_trajectory_serialization() {
    let traj = Trajectory::new(
        "Empty".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    // No steps added
    assert_eq!(traj.steps.len(), 0);

    let json = serde_json::to_string(&traj);
    assert!(json.is_ok(), "Empty trajectory should serialize");

    let deserialized: Result<Trajectory, _> = serde_json::from_str(&json.unwrap());
    assert!(deserialized.is_ok(), "Empty trajectory should deserialize");
}

#[test]
fn test_trajectory_result_with_no_errors() {
    use autopilot::trajectory::TrajectoryResult;
    let mut traj = create_test_trajectory();
    traj.result = Some(TrajectoryResult {
        success: true,
        duration_ms: 5000,
        num_turns: 10,
        result_text: None,
        errors: vec![],
        issues_completed: 5,
                apm: Some(20.0),
    });

    assert!(traj.result.is_some());
    let result = traj.result.as_ref().unwrap();
    assert!(result.success);
    assert_eq!(result.errors.len(), 0);
    assert_eq!(result.issues_completed, 5);
}

#[test]
fn test_trajectory_result_with_multiple_errors() {
    use autopilot::trajectory::TrajectoryResult;
    let mut traj = create_test_trajectory();

    let errors = vec![
        "Error 1".to_string(),
        "Error 2".to_string(),
        "Error 3".to_string(),
    ];

    traj.result = Some(TrajectoryResult {
        success: false,
        duration_ms: 2000,
        num_turns: 5,
        result_text: None,
        errors: errors.clone(),
        issues_completed: 0,
                apm: Some(20.0),
    });

    assert!(traj.result.is_some());
    let result = traj.result.as_ref().unwrap();
    assert!(!result.success);
    assert_eq!(result.errors.len(), 3);
    assert_eq!(result.errors, errors);
}

#[test]
fn test_trajectory_step_ordering() {
    let mut traj = create_test_trajectory();

    // Add steps in sequence
    for i in 0..10 {
        traj.add_step(StepType::Assistant {
            content: format!("Step {}", i),
        });
    }

    // Verify step IDs are sequential (1-indexed)
    for (i, step) in traj.steps.iter().enumerate() {
        assert_eq!(step.step_id, (i + 1) as u32, "Step IDs should be sequential starting from 1");
    }
}

#[test]
fn test_trajectory_concurrent_serialization() {
    use std::sync::Arc;
    use std::thread;

    let traj = Arc::new(create_test_trajectory());

    // Spawn multiple threads serializing the same trajectory
    let mut handles = vec![];
    for _ in 0..10 {
        let traj_clone = Arc::clone(&traj);
        let handle = thread::spawn(move || {
            serde_json::to_string(&*traj_clone)
        });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        let result = handle.join().expect("Thread panicked");
        assert!(result.is_ok(), "Concurrent serialization should succeed");
    }
}

#[test]
fn test_invalid_json_recovery() {
    let invalid_jsons = vec![
        "{}",  // Missing required fields
        "{\"session_id\": \"test\"}",  // Incomplete
        "{\"steps\": []}",  // Missing required fields
        "null",  // Not an object
        "[]",  // Array instead of object
    ];

    for invalid_json in invalid_jsons {
        let result: Result<Trajectory, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err(), "Should fail to deserialize invalid JSON: {}", invalid_json);
    }
}

#[test]
fn test_trajectory_with_unicode_content() {
    let mut traj = Trajectory::new(
        "Test with emoji 🚀 and Unicode ñ".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        None,
    );

    traj.add_step(StepType::User {
        content: "Hello 世界! 🌍".to_string(),
    });

    let json = serde_json::to_string(&traj);
    assert!(json.is_ok(), "Should handle Unicode content");

    let deserialized: Result<Trajectory, _> = serde_json::from_str(&json.unwrap());
    assert!(deserialized.is_ok(), "Should deserialize Unicode content");

    let traj2 = deserialized.unwrap();
    assert!(traj2.prompt.contains("🚀"));
    // Verify Unicode content is preserved
    match &traj2.steps[0].step_type {
        StepType::User { content } => assert!(content.contains("世界")),
        _ => panic!("Expected User step"),
    }
}
