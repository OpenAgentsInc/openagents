//! Flexible solar array wings: assumed modes on the two rigid array
//! colliders, excited by plume impingement and station-keeping burns.
//!
//! Each wing is a uniform cantilevered plate from its root at |x| = 13 m to
//! its tip at |x| = 30 m, spanning y from −0.5 to 12.5 m in the plane
//! z = −1.1 m, face-on to the Sun. Out-of-plane motion is along scene z.
//! Three assumed modes per wing, after Hughes, *Spacecraft Attitude
//! Dynamics* (1986), chapter 9:
//!
//! 1. First out-of-plane bending, the cantilever beam's first mode
//!    (0.15 Hz).
//! 2. First torsion about the span axis, linear across the chord and a
//!    quarter sine along the span (0.5 Hz).
//! 3. Second out-of-plane bending (0.94 Hz, the beam's ratio of 6.27).
//!
//! Every mode has 0.5 % of critical damping and a shape normalized to 1 at
//! the tip (for torsion, at the tip's chord edge). Generalized masses and
//! participation factors come from the shapes by quadrature. The rigid
//! collider stays authoritative for contact; flex changes only what is
//! drawn and what the wing stores.

use glam::DVec3;
use physics::{BodyId, ColliderId, Mode, Sample};
use serde::{Deserialize, Serialize};

/// Distance of each wing's root from the station's y–z plane, m.
pub const ROOT: f64 = 13.0;
/// Distance of each wing's tip, m.
pub const TIP: f64 = 30.0;
/// The wings' edges along y, m.
pub const CHORD: (f64, f64) = (-0.5, 12.5);
/// The wings' plane, z, m.
pub const PLANE: f64 = -1.1;
/// Mass of one wing, kg: about 1.4 kg/m² over 221 m², like a flexible
/// blanket array.
pub const WING_MASS: f64 = 300.0;
/// Natural frequencies, Hz.
pub const FREQUENCIES: [f64; 3] = [0.15, 0.5, 0.94];
/// Damping ratio of every mode.
pub const DAMPING: f64 = 0.005;

/// First two roots of the cantilever frequency equation and their mode
/// constants.
const BEAM: [(f64, f64); 2] = [(1.875_104_07, 0.734_095_51), (4.694_091_13, 1.018_467_32)];

/// A cantilever bending shape, normalized to 1 at the tip.
fn beam(mode: usize, s: f64) -> f64 {
    let (beta, sigma) = BEAM[mode];
    let raw = |s: f64| {
        let x = beta * s;
        x.cosh() - x.cos() - sigma * (x.sinh() - x.sin())
    };
    raw(s) / raw(1.0)
}

/// Mode shape `mode` at span `s` in [0, 1] (root to tip) and chord `c` in
/// [−1, 1] (−0.5 m to 12.5 m in y).
#[must_use]
pub fn shape(mode: usize, s: f64, c: f64) -> f64 {
    match mode {
        0 => beam(0, s),
        1 => c * (std::f64::consts::FRAC_PI_2 * s).sin(),
        _ => beam(1, s),
    }
}

/// One wing and its modes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wing {
    /// −1 for the wing on the −x side, +1 for the +x side.
    pub side: f64,
    /// The wing's rigid collider in the station world.
    pub collider: ColliderId,
    pub body: BodyId,
    pub modes: [Mode; 3],
    /// Mean of each mode shape over the wing: the share of a uniform load
    /// each mode takes.
    pub participation: [f64; 3],
}

impl Wing {
    #[must_use]
    pub fn new(side: f64, collider: ColliderId, body: BodyId) -> Self {
        // Midpoint quadrature over the plate.
        let (ns, nc) = (128, 16);
        let mut square = [0.0; 3];
        let mut mean = [0.0; 3];
        for i in 0..ns {
            let s = (f64::from(i) + 0.5) / f64::from(ns);
            for j in 0..nc {
                let c = (f64::from(j) + 0.5) / f64::from(nc) * 2.0 - 1.0;
                for (k, (sq, m)) in square.iter_mut().zip(&mut mean).enumerate() {
                    let phi = shape(k, s, c);
                    *sq += phi * phi;
                    *m += phi;
                }
            }
        }
        let cells = f64::from(ns * nc);
        let modes = std::array::from_fn(|k| {
            Mode::new(FREQUENCIES[k], DAMPING, WING_MASS * square[k] / cells)
        });
        Self {
            side,
            collider,
            body,
            modes,
            participation: mean.map(|m| m / cells),
        }
    }

    /// Span and chord coordinates of a world point on the wing.
    #[must_use]
    pub fn coordinates(&self, point: DVec3) -> (f64, f64) {
        let s = ((point.x * self.side - ROOT) / (TIP - ROOT)).clamp(0.0, 1.0);
        let c = ((point.y - CHORD.0) / (CHORD.1 - CHORD.0) * 2.0 - 1.0).clamp(-1.0, 1.0);
        (s, c)
    }

    /// Advance by `dt` under plume `samples` (those on this wing count) and
    /// the station's acceleration `base`, m/s². In the station's frame a
    /// base acceleration loads every element with minus its mass times it.
    pub fn step(&mut self, samples: &[Sample], base: DVec3, dt: f64) {
        let mut force = self.participation.map(|p| -WING_MASS * p * base.z);
        for sample in samples.iter().filter(|s| s.collider == self.collider) {
            let (s, c) = self.coordinates(sample.point);
            for (k, q) in force.iter_mut().enumerate() {
                *q += shape(k, s, c) * sample.force.z;
            }
        }
        for (mode, q) in self.modes.iter_mut().zip(force) {
            mode.step(q, dt);
        }
    }

