//! Graph - node-edge graph layout

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

use crate::grammar::{Edge, Node, NodeId, Topology, VizPrimitive};

/// A node-edge graph visualization
pub struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    highlighted: Vec<NodeId>,
    node_color: Hsla,
    highlight_color: Hsla,
    edge_color: Hsla,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            highlighted: Vec::new(),
            node_color: Hsla::new(0.0, 0.0, 0.3, 1.0),
            highlight_color: Hsla::new(200.0 / 360.0, 0.8, 0.5, 1.0),
            edge_color: Hsla::new(0.0, 0.0, 0.4, 1.0),
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
        // Draw edges first (as lines approximated with thin quads)
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

                // Draw line as thin rectangle aligned to edge direction
                let dx = to_pt.x - from_pt.x;
                let dy = to_pt.y - from_pt.y;
                let len = (dx * dx + dy * dy).sqrt();

                if len > 0.1 {
                    let edge_alpha = 0.3 + 0.7 * edge.weight.min(1.0);
                    let edge_color = Hsla::new(
                        self.edge_color.h,
                        self.edge_color.s,
                        self.edge_color.l,
                        edge_alpha,
                    );

                    // Use small quads along the edge
                    let steps = (len / 4.0).max(2.0) as i32;
                    for i in 0..steps {
                        let t = i as f32 / steps as f32;
                        let px = from_pt.x + dx * t;
                        let py = from_pt.y + dy * t;

                        let seg_bounds = Bounds {
                            origin: Point { x: px - 1.5, y: py - 1.5 },
                            size: Size { width: 3.0, height: 3.0 },
                        };
                        cx.scene.draw_quad(Quad::new(seg_bounds).with_background(edge_color));
                    }
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

            let size = node.size * if is_highlighted { 1.3 } else { 1.0 };

            // Draw as square (could use corner_radius for circle)
            let node_bounds = Bounds {
                origin: Point {
                    x: center.x - size / 2.0,
                    y: center.y - size / 2.0,
                },
                size: Size {
                    width: size,
                    height: size,
                },
            };

            cx.scene.draw_quad(
                Quad::new(node_bounds)
                    .with_background(color)
                    .with_corner_radius(size / 2.0)
            );
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
