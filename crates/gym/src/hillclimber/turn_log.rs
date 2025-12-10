//! Turn-by-turn action log

use gpui::*;

pub struct TurnLog {
    focus_handle: FocusHandle,
}

impl TurnLog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for TurnLog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TurnLog {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child("Turn Log - Under Construction")
    }
}
