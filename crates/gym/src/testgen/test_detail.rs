//! Test code viewer with syntax highlighting

use gpui::*;

pub struct TestDetail {
    focus_handle: FocusHandle,
}

impl TestDetail {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for TestDetail {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TestDetail {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child("Test Detail - Under Construction")
    }
}
