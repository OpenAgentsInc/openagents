//! HillClimber Integration Tests
//!
//! Tests for cross-module interactions:
//! - TestGen → HillClimber pytest generation
//! - Evaluator pytest parsing edge cases
//! - Store ↔ Orchestrator interaction
//!
//! Run with: cargo test -p hillclimber --test integration

use hillclimber::{
    decomposer::decompose_task,
    evaluator::parse_pytest_output,
    scoring::score_result,
    store::HillClimberStore,
    testgen_writer::format_as_pytest,
    types::{HillClimberConfigInput, HillClimberRunInput, TerminalBenchTask, VerificationConfig},
};
use testgen::{GeneratedTest, TestCategory, TestGenStore};

// ============================================================================
// TestGen → HillClimber Integration
// ============================================================================

#[test]
fn test_testgen_to_hillclimber_pytest_generation() {
    // Create a task
    let task = TerminalBenchTask {
        id: "integration-test".to_string(),
        description: "Write a regex pattern to match dates in YYYY-MM-DD format. Save your regex in /app/regex.txt".to_string(),
        source_path: None,
        verification: VerificationConfig::default(),
    };

    // Generate tests (simulating TestGen output)
    let tests = vec![
        GeneratedTest {
            id: "test_basic_date".to_string(),
            input: "2023-01-15".to_string(),
            expected_output: Some("[\"2023-01-15\"]".to_string()),
            reasoning: "Basic date matching".to_string(),
            category: TestCategory::Correctness,
            confidence: 0.9,
        },
        GeneratedTest {
            id: "test_no_match".to_string(),
            input: "not a date".to_string(),
            expected_output: Some("[]".to_string()),
            reasoning: "Should not match non-date text".to_string(),
            category: TestCategory::Boundary,
            confidence: 0.85,
        },
        GeneratedTest {
            id: "test_multiple_dates".to_string(),
            input: "Dates: 2023-01-01 and 2023-12-31".to_string(),
            expected_output: Some("[\"2023-01-01\", \"2023-12-31\"]".to_string()),
            reasoning: "Should match multiple dates".to_string(),
            category: TestCategory::Integration,
            confidence: 0.8,
        },
    ];

    // Convert to pytest format
    let pytest_code = format_as_pytest(&tests, &task.id, Some(&task.description));

    // Verify pytest structure
    assert!(pytest_code.contains("import pytest"), "Should have pytest import");
    assert!(pytest_code.contains("import re"), "Should have re import for regex tasks");
    assert!(pytest_code.contains("def test_"), "Should have test functions");

    // Verify all tests are included
    assert!(
        pytest_code.contains("basic_date") || pytest_code.contains("test_basic"),
        "Should include basic date test"
    );

    // Verify proper categorization
    assert!(pytest_code.contains("Correctness Tests"), "Should have correctness section");
    assert!(pytest_code.contains("Boundary Tests"), "Should have boundary section");
    assert!(pytest_code.contains("Integration Tests"), "Should have integration section");
}

#[test]
fn test_testgen_all_categories_generate_pytest() {
    let task = TerminalBenchTask {
        id: "all-categories".to_string(),
        description: "Test task for all categories. Output to /app/result.txt".to_string(),
        source_path: None,
        verification: VerificationConfig::default(),
    };

    // Create test for each category
    let categories = vec![
        TestCategory::AntiCheat,
        TestCategory::Existence,
        TestCategory::Correctness,
        TestCategory::Boundary,
        TestCategory::Integration,
    ];

    let tests: Vec<GeneratedTest> = categories
        .iter()
        .enumerate()
        .map(|(i, cat)| GeneratedTest {
            id: format!("test_{}", i),
            input: "test input".to_string(),
            expected_output: Some("expected".to_string()),
            reasoning: format!("Test for {:?}", cat),
            category: cat.clone(),
            confidence: 0.9,
        })
        .collect();

    let pytest_code = format_as_pytest(&tests, &task.id, Some(&task.description));

    // All categories should have sections
    assert!(pytest_code.contains("Anti-Cheat Tests"));
    assert!(pytest_code.contains("Existence Tests"));
    assert!(pytest_code.contains("Correctness Tests"));
    assert!(pytest_code.contains("Boundary Tests"));
    assert!(pytest_code.contains("Integration Tests"));
}

// ============================================================================
// Evaluator Pytest Parsing Integration
// ============================================================================

