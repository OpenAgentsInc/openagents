//! Animation System for wgpui
//!
//! Provides a declarative animation framework for smooth transitions.
//!
//! # Features
//! - Easing functions (linear, ease-in, ease-out, ease-in-out, cubic-bezier)
//! - Property animations (position, size, color, opacity)
//! - Keyframe sequences
//! - Spring physics animations
//! - Animation composition and chaining

mod animator;
mod easing;
mod transitions;

use crate::{Hsla, Point, Size};
use std::time::Duration;

pub use animator::{
    AnimatorId, AnimatorManagerKind, AnimatorMessage, AnimatorNode, AnimatorSettings,
    AnimatorSettingsUpdate, AnimatorState, AnimatorTiming, AnimatorTimingUpdate,
};
pub use easing::{EaseAmong, EaseSteps, EaseStepsDirection, Easing, ease_among, ease_steps};
pub use transitions::{Transition, TransitionAnimation, draw, fade, flicker, transition};

/// Animatable value that can be interpolated
pub trait Animatable: Clone + Copy {
    /// Linear interpolation between two values
    fn lerp(from: Self, to: Self, t: f32) -> Self;
}

impl Animatable for f32 {
    fn lerp(from: Self, to: Self, t: f32) -> Self {
        from + (to - from) * t
    }
}

impl Animatable for Point {
    fn lerp(from: Self, to: Self, t: f32) -> Self {
        Point::new(f32::lerp(from.x, to.x, t), f32::lerp(from.y, to.y, t))
    }
}

impl Animatable for Size {
    fn lerp(from: Self, to: Self, t: f32) -> Self {
        Size::new(
            f32::lerp(from.width, to.width, t),
            f32::lerp(from.height, to.height, t),
        )
    }
}

impl Animatable for Hsla {
    fn lerp(from: Self, to: Self, t: f32) -> Self {
        // Handle hue wrapping for shortest path
        let mut dh = to.h - from.h;
        if dh > 180.0 {
            dh -= 360.0;
        } else if dh < -180.0 {
            dh += 360.0;
        }

        Hsla::new(
            (from.h + dh * t).rem_euclid(360.0),
            f32::lerp(from.s, to.s, t),
            f32::lerp(from.l, to.l, t),
            f32::lerp(from.a, to.a, t),
        )
    }
}

/// Animation state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationState {
    /// Not yet started
    Pending,
    /// Currently playing
    Running,
    /// Paused
    Paused,
    /// Completed
    Finished,
}

/// A single property animation
#[derive(Debug, Clone)]
pub struct Animation<T: Animatable> {
    /// Starting value
    pub from: T,
    /// Ending value
    pub to: T,
    /// Animation duration
    pub duration: Duration,
    /// Easing function
    pub easing: Easing,
    /// Delay before starting
    pub delay: Duration,
    /// Number of iterations (0 = infinite)
    pub iterations: u32,
    /// Alternate direction on each iteration
    pub alternate: bool,
    /// Current state
    state: AnimationState,
    /// Elapsed time
    elapsed: Duration,
    /// Current iteration
    current_iteration: u32,
}

impl<T: Animatable> Animation<T> {
    /// Create a new animation
    pub fn new(from: T, to: T, duration: Duration) -> Self {
        Self {
            from,
            to,
            duration,
            easing: Easing::default(),
            delay: Duration::ZERO,
            iterations: 1,
            alternate: false,
            state: AnimationState::Pending,
            elapsed: Duration::ZERO,
            current_iteration: 0,
        }
    }

    /// Set easing function
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Set delay before starting
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Set number of iterations (0 = infinite)
    pub fn iterations(mut self, count: u32) -> Self {
        self.iterations = count;
        self
    }

    /// Enable alternating direction
    pub fn alternate(mut self) -> Self {
        self.alternate = true;
        self
    }

    /// Start the animation
    pub fn start(&mut self) {
        self.state = AnimationState::Running;
        self.elapsed = Duration::ZERO;
        self.current_iteration = 0;
    }

    /// Pause the animation
    pub fn pause(&mut self) {
        if self.state == AnimationState::Running {
            self.state = AnimationState::Paused;
        }
    }

    /// Resume the animation
    pub fn resume(&mut self) {
        if self.state == AnimationState::Paused {
            self.state = AnimationState::Running;
        }
    }

    /// Reset the animation
    pub fn reset(&mut self) {
        self.state = AnimationState::Pending;
        self.elapsed = Duration::ZERO;
        self.current_iteration = 0;
    }

