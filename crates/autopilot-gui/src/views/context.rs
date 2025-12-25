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
        path_text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y + theme::spacing::SM,
                bounds.size.width - padding * 2.0,
                line_height,
            ),
            cx,
        );
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