    /// Total strain and kinetic energy in the wing's modes, J.
    #[must_use]
    pub fn energy(&self) -> f64 {
        self.modes.iter().map(Mode::energy).sum()
    }

    /// The wing's shape between the last two steps; `alpha` in [0, 1].
    #[must_use]
    pub fn flex(&self, alpha: f64) -> ArrayFlex {
        ArrayFlex {
            side: self.side,
            q: self.modes.map(|m| m.interpolated(alpha)),
        }
    }
}

/// A wing's deflected shape at one instant, for drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrayFlex {
    /// −1 for the wing on the −x side, +1 for the +x side.
    pub side: f64,
    /// Modal amplitudes: tip deflection of the first and second bending
    /// modes and the tip chord edge's deflection in torsion, m.
    pub q: [f64; 3],
}

impl ArrayFlex {
    /// Out-of-plane displacement along scene z, m, at span `s` in [0, 1]
    /// from root (|x| = 13 m) to tip (|x| = 30 m) and chord `c` in [−1, 1]
    /// from the y = −0.5 m edge to the y = 12.5 m edge.
    #[must_use]
    pub fn displacement(&self, s: f64, c: f64) -> f64 {
        let (s, c) = (s.clamp(0.0, 1.0), c.clamp(-1.0, 1.0));
        self.q
            .iter()
            .enumerate()
            .map(|(k, q)| q * shape(k, s, c))
            .sum()
    }

    /// Twist of the chord line at span `s` about the span axis, rad,
    /// positive when the y = 12.5 m edge moves toward +z.
    #[must_use]
    pub fn twist(&self, s: f64) -> f64 {
        let half_chord = (CHORD.1 - CHORD.0) * 0.5;
        (self.q[1] * (std::f64::consts::FRAC_PI_2 * s.clamp(0.0, 1.0)).sin() / half_chord).atan()
    }

    /// The deflected position of the undeflected wing point `point`, scene
    /// coordinates, m.
    #[must_use]
    pub fn deflect(&self, point: DVec3) -> DVec3 {
        let s = ((point.x * self.side - ROOT) / (TIP - ROOT)).clamp(0.0, 1.0);
        let c = ((point.y - CHORD.0) / (CHORD.1 - CHORD.0) * 2.0 - 1.0).clamp(-1.0, 1.0);
        point + DVec3::Z * self.displacement(s, c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shapes_are_normalized_and_the_modal_masses_are_the_beams() {
        for k in [0, 2] {
            assert!((shape(k, 1.0, 0.0) - 1.0).abs() < 1e-12);
            assert!(shape(k, 0.0, 0.3).abs() < 1e-12, "clamped at the root");
        }
        assert!((shape(1, 1.0, 1.0) - 1.0).abs() < 1e-12);
        let wing = Wing::new(1.0, ColliderId(0), BodyId(0));
        // A tip-normalized cantilever mode has a quarter of the beam's mass.
        for k in [0, 2] {
            assert!((wing.modes[k].mass / WING_MASS - 0.25).abs() < 1e-3);
        }
        // Torsion: a third across the chord times a half along the span.
        assert!((wing.modes[1].mass / WING_MASS - 1.0 / 6.0).abs() < 1e-3);
        // The first mode takes 0.39 of a uniform load; torsion takes none.
        assert!((wing.participation[0] - 0.3915).abs() < 1e-3);
        assert!(wing.participation[1].abs() < 1e-12);
    }

    #[test]
    fn a_base_acceleration_bends_the_wing_and_it_rings_down() {
        let mut wing = Wing::new(-1.0, ColliderId(0), BodyId(0));
        let dt = 1.0 / 120.0;
        // A 1.2 s burn pushing the station toward the Earth.
        for _ in 0..144 {
            wing.step(&[], DVec3::new(0.0, 0.0, 0.01), dt);
        }
        let flex = wing.flex(1.0);
        // The wing lags the root, so its tip swings toward the Sun (-z).
        assert!(flex.displacement(1.0, 0.0) < -1e-3, "{flex:?}");
        assert!(flex.q[1].abs() < 1e-15, "no torsion from a uniform load");
        let energy = wing.energy();
        for _ in 0..(120 * 120) {
            wing.step(&[], DVec3::ZERO, dt);
        }
        // Two minutes at 0.5 % damping: the first mode keeps e^{-2ζωt}.
        let kept = wing.energy() / energy;
        let expected = (-2.0 * DAMPING * std::f64::consts::TAU * 0.15 * 120.0).exp();
        assert!(
            kept < expected * 1.05 && kept > expected * 0.3,
            "{kept} vs {expected}"
        );
    }

    #[test]
    fn an_off_center_load_twists_the_wing() {
        let mut wing = Wing::new(1.0, ColliderId(4), BodyId(4));
        let push = Sample {
            collider: ColliderId(4),
            body: BodyId(4),
            point: DVec3::new(29.0, 12.0, -0.95),
            force: DVec3::new(0.0, 0.0, -2.0),
        };
        let elsewhere = Sample {
            collider: ColliderId(5),
            ..push
        };
        for _ in 0..60 {
            wing.step(&[push, elsewhere], DVec3::ZERO, 1.0 / 120.0);
        }
        let flex = wing.flex(1.0);
        assert!(flex.q[0] < 0.0 && flex.q[1] < 0.0, "{flex:?}");
        assert!(flex.twist(1.0) < 0.0);
        let tip = flex.deflect(DVec3::new(30.0, 12.5, PLANE));
        assert!(tip.z < PLANE);
    }
}
