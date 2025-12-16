//! Easing functions for animations.
//!
//! These functions take a normalized time value `t` (0.0 to 1.0)
//! and return an eased value (also 0.0 to 1.0).

/// Easing function type.
pub type EasingFn = fn(f32) -> f32;

/// Linear interpolation (no easing).
#[inline]
pub fn linear(t: f32) -> f32 {
    t
}

/// Ease-out cubic - fast start, slow end.
/// This is the Arwes default easing.
#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Ease-in cubic - slow start, fast end.
#[inline]
pub fn ease_in_cubic(t: f32) -> f32 {
    t.powi(3)
}

/// Ease-in-out cubic - slow start and end.
#[inline]
pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t.powi(3)
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Ease-out exponential - dramatic deceleration.
#[inline]
pub fn ease_out_expo(t: f32) -> f32 {
    if t >= 1.0 {
        1.0
    } else {
        1.0 - 2.0_f32.powf(-10.0 * t)
    }
}

/// Ease-in exponential - dramatic acceleration.
#[inline]
pub fn ease_in_expo(t: f32) -> f32 {
    if t <= 0.0 {
        0.0
    } else {
        2.0_f32.powf(10.0 * t - 10.0)
    }
}

/// Ease-out quad - gentler than cubic.
#[inline]
pub fn ease_out_quad(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(2)
}

/// Ease-in quad - gentler than cubic.
#[inline]
pub fn ease_in_quad(t: f32) -> f32 {
    t.powi(2)
}

/// Ease-in-out quad.
#[inline]
pub fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t.powi(2)
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

/// Ease-in-out exponential.
#[inline]
pub fn ease_in_out_expo(t: f32) -> f32 {
    if t <= 0.0 {
        0.0
    } else if t >= 1.0 {
        1.0
    } else if t < 0.5 {
        2.0_f32.powf(20.0 * t - 10.0) / 2.0
    } else {
        (2.0 - 2.0_f32.powf(-20.0 * t + 10.0)) / 2.0
    }
}

// === Quartic ===

/// Ease-in quartic (power of 4).
#[inline]
pub fn ease_in_quart(t: f32) -> f32 {
    t.powi(4)
}

/// Ease-out quartic.
#[inline]
pub fn ease_out_quart(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(4)
}

/// Ease-in-out quartic.
#[inline]
pub fn ease_in_out_quart(t: f32) -> f32 {
    if t < 0.5 {
        8.0 * t.powi(4)
    } else {
        1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
    }
}

// === Quintic ===

/// Ease-in quintic (power of 5).
#[inline]
pub fn ease_in_quint(t: f32) -> f32 {
    t.powi(5)
}

/// Ease-out quintic.
#[inline]
pub fn ease_out_quint(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(5)
}

/// Ease-in-out quintic.
#[inline]
pub fn ease_in_out_quint(t: f32) -> f32 {
    if t < 0.5 {
        16.0 * t.powi(5)
    } else {
        1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
    }
}

// === Sinusoidal ===

/// Ease-in sine.
#[inline]
pub fn ease_in_sine(t: f32) -> f32 {
    1.0 - (t * std::f32::consts::FRAC_PI_2).cos()
}

/// Ease-out sine.
#[inline]
pub fn ease_out_sine(t: f32) -> f32 {
    (t * std::f32::consts::FRAC_PI_2).sin()
}

/// Ease-in-out sine.
#[inline]
pub fn ease_in_out_sine(t: f32) -> f32 {
    -((t * std::f32::consts::PI).cos() - 1.0) / 2.0
}

// === Circular ===

/// Ease-in circular.
#[inline]
pub fn ease_in_circ(t: f32) -> f32 {
    1.0 - (1.0 - t.powi(2)).sqrt()
}

/// Ease-out circular.
#[inline]
pub fn ease_out_circ(t: f32) -> f32 {
    (1.0 - (t - 1.0).powi(2)).sqrt()
}

