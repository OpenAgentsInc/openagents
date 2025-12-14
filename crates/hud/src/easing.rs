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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_bounds() {
        let easings: &[EasingFn] = &[
            linear,
            ease_out_cubic,
            ease_in_cubic,
            ease_in_out_cubic,
            ease_out_expo,
            ease_in_expo,
            ease_out_quad,
            ease_in_quad,
        ];

        for easing in easings {
            // t=0 should give ~0
            assert!((easing(0.0) - 0.0).abs() < 0.001);
            // t=1 should give ~1
            assert!((easing(1.0) - 1.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_ease_out_cubic_shape() {
        // ease-out should be > linear in the middle
        assert!(ease_out_cubic(0.5) > 0.5);
    }

    #[test]
    fn test_ease_in_cubic_shape() {
        // ease-in should be < linear in the middle
        assert!(ease_in_cubic(0.5) < 0.5);
    }
}
