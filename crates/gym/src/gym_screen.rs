//! Main Gym Screen component
//!
//! Multi-view container with tab navigation and sidebar.

use gpui::*;

use super::types::{GymTab, SidebarState};
use super::sidebar::Sidebar;
use super::trajectory_view::TrajectoryView;

pub struct GymScreen {
    /// Current active tab
    current_tab: GymTab,

    /// Focus handle
    focus_handle: FocusHandle,

    /// Sidebar state
    sidebar_state: SidebarState,
    sidebar_width: Pixels,
    sidebar_collapsed: bool,

    // View entities (to be implemented)
    // trajectory_view: Entity<TrajectoryView>,
    // tbcc_view: Entity<TBCCView>,
    // hillclimber_view: Entity<HillClimberMonitor>,
    // testgen_view: Entity<TestGenVisualizer>,
}

impl GymScreen {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            current_tab: GymTab::default(),
            focus_handle: cx.focus_handle(),
            sidebar_state: SidebarState::new(),
            sidebar_width: px(260.0),
            sidebar_collapsed: false,
        }
    }

    pub fn switch_tab(&mut self, tab: GymTab, cx: &mut Context<Self>) {
        self.current_tab = tab;
        cx.notify();
    }
}

impl Focusable for GymScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for GymScreen {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .h_full()
            .w_full()
            .child("Gym Screen - Under Construction")
    }
}
