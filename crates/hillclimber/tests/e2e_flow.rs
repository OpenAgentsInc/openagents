//! E2E Flow Tests
//!
//! These tests verify the complete HillClimber + TestGen pipeline:
//! 1. Load task from description
//! 2. Generate tests using TestGen
//! 3. Run HillClimber optimization
//! 4. Verify solution via Docker pytest
//!
//! These tests require:
//! - Docker available and running
//! - FM-Bridge server (or mock)
//!
//! Run with: cargo test -p hillclimber --test e2e_flow -- --ignored

use hillclimber::{
    decomposer::decompose_task,
    evaluator::{is_docker_available, parse_pytest_output, run_docker_verification},
    scoring::score_result,
    store::HillClimberStore,
    testgen_writer::format_as_pytest,
    types::{TerminalBenchTask, VerificationConfig},
};
use tempfile::TempDir;
use testgen::{GeneratedTest, TestCategory, TestGenStore};

// ============================================================================
// Test Utilities
// ============================================================================

fn create_simple_task() -> TerminalBenchTask {
    TerminalBenchTask {
        id: "e2e-test-task".to_string(),
        description: r#"
            Write a function that adds two numbers.
            Save the result to /app/solution.py
            The function should be called 'add' and take two arguments a and b.
        "#
        .to_string(),
        source_path: None,
        verification: VerificationConfig::default(),
    }
}

fn create_sample_tests() -> Vec<GeneratedTest> {
    vec![
        GeneratedTest {
            id: "test_add_positive".to_string(),
            input: "add(2, 3)".to_string(),
            expected_output: Some("5".to_string()),
            reasoning: "Basic positive number addition".to_string(),
            category: TestCategory::Correctness,
            confidence: 0.9,
        },
        GeneratedTest {
            id: "test_add_zero".to_string(),
            input: "add(0, 0)".to_string(),
            expected_output: Some("0".to_string()),
            reasoning: "Adding zeros".to_string(),
            category: TestCategory::Boundary,
            confidence: 0.9,
        },
        GeneratedTest {
            id: "test_add_negative".to_string(),
            input: "add(-1, 1)".to_string(),
            expected_output: Some("0".to_string()),
            reasoning: "Adding negative numbers".to_string(),
            category: TestCategory::Boundary,
            confidence: 0.8,
        },
    ]
}

// ============================================================================
// E2E Flow Tests
// ============================================================================

#[test]
fn test_e2e_task_decomposition() {
    // Step 1: Decompose a task
    let task = create_simple_task();
    let decomposition = decompose_task(&task);

    assert_eq!(decomposition.subtask_count, 4, "Should have 4 subtasks");
    assert!(decomposition.subtasks.len() > 0, "Should produce subtasks");

    // Verify subtask phases exist
    let subtask_names: Vec<&str> = decomposition.subtasks.iter().map(|s| s.name.as_str()).collect();
    assert!(
        subtask_names.iter().any(|n| n.contains("understand")),
        "Should have understand phase"
    );
    assert!(
        subtask_names.iter().any(|n| n.contains("write") || n.contains("implement")),
        "Should have write phase"
    );
}

#[test]
fn test_e2e_testgen_to_pytest() {
    // Step 2: Convert generated tests to pytest format
    let task = create_simple_task();
    let tests = create_sample_tests();

    // format_as_pytest takes (tests, task_id, task_description)
    let pytest_code = format_as_pytest(&tests, &task.id, Some(&task.description));

    // Verify pytest structure
    assert!(pytest_code.contains("def test_"), "Should contain test functions");
    assert!(pytest_code.contains("import"), "Should have imports");
    assert!(
        pytest_code.contains("test_add_positive") || pytest_code.contains("add_positive"),
        "Should include test names"
    );
}

#[test]
fn test_e2e_pytest_output_parsing() {
    // Step 3: Parse pytest output
    let passing_output = r#"
============================= test session starts ==============================
collected 3 items

test_solution.py::test_add_positive PASSED                               [ 33%]
test_solution.py::test_add_zero PASSED                                   [ 66%]
test_solution.py::test_add_negative PASSED                               [100%]

============================== 3 passed in 0.01s ===============================
"#;

    let result = parse_pytest_output(passing_output);
    // ParseResult has: total, passed, failed, failures
    // passed is a count (u32), not a bool
    assert_eq!(result.failed, 0, "Should detect all tests passing");
    assert_eq!(result.passed, 3);
    assert_eq!(result.total, 3);
    assert!(result.failures.is_empty(), "Should have no failures");
}

