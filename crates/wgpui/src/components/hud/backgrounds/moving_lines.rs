use std::time::Duration;

use crate::animation::{AnimatorState, AnimatorTiming, Easing};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad, Size};

use super::BackgroundAnimator;

/// Direction for moving lines.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LineDirection {
    /// Lines move from left to right.
    Right,
    /// Lines move from right to left.
    Left,
    /// Lines move from top to bottom.
    Down,
    /// Lines move from bottom to top.
    #[default]
    Up,
}

/// Animated moving lines background.
pub struct MovingLinesBackground {
    id: Option<ComponentId>,
    spacing: f32,
    line_width: f32,
    color: Hsla,
    sets: usize,
    seed: u64,
    direction: LineDirection,
    cycle: CycleTimer,
    line_easing: Easing,
    animator: BackgroundAnimator,
    lines_sets: Vec<Vec<MovingLine>>,
    last_size: Option<Size>,
}

impl MovingLinesBackground {
    pub fn new() -> Self {
        Self {
            id: None,
            spacing: 80.0,
            line_width: 1.0,
            color: Hsla::new(0.0, 0.0, 1.0, 0.08),
            sets: 5,
            seed: 1,
            direction: LineDirection::Up,
            cycle: CycleTimer::new(Duration::from_secs(10), Duration::ZERO),
            line_easing: Easing::EaseInOutCubic,
            animator: BackgroundAnimator::new(),
            lines_sets: Vec::new(),
            last_size: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(10.0);
        self.last_size = None;
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

    pub fn sets(mut self, sets: usize) -> Self {
        self.sets = sets.max(1);
        self.last_size = None;
        self
    }

    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed.max(1);
        self.last_size = None;
        self
    }

    pub fn direction(mut self, direction: LineDirection) -> Self {
        self.direction = direction;
        self.last_size = None;
        self
    }

    pub fn cycle_duration(mut self, duration: Duration) -> Self {
        self.cycle.set_interval(duration);
        self
    }

    pub fn cycle_pause(mut self, duration: Duration) -> Self {
        self.cycle.set_pause(duration);
        self
    }

    pub fn line_easing(mut self, easing: Easing) -> Self {
        self.line_easing = easing;
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
        self.cycle.update(delta, state);
        progress
    }

    pub fn update_with_delta(&mut self, state: AnimatorState, delta: Duration) -> f32 {
        let progress = self.animator.update_with_delta(state, delta);
        self.cycle.update(delta, state);
        progress
    }

    pub fn set_state(&mut self, state: AnimatorState) {
        self.animator.update_with_delta(state, Duration::ZERO);
        self.cycle.update(Duration::ZERO, state);
    }

    fn ensure_lines(&mut self, size: Size) {
        if self.last_size == Some(size) && !self.lines_sets.is_empty() {
            return;
        }

        let mut rng = PseudoRng::new(self.seed ^ size.width.to_bits() as u64);
        let (axis1_size, axis2_size) = match self.direction {
            LineDirection::Left | LineDirection::Right => (size.height, size.width),
            LineDirection::Up | LineDirection::Down => (size.width, size.height),
        };

        self.lines_sets =
            create_lines_sets(&mut rng, axis1_size, axis2_size, self.spacing, self.sets);
        self.last_size = Some(size);
    }
}

impl Default for MovingLinesBackground {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for MovingLinesBackground {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let fade = self.animator.progress();
        if fade <= 0.0 {
            return;
        }

        let size = bounds.size;
        if size.is_empty() {
            return;
        }

        self.ensure_lines(size);
        if self.lines_sets.is_empty() {
            return;
        }

        let line_color = self.color.with_alpha(self.color.a * fade);
        let line_width = self.line_width;
        let cycle_progress = self.cycle.progress();
        let sets_count = self.lines_sets.len().max(1) as f32;

        let (axis1_size, axis2_size) = match self.direction {
            LineDirection::Left | LineDirection::Right => (size.height, size.width),
            LineDirection::Up | LineDirection::Down => (size.width, size.height),
        };

