//! GymScreen test fixture
//!
//! Page Object Model fixture for testing the main GymScreen component.

use gpui::{Entity, TestAppContext};
use gym::{GymScreen, GymTab};

/// Page Object Model fixture for GymScreen
pub struct GymScreenFixture;

impl GymScreenFixture {
    /// Create a new GymScreen in a test window
    pub fn create(cx: &mut TestAppContext) -> Entity<GymScreen> {
        let (view, _vcx) = cx.add_window_view(|_window, cx| GymScreen::new(cx));
        view
    }

    /// Get the current active tab
    pub fn current_tab(view: &Entity<GymScreen>, cx: &TestAppContext) -> GymTab {
        cx.read(|cx| view.read(cx).current_tab)
    }

    /// Check if sidebar is collapsed
    pub fn sidebar_collapsed(view: &Entity<GymScreen>, cx: &TestAppContext) -> bool {
        cx.read(|cx| view.read(cx).sidebar_collapsed)
    }

    /// Get sidebar width
    pub fn sidebar_width(view: &Entity<GymScreen>, cx: &TestAppContext) -> f32 {
        cx.read(|cx| view.read(cx).sidebar_width.0)
    }

    /// Switch to a specific tab
    pub fn switch_tab(view: &Entity<GymScreen>, tab: GymTab, cx: &mut TestAppContext) {
        view.update(cx, |screen, cx| {
            screen.switch_tab(tab, cx);
        });
    }

    /// Toggle sidebar
    pub fn toggle_sidebar(view: &Entity<GymScreen>, cx: &mut TestAppContext) {
        view.update(cx, |screen, cx| {
            screen.toggle_sidebar(cx);
        });
    }

    /// Switch to Trajectories tab
    pub fn go_to_trajectories(view: &Entity<GymScreen>, cx: &mut TestAppContext) {
        Self::switch_tab(view, GymTab::Trajectories, cx);
    }

    /// Switch to TBCC tab
    pub fn go_to_tbcc(view: &Entity<GymScreen>, cx: &mut TestAppContext) {
        Self::switch_tab(view, GymTab::TBCC, cx);
    }

    /// Switch to HillClimber tab
    pub fn go_to_hillclimber(view: &Entity<GymScreen>, cx: &mut TestAppContext) {
        Self::switch_tab(view, GymTab::HillClimber, cx);
    }

    /// Switch to TestGen tab
    pub fn go_to_testgen(view: &Entity<GymScreen>, cx: &mut TestAppContext) {
        Self::switch_tab(view, GymTab::TestGen, cx);
    }
}
