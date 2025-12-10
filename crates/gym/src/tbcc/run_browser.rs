//! TBCC Run Browser Tab

use gpui::*;

pub struct RunBrowserView {
    focus_handle: FocusHandle,
}

impl RunBrowserView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for RunBrowserView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RunBrowserView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .child("TBCC Run Browser - Under Construction")
    }
}
