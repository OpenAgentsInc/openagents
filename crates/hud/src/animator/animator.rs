//! Core animation state machine.

use crate::easing::{ease_out_cubic, EasingFn};
use crate::theme::timing;

use super::state::AnimatorState;

/// Core animation controller - Arwes-style state machine.
///
/// The HudAnimator manages transitions between animation states and provides
/// eased progress values for rendering animated components.
///
/// # Example
///
/// ```ignore
/// let mut animator = HudAnimator::new()
///     .enter_duration(20)
///     .easing(ease_out_expo);
///
/// animator.enter();
///
/// // In update loop:
/// if animator.tick() {
///     // Still animating
/// }
///
/// let opacity = animator.progress(); // 0.0 to 1.0
/// ```
#[derive(Clone)]
pub struct HudAnimator {
    state: AnimatorState,
    /// Eased progress (0.0 to 1.0).
    progress: f32,
    /// Current frame count in animation.
    frame_count: u32,
    /// Duration for enter animation in frames.
    enter_duration: u32,
    /// Duration for exit animation in frames.
    exit_duration: u32,
    /// Easing function to apply.
    easing: EasingFn,
}

impl Default for HudAnimator {
    fn default() -> Self {
        Self::new()
    }
}

impl HudAnimator {
    /// Create a new animator with default settings.
    pub fn new() -> Self {
        Self {
            state: AnimatorState::Exited,
            progress: 0.0,
            frame_count: 0,
            enter_duration: timing::ENTER_FRAMES,
            exit_duration: timing::EXIT_FRAMES,
            easing: ease_out_cubic,
        }
    }

    /// Set the enter animation duration in frames.
    pub fn enter_duration(mut self, frames: u32) -> Self {
        self.enter_duration = frames.max(1);
        self
    }

    /// Set the exit animation duration in frames.
    pub fn exit_duration(mut self, frames: u32) -> Self {
        self.exit_duration = frames.max(1);
        self
    }

    /// Set the easing function.
    pub fn easing(mut self, f: EasingFn) -> Self {
        self.easing = f;
        self
    }

    /// Trigger enter animation.
    pub fn enter(&mut self) {
        if self.state == AnimatorState::Exited || self.state == AnimatorState::Exiting {
            self.state = AnimatorState::Entering;
            self.frame_count = 0;
        }
    }

    /// Trigger exit animation.
    pub fn exit(&mut self) {
        if self.state == AnimatorState::Entered || self.state == AnimatorState::Entering {
            self.state = AnimatorState::Exiting;
            self.frame_count = 0;
        }
    }

    /// Toggle between enter/exit.
    pub fn toggle(&mut self) {
        match self.state {
            AnimatorState::Exited | AnimatorState::Exiting => self.enter(),
            AnimatorState::Entered | AnimatorState::Entering => self.exit(),
        }
    }

    /// Immediately set to entered state (skip animation).
    pub fn set_entered(&mut self) {
        self.state = AnimatorState::Entered;
        self.progress = 1.0;
        self.frame_count = 0;
    }

    /// Immediately set to exited state (skip animation).
    pub fn set_exited(&mut self) {
        self.state = AnimatorState::Exited;
        self.progress = 0.0;
        self.frame_count = 0;
    }

    /// Advance animation by one frame.
    ///
    /// Returns `true` if still animating, `false` if animation is complete.
    pub fn tick(&mut self) -> bool {
        match self.state {
            AnimatorState::Entering => {
                self.frame_count += 1;
                let raw = (self.frame_count as f32 / self.enter_duration as f32).min(1.0);
                self.progress = (self.easing)(raw);

                if self.frame_count >= self.enter_duration {
                    self.state = AnimatorState::Entered;
                    self.progress = 1.0;
                    self.frame_count = 0;
                    return false;
                }
                true
            }
            AnimatorState::Exiting => {
                self.frame_count += 1;
                let raw = (self.frame_count as f32 / self.exit_duration as f32).min(1.0);
                // For exiting, we go from 1.0 to 0.0
                self.progress = 1.0 - (self.easing)(raw);

                if self.frame_count >= self.exit_duration {
                    self.state = AnimatorState::Exited;
                    self.progress = 0.0;
                    self.frame_count = 0;
                    return false;
                }
                true
            }
            _ => false,
        }
    }

    /// Get current state.
    #[inline]
    pub fn state(&self) -> AnimatorState {
        self.state
    }

    /// Get eased progress (0.0 to 1.0).
    #[inline]
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Get raw (linear) progress without easing.
    pub fn raw_progress(&self) -> f32 {
        match self.state {
            AnimatorState::Exited => 0.0,
            AnimatorState::Entering => {
                (self.frame_count as f32 / self.enter_duration as f32).min(1.0)
            }
            AnimatorState::Entered => 1.0,
            AnimatorState::Exiting => {
                1.0 - (self.frame_count as f32 / self.exit_duration as f32).min(1.0)
            }
        }
    }

    /// Check if currently animating.
    #[inline]
    pub fn is_animating(&self) -> bool {
        self.state.is_animating()
    }

    /// Check if visible (not fully exited).
    #[inline]
    pub fn is_visible(&self) -> bool {
        self.state.is_visible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let animator = HudAnimator::new();
        assert_eq!(animator.state(), AnimatorState::Exited);
        assert_eq!(animator.progress(), 0.0);
        assert!(!animator.is_visible());
    }

    #[test]
    fn test_enter_animation() {
        let mut animator = HudAnimator::new().enter_duration(10);
        animator.enter();

        assert_eq!(animator.state(), AnimatorState::Entering);
        assert!(animator.is_animating());

        // Tick through animation
        for _ in 0..10 {
            animator.tick();
        }

        assert_eq!(animator.state(), AnimatorState::Entered);
        assert_eq!(animator.progress(), 1.0);
        assert!(!animator.is_animating());
    }

    #[test]
    fn test_exit_animation() {
        let mut animator = HudAnimator::new().exit_duration(5);
        animator.set_entered();
        animator.exit();

        assert_eq!(animator.state(), AnimatorState::Exiting);

        for _ in 0..5 {
            animator.tick();
        }

        assert_eq!(animator.state(), AnimatorState::Exited);
        assert_eq!(animator.progress(), 0.0);
    }

    #[test]
    fn test_toggle() {
        let mut animator = HudAnimator::new();

        animator.toggle();
        assert_eq!(animator.state(), AnimatorState::Entering);

        animator.set_entered();
        animator.toggle();
        assert_eq!(animator.state(), AnimatorState::Exiting);
    }
}
