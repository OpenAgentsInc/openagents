use crate::components::context::PaintContext;
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DotShape {
    Box,
    Circle,
    Cross,
}

impl Default for DotShape {
    fn default() -> Self {
        Self::Box
    }
}

pub struct DotsGrid {
    id: Option<ComponentId>,
    color: Hsla,
    shape: DotShape,
    distance: f32,
    size: f32,
    cross_thickness: f32,
    opacity: f32,
}

impl DotsGrid {
    pub fn new() -> Self {
        Self {
            id: None,
            color: Hsla::new(0.0, 0.0, 0.4, 1.0),
            shape: DotShape::Box,
            distance: 30.0,
            size: 2.0,
            cross_thickness: 1.0,
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

    pub fn shape(mut self, shape: DotShape) -> Self {
        self.shape = shape;
        self
    }

    pub fn distance(mut self, distance: f32) -> Self {
        self.distance = distance.max(5.0);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(1.0);
        self
    }

    pub fn cross_thickness(mut self, thickness: f32) -> Self {
        self.cross_thickness = thickness.max(0.5);
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    fn draw_dot(&self, cx: &mut PaintContext, x: f32, y: f32) {
        let color = self.color.with_alpha(self.color.a * self.opacity);
        let s = self.size;
        let half = s / 2.0;

        match self.shape {
            DotShape::Box => {
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x - half, y - half, s, s))
                        .with_background(color),
                );
            }
            DotShape::Circle => {
                let segments = 6;
                let r = half;
                for i in 0..segments {
                    let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
                    let next_angle = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                    let x1 = x + angle.cos() * r * 0.5;
                    let y1 = y + angle.sin() * r * 0.5;
                    let x2 = x + next_angle.cos() * r * 0.5;
                    let y2 = y + next_angle.sin() * r * 0.5;
                    let min_x = x1.min(x2).min(x);
                    let min_y = y1.min(y2).min(y);
                    let max_x = x1.max(x2).max(x);
                    let max_y = y1.max(y2).max(y);
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(min_x, min_y, (max_x - min_x).max(1.0), (max_y - min_y).max(1.0)))
                            .with_background(color),
                    );
                }
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x - half, y - half, s, s))
                        .with_background(color),
                );
            }
            DotShape::Cross => {
                let t = self.cross_thickness;
                let half_t = t / 2.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x - half, y - half_t, s, t))
                        .with_background(color),
                );
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x - half_t, y - half, t, s))
                        .with_background(color),
                );
            }
        }
    }
}

impl Default for DotsGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for DotsGrid {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let d = self.distance;
        let x_count = (bounds.size.width / d).ceil() as i32 + 1;
        let y_count = (bounds.size.height / d).ceil() as i32 + 1;

        let x_margin = (bounds.size.width % d) / 2.0;
        let y_margin = (bounds.size.height % d) / 2.0;

        for xi in 0..x_count {
            let x = bounds.origin.x + x_margin + (xi as f32 * d);
            for yi in 0..y_count {
                let y = bounds.origin.y + y_margin + (yi as f32 * d);
                if x >= bounds.origin.x && x <= bounds.origin.x + bounds.size.width
                    && y >= bounds.origin.y && y <= bounds.origin.y + bounds.size.height
                {
                    self.draw_dot(cx, x, y);
                }
            }
        }
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut crate::components::context::EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dots_grid_new() {
        let grid = DotsGrid::new();
        assert_eq!(grid.distance, 30.0);
        assert_eq!(grid.size, 2.0);
    }

    #[test]
    fn test_dots_grid_builders() {
        let grid = DotsGrid::new()
            .distance(50.0)
            .size(4.0)
            .shape(DotShape::Cross)
            .opacity(0.5);

        assert_eq!(grid.distance, 50.0);
        assert_eq!(grid.size, 4.0);
        assert_eq!(grid.shape, DotShape::Cross);
        assert_eq!(grid.opacity, 0.5);
    }

    #[test]
    fn test_dots_grid_clamping() {
        let grid = DotsGrid::new()
            .distance(2.0)
            .size(0.5)
            .opacity(1.5);

        assert_eq!(grid.distance, 5.0);
        assert_eq!(grid.size, 1.0);
        assert_eq!(grid.opacity, 1.0);
    }
}