        for (set_index, lines) in self.lines_sets.iter().enumerate() {
            let set_offset = set_index as f32 / sets_count;
            let progress = wrap_unit(cycle_progress + set_offset);
            let eased = self.line_easing.apply(progress);
            let axis2_move = axis2_size * 2.0 * eased - axis2_size;

            for line in lines {
                let (start, end) = match self.direction {
                    LineDirection::Up | LineDirection::Left => {
                        let start = axis2_size - (line.axis2_initial + axis2_move);
                        let end = axis2_size - (line.axis2_initial + line.length + axis2_move);
                        if start < end {
                            (start, end)
                        } else {
                            (end, start)
                        }
                    }
                    LineDirection::Down | LineDirection::Right => {
                        let start = line.axis2_initial + axis2_move;
                        let end = line.axis2_initial + line.length + axis2_move;
                        if start < end {
                            (start, end)
                        } else {
                            (end, start)
                        }
                    }
                };

                let clamped_start = start.max(0.0).min(axis2_size);
                let clamped_end = end.max(0.0).min(axis2_size);
                if clamped_end <= clamped_start {
                    continue;
                }

                match self.direction {
                    LineDirection::Up | LineDirection::Down => {
                        let x = bounds.origin.x + line.axis1 - line_width / 2.0;
                        let y = bounds.origin.y + clamped_start;
                        let h = clamped_end - clamped_start;
                        if x + line_width < bounds.origin.x || x > bounds.origin.x + axis1_size {
                            continue;
                        }
                        cx.scene.draw_quad(
                            Quad::new(Bounds::new(x, y, line_width, h)).with_background(line_color),
                        );
                    }
                    LineDirection::Left | LineDirection::Right => {
                        let y = bounds.origin.y + line.axis1 - line_width / 2.0;
                        let x = bounds.origin.x + clamped_start;
                        let w = clamped_end - clamped_start;
                        if y + line_width < bounds.origin.y || y > bounds.origin.y + axis1_size {
                            continue;
                        }
                        cx.scene.draw_quad(
                            Quad::new(Bounds::new(x, y, w, line_width)).with_background(line_color),
                        );
                    }
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

#[derive(Clone, Copy, Debug)]
struct MovingLine {
    axis1: f32,
    axis2_initial: f32,
    length: f32,
}

struct CycleTimer {
    interval: Duration,
    pause: Duration,
    elapsed: Duration,
    pause_elapsed: Duration,
    paused: bool,
}

impl CycleTimer {
    fn new(interval: Duration, pause: Duration) -> Self {
        Self {
            interval: ensure_non_zero(interval),
            pause,
            elapsed: Duration::ZERO,
            pause_elapsed: Duration::ZERO,
            paused: false,
        }
    }

    fn set_interval(&mut self, interval: Duration) {
        self.interval = ensure_non_zero(interval);
    }

    fn set_pause(&mut self, pause: Duration) {
        self.pause = pause;
    }

    fn update(&mut self, delta: Duration, state: AnimatorState) {
        if !matches!(
            state,
            AnimatorState::Entering | AnimatorState::Entered | AnimatorState::Exiting
        ) {
            self.reset();
            return;
        }

        if self.paused {
            self.pause_elapsed += delta;
            if self.pause_elapsed >= self.pause {
                self.pause_elapsed = Duration::ZERO;
                self.paused = false;
                self.elapsed = Duration::ZERO;
            }
            return;
        }

        self.elapsed += delta;
        if self.elapsed >= self.interval {
            self.elapsed -= self.interval;
            if !self.pause.is_zero() {
                self.paused = true;
                self.pause_elapsed = Duration::ZERO;
            }
        }
    }

    fn progress(&self) -> f32 {
        if self.paused {
            return 1.0;
        }
        let secs = self.interval.as_secs_f32();
        if secs <= 0.0 {
            1.0
        } else {
            (self.elapsed.as_secs_f32() / secs).clamp(0.0, 1.0)
        }
    }

    fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
        self.pause_elapsed = Duration::ZERO;
        self.paused = false;
    }
}

struct PseudoRng {
    state: u64,
}

impl PseudoRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 32) as u32
    }

    fn next_f32(&mut self) -> f32 {
        let value = self.next_u32() as f32 / u32::MAX as f32;
        value.clamp(0.0, 1.0)
    }
}

fn create_lines_sets(
    rng: &mut PseudoRng,
    axis1_size: f32,
    axis2_size: f32,
    spacing: f32,
    sets: usize,
) -> Vec<Vec<MovingLine>> {
    if axis1_size <= 0.0 || axis2_size <= 0.0 || spacing <= 0.0 {
        return Vec::new();
    }

    let positions_length = 1 + (axis1_size / spacing).floor().max(0.0) as usize;
    let margin = axis1_size % spacing;
    let sets = sets.max(1);

    (0..sets)
        .map(|_| {
            let lines_length = ((random_range(rng, 0.1, 0.5) * positions_length as f32).floor()
                as usize)
                .max(1)
                .min(positions_length);
            let indices = shuffled_indices(rng, positions_length);

            indices
                .into_iter()
                .take(lines_length)
                .map(|position| {
                    let axis1 = margin / 2.0 + position as f32 * spacing;
                    let axis2_initial = rng.next_f32() * (axis2_size / 2.0);
                    let length = (random_range(rng, 0.1, 0.5) * axis2_size).floor().max(1.0);
                    MovingLine {
                        axis1,
                        axis2_initial,
                        length,
                    }
                })
                .collect()
        })
        .collect()
}

fn shuffled_indices(rng: &mut PseudoRng, len: usize) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..len).collect();
    if len <= 1 {
        return indices;
    }

    for i in (1..indices.len()).rev() {
        let j = (rng.next_f32() * (i + 1) as f32).floor() as usize;
        indices.swap(i, j);
    }
    indices
}

fn random_range(rng: &mut PseudoRng, min: f32, max: f32) -> f32 {
    min + (max - min) * rng.next_f32()
}

fn wrap_unit(value: f32) -> f32 {
    value.rem_euclid(1.0)
}

fn ensure_non_zero(duration: Duration) -> Duration {
    if duration.is_zero() {
        Duration::from_millis(1)
    } else {
        duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moving_lines_cycle_progress() {
        let mut lines = MovingLinesBackground::new().cycle_duration(Duration::from_secs(2));
        lines.update_with_delta(AnimatorState::Entered, Duration::from_secs(1));
        assert!((lines.cycle.progress() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_moving_lines_builder_clamps_and_direction() {
        let lines = MovingLinesBackground::new()
            .spacing(2.0)
            .line_width(0.1)
            .sets(0)
            .seed(0)
            .direction(LineDirection::Down);

        assert!(lines.spacing >= 10.0);
        assert!(lines.line_width >= 0.5);
        assert!(lines.sets >= 1);
        assert!(lines.seed >= 1);
        assert_eq!(lines.direction, LineDirection::Down);
    }
}
