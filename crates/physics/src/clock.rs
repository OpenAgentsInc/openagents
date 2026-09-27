//! A fixed-step clock: frame time accumulates and the world advances in
//! whole steps of `dt`, so results do not depend on the frame rate.

use serde::{Deserialize, Serialize};

/// Accumulates frame time into fixed steps.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixedStep {
    /// Step length, s.
    pub dt: f64,
    /// Most steps one frame may run; time beyond them is dropped and counted.
    pub max_steps: u32,
    /// Frame time not yet consumed by a step, s.
    pub accumulator: f64,
    /// Total frame time dropped because a frame exceeded `max_steps`, s.
    pub dropped: f64,
}

impl FixedStep {
    #[must_use]
    pub fn new(dt: f64, max_steps: u32) -> Self {
        Self {
            dt,
            max_steps,
            accumulator: 0.0,
            dropped: 0.0,
        }
    }

    /// Add `frame` seconds and return how many steps to run now. Non-finite
    /// or negative frame time is ignored.
    pub fn advance(&mut self, frame: f64) -> u32 {
        if !frame.is_finite() || frame <= 0.0 {
            return 0;
        }
        self.accumulator += frame;
        // A frame of exactly k steps must yield k even after rounding.
        let tolerance = self.dt * 1e-9;
        let mut steps = 0;
        while self.accumulator + tolerance >= self.dt {
            if steps == self.max_steps {
                self.dropped += self.accumulator;
                self.accumulator = 0.0;
                break;
            }
            self.accumulator = (self.accumulator - self.dt).max(0.0);
            steps += 1;
        }
        steps
    }

    /// Fraction of a step accumulated but not yet run, for interpolation.
    #[must_use]
    pub fn alpha(&self) -> f64 {
        (self.accumulator / self.dt).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_become_whole_steps() {
        let mut clock = FixedStep::new(1.0 / 120.0, 12);
        assert_eq!(clock.advance(1.0 / 60.0), 2);
        assert_eq!(clock.advance(1.0 / 30.0), 4);
        let mut total = 0;
        for _ in 0..144 {
            total += clock.advance(1.0 / 144.0);
        }
        assert_eq!(total, 120, "one second of 144 fps frames is 120 steps");
        assert_eq!(clock.dropped, 0.0);
    }

    #[test]
    fn a_long_frame_runs_the_cap_and_reports_the_rest() {
        let mut clock = FixedStep::new(0.01, 10);
        assert_eq!(clock.advance(0.25), 10);
        assert!((clock.dropped - 0.15).abs() < 1e-12);
        assert_eq!(clock.accumulator, 0.0);
        assert_eq!(clock.advance(f64::NAN), 0);
        assert_eq!(clock.advance(-1.0), 0);
    }
}
