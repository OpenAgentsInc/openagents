use std::cell::RefCell;
use std::rc::Rc;

use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext, Text, theme};

use crate::state::AppState;
use crate::views::fit_text;

pub struct ContextView {
    state: Rc<RefCell<AppState>>,
}

impl ContextView {
    pub fn new(state: Rc<RefCell<AppState>>) -> Self {
        Self { state }
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

        y = path_y + line_height_xs + theme::spacing::MD;
        let max_y = bounds.origin.y + bounds.size.height - padding;

        let timeline_entries = state.timeline_entries(6);
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
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }
}
