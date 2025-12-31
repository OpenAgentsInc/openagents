//! PR/Patch status badge for GitAfter (NIP-34).
//!
//! Displays the status of a pull request or patch.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Pull request status (kind:1630-1633)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrStatus {
    #[default]
    Draft,
    Open,
    Merged,
    Closed,
    NeedsReview,
    Approved,
    ChangesRequested,
}

impl PrStatus {
    pub fn label(&self) -> &'static str {
        match self {
            PrStatus::Draft => "Draft",
            PrStatus::Open => "Open",
            PrStatus::Merged => "Merged",
            PrStatus::Closed => "Closed",
            PrStatus::NeedsReview => "Review",
            PrStatus::Approved => "Approved",
            PrStatus::ChangesRequested => "Changes",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            PrStatus::Draft => Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
            PrStatus::Open => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            PrStatus::Merged => Hsla::new(280.0, 0.7, 0.55, 1.0), // Purple
            PrStatus::Closed => Hsla::new(0.0, 0.7, 0.5, 1.0), // Red
            PrStatus::NeedsReview => Hsla::new(45.0, 0.9, 0.5, 1.0), // Gold
            PrStatus::Approved => Hsla::new(140.0, 0.8, 0.45, 1.0), // Bright green
            PrStatus::ChangesRequested => Hsla::new(30.0, 0.9, 0.5, 1.0), // Orange
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PrStatus::Draft => "◌",
            PrStatus::Open => "↗",
            PrStatus::Merged => "✓",
            PrStatus::Closed => "✕",
            PrStatus::NeedsReview => "◎",
            PrStatus::Approved => "✓",
            PrStatus::ChangesRequested => "!",
        }
    }
}

/// Badge displaying PR status
pub struct PrStatusBadge {
    id: Option<ComponentId>,
    status: PrStatus,
    compact: bool,
}

impl PrStatusBadge {
    pub fn new(status: PrStatus) -> Self {
        Self {
            id: None,
            status,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Component for PrStatusBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        if self.compact {
            let icon = self.status.icon();
            let run = cx.text.layout(
                icon,
                Point::new(bounds.origin.x + 6.0, bounds.origin.y + 4.0),
                theme::font_size::SM,
                color,
            );
            cx.scene.draw_text(run);
        } else {
            let label = self.status.label();
            let text_w = label.len() as f32 * 6.5;
            let text_x = bounds.origin.x + (bounds.size.width - text_w) / 2.0;
            let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
            let run = cx.text.layout(
                label,
                Point::new(text_x, text_y),
                theme::font_size::XS,
                color,
            );
            cx.scene.draw_text(run);
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
            (Some(24.0), Some(22.0))
        } else {
            (Some(70.0), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pr_status() {
        assert_eq!(PrStatus::Merged.label(), "Merged");
        assert_eq!(PrStatus::Open.label(), "Open");
    }
}