    /// Get current state
    pub fn state(&self) -> AnimationState {
        self.state
    }

    /// Check if animation is running
    pub fn is_running(&self) -> bool {
        self.state == AnimationState::Running
    }

    /// Check if animation is finished
    pub fn is_finished(&self) -> bool {
        self.state == AnimationState::Finished
    }

    /// Advance animation by delta time and return current value
    pub fn tick(&mut self, delta: Duration) -> T {
        if self.state != AnimationState::Running {
            return if self.state == AnimationState::Finished {
                if self.alternate && self.current_iteration % 2 == 1 {
                    self.from
                } else {
                    self.to
                }
            } else {
                self.from
            };
        }

        self.elapsed += delta;

        // Handle delay
        if self.elapsed < self.delay {
            return self.from;
        }

        let active_elapsed = self.elapsed - self.delay;
        let progress = active_elapsed.as_secs_f32() / self.duration.as_secs_f32();

        // Check iteration completion
        if progress >= 1.0 {
            self.current_iteration += 1;
            if self.iterations > 0 && self.current_iteration >= self.iterations {
                self.state = AnimationState::Finished;
                return if self.alternate && self.iterations.is_multiple_of(2) {
                    self.from
                } else {
                    self.to
                };
            }
            // Reset for next iteration
            self.elapsed = self.delay;
        }

        let iteration_progress = progress.fract();
        let t = self.easing.apply(iteration_progress);

        // Handle alternating direction
        if self.alternate && self.current_iteration % 2 == 1 {
            T::lerp(self.to, self.from, t)
        } else {
            T::lerp(self.from, self.to, t)
        }
    }

    /// Get current value without advancing time
    pub fn current_value(&self) -> T {
        if self.state == AnimationState::Pending {
            return self.from;
        }
        if self.state == AnimationState::Finished {
            return if self.alternate && self.current_iteration % 2 == 1 {
                self.from
            } else {
                self.to
            };
        }

        let active_elapsed = if self.elapsed > self.delay {
            self.elapsed - self.delay
        } else {
            Duration::ZERO
        };

        let progress = (active_elapsed.as_secs_f32() / self.duration.as_secs_f32()).min(1.0);
        let t = self.easing.apply(progress);

        if self.alternate && self.current_iteration % 2 == 1 {
            T::lerp(self.to, self.from, t)
        } else {
            T::lerp(self.from, self.to, t)
        }
    }
}

/// Spring physics animation
#[derive(Debug, Clone)]
pub struct SpringAnimation<T: Animatable> {
    /// Target value
    pub target: T,
    /// Current value
    current: T,
    /// Current velocity
    velocity: T,
    /// Spring stiffness (higher = faster)
    pub stiffness: f32,
    /// Damping ratio (1.0 = critical damping)
    pub damping: f32,
    /// Mass
    pub mass: f32,
    /// Velocity threshold for completion
    pub threshold: f32,
    /// Whether the spring has settled
    settled: bool,
}

impl SpringAnimation<f32> {
    /// Create a new spring animation
    pub fn new(initial: f32, target: f32) -> Self {
        Self {
            target,
            current: initial,
            velocity: 0.0,
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
            threshold: 0.01,
            settled: false,
        }
    }

    /// Set spring stiffness
    pub fn stiffness(mut self, stiffness: f32) -> Self {
        self.stiffness = stiffness;
        self
    }

    /// Set damping ratio
    pub fn damping(mut self, damping: f32) -> Self {
        self.damping = damping;
        self
    }

    /// Set mass
    pub fn mass(mut self, mass: f32) -> Self {
        self.mass = mass;
        self
    }

    /// Update target value
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
        self.settled = false;
    }

    /// Check if spring has settled
    pub fn is_settled(&self) -> bool {
        self.settled
    }

    /// Advance spring physics by delta time
    pub fn tick(&mut self, delta: Duration) -> f32 {
        if self.settled {
            return self.target;
        }

        let dt = delta.as_secs_f32();
        let displacement = self.current - self.target;

        // Spring force: F = -kx - cv
        let spring_force = -self.stiffness * displacement;
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;

        // Semi-implicit Euler integration
        self.velocity += acceleration * dt;
        self.current += self.velocity * dt;

        // Check if settled
        if displacement.abs() < self.threshold && self.velocity.abs() < self.threshold {
            self.current = self.target;
            self.velocity = 0.0;
            self.settled = true;
        }

        self.current
    }

    /// Get current value
    pub fn current(&self) -> f32 {
        self.current
    }
}

