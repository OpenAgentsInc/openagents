//! Fluent assertions for Gym component testing
//!
//! Provides chainable assertion methods for ergonomic test writing.

use gpui::{Entity, TestAppContext};
use gym::{GymScreen, GymTab};
use gym::tbcc::{TBCCScreen, TBCCTab};
use gym::hillclimber::monitor::{HillClimberMonitor, HCSessionStatus, HCMode};
use gym::testgen::visualizer::{TestGenVisualizer, TestGenStatus};

// ============================================================================
// GymScreen Assertions
// ============================================================================

/// Fluent assertions for GymScreen
pub struct GymScreenAssertions<'a> {
    view: &'a Entity<GymScreen>,
    cx: &'a TestAppContext,
}

impl<'a> GymScreenAssertions<'a> {
    pub fn new(view: &'a Entity<GymScreen>, cx: &'a TestAppContext) -> Self {
        Self { view, cx }
    }

    /// Assert the current tab
    pub fn has_tab(self, expected: GymTab) -> Self {
        let actual = self.cx.read(|cx| self.view.read(cx).current_tab);
        assert_eq!(actual, expected, "Expected tab {:?}, got {:?}", expected, actual);
        self
    }

    /// Assert sidebar is collapsed
    pub fn sidebar_is_collapsed(self) -> Self {
        let collapsed = self.cx.read(|cx| self.view.read(cx).sidebar_collapsed);
        assert!(collapsed, "Expected sidebar to be collapsed");
        self
    }

    /// Assert sidebar is expanded
    pub fn sidebar_is_expanded(self) -> Self {
        let collapsed = self.cx.read(|cx| self.view.read(cx).sidebar_collapsed);
        assert!(!collapsed, "Expected sidebar to be expanded");
        self
    }

    /// Assert on Trajectories tab
    pub fn is_on_trajectories(self) -> Self {
        self.has_tab(GymTab::Trajectories)
    }

    /// Assert on TBCC tab
    pub fn is_on_tbcc(self) -> Self {
        self.has_tab(GymTab::TBCC)
    }

    /// Assert on HillClimber tab
    pub fn is_on_hillclimber(self) -> Self {
        self.has_tab(GymTab::HillClimber)
    }

    /// Assert on TestGen tab
    pub fn is_on_testgen(self) -> Self {
        self.has_tab(GymTab::TestGen)
    }
}

/// Extension trait for GymScreen assertions
pub trait GymScreenAssertExt {
    fn assert_that<'a>(&'a self, cx: &'a TestAppContext) -> GymScreenAssertions<'a>;
}

impl GymScreenAssertExt for Entity<GymScreen> {
    fn assert_that<'a>(&'a self, cx: &'a TestAppContext) -> GymScreenAssertions<'a> {
        GymScreenAssertions::new(self, cx)
    }
}

// ============================================================================
// TBCCScreen Assertions
// ============================================================================

/// Fluent assertions for TBCCScreen
pub struct TBCCAssertions<'a> {
    view: &'a Entity<TBCCScreen>,
    cx: &'a TestAppContext,
}

impl<'a> TBCCAssertions<'a> {
    pub fn new(view: &'a Entity<TBCCScreen>, cx: &'a TestAppContext) -> Self {
        Self { view, cx }
    }

    /// Assert the current tab
    pub fn has_tab(self, expected: TBCCTab) -> Self {
        let actual = self.cx.read(|cx| self.view.read(cx).current_tab);
        assert_eq!(actual, expected, "Expected TBCC tab {:?}, got {:?}", expected, actual);
        self
    }

    /// Assert on Dashboard tab
    pub fn is_on_dashboard(self) -> Self {
        self.has_tab(TBCCTab::Dashboard)
    }

    /// Assert on Tasks tab
    pub fn is_on_tasks(self) -> Self {
        self.has_tab(TBCCTab::Tasks)
    }

    /// Assert on Runs tab
    pub fn is_on_runs(self) -> Self {
        self.has_tab(TBCCTab::Runs)
    }

