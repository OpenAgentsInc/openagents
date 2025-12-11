//! Expandable tree sidebar for the Gym

use gpui_oa::prelude::*;
use gpui_oa::*;
use theme_oa::{bg, border, text, status, FONT_FAMILY};

use super::types::{TreeNode, TreeItemKind, ItemStatus, SidebarState};

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
        // Create initial tree structure with sample data
        let nodes = vec![
            TreeNode::Category {
                id: "sessions".to_string(),
                label: "Sessions".to_string(),
                icon: "📁",
                children: vec![
                    TreeNode::Item {
                        id: "session-1".to_string(),
                        kind: TreeItemKind::Session,
                        label: "Session abc123".to_string(),
                        metadata: "3 trajectories".to_string(),
                        status: ItemStatus::Idle,
                    },
                ],
            },
            TreeNode::Category {
                id: "hillclimber".to_string(),
                label: "HillClimber Runs".to_string(),
                icon: "🔬",
                children: vec![
                    TreeNode::Item {
                        id: "hc-1".to_string(),
                        kind: TreeItemKind::HillClimberRun,
                        label: "regex-log #1".to_string(),
                        metadata: "100% pass".to_string(),
                        status: ItemStatus::Success,
                    },
                    TreeNode::Item {
                        id: "hc-2".to_string(),
                        kind: TreeItemKind::HillClimberRun,
                        label: "regex-log #2".to_string(),
                        metadata: "Turn 8/15".to_string(),
                        status: ItemStatus::Running { progress: 0.67 },
                    },
                ],
            },
            TreeNode::Category {
                id: "testgen".to_string(),
                label: "TestGen Suites".to_string(),
                icon: "🧪",
                children: vec![
                    TreeNode::Item {
                        id: "tg-1".to_string(),
                        kind: TreeItemKind::TestGenSuite,
                        label: "regex-log suite".to_string(),
                        metadata: "20 tests".to_string(),
                        status: ItemStatus::Idle,
                    },
                ],
            },
        ];

        let mut state = SidebarState::new();
        // Expand sessions by default
        state.expanded.insert("sessions".to_string());

        Self {
            nodes,
            state,
            focus_handle: cx.focus_handle(),
        }
    }

    fn toggle_category(&mut self, id: String, cx: &mut Context<Self>) {
        self.state.toggle_expand(&id);
        cx.notify();
    }

    fn select_item(&mut self, id: String, cx: &mut Context<Self>) {
        self.state.select(id);
        cx.notify();
    }

    fn render_node(&self, node: &TreeNode, depth: usize, cx: &mut Context<Self>) -> AnyElement {
        match node {
            TreeNode::Category { id, label, icon, children } => {
                let is_expanded = self.state.is_expanded(id);
                let id_clone = id.clone();

                div()
                    .flex()
                    .flex_col()
                    .child(
                        // Category header
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .px(px(8.0 + (depth as f32 * 12.0)))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|el| el.bg(bg::HOVER))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |view, _event, _window, cx| {
                                view.toggle_category(id_clone.clone(), cx);
                            }))
                            .child(
                                // Chevron icon
                                div()
                                    .text_size(px(10.0))
                                    .text_color(text::MUTED)
                                    .child(if is_expanded { "▼" } else { "▶" })
                            )
                            .child(
                                // Category icon
                                div()
                                    .text_size(px(14.0))
                                    .child(*icon)
                            )
                            .child(
                                // Category label
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_size(px(12.0))
                                    .text_color(text::PRIMARY)
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(label.clone())
                            )
                    )
                    .when(is_expanded, |el| {
                        el.children(children.iter().map(|child| {
                            self.render_node(child, depth + 1, cx)
                        }))
                    })
                    .into_any_element()
            }
            TreeNode::Item { id, kind: _, label, metadata, status } => {
                let is_selected = self.state.is_selected(id);
                let id_clone = id.clone();

                let (status_color, status_icon) = match status {
                    ItemStatus::Idle => (text::MUTED, "○"),
                    ItemStatus::Running { .. } => (status::RUNNING, "●"),
                    ItemStatus::Success => (status::SUCCESS, "✓"),
                    ItemStatus::Failed => (status::ERROR, "✗"),
                    ItemStatus::Partial { .. } => (status::WARNING, "◐"),
                };

                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0 + (depth as f32 * 12.0)))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .when(is_selected, |el| {
                        el.bg(bg::SELECTED)
                            .border_1()
                            .border_color(border::SELECTED)
                    })
                    .when(!is_selected, |el| {
                        el.hover(|el| el.bg(bg::HOVER))
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |view, _event, _window, cx| {
                        view.select_item(id_clone.clone(), cx);
                    }))
                    .child(
                        // Spacer for alignment with category items
                        div()
                            .w(px(16.0))
                    )
                    .child(
                        // Status indicator
                        div()
                            .text_size(px(10.0))
                            .text_color(status_color)
                            .child(status_icon)
                    )
                    .child(
                        // Item content
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                // Item label
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_size(px(12.0))
                                    .text_color(if is_selected { text::BRIGHT } else { text::PRIMARY })
                                    .child(label.clone())
                            )
                            .child(
                                // Item metadata
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_size(px(10.0))
                                    .text_color(text::MUTED)
                                    .child(metadata.clone())
                            )
                    )
                    .into_any_element()
            }
        }
    }
}

impl Focusable for Sidebar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .children(self.nodes.iter().map(|node| {
                self.render_node(node, 0, cx)
            }))
    }
}
