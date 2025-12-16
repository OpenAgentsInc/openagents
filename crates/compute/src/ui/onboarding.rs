//! Onboarding screen for seed phrase generation/import

use crate::domain::UnifiedIdentity;
use crate::state::AppState;
use std::sync::Arc;
use wgpui::{Bounds, InputEvent, Scene};

/// Onboarding screen state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingState {
    /// Initial choice: generate or import
    Welcome,
    /// Showing generated seed phrase
    ShowSeed,
    /// Importing existing seed
    ImportSeed,
    /// Confirming backup
    ConfirmBackup,
}

/// Onboarding screen for new users
pub struct OnboardingScreen {
    state: Arc<AppState>,
    screen_state: OnboardingState,
    generated_identity: Option<UnifiedIdentity>,
    import_text: String,
    backup_confirmed: bool,
}

impl OnboardingScreen {
    /// Create a new onboarding screen
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            screen_state: OnboardingState::Welcome,
            generated_identity: None,
            import_text: String::new(),
            backup_confirmed: false,
        }
    }

    /// Generate a new identity
    pub fn generate_new(&mut self) {
        match UnifiedIdentity::generate() {
            Ok(identity) => {
                self.generated_identity = Some(identity);
                self.screen_state = OnboardingState::ShowSeed;
            }
            Err(e) => {
                log::error!("Failed to generate identity: {}", e);
            }
        }
    }

    /// Start import flow
    pub fn start_import(&mut self) {
        self.screen_state = OnboardingState::ImportSeed;
    }

    /// Try to import from the current text
    pub fn try_import(&mut self) -> Result<(), String> {
        let mnemonic = self.import_text.trim();
        match UnifiedIdentity::from_mnemonic(mnemonic, "") {
            Ok(identity) => {
                self.state.set_identity(identity);
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Confirm backup and proceed
    pub fn confirm_backup(&mut self) {
        if let Some(identity) = self.generated_identity.take() {
            self.state.set_identity(identity);
        }
    }

    /// Paint the screen
    pub fn paint(&mut self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        use wgpui::Quad;
        use wgpui::color::Hsla;

        log::debug!("Onboarding paint called, bounds: {:?}", bounds);

        // Background - dark blue (hue is 0-1 normalized, so 220/360 = 0.611)
        scene.draw_quad(Quad {
            bounds,
            background: Some(Hsla::new(220.0 / 360.0, 0.15, 0.12, 1.0)),
            ..Default::default()
        });

        match self.screen_state {
            OnboardingState::Welcome => {
                self.paint_welcome(bounds, scene, scale);
            }
            OnboardingState::ShowSeed => {
                self.paint_show_seed(bounds, scene, scale);
            }
            OnboardingState::ImportSeed => {
                self.paint_import_seed(bounds, scene, scale);
            }
            OnboardingState::ConfirmBackup => {
                self.paint_confirm_backup(bounds, scene, scale);
            }
        }
    }

    fn paint_welcome(&self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        use wgpui::{Point, Quad, Size};
        use wgpui::color::Hsla;
        use wgpui::geometry::CornerRadii;

        let center_x = bounds.origin.x + bounds.size.width / 2.0;
        let center_y = bounds.origin.y + bounds.size.height / 2.0;

        // Title area - cyan accent bar at top
        let title_bar = Bounds {
            origin: Point { x: bounds.origin.x, y: bounds.origin.y },
            size: Size { width: bounds.size.width, height: 60.0 * scale },
        };
        scene.draw_quad(Quad {
            bounds: title_bar,
            background: Some(Hsla::new(180.0 / 360.0, 0.8, 0.4, 1.0)), // Cyan
            ..Default::default()
        });

        // Main content card
        let card_width = 400.0 * scale;
        let card_height = 300.0 * scale;
        let card_bounds = Bounds {
            origin: Point {
                x: center_x - card_width / 2.0,
                y: center_y - card_height / 2.0,
            },
            size: Size { width: card_width, height: card_height },
        };
        scene.draw_quad(Quad {
            bounds: card_bounds,
            background: Some(Hsla::new(220.0 / 360.0, 0.2, 0.18, 1.0)),
            corner_radii: CornerRadii::uniform(12.0 * scale),
            ..Default::default()
        });

        // "Generate New" button (green)
        let btn_width = 300.0 * scale;
        let btn_height = 50.0 * scale;
        let generate_btn = Bounds {
            origin: Point {
                x: center_x - btn_width / 2.0,
                y: center_y - btn_height - 20.0 * scale,
            },
            size: Size { width: btn_width, height: btn_height },
        };
        scene.draw_quad(Quad {
            bounds: generate_btn,
            background: Some(Hsla::new(140.0 / 360.0, 0.6, 0.4, 1.0)), // Green
            corner_radii: CornerRadii::uniform(8.0 * scale),
            ..Default::default()
        });

        // "Import Existing" button (blue)
        let import_btn = Bounds {
            origin: Point {
                x: center_x - btn_width / 2.0,
                y: center_y + 20.0 * scale,
            },
            size: Size { width: btn_width, height: btn_height },
        };
        scene.draw_quad(Quad {
            bounds: import_btn,
            background: Some(Hsla::new(210.0 / 360.0, 0.6, 0.5, 1.0)), // Blue
            corner_radii: CornerRadii::uniform(8.0 * scale),
            ..Default::default()
        });

        // Status indicator at bottom (orange if no text rendering)
        let status_bar = Bounds {
            origin: Point {
                x: bounds.origin.x,
                y: bounds.origin.y + bounds.size.height - 40.0 * scale,
            },
            size: Size { width: bounds.size.width, height: 40.0 * scale },
        };
        scene.draw_quad(Quad {
            bounds: status_bar,
            background: Some(Hsla::new(30.0 / 360.0, 0.8, 0.5, 1.0)), // Orange
            ..Default::default()
        });

        log::debug!("Welcome screen painted with {} quads", 5);
    }

    fn paint_show_seed(&self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        use wgpui::{Point, Quad, Size};
        use wgpui::color::Hsla;
        use wgpui::geometry::CornerRadii;

        // Grid of 12 word slots
        let grid_cols = 3;
        let grid_rows = 4;
        let slot_width = 150.0 * scale;
        let slot_height = 40.0 * scale;
        let gap = 10.0 * scale;

        let grid_width = grid_cols as f32 * slot_width + (grid_cols - 1) as f32 * gap;
        let grid_height = grid_rows as f32 * slot_height + (grid_rows - 1) as f32 * gap;
        let start_x = bounds.origin.x + (bounds.size.width - grid_width) / 2.0;
        let start_y = bounds.origin.y + (bounds.size.height - grid_height) / 2.0;

        for row in 0..grid_rows {
            for col in 0..grid_cols {
                let slot_bounds = Bounds {
                    origin: Point {
                        x: start_x + col as f32 * (slot_width + gap),
                        y: start_y + row as f32 * (slot_height + gap),
                    },
                    size: Size { width: slot_width, height: slot_height },
                };
                scene.draw_quad(Quad {
                    bounds: slot_bounds,
                    background: Some(Hsla::new(220.0 / 360.0, 0.2, 0.25, 1.0)),
                    corner_radii: CornerRadii::uniform(4.0 * scale),
                    ..Default::default()
                });
            }
        }
    }

    fn paint_import_seed(&self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        use wgpui::{Point, Quad, Size};
        use wgpui::color::Hsla;
        use wgpui::geometry::CornerRadii;

        // Text input area
        let input_width = 500.0 * scale;
        let input_height = 150.0 * scale;
        let input_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x + (bounds.size.width - input_width) / 2.0,
                y: bounds.origin.y + (bounds.size.height - input_height) / 2.0,
            },
            size: Size { width: input_width, height: input_height },
        };
        scene.draw_quad(Quad {
            bounds: input_bounds,
            background: Some(Hsla::new(220.0 / 360.0, 0.15, 0.2, 1.0)),
            corner_radii: CornerRadii::uniform(8.0 * scale),
            ..Default::default()
        });
    }

    fn paint_confirm_backup(&self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        use wgpui::{Point, Quad, Size};
        use wgpui::color::Hsla;
        use wgpui::geometry::CornerRadii;

        // Checkbox area
        let checkbox_size = 30.0 * scale;
        let center_x = bounds.origin.x + bounds.size.width / 2.0;
        let center_y = bounds.origin.y + bounds.size.height / 2.0;

        let checkbox_bounds = Bounds {
            origin: Point {
                x: center_x - 200.0 * scale,
                y: center_y,
            },
            size: Size { width: checkbox_size, height: checkbox_size },
        };
        scene.draw_quad(Quad {
            bounds: checkbox_bounds,
            background: Some(Hsla::new(220.0 / 360.0, 0.2, 0.3, 1.0)),
            corner_radii: CornerRadii::uniform(4.0 * scale),
            ..Default::default()
        });
    }

    /// Handle input events
    pub fn handle_event(&mut self, _event: &InputEvent, _bounds: Bounds) -> bool {
        // TODO: Handle button clicks, text input, etc.
        false
    }
}
