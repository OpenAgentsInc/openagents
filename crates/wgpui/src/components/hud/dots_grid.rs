use crate::animation::{AnimatorState, AnimatorTiming, Easing};
use crate::components::context::PaintContext;
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

use super::backgrounds::BackgroundAnimator;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum DotShape {
    #[default]
    Box,
    Circle,
    Cross,
}

/// Origin point for animation reveal effect.
/// Dots animate outward from this point.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum DotsOrigin {
    /// Animate from left edge
    Left,
    /// Animate from right edge
    Right,
    /// Animate from top edge
    Top,
    /// Animate from bottom edge
    Bottom,
    /// Animate from center
    #[default]
    Center,
    /// Animate from custom point (x%, y%) where 0.0-1.0
    Point(f32, f32),
}

pub struct DotsGrid {
    id: Option<ComponentId>,
    color: Hsla,
    shape: DotShape,
    distance: f32,
    size: f32,
    cross_thickness: f32,
    opacity: f32,
    origin: DotsOrigin,
    animation_progress: f32,
    origin_inverted: bool,
    easing: Easing,
    animator: BackgroundAnimator,
    animator_enabled: bool,
    state: AnimatorState,
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
            origin: DotsOrigin::Center,
            animation_progress: 1.0,
            origin_inverted: false,
            easing: Easing::EaseIn,
            animator: BackgroundAnimator::new(),
            animator_enabled: false,
            state: AnimatorState::Entered,
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

    pub fn origin(mut self, origin: DotsOrigin) -> Self {
        self.origin = origin;
        self
    }

    pub fn animation_progress(mut self, progress: f32) -> Self {
        self.animation_progress = progress.clamp(0.0, 1.0);
        self.animator_enabled = false;
        self.state = match self.animation_progress {
            p if p <= 0.0 => AnimatorState::Exited,
            p if p >= 1.0 => AnimatorState::Entered,
            _ => AnimatorState::Entering,
        };
        self
    }

    pub fn origin_inverted(mut self, inverted: bool) -> Self {
        self.origin_inverted = inverted;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self.animator.set_easing(easing);
        self
    }

    pub fn timing(mut self, timing: AnimatorTiming) -> Self {
        self.animator.set_timing(timing);
        self.animator_enabled = true;
        self
    }

    pub fn set_timing(&mut self, timing: AnimatorTiming) {
        self.animator.set_timing(timing);
        self.animator_enabled = true;
    }

    pub fn set_easing(&mut self, easing: Easing) {
        self.easing = easing;
        self.animator.set_easing(easing);
    }

    pub fn progress(&self) -> f32 {
        if self.animator_enabled {
            self.animator.progress()
        } else {
            self.animation_progress
        }
    }

    pub fn update(&mut self, state: AnimatorState) -> f32 {
        self.animator_enabled = true;
        self.state = state;
        let progress = self.animator.update(state);
        self.animation_progress = progress;
        progress
    }

    pub fn update_with_delta(&mut self, state: AnimatorState, delta: std::time::Duration) -> f32 {
        self.animator_enabled = true;
        self.state = state;
        let progress = self.animator.update_with_delta(state, delta);
        self.animation_progress = progress;
        progress
    }

    pub fn set_state(&mut self, state: AnimatorState) {
        self.animator_enabled = true;
        self.state = state;
        self.animation_progress = self
            .animator
            .update_with_delta(state, std::time::Duration::ZERO);
    }

    fn distance_from_origin(&self, x: f32, y: f32, width: f32, height: f32) -> f32 {
        match self.origin {
            DotsOrigin::Left => x / width,
            DotsOrigin::Right => 1.0 - x / width,
            DotsOrigin::Top => y / height,
            DotsOrigin::Bottom => 1.0 - y / height,
            DotsOrigin::Center => self.distance_from_point(x, y, width, height, 0.5, 0.5),
            DotsOrigin::Point(px, py) => self.distance_from_point(x, y, width, height, px, py),
        }
    }

    fn distance_from_point(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        origin_x: f32,
        origin_y: f32,
    ) -> f32 {
        let ox = width * origin_x;
        let oy = height * origin_y;
        let dist = ((x - ox).powi(2) + (y - oy).powi(2)).sqrt();

        let corner_x = if origin_x < 0.5 { width } else { 0.0 };
        let corner_y = if origin_y < 0.5 { height } else { 0.0 };
        let max_dist = ((ox - corner_x).powi(2) + (oy - corner_y).powi(2)).sqrt();

        if max_dist > 0.0 { dist / max_dist } else { 0.0 }
    }

