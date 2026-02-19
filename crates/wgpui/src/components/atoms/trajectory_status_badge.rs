//! Trajectory status badge for NIP-SA.
//!
//! Shows the verification status of an agent's work trajectory.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Trajectory verification status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrajectoryStatus {
    #[default]
    Unknown,
    Verified,
    Partial,
    Suspicious,
    HasGaps,
    Mismatch,
}

impl TrajectoryStatus {
    pub fn label(&self) -> &'static str {
        match self {
            TrajectoryStatus::Unknown => "Unknown",
            TrajectoryStatus::Verified => "Verified",
            TrajectoryStatus::Partial => "Partial",
            TrajectoryStatus::Suspicious => "Suspicious",
            TrajectoryStatus::HasGaps => "Has Gaps",
            TrajectoryStatus::Mismatch => "Mismatch",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            TrajectoryStatus::Unknown => Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
            TrajectoryStatus::Verified => Hsla::new(120.0, 0.8, 0.45, 1.0), // Green
            TrajectoryStatus::Partial => Hsla::new(45.0, 0.8, 0.5, 1.0), // Gold
            TrajectoryStatus::Suspicious => Hsla::new(30.0, 0.9, 0.5, 1.0), // Orange
            TrajectoryStatus::HasGaps => Hsla::new(45.0, 0.7, 0.5, 1.0), // Yellow
            TrajectoryStatus::Mismatch => Hsla::new(0.0, 0.8, 0.5, 1.0), // Red
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            TrajectoryStatus::Unknown => "?",
            TrajectoryStatus::Verified => "✓",
            TrajectoryStatus::Partial => "◐",
            TrajectoryStatus::Suspicious => "!",
            TrajectoryStatus::HasGaps => "⋯",
            TrajectoryStatus::Mismatch => "✕",
        }
    }
}

/// Badge displaying trajectory verification status
pub struct TrajectoryStatusBadge {
    id: Option<ComponentId>,
    status: TrajectoryStatus,
    show_icon: bool,
    compact: bool,
}

impl TrajectoryStatusBadge {
    pub fn new(status: TrajectoryStatus) -> Self {
        Self {
            id: None,
            status,
            show_icon: true,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show_icon(mut self, show: bool) -> Self {
        self.show_icon = show;
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Component for TrajectoryStatusBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;

        if self.compact {
            // Just icon
            let icon = self.status.icon();
            let run = cx.text.layout_mono(
                icon,
                Point::new(bounds.origin.x + padding, text_y),
                theme::font_size::SM,
                color,
            );
            cx.scene.draw_text(run);
        } else {
            let mut x = bounds.origin.x + padding;

            // Icon
            if self.show_icon {
                let icon = self.status.icon();
                let icon_run =
                    cx.text
                        .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
                cx.scene.draw_text(icon_run);
                x += 14.0;
            }

            // Label
            let label = self.status.label();
            let label_run =
                cx.text
                    .layout(label, Point::new(x, text_y), theme::font_size::XS, color);
            cx.scene.draw_text(label_run);
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
            let mut width = 12.0;
            if self.show_icon {
                width += 14.0;
            }
            width += self.status.label().len() as f32 * 6.5;
            (Some(width), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_status() {
        assert_eq!(TrajectoryStatus::Verified.label(), "Verified");
        assert_eq!(TrajectoryStatus::HasGaps.label(), "Has Gaps");
    }
}
