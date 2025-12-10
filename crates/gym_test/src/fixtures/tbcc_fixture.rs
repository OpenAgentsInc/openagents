//! TBCC test fixture
//!
//! Page Object Model fixture for testing the TBCC (TerminalBench Command Center) component.

use gpui::{Entity, TestAppContext};
use gym::tbcc::{TBCCScreen, TBCCTab};

/// Page Object Model fixture for TBCCScreen
pub struct TBCCFixture;

impl TBCCFixture {
    /// Create a new TBCCScreen in a test window
    pub fn create(cx: &mut TestAppContext) -> Entity<TBCCScreen> {
        let (view, _vcx) = cx.add_window_view(|_window, cx| TBCCScreen::new(cx));
        view
    }

    /// Get the current active tab
    pub fn current_tab(view: &Entity<TBCCScreen>, cx: &TestAppContext) -> TBCCTab {
        cx.read(|cx| view.read(cx).current_tab)
    }

    /// Switch to a specific tab
    pub fn switch_tab(view: &Entity<TBCCScreen>, tab: TBCCTab, cx: &mut TestAppContext) {
        view.update(cx, |screen, cx| {
            screen.switch_tab(tab, cx);
        });
    }

    /// Switch to Dashboard tab
    pub fn go_to_dashboard(view: &Entity<TBCCScreen>, cx: &mut TestAppContext) {
        Self::switch_tab(view, TBCCTab::Dashboard, cx);
    }

    /// Switch to Tasks tab
    pub fn go_to_tasks(view: &Entity<TBCCScreen>, cx: &mut TestAppContext) {
        Self::switch_tab(view, TBCCTab::Tasks, cx);
    }

    /// Switch to Runs tab
    pub fn go_to_runs(view: &Entity<TBCCScreen>, cx: &mut TestAppContext) {
        Self::switch_tab(view, TBCCTab::Runs, cx);
    }

    /// Switch to Settings tab
    pub fn go_to_settings(view: &Entity<TBCCScreen>, cx: &mut TestAppContext) {
        Self::switch_tab(view, TBCCTab::Settings, cx);
    }

    /// Check if on Dashboard tab
    pub fn is_on_dashboard(view: &Entity<TBCCScreen>, cx: &TestAppContext) -> bool {
        Self::current_tab(view, cx) == TBCCTab::Dashboard
    }

    /// Check if on Tasks tab
    pub fn is_on_tasks(view: &Entity<TBCCScreen>, cx: &TestAppContext) -> bool {
        Self::current_tab(view, cx) == TBCCTab::Tasks
    }

    /// Check if on Runs tab
    pub fn is_on_runs(view: &Entity<TBCCScreen>, cx: &TestAppContext) -> bool {
        Self::current_tab(view, cx) == TBCCTab::Runs
    }

    /// Check if on Settings tab
    pub fn is_on_settings(view: &Entity<TBCCScreen>, cx: &TestAppContext) -> bool {
        Self::current_tab(view, cx) == TBCCTab::Settings
    }
}
