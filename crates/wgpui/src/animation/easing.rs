use std::f32::consts::PI;

/// Animation easing function.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Easing {
    /// Linear interpolation (constant speed).
    Linear,
    /// Slow start, fast end.
    EaseIn,
    /// Fast start, slow end.
    EaseOut,
    /// Slow start, fast middle, slow end.
    #[default]
    EaseInOut,
    /// Quadratic ease-in.
    EaseInQuad,
    /// Quadratic ease-out.
    EaseOutQuad,
    /// Quadratic ease-in-out.
    EaseInOutQuad,
    /// Cubic ease-in.
    EaseInCubic,
    /// Cubic ease-out.
    EaseOutCubic,
    /// Cubic ease-in-out.
    EaseInOutCubic,
    /// Quartic ease-in.
    EaseInQuart,
    /// Quartic ease-out.
    EaseOutQuart,
    /// Quartic ease-in-out.
    EaseInOutQuart,
    /// Quintic ease-in.
    EaseInQuint,
    /// Quintic ease-out.
    EaseOutQuint,
    /// Quintic ease-in-out.
    EaseInOutQuint,
    /// Sine ease-in.
    EaseInSine,
    /// Sine ease-out.
    EaseOutSine,
    /// Sine ease-in-out.
    EaseInOutSine,
    /// Exponential ease-in.
    EaseInExpo,
    /// Exponential ease-out.
    EaseOutExpo,
    /// Exponential ease-in-out.
    EaseInOutExpo,
    /// Circular ease-in.
    EaseInCirc,
    /// Circular ease-out.
    EaseOutCirc,
    /// Circular ease-in-out.
    EaseInOutCirc,
    /// Bounce ease-in.
    EaseInBounce,
    /// Bounce ease-out.
    EaseOutBounce,
    /// Bounce ease-in-out.
    EaseInOutBounce,
    /// Elastic bounce at start.
    EaseInElastic,
    /// Elastic bounce at end.
    EaseOutElastic,
    /// Elastic bounce at start and end.
    EaseInOutElastic,
    /// Overshoot and settle at start.
    EaseInBack,
    /// Overshoot and settle.
    EaseOutBack,
    /// Overshoot and settle at start and end.
    EaseInOutBack,
    /// Custom cubic bezier curve.
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    /// Apply easing function to normalized time (0.0 to 1.0).
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::EaseInQuad => t * t,
            Easing::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::EaseInCubic => t * t * t,
            Easing::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::EaseInQuart => t.powi(4),
            Easing::EaseOutQuart => 1.0 - (1.0 - t).powi(4),
            Easing::EaseInOutQuart => {
                if t < 0.5 {
                    8.0 * t.powi(4)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
                }
            }
            Easing::EaseInQuint => t.powi(5),
            Easing::EaseOutQuint => 1.0 - (1.0 - t).powi(5),
            Easing::EaseInOutQuint => {
                if t < 0.5 {
                    16.0 * t.powi(5)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
                }
            }
            Easing::EaseInSine => 1.0 - (t * PI / 2.0).cos(),
            Easing::EaseOutSine => (t * PI / 2.0).sin(),
            Easing::EaseInOutSine => -((PI * t).cos() - 1.0) / 2.0,
            Easing::EaseInExpo => {
                if t == 0.0 {
                    0.0
                } else {
                    2.0_f32.powf(10.0 * t - 10.0)
                }
            }
            Easing::EaseOutExpo => {
                if t == 1.0 {
                    1.0
                } else {
                    1.0 - 2.0_f32.powf(-10.0 * t)
                }
            }
            Easing::EaseInOutExpo => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else if t < 0.5 {
                    2.0_f32.powf(20.0 * t - 10.0) / 2.0
                } else {
                    (2.0 - 2.0_f32.powf(-20.0 * t + 10.0)) / 2.0
                }
            }
            Easing::EaseInCirc => 1.0 - (1.0 - t * t).sqrt(),
            Easing::EaseOutCirc => (1.0 - (t - 1.0).powi(2)).sqrt(),
            Easing::EaseInOutCirc => {
                if t < 0.5 {
                    (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
                } else {
                    ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
                }
            }
            Easing::EaseInBounce => 1.0 - ease_out_bounce(1.0 - t),
            Easing::EaseOutBounce => ease_out_bounce(t),
            Easing::EaseInOutBounce => {
                if t < 0.5 {
                    (1.0 - ease_out_bounce(1.0 - 2.0 * t)) / 2.0
                } else {
                    (1.0 + ease_out_bounce(2.0 * t - 1.0)) / 2.0
                }
            }
            Easing::EaseInElastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = (2.0 * PI) / 3.0;
                    -2.0_f32.powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * c4).sin()
                }
            }
            Easing::EaseOutElastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = (2.0 * PI) / 3.0;
                    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }
            Easing::EaseInOutElastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c5 = (2.0 * PI) / 4.5;
                    if t < 0.5 {
                        -(2.0_f32.powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0
                    } else {
                        (2.0_f32.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0
                            + 1.0
                    }
                }
            }
            Easing::EaseInBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                c3 * t * t * t - c1 * t * t
            }
            Easing::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Easing::EaseInOutBack => {
                let c1 = 1.70158;
                let c2 = c1 * 1.525;
                if t < 0.5 {
                    (2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2) / 2.0
                } else {
                    ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (2.0 * t - 2.0) + c2) + 2.0) / 2.0
                }
            }
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier_sample(t, *x1, *y1, *x2, *y2),
        }
    }
}

