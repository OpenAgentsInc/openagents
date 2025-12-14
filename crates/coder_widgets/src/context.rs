//! Widget contexts for paint and event handling.
//!
//! Contexts provide widgets access to shared resources like
//! the scene and text system.

use coder_ui_runtime::CommandBus;
use wgpui::{Point, Scene, TextSystem};

/// Context for the paint phase.
pub struct PaintContext<'a> {
    /// The scene to draw to.
    pub scene: &'a mut Scene,
    /// Text system for rendering text.
    pub text: &'a mut TextSystem,
    /// Scale factor.
    pub scale_factor: f32,
    /// Current scroll offset.
    pub scroll_offset: Point,
}

impl<'a> PaintContext<'a> {
    /// Create a new paint context.
    pub fn new(scene: &'a mut Scene, text: &'a mut TextSystem, scale_factor: f32) -> Self {
        Self {
            scene,
            text,
            scale_factor,
            scroll_offset: Point::ZERO,
        }
    }

    /// Create a paint context with scroll offset.
    pub fn with_scroll_offset(
        scene: &'a mut Scene,
        text: &'a mut TextSystem,
        scale_factor: f32,
        scroll_offset: Point,
    ) -> Self {
        Self {
            scene,
            text,
            scale_factor,
            scroll_offset,
        }
    }
}

/// Context for event handling.
pub struct EventContext<'a> {
    /// Command bus for dispatching commands.
    pub commands: &'a mut CommandBus,
    /// Focused widget ID.
    pub focused: Option<u64>,
    /// Hovered widget ID.
    pub hovered: Option<u64>,
    /// Current scroll offset.
    pub scroll_offset: Point,
}

impl<'a> EventContext<'a> {
    /// Create a new event context.
    pub fn new(commands: &'a mut CommandBus) -> Self {
        Self {
            commands,
            focused: None,
            hovered: None,
            scroll_offset: Point::ZERO,
        }
    }

    /// Set the focused widget.
    pub fn set_focus(&mut self, widget_id: u64) {
        self.focused = Some(widget_id);
    }

    /// Clear focus.
    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    /// Check if a widget has focus.
    pub fn has_focus(&self, widget_id: u64) -> bool {
        self.focused == Some(widget_id)
    }

    /// Set the hovered widget.
    pub fn set_hover(&mut self, widget_id: u64) {
        self.hovered = Some(widget_id);
    }

    /// Clear hover.
    pub fn clear_hover(&mut self) {
        self.hovered = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_context_focus() {
        let mut bus = CommandBus::new();
        let mut cx = EventContext::new(&mut bus);

        assert!(cx.focused.is_none());

        cx.set_focus(42);
        assert!(cx.has_focus(42));
        assert!(!cx.has_focus(99));

        cx.clear_focus();
        assert!(cx.focused.is_none());
    }
}
