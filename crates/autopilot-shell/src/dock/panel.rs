//! Panel trait for dockable content

use wgpui::{Bounds, EventContext, EventResult, InputEvent, PaintContext};

use super::DockPosition;

/// Trait for panel content that can be displayed in a dock
pub trait Panel {
    /// Unique identifier for this panel type
    fn panel_id(&self) -> &'static str;

    /// Display name shown in header/tabs
    fn title(&self) -> &str;

    /// Which dock position this panel prefers
    fn preferred_position(&self) -> DockPosition;

    /// Whether the panel can be closed
    fn can_close(&self) -> bool {
        true
    }

    /// Called when panel is about to be shown
    fn on_show(&mut self) {}

    /// Called when panel is about to be hidden
    fn on_hide(&mut self) {}

    /// Paint the panel content
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext);

    /// Handle input events
    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult;
}
