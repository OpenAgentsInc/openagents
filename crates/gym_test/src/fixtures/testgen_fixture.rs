//! TestGen test fixture
//!
//! Page Object Model fixture for testing the TestGen Visualizer component.

use gpui::{Entity, TestAppContext};
use gym::testgen::visualizer::{TestGenVisualizer, TestGenSession, TestGenStatus};
use gym::testgen::category_progress::TestCategory;
use gym::testgen::test_list::TestStatus;

/// Page Object Model fixture for TestGenVisualizer
pub struct TestGenFixture;

impl TestGenFixture {
    /// Create a new TestGenVisualizer in a test window
    pub fn create(cx: &mut TestAppContext) -> Entity<TestGenVisualizer> {
        let (view, _vcx) = cx.add_window_view(|_window, cx| TestGenVisualizer::new(cx));
        view
    }

    /// Get the current session
    pub fn session(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<TestGenSession> {
        cx.read(|cx| view.read(cx).session.clone())
    }

    /// Check if has active session
    pub fn has_session(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        Self::session(view, cx).is_some()
    }

    /// Get session status
    pub fn session_status(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<TestGenStatus> {
        Self::session(view, cx).map(|s| s.status)
    }

    /// Check if session is generating
    pub fn is_generating(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        matches!(Self::session_status(view, cx), Some(TestGenStatus::Generating))
    }

    /// Check if session is completed
    pub fn is_completed(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        matches!(Self::session_status(view, cx), Some(TestGenStatus::Completed))
    }

    /// Check if session is idle
    pub fn is_idle(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        matches!(Self::session_status(view, cx), Some(TestGenStatus::Idle))
    }

    /// Get current iteration
    pub fn iteration(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<u32> {
        Self::session(view, cx).map(|s| s.iteration)
    }

    /// Get max iterations
    pub fn max_iterations(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<u32> {
        Self::session(view, cx).map(|s| s.max_iterations)
    }

    /// Get comprehensiveness score (0.0 - 1.0)
    pub fn comprehensiveness(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<f32> {
        Self::session(view, cx).map(|s| s.comprehensiveness)
    }

    /// Get target comprehensiveness score (0.0 - 1.0)
    pub fn target_comprehensiveness(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<f32> {
        Self::session(view, cx).map(|s| s.target_comprehensiveness)
    }

    /// Check if comprehensiveness target is met
    pub fn is_target_met(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> bool {
        Self::session(view, cx)
            .map(|s| s.comprehensiveness >= s.target_comprehensiveness)
            .unwrap_or(false)
    }

    /// Get task name
    pub fn task_name(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<String> {
        Self::session(view, cx).map(|s| s.task_name)
    }

    /// Get task id
    pub fn task_id(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<String> {
        Self::session(view, cx).map(|s| s.task_id)
    }

    /// Get selected test id
    pub fn selected_test_id(view: &Entity<TestGenVisualizer>, cx: &TestAppContext) -> Option<String> {
        cx.read(|cx| view.read(cx).selected_test_id.clone())
    }
}
