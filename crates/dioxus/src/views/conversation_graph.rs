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
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub parent_id: Option<String>,
}

/// Layout constants
const NODE_SPACING_Y: f64 = 40.0;
const BRANCH_OFFSET_X: f64 = 30.0;
const MAIN_X: f64 = 20.0;

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
                    label: truncate(&msg.content, 12),
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
                    label: truncate(&tool.tool_name, 10),
                    x: MAIN_X + BRANCH_OFFSET_X,
                    y,
                    parent_id: current_parent.clone(),
                });

                y += NODE_SPACING_Y * 0.7;
            }
        }
    }
    nodes
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.lines().next().unwrap_or(s); // First line only
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max).collect::<String>())
    }
}

/// Theme colors matching MechaCoder
mod theme {
    pub const BG_SIDEBAR: &str = "#050505";
    pub const BORDER: &str = "#1A1A1A";
    pub const TEXT_MUTED: &str = "#9E9E9E";
    pub const USER: &str = "#FFB400";
    pub const ASSISTANT: &str = "#E6E6E6";
    pub const TOOL_RUNNING: &str = "#FFB400";
    pub const TOOL_SUCCESS: &str = "#00C853";
    pub const TOOL_ERROR: &str = "#D32F2F";
    pub const CONNECTION: &str = "#333333";
}

/// Main conversation graph sidebar component
#[component]
pub fn ConversationGraph(
    entries: Signal<Vec<ThreadEntry>>,
    on_node_click: EventHandler<String>,
) -> Element {
    let nodes = use_memo(move || build_nodes(&entries()));
    let height = (nodes.read().len() as f64 * 35.0 + 40.0).max(100.0);

    rsx! {
        div {
            style: "width: 140px; height: 100%; overflow-y: auto; border-right: 1px solid {theme::BORDER}; background: {theme::BG_SIDEBAR}; flex-shrink: 0;",

            svg {
                width: "100%",
                height: "{height}",
                view_box: "0 0 140 {height}",

                // Draw connections first (below nodes)
                for node in nodes.read().iter() {
                    if let Some(ref parent_id) = node.parent_id {
                        {
                            let parent_opt = nodes.read().iter().find(|n| &n.id == parent_id).cloned();
                            if let Some(parent) = parent_opt {
                                rsx! {
                                    line {
                                        x1: "{parent.x + 6.0}",
                                        y1: "{parent.y + 6.0}",
                                        x2: "{node.x + 6.0}",
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
    let color = match data.node_type {
        NodeType::User => theme::USER,
        NodeType::Assistant => theme::ASSISTANT,
        NodeType::ToolRunning => theme::TOOL_RUNNING,
        NodeType::ToolSuccess => theme::TOOL_SUCCESS,
        NodeType::ToolError => theme::TOOL_ERROR,
    };

    let id = data.id.clone();

    rsx! {
        g {
            style: "cursor: pointer;",
            onclick: move |_| on_click.call(id.clone()),

            // Node circle
            circle {
                cx: "{data.x + 6.0}",
                cy: "{data.y + 6.0}",
                r: "6",
                fill: color,
            }

            // Node label
            text {
                x: "{data.x + 16.0}",
                y: "{data.y + 10.0}",
                fill: theme::TEXT_MUTED,
                font_size: "9",
                font_family: "'Berkeley Mono', 'JetBrains Mono', monospace",
                "{data.label}"
            }
        }
    }
}
