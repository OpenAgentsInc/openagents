use std::time::Duration;

use crate::animation::{Animation, AnimationController, AnimatorState, AnimatorTiming, Easing};
use crate::components::{Component, ComponentId, EventContext, EventResult, PaintContext};
use crate::{Bounds, Hsla, InputEvent, Point, Quad};

/// Radial glow effect that follows a target position.
pub struct Illuminator {
    id: Option<ComponentId>,
    position: Point,
    target: Point,
    radius: f32,
    color: Hsla,
    intensity: f32,
    rings: u32,
    segments: u32,
    smoothing: f32,
    animator: IlluminatorAnimator,
}

impl Illuminator {
    pub fn new() -> Self {
        Self {
            id: None,
            position: Point::new(0.0, 0.0),
            target: Point::new(0.0, 0.0),
            radius: 100.0,
            color: Hsla::new(180.0, 0.6, 0.7, 0.2),
            intensity: 1.0,
            rings: 10,
            segments: 48,
            smoothing: 0.15,
            animator: IlluminatorAnimator::new(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(5.0);
        self
    }

    pub fn size(mut self, diameter: f32) -> Self {
        self.radius = (diameter / 2.0).max(5.0);
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.max(0.0);
        self
    }

    pub fn rings(mut self, rings: u32) -> Self {
        self.rings = rings.max(2);
        self
    }

    pub fn segments(mut self, segments: u32) -> Self {
        self.segments = segments.max(8);
        self
    }

    pub fn smoothing(mut self, smoothing: f32) -> Self {
        self.smoothing = smoothing.clamp(0.01, 1.0);
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

    pub fn position(&self) -> Point {
        self.position
    }

    pub fn target(&self) -> Point {
        self.target
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.target = Point::new(x, y);
    }

    pub fn snap_to_position(&mut self, x: f32, y: f32) {
        let point = Point::new(x, y);
        self.target = point;
        self.position = point;
    }

    pub fn update(&mut self, state: AnimatorState) -> f32 {
        let progress = self.animator.update(state);
        let delta = self.animator.last_delta();
        self.advance(delta);
        progress
    }

    pub fn update_with_delta(&mut self, state: AnimatorState, delta: Duration) -> f32 {
        let progress = self.animator.update_with_delta(state, delta);
        self.advance(delta);
        progress
    }

    pub fn set_state(&mut self, state: AnimatorState) {
        self.animator.update_with_delta(state, Duration::ZERO);
    }

    fn advance(&mut self, delta: Duration) {
        let factor = smoothing_factor(self.smoothing, delta);
        self.position.x += (self.target.x - self.position.x) * factor;
        self.position.y += (self.target.y - self.position.y) * factor;
    }
}

impl Default for Illuminator {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Illuminator {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let progress = self.animator.progress();
        if progress <= 0.0 || self.radius <= 0.0 {
            return;
        }

        let margin = self.radius;
        if self.position.x < bounds.origin.x - margin
            || self.position.x > bounds.origin.x + bounds.size.width + margin
            || self.position.y < bounds.origin.y - margin
            || self.position.y > bounds.origin.y + bounds.size.height + margin
        {
            return;
        }

        let base_alpha = (self.color.a * self.intensity * progress).clamp(0.0, 1.0);
        if base_alpha <= 0.0 {
            return;
        }

        let ring_count = self.rings.max(2) as usize;
        let ring_step = (self.radius / ring_count as f32).max(1.0);
        let segments = self.segments.max(8) as usize;
        let color = self.color;

        for ring in 0..ring_count {
            let t = ring as f32 / (ring_count - 1) as f32;
            let ring_radius = self.radius * t;
            let ring_alpha = base_alpha * (1.0 - t).powi(2);
            if ring_alpha <= 0.001 {
                continue;
            }

            let ring_size = ring_step.max(1.0);
            let half = ring_size / 2.0;

            if ring_radius <= ring_size * 0.5 {
                let bounds = Bounds::new(
                    self.position.x - half,
                    self.position.y - half,
                    ring_size,
                    ring_size,
                );
                cx.scene
                    .draw_quad(Quad::new(bounds).with_background(color.with_alpha(ring_alpha)));
                continue;
            }

            for i in 0..segments {
                let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
                let x = self.position.x + angle.cos() * ring_radius;
                let y = self.position.y + angle.sin() * ring_radius;
                let bounds = Bounds::new(x - half, y - half, ring_size, ring_size);
                cx.scene
                    .draw_quad(Quad::new(bounds).with_background(color.with_alpha(ring_alpha)));
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

/// SVG output for the illuminator effect.
pub struct IlluminatorSvg {
    position: Point,
    radius: f32,
    color: Hsla,
    intensity: f32,
    progress: f32,
    gradient_id: String,
}

impl IlluminatorSvg {
    pub fn new() -> Self {
        Self {
            position: Point::new(0.0, 0.0),
            radius: 100.0,
            color: Hsla::new(180.0, 0.6, 0.7, 0.2),
            intensity: 1.0,
            progress: 1.0,
            gradient_id: "illuminator".to_string(),
        }
    }

    pub fn position(mut self, x: f32, y: f32) -> Self {
        self.position = Point::new(x, y);
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(5.0);
        self
    }

    pub fn size(mut self, diameter: f32) -> Self {
        self.radius = (diameter / 2.0).max(5.0);
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.max(0.0);
        self
    }

    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self
    }

    pub fn gradient_id(mut self, id: impl Into<String>) -> Self {
        self.gradient_id = id.into();
        self
    }

    pub fn to_svg(&self) -> String {
        let base_alpha = (self.color.a * self.intensity * self.progress).clamp(0.0, 1.0);
        let mid_alpha = (base_alpha * 0.4).clamp(0.0, 1.0);
        let color = hsl_css(self.color);

        format!(
            "<defs>\
<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\">\
<stop offset=\"0%\" stop-color=\"{color}\" stop-opacity=\"{a0}\"/>\
<stop offset=\"60%\" stop-color=\"{color}\" stop-opacity=\"{a1}\"/>\
<stop offset=\"100%\" stop-color=\"{color}\" stop-opacity=\"0\"/>\
</radialGradient>\
</defs>\
<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\" fill=\"url(#{id})\"/>",
            id = escape_svg(&self.gradient_id),
            cx = format!("{:.2}", self.position.x),
            cy = format!("{:.2}", self.position.y),
            r = format!("{:.2}", self.radius),
            color = color,
            a0 = format!("{:.3}", base_alpha),
            a1 = format!("{:.3}", mid_alpha),
        )
    }
}

impl Default for IlluminatorSvg {
    fn default() -> Self {
        Self::new()
    }
}

struct IlluminatorAnimator {
    controller: AnimationController,
    timing: AnimatorTiming,
    easing: Easing,
    state: AnimatorState,
    animation: Option<Animation<f32>>,
    progress: f32,
    last_delta: Duration,
}

impl IlluminatorAnimator {
    fn new() -> Self {
        Self {
            controller: AnimationController::new(),
            timing: AnimatorTiming::default(),
            easing: Easing::EaseInOut,
            state: AnimatorState::Exited,
            animation: None,
            progress: 0.0,
            last_delta: Duration::ZERO,
        }
    }

    fn set_timing(&mut self, timing: AnimatorTiming) {
        self.timing = timing;
    }

    fn set_easing(&mut self, easing: Easing) {
        self.easing = easing;
    }

    fn progress(&self) -> f32 {
        self.progress
    }

    fn last_delta(&self) -> Duration {
        self.last_delta
    }

    fn update(&mut self, state: AnimatorState) -> f32 {
        let delta = self.controller.delta();
        self.update_with_delta(state, delta)
    }

    fn update_with_delta(&mut self, state: AnimatorState, delta: Duration) -> f32 {
        if state != self.state {
            self.start_for(state);
        }

        if let Some(animation) = &mut self.animation {
            self.progress = animation.tick(delta);
            if animation.is_finished() {
                self.animation = None;
            }
        } else {
            self.progress = match state {
                AnimatorState::Entered => 1.0,
                AnimatorState::Exited => 0.0,
                AnimatorState::Entering | AnimatorState::Exiting => self.progress,
            };
        }

        self.last_delta = delta;
        self.progress
    }

    fn start_for(&mut self, state: AnimatorState) {
        self.controller.reset();
        self.state = state;
        self.animation = None;

        match state {
            AnimatorState::Entering => {
                let duration = ensure_non_zero(self.timing.enter);
                let mut animation = Animation::new(0.0, 1.0, duration).easing(self.easing);
                animation.start();
                self.progress = 0.0;
                self.animation = Some(animation);
            }
            AnimatorState::Exiting => {
                let duration = ensure_non_zero(self.timing.exit);
                let mut animation = Animation::new(1.0, 0.0, duration).easing(self.easing);
                animation.start();
                self.progress = 1.0;
                self.animation = Some(animation);
            }
            AnimatorState::Entered => {
                self.progress = 1.0;
            }
            AnimatorState::Exited => {
                self.progress = 0.0;
            }
        }
    }
}

fn smoothing_factor(smoothing: f32, delta: Duration) -> f32 {
    if delta.is_zero() {
        return 0.0;
    }
    let base = 1.0 - smoothing.clamp(0.01, 1.0);
    1.0 - base.powf(delta.as_secs_f32() * 60.0)
}

fn ensure_non_zero(duration: Duration) -> Duration {
    if duration.is_zero() {
        Duration::from_millis(1)
    } else {
        duration
    }
}

fn hsl_css(color: Hsla) -> String {
    format!(
        "hsl({:.0}, {:.0}%, {:.0}%)",
        color.h,
        color.s * 100.0,
        color.l * 100.0
    )
}

fn escape_svg(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_illuminator_snap() {
        let mut illum = Illuminator::new().smoothing(1.0);
        illum.set_position(100.0, 200.0);
        illum.update_with_delta(AnimatorState::Entered, Duration::from_millis(16));
        assert!((illum.position().x - 100.0).abs() < 0.01);
        assert!((illum.position().y - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_svg_output_contains_gradient() {
        let svg = IlluminatorSvg::new()
            .position(10.0, 20.0)
            .radius(50.0)
            .gradient_id("glow")
            .to_svg();
        assert!(svg.contains("radialGradient"));
        assert!(svg.contains("id=\"glow\""));
        assert!(svg.contains("cx=\"10.00\""));
        assert!(svg.contains("cy=\"20.00\""));
    }
}
