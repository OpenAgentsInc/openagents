use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

pub struct Reticle {
    id: Option<ComponentId>,
    color: Hsla,
    thickness: f32,
    gap: f32,
    line_length: f32,
    center_size: f32,
    tick_length: f32,
    opacity: f32,
}

impl Reticle {
    pub fn new() -> Self {
        Self {
            id: None,
            color: Hsla::new(190.0, 0.6, 0.6, 0.85),
            thickness: 2.0,
            gap: 6.0,
            line_length: 28.0,
            center_size: 6.0,
            tick_length: 10.0,
            opacity: 1.0,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness.max(1.0);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn line_length(mut self, length: f32) -> Self {
        self.line_length = length.max(0.0);
        self
    }

    pub fn center_size(mut self, size: f32) -> Self {
        self.center_size = size.max(0.0);
        self
    }

    pub fn tick_length(mut self, length: f32) -> Self {
        self.tick_length = length.max(0.0);
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

impl Default for Reticle {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Reticle {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.color.with_alpha(self.color.a * self.opacity);
        let thickness = self.thickness.max(1.0);
        let gap = self.gap.max(0.0);
        let center = bounds.origin.x + bounds.size.width * 0.5;
        let middle = bounds.origin.y + bounds.size.height * 0.5;
        let max_center = bounds.size.width.min(bounds.size.height);
        let center_size = self.center_size.min(max_center).max(thickness);
        let center_half = center_size * 0.5;

        let max_line_x = (bounds.size.width * 0.5 - gap).max(0.0);
        let max_line_y = (bounds.size.height * 0.5 - gap).max(0.0);
        let line_length_x = self.line_length.min(max_line_x);
        let line_length_y = self.line_length.min(max_line_y);

        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                center - center_half,
                middle - center_half,
                center_size,
                center_size,
            ))
            .with_background(color.with_alpha(color.a * 0.6)),
        );

        if line_length_x > 0.0 {
            let left_x = center - gap - line_length_x;
            let right_x = center + gap;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    left_x,
                    middle - thickness * 0.5,
                    line_length_x,
                    thickness,
                ))
                .with_background(color),
            );
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    right_x,
                    middle - thickness * 0.5,
                    line_length_x,
                    thickness,
                ))
                .with_background(color),
            );

            let tick_len = self
                .tick_length
                .min(line_length_x.max(line_length_y))
                .max(thickness);
            if tick_len > 0.0 {
                let left_tick_x = left_x;
                let right_tick_x = right_x + line_length_x - thickness;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        left_tick_x,
                        middle - tick_len * 0.5,
                        thickness,
                        tick_len,
                    ))
                    .with_background(color),
                );
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        right_tick_x,
                        middle - tick_len * 0.5,
                        thickness,
                        tick_len,
                    ))
                    .with_background(color),
                );
            }
        }

        if line_length_y > 0.0 {
            let top_y = middle - gap - line_length_y;
            let bottom_y = middle + gap;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    center - thickness * 0.5,
                    top_y,
                    thickness,
                    line_length_y,
                ))
                .with_background(color),
            );
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    center - thickness * 0.5,
                    bottom_y,
                    thickness,
                    line_length_y,
                ))
                .with_background(color),
            );

            let tick_len = self
                .tick_length
                .min(line_length_x.max(line_length_y))
                .max(thickness);
            if tick_len > 0.0 {
                let top_tick_y = top_y;
                let bottom_tick_y = bottom_y + line_length_y - thickness;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        center - tick_len * 0.5,
                        top_tick_y,
                        tick_len,
                        thickness,
                    ))
                    .with_background(color),
                );
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        center - tick_len * 0.5,
                        bottom_tick_y,
                        tick_len,
                        thickness,
                    ))
                    .with_background(color),
                );
            }
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
    fn test_reticle_builder() {
        let reticle = Reticle::new()
            .with_id(11)
            .thickness(3.0)
            .gap(8.0)
            .line_length(24.0)
            .center_size(10.0)
            .tick_length(12.0)
            .opacity(0.4);

        assert_eq!(reticle.id, Some(11));
        assert_eq!(reticle.thickness, 3.0);
        assert_eq!(reticle.gap, 8.0);
        assert_eq!(reticle.line_length, 24.0);
        assert_eq!(reticle.center_size, 10.0);
        assert_eq!(reticle.tick_length, 12.0);
        assert_eq!(reticle.opacity, 0.4);
    }

    #[test]
    fn test_reticle_clamps() {
        let reticle = Reticle::new()
            .thickness(0.1)
            .gap(-2.0)
            .line_length(-5.0)
            .center_size(-1.0)
            .tick_length(-2.0)
            .opacity(1.5);

        assert_eq!(reticle.thickness, 1.0);
        assert_eq!(reticle.gap, 0.0);
        assert_eq!(reticle.line_length, 0.0);
        assert_eq!(reticle.center_size, 0.0);
        assert_eq!(reticle.tick_length, 0.0);
        assert_eq!(reticle.opacity, 1.0);
    }
}
