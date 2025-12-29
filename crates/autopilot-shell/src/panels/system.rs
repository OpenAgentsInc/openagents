//! System panel for the right sidebar

use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext,
    components::hud::Frame,
};
use crate::dock::{DockPosition, Panel};
use super::ClaudeUsage;

/// Right sidebar panel with Claude usage stats
pub struct SystemPanel {
    claude_usage: ClaudeUsage,
}

impl SystemPanel {
    pub fn new() -> Self {
        Self {
            claude_usage: ClaudeUsage::new(),
        }
    }
}

impl Default for SystemPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for SystemPanel {
    fn panel_id(&self) -> &'static str {
        "system"
    }

    fn title(&self) -> &str {
        "System"
    }

    fn preferred_position(&self) -> DockPosition {
        DockPosition::Right
    }

    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw simple line frame around panel
        let line_color = Hsla::new(0.0, 0.0, 0.3, 0.5);
        let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.9);

        let mut frame = Frame::lines()
            .line_color(line_color)
            .bg_color(bg_color)
            .stroke_width(1.0);
        frame.paint(bounds, cx);

        let padding = 16.0;

        // Full Auto toggle is painted by the shell at top (about 52px)
        // Start Claude Usage below that
        let toggle_space = 60.0;
        let usage_y = bounds.origin.y + toggle_space;
        let usage_height = bounds.size.height - toggle_space - padding;

        let usage_bounds = Bounds::new(
            bounds.origin.x + padding,
            usage_y,
            bounds.size.width - padding * 2.0,
            usage_height,
        );
        self.claude_usage.paint(usage_bounds, cx);
    }

    fn event(&mut self, _event: &InputEvent, _bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        EventResult::Ignored
    }
}