#[test]
fn test_evaluator_parses_various_pytest_outputs() {
    // Test all passing
    let all_pass = r#"
============================= test session starts ==============================
platform linux -- Python 3.11.0
collected 5 items

tests/test_solution.py .....                                             [100%]

============================== 5 passed in 0.12s ===============================
"#;

    let result = parse_pytest_output(all_pass);
    assert_eq!(result.total, 5);
    assert_eq!(result.passed, 5);
    assert_eq!(result.failed, 0);

    // Test all failing
    let all_fail = r#"
============================= test session starts ==============================
collected 3 items

tests/test_solution.py FFF                                               [100%]

=========================== short test summary info ============================
FAILED tests/test_solution.py::test_one
FAILED tests/test_solution.py::test_two
FAILED tests/test_solution.py::test_three
============================== 3 failed in 0.05s ===============================
"#;

    let result = parse_pytest_output(all_fail);
    assert_eq!(result.total, 3);
    assert_eq!(result.passed, 0);
    assert_eq!(result.failed, 3);
    assert_eq!(result.failures.len(), 3);

    // Test mixed results
    let mixed = r#"
============================= test session starts ==============================
collected 10 items

tests/test_solution.py ...F.F..FF                                        [100%]

=========================== short test summary info ============================
FAILED tests/test_solution.py::test_four
FAILED tests/test_solution.py::test_six
FAILED tests/test_solution.py::test_nine
FAILED tests/test_solution.py::test_ten
============================== 4 failed, 6 passed in 0.15s =====================
"#;

    let result = parse_pytest_output(mixed);
    assert_eq!(result.total, 10);
    assert_eq!(result.passed, 6);
    assert_eq!(result.failed, 4);
    assert_eq!(result.failures.len(), 4);
}

#[test]
fn test_evaluator_parses_verbose_failure_output() {
    let verbose_output = r#"
============================= test session starts ==============================
collected 2 items

tests/test_solution.py::test_basic PASSED
tests/test_solution.py::test_edge FAILED - AssertionError: Expected ['a', 'b'], but got ['a']

=================================== FAILURES ===================================
_________________________________ test_edge ____________________________________

    def test_edge():
        result = my_func("test")
>       assert result == ['a', 'b']
E       AssertionError: assert ['a'] == ['a', 'b']
E         Right contains one more item: 'b'

tests/test_solution.py:15: AssertionError
=========================== short test summary info ============================
FAILED tests/test_solution.py::test_edge - AssertionError
============================== 1 failed, 1 passed in 0.03s =====================
"#;

    let result = parse_pytest_output(verbose_output);
    assert_eq!(result.passed, 1);
    assert_eq!(result.failed, 1);
    assert_eq!(result.failures.len(), 1);

    let failure = &result.failures[0];
    assert!(failure.test_name.contains("test_edge"));
}

#[test]
fn test_evaluator_parses_our_summary_format() {
    // Test parsing of our own summary format (from format_for_prompt)
    let our_format = r#"
Verification: FAILED (8/12 tests)
  - test_basic: expected ['x'], got []
  - test_edge: pattern didn't match
  ... and 2 more failures
"#;

    let result = parse_pytest_output(our_format);
    assert_eq!(result.passed, 8);
    assert_eq!(result.total, 12);
    assert_eq!(result.failed, 4);
}

#[test]
fn test_evaluator_handles_empty_output() {
    let empty = "";
    let result = parse_pytest_output(empty);
    assert_eq!(result.total, 0);
    assert_eq!(result.passed, 0);
    assert_eq!(result.failed, 0);
}

#[test]
fn test_evaluator_handles_error_output() {
    let error_output = r#"
ERROR collecting tests/test_solution.py
ModuleNotFoundError: No module named 'solution'
"#;

    let result = parse_pytest_output(error_output);
    // Should not crash, returns zeros
    assert_eq!(result.total, 0);
}

// ============================================================================
// Store ↔ Components Integration
// ============================================================================

#[test]
fn test_store_tracks_improvement_over_runs() {
    let store = HillClimberStore::open_in_memory().unwrap();

    let config = store.ensure_default_config("improvement-test").unwrap();

    // Simulate runs that improve over time
    let run_results = vec![
        (false, 20, 400),  // First attempt fails
        (false, 18, 450),  // Slight improvement
        (false, 15, 520),  // Getting closer
        (true, 12, 900),   // First success!
        (true, 10, 950),   // Faster success
        (true, 8, 1000),   // Even faster
    ];

    let mut best_score = 0;
    for (i, (passed, turns, score)) in run_results.iter().enumerate() {
        let run_input = HillClimberRunInput {
            run_id: format!("improvement-run-{}", i),
            task_id: "improvement-test".to_string(),
            config_id: config.id,
            passed: *passed,
            turns: *turns,
            duration_ms: 10000,
            step_summary: None,
            error_message: None,
            meta_model: None,
            proposed_change: None,
            change_accepted: false,
            score: *score,
        };

        store.save_run(&run_input).unwrap();

        if *score > best_score {
            best_score = *score;
        }
    }

    // Verify stats reflect improvement
    let stats = store.get_stats().unwrap();
    let task_stats = stats.by_task.get("improvement-test").unwrap();

    assert_eq!(task_stats.total_runs, 6);
    assert_eq!(task_stats.pass_count, 3);
    assert!((task_stats.pass_rate - 0.5).abs() < 0.01, "Pass rate should be 50%");
    assert_eq!(task_stats.best_score, 1000);
}

