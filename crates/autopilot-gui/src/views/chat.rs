use std::cell::RefCell;
use std::rc::Rc;

use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext, Text, theme};

use crate::state::AppState;
use crate::views::fit_text;

pub struct ChatView {
    state: Rc<RefCell<AppState>>,
}

impl ChatView {
    pub fn new(state: Rc<RefCell<AppState>>) -> Self {
        Self { state }
    }
}

impl Component for ChatView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let mut y = bounds.origin.y + padding;
        let line_height = theme::font_size::XS * 1.6;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        let log_label = state
            .log_session_id
            .as_ref()
            .map(|id| format!("Session: {}", id))
            .unwrap_or_else(|| "Session: none".to_string());
        let log_label = fit_text(cx, &log_label, theme::font_size::SM, available_width);

        let mut header = Text::new(log_label)
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY);
        header.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height,
            ),
            cx,
        );
        y += line_height + theme::spacing::SM;

        let max_lines = ((bounds.size.height - (y - bounds.origin.y) - padding) / line_height)
            .floor()
            .max(0.0) as usize;

        let lines = if state.log_lines.is_empty() {
            vec!["No streaming log data yet.".to_string()]
        } else {
            let start = state
                .log_lines
                .len()
                .saturating_sub(max_lines.max(1));
            state.log_lines[start..].to_vec()
        };

        for line in lines.into_iter().take(max_lines.max(1)) {
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
