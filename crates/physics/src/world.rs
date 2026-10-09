//! A world of rigid bodies advanced in fixed steps.

use std::ops::{Index, IndexMut};

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::body::{Body, BodyKind};
use crate::collision::{Collider, ColliderId};
use crate::contact::{ContactReport, SolverSettings};
use crate::joint::Joint;
use crate::ledger::Momentum;

/// Index of a body in its world. Removing a body leaves its slot in place,
/// so ids stay valid and are never reused.
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
    #[serde(skip)]
    pub(crate) collision_index: crate::broadphase::Tree<usize>,
    #[cfg(test)]
    #[serde(skip)]
    pub(crate) exhaustive_detection: bool,
    #[serde(default)]
    pub(crate) joints: Vec<Option<Joint>>,
    /// When bodies fall asleep.
    #[serde(default)]
    pub sleep: SleepSettings,
    /// What each contact point did in the last step.
    #[serde(skip)]
    pub contacts: Vec<ContactReport>,
    /// Last step's contact impulses, which warm start the next step's
    /// solve. Saved with the world so a restored world continues exactly.
    #[serde(default)]
    pub(crate) warm: Vec<crate::contact::WarmContact>,
    /// Bodies put to sleep in the last step, with the momentum about the
    /// world origin that stopping them removed (tiny, and taken up by the
    /// fixed bodies the island rests on).
    #[serde(skip)]
    pub slept: Vec<(BodyId, Momentum)>,
    /// Bodies held up by something other than a contact for this step,
    /// such as still water ([`World::support`]); cleared by the step.
    #[serde(skip)]
    supported: Vec<BodyId>,
    /// Counts and timings of the last step, for profiling.
    #[serde(skip)]
    pub stats: StepStats,
}

/// Island sleep, after Genesis `examples/rigid/hibernation.py`, with a
/// microgravity rule: an island sleeps only while it touches a fixed or
/// sleeping body. A body drifting free is still moving, however slowly, so
/// it never sleeps.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SleepSettings {
    pub enabled: bool,
    /// Speeds below which a body counts as at rest, m/s and rad/s.
    pub linear: f64,
    pub angular: f64,
    /// Rest time before an island sleeps, s.
    pub time: f64,
}

impl PartialEq for StepStats {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Default for SleepSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            linear: 0.01,
            angular: 0.02,
            time: 0.5,
        }
    }
}

