//! Job queue panel showing active jobs

use crate::state::AppState;
use std::sync::Arc;
use wgpui::{Bounds, InputEvent, Quad, Scene};
use wgpui::color::Hsla;
use wgpui::geometry::CornerRadii;

/// Job queue panel
pub struct JobQueue {
    #[allow(dead_code)]
    state: Arc<AppState>,
}

impl JobQueue {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        // Panel background
        scene.draw_quad(Quad {
            bounds,
            background: Some(Hsla::new(220.0 / 360.0, 0.1, 0.15, 1.0)),
            corner_radii: CornerRadii::uniform(8.0 * scale),
            ..Default::default()
        });

        // TODO: Add text rendering for job list
    }

    pub fn handle_event(&mut self, _event: &InputEvent, _bounds: Bounds) -> bool {
        false
    }
}
