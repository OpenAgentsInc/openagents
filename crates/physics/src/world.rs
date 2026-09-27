//! A world of rigid bodies advanced in fixed steps.

use std::ops::{Index, IndexMut};

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::body::{Body, BodyKind};
use crate::collision::{Collider, ColliderId};
use crate::contact::{ContactReport, SolverSettings};
use crate::joint::Joint;

/// Index of a body in its world. Bodies are never removed, so ids stay valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BodyId(pub u32);

/// Acceleration every dynamic body feels, from gravity or a tidal field.
pub trait Field {
    fn accel(&self, pos: DVec3, vel: DVec3) -> DVec3;
}

impl<F: Fn(DVec3, DVec3) -> DVec3> Field for F {
    fn accel(&self, pos: DVec3, vel: DVec3) -> DVec3 {
        self(pos, vel)
    }
}

/// No field: free fall far from everything.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoField;

impl Field for NoField {
    fn accel(&self, _: DVec3, _: DVec3) -> DVec3 {
        DVec3::ZERO
    }
}

/// Uniform gravity.
#[derive(Clone, Copy, Debug)]
pub struct Uniform(pub DVec3);

impl Field for Uniform {
    fn accel(&self, _: DVec3, _: DVec3) -> DVec3 {
        self.0
    }
}

/// Bodies, a fixed step length, and a step counter. The whole world
/// serializes, so a saved world restores and continues bit for bit when
/// the format round-trips floats exactly (for `serde_json`, enable its
/// `float_roundtrip` feature).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct World {
    /// Serialized layout version; see [`World::VERSION`].
    pub version: u32,
    /// Step length, s.
    pub dt: f64,
    /// Steps taken.
    pub tick: u64,
    /// Contact solver tuning.
    #[serde(default)]
    pub solver: SolverSettings,
    bodies: Vec<Body>,
    #[serde(default)]
    colliders: Vec<Collider>,
    #[serde(default)]
    pub(crate) joints: Vec<Option<Joint>>,
    /// What each contact point did in the last step.
    #[serde(skip)]
    pub contacts: Vec<ContactReport>,
}

impl World {
    /// Bumped whenever the serialized layout changes meaning.
    pub const VERSION: u32 = 2;

    #[must_use]
    pub fn new(dt: f64) -> Self {
        Self {
            version: Self::VERSION,
            dt,
            tick: 0,
            solver: SolverSettings::default(),
            bodies: Vec::new(),
            colliders: Vec::new(),
            joints: Vec::new(),
            contacts: Vec::new(),
        }
    }

    /// Attach a collider to its body.
    pub fn add_collider(&mut self, collider: Collider) -> ColliderId {
        self.colliders.push(collider);
        ColliderId(u32::try_from(self.colliders.len() - 1).expect("fewer than 2^32 colliders"))
    }

    #[must_use]
    pub fn colliders(&self) -> &[Collider] {
        &self.colliders
    }

    pub fn collider_mut(&mut self, id: ColliderId) -> &mut Collider {
        &mut self.colliders[id.0 as usize]
    }

    pub fn add(&mut self, body: Body) -> BodyId {
        self.bodies.push(body);
        BodyId(u32::try_from(self.bodies.len() - 1).expect("fewer than 2^32 bodies"))
    }

    #[must_use]
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    pub fn bodies_mut(&mut self) -> &mut [Body] {
        &mut self.bodies
    }