    /// Assert on Settings tab
    pub fn is_on_settings(self) -> Self {
        self.has_tab(TBCCTab::Settings)
    }
}

/// Extension trait for TBCCScreen assertions
pub trait TBCCAssertExt {
    fn assert_that<'a>(&'a self, cx: &'a TestAppContext) -> TBCCAssertions<'a>;
}

impl TBCCAssertExt for Entity<TBCCScreen> {
    fn assert_that<'a>(&'a self, cx: &'a TestAppContext) -> TBCCAssertions<'a> {
        TBCCAssertions::new(self, cx)
    }
}

// ============================================================================
// HillClimberMonitor Assertions
// ============================================================================

/// Fluent assertions for HillClimberMonitor
pub struct HillClimberAssertions<'a> {
    view: &'a Entity<HillClimberMonitor>,
    cx: &'a TestAppContext,
}

impl<'a> HillClimberAssertions<'a> {
    pub fn new(view: &'a Entity<HillClimberMonitor>, cx: &'a TestAppContext) -> Self {
        Self { view, cx }
    }

    /// Assert has session
    pub fn has_session(self) -> Self {
        let has = self.cx.read(|cx| self.view.read(cx).session.is_some());
        assert!(has, "Expected HillClimber to have a session");
        self
    }

    /// Assert no session
    pub fn has_no_session(self) -> Self {
        let has = self.cx.read(|cx| self.view.read(cx).session.is_some());
        assert!(!has, "Expected HillClimber to have no session");
        self
    }

    /// Assert session status
    pub fn has_status(self, expected: HCSessionStatus) -> Self {
        let status = self.cx.read(|cx| self.view.read(cx).session.as_ref().map(|s| s.status));
        assert_eq!(status, Some(expected), "Expected status {:?}, got {:?}", expected, status);
        self
    }

    /// Assert session is running
    pub fn is_running(self) -> Self {
        self.has_status(HCSessionStatus::Running)
    }

    /// Assert session is completed
    pub fn is_completed(self) -> Self {
        self.has_status(HCSessionStatus::Completed)
    }

    /// Assert session is idle
    pub fn is_idle(self) -> Self {
        self.has_status(HCSessionStatus::Idle)
    }

    /// Assert session is paused
    pub fn is_paused(self) -> Self {
        self.has_status(HCSessionStatus::Paused)
    }

    /// Assert session mode
    pub fn has_mode(self, expected: HCMode) -> Self {
        let mode = self.cx.read(|cx| self.view.read(cx).session.as_ref().map(|s| s.mode));
        assert_eq!(mode, Some(expected), "Expected mode {:?}, got {:?}", expected, mode);
        self
    }

    /// Assert turn count
    pub fn has_turn(self, expected: u32) -> Self {
        let turn = self.cx.read(|cx| self.view.read(cx).session.as_ref().map(|s| s.current_turn));
        assert_eq!(turn, Some(expected), "Expected turn {}, got {:?}", expected, turn);
        self
    }

    /// Assert tests passed count
    pub fn has_tests_passed(self, expected: u32) -> Self {
        let passed = self.cx.read(|cx| self.view.read(cx).session.as_ref().map(|s| s.tests_passed));
        assert_eq!(passed, Some(expected), "Expected {} tests passed, got {:?}", expected, passed);
        self
    }

    /// Assert tests total count
    pub fn has_tests_total(self, expected: u32) -> Self {
        let total = self.cx.read(|cx| self.view.read(cx).session.as_ref().map(|s| s.tests_total));
        assert_eq!(total, Some(expected), "Expected {} tests total, got {:?}", expected, total);
        self
    }

    /// Assert task name
    pub fn has_task_name(self, expected: &str) -> Self {
        let name = self.cx.read(|cx| self.view.read(cx).session.as_ref().map(|s| s.task_name.clone()));
        assert_eq!(name.as_deref(), Some(expected), "Expected task name '{}', got {:?}", expected, name);
        self
    }
}

