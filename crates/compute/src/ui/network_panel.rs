//! Network panel showing relay connection status

use crate::state::AppState;
use std::sync::Arc;
use wgpui::{Bounds, InputEvent, Point, Quad, Scene, TextSystem};
use wgpui::color::Hsla;
use wgpui::geometry::CornerRadii;

/// Network status panel
pub struct NetworkPanel {
    state: Arc<AppState>,
}

impl NetworkPanel {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, _scale: f32, text_system: &mut TextSystem) {
        let white_70 = Hsla::new(0.0, 0.0, 1.0, 0.7);
        let white_40 = Hsla::new(0.0, 0.0, 1.0, 0.4);
        let white_10 = Hsla::new(0.0, 0.0, 1.0, 0.1);

        // Panel background
        scene.draw_quad(Quad {
            bounds,
            background: Some(white_10),
            corner_radii: CornerRadii::uniform(4.0),
            ..Default::default()
        });

        // Title
        let title = text_system.layout(
            "NETWORK",
            Point {
                x: bounds.origin.x + 10.0,
                y: bounds.origin.y + 10.0,
            },
            9.0,
            white_40,
        );
        scene.draw_text(title);

        // Get connected relays
        let relays = self.state.connected_relays.get();

        if relays.is_empty() {
            let status = text_system.layout(
                "Disconnected",
                Point {
                    x: bounds.origin.x + 10.0,
                    y: bounds.origin.y + 32.0,
                },
                10.0,
                white_40,
            );
            scene.draw_text(status);
            return;
        }

        // List relays (up to 3)
        for (i, relay) in relays.iter().take(3).enumerate() {
            let domain = relay
                .strip_prefix("wss://")
                .or_else(|| relay.strip_prefix("ws://"))
                .unwrap_or(relay);

            let relay_text = format!("* {}", domain);
            let label = text_system.layout(
                &relay_text,
                Point {
                    x: bounds.origin.x + 10.0,
                    y: bounds.origin.y + 32.0 + (i as f32 * 14.0),
                },
                10.0,
                white_70,
            );
            scene.draw_text(label);
        }
    }

    pub fn handle_event(&mut self, _event: &InputEvent, _bounds: Bounds) -> bool {
        false
    }
}
