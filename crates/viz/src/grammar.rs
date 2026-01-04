//! Core visual grammar traits for the 5 visual verbs.
//!
//! - **Fill**: "how much" - capacity, progress, levels
//! - **Pulse**: "something happened" - discrete events
//! - **Flow**: "data moving" - streaming, arcs
//! - **Heat**: "intensity/importance" - heatmaps, intensity
//! - **Topology**: "structure/connections" - graphs, trees

use wgpui::components::Component;
use wgpui::{Bounds, Point};

/// Base trait for all visualization primitives
pub trait VizPrimitive: Component {
    /// Update the current value
    fn update(&mut self, value: f32);

    /// Animate to a target value over duration
    fn animate_to(&mut self, value: f32, duration_ms: u32);
}

/// Fill: capacity, progress, levels ("how much")
pub trait Fill: VizPrimitive {
    /// Set the value range
    fn set_range(&mut self, min: f32, max: f32);

    /// Set warning and critical thresholds for visual indication
    fn set_thresholds(&mut self, warning: f32, critical: f32);
}

/// Pulse: discrete events ("something happened")
pub trait Pulse: VizPrimitive {
    /// Trigger a pulse event
    fn trigger(&mut self);

    /// Set decay time in milliseconds
    fn set_decay(&mut self, decay_ms: u32);
}

/// Flow: movement between points ("data moving")
pub trait Flow: VizPrimitive {
    /// Set the source point
    fn set_source(&mut self, point: Point);

    /// Set the target point
    fn set_target(&mut self, point: Point);

    /// Set throughput value (affects animation speed/density)
    fn set_throughput(&mut self, value: f32);
}

/// Color palette for heat visualization
#[derive(Clone, Debug)]
pub struct Palette {
    pub colors: Vec<[f32; 4]>,
}

impl Palette {
    pub fn viridis() -> Self {
        Self {
            colors: vec![
                [0.267, 0.004, 0.329, 1.0], // dark purple
                [0.282, 0.140, 0.458, 1.0],
                [0.254, 0.265, 0.530, 1.0],
                [0.207, 0.372, 0.553, 1.0],
                [0.164, 0.471, 0.558, 1.0],
                [0.128, 0.567, 0.551, 1.0],
                [0.134, 0.659, 0.518, 1.0],
                [0.267, 0.749, 0.441, 1.0],
                [0.478, 0.821, 0.318, 1.0],
                [0.741, 0.873, 0.150, 1.0],
                [0.993, 0.906, 0.144, 1.0], // yellow
            ],
        }
    }

    pub fn inferno() -> Self {
        Self {
            colors: vec![
                [0.001, 0.000, 0.014, 1.0], // black
                [0.133, 0.047, 0.224, 1.0],
                [0.341, 0.063, 0.314, 1.0],
                [0.533, 0.134, 0.280, 1.0],
                [0.702, 0.212, 0.212, 1.0],
                [0.847, 0.343, 0.110, 1.0],
                [0.945, 0.518, 0.035, 1.0],
                [0.988, 0.710, 0.165, 1.0],
                [0.988, 0.878, 0.545, 1.0],
                [0.988, 1.000, 0.644, 1.0], // bright yellow
            ],
        }
    }

    /// Sample the palette at a normalized position [0, 1]
    pub fn sample(&self, t: f32) -> [f32; 4] {
        let t = t.clamp(0.0, 1.0);
        if self.colors.is_empty() {
            return [1.0, 1.0, 1.0, 1.0];
        }
        if self.colors.len() == 1 {
            return self.colors[0];
        }

        let scaled = t * (self.colors.len() - 1) as f32;
        let idx = scaled.floor() as usize;
        let frac = scaled.fract();

        if idx >= self.colors.len() - 1 {
            return self.colors[self.colors.len() - 1];
        }

        let c0 = self.colors[idx];
        let c1 = self.colors[idx + 1];

        [
            c0[0] + (c1[0] - c0[0]) * frac,
            c0[1] + (c1[1] - c0[1]) * frac,
            c0[2] + (c1[2] - c0[2]) * frac,
            c0[3] + (c1[3] - c0[3]) * frac,
        ]
    }
}

/// Heat: intensity visualization ("importance")
pub trait Heat: VizPrimitive {
    /// Set the data values
    fn set_data(&mut self, data: &[f32]);

    /// Set the color palette
    fn set_palette(&mut self, palette: Palette);
}

/// Node in a topology graph
#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub label: String,
    pub position: Point,
    pub size: f32,
}

/// Node identifier
pub type NodeId = u64;

/// Edge between nodes
#[derive(Clone, Debug)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub weight: f32,
}

/// Topology: structural relationships ("connections")
pub trait Topology: VizPrimitive {
    /// Set the nodes
    fn set_nodes(&mut self, nodes: &[Node]);

    /// Set the edges
    fn set_edges(&mut self, edges: &[Edge]);

    /// Highlight specific nodes
    fn highlight(&mut self, ids: &[NodeId]);
}

/// Helper to compute screen bounds for a sub-region
pub fn sub_bounds(parent: Bounds, x_pct: f32, y_pct: f32, w_pct: f32, h_pct: f32) -> Bounds {
    Bounds {
        origin: Point {
            x: parent.origin.x + parent.size.width * x_pct,
            y: parent.origin.y + parent.size.height * y_pct,
        },
        size: wgpui::Size {
            width: parent.size.width * w_pct,
            height: parent.size.height * h_pct,
        },
    }
}
