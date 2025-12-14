//! TestHarness for headless widget testing.
//!
//! The test harness provides a way to mount widgets and test them
//! without requiring a GPU or window system.

mod mock_text;
mod mounted;

pub use mock_text::MockTextSystem;
pub use mounted::MountedWidget;

use crate::platform::MockPlatform;
use crate::reactive::SignalTracker;
use coder_ui_runtime::CommandBus;
use coder_widgets::context::EventContext;
use coder_widgets::widget::{EventResult, Widget};
use wgpui::{Bounds, InputEvent, Point, Scene, Size};

/// Test harness for headless widget testing.
///
/// Provides everything needed to mount, render, and interact with
/// widgets without a GPU or window system.
pub struct TestHarness {
    /// The captured scene from rendering.
    scene: Scene,
    /// Mock text system for text measurement.
    text_system: MockTextSystem,
    /// Mock platform for platform-specific operations.
    platform: MockPlatform,
    /// Signal tracker for reactive testing.
    signals: SignalTracker,
    /// Command bus for event handling.
    command_bus: CommandBus,
    /// Scale factor for rendering.
    scale_factor: f32,
    /// Current scroll offset.
    scroll_offset: Point,
    /// Viewport size.
    viewport_size: Size,
}

impl TestHarness {
    /// Create a new test harness with default settings.
    pub fn new() -> Self {
        Self {
            scene: Scene::new(),
            text_system: MockTextSystem::new(),
            platform: MockPlatform::new(),
            signals: SignalTracker::new(),
            command_bus: CommandBus::new(),
            scale_factor: 1.0,
            scroll_offset: Point::ZERO,
            viewport_size: Size::new(1280.0, 720.0),
        }
    }

    /// Create a test harness with custom viewport size.
    pub fn with_viewport(width: f32, height: f32) -> Self {
        let mut harness = Self::new();
        harness.viewport_size = Size::new(width, height);
        harness
    }

    /// Set the scale factor.
    pub fn set_scale_factor(&mut self, factor: f32) {
        self.scale_factor = factor;
    }

    /// Set the scroll offset.
    pub fn set_scroll_offset(&mut self, offset: Point) {
        self.scroll_offset = offset;
    }

    /// Set the viewport size.
    pub fn set_viewport_size(&mut self, size: Size) {
        self.viewport_size = size;
    }

    /// Get the viewport bounds.
    pub fn viewport_bounds(&self) -> Bounds {
        Bounds::from_origin_size(Point::ZERO, self.viewport_size)
    }

    /// Mount a widget for testing.
    pub fn mount<W: Widget + 'static>(&mut self, widget: W) -> MountedWidget<W> {
        MountedWidget::new(widget, self.viewport_bounds())
    }

    /// Clear the scene and prepare for new rendering.
    ///
    /// Note: Widget testing requires direct access to the scene via `scene_mut()`.
    /// This method just prepares a clean scene for assertions.
    pub fn clear_scene(&mut self) {
        self.scene.clear();
    }

    /// Dispatch an input event to a widget.
    pub fn dispatch<W: Widget>(
        &mut self,
        widget: &mut W,
        event: &InputEvent,
        bounds: Bounds,
    ) -> EventResult {
        let mut cx = self.create_event_context();
        widget.event(event, bounds, &mut cx)
    }

    /// Get a reference to the captured scene.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Get a mutable reference to the scene (for clearing, etc.).
    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    /// Get a reference to the mock text system.
    pub fn text_system(&self) -> &MockTextSystem {
        &self.text_system
    }

    /// Get a mutable reference to the mock text system.
    pub fn text_system_mut(&mut self) -> &mut MockTextSystem {
        &mut self.text_system
    }

    /// Get a reference to the mock platform.
    pub fn platform(&self) -> &MockPlatform {
        &self.platform
    }

    /// Get a mutable reference to the mock platform.
    pub fn platform_mut(&mut self) -> &mut MockPlatform {
        &mut self.platform
    }

    /// Get a reference to the signal tracker.
    pub fn signals(&self) -> &SignalTracker {
        &self.signals
    }

    /// Get a mutable reference to the signal tracker.
    pub fn signals_mut(&mut self) -> &mut SignalTracker {
        &mut self.signals
    }

    /// Get a reference to the command bus.
    pub fn command_bus(&self) -> &CommandBus {
        &self.command_bus
    }

    /// Get a mutable reference to the command bus.
    pub fn command_bus_mut(&mut self) -> &mut CommandBus {
        &mut self.command_bus
    }

    /// Create an event context for input handling.
    fn create_event_context(&mut self) -> EventContext<'_> {
        EventContext::new(&mut self.command_bus)
    }
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock paint context that doesn't require real GPU resources.
///
/// This provides the same interface as PaintContext but uses
/// the MockTextSystem instead of a real TextSystem.
pub struct MockPaintContext<'a> {
    /// The scene to draw to.
    pub scene: &'a mut Scene,
    /// Mock text system.
    pub text: &'a mut MockTextSystem,
    /// Scale factor.
    pub scale_factor: f32,
    /// Current scroll offset.
    pub scroll_offset: Point,
}

impl<'a> MockPaintContext<'a> {
    /// Create a new mock paint context.
    pub fn new(
        scene: &'a mut Scene,
        text: &'a mut MockTextSystem,
        scale_factor: f32,
    ) -> Self {
        Self {
            scene,
            text,
            scale_factor,
            scroll_offset: Point::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harness_creation() {
        let harness = TestHarness::new();
        assert_eq!(harness.scale_factor, 1.0);
        assert_eq!(harness.viewport_size.width, 1280.0);
        assert_eq!(harness.viewport_size.height, 720.0);
    }

    #[test]
    fn test_harness_with_viewport() {
        let harness = TestHarness::with_viewport(800.0, 600.0);
        assert_eq!(harness.viewport_size.width, 800.0);
        assert_eq!(harness.viewport_size.height, 600.0);
    }

    #[test]
    fn test_harness_viewport_bounds() {
        let harness = TestHarness::with_viewport(100.0, 50.0);
        let bounds = harness.viewport_bounds();

        assert_eq!(bounds.origin.x, 0.0);
        assert_eq!(bounds.origin.y, 0.0);
        assert_eq!(bounds.size.width, 100.0);
        assert_eq!(bounds.size.height, 50.0);
    }

    #[test]
    fn test_harness_scale_factor() {
        let mut harness = TestHarness::new();
        harness.set_scale_factor(2.0);
        assert_eq!(harness.scale_factor, 2.0);
    }
}
