//! Graph - node-edge graph layout

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Color, Point};

use crate::grammar::{Edge, Node, NodeId, Topology, VizPrimitive};

/// A node-edge graph visualization
pub struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    highlighted: Vec<NodeId>,
    node_color: Color,
    highlight_color: Color,
    edge_color: Color,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            highlighted: Vec::new(),
            node_color: Color::from_rgba(0.3, 0.3, 0.3, 1.0),
            highlight_color: Color::from_rgba(0.0, 0.8, 1.0, 1.0),
            edge_color: Color::from_rgba(0.4, 0.4, 0.4, 1.0),
        }
    }

    fn find_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Graph {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw edges first
        for edge in &self.edges {
            if let (Some(from), Some(to)) = (self.find_node(edge.from), self.find_node(edge.to)) {
                let from_pt = Point {
                    x: bounds.origin.x + from.position.x * bounds.size.width,
                    y: bounds.origin.y + from.position.y * bounds.size.height,
                };
                let to_pt = Point {
                    x: bounds.origin.x + to.position.x * bounds.size.width,
                    y: bounds.origin.y + to.position.y * bounds.size.height,
                };

                // Draw line as thin rectangle
                let dx = to_pt.x - from_pt.x;
                let dy = to_pt.y - from_pt.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.1 {
                    let nx = -dy / len * 1.5;
                    let ny = dx / len * 1.5;

                    let p1 = Point { x: from_pt.x + nx, y: from_pt.y + ny };
                    let p2 = Point { x: from_pt.x - nx, y: from_pt.y - ny };
                    let p3 = Point { x: to_pt.x - nx, y: to_pt.y - ny };
                    let p4 = Point { x: to_pt.x + nx, y: to_pt.y + ny };

                    let edge_alpha = 0.3 + 0.7 * edge.weight.min(1.0);
                    let edge_color = Color::from_rgba(
                        self.edge_color.r,
                        self.edge_color.g,
                        self.edge_color.b,
                        edge_alpha,
                    );

                    cx.scene.fill_triangle(p1, p2, p3, edge_color);
                    cx.scene.fill_triangle(p1, p3, p4, edge_color);
                }
            }
        }

        // Draw nodes
        for node in &self.nodes {
            let center = Point {
                x: bounds.origin.x + node.position.x * bounds.size.width,
                y: bounds.origin.y + node.position.y * bounds.size.height,
            };

            let is_highlighted = self.highlighted.contains(&node.id);
            let color = if is_highlighted {
                self.highlight_color
            } else {
                self.node_color
            };

            let radius = node.size * if is_highlighted { 1.3 } else { 1.0 };

            // Draw as circle approximation
            let segments = 16;
            for i in 0..segments {
                let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

                let p0 = Point {
                    x: center.x + radius * a0.cos(),
                    y: center.y + radius * a0.sin(),
                };
                let p1 = Point {
                    x: center.x + radius * a1.cos(),
                    y: center.y + radius * a1.sin(),
                };

                cx.scene.fill_triangle(center, p0, p1, color);
            }
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(200.0), Some(150.0))
    }
}

impl VizPrimitive for Graph {
    fn update(&mut self, _value: f32) {}

    fn animate_to(&mut self, _value: f32, _duration_ms: u32) {}
}

impl Topology for Graph {
    fn set_nodes(&mut self, nodes: &[Node]) {
        self.nodes = nodes.to_vec();
    }

    fn set_edges(&mut self, edges: &[Edge]) {
        self.edges = edges.to_vec();
    }

    fn highlight(&mut self, ids: &[NodeId]) {
        self.highlighted = ids.to_vec();
    }
}
