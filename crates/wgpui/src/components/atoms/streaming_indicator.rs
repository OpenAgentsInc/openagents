use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad, theme};

pub struct StreamingIndicator {
    id: Option<ComponentId>,
    active: bool,
    dot_count: usize,
    dot_size: f32,
    gap: f32,
    color: Hsla,
    frame: usize,
}

impl StreamingIndicator {
    pub fn new() -> Self {
        Self {
            id: None,
            active: true,
            dot_count: 3,
            dot_size: 4.0,
            gap: 4.0,
            color: theme::accent::PRIMARY,
            frame: 0,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn dot_count(mut self, count: usize) -> Self {
        self.dot_count = count;
        self
    }

    pub fn dot_size(mut self, size: f32) -> Self {
        self.dot_size = size;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn tick(&mut self) {
        if self.active {
            self.frame = (self.frame + 1) % (self.dot_count * 10);
        }
    }
}

impl Default for StreamingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for StreamingIndicator {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if !self.active {
            return;
        }

        let total_width =
            self.dot_count as f32 * self.dot_size + (self.dot_count - 1) as f32 * self.gap;
        let start_x = bounds.origin.x + (bounds.size.width - total_width) / 2.0;
        let center_y = bounds.origin.y + (bounds.size.height - self.dot_size) / 2.0;

        let active_dot = (self.frame / 10) % self.dot_count;

        for i in 0..self.dot_count {
            let x = start_x + i as f32 * (self.dot_size + self.gap);
            let alpha = if i == active_dot { 1.0 } else { 0.3 };

            cx.scene.draw_quad(
                Quad::new(Bounds::new(x, center_y, self.dot_size, self.dot_size))
                    .with_background(self.color.with_alpha(alpha)),
            );
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

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let total_width =
            self.dot_count as f32 * self.dot_size + (self.dot_count - 1) as f32 * self.gap;
        (Some(total_width), Some(self.dot_size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_indicator_new() {
        let indicator = StreamingIndicator::new();
        assert!(indicator.is_active());
        assert_eq!(indicator.dot_count, 3);
    }

    #[test]
    fn test_streaming_indicator_builder() {
        let indicator = StreamingIndicator::new()
            .with_id(1)
            .active(false)
            .dot_count(5)
            .dot_size(6.0);

        assert_eq!(indicator.id, Some(1));
        assert!(!indicator.is_active());
        assert_eq!(indicator.dot_count, 5);
        assert_eq!(indicator.dot_size, 6.0);
    }

    #[test]
    fn test_tick() {
        let mut indicator = StreamingIndicator::new();
        assert_eq!(indicator.frame, 0);
        indicator.tick();
        assert_eq!(indicator.frame, 1);
    }

    #[test]
    fn test_size_hint() {
        let indicator = StreamingIndicator::new().dot_count(3).dot_size(4.0);
        let (w, _) = indicator.size_hint();
        assert_eq!(w, Some(20.0));
    }
}
