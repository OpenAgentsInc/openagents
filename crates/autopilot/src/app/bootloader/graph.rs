//! Boot graph types for semantic graph visualization.

use wgpui::{CurvePrimitive, Hsla, Point};

use super::card::CardState;
use super::events::BootStage;

/// A node in the boot graph.
#[derive(Clone, Debug)]
pub struct BootNode {
    pub stage: Option<BootStage>,
    pub node_type: BootNodeType,
    pub state: CardState,
    pub position: Point,
    pub radius: f32,
    pub label: String,
}

/// Type of node in the boot graph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BootNodeType {
    /// Central origin node (hollow ring, subtle pulse)
    Origin,
    /// Primary boot process node (solid, glow when active)
    Primary,
    /// Feature/stage node (small, upstream)
    Feature,
    /// Context node (muted, downstream)
    Context,
}

impl BootNode {
    /// Create the origin node.
    pub fn origin(center: Point) -> Self {
        Self {
            stage: None,
            node_type: BootNodeType::Origin,
            state: CardState::Running,
            position: center,
            radius: 12.0,
            label: String::new(),
        }
    }

    /// Create the primary boot node.
    pub fn primary(center: Point) -> Self {
        Self {
            stage: None,
            node_type: BootNodeType::Primary,
            state: CardState::Running,
            position: center,
            radius: 8.0,
            label: "Boot".to_string(),
        }
    }

    /// Create a feature node for a boot stage.
    pub fn feature(stage: BootStage, position: Point) -> Self {
        Self {
            stage: Some(stage),
            node_type: BootNodeType::Feature,
            state: CardState::Pending,
            position,
            radius: 6.0,
            label: stage.name().to_string(),
        }
    }

    /// Create a context node.
    pub fn context(label: &str, position: Point) -> Self {
        Self {
            stage: None,
            node_type: BootNodeType::Context,
            state: CardState::Pending,
            position,
            radius: 4.0,
            label: label.to_string(),
        }
    }

    /// Get the fill color for this node.
    pub fn fill_color(&self) -> Hsla {
        match self.node_type {
            BootNodeType::Origin => Hsla::transparent(),
            BootNodeType::Primary => match self.state {
                CardState::Running => Hsla::new(200.0 / 360.0, 0.6, 0.5, 1.0), // Blue
                CardState::Complete => Hsla::new(120.0 / 360.0, 0.5, 0.4, 1.0), // Green
                CardState::Failed => Hsla::new(0.0, 0.6, 0.5, 1.0),            // Red
                _ => Hsla::new(0.0, 0.0, 0.3, 1.0),                            // Gray
            },
            BootNodeType::Feature => match self.state {
                CardState::Pending => Hsla::transparent(),
                CardState::Running => Hsla::new(200.0 / 360.0, 0.6, 0.5, 1.0), // Blue
                CardState::Complete => Hsla::new(120.0 / 360.0, 0.5, 0.4, 1.0), // Green
                CardState::Failed => Hsla::new(0.0, 0.6, 0.5, 1.0),            // Red
                CardState::Skipped => Hsla::new(0.0, 0.0, 0.25, 0.5),          // Muted
            },
            BootNodeType::Context => Hsla::new(0.0, 0.0, 0.2, 0.5), // Very muted
        }
    }

    /// Get the border color for this node.
    pub fn border_color(&self) -> Hsla {
        match self.node_type {
            BootNodeType::Origin => Hsla::new(0.0, 0.0, 0.4, 0.8), // Subtle gray ring
            _ => match self.state {
                CardState::Running => Hsla::new(200.0 / 360.0, 0.6, 0.6, 1.0), // Bright blue
                CardState::Complete => Hsla::new(120.0 / 360.0, 0.5, 0.5, 1.0), // Green
                CardState::Failed => Hsla::new(0.0, 0.6, 0.6, 1.0),            // Red
                _ => Hsla::new(0.0, 0.0, 0.3, 0.6),                            // Muted gray
            },
        }
    }

    /// Whether this node should have a glow effect.
    pub fn has_glow(&self) -> bool {
        self.state == CardState::Running && self.node_type != BootNodeType::Context
    }
}

/// An edge in the boot graph (bezier curve).
#[derive(Clone, Debug)]
pub struct BootEdge {
    pub from: Point,
    pub to: Point,
    pub state: CardState,
}

impl BootEdge {
    pub fn new(from: Point, to: Point) -> Self {
        Self {
            from,
            to,
            state: CardState::Pending,
        }
    }

