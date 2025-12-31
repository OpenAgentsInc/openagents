use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolStatus {
    #[default]
    Pending,
    Running,
    Success,
    Error,
    Cancelled,
}

impl ToolStatus {
    fn label(&self) -> &'static str {
        match self {
            ToolStatus::Pending => "pending",
            ToolStatus::Running => "running",
            ToolStatus::Success => "success",
            ToolStatus::Error => "error",
            ToolStatus::Cancelled => "cancelled",
        }
    }

    fn color(&self) -> Hsla {
        match self {
            ToolStatus::Pending => theme::text::MUTED,
            ToolStatus::Running => theme::accent::PRIMARY,
            ToolStatus::Success => theme::status::SUCCESS,
            ToolStatus::Error => theme::status::ERROR,
            ToolStatus::Cancelled => theme::text::MUTED,
        }
    }
}

pub struct ToolStatusBadge {
    id: Option<ComponentId>,
    status: ToolStatus,
    font_size: f32,
    show_icon: bool,
}

impl ToolStatusBadge {
    pub fn new(status: ToolStatus) -> Self {
        Self {
            id: None,
            status,
            font_size: theme::font_size::XS,
            show_icon: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn show_icon(mut self, show: bool) -> Self {
        self.show_icon = show;
        self
    }

    pub fn status(&self) -> ToolStatus {
        self.status
    }

    pub fn set_status(&mut self, status: ToolStatus) {
        self.status = status;
    }
}

impl Default for ToolStatusBadge {
    fn default() -> Self {
        Self::new(ToolStatus::default())
    }
}

impl Component for ToolStatusBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding_h = theme::spacing::SM;
        let padding_v = theme::spacing::XS;

        let icon = if self.show_icon {
            match self.status {
                ToolStatus::Pending => "○ ",
                ToolStatus::Running => "◐ ",
                ToolStatus::Success => "✓ ",
                ToolStatus::Error => "✗ ",
                ToolStatus::Cancelled => "— ",
            }
        } else {
            ""
        };

        let label = format!("{}{}", icon, self.status.label());
        let text_width = label.len() as f32 * self.font_size * 0.6;
        let badge_width = text_width + padding_h * 2.0;
        let badge_height = self.font_size + padding_v * 2.0;

        let badge_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + (bounds.size.height - badge_height) / 2.0,
            badge_width,
            badge_height,
        );

        cx.scene.draw_quad(
            Quad::new(badge_bounds)
                .with_background(self.status.color().with_alpha(0.15))
                .with_border(self.status.color(), 1.0),
        );

        let text_x = badge_bounds.origin.x + padding_h;
        let text_y = badge_bounds.origin.y + badge_height * 0.5 - self.font_size * 0.55;

        let text_run = cx.text.layout(
            &label,
            Point::new(text_x, text_y),
            self.font_size,
            self.status.color(),
        );
        cx.scene.draw_text(text_run);
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let padding_h = theme::spacing::SM;
        let padding_v = theme::spacing::XS;
        let icon_len = if self.show_icon { 2 } else { 0 };
        let label = self.status.label();
        let text_width = (label.len() + icon_len) as f32 * self.font_size * 0.6;
        (
            Some(text_width + padding_h * 2.0),
            Some(self.font_size + padding_v * 2.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_status_badge_new() {
        let badge = ToolStatusBadge::new(ToolStatus::Running);
        assert_eq!(badge.status(), ToolStatus::Running);
    }

    #[test]
    fn test_status_labels() {
        assert_eq!(ToolStatus::Success.label(), "success");
        assert_eq!(ToolStatus::Error.label(), "error");
    }

    #[test]
    fn test_set_status() {
        let mut badge = ToolStatusBadge::new(ToolStatus::Pending);
        badge.set_status(ToolStatus::Success);
        assert_eq!(badge.status(), ToolStatus::Success);
    }
}
