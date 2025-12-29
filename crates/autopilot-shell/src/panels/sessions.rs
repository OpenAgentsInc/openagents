//! Sessions panel for the left sidebar

use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point, Quad,
    components::hud::Frame,
};
use crate::dock::{DockPosition, Panel};

/// Left sidebar panel with session list
pub struct SessionsPanel {}

impl SessionsPanel {
    pub fn new() -> Self {
        Self {}
    }

    fn paint_hotkey_legend(&self, bounds: Bounds, cx: &mut PaintContext) {
        let hotkeys = [
            ("cmd-a", "Toggle Full Auto"),
            ("cmd-f", "Toggle Fullscreen"),
            ("cmd-[", "Toggle left sidebar"),
            ("cmd-]", "Toggle right sidebar"),
            ("cmd-\\", "Toggle all sidebars"),
            ("esc", "Exit"),
        ];

        let line_height = 16.0;
        let padding = 16.0;
        let x = bounds.origin.x + padding;
        let mut y = bounds.origin.y + bounds.size.height - (hotkeys.len() as f32 * line_height) - padding;

        let text_color = Hsla::new(0.0, 0.0, 0.4, 0.9);
        let key_color = Hsla::new(180.0, 0.4, 0.4, 0.9);
        let font_size = 9.0;

        for (key, desc) in &hotkeys {
            let key_run = cx.text.layout(key, Point::new(x, y), font_size, key_color);
            cx.scene.draw_text(key_run);

            let desc_run = cx.text.layout(desc, Point::new(x + 80.0, y), font_size, text_color);
            cx.scene.draw_text(desc_run);

            y += line_height;
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

        // Simple session list
        let y = bounds.origin.y + padding;
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

        // Hotkey legend at bottom
        self.paint_hotkey_legend(bounds, cx);
    }

    fn event(&mut self, _event: &InputEvent, _bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        EventResult::Ignored
    }
}
