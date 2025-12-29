//! Sessions panel for the left sidebar

use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point, Quad,
    components::TextInput,
    components::hud::Frame,
};
use crate::dock::{DockPosition, Panel};

/// Left sidebar panel with session search
pub struct SessionsPanel {
    search: TextInput,
}

impl SessionsPanel {
    pub fn new() -> Self {
        Self {
            search: TextInput::new()
                .placeholder("Search sessions...")
                .background(Hsla::new(0.0, 0.0, 0.1, 1.0)),
        }
    }
}

impl Default for SessionsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for SessionsPanel {
    fn panel_id(&self) -> &'static str {
        "sessions"
    }

    fn title(&self) -> &str {
        "Sessions"
    }

    fn preferred_position(&self) -> DockPosition {
        DockPosition::Left
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
        let search_h = 32.0;

        // Search bar at top
        let search_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding,
            bounds.size.width - padding * 2.0,
            search_h,
        );
        Component::paint(&mut self.search, search_bounds, cx);

        // Simple session list below
        let y = bounds.origin.y + padding + search_h + 16.0;
        let x = bounds.origin.x + padding;
        let w = bounds.size.width - padding * 2.0;

        // Current session indicator
        let item_h = 36.0;
        let item_bg = Hsla::new(0.0, 0.0, 0.1, 1.0);
        let accent = Hsla::new(180.0, 0.5, 0.5, 1.0); // cyan accent

        // Item background
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y, w, item_h)).with_background(item_bg));
        // Left accent bar
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y, 3.0, item_h)).with_background(accent));

        // Session title
        let label_color = Hsla::new(0.0, 0.0, 0.8, 1.0);
        let title = cx.text.layout("Current Session", Point::new(x + 12.0, y + 10.0), 12.0, label_color);
        cx.scene.draw_text(title);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let padding = 16.0;
        let search_h = 32.0;

        let search_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding,
            bounds.size.width - padding * 2.0,
            search_h,
        );
        self.search.event(event, search_bounds, cx)
    }
}
