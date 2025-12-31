//! Job status badge for marketplace compute jobs.
//!
//! Shows the status of NIP-90 DVM compute jobs.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Job status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JobStatus {
    #[default]
    Pending,
    Processing,
    Streaming,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn label(&self) -> &'static str {
        match self {
            JobStatus::Pending => "Pending",
            JobStatus::Processing => "Processing",
            JobStatus::Streaming => "Streaming",
            JobStatus::Completed => "Completed",
            JobStatus::Failed => "Failed",
            JobStatus::Cancelled => "Cancelled",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            JobStatus::Pending => "○",
            JobStatus::Processing => "◐",
            JobStatus::Streaming => "⋯",
            JobStatus::Completed => "●",
            JobStatus::Failed => "✕",
            JobStatus::Cancelled => "⊘",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            JobStatus::Pending => Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
            JobStatus::Processing => Hsla::new(200.0, 0.8, 0.55, 1.0), // Blue
            JobStatus::Streaming => Hsla::new(180.0, 0.7, 0.5, 1.0), // Cyan
            JobStatus::Completed => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            JobStatus::Failed => Hsla::new(0.0, 0.8, 0.5, 1.0),  // Red
            JobStatus::Cancelled => Hsla::new(30.0, 0.7, 0.5, 1.0), // Orange
        }
    }
}

/// Badge showing job status
pub struct JobStatusBadge {
    id: Option<ComponentId>,
    status: JobStatus,
    tokens: Option<(u32, u32)>, // (input, output)
    cost_sats: Option<u32>,
    compact: bool,
}

impl JobStatusBadge {
    pub fn new(status: JobStatus) -> Self {
        Self {
            id: None,
            status,
            tokens: None,
            cost_sats: None,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn tokens(mut self, input: u32, output: u32) -> Self {
        self.tokens = Some((input, output));
        self
    }

    pub fn cost_sats(mut self, sats: u32) -> Self {
        self.cost_sats = Some(sats);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Component for JobStatusBadge {
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
        let mut x = bounds.origin.x + padding;

        // Icon
        let icon = self.status.icon();
        let icon_run = cx
            .text
            .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
        cx.scene.draw_text(icon_run);

        if !self.compact {
            x += 14.0;
            // Label
            let label = self.status.label();
            let label_run =
                cx.text
                    .layout(label, Point::new(x, text_y), theme::font_size::XS, color);
            cx.scene.draw_text(label_run);
            x += label.len() as f32 * 6.5 + 8.0;

            // Cost
            if let Some(sats) = self.cost_sats {
                let cost_text = format!("{}sats", sats);
                let cost_run = cx.text.layout(
                    &cost_text,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(cost_run);
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
            let mut width = 12.0 + 14.0 + self.status.label().len() as f32 * 6.5;
            if self.cost_sats.is_some() {
                width += 60.0;
            }
            (Some(width), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_status() {
        assert_eq!(JobStatus::Completed.label(), "Completed");
        assert_eq!(JobStatus::Processing.label(), "Processing");
    }
}
