//! Wallet panel showing balance and Spark address

use crate::state::AppState;
use std::sync::Arc;
use wgpui::{Bounds, InputEvent, Point, Quad, Scene, TextSystem};
use wgpui::color::Hsla;
use wgpui::geometry::CornerRadii;

/// Wallet display panel
pub struct WalletPanel {
    state: Arc<AppState>,
}

impl WalletPanel {
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
            "WALLET",
            Point {
                x: bounds.origin.x + 10.0,
                y: bounds.origin.y + 10.0,
            },
            9.0,
            white_40,
        );
        scene.draw_text(title);

        // Balance
        let balance = self.state.balance_sats.get();
        let balance_text = format!("{} sats", balance);
        let balance_label = text_system.layout(
            &balance_text,
            Point {
                x: bounds.origin.x + 10.0,
                y: bounds.origin.y + 35.0,
            },
            14.0,
            white_70,
        );
        scene.draw_text(balance_label);
    }

    pub fn handle_event(&mut self, _event: &InputEvent, _bounds: Bounds) -> bool {
        false
    }
}
