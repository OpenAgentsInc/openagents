//! The One Euro filter (Casiez, Roussel, and Vogel, CHI 2012): a low-pass
//! filter whose cutoff rises with speed, so a still hand stops shaking and
//! a moving one doesn't lag. Reimplemented here from the paper.

use std::f32::consts::TAU;

/// The cutoff, in hertz, when the value isn't moving.
pub const MIN_CUTOFF: f32 = 1.7;
/// How fast the cutoff rises with speed, per unit of the value a second.
/// Landmarks move in fractions of the frame, so a hand crossing the frame
/// in a second lifts the cutoff by this many hertz.
pub const BETA: f32 = 12.0;
/// The cutoff for the speed estimate itself.
pub const D_CUTOFF: f32 = 1.0;

/// One filtered value.
#[derive(Clone, Copy, Debug)]
pub struct OneEuro {
    pub min_cutoff: f32,
    pub beta: f32,
    pub d_cutoff: f32,
    last: Option<(f32, f32, f64)>,
}

impl Default for OneEuro {
    fn default() -> Self {
        Self::new(MIN_CUTOFF, BETA, D_CUTOFF)
    }
}

/// The smoothing factor for a cutoff over a step of `dt` seconds.
fn alpha(cutoff: f32, dt: f32) -> f32 {
    let tau = 1.0 / (TAU * cutoff);
    1.0 / (1.0 + tau / dt)
}

impl OneEuro {
    pub fn new(min_cutoff: f32, beta: f32, d_cutoff: f32) -> Self {
        Self {
            min_cutoff,
            beta,
            d_cutoff,
            last: None,
        }
    }

    /// The filtered value of `x` read at `t` seconds.
    pub fn filter(&mut self, x: f32, t: f64) -> f32 {
        let Some((prev, prev_dx, prev_t)) = self.last else {
            self.last = Some((x, 0.0, t));
            return x;
        };
        let dt = ((t - prev_t) as f32).max(1e-3);
        let dx = (x - prev) / dt;
        let edx = prev_dx + alpha(self.d_cutoff, dt) * (dx - prev_dx);
        let cutoff = self.min_cutoff + self.beta * edx.abs();
        let out = prev + alpha(cutoff, dt) * (x - prev);
        self.last = Some((out, edx, t));
        out
    }

    pub fn reset(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic jitter in -1..1.
    fn noise(i: u32) -> f32 {
        let mut x = i.wrapping_mul(2_654_435_761) ^ 0x9e37_79b9;
        x ^= x >> 13;
        x = x.wrapping_mul(0x5bd1_e995);
        x ^= x >> 15;
        (x % 2001) as f32 / 1000.0 - 1.0
    }

    #[test]
    fn a_still_value_stops_shaking() {
        let mut f = OneEuro::default();
        let mut worst_raw: f32 = 0.0;
        let mut worst: f32 = 0.0;
        for i in 0..90 {
            let raw = 0.5 + 0.004 * noise(i);
            let out = f.filter(raw, i as f64 / 30.0);
            if i > 30 {
                worst_raw = worst_raw.max((raw - 0.5).abs());
                worst = worst.max((out - 0.5).abs());
            }
        }
        assert!(worst < worst_raw * 0.6, "{worst} vs {worst_raw}");
    }

    #[test]
    fn a_moving_value_keeps_up() {
        let mut f = OneEuro::default();
        let mut lag: f32 = 0.0;
        for i in 0..60 {
            let t = i as f64 / 30.0;
            // Across the whole frame in two seconds.
            let raw = 0.1 + 0.4 * t as f32;
            let out = f.filter(raw, t);
            if i > 10 {
                lag = lag.max(raw - out);
            }
        }
        // Under three hundredths of the frame behind.
        assert!(lag < 0.03, "{lag}");
        // A slow cutoff alone would trail far further.
        let mut slow = OneEuro::new(MIN_CUTOFF, 0.0, D_CUTOFF);
        let mut slow_lag: f32 = 0.0;
        for i in 0..60 {
            let t = i as f64 / 30.0;
            let raw = 0.1 + 0.4 * t as f32;
            slow_lag = slow_lag.max(raw - slow.filter(raw, t));
        }
        assert!(slow_lag > lag * 1.5, "{slow_lag} vs {lag}");
    }

    #[test]
    fn the_first_reading_passes_through() {
        let mut f = OneEuro::default();
        assert_eq!(f.filter(0.3, 1.0), 0.3);
        f.reset();
        assert_eq!(f.filter(0.7, 2.0), 0.7);
    }
}