/// Extension trait for HillClimberMonitor assertions
pub trait HillClimberAssertExt {
    fn assert_that<'a>(&'a self, cx: &'a TestAppContext) -> HillClimberAssertions<'a>;
}

impl HillClimberAssertExt for Entity<HillClimberMonitor> {
    fn assert_that<'a>(&'a self, cx: &'a TestAppContext) -> HillClimberAssertions<'a> {
        HillClimberAssertions::new(self, cx)
    }
}

// ============================================================================
// TestGenVisualizer Assertions
// ============================================================================

/// Fluent assertions for TestGenVisualizer
pub struct TestGenAssertions<'a> {
    view: &'a Entity<TestGenVisualizer>,
    cx: &'a TestAppContext,
}

impl<'a> TestGenAssertions<'a> {
    pub fn new(view: &'a Entity<TestGenVisualizer>, cx: &'a TestAppContext) -> Self {
        Self { view, cx }
    }

    /// Assert has session
    pub fn has_session(self) -> Self {
        let has = self.cx.read(|cx| self.view.read(cx).session.is_some());
        assert!(has, "Expected TestGen to have a session");
        self
    }

    /// Assert no session
    pub fn has_no_session(self) -> Self {
        let has = self.cx.read(|cx| self.view.read(cx).session.is_some());
        assert!(!has, "Expected TestGen to have no session");
        self
    }

    /// Assert session status
    pub fn has_status(self, expected: TestGenStatus) -> Self {
        let status = self.cx.read(|cx| self.view.read(cx).session.as_ref().map(|s| s.status));
        assert_eq!(status, Some(expected), "Expected status {:?}, got {:?}", expected, status);
        self
    }

    /// Assert session is generating
    pub fn is_generating(self) -> Self {
        self.has_status(TestGenStatus::Generating)
    }

    /// Assert session is completed
    pub fn is_completed(self) -> Self {
        self.has_status(TestGenStatus::Completed)
    }

    /// Assert session is idle
    pub fn is_idle(self) -> Self {
        self.has_status(TestGenStatus::Idle)
    }

    /// Assert iteration count
    pub fn has_iteration(self, expected: u32) -> Self {
        let iteration = self.cx.read(|cx| self.view.read(cx).session.as_ref().map(|s| s.iteration));
        assert_eq!(iteration, Some(expected), "Expected iteration {}, got {:?}", expected, iteration);
        self
    }

    /// Assert comprehensiveness score (0.0 - 1.0)
    pub fn has_comprehensiveness_at_least(self, min: f32) -> Self {
        let score = self.cx.read(|cx| self.view.read(cx).session.as_ref().map(|s| s.comprehensiveness));
        assert!(
            score.map_or(false, |s| s >= min),
            "Expected comprehensiveness >= {}, got {:?}",
            min,
            score
        );
        self
    }

    /// Assert comprehensiveness target is met
    pub fn target_is_met(self) -> Self {
        let (score, target) = self.cx.read(|cx| {
            self.view.read(cx).session.as_ref().map(|s| (s.comprehensiveness, s.target_comprehensiveness))
        }).unwrap_or((0.0, 1.0));
        assert!(
            score >= target,
            "Expected comprehensiveness {} >= target {}",
            score,
            target
        );
        self
    }

    /// Assert task name
    pub fn has_task_name(self, expected: &str) -> Self {
        let name = self.cx.read(|cx| self.view.read(cx).session.as_ref().map(|s| s.task_name.clone()));
        assert_eq!(name.as_deref(), Some(expected), "Expected task name '{}', got {:?}", expected, name);
        self
    }
}

/// Extension trait for TestGenVisualizer assertions
pub trait TestGenAssertExt {
    fn assert_that<'a>(&'a self, cx: &'a TestAppContext) -> TestGenAssertions<'a>;
}

impl TestGenAssertExt for Entity<TestGenVisualizer> {
    fn assert_that<'a>(&'a self, cx: &'a TestAppContext) -> TestGenAssertions<'a> {
        TestGenAssertions::new(self, cx)
    }
}
