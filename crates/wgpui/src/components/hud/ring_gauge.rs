use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

/// A circular progress/level gauge using dot segments.
///
/// # Deprecation
/// This component is deprecated. Use [`viz::fill::Ring`] instead, which provides
/// the same functionality with a unified visualization grammar.
#[deprecated(since = "0.1.0", note = "Use viz::fill::Ring instead")]
pub struct RingGauge {
    id: Option<ComponentId>,
    segments: usize,
    level: f32,
    dot_size: f32,
    active_color: Hsla,
    inactive_color: Hsla,
    head_color: Hsla,
    head: Option<usize>,
    start_angle: f32,
    sweep: f32,
}

impl RingGauge {
    pub fn new() -> Self {
        Self {
            id: None,
            segments: 64,
            level: 0.0,
            dot_size: 6.0,
            active_color: Hsla::from_hex(0x2ec4d6),
            inactive_color: Hsla::from_hex(0x10212a),
            head_color: Hsla::from_hex(0xf5faff),
            head: None,
            start_angle: -std::f32::consts::FRAC_PI_2,
            sweep: std::f32::consts::TAU,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn segments(mut self, segments: usize) -> Self {
        self.segments = segments.max(1);
        self
    }

    pub fn level(mut self, level: f32) -> Self {
        self.level = level.clamp(0.0, 1.0);
        self
    }

    pub fn dot_size(mut self, dot_size: f32) -> Self {
        self.dot_size = dot_size.max(1.0);
        self
    }

    pub fn active_color(mut self, color: Hsla) -> Self {
        self.active_color = color;
        self
    }

    pub fn inactive_color(mut self, color: Hsla) -> Self {
        self.inactive_color = color;
        self
    }

    pub fn head_color(mut self, color: Hsla) -> Self {
        self.head_color = color;
        self
    }

    pub fn head(mut self, head: Option<usize>) -> Self {
        self.head = head;
        self
    }

    pub fn set_level(&mut self, level: f32) {
        self.level = level.clamp(0.0, 1.0);
    }
}

impl Default for RingGauge {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for RingGauge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let segments = self.segments.max(1);
        let filled = ((segments as f32) * self.level).round() as usize;
        let head = self.head.unwrap_or_else(|| filled.saturating_sub(1));

        let size = bounds.size.width.min(bounds.size.height);
        let radius = (size * 0.5 - self.dot_size).max(0.0);
        let center_x = bounds.origin.x + bounds.size.width * 0.5;
        let center_y = bounds.origin.y + bounds.size.height * 0.5;
        let angle_step = self.sweep / segments as f32;

        for i in 0..segments {
            let angle = self.start_angle + i as f32 * angle_step;
            let x = center_x + radius * angle.cos();
            let y = center_y + radius * angle.sin();
            let color = if i == head {
                self.head_color
            } else if i < filled {
                self.active_color
            } else {
                self.inactive_color
            };
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    x - self.dot_size * 0.5,
                    y - self.dot_size * 0.5,
                    self.dot_size,
                    self.dot_size,
                ))
                .with_background(color),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_gauge_builder() {
        let gauge = RingGauge::new()
            .with_id(11)
            .segments(24)
            .level(0.5)
            .dot_size(8.0);

        assert_eq!(gauge.id, Some(11));
        assert_eq!(gauge.segments, 24);
        assert_eq!(gauge.level, 0.5);
        assert_eq!(gauge.dot_size, 8.0);
    }
}
