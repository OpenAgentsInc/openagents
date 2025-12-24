use std::time::Duration;

use crate::animation::{AnimatorState, AnimatorTiming, Easing};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

use super::BackgroundAnimator;

/// Direction for moving lines.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LineDirection {
    /// Lines move from left to right.
    #[default]
    Right,
    /// Lines move from right to left.
    Left,
    /// Lines move from top to bottom.
    Down,
    /// Lines move from bottom to top.
    Up,
}

/// Animated moving lines background.
pub struct MovingLinesBackground {
    id: Option<ComponentId>,
    spacing: f32,
    line_width: f32,
    color: Hsla,
    speed: f32,
    direction: LineDirection,
    offset: f32,
    animator: BackgroundAnimator,
}

impl MovingLinesBackground {
    pub fn new() -> Self {
        Self {
            id: None,
            spacing: 80.0,
            line_width: 1.0,
            color: Hsla::new(0.0, 0.0, 1.0, 0.08),
            speed: 30.0,
            direction: LineDirection::Right,
            offset: 0.0,
            animator: BackgroundAnimator::new(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(10.0);
        self
    }

    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width.max(0.5);
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Movement speed in pixels per second.
    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    pub fn direction(mut self, direction: LineDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn timing(mut self, timing: AnimatorTiming) -> Self {
        self.animator.set_timing(timing);
        self
    }

    pub fn set_timing(&mut self, timing: AnimatorTiming) {
        self.animator.set_timing(timing);
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.animator.set_easing(easing);
        self
    }

    pub fn set_easing(&mut self, easing: Easing) {
        self.animator.set_easing(easing);
    }

    pub fn progress(&self) -> f32 {
        self.animator.progress()
    }

    pub fn update(&mut self, state: AnimatorState) -> f32 {
        let progress = self.animator.update(state);
        let delta = self.animator.last_delta();
        self.advance(delta, state);
        progress
    }

    pub fn update_with_delta(&mut self, state: AnimatorState, delta: Duration) -> f32 {
        let progress = self.animator.update_with_delta(state, delta);
        self.advance(delta, state);
        progress
    }

    pub fn set_state(&mut self, state: AnimatorState) {
        self.animator.update_with_delta(state, Duration::ZERO);
    }

    fn advance(&mut self, delta: Duration, state: AnimatorState) {
        if !matches!(
            state,
            AnimatorState::Entering | AnimatorState::Entered | AnimatorState::Exiting
        ) {
            return;
        }

        let direction = match self.direction {
            LineDirection::Right | LineDirection::Down => 1.0,
            LineDirection::Left | LineDirection::Up => -1.0,
        };
        let step = self.speed * delta.as_secs_f32() * direction;

        if self.spacing > 0.0 {
            self.offset = (self.offset + step).rem_euclid(self.spacing);
        }
    }
}

impl Default for MovingLinesBackground {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for MovingLinesBackground {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let color = self.color.with_alpha(self.color.a * progress);
        let w = self.line_width;

        let x = bounds.origin.x;
        let y = bounds.origin.y;
        let width = bounds.size.width;
        let height = bounds.size.height;

        match self.direction {
            LineDirection::Right | LineDirection::Left => {
                let start_offset = match self.direction {
                    LineDirection::Right => self.offset,
                    LineDirection::Left => self.spacing - self.offset,
                    _ => 0.0,
                };

                let mut line_x = x - self.spacing + start_offset;
                while line_x <= x + width + self.spacing {
                    if line_x >= x - w && line_x <= x + width + w {
                        cx.scene.draw_quad(
                            Quad::new(Bounds::new(line_x - w / 2.0, y, w, height))
                                .with_background(color),
                        );
                    }
                    line_x += self.spacing;
                }
            }
            LineDirection::Down | LineDirection::Up => {
                let start_offset = match self.direction {
                    LineDirection::Down => self.offset,
                    LineDirection::Up => self.spacing - self.offset,
                    _ => 0.0,
                };

                let mut line_y = y - self.spacing + start_offset;
                while line_y <= y + height + self.spacing {
                    if line_y >= y - w && line_y <= y + height + w {
                        cx.scene.draw_quad(
                            Quad::new(Bounds::new(x, line_y - w / 2.0, width, w))
                                .with_background(color),
                        );
                    }
                    line_y += self.spacing;
                }
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
    fn test_moving_lines_update_offset() {
        let mut lines = MovingLinesBackground::new().spacing(50.0).speed(10.0);
        lines.update_with_delta(AnimatorState::Entered, Duration::from_secs(1));
        assert!((lines.offset - 10.0).abs() < 0.01);
    }
}
