//! APM session row molecule for displaying session APM metrics.
//!
//! Shows session title, APM score, and tier in a compact row format.

use crate::components::atoms::{ApmGauge, ApmLevel, SessionStatus, SessionStatusBadge};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

/// APM session data for display
#[derive(Debug, Clone)]
pub struct ApmSessionData {
    pub id: String,
    pub title: String,
    pub apm: f32,
    pub level: ApmLevel,
    pub status: SessionStatus,
    pub duration_secs: Option<u64>,
    pub rank: Option<u32>,
}

impl ApmSessionData {
    pub fn new(id: impl Into<String>, title: impl Into<String>, apm: f32) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            apm,
            level: ApmLevel::from_apm(apm),
            status: SessionStatus::Completed,
            duration_secs: None,
            rank: None,
        }
    }

    pub fn status(mut self, status: SessionStatus) -> Self {
        self.status = status;
        self
    }

    pub fn duration(mut self, secs: u64) -> Self {
        self.duration_secs = Some(secs);
        self
    }

    pub fn rank(mut self, rank: u32) -> Self {
        self.rank = Some(rank);
        self
    }
}

/// A row displaying session APM metrics
pub struct ApmSessionRow {
    id: Option<ComponentId>,
    session: ApmSessionData,
    hovered: bool,
    on_click: Option<Box<dyn FnMut(String)>>,
}

impl ApmSessionRow {
    pub fn new(session: ApmSessionData) -> Self {
        Self {
            id: None,
            session,
            hovered: false,
            on_click: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    pub fn session(&self) -> &ApmSessionData {
        &self.session
    }

    fn format_duration(secs: u64) -> String {
        let mins = secs / 60;
        let hours = mins / 60;
        if hours > 0 {
            format!("{}h {}m", hours, mins % 60)
        } else if mins > 0 {
            format!("{}m", mins)
        } else {
            format!("{}s", secs)
        }
    }
}

impl Component for ApmSessionRow {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let level_color = self.session.level.color();
        let bg = if self.hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };

        // Row background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = 12.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::SM) / 2.0;
        let mut x = bounds.origin.x + padding;

        // Rank (if present)
        if let Some(rank) = self.session.rank {
            let rank_text = format!("#{}", rank);
            let rank_run = cx.text.layout(
                &rank_text,
                Point::new(x, text_y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(rank_run);
            x += 40.0;
        }

        // APM Gauge (compact)
        let mut gauge = ApmGauge::new(self.session.apm).compact(true);
        gauge.paint(
            Bounds::new(
                x,
                bounds.origin.y + (bounds.size.height - 24.0) / 2.0,
                50.0,
                24.0,
            ),
            cx,
        );
        x += 60.0;

        // Title
        let max_title_width = 200.0;
        let title = if self.session.title.len() > 30 {
            format!("{}...", &self.session.title[..27])
        } else {
            self.session.title.clone()
        };
        let title_run = cx.text.layout(
            &title,
            Point::new(x, text_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);
        x += max_title_width + 16.0;

        // Status badge (compact)
        let mut status = SessionStatusBadge::new(self.session.status).compact(true);
        status.paint(
            Bounds::new(
                x,
                bounds.origin.y + (bounds.size.height - 22.0) / 2.0,
                28.0,
                22.0,
            ),
            cx,
        );
        x += 36.0;

        // Duration
        if let Some(secs) = self.session.duration_secs {
            let dur_text = Self::format_duration(secs);
            let dur_run = cx.text.layout(
                &dur_text,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(dur_run);
        }

        // APM value (right aligned)
        let apm_text = format!("{:.0} APM", self.session.apm);
        let apm_run = cx.text.layout(
            &apm_text,
            Point::new(bounds.origin.x + bounds.size.width - padding - 80.0, text_y),
            theme::font_size::SM,
            level_color,
        );
        cx.scene.draw_text(apm_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    if let Some(callback) = &mut self.on_click {
                        callback(self.session.id.clone());
                    }
                    return EventResult::Handled;
                }
            }
            _ => {}
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(44.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apm_session_data() {
        let data = ApmSessionData::new("sess-1", "Build feature X", 85.0)
            .status(SessionStatus::Completed)
            .duration(3600)
            .rank(1);

        assert_eq!(data.id, "sess-1");
        assert_eq!(data.apm, 85.0);
        assert_eq!(data.rank, Some(1));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(ApmSessionRow::format_duration(45), "45s");
        assert_eq!(ApmSessionRow::format_duration(125), "2m");
        assert_eq!(ApmSessionRow::format_duration(3665), "1h 1m");
    }

    #[test]
    fn test_apm_session_row() {
        let data = ApmSessionData::new("1", "Test", 50.0);
        let row = ApmSessionRow::new(data);
        assert!(!row.hovered);
    }
}