    /// Create a bezier curve for this edge.
    /// Control points are calculated to create a smooth arc.
    pub fn to_curve(&self, stroke_width: f32) -> CurvePrimitive {
        let dx = self.to.x - self.from.x;
        let dy = self.to.y - self.from.y;

        // Perpendicular offset for the curve bow
        let len = (dx * dx + dy * dy).sqrt();
        let bow_factor = len * 0.15; // 15% of length as bow

        // Perpendicular direction
        let perp_x = -dy / len * bow_factor;
        let perp_y = dx / len * bow_factor;

        let control1 = Point::new(
            self.from.x + dx * 0.25 + perp_x,
            self.from.y + dy * 0.25 + perp_y,
        );
        let control2 = Point::new(
            self.from.x + dx * 0.75 + perp_x,
            self.from.y + dy * 0.75 + perp_y,
        );

        CurvePrimitive::new(self.from, control1, control2, self.to)
            .with_stroke_width(stroke_width)
            .with_color(self.edge_color())
    }

    /// Get the color for this edge based on state.
    pub fn edge_color(&self) -> Hsla {
        match self.state {
            CardState::Complete => Hsla::new(0.0, 0.0, 0.5, 1.0), // 50% gray, full alpha
            CardState::Running => Hsla::new(200.0 / 360.0, 0.6, 0.5, 1.0), // Blue, full alpha
            _ => Hsla::new(0.0, 0.0, 0.35, 0.9),                  // 35% gray, 90% alpha
        }
    }
}

/// Calculate positions for boot nodes in a radial layout.
pub fn calculate_radial_layout(
    center_x: f32,
    center_y: f32,
    inner_radius: f32,
    outer_radius: f32,
) -> BootGraphLayout {
    use std::f32::consts::PI;

    // Origin at center
    let origin = BootNode::origin(Point::new(center_x, center_y));

    // Primary node slightly offset from origin
    let primary = BootNode::primary(Point::new(center_x + inner_radius * 0.5, center_y));

    // Stage nodes arranged radially
    // Angles in degrees, converted to radians
    let stage_angles = [
        (BootStage::Hardware, 315.0_f32), // Top-right
        (BootStage::Compute, 350.0),
        (BootStage::Network, 25.0),
        (BootStage::Identity, 60.0),
        (BootStage::Workspace, 95.0),
        (BootStage::Summary, 225.0), // Bottom-left (context)
    ];

    let mut stage_nodes = Vec::new();
    for (stage, angle_deg) in stage_angles {
        let angle = angle_deg * PI / 180.0;
        let x = center_x + outer_radius * angle.cos();
        let y = center_y - outer_radius * angle.sin(); // Negative because Y is flipped

        let mut node = if stage == BootStage::Summary {
            BootNode::context(stage.name(), Point::new(x, y))
        } else {
            BootNode::feature(stage, Point::new(x, y))
        };
        node.stage = Some(stage);
        stage_nodes.push(node);
    }

    // Create edges from primary to each stage
    let mut edges = Vec::new();
    for node in &stage_nodes {
        edges.push(BootEdge::new(primary.position, node.position));
    }

    BootGraphLayout {
        origin,
        primary,
        stage_nodes,
        edges,
    }
}

/// The complete boot graph layout.
pub struct BootGraphLayout {
    pub origin: BootNode,
    pub primary: BootNode,
    pub stage_nodes: Vec<BootNode>,
    pub edges: Vec<BootEdge>,
}

impl BootGraphLayout {
    /// Update node state for a stage.
    pub fn update_stage_state(&mut self, stage: BootStage, state: CardState) {
        for node in &mut self.stage_nodes {
            if node.stage == Some(stage) {
                node.state = state;
            }
        }

        // Update edge state to match
        for (i, node) in self.stage_nodes.iter().enumerate() {
            if node.stage == Some(stage) {
                if let Some(edge) = self.edges.get_mut(i) {
                    edge.state = state;
                }
            }
        }
    }

    /// Check if all stages are complete.
    pub fn all_complete(&self) -> bool {
        self.stage_nodes
            .iter()
            .all(|n| n.state == CardState::Complete || n.state == CardState::Skipped)
    }

    /// Update primary node state based on overall progress.
    pub fn update_primary_state(&mut self) {
        if self.all_complete() {
            self.primary.state = CardState::Complete;
        } else if self
            .stage_nodes
            .iter()
            .any(|n| n.state == CardState::Running)
        {
            self.primary.state = CardState::Running;
        }
    }
}
