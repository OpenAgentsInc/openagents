//! Main Gym Screen component
//!
//! Multi-view container with tab navigation. Each tab manages its own layout.

use gpui::prelude::*;
use gpui::*;
use std::sync::{Arc, Mutex};
use atif_store::TrajectoryStore;
use theme::{bg, border, text, FONT_FAMILY};

use super::types::GymTab;
use super::trajectory_view::TrajectoryView;
use super::tbcc::TBCCScreen;
use super::hillclimber::monitor::HillClimberMonitor;
use super::testgen::visualizer::TestGenVisualizer;
use super::regex_crusade::RegexCrusadeScreen;
use super::actions::*;

pub struct GymScreen {
    /// Current active tab
    pub current_tab: GymTab,

    /// Focus handle
    focus_handle: FocusHandle,

    /// View entities
    trajectory_view: Entity<TrajectoryView>,
    tbcc_screen: Entity<TBCCScreen>,
    hillclimber_view: Entity<HillClimberMonitor>,
    testgen_view: Entity<TestGenVisualizer>,
    regex_crusade_view: Entity<RegexCrusadeScreen>,
}

impl GymScreen {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::with_store(cx, None)
    }

    pub fn with_store(cx: &mut Context<Self>, store: Option<Arc<Mutex<TrajectoryStore>>>) -> Self {
        let trajectory_view = cx.new(|cx| {
            let mut view = TrajectoryView::new(cx);
            if let Some(ref s) = store {
                view.set_store(s.clone(), cx);
            }
            view
        });
        let tbcc_screen = cx.new(|cx| TBCCScreen::new(cx));
        let hillclimber_view = cx.new(|cx| HillClimberMonitor::new(cx));
        let testgen_view = cx.new(|cx| TestGenVisualizer::new(cx));
        let regex_crusade_view = cx.new(|cx| RegexCrusadeScreen::new(cx));

        Self {
            current_tab: GymTab::default(),
            focus_handle: cx.focus_handle(),
            trajectory_view,
            tbcc_screen,
            hillclimber_view,
            testgen_view,
            regex_crusade_view,
        }
    }

    pub fn switch_tab(&mut self, tab: GymTab, cx: &mut Context<Self>) {
        self.current_tab = tab;
        cx.notify();
    }

    // Action handlers

    fn switch_to_trajectories(&mut self, _: &SwitchToTrajectories, _window: &mut Window, cx: &mut Context<Self>) {
        self.switch_tab(GymTab::Trajectories, cx);
    }

    fn switch_to_tbcc(&mut self, _: &SwitchToTBCC, _window: &mut Window, cx: &mut Context<Self>) {
        self.switch_tab(GymTab::TBCC, cx);
    }

    fn switch_to_hillclimber(&mut self, _: &SwitchToHillClimber, _window: &mut Window, cx: &mut Context<Self>) {
        self.switch_tab(GymTab::HillClimber, cx);
    }

    fn switch_to_testgen(&mut self, _: &SwitchToTestGen, _window: &mut Window, cx: &mut Context<Self>) {
        self.switch_tab(GymTab::TestGen, cx);
    }

    fn switch_to_regex_crusade(&mut self, _: &SwitchToRegexCrusade, _window: &mut Window, cx: &mut Context<Self>) {
        self.switch_tab(GymTab::RegexCrusade, cx);
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

    fn render_active_tab_content(&self, _window: &mut Window, _cx: &mut Context<Self>) -> AnyElement {
        match self.current_tab {
            GymTab::Trajectories => self.trajectory_view.clone().into_any_element(),
            GymTab::TBCC => self.tbcc_screen.clone().into_any_element(),
            GymTab::HillClimber => self.hillclimber_view.clone().into_any_element(),
            GymTab::TestGen => self.testgen_view.clone().into_any_element(),
            GymTab::RegexCrusade => self.regex_crusade_view.clone().into_any_element(),
        }
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
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Register action handlers
            .on_action(cx.listener(Self::switch_to_trajectories))
            .on_action(cx.listener(Self::switch_to_tbcc))
            .on_action(cx.listener(Self::switch_to_hillclimber))
            .on_action(cx.listener(Self::switch_to_testgen))
            .on_action(cx.listener(Self::switch_to_regex_crusade))
            // Tab bar
            .child(self.render_tab_bar(window, cx))
            // Tab content - each tab manages its own layout
            .child(
                div()
                    .id("gym-tab-content")
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_active_tab_content(window, cx))
            )
    }
}
