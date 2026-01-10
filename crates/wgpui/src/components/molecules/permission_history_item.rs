//! Permission history item for displaying past permission decisions.
//!
//! Shows what permission was requested, when, and what decision was made.

use crate::components::atoms::{ToolIcon, ToolType};
use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::permission_rule_row::PermissionDecision;
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

/// A historical permission request
#[derive(Debug, Clone)]
pub struct PermissionHistory {
    pub id: String,
    pub tool_type: ToolType,
    pub tool_name: String,
    pub description: String,
    pub decision: PermissionDecision,
    pub timestamp: String,
    pub session_id: Option<String>,
}

impl PermissionHistory {
    pub fn new(
        id: impl Into<String>,
        tool_type: ToolType,
        tool_name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            tool_type,
            tool_name: tool_name.into(),
            description: description.into(),
            decision: PermissionDecision::Ask,
            timestamp: String::new(),
            session_id: None,
        }
    }

    pub fn decision(mut self, decision: PermissionDecision) -> Self {
        self.decision = decision;
        self
    }

    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = ts.into();
        self
    }

    pub fn session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

/// A row displaying permission history
pub struct PermissionHistoryItem {
    id: Option<ComponentId>,
    history: PermissionHistory,
    hovered: bool,
    compact: bool,
}

impl PermissionHistoryItem {
    pub fn new(history: PermissionHistory) -> Self {
        Self {
            id: None,
            history,
            hovered: false,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn history(&self) -> &PermissionHistory {
        &self.history
    }
}

impl Component for PermissionHistoryItem {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let decision_color = self.history.decision.color();
        let bg = if self.hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };

        // Row background with left accent
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Left decision indicator
        let indicator = Bounds::new(bounds.origin.x, bounds.origin.y, 3.0, bounds.size.height);
        cx.scene
            .draw_quad(Quad::new(indicator).with_background(decision_color));

        let padding = 12.0;
        let mut x = bounds.origin.x + padding + 4.0;

        if self.compact {
            // Compact: single line layout
            let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::SM) / 2.0;

            // Tool icon
            let mut icon = ToolIcon::new(self.history.tool_type).size(16.0);
            icon.paint(
                Bounds::new(
                    x,
                    bounds.origin.y + (bounds.size.height - 16.0) / 2.0,
                    16.0,
                    16.0,
                ),
                cx,
            );
            x += 22.0;

            // Tool name
            let name_run = cx.text.layout_mono(
                &self.history.tool_name,
                Point::new(x, text_y),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(name_run);
            x += self.history.tool_name.len() as f32 * 7.0 + 12.0;

            // Decision
            let decision_run = cx.text.layout_mono(
                self.history.decision.short_label(),
                Point::new(x, text_y),
                theme::font_size::XS,
                decision_color,
            );
            cx.scene.draw_text(decision_run);

            // Timestamp (right aligned)
            if !self.history.timestamp.is_empty() {
                let ts_run = cx.text.layout_mono(
                    &self.history.timestamp,
                    Point::new(bounds.origin.x + bounds.size.width - padding - 80.0, text_y),
                    theme::font_size::XS,
                    theme::text::DISABLED,
                );
                cx.scene.draw_text(ts_run);
            }
        } else {
            // Full: two-line layout
            let mut y = bounds.origin.y + 8.0;

            // First row: tool icon, name, decision
            let mut icon = ToolIcon::new(self.history.tool_type).size(18.0);
            icon.paint(Bounds::new(x, y, 18.0, 18.0), cx);
            x += 26.0;

            let name_run = cx.text.layout_mono(
                &self.history.tool_name,
                Point::new(x, y + 2.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(name_run);
            x += self.history.tool_name.len() as f32 * 7.0 + 12.0;

            // Decision badge
            let decision_bounds = Bounds::new(x, y, 60.0, 20.0);
            cx.scene.draw_quad(
                Quad::new(decision_bounds)
                    .with_background(decision_color.with_alpha(0.2))
                    .with_border(decision_color, 1.0),
            );

            let decision_run = cx.text.layout_mono(
                self.history.decision.short_label(),
                Point::new(
                    decision_bounds.origin.x + 6.0,
                    decision_bounds.origin.y + 3.0,
                ),
                theme::font_size::XS,
                decision_color,
            );
            cx.scene.draw_text(decision_run);

            // Timestamp (right aligned)
            if !self.history.timestamp.is_empty() {
                let ts_run = cx.text.layout_mono(
                    &self.history.timestamp,
                    Point::new(
                        bounds.origin.x + bounds.size.width - padding - 80.0,
                        y + 2.0,
                    ),
                    theme::font_size::XS,
                    theme::text::DISABLED,
                );
                cx.scene.draw_text(ts_run);
            }

            y += 24.0;

            // Second row: description
            let desc_run = cx.text.layout_mono(
                &self.history.description,
                Point::new(bounds.origin.x + padding + 30.0, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(desc_run);

            // Session ID (if present)
            if let Some(session_id) = &self.history.session_id {
                let session_text = format!("Session: {}", &session_id[..session_id.len().min(8)]);
                let session_run = cx.text.layout_mono(
                    &session_text,
                    Point::new(bounds.origin.x + bounds.size.width - padding - 100.0, y),
                    theme::font_size::XS,
                    theme::text::DISABLED,
                );
                cx.scene.draw_text(session_run);
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        if let InputEvent::MouseMove { x, y } = event {
            let was_hovered = self.hovered;
            self.hovered = bounds.contains(Point::new(*x, *y));
            if was_hovered != self.hovered {
                return EventResult::Handled;
            }
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let height = if self.compact { 36.0 } else { 56.0 };
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_history() {
        let history =
            PermissionHistory::new("h-1", ToolType::Bash, "Bash", "Execute shell command")
                .decision(PermissionDecision::AllowOnce)
                .timestamp("2 min ago")
                .session("sess-123");

        assert_eq!(history.tool_name, "Bash");
        assert_eq!(history.decision, PermissionDecision::AllowOnce);
        assert!(history.session_id.is_some());
    }

    #[test]
    fn test_permission_history_item() {
        let history = PermissionHistory::new("1", ToolType::Read, "Read", "Read file");
        let item = PermissionHistoryItem::new(history).compact(true);
        assert!(item.compact);
    }
}
