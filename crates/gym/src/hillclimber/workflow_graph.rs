//! Workflow graph visualization (TestGen→Decomposer→FM→Verifier→Results)

use gpui::*;

pub struct WorkflowGraph {
    focus_handle: FocusHandle,
}

impl WorkflowGraph {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for WorkflowGraph {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for WorkflowGraph {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child("Workflow Graph - Under Construction")
    }
}