#[test]
fn test_decomposer_output_feeds_orchestration() {
    // Verify decomposer output is suitable for orchestration
    let task = TerminalBenchTask {
        id: "decomp-test".to_string(),
        description: "Write a script that processes CSV files and outputs JSON".to_string(),
        source_path: None,
        verification: VerificationConfig::default(),
    };

    let decomposition = decompose_task(&task);

    // Verify we have the expected phases
    assert_eq!(decomposition.subtask_count, 4);
    assert_eq!(decomposition.subtasks.len(), 4);

    // Each subtask should have valid data
    for subtask in &decomposition.subtasks {
        assert!(!subtask.name.is_empty(), "Subtask should have name");
        assert!(!subtask.goal.is_empty(), "Subtask should have goal");
        assert!(!subtask.checkpoint.is_empty(), "Subtask should have checkpoint");
        assert!(subtask.max_turns > 0, "Subtask should have max turns");
    }

    // Dependencies should form a valid DAG
    for subtask in &decomposition.subtasks {
        for dep in &subtask.depends_on {
            assert!(
                *dep < subtask.id,
                "Dependency {} should come before subtask {}",
                dep,
                subtask.id
            );
        }
    }
}

// ============================================================================
// TestGen Store ↔ HillClimber Store Integration
// ============================================================================

#[test]
fn test_testgen_and_hillclimber_stores_independent() {
    // Both stores should work independently
    let hc_store = HillClimberStore::open_in_memory().unwrap();
    let tg_store = TestGenStore::open_in_memory().unwrap();

    // Save to HillClimber store
    let hc_config = hc_store.ensure_default_config("shared-task").unwrap();
    let hc_run = HillClimberRunInput {
        run_id: "hc-run-1".to_string(),
        task_id: "shared-task".to_string(),
        config_id: hc_config.id,
        passed: true,
        turns: 5,
        duration_ms: 10000,
        step_summary: None,
        error_message: None,
        meta_model: None,
        proposed_change: None,
        change_accepted: false,
        score: 900,
    };
    hc_store.save_run(&hc_run).unwrap();

    // Save to TestGen store
    let tg_config = tg_store.save_config(&testgen::types::TestGenConfigInput::default()).unwrap();
    let tg_run = testgen::types::TestGenRunInput {
        run_id: "tg-run-1".to_string(),
        session_id: "session-1".to_string(),
        config_id: tg_config.id,
        task_id: "shared-task".to_string(),
        total_tests: 15,
        comprehensiveness_score: Some(0.82),
        duration_ms: 5000,
        total_tokens: 8000,
        category_balance: Some(0.9),
        anti_cheat_coverage: Some(0.95),
        parameter_discovery: Some(0.7),
        reflection_effectiveness: Some(0.8),
        token_efficiency: Some(0.6),
        meta_model: None,
        proposed_change: None,
        change_accepted: false,
        score: 850,
    };
    tg_store.save_run(&tg_run).unwrap();

    // Both should have their data independently
    let hc_stats = hc_store.get_stats().unwrap();
    let tg_stats = tg_store.get_stats().unwrap();

    assert_eq!(hc_stats.total_runs, 1);
    assert_eq!(tg_stats.total_runs, 1);
    assert_eq!(hc_stats.by_task.get("shared-task").unwrap().best_score, 900);
    assert_eq!(tg_stats.best_score, 850);
}

// ============================================================================
// Scoring Integration
// ============================================================================

#[test]
fn test_scoring_integrates_with_store() {
    let store = HillClimberStore::open_in_memory().unwrap();
    let config = store.ensure_default_config("scoring-test").unwrap();

    // Create runs with calculated scores
    let scenarios = vec![
        (true, 3),   // Early success
        (true, 8),   // Mid success
        (true, 15),  // Late success
        (false, 5),  // Failure
    ];

    for (i, (passed, turns)) in scenarios.iter().enumerate() {
        let score = score_result(*passed, *turns);

        let run_input = HillClimberRunInput {
            run_id: format!("score-run-{}", i),
            task_id: "scoring-test".to_string(),
            config_id: config.id,
            passed: *passed,
            turns: *turns,
            duration_ms: 10000,
            step_summary: None,
            error_message: None,
            meta_model: None,
            proposed_change: None,
            change_accepted: false,
            score,
        };

        store.save_run(&run_input).unwrap();
    }

    // Best score should be from early success
    let stats = store.get_stats().unwrap();
    let task_stats = stats.by_task.get("scoring-test").unwrap();

    let expected_best = score_result(true, 3);
    assert_eq!(task_stats.best_score, expected_best);
}
