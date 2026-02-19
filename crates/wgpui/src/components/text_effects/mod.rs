mod decipher;
mod sequence;

use std::time::Duration;

use crate::animation::{AnimationController, AnimatorState, Easing};

pub use decipher::TextDecipher;
pub use sequence::TextSequence;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextEffectTiming {
    pub duration: Duration,
    pub char_delay: Duration,
}

impl TextEffectTiming {
    pub const fn new(duration: Duration, char_delay: Duration) -> Self {
        Self {
            duration,
            char_delay,
        }
    }

    pub fn total_duration(self, length: usize) -> Duration {
        let extra = self
            .char_delay
            .checked_mul(length.saturating_sub(1) as u32)
            .unwrap_or(Duration::MAX);
        self.duration.saturating_add(extra)
    }
}

impl Default for TextEffectTiming {
    fn default() -> Self {
        Self {
            duration: Duration::from_millis(400),
            char_delay: Duration::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TextDurationOptions {
    pub max_duration: Duration,
    pub characters_per_second: f32,
}

impl Default for TextDurationOptions {
    fn default() -> Self {
        Self {
            max_duration: Duration::from_secs(4),
            characters_per_second: 100.0,
        }
    }
}

/// Compute a text animation duration based on text length.
pub fn animation_text_duration(length: usize, options: TextDurationOptions) -> Duration {
    if length == 0 {
        return Duration::ZERO;
    }

    let cps = options.characters_per_second.max(1.0);
    let seconds = length as f32 / cps;
    let real = Duration::from_secs_f32(seconds);
    if real > options.max_duration {
        options.max_duration
    } else {
        real
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TextEffectFrame {
    pub state: AnimatorState,
    pub length: usize,
    pub timing: TextEffectTiming,
    pub total_duration: Duration,
    pub eased_time: Duration,
    progress: f32,
}

impl TextEffectFrame {
    pub fn progress(&self) -> f32 {
        self.progress
    }

    pub fn is_visible(&self, index: usize) -> bool {
        if self.length == 0 || index >= self.length {
            return false;
        }

        match self.state {
            AnimatorState::Entered => true,
            AnimatorState::Exited => false,
            AnimatorState::Entering => {
                if self.progress >= 1.0 {
                    return true;
                }
                let threshold = reveal_threshold(index, self.length, self.timing);
                self.eased_time > threshold
            }
            AnimatorState::Exiting => {
                if self.progress <= 0.0 {
                    return false;
                }
                let threshold = reveal_threshold(index, self.length, self.timing);
                self.eased_time <= self.total_duration.saturating_sub(threshold)
            }
        }
    }
}

pub struct TextEffectAnimator {
    controller: AnimationController,
    timing: TextEffectTiming,
    easing: Easing,
    state: AnimatorState,
    elapsed: Duration,
    progress: f32,
    last_delta: Duration,
}

impl Default for TextEffectAnimator {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEffectAnimator {
    pub fn new() -> Self {
        Self {
            controller: AnimationController::new(),
            timing: TextEffectTiming::default(),
            easing: Easing::Linear,
            state: AnimatorState::Exited,
            elapsed: Duration::ZERO,
            progress: 0.0,
            last_delta: Duration::ZERO,
        }
    }

    pub fn timing(mut self, timing: TextEffectTiming) -> Self {
        self.timing = timing;
        self
    }

    pub fn set_timing(&mut self, timing: TextEffectTiming) {
        self.timing = timing;
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn set_easing(&mut self, easing: Easing) {
        self.easing = easing;
    }

    pub fn state(&self) -> AnimatorState {
        self.state
    }

    pub fn timing_value(&self) -> TextEffectTiming {
        self.timing
    }

    pub fn easing_value(&self) -> Easing {
        self.easing
    }

    pub fn progress(&self) -> f32 {
        self.progress
    }

    pub fn last_delta(&self) -> Duration {
        self.last_delta
    }

    pub fn update(&mut self, state: AnimatorState, length: usize) -> TextEffectFrame {
        if state != self.state {
            self.controller.reset();
        }
        let delta = self.controller.delta();
        self.update_with_delta(state, length, delta)
    }

    pub fn update_with_delta(
        &mut self,
        state: AnimatorState,
        length: usize,
        delta: Duration,
    ) -> TextEffectFrame {
        if state != self.state {
            self.state = state;
            self.elapsed = Duration::ZERO;
        }

        let total_duration = ensure_non_zero(self.timing.total_duration(length));

        match state {
            AnimatorState::Entering | AnimatorState::Exiting => {
                self.elapsed = (self.elapsed + delta).min(total_duration);
            }
            AnimatorState::Entered => {
                self.elapsed = total_duration;
            }
            AnimatorState::Exited => {
                self.elapsed = Duration::ZERO;
            }
        }

        let total_secs = total_duration.as_secs_f32();
        let raw = if total_secs > 0.0 {
            (self.elapsed.as_secs_f32() / total_secs).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let eased = self.easing.apply(raw);

        let (progress, eased_time) = match state {
            AnimatorState::Entering => (eased, Duration::from_secs_f32(eased * total_secs)),
            AnimatorState::Exiting => (1.0 - eased, Duration::from_secs_f32(eased * total_secs)),
            AnimatorState::Entered => (1.0, total_duration),
            AnimatorState::Exited => (0.0, Duration::ZERO),
        };

        self.progress = progress.clamp(0.0, 1.0);
        self.last_delta = delta;

        TextEffectFrame {
            state,
            length,
            timing: self.timing,
            total_duration,
            eased_time,
            progress: self.progress,
        }
    }
}

pub(crate) struct CursorBlink {
    period: Duration,
    elapsed: Duration,
    visible: bool,
}

impl CursorBlink {
    pub fn new() -> Self {
        Self {
            period: Duration::from_millis(500),
            elapsed: Duration::ZERO,
            visible: true,
        }
    }

    pub fn set_period(&mut self, period: Duration) {
        self.period = ensure_non_zero(period);
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn update(&mut self, delta: Duration, state: AnimatorState, enabled: bool) {
        if !enabled {
            self.visible = false;
            self.elapsed = Duration::ZERO;
            return;
        }

        match state {
            AnimatorState::Entered => {
                self.elapsed += delta;
                while self.elapsed >= self.period {
                    self.elapsed -= self.period;
                    self.visible = !self.visible;
                }
            }
            AnimatorState::Entering | AnimatorState::Exiting => {
                self.visible = true;
                self.elapsed = Duration::ZERO;
            }
            AnimatorState::Exited => {
                self.visible = false;
                self.elapsed = Duration::ZERO;
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

fn reveal_threshold(index: usize, length: usize, timing: TextEffectTiming) -> Duration {
    if length <= 1 {
        return Duration::ZERO;
    }

    let length_minus_one = (length - 1) as f32;
    let base = timing.duration.as_secs_f32() * (index as f32 / length_minus_one);
    let delay = timing.char_delay.as_secs_f32() * index as f32;
    Duration::from_secs_f32(base + delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_effect_timing_total_duration() {
        let timing = TextEffectTiming::new(Duration::from_secs(2), Duration::from_millis(500));
        let total = timing.total_duration(4);
        assert_eq!(total, Duration::from_secs(3) + Duration::from_millis(500));
    }

    #[test]
    fn test_cursor_blink_toggles() {
        let mut blink = CursorBlink::new();
        blink.set_period(Duration::from_millis(100));

        blink.update(Duration::from_millis(50), AnimatorState::Entered, true);
        assert!(blink.visible());

        blink.update(Duration::from_millis(60), AnimatorState::Entered, true);
        assert!(!blink.visible());
    }

    #[test]
    fn test_animation_text_duration() {
        let duration = animation_text_duration(
            200,
            TextDurationOptions {
                max_duration: Duration::from_secs(4),
                characters_per_second: 100.0,
            },
        );
        assert_eq!(duration, Duration::from_secs(2));

        let duration = animation_text_duration(
            1000,
            TextDurationOptions {
                max_duration: Duration::from_secs(1),
                characters_per_second: 50.0,
            },
        );
        assert_eq!(duration, Duration::from_secs(1));
    }
}
