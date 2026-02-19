//! Stack layer indicator for GitAfter stacked diffs.
//!
//! Shows the position in a stack (e.g., "2 of 4").

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Stack layer status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackLayerStatus {
    #[default]
    Pending,
    Ready,
    Merged,
    Blocked,
}

impl StackLayerStatus {
    pub fn color(&self) -> Hsla {
        match self {
            StackLayerStatus::Pending => Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
            StackLayerStatus::Ready => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            StackLayerStatus::Merged => Hsla::new(280.0, 0.7, 0.55, 1.0), // Purple
            StackLayerStatus::Blocked => Hsla::new(45.0, 0.9, 0.5, 1.0), // Gold
        }
    }
}

/// Badge showing stack layer position
pub struct StackLayerBadge {
    id: Option<ComponentId>,
    layer: u8,
    total: u8,
    status: StackLayerStatus,
    compact: bool,
}

impl StackLayerBadge {
    pub fn new(layer: u8, total: u8) -> Self {
        Self {
            id: None,
            layer,
            total,
            status: StackLayerStatus::Pending,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn status(mut self, status: StackLayerStatus) -> Self {
        self.status = status;
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Component for StackLayerBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.1, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;

        if self.compact {
            // Show just "2/4"
            let text = format!("{}/{}", self.layer, self.total);
            let text_x = bounds.origin.x + (bounds.size.width - text.len() as f32 * 6.0) / 2.0;
            let run = cx.text.layout_mono(
                &text,
                Point::new(text_x, text_y),
                theme::font_size::XS,
                color,
            );
            cx.scene.draw_text(run);
        } else {
            // Show "Layer 2 of 4"
            let padding = 8.0;
            let mut x = bounds.origin.x + padding;

            // Stack icon
            let icon_run = cx.text.layout_mono(
                "≡",
                Point::new(x, text_y - 1.0),
                theme::font_size::SM,
                color,
            );
            cx.scene.draw_text(icon_run);
            x += 14.0;

            // Layer number
            let layer_text = format!("{}", self.layer);
            let layer_run = cx.text.layout_mono(
                &layer_text,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(layer_run);
            x += layer_text.len() as f32 * 7.0 + 2.0;

            // "of"
            let of_run = cx.text.layout_mono(
                "of",
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(of_run);
            x += 16.0;

            // Total
            let total_text = format!("{}", self.total);
            let total_run = cx.text.layout_mono(
                &total_text,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(total_run);
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
            (Some(80.0), Some(24.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_layer() {
        let badge = StackLayerBadge::new(2, 4);
        assert_eq!(badge.layer, 2);
        assert_eq!(badge.total, 4);
    }
}
