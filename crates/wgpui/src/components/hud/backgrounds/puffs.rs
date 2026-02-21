use std::time::Duration;

use crate::animation::{AnimatorState, AnimatorTiming, Easing};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad, Size};

use super::BackgroundAnimator;

/// Animated puffs background with expanding glow bursts (Arwes-style).
/// When `grid_distance` is set, puffs spawn on grid columns and float up along the grid.
pub struct PuffsBackground {
    id: Option<ComponentId>,
    color: Hsla,
    quantity: usize,
    padding: f32,
    /// When set, puffs spawn at x positions aligned to this spacing (grid columns).
    grid_distance: Option<f32>,
    x_offset: (f32, f32),
    y_offset: (f32, f32),
    radius_initial: f32,
    radius_offset: (f32, f32),
    sets: usize,
    layers: usize,
    seed: u64,
    cycle: CycleTimer,
    puff_easing: Easing,
    animator: BackgroundAnimator,
    puffs_sets: Vec<Vec<Puff>>,
    last_size: Option<Size>,
}

impl PuffsBackground {
    pub fn new() -> Self {
        Self {
            id: None,
            color: Hsla::new(180.0, 0.5, 0.7, 0.2),
            quantity: 10,
            padding: 50.0,
            grid_distance: None,
            x_offset: (0.0, 0.0),
            y_offset: (-10.0, -100.0),
            radius_initial: 4.0,
            radius_offset: (4.0, 40.0),
            sets: 5,
            layers: 8,
            seed: 1,
            cycle: CycleTimer::new(Duration::from_secs(2), Duration::ZERO),
            puff_easing: Easing::EaseOutSine,
            animator: BackgroundAnimator::new(),
            puffs_sets: Vec::new(),
            last_size: None,
        }
    }

    /// Spawn puffs on grid columns (x aligned to spacing); they float up along grid lines.
    pub fn grid_distance(mut self, distance: Option<f32>) -> Self {
        self.grid_distance = distance.filter(|&d| d > 0.0);
        self.last_size = None;
        self
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn quantity(mut self, quantity: usize) -> Self {
        self.quantity = quantity.max(1);
        self.last_size = None;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.0);
        self.last_size = None;
        self
    }

    /// Offset X as (fixed, random range).
    pub fn x_offset(mut self, offset: (f32, f32)) -> Self {
        self.x_offset = offset;
        self.last_size = None;
        self
    }

    /// Offset Y as (fixed, random range).
    pub fn y_offset(mut self, offset: (f32, f32)) -> Self {
        self.y_offset = offset;
        self.last_size = None;
        self
    }

    pub fn radius_initial(mut self, radius: f32) -> Self {
        self.radius_initial = radius.max(1.0);
        self.last_size = None;
        self
    }

    /// Radius offset as (fixed, random range).
    pub fn radius_offset(mut self, offset: (f32, f32)) -> Self {
        self.radius_offset = offset;
        self.last_size = None;
        self
    }

    pub fn sets(mut self, sets: usize) -> Self {
        self.sets = sets.max(1);
        self.last_size = None;
        self
    }

    pub fn layers(mut self, layers: usize) -> Self {
        self.layers = clamp_layers(layers);
        self
    }

    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
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

    pub fn puff_easing(mut self, easing: Easing) -> Self {
        self.puff_easing = easing;
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

    fn ensure_puffs(&mut self, size: Size) {
        if self.last_size == Some(size) && !self.puffs_sets.is_empty() {
            return;
        }

        let mut rng = PseudoRng::new(self.seed ^ size.width.to_bits() as u64);
        self.puffs_sets = create_puffs_sets(
            &mut rng,
            size,
            self.quantity,
            self.sets,
            self.padding,
            self.grid_distance,
            self.x_offset,
            self.y_offset,
            self.radius_initial,
            self.radius_offset,
        );
        self.last_size = Some(size);
    }
}

impl Default for PuffsBackground {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PuffsBackground {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let fade = self.animator.progress();
        if fade <= 0.0 {
            return;
        }

        let size = bounds.size;
        if size.is_empty() {
            return;
        }

        self.ensure_puffs(size);
        if self.puffs_sets.is_empty() {
            return;
        }

        let cycle_progress = self.cycle.progress();
        let sets = self.puffs_sets.len().max(1) as f32;

