//! TestGen test fixture
//!
//! Page Object Model fixture for testing the TestGen Visualizer component.

use gpui::{Entity, TestAppContext};
use gym::testgen::visualizer::{GenerationStatus, TestGenSession, TestGenStatus, TestGenVisualizer};

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
}