impl SpringAnimation<Point> {
    /// Create a new spring animation for Point
    pub fn new_point(initial: Point, target: Point) -> Self {
        Self {
            target,
            current: initial,
            velocity: Point::new(0.0, 0.0),
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
            threshold: 0.01,
            settled: false,
        }
    }

    /// Set spring stiffness
    pub fn stiffness(mut self, stiffness: f32) -> Self {
        self.stiffness = stiffness;
        self
    }

    /// Set damping ratio
    pub fn damping(mut self, damping: f32) -> Self {
        self.damping = damping;
        self
    }

    /// Update target value
    pub fn set_target(&mut self, target: Point) {
        self.target = target;
        self.settled = false;
    }

    /// Check if spring has settled
    pub fn is_settled(&self) -> bool {
        self.settled
    }

    /// Advance spring physics by delta time
    pub fn tick(&mut self, delta: Duration) -> Point {
        if self.settled {
            return self.target;
        }

        let dt = delta.as_secs_f32();
        let dx = self.current.x - self.target.x;
        let dy = self.current.y - self.target.y;

        // Spring force for X
        let spring_force_x = -self.stiffness * dx;
        let damping_force_x = -self.damping * self.velocity.x;
        let accel_x = (spring_force_x + damping_force_x) / self.mass;

        // Spring force for Y
        let spring_force_y = -self.stiffness * dy;
        let damping_force_y = -self.damping * self.velocity.y;
        let accel_y = (spring_force_y + damping_force_y) / self.mass;

        // Semi-implicit Euler integration
        self.velocity.x += accel_x * dt;
        self.velocity.y += accel_y * dt;
        self.current.x += self.velocity.x * dt;
        self.current.y += self.velocity.y * dt;

        // Check if settled
        let displacement = (dx * dx + dy * dy).sqrt();
        let vel_mag =
            (self.velocity.x * self.velocity.x + self.velocity.y * self.velocity.y).sqrt();
        if displacement < self.threshold && vel_mag < self.threshold {
            self.current = self.target;
            self.velocity = Point::new(0.0, 0.0);
            self.settled = true;
        }

        self.current
    }

    /// Get current value
    pub fn current(&self) -> Point {
        self.current
    }
}

/// Keyframe in a keyframe animation
#[derive(Debug, Clone)]
pub struct Keyframe<T: Animatable> {
    /// Value at this keyframe
    pub value: T,
    /// Time offset from start (0.0 to 1.0)
    pub offset: f32,
    /// Easing to use for transition TO this keyframe
    pub easing: Easing,
}

impl<T: Animatable> Keyframe<T> {
    /// Create a new keyframe
    pub fn new(value: T, offset: f32) -> Self {
        Self {
            value,
            offset: offset.clamp(0.0, 1.0),
            easing: Easing::Linear,
        }
    }

    /// Set easing function
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

/// Keyframe animation with multiple waypoints
#[derive(Debug, Clone)]
pub struct KeyframeAnimation<T: Animatable> {
    /// Keyframes (must be sorted by offset)
    keyframes: Vec<Keyframe<T>>,
    /// Total duration
    pub duration: Duration,
    /// Number of iterations
    pub iterations: u32,
    /// Alternate direction
    pub alternate: bool,
    /// Current state
    state: AnimationState,
    /// Elapsed time
    elapsed: Duration,
    /// Current iteration
    current_iteration: u32,
}

impl<T: Animatable> KeyframeAnimation<T> {
    /// Create a new keyframe animation
    pub fn new(keyframes: Vec<Keyframe<T>>, duration: Duration) -> Self {
        let mut kf = keyframes;
        kf.sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap());

        Self {
            keyframes: kf,
            duration,
            iterations: 1,
            alternate: false,
            state: AnimationState::Pending,
            elapsed: Duration::ZERO,
            current_iteration: 0,
        }
    }

    /// Set number of iterations
    pub fn iterations(mut self, count: u32) -> Self {
        self.iterations = count;
        self
    }

    /// Enable alternating direction
    pub fn alternate(mut self) -> Self {
        self.alternate = true;
        self
    }

    /// Start the animation
    pub fn start(&mut self) {
        self.state = AnimationState::Running;
        self.elapsed = Duration::ZERO;
        self.current_iteration = 0;
    }

    /// Reset the animation
    pub fn reset(&mut self) {
        self.state = AnimationState::Pending;
        self.elapsed = Duration::ZERO;
        self.current_iteration = 0;
    }