/// Ease-in-out circular.
#[inline]
pub fn ease_in_out_circ(t: f32) -> f32 {
    if t < 0.5 {
        (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
    } else {
        ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
    }
}

// === Back (overshoot) ===

const BACK_C1: f32 = 1.70158;
const BACK_C2: f32 = BACK_C1 * 1.525;
const BACK_C3: f32 = BACK_C1 + 1.0;

/// Ease-in back - slight overshoot at start.
#[inline]
pub fn ease_in_back(t: f32) -> f32 {
    BACK_C3 * t.powi(3) - BACK_C1 * t.powi(2)
}

/// Ease-out back - slight overshoot at end.
#[inline]
pub fn ease_out_back(t: f32) -> f32 {
    1.0 + BACK_C3 * (t - 1.0).powi(3) + BACK_C1 * (t - 1.0).powi(2)
}

/// Ease-in-out back - overshoot at both ends.
#[inline]
pub fn ease_in_out_back(t: f32) -> f32 {
    if t < 0.5 {
        ((2.0 * t).powi(2) * ((BACK_C2 + 1.0) * 2.0 * t - BACK_C2)) / 2.0
    } else {
        ((2.0 * t - 2.0).powi(2) * ((BACK_C2 + 1.0) * (t * 2.0 - 2.0) + BACK_C2) + 2.0) / 2.0
    }
}

// === Elastic ===

const ELASTIC_C4: f32 = (2.0 * std::f32::consts::PI) / 3.0;
const ELASTIC_C5: f32 = (2.0 * std::f32::consts::PI) / 4.5;

/// Ease-in elastic - rubber band effect at start.
#[inline]
pub fn ease_in_elastic(t: f32) -> f32 {
    if t <= 0.0 {
        0.0
    } else if t >= 1.0 {
        1.0
    } else {
        -2.0_f32.powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * ELASTIC_C4).sin()
    }
}

/// Ease-out elastic - rubber band effect at end.
#[inline]
pub fn ease_out_elastic(t: f32) -> f32 {
    if t <= 0.0 {
        0.0
    } else if t >= 1.0 {
        1.0
    } else {
        2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * ELASTIC_C4).sin() + 1.0
    }
}

/// Ease-in-out elastic.
#[inline]
pub fn ease_in_out_elastic(t: f32) -> f32 {
    if t <= 0.0 {
        0.0
    } else if t >= 1.0 {
        1.0
    } else if t < 0.5 {
        -(2.0_f32.powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * ELASTIC_C5).sin()) / 2.0
    } else {
        (2.0_f32.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * ELASTIC_C5).sin()) / 2.0 + 1.0
    }
}

// === Bounce ===

/// Ease-out bounce - bouncing ball effect.
#[inline]
pub fn ease_out_bounce(t: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;

    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t = t - 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        let t = t - 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / D1;
        N1 * t * t + 0.984375
    }
}

/// Ease-in bounce.
#[inline]
pub fn ease_in_bounce(t: f32) -> f32 {
    1.0 - ease_out_bounce(1.0 - t)
}

/// Ease-in-out bounce.
#[inline]
pub fn ease_in_out_bounce(t: f32) -> f32 {
    if t < 0.5 {
        (1.0 - ease_out_bounce(1.0 - 2.0 * t)) / 2.0
    } else {
        (1.0 + ease_out_bounce(2.0 * t - 1.0)) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_bounds() {
        let easings: &[EasingFn] = &[
            linear,
            // Quad
            ease_in_quad,
            ease_out_quad,
            ease_in_out_quad,
            // Cubic
            ease_in_cubic,
            ease_out_cubic,
            ease_in_out_cubic,
            // Quart
            ease_in_quart,
            ease_out_quart,
            ease_in_out_quart,
            // Quint
            ease_in_quint,
            ease_out_quint,
            ease_in_out_quint,
            // Expo
            ease_in_expo,
            ease_out_expo,
            ease_in_out_expo,
            // Sine
            ease_in_sine,
            ease_out_sine,
            ease_in_out_sine,
            // Circ
            ease_in_circ,
            ease_out_circ,
            ease_in_out_circ,
            // Back
            ease_in_back,
            ease_out_back,
            ease_in_out_back,
            // Elastic
            ease_in_elastic,
            ease_out_elastic,
            ease_in_out_elastic,
            // Bounce
            ease_in_bounce,
            ease_out_bounce,
            ease_in_out_bounce,
        ];

        for easing in easings {
            // t=0 should give ~0
            assert!((easing(0.0) - 0.0).abs() < 0.01, "easing(0) failed");
            // t=1 should give ~1
            assert!((easing(1.0) - 1.0).abs() < 0.01, "easing(1) failed");
        }
    }

    #[test]
    fn test_ease_out_faster_than_linear() {
        // ease-out should be > linear in the middle (faster start)
        assert!(ease_out_cubic(0.5) > 0.5);
        assert!(ease_out_quad(0.5) > 0.5);
        assert!(ease_out_quart(0.5) > 0.5);
        assert!(ease_out_quint(0.5) > 0.5);
    }

    #[test]
    fn test_ease_in_slower_than_linear() {
        // ease-in should be < linear in the middle (slower start)
        assert!(ease_in_cubic(0.5) < 0.5);
        assert!(ease_in_quad(0.5) < 0.5);
        assert!(ease_in_quart(0.5) < 0.5);
        assert!(ease_in_quint(0.5) < 0.5);
    }

    #[test]
    fn test_back_overshoots() {
        // Back easing should overshoot (go negative or >1)
        assert!(
            ease_in_back(0.2) < 0.0,
            "ease_in_back should overshoot negative"
        );
        assert!(
            ease_out_back(0.8) > 1.0,
            "ease_out_back should overshoot >1"
        );
    }
}
