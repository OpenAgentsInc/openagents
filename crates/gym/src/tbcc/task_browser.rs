//! TBCC Task Browser Tab

use gpui::*;

pub struct TaskBrowserView {
    focus_handle: FocusHandle,
}

impl TaskBrowserView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for TaskBrowserView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TaskBrowserView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .child("TBCC Task Browser - Under Construction")
    }
}
