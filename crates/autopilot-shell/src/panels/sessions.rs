//! Sessions panel for the left sidebar

use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, theme,
    components::TextInput,
    components::molecules::{SessionCard, SessionInfo},
    components::hud::{CornerConfig, Frame},
};
use crate::dock::{DockPosition, Panel};

/// Left sidebar panel with session search and card
pub struct SessionsPanel {
    search: TextInput,
    card: SessionCard,
}

impl SessionsPanel {
    pub fn new() -> Self {
        Self {
            search: TextInput::new()
                .placeholder("Search sessions...")
                .background(theme::bg::SURFACE),
            card: SessionCard::new(
                SessionInfo::new("session", "Autopilot Shell")
                    .model("claude-sonnet-4-5")
                    .task_count(0),
            ),
        }
    }

    pub fn set_task_count(&mut self, count: u32) {
        self.card = SessionCard::new(
            SessionInfo::new("session", "Autopilot Shell")
                .model("claude-sonnet-4-5")
                .task_count(count),
        );
    }
}

impl Default for SessionsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for SessionsPanel {
    fn panel_id(&self) -> &'static str {
        "sessions"
    }

    fn title(&self) -> &str {
        "Sessions"
    }

    fn preferred_position(&self) -> DockPosition {
        DockPosition::Left
    }

    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw HUD frame around panel
        let line_color = Hsla::new(0.0, 0.0, 0.5, 0.6);
        let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.85);

        let mut frame = Frame::nefrex()
            .line_color(line_color)
            .bg_color(bg_color)
            .stroke_width(1.0)
            .corner_config(CornerConfig::all())
            .square_size(6.0)
            .small_line_length(6.0)
            .large_line_length(20.0);
        frame.paint(bounds, cx);

        let padding = 16.0;
        let search_h = 32.0;

        // Search bar at top
        let search_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding,
            bounds.size.width - padding * 2.0,
            search_h,
        );
        Component::paint(&mut self.search, search_bounds, cx);

        // Session card below search
        let card_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding + search_h + theme::spacing::SM,
            bounds.size.width - padding * 2.0,
            120.0,
        );
        Component::paint(&mut self.card, card_bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let padding = 16.0;
        let search_h = 32.0;

        let search_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding,
            bounds.size.width - padding * 2.0,
            search_h,
        );
        if Component::event(&mut self.search, event, search_bounds, cx).is_handled() {
            return EventResult::Handled;
        }

        let card_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding + search_h + theme::spacing::SM,
            bounds.size.width - padding * 2.0,
            120.0,
        );
        Component::event(&mut self.card, event, card_bounds, cx)
    }
}
