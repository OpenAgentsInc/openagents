//! Full Auto Toggle component
//!
//! Terminal-style toggle for enabling/disabling autopilot loop.
//! Visual: [■][_] FULL AUTO: OFF / [_][■] FULL AUTO: ON

use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point, Quad,
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
        let line_color = Hsla::new(0.0, 0.0, 0.3, 0.5);

        let padding = 8.0;
        let x = bounds.origin.x + padding;
        let y = bounds.origin.y + 8.0;

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
            ("ON", Hsla::new(120.0, 1.0, 0.45, 1.0)) // Bright green
        } else {
            ("OFF", Hsla::new(0.0, 0.8, 0.5, 1.0)) // Red
        };
        let state_label = cx.text.layout(state_text, Point::new(state_x, y), 11.0, state_color);
        cx.scene.draw_text(state_label);

        // Hint text
        let hint_x = state_x + 35.0;
        let hint_color = Hsla::new(0.0, 0.0, 0.35, 1.0);
        let hint = cx.text.layout("(cmd-a)", Point::new(hint_x, y), 10.0, hint_color);
        cx.scene.draw_text(hint);

        // Divider line at bottom
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height - 1.0,
                bounds.size.width,
                1.0,
            ))
            .with_background(line_color),
        );
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
