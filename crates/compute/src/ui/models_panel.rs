//! Models panel showing available Ollama models

use crate::state::AppState;
use std::sync::Arc;
use wgpui::{Bounds, InputEvent, Point, Quad, Scene, TextSystem};
use wgpui::color::Hsla;
use wgpui::geometry::CornerRadii;

/// Models display panel
pub struct ModelsPanel {
    state: Arc<AppState>,
}

impl ModelsPanel {
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
            "MODELS",
            Point {
                x: bounds.origin.x + 10.0,
                y: bounds.origin.y + 10.0,
            },
            9.0,
            white_40,
        );
        scene.draw_text(title);

        // Get models
        let models = self.state.available_models.get();
        let ollama_available = self.state.ollama_available.get();

        if !ollama_available {
            let status = text_system.layout(
                "Ollama unavailable",
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

        if models.is_empty() {
            let status = text_system.layout(
                "No models",
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

        // List models (up to 3)
        for (i, model) in models.iter().take(3).enumerate() {
            let checkbox = if model.selected { "[x]" } else { "[ ]" };
            let model_text = format!("{} {}", checkbox, model.name);
            let label = text_system.layout(
                &model_text,
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