    /// Get current state
    pub fn state(&self) -> AnimationState {
        self.state
    }

    /// Check if animation is finished
    pub fn is_finished(&self) -> bool {
        self.state == AnimationState::Finished
    }

    /// Advance animation and return current value
    pub fn tick(&mut self, delta: Duration) -> T {
        if self.keyframes.is_empty() {
            return T::lerp(
                self.keyframes.first().map(|k| k.value).unwrap_or_else(|| {
                    // This shouldn't happen, but provide a safe default
                    self.keyframes.first().unwrap().value
                }),
                self.keyframes.first().unwrap().value,
                0.0,
            );
        }

        if self.state != AnimationState::Running {
            return self.keyframes.last().unwrap().value;
        }

        self.elapsed += delta;
        let mut progress = self.elapsed.as_secs_f32() / self.duration.as_secs_f32();

        // Handle iteration completion
        if progress >= 1.0 {
            self.current_iteration += 1;
            if self.iterations > 0 && self.current_iteration >= self.iterations {
                self.state = AnimationState::Finished;
                return self.keyframes.last().unwrap().value;
            }
            self.elapsed = Duration::ZERO;
            progress = 0.0;
        }

        // Handle alternating
        let effective_progress = if self.alternate && self.current_iteration % 2 == 1 {
            1.0 - progress
        } else {
            progress
        };

        // Find surrounding keyframes
        let mut prev_kf = &self.keyframes[0];
        let mut next_kf = &self.keyframes[0];

        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.offset <= effective_progress {
                prev_kf = kf;
                next_kf = self.keyframes.get(i + 1).unwrap_or(kf);
            }
        }

        if prev_kf.offset == next_kf.offset {
            return prev_kf.value;
        }

        // Interpolate between keyframes
        let segment_progress =
            (effective_progress - prev_kf.offset) / (next_kf.offset - prev_kf.offset);
        let t = next_kf.easing.apply(segment_progress);

        T::lerp(prev_kf.value, next_kf.value, t)
    }
}

/// Animation controller that manages multiple animations
#[derive(Debug, Default)]
pub struct AnimationController {
    /// Timestamp of last tick (uses web_time for cross-platform WASM support)
    last_tick: Option<web_time::Instant>,
}

impl AnimationController {
    /// Create a new animation controller
    pub fn new() -> Self {
        Self { last_tick: None }
    }

    /// Get delta time since last tick
    pub fn delta(&mut self) -> Duration {
        let now = web_time::Instant::now();
        let delta = self.last_tick.map(|t| now - t).unwrap_or(Duration::ZERO);
        self.last_tick = Some(now);
        delta
    }

