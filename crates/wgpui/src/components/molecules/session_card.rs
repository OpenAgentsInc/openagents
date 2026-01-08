//! Session card molecule for displaying session preview information.
//!
//! Shows session status, duration, task count, and allows resume/fork actions.

use crate::components::atoms::{SessionStatus, SessionStatusBadge};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

/// Session information for display
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub status: SessionStatus,
    pub duration_secs: Option<u64>,
    pub task_count: Option<u32>,
    pub timestamp: Option<String>,
    pub model: Option<String>,
}

impl SessionInfo {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status: SessionStatus::Pending,
            duration_secs: None,
            task_count: None,
            timestamp: None,
            model: None,
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

    pub fn task_count(mut self, count: u32) -> Self {
        self.task_count = Some(count);
        self
    }

    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = Some(ts.into());
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

/// Actions available on a session card
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAction {
    Select,
    Resume,
    Fork,
    Delete,
}

/// A card displaying session information
pub struct SessionCard {
    id: Option<ComponentId>,
    session: SessionInfo,
    hovered: bool,
    show_actions: bool,
    hovered_action: Option<SessionAction>,
    on_action: Option<Box<dyn FnMut(SessionAction, String)>>,
}

impl SessionCard {
    pub fn new(session: SessionInfo) -> Self {
        Self {
            id: None,
            session,
            hovered: false,
            show_actions: true,
            hovered_action: None,
            on_action: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show_actions(mut self, show: bool) -> Self {
        self.show_actions = show;
        self
    }

    pub fn on_action<F>(mut self, f: F) -> Self
    where
        F: FnMut(SessionAction, String) + 'static,
    {
        self.on_action = Some(Box::new(f));
        self
    }

    pub fn session(&self) -> &SessionInfo {
        &self.session
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

    fn action_bounds(&self, bounds: &Bounds) -> Vec<(SessionAction, Bounds)> {
        if !self.show_actions
            || !self.session.status.can_resume() && !self.session.status.can_fork()
        {
            return Vec::new();
        }

        let mut actions = Vec::new();
        let btn_width = 50.0;
        let btn_height = 22.0;
        let gap = 6.0;
        let padding = 12.0;
        let y = bounds.origin.y + bounds.size.height - padding - btn_height;
        let mut x = bounds.origin.x + bounds.size.width - padding;

        if self.session.status.can_resume() {
            x -= btn_width;
            actions.push((
                SessionAction::Resume,
                Bounds::new(x, y, btn_width, btn_height),
            ));
            x -= gap;
        }

        if self.session.status.can_fork() {
            x -= btn_width;
            actions.push((
                SessionAction::Fork,
                Bounds::new(x, y, btn_width, btn_height),
            ));
        }

        actions
    }
}

impl Component for SessionCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let status_color = self.session.status.color();
        let bg = if self.hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };

        // Card background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(status_color.with_alpha(0.3), 1.0),
        );

        // Left status accent bar
        let accent_bar = Bounds::new(bounds.origin.x, bounds.origin.y, 3.0, bounds.size.height);
        cx.scene
            .draw_quad(Quad::new(accent_bar).with_background(status_color));

        let padding = 12.0;
        let content_x = bounds.origin.x + padding + 4.0;
        let mut y = bounds.origin.y + padding;

        // Title row
        let title_run = cx.text.layout(
            &self.session.title,
            Point::new(content_x, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Session ID (right aligned)
        let id_text = format!("#{}", &self.session.id[..self.session.id.len().min(8)]);
        let id_run = cx.text.layout(
            &id_text,
            Point::new(bounds.origin.x + bounds.size.width - padding - 60.0, y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(id_run);

        y += 20.0;

        // Status badge
        let mut status_badge = SessionStatusBadge::new(self.session.status);
        if let Some(secs) = self.session.duration_secs {
            status_badge = status_badge.duration(secs);
        }
        if let Some(count) = self.session.task_count {
            status_badge = status_badge.task_count(count);
        }
        status_badge.paint(Bounds::new(content_x, y, 200.0, 22.0), cx);

        y += 26.0;

        // Metadata row
        let mut meta_parts = Vec::new();
        if let Some(ts) = &self.session.timestamp {
            meta_parts.push(ts.clone());
        }
        if let Some(model) = &self.session.model {
            meta_parts.push(model.clone());
        }
        if let Some(secs) = self.session.duration_secs {
            meta_parts.push(Self::format_duration(secs));
        }

        if !meta_parts.is_empty() {
            let meta_text = meta_parts.join(" • ");
            let meta_run = cx.text.layout(
                &meta_text,
                Point::new(content_x, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(meta_run);
        }

        // Action buttons
        for (action, action_bounds) in self.action_bounds(&bounds) {
            let is_hovered = self.hovered_action == Some(action);
            let btn_bg = if is_hovered {
                theme::accent::PRIMARY
            } else {
                theme::bg::MUTED
            };
            let btn_text_color = if is_hovered {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            };

            cx.scene
                .draw_quad(Quad::new(action_bounds).with_background(btn_bg));

            let label = match action {
                SessionAction::Resume => "Resume",
                SessionAction::Fork => "Fork",
                SessionAction::Select => "Select",
                SessionAction::Delete => "Delete",
            };

            let label_run = cx.text.layout(
                label,
                Point::new(
                    action_bounds.origin.x + 4.0,
                    action_bounds.origin.y
                        + (action_bounds.size.height - theme::font_size::XS) / 2.0,
                ),
                theme::font_size::XS,
                btn_text_color,
            );
            cx.scene.draw_text(label_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered;
                let was_action = self.hovered_action;

                self.hovered = bounds.contains(point);
                self.hovered_action = None;

                for (action, action_bounds) in self.action_bounds(&bounds) {
                    if action_bounds.contains(point) {
                        self.hovered_action = Some(action);
                        break;
                    }
                }

                if was_hovered != self.hovered || was_action != self.hovered_action {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    // Check action buttons first
                    for (action, action_bounds) in self.action_bounds(&bounds) {
                        if action_bounds.contains(point) {
                            if let Some(callback) = &mut self.on_action {
                                callback(action, self.session.id.clone());
                            }
                            return EventResult::Handled;
                        }
                    }

                    // Card click = select
                    if bounds.contains(point) {
                        if let Some(callback) = &mut self.on_action {
                            callback(SessionAction::Select, self.session.id.clone());
                        }
                        return EventResult::Handled;
                    }
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
        (None, Some(100.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_info() {
        let info = SessionInfo::new("sess-123", "Build feature X")
            .status(SessionStatus::Running)
            .duration(3600)
            .task_count(15);

        assert_eq!(info.id, "sess-123");
        assert_eq!(info.title, "Build feature X");
        assert_eq!(info.duration_secs, Some(3600));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(SessionCard::format_duration(45), "45s");
        assert_eq!(SessionCard::format_duration(125), "2m 5s");
        assert_eq!(SessionCard::format_duration(3665), "1h 1m");
    }

    #[test]
    fn test_session_card() {
        let info = SessionInfo::new("123", "Test").status(SessionStatus::Completed);
        let card = SessionCard::new(info).show_actions(true);
        assert!(card.show_actions);
    }
}
