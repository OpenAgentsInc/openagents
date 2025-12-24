use std::time::Duration;

use crate::animation::{AnimatorState, AnimatorTiming, Easing};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

use super::BackgroundAnimator;

/// Animated grid line background pattern.
pub struct GridLinesBackground {
    id: Option<ComponentId>,
    spacing: f32,
    line_width: f32,
    color: Hsla,
    horizontal: bool,
    vertical: bool,
    animator: BackgroundAnimator,
}

impl GridLinesBackground {
    pub fn new() -> Self {
        Self {
            id: None,
            spacing: 50.0,
            line_width: 1.0,
            color: Hsla::new(0.0, 0.0, 1.0, 0.1),
            horizontal: true,
            vertical: true,
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

    pub fn horizontal(mut self, enabled: bool) -> Self {
        self.horizontal = enabled;
        self
    }

    pub fn vertical(mut self, enabled: bool) -> Self {
        self.vertical = enabled;
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
        self.animator.update(state)
    }

    pub fn update_with_delta(&mut self, state: AnimatorState, delta: Duration) -> f32 {
        self.animator.update_with_delta(state, delta)
    }

    pub fn set_state(&mut self, state: AnimatorState) {
        self.animator.update_with_delta(state, Duration::ZERO);
    }
}

impl Default for GridLinesBackground {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for GridLinesBackground {
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

        if self.vertical {
            let mut line_x = x;
            while line_x <= x + width {
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(line_x - w / 2.0, y, w, height))
                        .with_background(color),
                );
                line_x += self.spacing;
            }
        }

        if self.horizontal {
            let mut line_y = y;
            while line_y <= y + height {
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x, line_y - w / 2.0, width, w))
                        .with_background(color),
                );
                line_y += self.spacing;
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
    fn test_grid_lines_builder_clamps() {
        let grid = GridLinesBackground::new().spacing(2.0).line_width(0.1);
        assert!(grid.spacing >= 10.0);
        assert!(grid.line_width >= 0.5);
    }
}
