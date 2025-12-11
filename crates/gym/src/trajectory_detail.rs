//! Reusable trajectory step renderer

use gpui_oa::*;

pub struct TrajectoryDetail {
    /// Focus handle
    focus_handle: FocusHandle,
}

impl TrajectoryDetail {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for TrajectoryDetail {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TrajectoryDetail {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child("Trajectory Detail - Under Construction")
    }
}
