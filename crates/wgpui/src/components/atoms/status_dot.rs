use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Online,
    Offline,
    Busy,
    Away,
    Error,
}

impl Status {
    fn color(&self) -> Hsla {
        match self {
            Status::Online => theme::status::SUCCESS,
            Status::Offline => theme::text::MUTED,
            Status::Busy => theme::status::WARNING,
            Status::Away => theme::status::WARNING,
            Status::Error => theme::status::ERROR,
        }
    }
}

pub struct StatusDot {
    id: Option<ComponentId>,
    status: Status,
    size: f32,
    pulsing: bool,
}

impl StatusDot {
    pub fn new(status: Status) -> Self {
        Self {
            id: None,
            status,
            size: 8.0,
            pulsing: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn pulsing(mut self, pulsing: bool) -> Self {
        self.pulsing = pulsing;
        self
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn set_status(&mut self, status: Status) {
        self.status = status;
    }
}

impl Default for StatusDot {
    fn default() -> Self {
        Self::new(Status::default())
    }
}

impl Component for StatusDot {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let center_x = bounds.origin.x + (bounds.size.width - self.size) / 2.0;
        let center_y = bounds.origin.y + (bounds.size.height - self.size) / 2.0;

        cx.scene.draw_quad(
            Quad::new(Bounds::new(center_x, center_y, self.size, self.size))
                .with_background(self.status.color()),
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
        (Some(self.size), Some(self.size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_dot_new() {
        let dot = StatusDot::new(Status::Online);
        assert_eq!(dot.status(), Status::Online);
        assert_eq!(dot.size, 8.0);
    }

    #[test]
    fn test_status_dot_builder() {
        let dot = StatusDot::new(Status::Busy)
            .with_id(42)
            .size(12.0)
            .pulsing(true);

        assert_eq!(dot.id, Some(42));
        assert_eq!(dot.size, 12.0);
        assert!(dot.pulsing);
    }

    #[test]
    fn test_status_colors() {
        assert_eq!(Status::Online.color(), theme::status::SUCCESS);
        assert_eq!(Status::Error.color(), theme::status::ERROR);
    }

    #[test]
    fn test_set_status() {
        let mut dot = StatusDot::new(Status::Online);
        dot.set_status(Status::Offline);
        assert_eq!(dot.status(), Status::Offline);
    }

    #[test]
    fn test_size_hint() {
        let dot = StatusDot::new(Status::Online).size(16.0);
        let (w, h) = dot.size_hint();
        assert_eq!(w, Some(16.0));
        assert_eq!(h, Some(16.0));
    }
}
