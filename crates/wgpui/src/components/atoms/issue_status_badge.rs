//! Issue status badge for GitAfter (NIP-34).
//!
//! Displays the status of a git issue with appropriate styling.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Git issue status (kind:1630-1633)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IssueStatus {
    #[default]
    Open,
    Closed,
    Claimed,
    InProgress,
    Draft,
}

impl IssueStatus {
    pub fn label(&self) -> &'static str {
        match self {
            IssueStatus::Open => "Open",
            IssueStatus::Closed => "Closed",
            IssueStatus::Claimed => "Claimed",
            IssueStatus::InProgress => "In Progress",
            IssueStatus::Draft => "Draft",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            IssueStatus::Open => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            IssueStatus::Closed => Hsla::new(280.0, 0.6, 0.5, 1.0), // Purple
            IssueStatus::Claimed => Hsla::new(45.0, 0.9, 0.5, 1.0), // Gold
            IssueStatus::InProgress => Hsla::new(200.0, 0.8, 0.5, 1.0), // Blue
            IssueStatus::Draft => Hsla::new(0.0, 0.0, 0.5, 1.0),   // Gray
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            IssueStatus::Open => "○",
            IssueStatus::Closed => "●",
            IssueStatus::Claimed => "◎",
            IssueStatus::InProgress => "◐",
            IssueStatus::Draft => "◌",
        }
    }
}

/// Badge displaying issue status
pub struct IssueStatusBadge {
    id: Option<ComponentId>,
    status: IssueStatus,
    compact: bool,
}

impl IssueStatusBadge {
    pub fn new(status: IssueStatus) -> Self {
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

impl Component for IssueStatusBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        if self.compact {
            // Just icon
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
            (Some(80.0), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_status() {
        assert_eq!(IssueStatus::Open.label(), "Open");
        assert_eq!(IssueStatus::Claimed.label(), "Claimed");
    }
}
