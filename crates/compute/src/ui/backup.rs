//! Backup screen for viewing and confirming seed phrase backup

use crate::state::AppState;
use std::sync::Arc;
use wgpui::color::Hsla;
use wgpui::{Bounds, InputEvent, Point, Scene, Size, TextSystem};

/// Backup screen for viewing seed phrase
pub struct BackupScreen {
    state: Arc<AppState>,
}

impl BackupScreen {
    /// Create a new backup screen
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Paint the screen
    pub fn paint(&mut self, bounds: Bounds, scene: &mut Scene, _scale: f32, text_system: &mut TextSystem) {
        use wgpui::geometry::CornerRadii;
        use wgpui::Quad;

        let white_70 = Hsla::new(0.0, 0.0, 1.0, 0.7);
        let white_40 = Hsla::new(0.0, 0.0, 1.0, 0.4);
        let white_10 = Hsla::new(0.0, 0.0, 1.0, 0.1);
        let black = Hsla::new(0.0, 0.0, 0.0, 1.0);
        let dark_gray = Hsla::new(0.0, 0.0, 0.08, 1.0);

        // Black background
        scene.draw_quad(Quad {
            bounds,
            background: Some(black),
            ..Default::default()
        });

        // Header
        let header = Bounds {
            origin: bounds.origin,
            size: Size { width: bounds.size.width, height: 40.0 },
        };
        scene.draw_quad(Quad {
            bounds: header,
            background: Some(dark_gray),
            ..Default::default()
        });

        let title_text = text_system.layout(
            "BACKUP SEED",
            Point { x: bounds.origin.x + 16.0, y: bounds.origin.y + 12.0 },
            14.0,
            white_70,
        );
        scene.draw_text(title_text);

        // Warning text - centered
        let warning = "Write down these 12 words!";
        let warning_width = text_system.measure(warning, 11.0);
        let warning_text = text_system.layout(
            warning,
            Point {
                x: bounds.origin.x + (bounds.size.width - warning_width) / 2.0,
                y: bounds.origin.y + 60.0,
            },
            11.0,
            white_40,
        );
        scene.draw_text(warning_text);

        // Get mnemonic words from identity
        let words: Vec<String> = self.state.identity.get()
            .map(|id| id.mnemonic().split_whitespace().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        // Grid layout for words
        let grid_cols = 3;
        let grid_rows = 4;
        let slot_width = 100.0;
        let slot_height = 28.0;
        let gap = 6.0;

        let grid_width = grid_cols as f32 * slot_width + (grid_cols - 1) as f32 * gap;
        let start_x = bounds.origin.x + (bounds.size.width - grid_width) / 2.0;
        let start_y = bounds.origin.y + 90.0;

        for row in 0..grid_rows {
            for col in 0..grid_cols {
                let idx = row * grid_cols + col;
                let slot_bounds = Bounds {
                    origin: Point {
                        x: start_x + col as f32 * (slot_width + gap),
                        y: start_y + row as f32 * (slot_height + gap),
                    },
                    size: Size { width: slot_width, height: slot_height },
                };
                scene.draw_quad(Quad {
                    bounds: slot_bounds,
                    background: Some(white_10),
                    corner_radii: CornerRadii::uniform(3.0),
                    ..Default::default()
                });

                if let Some(word) = words.get(idx) {
                    let word_label = format!("{}. {}", idx + 1, word);
                    let word_text = text_system.layout(
                        &word_label,
                        Point {
                            x: slot_bounds.origin.x + 6.0,
                            y: slot_bounds.origin.y + 7.0,
                        },
                        10.0,
                        white_70,
                    );
                    scene.draw_text(word_text);
                }
            }
        }

        // Done button
        let btn_width = 160.0;
        let btn_height = 32.0;
        let btn_bounds = self.get_done_button_bounds(bounds);
        scene.draw_quad(Quad {
            bounds: btn_bounds,
            background: Some(white_10),
            corner_radii: CornerRadii::uniform(4.0),
            ..Default::default()
        });

        let btn_label = "Done";
        let btn_text_width = text_system.measure(btn_label, 10.0);
        let btn_text = text_system.layout(
            btn_label,
            Point {
                x: btn_bounds.origin.x + (btn_width - btn_text_width) / 2.0,
                y: btn_bounds.origin.y + (btn_height - 10.0) / 2.0,
            },
            10.0,
            white_70,
        );
        scene.draw_text(btn_text);
    }

    fn get_done_button_bounds(&self, bounds: Bounds) -> Bounds {
        let btn_width = 160.0;
        let btn_height = 32.0;
        Bounds {
            origin: Point {
                x: bounds.origin.x + (bounds.size.width - btn_width) / 2.0,
                y: bounds.origin.y + bounds.size.height - 80.0,
            },
            size: Size { width: btn_width, height: btn_height },
        }
    }

    /// Handle input events
    pub fn handle_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        if let InputEvent::MouseDown { position, .. } = event {
            let btn_bounds = self.get_done_button_bounds(bounds);
            if btn_bounds.contains(*position) {
                log::info!("Backup confirmed");
                self.state.mark_backed_up();
                return true;
            }
        }
        false
    }
}
