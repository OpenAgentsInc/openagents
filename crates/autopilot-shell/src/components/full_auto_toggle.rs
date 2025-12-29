//! Full Auto Toggle component
//!
//! Terminal-style toggle for enabling/disabling autopilot loop.
//! Visual: [■][_] FULL AUTO: OFF / [_][■] FULL AUTO: ON

use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point, Quad,
    components::hud::{CornerConfig, Frame},
};

/// Full Auto mode toggle
pub struct FullAutoToggle {
    pub enabled: bool,
    hovered: bool,
}

impl FullAutoToggle {
    pub fn new() -> Self {
        Self {
            enabled: false,
            hovered: false,
        }
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for FullAutoToggle {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for FullAutoToggle {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw HUD frame
        let line_color = if self.enabled {
            Hsla::new(120.0, 0.6, 0.4, 0.7) // Green border when on
        } else {
            Hsla::new(0.0, 0.0, 0.4, 0.5)
        };
        let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.9);

        let mut frame = Frame::nefrex()
            .line_color(line_color)
            .bg_color(bg_color)
            .stroke_width(1.0)
            .corner_config(CornerConfig::all())
            .square_size(4.0)
            .small_line_length(4.0)
            .large_line_length(10.0);
        frame.paint(bounds, cx);

        let padding = 12.0;
        let x = bounds.origin.x + padding;
        let y = bounds.origin.y + (bounds.size.height - 12.0) / 2.0;

        // Toggle boxes - two squares side by side
        let box_size = 12.0;
        let box_border = Hsla::new(0.0, 0.0, 0.9, 1.0);
        let box_fill = Hsla::new(0.0, 0.0, 0.9, 1.0);
        let box_empty = Hsla::new(0.0, 0.0, 0.1, 1.0);

        // Left box (filled when OFF)
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x, y, box_size, box_size))
                .with_background(if !self.enabled { box_fill } else { box_empty })
                .with_border(box_border, 1.0),
        );

        // Right box (filled when ON)
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + box_size - 1.0, y, box_size, box_size))
                .with_background(if self.enabled { box_fill } else { box_empty })
                .with_border(box_border, 1.0),
        );

        // Label
        let label_x = x + box_size * 2.0 + 8.0;
        let label_color = Hsla::new(0.0, 0.0, 0.5, 1.0);
        let label = cx.text.layout("FULL AUTO:", Point::new(label_x, y), 11.0, label_color);
        cx.scene.draw_text(label);

        // State indicator
        let state_x = label_x + 85.0;
        let (state_text, state_color) = if self.enabled {
            ("ON", Hsla::new(120.0, 0.7, 0.5, 1.0)) // Green
        } else {
            ("OFF", Hsla::new(0.0, 0.7, 0.5, 1.0)) // Red
        };
        let state_label = cx.text.layout(state_text, Point::new(state_x, y), 11.0, state_color);
        cx.scene.draw_text(state_label);

        // Hint text
        let hint_x = state_x + 35.0;
        let hint_color = Hsla::new(0.0, 0.0, 0.35, 1.0);
        let hint = cx.text.layout("(cmd-f)", Point::new(hint_x, y), 10.0, hint_color);
        cx.scene.draw_text(hint);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                self.hovered = bounds.contains(Point::new(*x, *y));
                EventResult::Ignored
            }
            InputEvent::MouseDown { x, y, .. } => {
                if bounds.contains(Point::new(*x, *y)) {
                    self.toggle();
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }
            _ => EventResult::Ignored,
        }
    }
}
