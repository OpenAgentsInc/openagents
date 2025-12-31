//! APM comparison card molecule for comparing two sessions.
//!
//! Shows side-by-side APM metrics for session comparison.

use crate::components::atoms::{ApmGauge, ApmLevel};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Comparison data for a single session
#[derive(Debug, Clone)]
pub struct ComparisonSession {
    pub id: String,
    pub title: String,
    pub apm: f32,
    pub level: ApmLevel,
    pub messages: u32,
    pub tool_calls: u32,
    pub duration_secs: u64,
}

impl ComparisonSession {
    pub fn new(id: impl Into<String>, title: impl Into<String>, apm: f32) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            apm,
            level: ApmLevel::from_apm(apm),
            messages: 0,
            tool_calls: 0,
            duration_secs: 0,
        }
    }

    pub fn messages(mut self, count: u32) -> Self {
        self.messages = count;
        self
    }

    pub fn tool_calls(mut self, count: u32) -> Self {
        self.tool_calls = count;
        self
    }

    pub fn duration(mut self, secs: u64) -> Self {
        self.duration_secs = secs;
        self
    }
}

/// A card comparing two sessions' APM metrics
pub struct ApmComparisonCard {
    id: Option<ComponentId>,
    session_a: ComparisonSession,
    session_b: ComparisonSession,
}

impl ApmComparisonCard {
    pub fn new(session_a: ComparisonSession, session_b: ComparisonSession) -> Self {
        Self {
            id: None,
            session_a,
            session_b,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    fn format_duration(secs: u64) -> String {
        let mins = secs / 60;
        let hours = mins / 60;
        if hours > 0 {
            format!("{}h {}m", hours, mins % 60)
        } else if mins > 0 {
            format!("{}m {}s", mins, secs % 60)
        } else {
            format!("{}s", secs)
        }
    }

    fn delta_color(delta: f32) -> Hsla {
        if delta > 0.0 {
            Hsla::new(120.0, 0.7, 0.45, 1.0) // Green
        } else if delta < 0.0 {
            Hsla::new(0.0, 0.8, 0.5, 1.0) // Red
        } else {
            theme::text::MUTED
        }
    }

    fn delta_symbol(delta: f32) -> &'static str {
        if delta > 0.0 { "+" } else { "" }
    }
}

impl Component for ApmComparisonCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Card background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = 16.0;
        let half_width = (bounds.size.width - padding * 3.0) / 2.0;
        let mut y = bounds.origin.y + padding;

        // Title
        let title_run = cx.text.layout(
            "Session Comparison",
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);
        y += 24.0;

        // Divider
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                1.0,
            ))
            .with_background(theme::border::DEFAULT),
        );
        y += 12.0;

        // Session headers
        let session_a_x = bounds.origin.x + padding;
        let session_b_x = bounds.origin.x + padding * 2.0 + half_width;

        // Session A title
        let title_a = if self.session_a.title.len() > 20 {
            format!("{}...", &self.session_a.title[..17])
        } else {
            self.session_a.title.clone()
        };
        let title_a_run = cx.text.layout(
            &title_a,
            Point::new(session_a_x, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(title_a_run);

        // Session B title
        let title_b = if self.session_b.title.len() > 20 {
            format!("{}...", &self.session_b.title[..17])
        } else {
            self.session_b.title.clone()
        };
        let title_b_run = cx.text.layout(
            &title_b,
            Point::new(session_b_x, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(title_b_run);
        y += 20.0;

        // APM Gauges
        let mut gauge_a = ApmGauge::new(self.session_a.apm);
        gauge_a.paint(Bounds::new(session_a_x, y, half_width, 32.0), cx);

        let mut gauge_b = ApmGauge::new(self.session_b.apm);
        gauge_b.paint(Bounds::new(session_b_x, y, half_width, 32.0), cx);
        y += 44.0;

        // APM Delta
        let apm_delta = self.session_b.apm - self.session_a.apm;
        let delta_text = format!(
            "{}APM: {}{:.1}",
            '\u{0394}',
            Self::delta_symbol(apm_delta),
            apm_delta
        );
        let center_x = bounds.origin.x + bounds.size.width / 2.0 - 40.0;
        let delta_run = cx.text.layout(
            &delta_text,
            Point::new(center_x, y),
            theme::font_size::SM,
            Self::delta_color(apm_delta),
        );
        cx.scene.draw_text(delta_run);
        y += 28.0;

        // Metrics comparison
        let metrics = [
            ("Messages", self.session_a.messages, self.session_b.messages),
            (
                "Tool Calls",
                self.session_a.tool_calls,
                self.session_b.tool_calls,
            ),
        ];

        for (label, val_a, val_b) in metrics {
            // Label
            let label_run = cx.text.layout(
                label,
                Point::new(session_a_x, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);

            // Value A
            let val_a_run = cx.text.layout(
                &val_a.to_string(),
                Point::new(session_a_x + 80.0, y),
                theme::font_size::SM,
                self.session_a.level.color(),
            );
            cx.scene.draw_text(val_a_run);

            // Value B
            let val_b_run = cx.text.layout(
                &val_b.to_string(),
                Point::new(session_b_x + 80.0, y),
                theme::font_size::SM,
                self.session_b.level.color(),
            );
            cx.scene.draw_text(val_b_run);

            y += 20.0;
        }

        // Duration comparison
        let dur_label = cx.text.layout(
            "Duration",
            Point::new(session_a_x, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(dur_label);

        let dur_a = Self::format_duration(self.session_a.duration_secs);
        let dur_a_run = cx.text.layout(
            &dur_a,
            Point::new(session_a_x + 80.0, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(dur_a_run);

        let dur_b = Self::format_duration(self.session_b.duration_secs);
        let dur_b_run = cx.text.layout(
            &dur_b,
            Point::new(session_b_x + 80.0, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(dur_b_run);
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
        (None, Some(200.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_session() {
        let session = ComparisonSession::new("sess-1", "Build feature", 75.0)
            .messages(50)
            .tool_calls(30)
            .duration(1800);

        assert_eq!(session.messages, 50);
        assert_eq!(session.tool_calls, 30);
        assert_eq!(session.duration_secs, 1800);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(ApmComparisonCard::format_duration(45), "45s");
        assert_eq!(ApmComparisonCard::format_duration(125), "2m 5s");
        assert_eq!(ApmComparisonCard::format_duration(3665), "1h 1m");
    }

    #[test]
    fn test_delta_color() {
        // Positive delta should be green
        let green = ApmComparisonCard::delta_color(10.0);
        assert!(green.s > 0.5); // High saturation means colored

        // Negative delta should be red
        let red = ApmComparisonCard::delta_color(-10.0);
        assert!(red.h < 1.0 || red.h > 350.0); // Red hue

        // Zero delta should be muted
        let muted = ApmComparisonCard::delta_color(0.0);
        assert!(muted.s < 0.5); // Low saturation means muted
    }
}
