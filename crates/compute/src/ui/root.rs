//! Root view - main layout container

use crate::state::AppState;
use crate::ui::{DashboardScreen, OnboardingScreen};
use std::sync::Arc;
use wgpui::{Bounds, Scene};

/// Root view that switches between onboarding and dashboard
pub struct RootView {
    state: Arc<AppState>,
    onboarding: OnboardingScreen,
    dashboard: DashboardScreen,
}

impl RootView {
    /// Create a new root view
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            onboarding: OnboardingScreen::new(state.clone()),
            dashboard: DashboardScreen::new(state.clone()),
            state,
        }
    }

    /// Paint the view to the scene
    pub fn paint(&mut self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        if self.state.is_onboarding.get() {
            self.onboarding.paint(bounds, scene, scale);
        } else {
            self.dashboard.paint(bounds, scene, scale);
        }
    }

    /// Handle input events
    pub fn handle_event(&mut self, event: &wgpui::InputEvent, bounds: Bounds) -> bool {
        if self.state.is_onboarding.get() {
            self.onboarding.handle_event(event, bounds)
        } else {
            self.dashboard.handle_event(event, bounds)
        }
    }
}
