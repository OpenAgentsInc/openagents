use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad, theme};

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
    fn color(&self) -> Hsla {
        match self {
            ToolStatus::Pending => theme::status::WARNING,
            ToolStatus::Running => theme::status::WARNING,
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
        let dot_size = (self.font_size * 0.6).clamp(6.0, 10.0).round();
        let dot_y = (bounds.origin.y + (bounds.size.height - dot_size) / 2.0).round();
        let dot_bounds = Bounds::new(bounds.origin.x, dot_y, dot_size, dot_size);

        cx.scene.draw_quad(
            Quad::new(dot_bounds)
                .with_background(self.status.color())
                .with_corner_radius(dot_size / 2.0),
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

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let dot_size = (self.font_size * 0.6).clamp(6.0, 10.0);
        (Some(dot_size), Some(dot_size))
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
    fn test_set_status() {
        let mut badge = ToolStatusBadge::new(ToolStatus::Pending);
        badge.set_status(ToolStatus::Success);
        assert_eq!(badge.status(), ToolStatus::Success);
    }
}
