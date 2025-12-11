//! HillClimber test fixture
//!
//! Page Object Model fixture for testing the HillClimber Monitor component.

use gpui_oa::{Entity, TestAppContext};
use crate::hillclimber::monitor::{HillClimberMonitor, HCSession, HCSessionStatus, HCMode};

/// Page Object Model fixture for HillClimberMonitor
pub struct HillClimberFixture;

impl HillClimberFixture {
    /// Create a new HillClimberMonitor in a test window
    pub fn create(cx: &mut TestAppContext) -> Entity<HillClimberMonitor> {
        let (view, _vcx) = cx.add_window_view(|_window, cx| HillClimberMonitor::new(cx));
        view
    }

    /// Get the current session
    pub fn session(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> Option<HCSession> {
        cx.read(|cx| view.read(cx).session.clone())
    }

    /// Check if has active session
    pub fn has_session(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> bool {
        Self::session(view, cx).is_some()
    }

    /// Get session status
    pub fn session_status(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> Option<HCSessionStatus> {
        Self::session(view, cx).map(|s| s.status)
    }

    /// Check if session is running
    pub fn is_running(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> bool {
        matches!(Self::session_status(view, cx), Some(HCSessionStatus::Running))
    }

    /// Check if session is completed
    pub fn is_completed(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> bool {
        matches!(Self::session_status(view, cx), Some(HCSessionStatus::Completed))
    }

    /// Check if session is idle
    pub fn is_idle(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> bool {
        matches!(Self::session_status(view, cx), Some(HCSessionStatus::Idle))
    }

    /// Get current turn number
    pub fn current_turn(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> Option<u32> {
        Self::session(view, cx).map(|s| s.current_turn)
    }

    /// Get max turns
    pub fn max_turns(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> Option<u32> {
        Self::session(view, cx).map(|s| s.max_turns)
    }

    /// Get tests passed count
    pub fn tests_passed(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> Option<u32> {
        Self::session(view, cx).map(|s| s.tests_passed)
    }

    /// Get tests total count
    pub fn tests_total(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> Option<u32> {
        Self::session(view, cx).map(|s| s.tests_total)
    }

    /// Get pass rate as percentage
    pub fn pass_rate(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> Option<f32> {
        Self::session(view, cx).map(|s| {
            if s.tests_total == 0 {
                0.0
            } else {
                (s.tests_passed as f32 / s.tests_total as f32) * 100.0
            }
        })
    }

    /// Get session mode
    pub fn mode(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> Option<HCMode> {
        Self::session(view, cx).map(|s| s.mode)
    }

    /// Check if left panel is collapsed
    pub fn left_collapsed(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> bool {
        cx.read(|cx| view.read(cx).left_collapsed)
    }

    /// Get task name
    pub fn task_name(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> Option<String> {
        Self::session(view, cx).map(|s| s.task_name)
    }

    /// Get task id
    pub fn task_id(view: &Entity<HillClimberMonitor>, cx: &TestAppContext) -> Option<String> {
        Self::session(view, cx).map(|s| s.task_id)
    }
}
