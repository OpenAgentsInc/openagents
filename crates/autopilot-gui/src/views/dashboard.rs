use std::cell::RefCell;
use std::rc::Rc;

use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext, Text, theme};

use crate::state::AppState;
use crate::views::fit_text;

pub struct DashboardView {
    state: Rc<RefCell<AppState>>,
}

impl DashboardView {
    pub fn new(state: Rc<RefCell<AppState>>) -> Self {
        Self { state }
    }
}

impl Component for DashboardView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let mut y = bounds.origin.y + padding;
        let line_height = theme::font_size::SM * 1.5;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        let summary_lines = [
            format!("Total sessions: {}", state.summary.total_sessions),
            format!("Issues completed: {}", state.summary.total_issues_completed),
            format!("Total cost: ${:.3}", state.summary.total_cost_usd),
            format!("Completion rate: {:.1}%", state.summary.completion_rate * 100.0),
            format!("Avg duration: {:.1}s", state.summary.avg_duration_seconds),
            format!("Avg tokens: {:.0}", state.summary.avg_tokens_per_session),
        ];

        for line in summary_lines {
            let line = fit_text(cx, &line, theme::font_size::SM, available_width);
            let mut text = Text::new(line)
                .font_size(theme::font_size::SM)
                .color(theme::text::PRIMARY);
            text.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    line_height,
                ),
                cx,
            );
            y += line_height;
        }

        y += theme::spacing::SM;
        let header_line = format!("Recent sessions: {}", state.sessions.len());
        let header_line = fit_text(cx, &header_line, theme::font_size::XS, available_width);
        let mut header = Text::new(header_line)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        header.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height,
            ),
            cx,
        );
        y += line_height;

        for session in state.sessions.iter().take(6) {
            let id = session.id.chars().take(8).collect::<String>();
            let line = format!(
                "{}  {}  ${:.3}",
                id,
                format!("{:?}", session.final_status),
                session.cost_usd
            );
            let line = fit_text(cx, &line, theme::font_size::XS, available_width);
            let mut text = Text::new(line)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            text.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    line_height,
                ),
                cx,
            );
            y += line_height;
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
}
