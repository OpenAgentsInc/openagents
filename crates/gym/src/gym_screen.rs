//! Main Gym Screen component
//!
//! Multi-view container with tab navigation and sidebar.

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, text, FONT_FAMILY};

use super::types::GymTab;
use super::sidebar::Sidebar;
use super::trajectory_view::TrajectoryView;
use super::tbcc::dashboard::DashboardView;
use super::hillclimber::monitor::HillClimberMonitor;
use super::testgen::visualizer::TestGenVisualizer;

pub struct GymScreen {
    /// Current active tab
    current_tab: GymTab,

    /// Focus handle
    focus_handle: FocusHandle,

    /// Sidebar
    sidebar: Entity<Sidebar>,
    sidebar_width: Pixels,
    sidebar_collapsed: bool,

    /// View entities
    trajectory_view: Entity<TrajectoryView>,
    dashboard_view: Entity<DashboardView>,
    hillclimber_view: Entity<HillClimberMonitor>,
    testgen_view: Entity<TestGenVisualizer>,
}

impl GymScreen {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| Sidebar::new(cx));
        let trajectory_view = cx.new(|cx| TrajectoryView::new(cx));
        let dashboard_view = cx.new(|cx| DashboardView::new(cx));
        let hillclimber_view = cx.new(|cx| HillClimberMonitor::new(cx));
        let testgen_view = cx.new(|cx| TestGenVisualizer::new(cx));

        Self {
            current_tab: GymTab::default(),
            focus_handle: cx.focus_handle(),
            sidebar,
            sidebar_width: px(260.0),
            sidebar_collapsed: false,
            trajectory_view,
            dashboard_view,
            hillclimber_view,
            testgen_view,
        }
    }

    pub fn switch_tab(&mut self, tab: GymTab, cx: &mut Context<Self>) {
        self.current_tab = tab;
        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    fn render_tab_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .h(px(48.0))
            .bg(bg::SURFACE)
            .border_b_1()
            .border_color(border::DEFAULT)
            .px(px(20.0))
            .gap(px(4.0))
            .children(GymTab::all().iter().map(|tab| {
                let is_active = *tab == self.current_tab;
                let tab_clone = *tab;

                div()
                    .px(px(16.0))
                    .py(px(10.0))
                    .rounded(px(6.0))
                    .font_family(FONT_FAMILY)
                    .text_size(px(13.0))
                    .cursor_pointer()
                    .when(is_active, |el| {
                        el.bg(bg::SELECTED)
                            .text_color(text::BRIGHT)
                            .border_1()
                            .border_color(border::SELECTED)
                    })
                    .when(!is_active, |el| {
                        el.text_color(text::MUTED)
                            .hover(|el| el.bg(bg::HOVER).text_color(text::PRIMARY))
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |view, _event, _window, cx| {
                        view.switch_tab(tab_clone, cx);
                    }))
                    .child(tab.label())
            }))
    }

    fn render_active_tab_content(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.current_tab {
            GymTab::Trajectories => self.trajectory_view.clone().into_any_element(),
            GymTab::TBCC => self.dashboard_view.clone().into_any_element(),
            GymTab::HillClimber => self.hillclimber_view.clone().into_any_element(),
            GymTab::TestGen => self.testgen_view.clone().into_any_element(),
        }
    }

    fn render_sidebar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .when(!self.sidebar_collapsed, |el| el.w(self.sidebar_width))
            .when(self.sidebar_collapsed, |el| el.w(px(0.0)))
            .h_full()
            .bg(bg::SIDEBAR)
            .border_r_1()
            .border_color(border::DEFAULT)
            .when(!self.sidebar_collapsed, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .h_full()
                        .child(
                            // Sidebar header
                            div()
                                .h(px(48.0))
                                .flex()
                                .items_center()
                                .px(px(16.0))
                                .border_b_1()
                                .border_color(border::DEFAULT)
                                .font_family(FONT_FAMILY)
                                .text_size(px(12.0))
                                .text_color(text::MUTED)
                                .child("WORKSPACE")
                        )
                        .child(
                            // Sidebar content - actual tree
                            div()
                                .id("gym-sidebar-scroll")
                                .flex_1()
                                .overflow_y_scroll()
                                .p(px(8.0))
                                .child(self.sidebar.clone())
                        )
                )
            })
    }
}

impl Focusable for GymScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for GymScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Sidebar
            .child(self.render_sidebar(window, cx))
            // Main content area
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .h_full()
                    // Tab bar
                    .child(self.render_tab_bar(window, cx))
                    // Tab content
                    .child(
                        div()
                            .id("gym-tab-content-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .child(self.render_active_tab_content(window, cx))
                    )
            )
    }
}
