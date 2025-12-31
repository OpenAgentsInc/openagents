//! Tick event badge for NIP-SA Sovereign Agents.
//!
//! Shows tick request/result status.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Tick event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TickEventType {
    #[default]
    Request,
    Result,
}

/// Tick result outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TickOutcome {
    #[default]
    Pending,
    Success,
    Failure,
    Timeout,
    Skipped,
}

impl TickOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            TickOutcome::Pending => "Pending",
            TickOutcome::Success => "Success",
            TickOutcome::Failure => "Failure",
            TickOutcome::Timeout => "Timeout",
            TickOutcome::Skipped => "Skipped",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            TickOutcome::Pending => "◐",
            TickOutcome::Success => "✓",
            TickOutcome::Failure => "✕",
            TickOutcome::Timeout => "⏱",
            TickOutcome::Skipped => "→",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            TickOutcome::Pending => Hsla::new(200.0, 0.7, 0.55, 1.0), // Blue
            TickOutcome::Success => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            TickOutcome::Failure => Hsla::new(0.0, 0.8, 0.5, 1.0),    // Red
            TickOutcome::Timeout => Hsla::new(45.0, 0.8, 0.5, 1.0),   // Gold
            TickOutcome::Skipped => Hsla::new(0.0, 0.0, 0.5, 1.0),    // Gray
        }
    }
}

/// Badge showing tick event status
pub struct TickEventBadge {
    id: Option<ComponentId>,
    event_type: TickEventType,
    outcome: TickOutcome,
    duration_ms: Option<u64>,
    compact: bool,
}

impl TickEventBadge {
    pub fn request() -> Self {
        Self {
            id: None,
            event_type: TickEventType::Request,
            outcome: TickOutcome::Pending,
            duration_ms: None,
            compact: false,
        }
    }

    pub fn result(outcome: TickOutcome) -> Self {
        Self {
            id: None,
            event_type: TickEventType::Result,
            outcome,
            duration_ms: None,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    fn format_duration(&self) -> Option<String> {
        self.duration_ms.map(|ms| {
            if ms >= 60000 {
                format!("{}m", ms / 60000)
            } else if ms >= 1000 {
                format!("{:.1}s", ms as f64 / 1000.0)
            } else {
                format!("{}ms", ms)
            }
        })
    }
}

impl Component for TickEventBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.outcome.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let mut x = bounds.origin.x + padding;

        if self.compact {
            // Just icon
            let icon = self.outcome.icon();
            let run = cx
                .text
                .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
            cx.scene.draw_text(run);
        } else {
            // Type indicator
            let type_icon = match self.event_type {
                TickEventType::Request => "→",
                TickEventType::Result => "←",
            };
            let type_run = cx.text.layout(
                type_icon,
                Point::new(x, text_y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(type_run);
            x += 14.0;

            // Outcome icon
            let icon = self.outcome.icon();
            let icon_run = cx
                .text
                .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
            cx.scene.draw_text(icon_run);
            x += 14.0;

            // Label
            let label = self.outcome.label();
            let label_run =
                cx.text
                    .layout(label, Point::new(x, text_y), theme::font_size::XS, color);
            cx.scene.draw_text(label_run);
            x += label.len() as f32 * 6.5 + 8.0;

            // Duration
            if let Some(duration) = self.format_duration() {
                let dur_run = cx.text.layout(
                    &duration,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(dur_run);
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
            let mut width = 12.0 + 14.0 + 14.0 + self.outcome.label().len() as f32 * 6.5;
            if self.duration_ms.is_some() {
                width += 45.0;
            }
            (Some(width), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_event() {
        let request = TickEventBadge::request();
        assert_eq!(request.event_type, TickEventType::Request);

        let result = TickEventBadge::result(TickOutcome::Success);
        assert_eq!(result.outcome, TickOutcome::Success);
    }

    #[test]
    fn test_duration_format() {
        let badge = TickEventBadge::result(TickOutcome::Success).duration_ms(1500);
        assert_eq!(badge.format_duration(), Some("1.5s".to_string()));

        let badge_ms = TickEventBadge::result(TickOutcome::Success).duration_ms(500);
        assert_eq!(badge_ms.format_duration(), Some("500ms".to_string()));
    }
}
