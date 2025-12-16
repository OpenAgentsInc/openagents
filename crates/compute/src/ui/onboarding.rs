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

        // Background
        scene.draw_quad(Quad {
            bounds,
            background: Some(Hsla::new(220.0, 0.08, 0.08, 1.0)),
            ..Default::default()
        });

        // TODO: Implement full UI with HUD components
        // For now, this is a placeholder that will be filled in
        // with actual HUD button, text, and input components

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

    fn paint_welcome(&self, _bounds: Bounds, _scene: &mut Scene, _scale: f32) {
        // TODO: Paint welcome screen with Generate/Import buttons
    }

    fn paint_show_seed(&self, _bounds: Bounds, _scene: &mut Scene, _scale: f32) {
        // TODO: Paint seed phrase display grid
    }

    fn paint_import_seed(&self, _bounds: Bounds, _scene: &mut Scene, _scale: f32) {
        // TODO: Paint seed import text area
    }

    fn paint_confirm_backup(&self, _bounds: Bounds, _scene: &mut Scene, _scale: f32) {
        // TODO: Paint backup confirmation checkbox
    }

    /// Handle input events
    pub fn handle_event(&mut self, _event: &InputEvent, _bounds: Bounds) -> bool {
        // TODO: Handle button clicks, text input, etc.
        false
    }
}
