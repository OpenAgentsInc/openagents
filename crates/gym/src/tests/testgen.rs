//! TestGen Visualizer tests
//!
//! Tests for the TestGen test generation visualization component.
//!
//! These tests simulate user interactions with the TestGen UI to validate
//! the complete user flow from task selection through test generation.

use gpui_oa::TestAppContext;
use crate::tests::fixtures::{TestGenAssertExt, TestGenFixture};
use crate::tests::fixtures::types::{GenerationStatus, TestCategory, TestGenStatus};

// ============================================================================
// TestGenVisualizer Initial State Tests
// ============================================================================

#[gpui_oa::test]
fn test_testgen_visualizer_renders(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Should start with no task selected
    view.assert_that(cx).has_no_task_selected();
}

#[gpui_oa::test]
fn test_testgen_starts_idle(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Should start in idle state
    view.assert_that(cx).generation_is_idle();
}

#[gpui_oa::test]
fn test_testgen_task_count_query(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Task count query should work (may be 0 in test environment without task files)
    let _count = TestGenFixture::task_count(&view, cx);
    // Just verify the query works - actual task availability depends on environment
    // count is usize so always >= 0
}

#[gpui_oa::test]
fn test_testgen_no_tests_initially(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Should have no generated tests initially
    view.assert_that(cx).has_no_generated_tests();

    let count = TestGenFixture::generated_test_count(&view, cx);
    assert_eq!(count, 0, "Should have no generated tests");
}

#[gpui_oa::test]
fn test_testgen_cannot_generate_without_task(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Cannot generate without selecting a task
    view.assert_that(cx).cannot_generate();

    let can = TestGenFixture::can_generate(&view, cx);
    assert!(!can, "Should not be able to generate without task selected");
}

// ============================================================================
// Task Selection Tests
// ============================================================================

#[gpui_oa::test]
fn test_testgen_select_task(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    let task_count = TestGenFixture::task_count(&view, cx);
    if task_count == 0 {
        return; // Skip test if no tasks available in test environment
    }

    // Select the first task
    TestGenFixture::select_task(&view, 0, cx);

    // Should now have a task selected
    view.assert_that(cx)
        .has_task_selected()
        .has_selected_task_idx(0);
}

#[gpui_oa::test]
fn test_testgen_select_task_enables_generate(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    let task_count = TestGenFixture::task_count(&view, cx);
    if task_count == 0 {
        return; // Skip test if no tasks available in test environment
    }

    // Initially cannot generate
    view.assert_that(cx).cannot_generate();

    // Select a task
    TestGenFixture::select_task(&view, 0, cx);

    // Now can generate
    view.assert_that(cx).can_generate();
}

#[gpui_oa::test]
fn test_testgen_select_next_task(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    let task_count = TestGenFixture::task_count(&view, cx);
    if task_count < 2 {
        return; // Need at least 2 tasks to test navigation
    }

    // Select first task
    TestGenFixture::select_task(&view, 0, cx);
    view.assert_that(cx).has_selected_task_idx(0);

    // Navigate to next
    TestGenFixture::select_next_task(&view, cx);
    view.assert_that(cx).has_selected_task_idx(1);
}

#[gpui_oa::test]
fn test_testgen_select_prev_task(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    let task_count = TestGenFixture::task_count(&view, cx);
    if task_count < 2 {
        return; // Need at least 2 tasks to test navigation
    }

    // Select second task
    TestGenFixture::select_task(&view, 1, cx);
    view.assert_that(cx).has_selected_task_idx(1);

    // Navigate to previous
    TestGenFixture::select_prev_task(&view, cx);
    view.assert_that(cx).has_selected_task_idx(0);
}

#[gpui_oa::test]
fn test_testgen_select_task_wraps_around(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    let task_count = TestGenFixture::task_count(&view, cx);
    if task_count < 2 {
        return; // Need at least 2 tasks to test wrapping
    }

    // Select last task
    TestGenFixture::select_task(&view, task_count - 1, cx);
    view.assert_that(cx).has_selected_task_idx(task_count - 1);

    // Navigate to next - should wrap to first
    TestGenFixture::select_next_task(&view, cx);
    view.assert_that(cx).has_selected_task_idx(0);
}

