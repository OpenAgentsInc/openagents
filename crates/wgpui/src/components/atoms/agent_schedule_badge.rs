//! Agent schedule badge for NIP-SA.
//!
//! Shows the agent's heartbeat interval and trigger configuration.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Trigger types for agent activation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    Heartbeat,
    Mention,
    DirectMessage,
    Zap,
    Issue,
    PullRequest,
}

impl TriggerType {
    pub fn label(&self) -> &'static str {
        match self {
            TriggerType::Heartbeat => "Heartbeat",
            TriggerType::Mention => "Mention",
            TriggerType::DirectMessage => "DM",
            TriggerType::Zap => "Zap",
            TriggerType::Issue => "Issue",
            TriggerType::PullRequest => "PR",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            TriggerType::Heartbeat => "♥",
            TriggerType::Mention => "@",
            TriggerType::DirectMessage => "✉",
            TriggerType::Zap => "⚡",
            TriggerType::Issue => "●",
            TriggerType::PullRequest => "⎇",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            TriggerType::Heartbeat => Hsla::new(0.0, 0.8, 0.55, 1.0), // Red
            TriggerType::Mention => Hsla::new(200.0, 0.7, 0.5, 1.0),  // Blue
            TriggerType::DirectMessage => Hsla::new(280.0, 0.6, 0.5, 1.0), // Purple
            TriggerType::Zap => Hsla::new(45.0, 0.9, 0.5, 1.0),       // Gold
            TriggerType::Issue => Hsla::new(120.0, 0.7, 0.45, 1.0),   // Green
            TriggerType::PullRequest => Hsla::new(30.0, 0.8, 0.5, 1.0), // Orange
        }
    }
}

/// Badge showing agent schedule configuration
pub struct AgentScheduleBadge {
    id: Option<ComponentId>,
    heartbeat_seconds: u32,
    triggers: Vec<TriggerType>,
    compact: bool,
}

impl AgentScheduleBadge {
    pub fn new(heartbeat_seconds: u32) -> Self {
        Self {
            id: None,
            heartbeat_seconds,
            triggers: Vec::new(),
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn triggers(mut self, triggers: Vec<TriggerType>) -> Self {
        self.triggers = triggers;
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    fn format_interval(&self) -> String {
        let secs = self.heartbeat_seconds;
        if secs >= 3600 {
            format!("{}h", secs / 3600)
        } else if secs >= 60 {
            format!("{}m", secs / 60)
        } else {
            format!("{}s", secs)
        }
    }
}

impl Component for AgentScheduleBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let bg = Hsla::new(0.0, 0.0, 0.1, 0.95);
        let border_color = Hsla::new(200.0, 0.5, 0.4, 1.0);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(border_color, 1.0),
        );

        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let padding = 6.0;
        let mut x = bounds.origin.x + padding;

        if self.compact {
            // Just heartbeat interval
            let interval = self.format_interval();
            let heartbeat_color = TriggerType::Heartbeat.color();
            let run = cx.text.layout_mono(
                "♥",
                Point::new(x, text_y),
                theme::font_size::SM,
                heartbeat_color,
            );
            cx.scene.draw_text(run);
            x += 14.0;
            let run = cx.text.layout_mono(
                &interval,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(run);
        } else {
            // Heartbeat
            let heartbeat_color = TriggerType::Heartbeat.color();
            let run = cx.text.layout_mono(
                "♥",
                Point::new(x, text_y),
                theme::font_size::SM,
                heartbeat_color,
            );
            cx.scene.draw_text(run);
            x += 14.0;

            let interval = self.format_interval();
            let run = cx.text.layout_mono(
                &interval,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(run);
            x += interval.len() as f32 * 7.0 + 12.0;

            // Triggers
            for trigger in &self.triggers {
                if trigger != &TriggerType::Heartbeat {
                    let icon = trigger.icon();
                    let color = trigger.color();
                    let run =
                        cx.text
                            .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
                    cx.scene.draw_text(run);
                    x += 16.0;
                }
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
            (Some(50.0), Some(22.0))
        } else {
            let trigger_width = self
                .triggers
                .iter()
                .filter(|t| **t != TriggerType::Heartbeat)
                .count() as f32
                * 16.0;
            (Some(70.0 + trigger_width), Some(24.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_badge() {
        let badge = AgentScheduleBadge::new(900);
        assert_eq!(badge.format_interval(), "15m");
    }

    #[test]
    fn test_format_intervals() {
        let badge_secs = AgentScheduleBadge::new(30);
        assert_eq!(badge_secs.format_interval(), "30s");

        let badge_hours = AgentScheduleBadge::new(7200);
        assert_eq!(badge_hours.format_interval(), "2h");
    }
}
