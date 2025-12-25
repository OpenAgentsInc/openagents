use std::cell::RefCell;
use std::rc::Rc;

use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext, Text, theme};

use crate::state::AppState;
use crate::views::fit_text;

pub struct ParallelView {
    state: Rc<RefCell<AppState>>,
}

impl ParallelView {
    pub fn new(state: Rc<RefCell<AppState>>) -> Self {
        Self { state }
    }
}

impl Component for ParallelView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        let header_line = format!("Agents detected: {}", state.agents.len());
        let header_line = fit_text(cx, &header_line, theme::font_size::SM, available_width);
        let mut header = Text::new(header_line)
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY);
        header.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + padding,
                bounds.size.width - padding * 2.0,
                line_height,
            ),
            cx,
        );

        let mut y = bounds.origin.y + padding + line_height + theme::spacing::SM;
        for agent in state.agents.iter().take(8) {
            let issue = agent
                .current_issue
                .map(|num| format!("#{}", num))
                .unwrap_or_else(|| "-".to_string());
            let line = format!("{}  {}  {}", agent.id, agent.status, issue);
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
