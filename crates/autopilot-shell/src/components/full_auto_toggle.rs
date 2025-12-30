//! Full Auto Toggle component
//!
//! Compact one-line toggle for enabling/disabling autopilot loop.
//! Visual: ○ FULL AUTO OFF / ● FULL AUTO ON

use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point, Quad, theme,
};

/// Full Auto mode toggle - compact one-line style
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
        let font_size = 11.0;
        let padding = 8.0;
        let x = bounds.origin.x + padding;
        let y = bounds.origin.y + (bounds.size.height - font_size) / 2.0;

        // Hover background
        if self.hovered {
            cx.scene.draw_quad(
                Quad::new(bounds).with_background(theme::bg::MUTED.with_alpha(0.3)),
            );
        }

        // Indicator + Label + State all on one line
        // Use brighter colors for visibility on dark background
        let bright_green = Hsla::new(0.403, 1.0, 0.6, 1.0); // Brighter than theme::accent::GREEN
        let bright_red = Hsla::new(0.0, 0.8, 0.6, 1.0);     // Brighter than theme::accent::RED
        let (indicator, state_text, state_color) = if self.enabled {
            ("●", "ON", bright_green)
        } else {
            ("○", "OFF", bright_red)
        };

        // Draw indicator
        let indicator_run = cx.text.layout(indicator, Point::new(x, y), font_size, state_color);
        cx.scene.draw_text(indicator_run);

        // "FULL AUTO" label
        let label_x = x + font_size * 1.2;
        let label = cx.text.layout("FULL AUTO", Point::new(label_x, y), font_size, theme::text::MUTED);
        cx.scene.draw_text(label);

        // State text (ON/OFF)
        let state_x = label_x + font_size * 6.5;
        let state_label = cx.text.layout(state_text, Point::new(state_x, y), font_size, state_color);
        cx.scene.draw_text(state_label);

        // Hint text
        let hint_x = state_x + font_size * 2.5;
        let hint = cx.text.layout("(⌘A)", Point::new(hint_x, y), 10.0, theme::text::MUTED.with_alpha(0.5));
        cx.scene.draw_text(hint);

        // Subtle divider line at bottom
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height - 1.0,
                bounds.size.width,
                1.0,
            ))
            .with_background(theme::border::DEFAULT.with_alpha(0.3)),
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
