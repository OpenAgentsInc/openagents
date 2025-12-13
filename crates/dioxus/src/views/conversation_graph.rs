//! Conversation graph sidebar - visualizes chat as a tree of nodes
//!
//! Each message and tool call is rendered as a clickable node.
//! Tool calls branch off the main conversation flow.

use dioxus::prelude::*;
use mechacoder::{ThreadEntry, ToolStatus};

/// Node types for styling
#[derive(Clone, PartialEq)]
pub enum NodeType {
    User,
    Assistant,
    ToolRunning,
    ToolSuccess,
    ToolError,
}

/// Render data for a single node
#[derive(Clone, PartialEq)]
pub struct NodeData {
    pub id: String,
    pub node_type: NodeType,
    pub x: f64,
    pub y: f64,
    pub parent_id: Option<String>,
}

/// Layout constants
const NODE_SPACING_Y: f64 = 28.0;
const BRANCH_OFFSET_X: f64 = 14.0;
const MAIN_X: f64 = 16.0;

/// Convert ThreadEntry list to positioned nodes
pub fn build_nodes(entries: &[ThreadEntry]) -> Vec<NodeData> {
    let mut nodes = Vec::new();
    let mut y = 20.0;
    let mut current_parent: Option<String> = None;

    for entry in entries {
        match entry {
            ThreadEntry::Message(msg) => {
                let node_type = if msg.role == "user" {
                    NodeType::User
                } else {
                    NodeType::Assistant
                };

                nodes.push(NodeData {
                    id: msg.id.to_string(),
                    node_type,
                    x: MAIN_X,
                    y,
                    parent_id: current_parent.clone(),
                });

                current_parent = Some(msg.id.to_string());
                y += NODE_SPACING_Y;
            }
            ThreadEntry::ToolUse(tool) => {
                let node_type = match tool.status {
                    ToolStatus::Running => NodeType::ToolRunning,
                    ToolStatus::Completed => NodeType::ToolSuccess,
                    ToolStatus::Error => NodeType::ToolError,
                };

                nodes.push(NodeData {
                    id: tool.tool_use_id.clone(),
                    node_type,
                    x: MAIN_X + BRANCH_OFFSET_X,
                    y,
                    parent_id: current_parent.clone(),
                });

                y += NODE_SPACING_Y * 0.6;
            }
        }
    }
    nodes
}

/// Theme colors - grayscale only
mod theme {
    pub const BG_SIDEBAR: &str = "#050505";
    pub const BORDER: &str = "#1A1A1A";
    pub const CONNECTION: &str = "#333333";
    pub const DOT_FILL: &str = "#E6E6E6";      // Filled dots (user)
    pub const DOT_STROKE: &str = "#666666";    // Stroke for hollow dots
}

/// Main conversation graph sidebar component
#[component]
pub fn ConversationGraph(
    entries: Signal<Vec<ThreadEntry>>,
    on_node_click: EventHandler<String>,
) -> Element {
    let nodes = use_memo(move || build_nodes(&entries()));
    let height = (nodes.read().len() as f64 * 28.0 + 40.0).max(100.0);

    rsx! {
        div {
            style: "width: 48px; height: 100%; overflow-y: auto; border-right: 1px solid {theme::BORDER}; background: {theme::BG_SIDEBAR}; flex-shrink: 0;",

            svg {
                width: "100%",
                height: "{height}",
                view_box: "0 0 48 {height}",

                // Draw connections first (below nodes)
                for node in nodes.read().iter() {
                    if let Some(ref parent_id) = node.parent_id {
                        {
                            let parent_opt = nodes.read().iter().find(|n| &n.id == parent_id).cloned();
                            if let Some(parent) = parent_opt {
                                rsx! {
                                    line {
                                        x1: "{parent.x}",
                                        y1: "{parent.y}",
                                        x2: "{node.x}",
                                        y2: "{node.y}",
                                        stroke: theme::CONNECTION,
                                        stroke_width: "1",
                                    }
                                }
                            } else {
                                rsx! {}
                            }
                        }
                    }
                }

                // Draw nodes on top
                for node in nodes.read().iter() {
                    {
                        let node_clone = node.clone();
                        rsx! {
                            ConversationNode {
                                key: "{node_clone.id}",
                                data: node_clone,
                                on_click: move |id| on_node_click.call(id),
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Individual node in the conversation graph
#[component]
fn ConversationNode(data: NodeData, on_click: EventHandler<String>) -> Element {
    let id = data.id.clone();

    // Different styles for different node types:
    // User: filled circle
    // Assistant: hollow circle (stroke only)
    // Tool: smaller hollow circle
    let (radius, fill, stroke, stroke_width) = match data.node_type {
        NodeType::User => ("5", theme::DOT_FILL, "none", "0"),
        NodeType::Assistant => ("5", "none", theme::DOT_FILL, "1.5"),
        NodeType::ToolRunning | NodeType::ToolSuccess | NodeType::ToolError => {
            ("3", "none", theme::DOT_STROKE, "1")
        }
    };

    rsx! {
        circle {
            style: "cursor: pointer;",
            onclick: move |_| on_click.call(id.clone()),
            cx: "{data.x}",
            cy: "{data.y}",
            r: radius,
            fill: fill,
            stroke: stroke,
            stroke_width: stroke_width,
        }
    }
}