    /// Reset the controller
    pub fn reset(&mut self) {
        self.last_tick = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_linear() {
        let easing = Easing::Linear;
        assert!((easing.apply(0.0) - 0.0).abs() < 0.001);
        assert!((easing.apply(0.5) - 0.5).abs() < 0.001);
        assert!((easing.apply(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_easing_ease_in() {
        let easing = Easing::EaseIn;
        assert!((easing.apply(0.0) - 0.0).abs() < 0.001);
        assert!(easing.apply(0.5) < 0.5); // Slower at start
        assert!((easing.apply(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_easing_ease_out() {
        let easing = Easing::EaseOut;
        assert!((easing.apply(0.0) - 0.0).abs() < 0.001);
        assert!(easing.apply(0.5) > 0.5); // Faster at start
        assert!((easing.apply(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_animation_basic() {
        let mut anim =
            Animation::new(0.0_f32, 100.0, Duration::from_millis(100)).easing(Easing::Linear);

        anim.start();
        assert!(anim.is_running());

        // Tick to 50%
        let val = anim.tick(Duration::from_millis(50));
        assert!((val - 50.0).abs() < 1.0);

        // Tick to 100%
        let val = anim.tick(Duration::from_millis(50));
        assert!((val - 100.0).abs() < 1.0);
        assert!(anim.is_finished());
    }

    #[test]
    fn test_animation_with_delay() {
        let mut anim = Animation::new(0.0_f32, 100.0, Duration::from_millis(100))
            .delay(Duration::from_millis(50));

        anim.start();

        // During delay
        let val = anim.tick(Duration::from_millis(25));
        assert!((val - 0.0).abs() < 0.001);

        // After delay, at 50%
        let val = anim.tick(Duration::from_millis(75));
        assert!((val - 50.0).abs() < 5.0);
    }

    #[test]
    fn test_animation_iterations() {
        let mut anim = Animation::new(0.0_f32, 100.0, Duration::from_millis(100))
            .iterations(2)
            .easing(Easing::Linear);

        anim.start();

        // Complete first iteration
        anim.tick(Duration::from_millis(100));
        assert!(anim.is_running());

        // Complete second iteration
        anim.tick(Duration::from_millis(100));
        assert!(anim.is_finished());
    }

    #[test]
    fn test_animation_alternate() {
        let mut anim = Animation::new(0.0_f32, 100.0, Duration::from_millis(100))
            .iterations(2)
            .alternate()
            .easing(Easing::Linear);

        anim.start();

        // First iteration - forward
        let val = anim.tick(Duration::from_millis(50));
        assert!((val - 50.0).abs() < 5.0);

        anim.tick(Duration::from_millis(50)); // Complete first

        // Second iteration - backward
        let val = anim.tick(Duration::from_millis(50));
        assert!((val - 50.0).abs() < 5.0);
    }

    #[test]
    fn test_spring_animation() {
        let mut spring = SpringAnimation::new(0.0, 100.0)
            .stiffness(200.0)
            .damping(20.0);

        // Simulate several frames
        for _ in 0..100 {
            spring.tick(Duration::from_millis(16));
        }

        // Should be close to target
        assert!((spring.current() - 100.0).abs() < 1.0);
        assert!(spring.is_settled());
    }

    #[test]
    fn test_point_lerp() {
        let from = Point::new(0.0, 0.0);
        let to = Point::new(100.0, 200.0);

        let mid = Point::lerp(from, to, 0.5);
        assert!((mid.x - 50.0).abs() < 0.001);
        assert!((mid.y - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_color_lerp() {
        let from = Hsla::new(0.0, 1.0, 0.5, 1.0);
        let to = Hsla::new(120.0, 1.0, 0.5, 1.0);

        let mid = Hsla::lerp(from, to, 0.5);
        assert!((mid.h - 60.0).abs() < 1.0);
    }

    #[test]
    fn test_keyframe_animation() {
        let keyframes = vec![
            Keyframe::new(0.0_f32, 0.0),
            Keyframe::new(50.0, 0.5).easing(Easing::Linear),
            Keyframe::new(100.0, 1.0).easing(Easing::Linear),
        ];

        let mut anim = KeyframeAnimation::new(keyframes, Duration::from_millis(100));
        anim.start();

        // At 25% - between first and second keyframe
        let val = anim.tick(Duration::from_millis(25));
        assert!((val - 25.0).abs() < 5.0);

        // At 75% - between second and third keyframe
        let val = anim.tick(Duration::from_millis(50));
        assert!((val - 75.0).abs() < 5.0);
    }

    #[test]
    fn test_animation_controller() {
        let mut controller = AnimationController::new();

        // First call returns zero
        let delta1 = controller.delta();
        assert!(delta1.as_millis() == 0);

        // Subsequent calls return elapsed time
        std::thread::sleep(Duration::from_millis(10));
        let delta2 = controller.delta();
        assert!(delta2.as_millis() >= 5); // Allow some tolerance
    }

    #[test]
    fn test_easing_cubic_bezier() {
        let easing = Easing::CubicBezier(0.25, 0.1, 0.25, 1.0);
        assert!((easing.apply(0.0) - 0.0).abs() < 0.01);
        assert!((easing.apply(1.0) - 1.0).abs() < 0.01);
        // Middle should be valid
        let mid = easing.apply(0.5);
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn test_easing_elastic() {
        let easing = Easing::EaseOutElastic;
        assert!((easing.apply(0.0) - 0.0).abs() < 0.001);
        assert!((easing.apply(1.0) - 1.0).abs() < 0.001);
        // Elastic can overshoot
        let mid = easing.apply(0.5);
        assert!(mid > 0.0);
    }

    #[test]
    fn test_transition_builder() {
        let mut simple_transition = transition(
            0.0_f32,
            1.0,
            Duration::from_millis(10),
            Easing::Linear,
            None,
        );
        simple_transition.entering.start();
        let v = simple_transition.entering.tick(Duration::from_millis(5));
        assert!((v - 0.5).abs() < 0.1);

        let mut back_transition = transition(
            0.0_f32,
            1.0,
            Duration::from_millis(10),
            Easing::Linear,
            Some(-1.0),
        );
        back_transition.exiting.start();
        let v = back_transition.exiting.tick(Duration::from_millis(10));
        assert!((v + 1.0).abs() < 0.1);
    }
}