#[gpui_oa::test]
fn test_testgen_select_prev_task_wraps_around(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    let task_count = TestGenFixture::task_count(&view, cx);
    if task_count < 2 {
        return; // Need at least 2 tasks to test wrapping
    }

    // Select first task
    TestGenFixture::select_task(&view, 0, cx);
    view.assert_that(cx).has_selected_task_idx(0);

    // Navigate to previous - should wrap to last
    TestGenFixture::select_prev_task(&view, cx);
    view.assert_that(cx).has_selected_task_idx(task_count - 1);
}

// ============================================================================
// Generation Status Tests
// ============================================================================

#[gpui_oa::test]
fn test_testgen_generation_status_labels(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Verify status starts as Idle
    let status = TestGenFixture::generation_status(&view, cx);
    assert!(matches!(status, GenerationStatus::Idle));

    // Verify is_idle helper
    assert!(TestGenFixture::is_idle(&view, cx));
    assert!(!TestGenFixture::is_generating(&view, cx));
    assert!(!TestGenFixture::is_complete(&view, cx));
    assert!(!TestGenFixture::is_failed(&view, cx));
}

// ============================================================================
// Category Tests (Static - no view needed)
// ============================================================================

#[gpui_oa::test]
fn test_testgen_category_labels() {
    assert_eq!(TestCategory::AntiCheat.label(), "Anti-Cheat");
    assert_eq!(TestCategory::Existence.label(), "Existence");
    assert_eq!(TestCategory::Correctness.label(), "Correctness");
    assert_eq!(TestCategory::Boundary.label(), "Boundary");
    assert_eq!(TestCategory::Integration.label(), "Integration");
}

#[gpui_oa::test]
fn test_testgen_category_descriptions() {
    assert_eq!(
        TestCategory::AntiCheat.description(),
        "Prevent hardcoded solutions"
    );
    assert_eq!(
        TestCategory::Existence.description(),
        "Basic functionality exists"
    );
    assert_eq!(TestCategory::Correctness.description(), "Outputs are correct");
    assert_eq!(TestCategory::Boundary.description(), "Edge cases handled");
    assert_eq!(
        TestCategory::Integration.description(),
        "Components work together"
    );
}

#[gpui_oa::test]
fn test_testgen_category_icons() {
    assert_eq!(TestCategory::AntiCheat.icon(), "AC");
    assert_eq!(TestCategory::Existence.icon(), "EX");
    assert_eq!(TestCategory::Correctness.icon(), "CO");
    assert_eq!(TestCategory::Boundary.icon(), "BO");
    assert_eq!(TestCategory::Integration.icon(), "IN");
}

// ============================================================================
// Status Label Tests
// ============================================================================

#[gpui_oa::test]
fn test_testgen_status_labels() {
    assert_eq!(TestGenStatus::Idle.label(), "Idle");
    assert_eq!(TestGenStatus::Generating.label(), "Generating");
    assert_eq!(TestGenStatus::Completed.label(), "Completed");
    assert_eq!(TestGenStatus::Failed.label(), "Failed");
}

// ============================================================================
// Selected Test Tests
// ============================================================================

#[gpui_oa::test]
fn test_testgen_no_test_selected_initially(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    let selected = TestGenFixture::selected_test_id(&view, cx);
    assert!(selected.is_none(), "No test should be selected initially");
}

// ============================================================================
// User Flow: Complete Generation Simulation
// ============================================================================

#[gpui_oa::test]
fn test_user_flow_simulate_generation(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // User starts with idle state
    view.assert_that(cx).generation_is_idle();

    // Simulate: User clicks generate (starts generation)
    TestGenFixture::simulate_generation_start(&view, cx);
    view.assert_that(cx).generation_is_running();
    assert!(TestGenFixture::is_generating(&view, cx));

    // Simulate: Progress updates as tests are generated
    TestGenFixture::simulate_generation_progress(&view, 2, 5, cx);
    assert!(TestGenFixture::is_generating(&view, cx));
    assert_eq!(TestGenFixture::total_tests(&view, cx), Some(5));

    // Simulate: Generation completes
    TestGenFixture::simulate_generation_complete(&view, 15, cx);
    view.assert_that(cx).generation_is_complete();
    assert!(TestGenFixture::is_complete(&view, cx));
    assert_eq!(TestGenFixture::total_tests(&view, cx), Some(15));
}