        for (set_index, puffs) in self.puffs_sets.iter().enumerate() {
            let set_offset = set_index as f32 / sets;
            let progress = wrap_unit(cycle_progress + set_offset);
            let eased = self.puff_easing.apply(progress);
            let puff_alpha = puff_alpha(progress) * fade;
            if puff_alpha <= 0.0 {
                continue;
            }

            for puff in puffs {
                let x = bounds.origin.x + puff.x + eased * puff.xo;
                let y = bounds.origin.y + puff.y + eased * puff.yo;
                let r = puff.r + eased * puff.ro;
                draw_puff(cx, x, y, r, self.layers, self.color, puff_alpha);
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
struct Puff {
    x: f32,
    y: f32,
    r: f32,
    xo: f32,
    yo: f32,
    ro: f32,
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

#[expect(clippy::too_many_arguments)]
fn create_puffs_sets(
    rng: &mut PseudoRng,
    size: Size,
    quantity: usize,
    sets: usize,
    padding: f32,
    grid_distance: Option<f32>,
    x_offset: (f32, f32),
    y_offset: (f32, f32),
    radius_initial: f32,
    radius_offset: (f32, f32),
) -> Vec<Vec<Puff>> {
    let sets = sets.max(1);
    let per_set = ((quantity as f32) / sets as f32).round().max(1.0) as usize;
    let width = (size.width - padding * 2.0).max(1.0);
    let height = (size.height - padding * 2.0).max(1.0);

    let grid_d = grid_distance.filter(|&v| v > 0.0);
    let n_cols = grid_d.map(|d| (width / d).floor().max(1.0) as u32);

    (0..sets)
        .map(|_| {
            (0..per_set)
                .map(|_| {
                    let x = match (grid_d, n_cols) {
                        (Some(d), Some(cols)) => {
                            let col = (rng.next_f32() * cols as f32).min((cols - 1) as f32) as u32;
                            padding + col as f32 * d
                        }
                        _ => padding + rng.next_f32() * width,
                    };
                    let y = padding + rng.next_f32() * height;
                    let r = radius_initial.max(1.0);
                    let xo = x_offset.0 + rng.next_f32() * x_offset.1;
                    let yo = y_offset.0 + rng.next_f32() * y_offset.1;
                    let ro = radius_offset.0 + rng.next_f32() * radius_offset.1;
                    Puff {
                        x,
                        y,
                        r,
                        xo,
                        yo,
                        ro,
                    }
                })
                .collect()
        })
        .collect()
}

fn draw_puff(
    cx: &mut PaintContext,
    x: f32,
    y: f32,
    radius: f32,
    layers: usize,
    color: Hsla,
    alpha: f32,
) {
    let layers = clamp_layers(layers);
    let base_alpha = color.a * alpha;

    for i in 0..layers {
        let t = (i + 1) as f32 / layers as f32;
        let layer_radius = radius * t;
        let layer_alpha = base_alpha * (1.0 - t).powi(2);
        if layer_alpha <= 0.0 {
            continue;
        }

        let size = (layer_radius * 2.0).max(1.0);
        let bounds = Bounds::new(x - layer_radius, y - layer_radius, size, size);
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(color.with_alpha(layer_alpha))
                .with_corner_radius(layer_radius),
        );
    }
}

fn puff_alpha(progress: f32) -> f32 {
    if progress <= 0.5 {
        (progress * 2.0).clamp(0.0, 1.0)
    } else {
        ((1.0 - progress) * 2.0).clamp(0.0, 1.0)
    }
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

fn clamp_layers(layers: usize) -> usize {
    layers.clamp(1, 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_puff_alpha_curve() {
        assert!((puff_alpha(0.0) - 0.0).abs() < 0.001);
        assert!((puff_alpha(0.5) - 1.0).abs() < 0.001);
        assert!((puff_alpha(1.0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_puffs_builder_clamps() {
        let puffs = PuffsBackground::new()
            .quantity(0)
            .padding(-10.0)
            .radius_initial(0.1)
            .layers(100)
            .seed(0);

        assert_eq!(puffs.quantity, 1);
        assert_eq!(puffs.padding, 0.0);
        assert_eq!(puffs.radius_initial, 1.0);
        assert_eq!(puffs.layers, 32);
        assert_eq!(puffs.seed, 0);
    }

    #[test]
    fn test_cycle_timer_progress() {
        let mut cycle = CycleTimer::new(Duration::from_secs(2), Duration::ZERO);
        cycle.update(Duration::from_secs(1), AnimatorState::Entered);
        assert!((cycle.progress() - 0.5).abs() < 0.01);
    }
}
