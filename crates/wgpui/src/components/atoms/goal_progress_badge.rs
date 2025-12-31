//! Goal progress badge for NIP-SA Sovereign Agents.
//!
//! Shows progress towards agent goals.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Goal priority level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoalPriority {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

impl GoalPriority {
    pub fn label(&self) -> &'static str {
        match self {
            GoalPriority::Low => "Low",
            GoalPriority::Medium => "Med",
            GoalPriority::High => "High",
            GoalPriority::Critical => "Critical",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            GoalPriority::Low => Hsla::new(200.0, 0.5, 0.5, 1.0), // Blue
            GoalPriority::Medium => Hsla::new(45.0, 0.7, 0.5, 1.0), // Gold
            GoalPriority::High => Hsla::new(30.0, 0.8, 0.5, 1.0), // Orange
            GoalPriority::Critical => Hsla::new(0.0, 0.8, 0.5, 1.0), // Red
        }
    }
}

/// Goal status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoalStatus {
    #[default]
    NotStarted,
    InProgress,
    Blocked,
    Completed,
    Failed,
}

impl GoalStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            GoalStatus::NotStarted => "○",
            GoalStatus::InProgress => "◐",
            GoalStatus::Blocked => "⊘",
            GoalStatus::Completed => "●",
            GoalStatus::Failed => "✕",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            GoalStatus::NotStarted => Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
            GoalStatus::InProgress => Hsla::new(200.0, 0.8, 0.55, 1.0), // Blue
            GoalStatus::Blocked => Hsla::new(45.0, 0.8, 0.5, 1.0),   // Gold
            GoalStatus::Completed => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            GoalStatus::Failed => Hsla::new(0.0, 0.8, 0.5, 1.0),     // Red
        }
    }
}

/// Badge showing goal progress
pub struct GoalProgressBadge {
    id: Option<ComponentId>,
    progress: f32, // 0.0 to 1.0
    status: GoalStatus,
    priority: GoalPriority,
    show_percentage: bool,
}

impl GoalProgressBadge {
    pub fn new(progress: f32) -> Self {
        Self {
            id: None,
            progress: progress.clamp(0.0, 1.0),
            status: GoalStatus::InProgress,
            priority: GoalPriority::Medium,
            show_percentage: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn status(mut self, status: GoalStatus) -> Self {
        self.status = status;
        self
    }

    pub fn priority(mut self, priority: GoalPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn show_percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }
}

impl Component for GoalProgressBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let status_color = self.status.color();
        let bg = Hsla::new(0.0, 0.0, 0.1, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(status_color, 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let mut x = bounds.origin.x + padding;

        // Status icon
        let icon = self.status.icon();
        let icon_run = cx.text.layout(
            icon,
            Point::new(x, text_y),
            theme::font_size::SM,
            status_color,
        );
        cx.scene.draw_text(icon_run);
        x += 14.0;

        // Progress bar
        let bar_width = 60.0;
        let bar_height = 6.0;
        let bar_y = bounds.origin.y + (bounds.size.height - bar_height) / 2.0;

        // Background track
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x, bar_y, bar_width, bar_height))
                .with_background(Hsla::new(0.0, 0.0, 0.2, 1.0)),
        );

        // Progress fill
        let fill_width = bar_width * self.progress;
        if fill_width > 0.0 {
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x, bar_y, fill_width, bar_height))
                    .with_background(status_color),
            );
        }
        x += bar_width + 6.0;

        // Percentage
        if self.show_percentage {
            let percent = format!("{}%", (self.progress * 100.0) as u8);
            let percent_run = cx.text.layout(
                &percent,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(percent_run);
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
        let mut width = 86.0; // icon + bar + padding
        if self.show_percentage {
            width += 36.0;
        }
        (Some(width), Some(22.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_progress() {
        let badge = GoalProgressBadge::new(0.75);
        assert_eq!(badge.progress, 0.75);
    }

    #[test]
    fn test_progress_clamping() {
        let badge_over = GoalProgressBadge::new(1.5);
        assert_eq!(badge_over.progress, 1.0);

        let badge_under = GoalProgressBadge::new(-0.5);
        assert_eq!(badge_under.progress, 0.0);
    }
}