#[gpui_oa::test]
fn test_user_flow_generation_failure(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Start generation
    TestGenFixture::simulate_generation_start(&view, cx);
    view.assert_that(cx).generation_is_running();

    // Simulate failure
    TestGenFixture::simulate_generation_failed(&view, "API rate limit exceeded", cx);
    view.assert_that(cx).generation_is_failed();
    assert!(TestGenFixture::is_failed(&view, cx));
}

// ============================================================================
// User Flow: Test List Interaction
// ============================================================================

#[gpui_oa::test]
fn test_user_flow_add_and_view_tests(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Initially no tests
    view.assert_that(cx).has_no_generated_tests();

    // Add mock tests (simulating generation results)
    let tests = TestGenFixture::create_mock_test_suite();
    TestGenFixture::set_mock_tests(&view, tests, cx);

    // Verify tests were added
    let test = TestGenFixture::get_test(&view, "ac-1", cx);
    assert!(test.is_some());
    assert_eq!(test.unwrap().name, "anti_cheat_no_hardcode");
}

#[gpui_oa::test]
fn test_user_flow_select_test(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Add tests
    let tests = TestGenFixture::create_mock_test_suite();
    TestGenFixture::set_mock_tests(&view, tests, cx);

    // Select a test (simulating user click)
    TestGenFixture::select_test(&view, "co-1", cx);

    // Verify test is found (selection is in the list component)
    let test = TestGenFixture::get_test(&view, "co-1", cx);
    assert!(test.is_some());
    assert_eq!(test.unwrap().category, TestCategory::Correctness);
}

#[gpui_oa::test]
fn test_user_flow_clear_tests(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Add tests
    let tests = TestGenFixture::create_mock_test_suite();
    TestGenFixture::set_mock_tests(&view, tests, cx);

    // Verify tests exist
    let test = TestGenFixture::get_test(&view, "ac-1", cx);
    assert!(test.is_some());

    // Clear tests (simulating new generation starting)
    TestGenFixture::clear_tests(&view, cx);

    // Verify tests are gone
    let test = TestGenFixture::get_test(&view, "ac-1", cx);
    assert!(test.is_none());
}

// ============================================================================
// User Flow: Category Filtering
// ============================================================================

#[gpui_oa::test]
fn test_user_flow_filter_by_category(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Add tests from all categories
    let tests = TestGenFixture::create_mock_test_suite();
    TestGenFixture::set_mock_tests(&view, tests, cx);

    // Filter by Anti-Cheat category (simulating user clicking category)
    TestGenFixture::filter_by_category(&view, Some(TestCategory::AntiCheat), cx);

    // Anti-cheat tests should still be accessible
    let ac_test = TestGenFixture::get_test(&view, "ac-1", cx);
    assert!(ac_test.is_some());

    // Tests from other categories should still exist in the list
    // (filtering is just for display)
    let co_test = TestGenFixture::get_test(&view, "co-1", cx);
    assert!(co_test.is_some());
}

#[gpui_oa::test]
fn test_user_flow_clear_category_filter(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Add tests
    let tests = TestGenFixture::create_mock_test_suite();
    TestGenFixture::set_mock_tests(&view, tests, cx);

    // Apply filter
    TestGenFixture::filter_by_category(&view, Some(TestCategory::Boundary), cx);

    // Clear filter (user clicks "All" or clicks same category again)
    TestGenFixture::filter_by_category(&view, None, cx);

    // All tests should be accessible
    let ac_test = TestGenFixture::get_test(&view, "ac-1", cx);
    let bo_test = TestGenFixture::get_test(&view, "bo-1", cx);
    assert!(ac_test.is_some());
    assert!(bo_test.is_some());
}

