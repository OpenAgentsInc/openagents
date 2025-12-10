//! HillClimber controls (Start/Stop, mode selector, session dropdown)

use gpui::*;

pub struct HCControls {
    focus_handle: FocusHandle,
}

impl HCControls {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for HCControls {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HCControls {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child("HC Controls - Under Construction")
    }
}