#[test]
fn test_e2e_pytest_output_parsing_failures() {
    let failing_output = r#"
============================= test session starts ==============================
collected 3 items

test_solution.py::test_add_positive PASSED                               [ 33%]
test_solution.py::test_add_zero FAILED                                   [ 66%]
test_solution.py::test_add_negative PASSED                               [100%]

=================================== FAILURES ===================================
________________________________ test_add_zero _________________________________

    def test_add_zero():
>       assert add(0, 0) == 1
E       AssertionError: assert 0 == 1

test_solution.py:15: AssertionError
=========================== short test summary info ============================
FAILED test_solution.py::test_add_zero - AssertionError: assert 0 == 1
============================== 1 failed, 2 passed in 0.02s =====================
"#;

    let result = parse_pytest_output(failing_output);
    // ParseResult.failed > 0 means failure
    assert!(result.failed > 0, "Should detect failure");
    assert_eq!(result.passed, 2);
    assert_eq!(result.total, 3);
    assert_eq!(result.failures.len(), 1, "Should have one failure");
    assert!(
        result.failures[0].test_name.contains("test_add_zero"),
        "Should identify failing test"
    );
}

#[test]
fn test_e2e_scoring_integration() {
    // Step 4: Score calculation
    let score_pass = score_result(true, 5);
    let score_fail = score_result(false, 5);

    assert!(score_pass > score_fail, "Passing should score higher");
    assert!(score_pass >= 1000, "Passing gives bonus");
    assert!(score_fail < 1000, "Failing doesn't get bonus");

    // Earlier turns should score higher
    let score_early = score_result(true, 3);
    let score_late = score_result(true, 10);
    assert!(score_early > score_late, "Earlier completion should score higher");
}

#[test]
fn test_e2e_store_integration() {
    // Step 5: Store persistence
    let store = HillClimberStore::open_in_memory().unwrap();

    // Create and save config
    let config = store.ensure_default_config("e2e-test").unwrap();
    assert!(config.id > 0, "Config should have ID");

    // Verify retrieval
    let retrieved = store.get_current_config("e2e-test").unwrap();
    assert!(retrieved.is_some(), "Should retrieve config");
    assert_eq!(retrieved.unwrap().id, config.id);
}

// ============================================================================
// Docker Integration Tests (require Docker)
// ============================================================================

#[tokio::test]
#[ignore] // Requires Docker
async fn test_e2e_docker_available() {
    // is_docker_available() is async
    let available = is_docker_available().await;
    assert!(available, "Docker should be available for E2E tests");
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_e2e_docker_verification_passing() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path();

    // Create tests subdirectory (Docker verification expects tests/ dir)
    std::fs::create_dir(workspace.join("tests")).unwrap();

    // Create a simple Python solution
    let solution = r#"
def add(a, b):
    return a + b
"#;
    std::fs::write(workspace.join("solution.py"), solution).unwrap();

    // Create test file in tests/ directory
    let test_code = r#"
import sys
sys.path.insert(0, '..')
from solution import add

def test_add_positive():
    assert add(2, 3) == 5

def test_add_zero():
    assert add(0, 0) == 0

def test_add_negative():
    assert add(-1, 1) == 0
"#;
    std::fs::write(workspace.join("tests/test_solution.py"), test_code).unwrap();

    // Create task for verification
    let task = create_simple_task();

    // run_docker_verification takes (task, workspace, timeout_secs)
    let result = run_docker_verification(&task, workspace, 60).await;

    assert!(result.is_ok(), "Docker verification should succeed: {:?}", result.err());
    let eval_result = result.unwrap();
    assert!(eval_result.passed, "All tests should pass");
    assert_eq!(eval_result.tests_passing, 3);
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_e2e_docker_verification_failing() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path();

    // Create tests subdirectory
    std::fs::create_dir(workspace.join("tests")).unwrap();

    // Create a buggy solution
    let solution = r#"
def add(a, b):
    return a - b  # BUG: subtraction instead of addition
"#;
    std::fs::write(workspace.join("solution.py"), solution).unwrap();

    // Create test file
    let test_code = r#"
import sys
sys.path.insert(0, '..')
from solution import add

def test_add_positive():
    assert add(2, 3) == 5

def test_add_zero():
    assert add(0, 0) == 0  # This passes even with the bug!

def test_add_negative():
    assert add(-1, 1) == 0
"#;
    std::fs::write(workspace.join("tests/test_solution.py"), test_code).unwrap();

    let task = create_simple_task();
    let result = run_docker_verification(&task, workspace, 60).await;

    assert!(result.is_ok(), "Docker should run");
    let eval_result = result.unwrap();
    assert!(!eval_result.passed, "Tests should fail");
    assert!(eval_result.tests_passing < eval_result.tests_total, "Some tests should fail");
}