// ============================================================================
// User Flow: Complete End-to-End Scenario
// ============================================================================

#[gpui_oa::test]
fn test_user_flow_complete_e2e_scenario(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // === Step 1: Initial state ===
    view.assert_that(cx)
        .has_no_task_selected()
        .generation_is_idle()
        .has_no_generated_tests();

    // === Step 2: User selects a task (if available) ===
    let task_count = TestGenFixture::task_count(&view, cx);
    if task_count > 0 {
        TestGenFixture::select_task(&view, 0, cx);
        view.assert_that(cx).has_task_selected().can_generate();
    }

    // === Step 3: User starts generation ===
    TestGenFixture::simulate_generation_start(&view, cx);
    view.assert_that(cx).generation_is_running().cannot_generate();

    // === Step 4: Tests arrive as generation progresses ===
    TestGenFixture::simulate_generation_progress(&view, 2, 3, cx);

    // Add some mock tests (simulating events from service)
    TestGenFixture::add_mock_test(
        &view,
        TestGenFixture::create_mock_test("gen-1", "generated_test_1", TestCategory::AntiCheat),
        cx,
    );
    TestGenFixture::add_mock_test(
        &view,
        TestGenFixture::create_mock_test("gen-2", "generated_test_2", TestCategory::Existence),
        cx,
    );
    TestGenFixture::add_mock_test(
        &view,
        TestGenFixture::create_mock_test("gen-3", "generated_test_3", TestCategory::Correctness),
        cx,
    );

    // === Step 5: Generation completes ===
    TestGenFixture::simulate_generation_complete(&view, 3, cx);
    view.assert_that(cx).generation_is_complete();

    // Can generate again now
    if task_count > 0 {
        view.assert_that(cx).can_generate();
    }

    // === Step 6: User clicks on a test to view details ===
    TestGenFixture::select_test(&view, "gen-2", cx);
    let test = TestGenFixture::get_test(&view, "gen-2", cx);
    assert!(test.is_some());
    assert_eq!(test.unwrap().category, TestCategory::Existence);

    // === Step 7: User filters by category ===
    TestGenFixture::filter_by_category(&view, Some(TestCategory::AntiCheat), cx);

    // === Step 8: User clears filter ===
    TestGenFixture::filter_by_category(&view, None, cx);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[gpui_oa::test]
fn test_edge_case_select_nonexistent_test(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Add tests
    let tests = TestGenFixture::create_mock_test_suite();
    TestGenFixture::set_mock_tests(&view, tests, cx);

    // Try to get a test that doesn't exist
    let test = TestGenFixture::get_test(&view, "nonexistent-test", cx);
    assert!(test.is_none());
}

#[gpui_oa::test]
fn test_edge_case_empty_test_list_filter(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Filter on empty list (should not crash)
    TestGenFixture::filter_by_category(&view, Some(TestCategory::Integration), cx);

    // Clear filter on empty list
    TestGenFixture::filter_by_category(&view, None, cx);
}

#[gpui_oa::test]
fn test_edge_case_rapid_status_changes(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Rapidly change status (simulating fast events)
    TestGenFixture::simulate_generation_start(&view, cx);
    TestGenFixture::simulate_generation_progress(&view, 1, 1, cx);
    TestGenFixture::simulate_generation_progress(&view, 2, 3, cx);
    TestGenFixture::simulate_generation_progress(&view, 3, 6, cx);
    TestGenFixture::simulate_generation_complete(&view, 10, cx);

    // Should end up in complete state
    view.assert_that(cx).generation_is_complete();
}

#[gpui_oa::test]
fn test_edge_case_add_tests_incrementally(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Add tests one by one (like real generation would)
    for i in 1..=5 {
        let test = TestGenFixture::create_mock_test(
            &format!("test-{}", i),
            &format!("incremental_test_{}", i),
            TestCategory::Correctness,
        );
        TestGenFixture::add_mock_test(&view, test, cx);
    }

    // All tests should exist
    for i in 1..=5 {
        let test = TestGenFixture::get_test(&view, &format!("test-{}", i), cx);
        assert!(test.is_some(), "Test {} should exist", i);
    }
}
