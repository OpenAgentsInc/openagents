//! Expandable tree sidebar for the Gym

use gpui::*;

use super::types::{TreeNode, SidebarState};

pub struct Sidebar {
    /// Tree nodes
    nodes: Vec<TreeNode>,

    /// Sidebar state (expansion, selection)
    state: SidebarState,

    /// Focus handle
    focus_handle: FocusHandle,
}

impl Sidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            nodes: Vec::new(),
            state: SidebarState::new(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for Sidebar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(260.0))
            .h_full()
            .child("Sidebar - Under Construction")
    }
}
