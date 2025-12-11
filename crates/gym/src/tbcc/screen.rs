//! TBCC Screen - Main container with 4 sub-tabs and data service integration

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use super::types::TBCCTab;
use super::dashboard::DashboardView;
use super::task_browser::TaskBrowserView;
use super::run_browser::RunBrowserView;
use super::settings::SettingsView;
use crate::services::TBCCDataService;

pub struct TBCCScreen {
    pub current_tab: TBCCTab,
    #[allow(dead_code)]
    data_service: TBCCDataService,
    dashboard_view: Entity<DashboardView>,
    task_browser_view: Entity<TaskBrowserView>,
    run_browser_view: Entity<RunBrowserView>,
    settings_view: Entity<SettingsView>,
    focus_handle: FocusHandle,
}

impl TBCCScreen {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let data_service = TBCCDataService::new();
        let run_store = data_service.runs();
        let _settings_store_path = data_service.settings().load();

        // Create views
        let dashboard_view = cx.new(|cx| {
            let mut view = DashboardView::new(cx);
            view.set_run_store(run_store.clone(), cx);
            view
        });

        let task_browser_view = cx.new(|cx| TaskBrowserView::new(cx));

        let run_browser_view = cx.new(|cx| {
            let mut view = RunBrowserView::new(cx);
            view.set_run_store(run_store.clone(), cx);
            view
        });

        let settings_view = cx.new(|cx| {
            let view = SettingsView::new(cx);
            // Note: SettingsStore is not Clone, so we create a new one
            // In a real app, we'd use Arc<RwLock<SettingsStore>> or similar
            view
        });

        Self {
            current_tab: TBCCTab::Dashboard,
            data_service,
            dashboard_view,
            task_browser_view,
            run_browser_view,
            settings_view,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Switch to a specific TBCC tab
    pub fn switch_tab(&mut self, tab: TBCCTab, cx: &mut Context<Self>) {
        self.current_tab = tab;
        cx.notify();
    }

    fn render_tab_button(&self, tab: TBCCTab, label: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = self.current_tab == tab;
        let label = label.to_string();

        div()
            .px(px(16.0))
            .py(px(10.0))
            .cursor_pointer()
            .text_size(px(13.0))
            .font_family(FONT_FAMILY)
            .when(is_active, |el| {
                el.text_color(text::BRIGHT)
                    .border_b_2()
                    .border_color(status::INFO)
            })
            .when(!is_active, |el| {
                el.text_color(text::MUTED)
                    .hover(|el| el.text_color(text::PRIMARY))
            })
            .on_mouse_down(MouseButton::Left, cx.listener(move |view, _event, _window, cx| {
                view.switch_tab(tab, cx);
            }))
            .child(label)
    }

    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
            .child(self.render_tab_button(TBCCTab::Dashboard, "Dashboard", cx))
            .child(self.render_tab_button(TBCCTab::Tasks, "Tasks", cx))
            .child(self.render_tab_button(TBCCTab::Runs, "Runs", cx))
            .child(self.render_tab_button(TBCCTab::Settings, "Settings", cx))
    }

    fn render_content(&self) -> AnyElement {
        match self.current_tab {
            TBCCTab::Dashboard => self.dashboard_view.clone().into_any_element(),
            TBCCTab::Tasks => self.task_browser_view.clone().into_any_element(),
            TBCCTab::Runs => self.run_browser_view.clone().into_any_element(),
            TBCCTab::Settings => self.settings_view.clone().into_any_element(),
        }
    }
}

impl Focusable for TBCCScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TBCCScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Tab bar
            .child(self.render_tab_bar(cx))
            // Content area
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_content())
            )
    }
}
