//! Test results display (X/Y passed, failed test names)

use gpui::*;

pub struct TestResults {
    focus_handle: FocusHandle,
}

impl TestResults {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for TestResults {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TestResults {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child("Test Results - Under Construction")
    }
}
