//! Category progress bars (anti_cheat, existence, correctness, boundary, integration)

use gpui::*;

pub struct CategoryProgress {
    focus_handle: FocusHandle,
}

impl CategoryProgress {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for CategoryProgress {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CategoryProgress {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child("Category Progress - Under Construction")
    }
}
