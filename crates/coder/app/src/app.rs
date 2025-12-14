//! Application core - manages the main application lifecycle.

use crate::state::AppState;
use coder_shell::{Chrome, Navigation, Route, ViewRegistry};
use coder_ui_runtime::{CommandBus, Scheduler};
use coder_widgets::context::{EventContext, PaintContext};
use coder_widgets::{EventResult, Widget};
use wgpui::markdown::{MarkdownRenderer, StreamingConfig, StreamingMarkdown};
use wgpui::{Bounds, InputEvent, Point, Quad, Scene};

// Demo markdown content for streaming demo
const DEMO_MARKDOWN: &str = r#"# BUILD v5

This is a **GPU-accelerated** markdown renderer with *streaming* support.

## Features

- Syntax highlighting via syntect
- Streaming text support
- Full markdown rendering

## Code Example

```rust
fn main() {
    let greeting = "Hello, wgpui!";
    println!("{}", greeting);
}
```

> Blockquotes are styled with a yellow accent bar

---

### Inline Styles

You can use `inline code`, **bold**, *italic*, and ~~strikethrough~~.

1. Ordered lists
2. Work great
3. With numbers
"#;

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

    /// Streaming markdown demo.
    demo_streaming: StreamingMarkdown,

    /// Character index for streaming demo.
    demo_char_index: usize,

    /// Markdown renderer.
    markdown_renderer: MarkdownRenderer,
}

impl App {
    /// Create a new application.
    pub fn new() -> Self {
        // Set up streaming markdown with fade-in enabled and no debounce pauses
        let streaming_config = StreamingConfig {
            fade_in_frames: Some(15), // Fade in over ~250ms at 60fps
            debounce_ms: 0, // No debouncing to avoid pauses
            ..Default::default()
        };

        Self {
            state: AppState::new(),
            navigation: Navigation::new(),
            views: ViewRegistry::new(),
            chrome: Chrome::new().title("Coder"),
            scheduler: Scheduler::new(),
            commands: CommandBus::new(),
            window_size: (800.0, 600.0),
            scale_factor: 1.0,
            demo_streaming: StreamingMarkdown::with_config(streaming_config),
            demo_char_index: 0,
            markdown_renderer: MarkdownRenderer::new(),
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

        // Simulate streaming: append characters over time (faster, smoother)
        let chars_per_frame = if self.demo_char_index < 150 { 8 } else { 3 };

        if self.demo_char_index < DEMO_MARKDOWN.len() {
            let end = (self.demo_char_index + chars_per_frame).min(DEMO_MARKDOWN.len());
            self.demo_streaming.append(&DEMO_MARKDOWN[self.demo_char_index..end]);
            self.demo_char_index = end;
        } else if !self.demo_streaming.document().is_complete {
            self.demo_streaming.complete();
        }

        self.demo_streaming.tick();
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

    /// Paint the home screen with streaming markdown demo.
    fn paint_home(&self, bounds: Bounds, cx: &mut PaintContext) {
        // Header bar
        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            48.0,
        );
        cx.scene.draw_quad(
            Quad::new(header_bounds)
                .with_background(wgpui::theme::bg::SURFACE)
                .with_border(wgpui::theme::border::DEFAULT, 1.0),
        );

        // Header title
        let title_run = cx.text.layout(
            "wgpui Markdown Demo",
            Point::new(bounds.origin.x + 16.0, bounds.origin.y + 16.0),
            14.0,
            wgpui::theme::accent::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Streaming status
        let status_text = if self.demo_streaming.document().is_complete {
            "Complete"
        } else {
            "Streaming..."
        };
        let status_color = if self.demo_streaming.document().is_complete {
            wgpui::theme::status::SUCCESS
        } else {
            wgpui::theme::accent::PRIMARY
        };
        let status_run = cx.text.layout(
            status_text,
            Point::new(
                bounds.origin.x + bounds.size.width - 140.0,
                bounds.origin.y + 16.0,
            ),
            12.0,
            status_color,
        );
        cx.scene.draw_text(status_run);

        // Content area
        let content_x = bounds.origin.x + 20.0;
        let content_y = bounds.origin.y + 64.0;
        let content_width = (bounds.size.width - 40.0).min(700.0);

        // Render markdown with fade-in effect
        let fade = self.demo_streaming.fade_state();
        self.markdown_renderer.render_with_opacity(
            self.demo_streaming.document(),
            Point::new(content_x, content_y),
            content_width,
            cx.text,
            cx.scene,
            fade.new_content_opacity,
        );
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