    /// Simulated time, s.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.tick as f64 * self.dt
    }

    /// Advance one step of `dt`: accumulated forces, torques, and the field
    /// change velocities (then the accumulators clear); contacts found at the
    /// current poses correct velocities; then positions advance by
    /// semi-implicit Euler and rotation by the torque-free [`Body::rotate`].
    pub fn step(&mut self, field: &impl Field) {
        let dt = self.dt;
        for body in &mut self.bodies {
            body.prev_pos = body.pos;
            body.prev_orientation = body.orientation;
            if body.kind == BodyKind::Dynamic {
                body.vel +=
                    (field.accel(body.pos, body.vel) + body.force * body.inverse_mass()) * dt;
                body.apply_angular_impulse(body.torque * dt);
            }
            body.force = DVec3::ZERO;
            body.torque = DVec3::ZERO;
        }
        self.contacts = if self.colliders.is_empty() && self.joints.iter().all(Option::is_none) {
            Vec::new()
        } else {
            let base = self.solver.margin;
            let bodies = &self.bodies;
            let manifolds = self.detect(&|a, b| {
                let reach = |c: &Collider| {
                    let body = &bodies[c.body.0 as usize];
                    body.vel.length()
                        + body.omega_world().length() * (c.offset.length() + c.shape.bound())
                };
                base + (reach(a) + reach(b)) * dt
            });
            self.solve(&manifolds, dt)
        };
        for body in &mut self.bodies {
            if body.moves() {
                body.pos += body.vel * dt;
                body.rotate(dt);
            }
        }
        self.tick += 1;
    }

    /// Restore a serialized world, refusing another layout version.
    ///
    /// # Errors
    ///
    /// Returns a message when the version is not [`World::VERSION`].
    pub fn check_version(&self) -> Result<(), String> {
        if self.version == Self::VERSION {
            Ok(())
        } else {
            Err(format!(
                "physics world version {} is not the supported version {}",
                self.version,
                Self::VERSION
            ))
        }
    }
}

impl Index<BodyId> for World {
    type Output = Body;
    fn index(&self, id: BodyId) -> &Body {
        &self.bodies[id.0 as usize]
    }
}

impl IndexMut<BodyId> for World {
    fn index_mut(&mut self, id: BodyId) -> &mut Body {
        &mut self.bodies[id.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{Tolerance, Trace};
    use glam::DQuat;

    fn scene() -> World {
        let mut world = World::new(1.0 / 120.0);
        let mut ball = Body::new(2.0, DVec3::splat(0.1), DVec3::new(0.0, 10.0, 0.0));
        ball.vel = DVec3::new(1.0, 0.0, 0.0);
        ball.omega = DVec3::new(0.2, 1.0, 0.01);
        world.add(ball);
        world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
        let mut conveyor = Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Kinematic);
        conveyor.vel = DVec3::new(0.0, 0.0, 0.5);
        world.add(conveyor);
        world
    }

    #[test]
    fn gravity_is_exact_in_velocity_and_static_bodies_stay() {
        let mut world = scene();
        let g = Uniform(DVec3::new(0.0, -9.81, 0.0));
        for _ in 0..120 {
            world.step(&g);
        }
        assert_eq!(world.tick, 120);
        assert!((world.time() - 1.0).abs() < 1e-12);
        let ball = world[BodyId(0)];
        assert!((ball.vel.y + 9.81).abs() < 1e-9);
        // Semi-implicit Euler drops g t^2 / 2 plus g dt t / 2.
        let expected = 10.0 - 0.5 * 9.81 - 0.5 * 9.81 / 120.0;
        assert!((ball.pos.y - expected).abs() < 1e-9, "{}", ball.pos.y);
        assert_eq!(world[BodyId(1)].pos, DVec3::ZERO);
        // Kinematic bodies ignore the field and keep their velocity.
        assert!((world[BodyId(2)].pos - DVec3::new(0.0, 0.0, 0.5)).length() < 1e-12);
    }

    #[test]
    fn a_restored_world_continues_bit_for_bit() {
        let mut world = scene();
        let g = Uniform(DVec3::new(0.0, -1.62, 0.0));
        for _ in 0..50 {
            world.step(&g);
        }
        let json = serde_json::to_string(&world).unwrap();
        let mut restored: World = serde_json::from_str(&json).unwrap();
        restored.check_version().unwrap();
        assert_eq!(restored, world);
        let (mut a, mut b) = (Trace::default(), Trace::default());
        for _ in 0..200 {
            world.step(&g);
            restored.step(&g);
            a.record(&world);
            b.record(&restored);
        }
        a.compare(&b, Tolerance::EXACT).unwrap();
    }

    #[test]
    fn interpolation_runs_from_the_previous_pose() {
        let mut world = scene();
        world.step(&NoField);
        let ball = world[BodyId(0)];
        let (half, _) = ball.interpolated(0.5);
        assert!((half - (ball.prev_pos + ball.pos) * 0.5).length() < 1e-12);
        let (_, start) = ball.interpolated(0.0);
        assert!(start.angle_between(DQuat::IDENTITY) < 1e-12);
    }

    #[test]
    fn other_layout_versions_are_refused() {
        let mut world = scene();
        world.version = 99;
        assert!(world.check_version().is_err());
    }
}
