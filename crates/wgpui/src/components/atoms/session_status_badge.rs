//! Session status badge for autopilot sessions.
//!
//! Shows the lifecycle status of autopilot sessions.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionStatus {
    #[default]
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Aborted,
}

impl SessionStatus {
    pub fn label(&self) -> &'static str {
        match self {
            SessionStatus::Pending => "Pending",
            SessionStatus::Running => "Running",
            SessionStatus::Paused => "Paused",
            SessionStatus::Completed => "Completed",
            SessionStatus::Failed => "Failed",
            SessionStatus::Aborted => "Aborted",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            SessionStatus::Pending => "○",
            SessionStatus::Running => "▶",
            SessionStatus::Paused => "❚❚",
            SessionStatus::Completed => "✓",
            SessionStatus::Failed => "✕",
            SessionStatus::Aborted => "⊘",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            SessionStatus::Pending => Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
            SessionStatus::Running => Hsla::new(200.0, 0.8, 0.55, 1.0), // Blue
            SessionStatus::Paused => Hsla::new(45.0, 0.7, 0.5, 1.0), // Gold
            SessionStatus::Completed => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            SessionStatus::Failed => Hsla::new(0.0, 0.8, 0.5, 1.0),  // Red
            SessionStatus::Aborted => Hsla::new(30.0, 0.7, 0.5, 1.0), // Orange
        }
    }

    /// Can this session be resumed?
    pub fn can_resume(&self) -> bool {
        matches!(self, SessionStatus::Paused)
    }

    /// Can this session be forked?
    pub fn can_fork(&self) -> bool {
        matches!(
            self,
            SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Aborted
        )
    }

    /// Is the session currently active?
    pub fn is_active(&self) -> bool {
        matches!(self, SessionStatus::Running | SessionStatus::Paused)
    }

    /// Is the session in a terminal state?
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Aborted
        )
    }
}

/// Badge showing session status
pub struct SessionStatusBadge {
    id: Option<ComponentId>,
    status: SessionStatus,
    duration_secs: Option<u64>,
    task_count: Option<u32>,
    compact: bool,
}

impl SessionStatusBadge {
    pub fn new(status: SessionStatus) -> Self {
        Self {
            id: None,
            status,
            duration_secs: None,
            task_count: None,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn duration(mut self, secs: u64) -> Self {
        self.duration_secs = Some(secs);
        self
    }

    pub fn task_count(mut self, count: u32) -> Self {
        self.task_count = Some(count);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

/// Format duration as HH:MM:SS or MM:SS
fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, mins, secs)
    } else {
        format!("{}:{:02}", mins, secs)
    }
}

impl Component for SessionStatusBadge {
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

            // Duration
            if let Some(secs) = self.duration_secs {
                let dur = format_duration(secs);
                let dur_run = cx.text.layout_mono(
                    &dur,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(dur_run);
                x += dur.len() as f32 * 6.5 + 6.0;
            }

            // Task count
            if let Some(count) = self.task_count {
                let count_text = format!("{} tasks", count);
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
            let mut width = 12.0 + 14.0 + self.status.label().len() as f32 * 6.5;
            if self.duration_secs.is_some() {
                width += 50.0;
            }
            if self.task_count.is_some() {
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
    fn test_session_status() {
        assert_eq!(SessionStatus::Running.label(), "Running");
        assert_eq!(SessionStatus::Completed.label(), "Completed");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(65), "1:05");
        assert_eq!(format_duration(3665), "1:01:05");
    }
}
