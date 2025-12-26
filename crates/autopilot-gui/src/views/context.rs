use std::cell::RefCell;
use std::rc::Rc;

use autopilot::metrics::SessionStatus as MetricsStatus;
use wgpui::components::atoms::{SessionStatus as UiSessionStatus, SessionStatusBadge};
use wgpui::components::molecules::SessionSearchBar;
use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext, Text, theme};

use crate::state::AppState;
use crate::views::fit_text;

pub struct ContextView {
    state: Rc<RefCell<AppState>>,
    search_bar: SessionSearchBar,
}

impl ContextView {
    pub fn new(state: Rc<RefCell<AppState>>) -> Self {
        Self {
            state,
            search_bar: SessionSearchBar::new(),
        }
    }

    fn search_bounds(bounds: Bounds, padding: f32) -> Bounds {
        let line_height = theme::font_size::SM * 1.5;
        let line_height_xs = theme::font_size::XS * 1.5;
        let y = bounds.origin.y
            + padding
            + line_height
            + theme::spacing::SM
            + line_height_xs
            + theme::spacing::MD;
        let width = (bounds.size.width - padding * 2.0).max(0.0);
        Bounds::new(bounds.origin.x + padding, y, width, 40.0)
    }
}

impl Component for ContextView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let line_height_xs = theme::font_size::XS * 1.5;
        let mut y = bounds.origin.y + padding;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        let session_line = state
            .log_session_id
            .as_ref()
            .map(|id| format!("Active session: {}", id))
            .unwrap_or_else(|| "Active session: none".to_string());
        let session_line = fit_text(cx, &session_line, theme::font_size::SM, available_width);

        let mut session_text = Text::new(session_line)
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY);
        session_text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height,
            ),
            cx,
        );
        y += line_height;

        let path_line = state
            .log_path
            .as_ref()
            .map(|path| format!("Log path: {}", path.display()))
            .unwrap_or_else(|| "Log path: none".to_string());
        let path_line = fit_text(cx, &path_line, theme::font_size::XS, available_width);

        let mut path_text = Text::new(path_line)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        let path_y = y + theme::spacing::SM;
        path_text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                path_y,
                bounds.size.width - padding * 2.0,
                line_height_xs,
            ),
            cx,
        );

        let max_y = bounds.origin.y + bounds.size.height - padding;

        let search_bounds = Self::search_bounds(bounds, padding);
        self.search_bar.paint(search_bounds, cx);
        y = search_bounds.origin.y + search_bounds.size.height + theme::spacing::SM;

        let query = self.search_bar.search_value().trim().to_lowercase();
        let active_filters = self.search_bar.active_filters();
        let mut filtered_sessions = Vec::new();
        for session in &state.sessions {
            let status = map_session_status(session.final_status);
            if !active_filters.is_empty() && !active_filters.contains(&status) {
                continue;
            }
            if !query.is_empty() {
                let query_match = session.id.to_lowercase().contains(&query)
                    || session.prompt.to_lowercase().contains(&query)
                    || session.model.to_lowercase().contains(&query)
                    || session
                        .issue_numbers
                        .as_ref()
                        .map(|issues| issues.to_lowercase().contains(&query))
                        .unwrap_or(false);
                if !query_match {
                    continue;
                }
            }
            filtered_sessions.push(session);
        }

        let session_header = if query.is_empty() && active_filters.is_empty() {
            format!("Sessions: {}", state.sessions.len())
        } else {
            format!(
                "Sessions: {} of {}",
                filtered_sessions.len(),
                state.sessions.len()
            )
        };
        let session_header = fit_text(cx, &session_header, theme::font_size::XS, available_width);
        let mut session_header_text = Text::new(session_header)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        session_header_text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height_xs,
            ),
            cx,
        );
        y += line_height_xs;

        let timeline_reserved = line_height_xs * 4.0 + theme::spacing::SM;
        let session_max_y = (max_y - timeline_reserved).max(y);
        let row_height = line_height_xs.max(24.0);

        if filtered_sessions.is_empty() {
            let empty_line = fit_text(cx, "No sessions match filters", theme::font_size::XS, available_width);
            let mut empty_text = Text::new(empty_line)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            empty_text.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    row_height,
                ),
                cx,
            );
            y += row_height;
        } else {
            for session in filtered_sessions.iter() {
                if y + row_height > session_max_y {
                    break;
                }

                let status = map_session_status(session.final_status);
                let mut badge = SessionStatusBadge::new(status).compact(true);
                if session.duration_seconds > 0.0 {
                    badge = badge.duration(session.duration_seconds as u64);
                }
                if session.issues_completed > 0 {
                    badge = badge.task_count(session.issues_completed as u32);
                }
                let (badge_w, badge_h) = badge.size_hint();
                let badge_w = badge_w.unwrap_or(28.0);
                let badge_h = badge_h.unwrap_or(row_height);
                let badge_bounds = Bounds::new(
                    bounds.origin.x + padding,
                    y + (row_height - badge_h) * 0.5,
                    badge_w,
                    badge_h,
                );
                badge.paint(badge_bounds, cx);

                let id = session.id.chars().take(8).collect::<String>();
                let line = format!("{}  {}", id, session.prompt);
                let line = fit_text(
                    cx,
                    &line,
                    theme::font_size::XS,
                    available_width - badge_w - 8.0,
                );
                let mut text = Text::new(line)
                    .font_size(theme::font_size::XS)
                    .color(theme::text::PRIMARY);
                text.paint(
                    Bounds::new(
                        badge_bounds.origin.x + badge_w + 8.0,
                        y,
                        bounds.size.width - padding * 2.0 - badge_w - 8.0,
                        row_height,
                    ),
                    cx,
                );
                y += row_height;
            }
        }

        y += theme::spacing::SM;
        let timeline_entries = state.timeline_entries(4);
        let header_line = if timeline_entries.is_empty() {
            "Recent activity: none".to_string()
        } else {
            "Recent activity".to_string()
        };
        let header_line = fit_text(cx, &header_line, theme::font_size::XS, available_width);
        let mut header_text = Text::new(header_line)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        header_text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height_xs,
            ),
            cx,
        );
        y += line_height_xs;

        for entry in timeline_entries {
            if y + line_height_xs > max_y {
                break;
            }
            let time = entry.timestamp.as_deref().unwrap_or("-");
            let line = format!("{}  {}", time, entry.label);
            let line = fit_text(cx, &line, theme::font_size::XS, available_width);
            let mut text = Text::new(line)
                .font_size(theme::font_size::XS)
                .color(theme::text::PRIMARY);
            text.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    line_height_xs,
                ),
                cx,
            );
            y += line_height_xs;
        }
    }

    fn event(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        let padding = theme::spacing::MD;
        let search_bounds = Self::search_bounds(bounds, padding);
        let result = self.search_bar.event(event, search_bounds, cx);
        if matches!(result, EventResult::Handled) {
            return result;
        }
        EventResult::Ignored
    }
}

fn map_session_status(status: MetricsStatus) -> UiSessionStatus {
    match status {
        MetricsStatus::Completed => UiSessionStatus::Completed,
        MetricsStatus::Running => UiSessionStatus::Running,
        MetricsStatus::Crashed | MetricsStatus::BudgetExhausted | MetricsStatus::MaxTurns => {
            UiSessionStatus::Failed
        }
    }
}
