use anyhow::Result;
use openagents::solver::state::{SolverState, SolverStatus};
use serde_json::json;

#[test]
#[ignore = "requires solver setup"]
fn test_state_serialization() -> Result<()> {
    let mut state = SolverState::new("Testing state serialization".to_string());
    
    let file = state.add_file(
        "src/main.rs".to_string(),
        "Main entry point".to_string(),
        0.9,
    );
    
    file.add_change(
        "fn old_code()".to_string(),
        "fn new_code()".to_string(),
        "Improving function name".to_string(),
    );

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&state)?;
    println!("Serialized state:\n{}", json);

    // Deserialize back
    let deserialized: SolverState = serde_json::from_str(&json)?;
    
    assert_eq!(state.id, deserialized.id);
    assert_eq!(state.run_id, deserialized.run_id);
    assert_eq!(state.status, deserialized.status);
    assert_eq!(state.analysis, deserialized.analysis);
    assert_eq!(state.files.len(), deserialized.files.len());
    
    let original_file = &state.files[0];
    let deserialized_file = &deserialized.files[0];
    
    assert_eq!(original_file.id, deserialized_file.id);
    assert_eq!(original_file.path, deserialized_file.path);
    assert_eq!(original_file.analysis, deserialized_file.analysis);
    assert_eq!(original_file.relevance_score, deserialized_file.relevance_score);
    
    Ok(())
}

#[test]
#[ignore = "requires solver setup"]
fn test_state_transitions() -> Result<()> {
    let mut state = SolverState::new("Initial state".to_string());
    
    // Test initial state
    assert_eq!(state.status, SolverStatus::CollectingContext);
    
    // Test state transitions
    let transitions = vec![
        SolverStatus::Thinking,
        SolverStatus::GeneratingCode,
        SolverStatus::ReadyForCoding,
        SolverStatus::Testing,
        SolverStatus::CreatingPr,
        SolverStatus::Complete,
    ];
    
    for status in transitions {
        state.update_status(status.clone());
        assert_eq!(state.status, status);
    }
    
    Ok(())
}

#[test]
#[ignore = "requires solver setup"]
fn test_file_management() -> Result<()> {
    let mut state = SolverState::new("Testing file management".to_string());
    
    // Add multiple files
    let files = vec![
        ("src/main.rs", "Main entry point", 0.9),
        ("src/lib.rs", "Core library code", 0.8),
        ("tests/main.rs", "Main tests", 0.7),
    ];
    
    for (path, analysis, score) in files {
        state.add_file(
            path.to_string(),
            analysis.to_string(),
            score,
        );
    }
    
    assert_eq!(state.files.len(), 3);
    assert_eq!(state.files[0].path, "src/main.rs");
    assert_eq!(state.files[1].path, "src/lib.rs");
    assert_eq!(state.files[2].path, "tests/main.rs");
    
    // Add changes to files
    let main_file = &mut state.files[0];
    main_file.add_change(
        "old_main".to_string(),
        "new_main".to_string(),
        "Updated main function".to_string(),
    );
    
    assert_eq!(main_file.changes.len(), 1);
    assert_eq!(main_file.changes[0].search, "old_main");
    assert_eq!(main_file.changes[0].replace, "new_main");
    
    Ok(())
}

#[test]
#[ignore = "requires solver setup"]
fn test_json_schema_compatibility() -> Result<()> {
    // Create a state object matching the schema from the issue
    let json_data = json!({
        "id": "test-state-1",
        "run_id": "test-run-1",
        "status": "collecting_context",
        "analysis": "Initial analysis of the issue",
        "files": [{
            "id": "file-1",
            "path": "src/llm_utils.rs",
            "analysis": "Contains LLM utility functions",
            "relevance_score": 0.9,
            "changes": [{
                "search": "old code",
                "replace": "new code",
                "analysis": "Improving performance"
            }]
        }]
    });

    // Should deserialize without errors
    let state: SolverState = serde_json::from_value(json_data)?;
    
    assert_eq!(state.id, "test-state-1");
    assert_eq!(state.run_id, "test-run-1");
    assert_eq!(state.status, SolverStatus::CollectingContext);
    assert_eq!(state.files.len(), 1);
    
    let file = &state.files[0];
    assert_eq!(file.path, "src/llm_utils.rs");
    assert_eq!(file.changes.len(), 1);
    
    Ok(())
}