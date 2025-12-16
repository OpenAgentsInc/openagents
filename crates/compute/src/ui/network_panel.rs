//! Network panel showing relay connection status

use crate::state::AppState;
use std::sync::Arc;
use wgpui::{Bounds, InputEvent, Quad, Scene};
use wgpui::color::Hsla;
use wgpui::geometry::CornerRadii;

/// Network status panel
pub struct NetworkPanel {
    #[allow(dead_code)]
    state: Arc<AppState>,
}

impl NetworkPanel {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        // Panel background
        scene.draw_quad(Quad {
            bounds,
            background: Some(Hsla::new(220.0, 0.1, 0.15, 1.0)),
            corner_radii: CornerRadii::uniform(8.0 * scale),
            ..Default::default()
        });

        // TODO: Add text rendering for relay status
    }

    pub fn handle_event(&mut self, _event: &InputEvent, _bounds: Bounds) -> bool {
        false
    }
}