    fn dot_alpha(&self, distance_progress: f32) -> f32 {
        let invert_origin = match self.state {
            AnimatorState::Exiting => !self.origin_inverted,
            _ => self.origin_inverted,
        };
        let dist = if invert_origin {
            1.0 - distance_progress
        } else {
            distance_progress
        };

        let eased_progress = self.easing.apply(self.animation_progress);

        if dist <= 0.0 {
            return eased_progress;
        }

        let alpha_progress = eased_progress / dist;
        let alpha = alpha_progress.clamp(0.0, 1.0);
        if self.state == AnimatorState::Exiting {
            1.0 - alpha
        } else {
            alpha
        }
    }

    fn draw_dot(&self, cx: &mut PaintContext, x: f32, y: f32, alpha: f32) {
        let color = self.color.with_alpha(self.color.a * self.opacity * alpha);
        let s = self.size;
        let half = s / 2.0;

        match self.shape {
            DotShape::Box => {
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x - half, y - half, s, s)).with_background(color),
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
                        Quad::new(Bounds::new(
                            min_x,
                            min_y,
                            (max_x - min_x).max(1.0),
                            (max_y - min_y).max(1.0),
                        ))
                        .with_background(color),
                    );
                }
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x - half, y - half, s, s)).with_background(color),
                );
            }
            DotShape::Cross => {
                let t = self.cross_thickness;
                let half_t = t / 2.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x - half, y - half_t, s, t)).with_background(color),
                );
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x - half_t, y - half, t, s)).with_background(color),
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
        if self.animation_progress <= 0.0 {
            return;
        }

        let d = self.distance;
        let x_count = (bounds.size.width / d).ceil() as i32 + 1;
        let y_count = (bounds.size.height / d).ceil() as i32 + 1;

        let x_margin = (bounds.size.width % d) / 2.0;
        let y_margin = (bounds.size.height % d) / 2.0;

        let w = bounds.size.width;
        let h = bounds.size.height;

        for xi in 0..x_count {
            let local_x = x_margin + (xi as f32 * d);
            let x = bounds.origin.x + local_x;
            for yi in 0..y_count {
                let local_y = y_margin + (yi as f32 * d);
                let y = bounds.origin.y + local_y;
                if x >= bounds.origin.x
                    && x <= bounds.origin.x + w
                    && y >= bounds.origin.y
                    && y <= bounds.origin.y + h
                {
                    let dist = self.distance_from_origin(local_x, local_y, w, h);
                    let alpha = self.dot_alpha(dist);
                    if alpha > 0.001 {
                        self.draw_dot(cx, x, y, alpha);
                    }
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
        assert_eq!(grid.animation_progress, 1.0);
        assert_eq!(grid.origin, DotsOrigin::Center);
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
        let grid = DotsGrid::new().distance(2.0).size(0.5).opacity(1.5);

        assert_eq!(grid.distance, 5.0);
        assert_eq!(grid.size, 1.0);
        assert_eq!(grid.opacity, 1.0);
    }

    #[test]
    fn test_dots_grid_animation_builders() {
        let grid = DotsGrid::new()
            .origin(DotsOrigin::Left)
            .animation_progress(0.5)
            .origin_inverted(true)
            .easing(Easing::EaseOut);

        assert_eq!(grid.origin, DotsOrigin::Left);
        assert_eq!(grid.animation_progress, 0.5);
        assert!(grid.origin_inverted);
    }

    #[test]
    fn test_dots_grid_animation_progress_clamping() {
        let grid = DotsGrid::new().animation_progress(1.5);
        assert_eq!(grid.animation_progress, 1.0);

        let grid = DotsGrid::new().animation_progress(-0.5);
        assert_eq!(grid.animation_progress, 0.0);
    }

    #[test]
    fn test_distance_from_origin_left() {
        let grid = DotsGrid::new().origin(DotsOrigin::Left);
        let dist = grid.distance_from_origin(0.0, 50.0, 100.0, 100.0);
        assert!((dist - 0.0).abs() < 0.01);

        let dist = grid.distance_from_origin(100.0, 50.0, 100.0, 100.0);
        assert!((dist - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_distance_from_origin_center() {
        let grid = DotsGrid::new().origin(DotsOrigin::Center);
        let dist = grid.distance_from_origin(50.0, 50.0, 100.0, 100.0);
        assert!((dist - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_dot_alpha_full_progress() {
        let grid = DotsGrid::new().animation_progress(1.0);
        let alpha = grid.dot_alpha(0.5);
        assert!((alpha - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_dot_alpha_zero_progress() {
        let grid = DotsGrid::new().animation_progress(0.0);
        let alpha = grid.dot_alpha(0.5);
        assert!((alpha - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_dot_alpha_partial_progress() {
        let grid = DotsGrid::new()
            .animation_progress(0.5)
            .easing(Easing::Linear);

        let alpha_near = grid.dot_alpha(0.25);
        let alpha_far = grid.dot_alpha(0.75);

        assert!(alpha_near > alpha_far);
    }
}
