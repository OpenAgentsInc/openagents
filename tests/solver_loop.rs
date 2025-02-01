use anyhow::Result;
use openagents::solver::state::{SolverState, SolverStatus};

#[tokio::test]
async fn test_solver_loop_state_transitions() -> Result<()> {
    // Initialize state
    let mut state = SolverState::new("Test solver state".to_string());
    assert_eq!(state.status, SolverStatus::CollectingContext);

    // Test context collection
    state.update_status(SolverStatus::CollectingContext);
    assert_eq!(state.status, SolverStatus::CollectingContext);

    // Test file identification
    state.update_status(SolverStatus::Thinking);
    assert_eq!(state.status, SolverStatus::Thinking);

    // Add test files
    state.add_file(
        "src/test1.rs".to_string(),
        "Test file 1".to_string(),
        0.9,
    );
    state.add_file(
        "src/test2.rs".to_string(),
        "Test file 2".to_string(),
        0.8,
    );
    assert_eq!(state.files.len(), 2);

    // Test code generation
    state.update_status(SolverStatus::GeneratingCode);
    assert_eq!(state.status, SolverStatus::GeneratingCode);

    // Add test changes
    let file = &mut state.files[0];
    file.add_change(
        "old code".to_string(),
        "new code".to_string(),
        "Test change".to_string(),
    );
    assert_eq!(state.files[0].changes.len(), 1);

    // Test ready state
    state.update_status(SolverStatus::ReadyForCoding);
    assert_eq!(state.status, SolverStatus::ReadyForCoding);

    // Test testing state
    state.update_status(SolverStatus::Testing);
    assert_eq!(state.status, SolverStatus::Testing);

    // Test PR creation state
    state.update_status(SolverStatus::CreatingPr);
    assert_eq!(state.status, SolverStatus::CreatingPr);

    // Test completion
    state.update_status(SolverStatus::Complete);
    assert_eq!(state.status, SolverStatus::Complete);

    Ok(())
}

#[tokio::test]
async fn test_solver_loop_error_handling() -> Result<()> {
    let mut state = SolverState::new("Test error state".to_string());

    // Test error state
    state.update_status(SolverStatus::Error);
    assert_eq!(state.status, SolverStatus::Error);

    // Test recovery
    state.update_status(SolverStatus::CollectingContext);
    assert_eq!(state.status, SolverStatus::CollectingContext);

    Ok(())
}

#[tokio::test]
async fn test_solver_loop_file_management() -> Result<()> {
    let mut state = SolverState::new("Test file management".to_string());

    // Add files
    let file1 = state.add_file(
        "src/test1.rs".to_string(),
        "Test file 1".to_string(),
        0.9,
    );
    file1.add_change(
        "old code 1".to_string(),
        "new code 1".to_string(),
        "Change 1".to_string(),
    );

    let file2 = state.add_file(
        "src/test2.rs".to_string(),
        "Test file 2".to_string(),
        0.8,
    );
    file2.add_change(
        "old code 2".to_string(),
        "new code 2".to_string(),
        "Change 2".to_string(),
    );

    // Verify file count
    assert_eq!(state.files.len(), 2);

    // Verify file details
    assert_eq!(state.files[0].path, "src/test1.rs");
    assert_eq!(state.files[0].analysis, "Test file 1");
    assert_eq!(state.files[0].relevance_score, 0.9);
    assert_eq!(state.files[0].changes.len(), 1);

    assert_eq!(state.files[1].path, "src/test2.rs");
    assert_eq!(state.files[1].analysis, "Test file 2");
    assert_eq!(state.files[1].relevance_score, 0.8);
    assert_eq!(state.files[1].changes.len(), 1);

    // Verify changes
    assert_eq!(state.files[0].changes[0].search, "old code 1");
    assert_eq!(state.files[0].changes[0].replace, "new code 1");
    assert_eq!(state.files[0].changes[0].analysis, "Change 1");

    assert_eq!(state.files[1].changes[0].search, "old code 2");
    assert_eq!(state.files[1].changes[0].replace, "new code 2");
    assert_eq!(state.files[1].changes[0].analysis, "Change 2");

    Ok(())
}