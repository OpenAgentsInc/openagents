//! HillClimber Monitor - Main HC visualization view

use gpui::*;

pub struct HillClimberMonitor {
    focus_handle: FocusHandle,
}

impl HillClimberMonitor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for HillClimberMonitor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HillClimberMonitor {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .child("HillClimber Monitor - Under Construction")
    }
}
