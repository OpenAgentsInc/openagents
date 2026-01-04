//! LayerStack - stacked layer visualization

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Color, Point, Size};

use crate::grammar::{Edge, Node, NodeId, Topology, VizPrimitive};

/// A layer in the stack
#[derive(Clone)]
pub struct Layer {
    pub id: NodeId,
    pub label: String,
    pub intensity: f32, // 0.0 to 1.0
    pub active: bool,
}

/// A stacked layer visualization (like neural network layers)
pub struct LayerStack {
    layers: Vec<Layer>,
    highlighted: Vec<NodeId>,
    layer_color: Color,
    active_color: Color,
    highlight_color: Color,
    gap: f32,
}

impl LayerStack {
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            highlighted: Vec::new(),
            layer_color: Color::from_rgba(0.2, 0.2, 0.2, 1.0),
            active_color: Color::from_rgba(0.0, 0.6, 0.3, 1.0),
            highlight_color: Color::from_rgba(0.0, 0.8, 1.0, 1.0),
            gap: 4.0,
        }
    }

    pub fn with_layers(mut self, labels: &[&str]) -> Self {
        self.layers = labels
            .iter()
            .enumerate()
            .map(|(i, &label)| Layer {
                id: i as NodeId,
                label: label.to_string(),
                intensity: 0.0,
                active: false,
            })
            .collect();
        self
    }

    pub fn set_layer_intensity(&mut self, index: usize, intensity: f32) {
        if let Some(layer) = self.layers.get_mut(index) {
            layer.intensity = intensity.clamp(0.0, 1.0);
        }
    }

    pub fn set_layer_active(&mut self, index: usize, active: bool) {
        if let Some(layer) = self.layers.get_mut(index) {
            layer.active = active;
        }
    }
}

impl Default for LayerStack {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for LayerStack {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let count = self.layers.len();
        if count == 0 {
            return;
        }

        let layer_height =
            (bounds.size.height - self.gap * (count - 1) as f32) / count as f32;

        for (i, layer) in self.layers.iter().enumerate() {
            let y = bounds.origin.y + i as f32 * (layer_height + self.gap);

            let is_highlighted = self.highlighted.contains(&layer.id);

            let base_color = if is_highlighted {
                self.highlight_color
            } else if layer.active {
                self.active_color
            } else {
                self.layer_color
            };

            // Interpolate intensity
            let color = Color::from_rgba(
                base_color.r * (0.3 + 0.7 * layer.intensity),
                base_color.g * (0.3 + 0.7 * layer.intensity),
                base_color.b * (0.3 + 0.7 * layer.intensity),
                1.0,
            );

            let layer_bounds = Bounds {
                origin: Point {
                    x: bounds.origin.x,
                    y,
                },
                size: Size {
                    width: bounds.size.width,
                    height: layer_height,
                },
            };

            cx.scene.fill_rect(layer_bounds, color);
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(120.0), Some(80.0))
    }
}

impl VizPrimitive for LayerStack {
    fn update(&mut self, _value: f32) {}

    fn animate_to(&mut self, _value: f32, _duration_ms: u32) {}
}

impl Topology for LayerStack {
    fn set_nodes(&mut self, nodes: &[Node]) {
        self.layers = nodes
            .iter()
            .map(|n| Layer {
                id: n.id,
                label: n.label.clone(),
                intensity: 0.0,
                active: false,
            })
            .collect();
    }

    fn set_edges(&mut self, _edges: &[Edge]) {
        // LayerStack doesn't use edges
    }

    fn highlight(&mut self, ids: &[NodeId]) {
        self.highlighted = ids.to_vec();
    }
}
