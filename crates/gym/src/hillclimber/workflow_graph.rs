//! Workflow graph visualization (TestGen -> Decomposer -> FM -> Verifier -> Results)

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

/// Node types in the workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Task,
    TestGen,
    Decomposer,
    FM,
    Verifier,
    Results,
}

impl NodeKind {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Task => "T",
            Self::TestGen => "TG",
            Self::Decomposer => "D",
            Self::FM => "FM",
            Self::Verifier => "V",
            Self::Results => "R",
        }
    }
}

/// Node status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeStatus {
    #[default]
    Waiting,
    Active,
    Completed,
    Failed,
}

/// A node in the workflow graph
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub status: NodeStatus,
    pub position: (f32, f32),
}

/// Workflow graph visualization component
pub struct WorkflowGraph {
    nodes: Vec<GraphNode>,
    focus_handle: FocusHandle,
}

impl WorkflowGraph {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            nodes: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_nodes(&mut self, nodes: Vec<GraphNode>) {
        self.nodes = nodes;
    }

    fn render_node(&self, node: &GraphNode) -> impl IntoElement {
        let (bg_color, border_color, text_color) = match node.status {
            NodeStatus::Waiting => (bg::ELEVATED, border::DEFAULT, text::MUTED),
            NodeStatus::Active => (status::INFO_BG, status::INFO, text::PRIMARY),
            NodeStatus::Completed => (status::SUCCESS_BG, status::SUCCESS.opacity(0.5), status::SUCCESS),
            NodeStatus::Failed => (status::ERROR_BG, status::ERROR.opacity(0.5), status::ERROR),
        };

        let label = node.label.clone();
        let kind_icon = node.kind.icon().to_string();
        let is_active = node.status == NodeStatus::Active;

        div()
            .absolute()
            .left(px(node.position.0))
            .top(px(node.position.1))
            .w(px(100.0))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.0))
            // Node circle
            .child(
                div()
                    .w(px(48.0))
                    .h(px(48.0))
                    .rounded_full()
                    .bg(bg_color)
                    .border_2()
                    .border_color(border_color)
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(is_active, |el| {
                        el.shadow_md()
                    })
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text_color)
                            .font_weight(FontWeight::BOLD)
                            .child(kind_icon)
                    )
            )
            // Label
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::PRIMARY)
                    .text_center()
                    .child(label)
            )
    }

    fn render_connection(&self, from: (f32, f32), to: (f32, f32)) -> impl IntoElement {
        // Simple horizontal line connection
        let start_x = from.0 + 100.0; // Start from right of node
        let end_x = to.0;
        let y = from.1 + 24.0; // Center of node

        div()
            .absolute()
            .left(px(start_x))
            .top(px(y))
            .w(px(end_x - start_x))
            .h(px(2.0))
            .bg(border::DEFAULT)
    }
}

impl Focusable for WorkflowGraph {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for WorkflowGraph {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Build connections between nodes
        let connections: Vec<_> = self.nodes.windows(2).map(|pair| {
            (pair[0].position, pair[1].position)
        }).collect();

        div()
            .relative()
            .h_full()
            .w_full()
            .bg(bg::APP)
            .p(px(20.0))
            // Render connections first (behind nodes)
            .children(connections.iter().map(|(from, to)| {
                self.render_connection(*from, *to)
            }))
            // Render nodes
            .children(self.nodes.iter().map(|node| {
                self.render_node(node)
            }))
            // Legend
            .child(
                div()
                    .absolute()
                    .bottom(px(16.0))
                    .left(px(16.0))
                    .flex()
                    .gap(px(16.0))
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .w(px(10.0))
                                    .h(px(10.0))
                                    .rounded_full()
                                    .bg(bg::ELEVATED)
                                    .border_1()
                                    .border_color(border::DEFAULT)
                            )
                            .child("Waiting")
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .w(px(10.0))
                                    .h(px(10.0))
                                    .rounded_full()
                                    .bg(status::INFO_BG)
                                    .border_1()
                                    .border_color(status::INFO)
                            )
                            .child("Active")
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .w(px(10.0))
                                    .h(px(10.0))
                                    .rounded_full()
                                    .bg(status::SUCCESS_BG)
                                    .border_1()
                                    .border_color(status::SUCCESS.opacity(0.5))
                            )
                            .child("Completed")
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .w(px(10.0))
                                    .h(px(10.0))
                                    .rounded_full()
                                    .bg(status::ERROR_BG)
                                    .border_1()
                                    .border_color(status::ERROR.opacity(0.5))
                            )
                            .child("Failed")
                    )
            )
    }
}
