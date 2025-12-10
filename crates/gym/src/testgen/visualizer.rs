//! TestGen Visualizer - Main TestGen view

use gpui::*;

pub struct TestGenVisualizer {
    focus_handle: FocusHandle,
}

impl TestGenVisualizer {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for TestGenVisualizer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TestGenVisualizer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .child("TestGen Visualizer - Under Construction")
    }
}
