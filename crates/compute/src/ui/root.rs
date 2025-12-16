//! Root view - main layout container

use crate::state::AppState;
use crate::ui::{BackupScreen, DashboardScreen};
use std::sync::Arc;
use wgpui::{Bounds, Scene, TextSystem};

/// Root view that switches between dashboard and backup screen
pub struct RootView {
    state: Arc<AppState>,
    dashboard: DashboardScreen,
    backup_screen: BackupScreen,
}

impl RootView {
    /// Create a new root view
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            dashboard: DashboardScreen::new(state.clone()),
            backup_screen: BackupScreen::new(state.clone()),
            state,
        }
    }

    /// Paint the view to the scene
    pub fn paint(&mut self, bounds: Bounds, scene: &mut Scene, scale: f32, text_system: &mut TextSystem) {
        if self.state.show_backup.get() {
            self.backup_screen.paint(bounds, scene, scale, text_system);
        } else {
            self.dashboard.paint(bounds, scene, scale, text_system);
        }
    }

    /// Handle input events
    pub fn handle_event(&mut self, event: &wgpui::InputEvent, bounds: Bounds) -> bool {
        if self.state.show_backup.get() {
            self.backup_screen.handle_event(event, bounds)
        } else {
            self.dashboard.handle_event(event, bounds)
        }
    }
}
