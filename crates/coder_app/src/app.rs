//! Application core - manages the main application lifecycle.

use crate::state::AppState;
use coder_shell::{Chrome, Navigation, Route, ViewRegistry};
use coder_ui_runtime::{CommandBus, Scheduler};
use coder_widgets::context::{EventContext, PaintContext};
use coder_widgets::{EventResult, Widget};
use wgpui::{Bounds, InputEvent, Scene};

/// The main application.
pub struct App {
    /// Application state.
    state: AppState,

    /// Navigation controller.
    navigation: Navigation,

    /// View registry.
    views: ViewRegistry,

    /// Application chrome.
    chrome: Chrome,

    /// Frame scheduler.
    scheduler: Scheduler,

    /// Command bus.
    commands: CommandBus,

    /// Current window size.
    window_size: (f32, f32),

    /// Scale factor for high-DPI.
    scale_factor: f32,
}

impl App {
    /// Create a new application.
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
            navigation: Navigation::new(),
            views: ViewRegistry::new(),
            chrome: Chrome::new().title("Coder"),
            scheduler: Scheduler::new(),
            commands: CommandBus::new(),
            window_size: (800.0, 600.0),
            scale_factor: 1.0,
        }
    }

    /// Initialize the application.
    pub fn init(&mut self) {
        log::info!("Initializing Coder application");

        // Set up initial route
        self.navigation.navigate(Route::Home);

        // Update chrome with initial breadcrumbs
        let crumbs = self.navigation.breadcrumbs();
        self.chrome.set_breadcrumbs(crumbs);

        log::info!("Coder application initialized");
    }

    /// Set the window size.
    pub fn set_size(&mut self, width: f32, height: f32) {
        self.window_size = (width, height);
    }

    /// Get the window size.
    pub fn size(&self) -> (f32, f32) {
        self.window_size
    }

    /// Handle an input event.
    pub fn handle_event(&mut self, event: &InputEvent) -> EventResult {
        let bounds = Bounds::new(0.0, 0.0, self.window_size.0, self.window_size.1);

        // Create event context
        let mut cx = EventContext::new(&mut self.commands);

        // Let chrome handle events first
        let result = self.chrome.event(event, bounds, &mut cx);
        if result.is_handled() {
            return result;
        }

        // Get content bounds (inside chrome)
        let content_bounds = self.chrome.content_bounds(bounds);

        // Let active view handle events
        if let Some(view) = self.views.active() {
            let result = view.widget().event(event, content_bounds, &mut cx);
            if result.is_handled() {
                return result;
            }
        }

        EventResult::Ignored
    }

    /// Run a frame update cycle.
    pub fn update(&mut self) {
        // Run the scheduler
        let _stats = self.scheduler.run_frame();
    }

    /// Paint the application to a scene.
    pub fn paint(&mut self, scene: &mut Scene, text_system: &mut wgpui::TextSystem) {
        let bounds = Bounds::new(0.0, 0.0, self.window_size.0, self.window_size.1);

        // Create paint context
        let mut cx = PaintContext::new(scene, text_system, self.scale_factor);

        // Paint background
        cx.scene.draw_quad(
            wgpui::Quad::new(bounds).with_background(wgpui::theme::bg::APP),
        );

        // Paint chrome
        self.chrome.paint(bounds, &mut cx);

        // Get content bounds (inside chrome)
        let content_bounds = self.chrome.content_bounds(bounds);

        // Paint active view
        if let Some(view) = self.views.active() {
            view.widget().paint(content_bounds, &mut cx);
        } else {
            // Paint default content when no view is active
            self.paint_home(content_bounds, &mut cx);
        }
    }

    /// Paint the home screen.
    fn paint_home(&self, bounds: Bounds, cx: &mut PaintContext) {
        // Center the welcome message
        let title = "Welcome to Coder";
        let subtitle = "Your AI-powered coding assistant";

        let title_x = bounds.origin.x + bounds.size.width / 2.0 - 100.0;
        let title_y = bounds.origin.y + bounds.size.height / 2.0 - 40.0;

        let title_run = cx.text.layout(
            title,
            wgpui::Point::new(title_x, title_y),
            24.0,
            wgpui::theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        let subtitle_run = cx.text.layout(
            subtitle,
            wgpui::Point::new(title_x - 20.0, title_y + 36.0),
            14.0,
            wgpui::theme::text::SECONDARY,
        );
        cx.scene.draw_text(subtitle_run);
    }

    /// Navigate to a route.
    pub fn navigate(&mut self, route: Route) {
        self.navigation.navigate(route.clone());

        // Update chrome breadcrumbs
        let crumbs = self.navigation.breadcrumbs();
        self.chrome.set_breadcrumbs(crumbs);

        // Activate view for route
        self.views.activate_for_route(&route);
    }

    /// Get the current route.
    pub fn current_route(&self) -> Route {
        self.navigation.current()
    }

    /// Get the navigation controller.
    pub fn navigation(&self) -> &Navigation {
        &self.navigation
    }

    /// Get mutable navigation controller.
    pub fn navigation_mut(&mut self) -> &mut Navigation {
        &mut self.navigation
    }

    /// Get the app state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get mutable app state.
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = App::new();
        assert_eq!(app.window_size, (800.0, 600.0));
    }

    #[test]
    fn test_app_set_size() {
        let mut app = App::new();
        app.set_size(1920.0, 1080.0);
        assert_eq!(app.size(), (1920.0, 1080.0));
    }

    #[test]
    fn test_app_init() {
        let mut app = App::new();
        app.init();
        assert!(app.current_route().is_home());
    }

    #[test]
    fn test_app_navigation() {
        let mut app = App::new();
        app.init();

        app.navigate(Route::Settings);
        assert_eq!(app.current_route(), Route::Settings);
    }
}
