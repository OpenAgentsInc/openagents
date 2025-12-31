//! Threshold key indicator for NIP-SA Sovereign Agents.
//!
//! Shows the threshold signature scheme configuration (e.g., "2 of 3").

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Threshold key share status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyShareStatus {
    #[default]
    Unknown,
    Available,
    Unavailable,
    Signing,
}

impl KeyShareStatus {
    pub fn color(&self) -> Hsla {
        match self {
            KeyShareStatus::Unknown => Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
            KeyShareStatus::Available => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            KeyShareStatus::Unavailable => Hsla::new(0.0, 0.8, 0.5, 1.0), // Red
            KeyShareStatus::Signing => Hsla::new(200.0, 0.8, 0.55, 1.0), // Blue
        }
    }
}

/// Threshold key configuration display
pub struct ThresholdKeyBadge {
    id: Option<ComponentId>,
    threshold: u8,
    total: u8,
    shares_available: u8,
    compact: bool,
}

impl ThresholdKeyBadge {
    pub fn new(threshold: u8, total: u8) -> Self {
        Self {
            id: None,
            threshold,
            total,
            shares_available: 0,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn shares_available(mut self, available: u8) -> Self {
        self.shares_available = available;
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    fn status_color(&self) -> Hsla {
        if self.shares_available >= self.threshold {
            Hsla::new(120.0, 0.7, 0.45, 1.0) // Green - can sign
        } else if self.shares_available > 0 {
            Hsla::new(45.0, 0.8, 0.5, 1.0) // Gold - partial
        } else {
            Hsla::new(0.0, 0.0, 0.5, 1.0) // Gray - unknown
        }
    }
}

impl Component for ThresholdKeyBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status_color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.1, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let padding = 6.0;

        if self.compact {
            // Show just "2/3"
            let text = format!("{}/{}", self.threshold, self.total);
            let text_x = bounds.origin.x + (bounds.size.width - text.len() as f32 * 6.0) / 2.0;
            let run = cx.text.layout(
                &text,
                Point::new(text_x, text_y),
                theme::font_size::XS,
                color,
            );
            cx.scene.draw_text(run);
        } else {
            let mut x = bounds.origin.x + padding;

            // Key icon
            let icon_run = cx.text.layout(
                "🔐",
                Point::new(x, text_y - 1.0),
                theme::font_size::SM,
                color,
            );
            cx.scene.draw_text(icon_run);
            x += 18.0;

            // Threshold text
            let thresh_text = format!("{}-of-{}", self.threshold, self.total);
            let thresh_run = cx.text.layout(
                &thresh_text,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(thresh_run);
            x += thresh_text.len() as f32 * 6.5 + 8.0;

            // Available shares
            if self.shares_available > 0 {
                let avail_text = format!("({} ready)", self.shares_available);
                let avail_run = cx.text.layout(
                    &avail_text,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    color,
                );
                cx.scene.draw_text(avail_run);
            }
        }
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        if self.compact {
            (Some(36.0), Some(22.0))
        } else {
            let mut width = 90.0;
            if self.shares_available > 0 {
                width += 60.0;
            }
            (Some(width), Some(24.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_key() {
        let badge = ThresholdKeyBadge::new(2, 3);
        assert_eq!(badge.threshold, 2);
        assert_eq!(badge.total, 3);
    }
}
