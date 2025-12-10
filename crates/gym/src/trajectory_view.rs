//! Trajectory viewer component (extracted from main.rs)

use gpui::*;

pub struct TrajectoryView {
    /// Focus handle
    focus_handle: FocusHandle,
}

impl TrajectoryView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for TrajectoryView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TrajectoryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .h_full()
            .child("Trajectory View - Under Construction")
    }
}