// ============================================================================
// Full Pipeline Test (requires Docker + patience)
// ============================================================================

#[tokio::test]
#[ignore] // Requires Docker, takes time
async fn test_e2e_full_pipeline() {
    // This is the complete E2E test:
    // 1. Create task
    // 2. Generate tests (using sample tests for now)
    // 3. Create solution workspace
    // 4. Run Docker verification
    // 5. Score result
    // 6. Save to store

    let task = create_simple_task();
    let tests = create_sample_tests();

    // Generate pytest code
    let pytest_code = format_as_pytest(&tests, &task.id, Some(&task.description));

    // Create workspace
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path();

    // Create tests directory
    std::fs::create_dir(workspace.join("tests")).unwrap();

    // Create solution
    let solution = r#"
def add(a, b):
    return a + b
"#;
    std::fs::write(workspace.join("solution.py"), solution).unwrap();
    std::fs::write(workspace.join("tests/test_solution.py"), &pytest_code).unwrap();

    // Run verification
    let result = run_docker_verification(&task, workspace, 120).await;

    // Score and store
    let eval_result = result.expect("Docker verification should complete");
    let score = score_result(eval_result.passed, 1); // Assume 1 turn for test

    // Save to store
    let store = HillClimberStore::open_in_memory().unwrap();
    let config = store.ensure_default_config(&task.id).unwrap();

    let run_input = hillclimber::types::HillClimberRunInput {
        run_id: format!("e2e-test-{}", chrono::Utc::now().timestamp()),
        task_id: task.id.clone(),
        config_id: config.id,
        passed: eval_result.passed,
        turns: 1,
        duration_ms: eval_result.duration_ms,
        step_summary: None,
        error_message: None,
        meta_model: None,
        proposed_change: None,
        change_accepted: false,
        score,
    };

    let saved_run = store.save_run(&run_input).unwrap();

    println!("E2E Pipeline Complete:");
    println!("  Task: {}", task.id);
    println!("  Tests: {} generated", tests.len());
    println!("  Result: {} ({}/{})",
        if eval_result.passed { "PASS" } else { "FAIL" },
        eval_result.tests_passing,
        eval_result.tests_total
    );
    println!("  Score: {}", score);
    println!("  Run ID: {}", saved_run.run_id);

    assert!(eval_result.passed, "E2E test should pass");
}

// ============================================================================
// TestGen Store Integration
// ============================================================================

#[test]
fn test_e2e_testgen_store_integration() {
    let store = TestGenStore::open_in_memory().unwrap();

    // Save config
    let config = store
        .save_config(&testgen::types::TestGenConfigInput::default())
        .unwrap();
    assert!(config.id > 0);

    // Save run
    let run_input = testgen::types::TestGenRunInput {
        run_id: "tg-e2e-test".to_string(),
        session_id: "session-e2e".to_string(),
        config_id: config.id,
        task_id: "e2e-test-task".to_string(),
        total_tests: 10,
        comprehensiveness_score: Some(8.5),
        duration_ms: 5000,
        total_tokens: 10000,
        category_balance: Some(0.8),
        anti_cheat_coverage: Some(0.9),
        parameter_discovery: Some(0.7),
        reflection_effectiveness: Some(0.6),
        token_efficiency: Some(0.5),
        meta_model: None,
        proposed_change: None,
        change_accepted: false,
        score: 800,
    };

    let saved_run = store.save_run(&run_input).unwrap();
    assert!(saved_run.id > 0);

    // Verify stats
    let stats = store.get_stats().unwrap();
    assert_eq!(stats.total_runs, 1);
    assert_eq!(stats.best_score, 800);
}
