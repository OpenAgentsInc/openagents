//! Trajectory source badge for marketplace data contribution.
//!
//! Shows the source of trajectory data (Codex, Cursor, Windsurf, etc.).

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Trajectory data source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrajectorySource {
    #[default]
    Codex,
    Cursor,
    Windsurf,
    Custom,
}

impl TrajectorySource {
    pub fn label(&self) -> &'static str {
        match self {
            TrajectorySource::Codex => "Codex",
            TrajectorySource::Cursor => "Cursor",
            TrajectorySource::Windsurf => "Windsurf",
            TrajectorySource::Custom => "Custom",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            TrajectorySource::Codex => "⟨⟩",
            TrajectorySource::Cursor => "▸",
            TrajectorySource::Windsurf => "~",
            TrajectorySource::Custom => "◇",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            TrajectorySource::Codex => Hsla::new(160.0, 0.6, 0.45, 1.0), // Teal (OpenAI)
            TrajectorySource::Cursor => Hsla::new(200.0, 0.7, 0.5, 1.0), // Blue
            TrajectorySource::Windsurf => Hsla::new(220.0, 0.7, 0.55, 1.0), // Indigo
            TrajectorySource::Custom => Hsla::new(0.0, 0.0, 0.5, 1.0),   // Gray
        }
    }
}

/// Contribution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContributionStatus {
    #[default]
    Scanned,
    Redacted,
    Pending,
    Accepted,
    Rejected,
}

impl ContributionStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ContributionStatus::Scanned => "Scanned",
            ContributionStatus::Redacted => "Redacted",
            ContributionStatus::Pending => "Pending",
            ContributionStatus::Accepted => "Accepted",
            ContributionStatus::Rejected => "Rejected",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            ContributionStatus::Scanned => Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
            ContributionStatus::Redacted => Hsla::new(45.0, 0.7, 0.5, 1.0), // Gold
            ContributionStatus::Pending => Hsla::new(200.0, 0.7, 0.55, 1.0), // Blue
            ContributionStatus::Accepted => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            ContributionStatus::Rejected => Hsla::new(0.0, 0.8, 0.5, 1.0), // Red
        }
    }
}

/// Badge showing trajectory source
pub struct TrajectorySourceBadge {
    id: Option<ComponentId>,
    source: TrajectorySource,
    status: Option<ContributionStatus>,
    session_count: Option<u32>,
    compact: bool,
}

impl TrajectorySourceBadge {
    pub fn new(source: TrajectorySource) -> Self {
        Self {
            id: None,
            source,
            status: None,
            session_count: None,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn status(mut self, status: ContributionStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn session_count(mut self, count: u32) -> Self {
        self.session_count = Some(count);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Component for TrajectorySourceBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.source.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let mut x = bounds.origin.x + padding;

        // Icon
        let icon = self.source.icon();
        let icon_run = cx
            .text
            .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
        cx.scene.draw_text(icon_run);

        if !self.compact {
            x += 14.0;
            // Label
            let label = self.source.label();
            let label_run =
                cx.text
                    .layout(label, Point::new(x, text_y), theme::font_size::XS, color);
            cx.scene.draw_text(label_run);
            x += label.len() as f32 * 6.5 + 8.0;

            // Status
            if let Some(status) = self.status {
                let status_color = status.color();
                let status_label = status.label();
                let status_run = cx.text.layout_mono(
                    status_label,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    status_color,
                );
                cx.scene.draw_text(status_run);
                x += status_label.len() as f32 * 6.5 + 6.0;
            }

            // Session count
            if let Some(count) = self.session_count {
                let count_text = format!("({})", count);
                let count_run = cx.text.layout_mono(
                    &count_text,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(count_run);
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
            (Some(28.0), Some(22.0))
        } else {
            let mut width = 12.0 + 14.0 + self.source.label().len() as f32 * 6.5;
            if let Some(status) = self.status {
                width += status.label().len() as f32 * 6.5 + 8.0;
            }
            if self.session_count.is_some() {
                width += 30.0;
            }
            (Some(width), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_source() {
        assert_eq!(TrajectorySource::Codex.label(), "Codex");
        assert_eq!(TrajectorySource::Cursor.label(), "Cursor");
    }
}
