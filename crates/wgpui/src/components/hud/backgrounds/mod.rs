mod grid_lines;
mod moving_lines;
mod puffs;

use std::time::Duration;

use crate::animation::{Animation, AnimationController, AnimatorState, AnimatorTiming, Easing};

pub use grid_lines::GridLinesBackground;
pub use moving_lines::{LineDirection, MovingLinesBackground};
pub use puffs::PuffsBackground;

/// Animator helper for HUD backgrounds driven by AnimatorState.
pub struct BackgroundAnimator {
    controller: AnimationController,
    timing: AnimatorTiming,
    easing: Easing,
    state: AnimatorState,
    animation: Option<Animation<f32>>,
    progress: f32,
    last_delta: Duration,
}

impl BackgroundAnimator {
    pub fn new() -> Self {
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

    #[allow(dead_code)]
    pub fn timing(mut self, timing: AnimatorTiming) -> Self {
        self.timing = timing;
        self
    }

    pub fn set_timing(&mut self, timing: AnimatorTiming) {
        self.timing = timing;
    }

    #[allow(dead_code)]
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn set_easing(&mut self, easing: Easing) {
        self.easing = easing;
    }

    pub fn progress(&self) -> f32 {
        self.progress
    }

    pub fn state(&self) -> AnimatorState {
        self.state
    }

    pub fn last_delta(&self) -> Duration {
        self.last_delta
    }

    pub fn update(&mut self, state: AnimatorState) -> f32 {
        let delta = self.controller.delta();
        self.update_with_delta(state, delta)
    }

    pub fn update_with_delta(&mut self, state: AnimatorState, delta: Duration) -> f32 {
        if state != self.state() {
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
    fn test_background_animator_progress() {
        let timing = AnimatorTiming::new(
            Duration::from_millis(10),
            Duration::from_millis(10),
            Duration::ZERO,
        );
        let mut animator = BackgroundAnimator::new()
            .timing(timing)
            .easing(Easing::Linear);
        assert_eq!(animator.state(), AnimatorState::Exited);

        animator.update_with_delta(AnimatorState::Entering, Duration::from_millis(5));
        assert!(animator.progress() > 0.0);

        animator.update_with_delta(AnimatorState::Entered, Duration::from_millis(1));
        assert_eq!(animator.progress(), 1.0);

        animator.update_with_delta(AnimatorState::Exiting, Duration::from_millis(5));
        assert!(animator.progress() < 1.0);
    }
}
