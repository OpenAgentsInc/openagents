//! TBCC Dashboard Tab - KPIs, recent runs, quick actions

use gpui::*;

use super::types::DashboardStats;

pub struct DashboardView {
    stats: Option<DashboardStats>,
    loading: bool,
    focus_handle: FocusHandle,
}

impl DashboardView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            stats: None,
            loading: false,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for DashboardView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DashboardView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .child("TBCC Dashboard - Under Construction")
    }
}