/// What the last step did and how long it took. Timings are wall-clock and
/// only for profiling; they never affect the simulation, and two worlds
/// compare equal whatever their stats.
#[derive(Clone, Copy, Debug, Default)]
pub struct StepStats {
    pub awake: usize,
    pub asleep: usize,
    pub manifolds: usize,
    pub detection: crate::collision::DetectionStats,
    pub contact_points: usize,
    pub detect: std::time::Duration,
    pub solve: std::time::Duration,
    pub total: std::time::Duration,
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
            collision_index: Default::default(),
            #[cfg(test)]
            exhaustive_detection: false,
            joints: Vec::new(),
            sleep: SleepSettings::default(),
            contacts: Vec::new(),
            warm: Vec::new(),
            slept: Vec::new(),
            supported: Vec::new(),
            stats: StepStats::default(),
        }
    }

    /// Count body `id` as resting on something fixed for the next step's
    /// sleep test, as a contact with a fixed body would. Still water calls
    /// this for the bodies it floats ([`crate::water::apply`]).
    pub fn support(&mut self, id: BodyId) {
        self.supported.push(id);
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
        let started = crate::wall::Instant::now();
        self.stats = StepStats::default();
        let dt = self.dt;
        self.slept.clear();
        for body in &mut self.bodies {
            body.prev_pos = body.pos;
            body.prev_orientation = body.orientation;
            // An owner that set a sleeper moving has woken it.
            if body.sleeping && (body.vel != DVec3::ZERO || body.omega != DVec3::ZERO) {
                body.wake();
            }
            if body.responds() {
                body.vel +=
                    (field.accel(body.pos, body.vel) + body.force * body.inverse_mass()) * dt;
                if body.torque != DVec3::ZERO {
                    body.apply_angular_impulse(body.torque * dt);
                }
            }
            body.force = DVec3::ZERO;
            body.torque = DVec3::ZERO;
        }
        let mut manifolds = Vec::new();
        self.contacts = if self.colliders.is_empty() && self.joints.iter().all(Option::is_none) {
            Vec::new()
        } else {
            let detecting = crate::wall::Instant::now();
            let base = self.solver.margin;
            let (wake_nodes, wake_candidates, wake_updates) = self.wake_near_kinematic(base);
            let speeds: Vec<_> = self
                .colliders
                .iter()
                .map(|c| {
                    let body = &self.bodies[c.body.0 as usize];
                    body.vel.length()
                        + body.omega_world().length() * (c.offset.length() + c.shape.bound())
                })
                .collect();
            let reaches: Vec<_> = speeds.iter().map(|speed| speed * dt).collect();
            let (detected, mut detection) = self
                .detect_motion_with_margin(base, &reaches, &|i, j| {
                    base + (speeds[i] + speeds[j]) * dt
                })
                .unwrap_or_else(|_| {
                    // Preserve the legacy callback's behavior for unsupported nonfinite/negative settings.
                    let bodies = &self.bodies;
                    self.detect_profiled(&|a, b| {
                        let reach = |c: &Collider| {
                            let body = &bodies[c.body.0 as usize];
                            body.vel.length()
                                + body.omega_world().length()
                                    * (c.offset.length() + c.shape.bound())
                        };
                        base + (reach(a) + reach(b)) * dt
                    })
                });
            manifolds = detected;
            detection.wake_scene_nodes = wake_nodes;
            detection.wake_candidate_pairs = wake_candidates;
            detection.wake_index_updates = wake_updates;
            self.stats.detection = detection;
            self.stats.detect = detecting.elapsed();
            self.wake_touched(&manifolds);
            let solving = crate::wall::Instant::now();
            let contacts = self.solve(&manifolds, dt);
            self.stats.solve = solving.elapsed();
            contacts
        };
        for body in &mut self.bodies {
            if body.moves() && !body.sleeping {
                body.pos += body.vel * dt;
                body.rotate(dt);
            }
        }
        if self.sleep.enabled {
            self.settle(&manifolds);
        }
        self.supported.clear();
        self.tick += 1;
        self.stats.manifolds = manifolds.len();
        self.stats.contact_points = self.contacts.len();
        self.stats.asleep = self.bodies.iter().filter(|b| b.sleeping).count();
        self.stats.awake = self
            .bodies
            .iter()
            .filter(|b| b.kind == BodyKind::Dynamic && !b.sleeping)
            .count();
        self.stats.total = started.elapsed();
    }

    /// Remove a body: it becomes fixed and inert, its colliders stop
    /// colliding, and its joints are removed, waking what they held. Its id
    /// stays valid and is not reused.
    pub fn remove_body(&mut self, id: BodyId) {
        let joints: Vec<crate::joint::JointId> = self
            .joints()
            .filter(|(_, j)| j.a == id || j.b == id)
            .map(|(joint, _)| joint)
            .collect();
        for joint in joints {
            self.remove_joint(joint);
        }
        for collider in &mut self.colliders {
            if collider.body == id {
                collider.filter = crate::collision::Filter::NONE;
            }
        }
        let body = &mut self[id];
        body.kind = BodyKind::Static;
        body.vel = DVec3::ZERO;
        body.omega = DVec3::ZERO;
        body.force = DVec3::ZERO;
        body.torque = DVec3::ZERO;
        body.sleeping = false;
        body.removed = true;
    }

    /// Wake a body.
    pub fn wake(&mut self, id: BodyId) {
        self[id].wake();
    }

    fn moving(&self, body: &Body) -> bool {
        body.vel.length() >= self.sleep.linear || body.omega_world().length() >= self.sleep.angular
    }

    /// A moving body that touches or is joined to a sleeper wakes it.
    fn wake_touched(&mut self, manifolds: &[crate::collision::Manifold]) {
        let mut pairs: Vec<(usize, usize)> = manifolds
            .iter()
            .map(|m| {
                (
                    self.colliders[m.a.0 as usize].body.0 as usize,
                    self.colliders[m.b.0 as usize].body.0 as usize,
                )
            })
            .collect();
        pairs.extend(
            self.joints
                .iter()
                .flatten()
                .map(|j| (j.a.0 as usize, j.b.0 as usize)),
        );
        for (a, b) in pairs {
            for (sleeper, other) in [(a, b), (b, a)] {
                if self.bodies[sleeper].sleeping && self.pushes(other) {
                    self.bodies[sleeper].wake();
                }
            }
        }
    }

    /// Whether body `i` can disturb a sleeper: an awake dynamic body that is
    /// moving, or a kinematic body its owner is moving.
    fn pushes(&self, i: usize) -> bool {
        let body = &self.bodies[i];
        match body.kind {
            BodyKind::Dynamic => body.responds() && self.moving(body),
            BodyKind::Kinematic => body.vel != DVec3::ZERO || body.omega != DVec3::ZERO,
            BodyKind::Static => false,
        }
    }

    /// Wake sleepers whose colliders come within `margin` of a moving
    /// kinematic body's. Detection skips pairs where neither side can
    /// respond, so without this a scripted body would pass through a
    /// sleeper instead of pushing it.
    fn wake_near_kinematic(&mut self, margin: f64) -> (usize, usize, usize) {
        let movers: std::collections::BTreeSet<usize> = (0..self.bodies.len())
            .filter(|&i| self.bodies[i].kind == BodyKind::Kinematic && self.pushes(i))
            .collect();
        if movers.is_empty() {
            return (0, 0, 0);
        }
        let bound = |c: &Collider, world: &Self| (c.pose(world).0, c.shape.bound());
        let mut updates = 0;
        #[cfg(test)]
        let use_index = !self.exhaustive_detection;
        #[cfg(not(test))]
        let use_index = true;
        let indexed = use_index
            && margin.is_finite()
            && margin >= 0.
            && self.dt.is_finite()
            && self.dt >= 0.
            && self.colliders.iter().all(|c| {
                let (pos, radius) = bound(c, self);
                pos.is_finite()
                    && radius.is_finite()
                    && radius >= 0.
                    && self[c.body].vel.is_finite()
                    && self[c.body].omega.is_finite()
            });
        if indexed {
            for i in 0..self.colliders.len() {
                let c = &self.colliders[i];
                let (pos, radius) = bound(c, self);
                let body = &self[c.body];
                let reach = (body.vel.length()
                    + body.omega_world().length() * (c.offset.length() + radius))
                    * self.dt;
                let extent = DVec3::splat(radius + margin * 0.5 + reach);
                if c.filter == crate::collision::Filter::NONE {
                    updates += usize::from(self.collision_index.remove(i));
                } else {
                    updates += usize::from(self.collision_index.set(
                        i,
                        crate::broadphase::Bounds {
                            min: pos - extent,
                            max: pos + extent,
                        },
                    ));
                }
            }
        }
        let mut nodes = 0;
        let mut candidates = 0;
        let mut wake = Vec::new();
        for mover in &self.colliders {
            if !movers.contains(&(mover.body.0 as usize)) {
                continue;
            }
            let (pa, ra) = bound(mover, self);
            let reach = self[mover.body].vel.length() * self.dt + margin;
            let indices = if indexed {
                let extent = DVec3::splat(ra + reach);
                self.collision_index.query(
                    crate::broadphase::Bounds {
                        min: pa - extent,
                        max: pa + extent,
                    },
                    &mut nodes,
                )
            } else {
                (0..self.colliders.len()).collect()
            };
            for index in indices {
                candidates += 1;
                let other = &self.colliders[index];
                if other.body == mover.body
                    || !self[other.body].sleeping
                    || !mover.filter.allows(other.filter)
                {
                    continue;
                }
                let (pb, rb) = bound(other, self);
                if pa.distance(pb) <= ra + rb + reach {
                    wake.push(other.body);
                }
            }
        }
        for id in wake {
            self[id].wake();
        }
        (nodes, candidates, updates)
    }

    /// Advance sleep timers and put settled islands to sleep.
    fn settle(&mut self, manifolds: &[crate::collision::Manifold]) {
        let n = self.bodies.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        let mut grounded_bodies = vec![false; n];
        let touching = |m: &crate::collision::Manifold| {
            m.points
                .iter()
                .any(|p| p.separation <= self.solver.slop * 2.0)
        };
        let mut links: Vec<(usize, usize)> = manifolds
            .iter()
            .filter(|m| touching(m))
            .map(|m| {
                (
                    self.colliders[m.a.0 as usize].body.0 as usize,
                    self.colliders[m.b.0 as usize].body.0 as usize,
                )
            })
            .collect();
        // Welds and point joints always connect; a tether only while taut,
        // so a slack line to the station does not ground a drifting body.
        links.extend(
            self.joints
                .iter()
                .flatten()
                .filter(|j| {
                    !matches!(j.kind, crate::joint::JointKind::Tether { .. })
                        || j.impulse != DVec3::ZERO
                })
                .map(|j| (j.a.0 as usize, j.b.0 as usize)),
        );
        for (a, b) in links {
            let (ra, rb) = (self.bodies[a].responds(), self.bodies[b].responds());
            match (ra, rb) {
                (true, true) => {
                    let (x, y) = (root(&mut parent, a), root(&mut parent, b));
                    parent[x] = y;
                }
                // Resting on something fixed or asleep (a moving kinematic
                // body does not count).
                (true, false) if self.rests_on(b) => grounded_bodies[a] = true,
                (false, true) if self.rests_on(a) => grounded_bodies[b] = true,
                _ => {}
            }
        }
        for id in &self.supported {
            if let Some(grounded) = grounded_bodies.get_mut(id.0 as usize) {
                *grounded = true;
            }
        }
        let dt = self.dt;
        let mut island_time = vec![f64::INFINITY; n];
        let mut island_grounded = vec![false; n];
        for (i, grounded) in grounded_bodies.iter().enumerate() {
            if !self.bodies[i].responds() {
                continue;
            }
            let slow = !self.moving(&self.bodies[i]);
            let body = &mut self.bodies[i];
            body.sleep_time = if slow { body.sleep_time + dt } else { 0.0 };
            let r = root(&mut parent, i);
            island_time[r] = island_time[r].min(self.bodies[i].sleep_time);
            island_grounded[r] |= *grounded;
        }
        for i in 0..n {
            if !self.bodies[i].responds() {
                continue;
            }
            let r = root(&mut parent, i);
            if island_grounded[r] && island_time[r] >= self.sleep.time {
                let removed = Momentum::of(&self.bodies[i], DVec3::ZERO);
                let body = &mut self.bodies[i];
                body.vel = DVec3::ZERO;
                body.omega = DVec3::ZERO;
                body.sleeping = true;
                self.slept.push((BodyId(i as u32), removed));
            }
        }
    }

    fn rests_on(&self, i: usize) -> bool {
        let body = &self.bodies[i];
        body.kind == BodyKind::Static
            || body.sleeping
            || (body.kind == BodyKind::Kinematic
                && body.vel == DVec3::ZERO
                && body.omega == DVec3::ZERO)
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
