//! Sampled body states for comparing two runs of the same scene.

use glam::{DQuat, DVec3, DVec4};
use serde::{Deserialize, Serialize};

use crate::world::World;

/// One body's state at one tick.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BodyState {
    pub pos: DVec3,
    pub vel: DVec3,
    pub orientation: DQuat,
    pub omega: DVec3,
}

/// Every body at one tick.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub tick: u64,
    pub bodies: Vec<BodyState>,
}

/// Largest allowed difference per quantity. Orientation compares the angle
/// between the two attitudes, rad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    pub pos: f64,
    pub vel: f64,
    pub angle: f64,
    pub omega: f64,
}

impl Tolerance {
    /// Bitwise-equal runs.
    pub const EXACT: Self = Self {
        pos: 0.0,
        vel: 0.0,
        angle: 0.0,
        omega: 0.0,
    };
}

/// Small-angle difference between attitudes, rad. Unlike an arccosine of
/// the dot product, it is exactly zero for equal quaternions and treats `q`
/// and `-q` as the same attitude.
#[must_use]
pub fn attitude_difference(a: DQuat, b: DQuat) -> f64 {
    let (a, b) = (DVec4::from(a), DVec4::from(b));
    2.0 * (a - b).length().min((a + b).length())
}

/// Where two traces first differ beyond tolerance.
#[derive(Clone, Debug, PartialEq)]
pub struct Divergence {
    pub tick: u64,
    pub body: usize,
    pub quantity: &'static str,
    pub difference: f64,
}

/// A run's samples in tick order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub samples: Vec<Sample>,
}

impl Trace {
    /// Append the world's current state.
    pub fn record(&mut self, world: &World) {
        self.samples.push(Sample {
            tick: world.tick,
            bodies: world
                .bodies()
                .iter()
                .map(|b| BodyState {
                    pos: b.pos,
                    vel: b.vel,
                    orientation: b.orientation,
                    omega: b.omega,
                })
                .collect(),
        });
    }

    /// Largest difference per quantity over matching samples and bodies
    /// (by tick), for reporting how far two runs drift apart.
    #[must_use]
    pub fn max_difference(&self, other: &Self) -> Tolerance {
        let mut worst = Tolerance::EXACT;
        for (a, b) in self.samples.iter().zip(&other.samples) {
            for (x, y) in a.bodies.iter().zip(&b.bodies) {
                worst.pos = worst.pos.max(x.pos.distance(y.pos));
                worst.vel = worst.vel.max(x.vel.distance(y.vel));
                worst.angle = worst
                    .angle
                    .max(attitude_difference(x.orientation, y.orientation));
                worst.omega = worst.omega.max(x.omega.distance(y.omega));
            }
        }
        worst
    }

    /// Compare sample by sample.
    ///
    /// # Errors
    ///
    /// Returns the first divergence: a tick or body-count mismatch, or a
    /// quantity beyond `tolerance`.
    pub fn compare(&self, other: &Self, tolerance: Tolerance) -> Result<(), Divergence> {
        let mismatch = |tick, quantity| Divergence {
            tick,
            body: 0,
            quantity,
            difference: f64::INFINITY,
        };
        if self.samples.len() != other.samples.len() {
            return Err(mismatch(0, "sample count"));
        }
        for (a, b) in self.samples.iter().zip(&other.samples) {
            if a.tick != b.tick {
                return Err(mismatch(a.tick, "tick"));
            }
            if a.bodies.len() != b.bodies.len() {
                return Err(mismatch(a.tick, "body count"));
            }
            for (i, (x, y)) in a.bodies.iter().zip(&b.bodies).enumerate() {
                let checks = [
                    ("pos", x.pos.distance(y.pos), tolerance.pos),
                    ("vel", x.vel.distance(y.vel), tolerance.vel),
                    (
                        "orientation",
                        attitude_difference(x.orientation, y.orientation),
                        tolerance.angle,
                    ),
                    ("omega", x.omega.distance(y.omega), tolerance.omega),
                ];
                for (quantity, difference, limit) in checks {
                    if difference > limit || difference.is_nan() {
                        return Err(Divergence {
                            tick: a.tick,
                            body: i,
                            quantity,
                            difference,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
