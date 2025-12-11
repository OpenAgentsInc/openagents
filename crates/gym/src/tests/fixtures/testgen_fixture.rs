//! TestGen test fixture
//!
//! Page Object Model fixture for testing the TestGen Visualizer component.

use gpui_oa::{Entity, TestAppContext};
use crate::testgen::category_progress::TestCategory;
use crate::testgen::test_list::{TestCase, TestStatus};
use crate::testgen::visualizer::{GenerationStatus, TestGenSession, TestGenStatus, TestGenVisualizer};

/// Page Object Model fixture for TestGenVisualizer
pub struct TestGenFixture;

impl TestGenFixture {
    // ========================================================================
    // Creation
    // ========================================================================

    /// Create a new TestGenVisualizer in a test window
    pub fn create(cx: &mut TestAppContext) -> Entity<TestGenVisualizer> {
        let (view, _vcx) = cx.add_window_view(|_window, cx| TestGenVisualizer::new(cx));
        view
    }

    // ========================================================================
    // Task Selection Queries
    // ========================================================================

    /// Get number of available tasks
    pub fn task_count(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> usize {
        cx.read(|cx| view.read(cx).available_tasks.len())
    }

    /// Get selected task index
    pub fn selected_task_idx(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<usize> {
        cx.read(|cx| view.read(cx).selected_task_idx)
    }

    /// Check if a task is selected
    pub fn has_task_selected(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        Self::selected_task_idx(view, cx).is_some()
    }

    /// Get selected task name
    pub fn selected_task_name(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<String> {
        cx.read(|cx| view.read(cx).selected_task().map(|t| t.name.clone()))
    }

    /// Get selected task id
    pub fn selected_task_id(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<String> {
        cx.read(|cx| view.read(cx).selected_task().map(|t| t.id.clone()))
    }

    // ========================================================================
    // Generation Status Queries
    // ========================================================================

    /// Get the current generation status
    pub fn generation_status(
        view: &Entity<TestGenVisualizer>,
        cx: &TestAppContext,
    ) -> GenerationStatus {
        cx.read(|cx| view.read(cx).generation_status.clone())
    }

    /// Check if generation is idle (ready to start)
    pub fn is_idle(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        matches!(Self::generation_status(view, cx), GenerationStatus::Idle)
    }

    /// Check if generation is in progress
    pub fn is_generating(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        matches!(
            Self::generation_status(view, cx),
            GenerationStatus::Generating { .. }
        )
    }

    /// Check if generation is complete
    pub fn is_complete(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        matches!(
            Self::generation_status(view, cx),
            GenerationStatus::Complete { .. }
        )
    }

    /// Check if generation failed
    pub fn is_failed(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        matches!(
            Self::generation_status(view, cx),
            GenerationStatus::Failed { .. }
        )
    }

    /// Check if can start generation (task selected and not generating)
    pub fn can_generate(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        cx.read(|cx| view.read(cx).can_generate())
    }

    /// Get total tests generated (if complete)
    pub fn total_tests(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<u32> {
        match Self::generation_status(view, cx) {
            GenerationStatus::Complete { total_tests, .. } => Some(total_tests),
            GenerationStatus::Generating { tests_so_far, .. } => Some(tests_so_far),
            _ => None,
        }
    }

    // ========================================================================
    // Generated Tests Queries
    // ========================================================================

    /// Get count of generated tests in memory
    pub fn generated_test_count(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> usize {
        cx.read(|cx| view.read(cx).generated_tests.len())
    }

    /// Get selected test id
    pub fn selected_test_id(
        view: &Entity<TestGenVisualizer>,
        cx: &TestAppContext,
    ) -> Option<String> {
        cx.read(|cx| view.read(cx).selected_test_id.clone())
    }

    // ========================================================================
    // Legacy Session Queries (for backward compatibility)
    // ========================================================================

    /// Get the current session (legacy - may be None in new implementation)
    pub fn session(
        view: &Entity<TestGenVisualizer>,
        cx: &TestAppContext,
    ) -> Option<TestGenSession> {
        cx.read(|cx| view.read(cx).session.clone())
    }

    /// Check if has active session (legacy)
    pub fn has_session(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        Self::session(view, cx).is_some()
    }

    /// Get session status (legacy)
    pub fn session_status(
        view: &Entity<TestGenVisualizer>,
        cx: &TestAppContext,
    ) -> Option<TestGenStatus> {
        Self::session(view, cx).map(|s| s.status)
    }

    // ========================================================================
    // Actions
    // ========================================================================

    /// Select task by index
    pub fn select_task(view: &Entity<TestGenVisualizer>, idx: usize, cx: &mut TestAppContext) {
        view.update(cx, |v, cx| {
            v.select_task(idx, cx);
        });
        cx.run_until_parked();
    }

    /// Select next task
    pub fn select_next_task(view: &Entity<TestGenVisualizer>, cx: &mut TestAppContext) {
        view.update(cx, |v, cx| {
            v.select_next_task(cx);
        });
        cx.run_until_parked();
    }

    /// Select previous task
    pub fn select_prev_task(view: &Entity<TestGenVisualizer>, cx: &mut TestAppContext) {
        view.update(cx, |v, cx| {
            v.select_prev_task(cx);
        });
        cx.run_until_parked();
    }

    // ========================================================================
    // Test List Actions (for simulating user interactions)
    // ========================================================================

    /// Add a mock test to the test list
    pub fn add_mock_test(
        view: &Entity<TestGenVisualizer>,
        test: TestCase,
        cx: &mut TestAppContext,
    ) {
        view.update(cx, |v, cx| {
            v.test_list.update(cx, |list, _cx| {
                list.add_test(test);
            });
        });
        cx.run_until_parked();
    }

    /// Set multiple mock tests in the test list
    pub fn set_mock_tests(
        view: &Entity<TestGenVisualizer>,
        tests: Vec<TestCase>,
        cx: &mut TestAppContext,
    ) {
        view.update(cx, |v, cx| {
            v.test_list.update(cx, |list, _cx| {
                list.set_tests(tests);
            });
        });
        cx.run_until_parked();
    }

    /// Clear all tests from the test list
    pub fn clear_tests(view: &Entity<TestGenVisualizer>, cx: &mut TestAppContext) {
        view.update(cx, |v, cx| {
            v.test_list.update(cx, |list, _cx| {
                list.clear_tests();
            });
        });
        cx.run_until_parked();
    }

    /// Select a test by ID in the test list
    pub fn select_test(
        view: &Entity<TestGenVisualizer>,
        test_id: &str,
        cx: &mut TestAppContext,
    ) {
        let id = test_id.to_string();
        view.update(cx, |v, cx| {
            v.test_list.update(cx, |list, _cx| {
                list.select(id);
            });
        });
        cx.run_until_parked();
    }

    /// Filter tests by category
    pub fn filter_by_category(
        view: &Entity<TestGenVisualizer>,
        category: Option<TestCategory>,
        cx: &mut TestAppContext,
    ) {
        view.update(cx, |v, cx| {
            v.test_list.update(cx, |list, _cx| {
                list.filter_by_category(category);
            });
        });
        cx.run_until_parked();
    }

    /// Get a test from the list by ID
    pub fn get_test(
        view: &Entity<TestGenVisualizer>,
        test_id: &str,
        cx: &TestAppContext,
    ) -> Option<TestCase> {
        cx.read(|cx| {
            let visualizer = view.read(cx);
            visualizer.test_list.read(cx).get_test(test_id).cloned()
        })
    }

    // ========================================================================
    // Generation Status Actions (for simulating generation)
    // ========================================================================

    /// Set the generation status (for testing)
    pub fn set_generation_status(
        view: &Entity<TestGenVisualizer>,
        status: GenerationStatus,
        cx: &mut TestAppContext,
    ) {
        view.update(cx, |v, _cx| {
            v.generation_status = status;
        });
        cx.run_until_parked();
    }

    /// Simulate generation starting
    pub fn simulate_generation_start(
        view: &Entity<TestGenVisualizer>,
        cx: &mut TestAppContext,
    ) {
        Self::set_generation_status(
            view,
            GenerationStatus::Generating {
                iteration: 1,
                max_iterations: 8,
                tests_so_far: 0,
            },
            cx,
        );
    }

    /// Simulate generation progress
    pub fn simulate_generation_progress(
        view: &Entity<TestGenVisualizer>,
        iteration: u32,
        tests_so_far: u32,
        cx: &mut TestAppContext,
    ) {
        Self::set_generation_status(
            view,
            GenerationStatus::Generating {
                iteration,
                max_iterations: 8,
                tests_so_far,
            },
            cx,
        );
    }

    /// Simulate generation complete
    pub fn simulate_generation_complete(
        view: &Entity<TestGenVisualizer>,
        total_tests: u32,
        cx: &mut TestAppContext,
    ) {
        Self::set_generation_status(
            view,
            GenerationStatus::Complete {
                total_tests,
                duration_ms: 5000,
            },
            cx,
        );
    }

    /// Simulate generation failed
    pub fn simulate_generation_failed(
        view: &Entity<TestGenVisualizer>,
        error: &str,
        cx: &mut TestAppContext,
    ) {
        Self::set_generation_status(
            view,
            GenerationStatus::Failed {
                error: error.to_string(),
            },
            cx,
        );
    }

    // ========================================================================
    // Test Creation Helpers
    // ========================================================================

    /// Create a mock test case for testing
    pub fn create_mock_test(id: &str, name: &str, category: TestCategory) -> TestCase {
        TestCase {
            id: id.to_string(),
            name: name.to_string(),
            category,
            status: TestStatus::Generated,
            description: format!("Test description for {}", name),
            code: format!("def {}():\n    assert True", name),
            confidence: 0.85,
        }
    }

    /// Create a set of mock tests covering all categories
    pub fn create_mock_test_suite() -> Vec<TestCase> {
        vec![
            Self::create_mock_test("ac-1", "anti_cheat_no_hardcode", TestCategory::AntiCheat),
            Self::create_mock_test("ac-2", "anti_cheat_random_input", TestCategory::AntiCheat),
            Self::create_mock_test("ex-1", "existence_basic", TestCategory::Existence),
            Self::create_mock_test("ex-2", "existence_output_format", TestCategory::Existence),
            Self::create_mock_test("co-1", "correctness_simple", TestCategory::Correctness),
            Self::create_mock_test("co-2", "correctness_complex", TestCategory::Correctness),
            Self::create_mock_test("bo-1", "boundary_empty_input", TestCategory::Boundary),
            Self::create_mock_test("bo-2", "boundary_large_input", TestCategory::Boundary),
            Self::create_mock_test("in-1", "integration_full_flow", TestCategory::Integration),
        ]
    }
}
