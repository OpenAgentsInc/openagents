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
    horizontal_dash: Vec<f32>,
    vertical_dash: Vec<f32>,
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
            horizontal_dash: vec![4.0],
            vertical_dash: Vec::new(),
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

    pub fn horizontal_dash(mut self, dash: impl Into<Vec<f32>>) -> Self {
        self.horizontal_dash = dash.into();
        self
    }

    pub fn vertical_dash(mut self, dash: impl Into<Vec<f32>>) -> Self {
        self.vertical_dash = dash.into();
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
        let spacing = self.spacing;
        if spacing <= 0.0 {
            return;
        }

        let x_length = 1 + (width / spacing).floor() as i32;
        let y_length = 1 + (height / spacing).floor() as i32;
        let x_margin = width % spacing;
        let y_margin = height % spacing;

        if self.vertical {
            let dash = normalized_dash(&self.vertical_dash);
            for index in 0..x_length {
                let line_x = x + x_margin / 2.0 + index as f32 * spacing;
                draw_dashed(cx, line_x - w / 2.0, y, w, height, false, color, &dash);
            }
        }

        if self.horizontal {
            let dash = normalized_dash(&self.horizontal_dash);
            for index in 0..y_length {
                let line_y = y + y_margin / 2.0 + index as f32 * spacing;
                draw_dashed(cx, x, line_y - w / 2.0, width, w, true, color, &dash);
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

    #[test]
    fn test_grid_lines_flags_and_progress() {
        let mut grid = GridLinesBackground::new()
            .horizontal(false)
            .vertical(false)
            .spacing(40.0)
            .line_width(2.0);

        assert!(!grid.horizontal);
        assert!(!grid.vertical);
        assert_eq!(grid.spacing, 40.0);
        assert_eq!(grid.line_width, 2.0);

        let progress = grid.update(AnimatorState::Entered);
        assert!(progress >= 0.0 && progress <= 1.0);
    }
}

fn normalized_dash(dash: &[f32]) -> Vec<f32> {
    let mut filtered: Vec<f32> = dash.iter().copied().filter(|v| *v > 0.0).collect();
    if filtered.len() == 1 {
        filtered.push(filtered[0]);
    } else if filtered.len() % 2 == 1 {
        filtered.push(*filtered.first().unwrap_or(&1.0));
    }
    filtered
}

#[expect(clippy::too_many_arguments)]
fn draw_dashed(
    cx: &mut PaintContext,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    horizontal: bool,
    color: Hsla,
    dash: &[f32],
) {
    if dash.is_empty() {
        cx.scene
            .draw_quad(Quad::new(Bounds::new(x, y, width, height)).with_background(color));
        return;
    }

    let length = if horizontal { width } else { height };
    let thickness = if horizontal { height } else { width };
    let mut pos = 0.0;
    let mut index = 0usize;

    while pos < length {
        let segment = dash[index % dash.len()];
        let next = (pos + segment).min(length);
        if index.is_multiple_of(2) {
            if horizontal {
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x + pos, y, next - pos, thickness))
                        .with_background(color),
                );
            } else {
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x, y + pos, thickness, next - pos))
                        .with_background(color),
                );
            }
        }
        pos = next;
        index += 1;
    }
}