/// Sample cubic bezier curve at time t.
fn cubic_bezier_sample(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    // Newton-Raphson iteration to find parameter for x.
    let mut guess = t;
    for _ in 0..8 {
        let x = cubic_bezier_value(guess, x1, x2) - t;
        if x.abs() < 0.0001 {
            break;
        }
        let dx = cubic_bezier_derivative(guess, x1, x2);
        if dx.abs() < 0.0001 {
            break;
        }
        guess -= x / dx;
    }
    cubic_bezier_value(guess.clamp(0.0, 1.0), y1, y2)
}

fn cubic_bezier_value(t: f32, p1: f32, p2: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    3.0 * mt2 * t * p1 + 3.0 * mt * t2 * p2 + t3
}

fn cubic_bezier_derivative(t: f32, p1: f32, p2: f32) -> f32 {
    let mt = 1.0 - t;
    3.0 * mt * mt * p1 + 6.0 * mt * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

fn ease_out_bounce(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;

    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

/// Direction for step easing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EaseStepsDirection {
    Start,
    End,
}

/// Step-based easing, equivalent to CSS `steps(n, start|end)`.
#[derive(Debug, Clone, Copy)]
pub struct EaseSteps {
    steps: u32,
    direction: EaseStepsDirection,
}

impl EaseSteps {
    pub fn new(steps: u32, direction: EaseStepsDirection) -> Self {
        Self {
            steps: steps.max(1),
            direction,
        }
    }

    pub fn apply(&self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        if progress == 0.0 || progress == 1.0 {
            return progress;
        }

        let progress = match self.direction {
            EaseStepsDirection::End => progress.min(0.999),
            EaseStepsDirection::Start => progress.max(0.001),
        };

        let steps = self.steps as f32;
        let expanded = progress * steps;
        let rounded = match self.direction {
            EaseStepsDirection::End => expanded.floor(),
            EaseStepsDirection::Start => expanded.ceil(),
        };

        (rounded / steps).clamp(0.0, 1.0)
    }
}

/// Piecewise linear easing across provided breakpoints.
#[derive(Debug, Clone)]
pub struct EaseAmong {
    breakpoints: Vec<f32>,
}

impl EaseAmong {
    pub fn new(breakpoints: Vec<f32>) -> Self {
        Self { breakpoints }
    }

    pub fn apply(&self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        let breakpoints = &self.breakpoints;

        if breakpoints.is_empty() {
            return progress;
        }

        if breakpoints.len() == 1 {
            return progress * breakpoints[0];
        }

        if progress <= 0.0 {
            return breakpoints[0];
        }

        if progress >= 1.0 {
            return breakpoints[breakpoints.len() - 1];
        }

        let portion = 1.0 / (breakpoints.len() as f32 - 1.0);
        let portion_base = (progress / portion).floor();
        let portion_progress = progress / portion - portion_base;

        let current = (progress * (breakpoints.len() as f32 - 1.0)).floor() as usize;
        let next = (current + 1).min(breakpoints.len() - 1);
        let from = breakpoints[current];
        let to = breakpoints[next];

        if to > from {
            from + (to - from) * portion_progress
        } else {
            from - (from - to) * portion_progress
        }
    }
}

pub fn ease_steps(steps: u32, direction: EaseStepsDirection, progress: f32) -> f32 {
    EaseSteps::new(steps, direction).apply(progress)
}

pub fn ease_among(breakpoints: &[f32], progress: f32) -> f32 {
    EaseAmong::new(breakpoints.to_vec()).apply(progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_quart_family() {
        let in_quart = Easing::EaseInQuart;
        let out_quart = Easing::EaseOutQuart;
        let in_out_quart = Easing::EaseInOutQuart;

        assert!((in_quart.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_quart.apply(1.0) - 1.0).abs() < 0.001);
        assert!((in_quart.apply(0.5) - 0.0625).abs() < 0.01);

        assert!((out_quart.apply(0.0) - 0.0).abs() < 0.001);
        assert!((out_quart.apply(1.0) - 1.0).abs() < 0.001);
        assert!((out_quart.apply(0.5) - 0.9375).abs() < 0.01);

        assert!((in_out_quart.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_out_quart.apply(1.0) - 1.0).abs() < 0.001);
        assert!((in_out_quart.apply(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_easing_quint_family() {
        let in_quint = Easing::EaseInQuint;
        let out_quint = Easing::EaseOutQuint;
        let in_out_quint = Easing::EaseInOutQuint;

        assert!((in_quint.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_quint.apply(1.0) - 1.0).abs() < 0.001);
        assert!((in_quint.apply(0.5) - 0.03125).abs() < 0.01);

        assert!((out_quint.apply(0.0) - 0.0).abs() < 0.001);
        assert!((out_quint.apply(1.0) - 1.0).abs() < 0.001);
        assert!((out_quint.apply(0.5) - 0.96875).abs() < 0.01);

        assert!((in_out_quint.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_out_quint.apply(1.0) - 1.0).abs() < 0.001);
        assert!((in_out_quint.apply(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_easing_sine_family() {
        let in_sine = Easing::EaseInSine;
        let out_sine = Easing::EaseOutSine;
        let in_out_sine = Easing::EaseInOutSine;

        assert!((in_sine.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_sine.apply(1.0) - 1.0).abs() < 0.001);
        assert!((in_sine.apply(0.5) - (1.0 - (PI / 4.0).cos())).abs() < 0.01);

        assert!((out_sine.apply(0.0) - 0.0).abs() < 0.001);
        assert!((out_sine.apply(1.0) - 1.0).abs() < 0.001);
        assert!((out_sine.apply(0.5) - (PI / 4.0).sin()).abs() < 0.01);

        assert!((in_out_sine.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_out_sine.apply(1.0) - 1.0).abs() < 0.001);
        assert!((in_out_sine.apply(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_easing_expo_family() {
        let in_expo = Easing::EaseInExpo;
        let out_expo = Easing::EaseOutExpo;
        let in_out_expo = Easing::EaseInOutExpo;

        assert!((in_expo.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_expo.apply(1.0) - 1.0).abs() < 0.001);

        assert!((out_expo.apply(0.0) - 0.0).abs() < 0.001);
        assert!((out_expo.apply(1.0) - 1.0).abs() < 0.001);

        assert!((in_out_expo.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_out_expo.apply(1.0) - 1.0).abs() < 0.001);
        assert!((in_out_expo.apply(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_easing_circ_family() {
        let in_circ = Easing::EaseInCirc;
        let out_circ = Easing::EaseOutCirc;
        let in_out_circ = Easing::EaseInOutCirc;

        assert!((in_circ.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_circ.apply(1.0) - 1.0).abs() < 0.001);
        assert!((in_circ.apply(0.5) - (1.0_f32 - (1.0_f32 - 0.25_f32).sqrt())).abs() < 0.01);

        assert!((out_circ.apply(0.0) - 0.0).abs() < 0.001);
        assert!((out_circ.apply(1.0) - 1.0).abs() < 0.001);
        assert!((out_circ.apply(0.5) - (1.0_f32 - 0.25_f32).sqrt()).abs() < 0.01);

        assert!((in_out_circ.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_out_circ.apply(1.0) - 1.0).abs() < 0.001);
        assert!((in_out_circ.apply(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_easing_bounce_family() {
        let in_bounce = Easing::EaseInBounce;
        let out_bounce = Easing::EaseOutBounce;
        let in_out_bounce = Easing::EaseInOutBounce;

        assert!((in_bounce.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_bounce.apply(1.0) - 1.0).abs() < 0.001);
        let mid_in = in_bounce.apply(0.5);
        assert!(mid_in > 0.0 && mid_in < 1.0);

        assert!((out_bounce.apply(0.0) - 0.0).abs() < 0.001);
        assert!((out_bounce.apply(1.0) - 1.0).abs() < 0.001);
        let mid_out = out_bounce.apply(0.5);
        assert!(mid_out > 0.0 && mid_out < 1.0);

        assert!((in_out_bounce.apply(0.0) - 0.0).abs() < 0.001);
        assert!((in_out_bounce.apply(1.0) - 1.0).abs() < 0.001);
        assert!((in_out_bounce.apply(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_easing_back_and_elastic_endpoints() {
        let easings = [
            Easing::EaseInBack,
            Easing::EaseOutBack,
            Easing::EaseInOutBack,
            Easing::EaseInElastic,
            Easing::EaseOutElastic,
            Easing::EaseInOutElastic,
        ];

        for easing in easings {
            assert!((easing.apply(0.0) - 0.0).abs() < 0.001);
            assert!((easing.apply(1.0) - 1.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_ease_steps() {
        let steps_end = EaseSteps::new(4, EaseStepsDirection::End);
        assert_eq!(steps_end.apply(0.0), 0.0);
        assert_eq!(steps_end.apply(1.0), 1.0);
        assert!((steps_end.apply(0.26) - 0.25).abs() < 0.01);

        let steps_start = EaseSteps::new(4, EaseStepsDirection::Start);
        assert_eq!(steps_start.apply(0.0), 0.0);
        assert_eq!(steps_start.apply(1.0), 1.0);
        assert!((steps_start.apply(0.26) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_ease_among() {
        let among = EaseAmong::new(vec![0.0, 1.0, 0.0]);
        assert!((among.apply(0.0) - 0.0).abs() < 0.001);
        assert!((among.apply(0.5) - 1.0).abs() < 0.01);
        assert!((among.apply(1.0) - 0.0).abs() < 0.001);
    }
}
