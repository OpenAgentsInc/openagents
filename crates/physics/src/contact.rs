//! Sequential-impulse contact solver: non-penetration with restitution and
//! position correction, Coulomb friction on an elliptic cone (the tangential
//! impulse's magnitude, not each axis, is bounded by the friction limit),
//! and torsional friction about the normal (Genesis
//! `examples/rigid/friction_breakaway.py`, `torsional_grasp.py`).
//!
//! Impulses act equally and oppositely at each contact point, so contacts
//! between moving bodies conserve linear and angular momentum.

use glam::{DMat3, DVec3};
use serde::{Deserialize, Serialize};

use crate::collision::{ColliderId, Manifold};
use crate::world::{BodyId, World};

/// Solver tuning. Named, so a scene that needs a stiffer contact (the tight
/// thread in Genesis `bolt_nut_self_screw.py`) states it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolverSettings {
    /// Velocity iterations per step.
    pub iterations: u32,
    /// Fraction of penetration beyond `slop` removed per step.
    pub erp: f64,
    /// Penetration tolerated without correction, m.
    pub slop: f64,
    /// Closing speed below which contacts do not bounce, m/s.
    pub bounce_threshold: f64,
    /// Contacts start this far before touching, m, plus the distance the
    /// pair can close in one step.
    pub margin: f64,
}

impl Default for SolverSettings {
    fn default() -> Self {
        Self {
            iterations: 20,
            erp: 0.2,
            slop: 0.005,
            bounce_threshold: 0.1,
            margin: 0.01,
        }
    }
}

/// What one contact point did in the last step.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContactReport {
    pub a: ColliderId,
    pub b: ColliderId,
    pub body_a: BodyId,
    pub body_b: BodyId,
    pub point: DVec3,
    /// From `a` to `b`.
    pub normal: DVec3,
    pub separation: f64,
    /// Impulse on `b` at `point`, N s; `a` received the opposite.
    pub impulse: DVec3,
    /// Angular impulse on `b` from torsional friction, N m s; `a` received
    /// the opposite.
    pub twist: DVec3,
}

/// Velocity state of one body during the solve, world frame.
#[derive(Clone, Copy)]
struct Motion {
    vel: DVec3,
    omega: DVec3,
    inverse_mass: f64,
    inverse_inertia: DMat3,
}

impl Motion {
    fn point_velocity(&self, r: DVec3) -> DVec3 {
        self.vel + self.omega.cross(r)
    }

    fn push(&mut self, impulse: DVec3, r: DVec3) {
        self.vel += impulse * self.inverse_mass;
        self.omega += self.inverse_inertia * r.cross(impulse);
    }

    fn twist(&mut self, angular: DVec3) {
        self.omega += self.inverse_inertia * angular;
    }
}

struct Row {
    a: usize,
    b: usize,
    ra: DVec3,
    rb: DVec3,
    normal: DVec3,
    tangents: [DVec3; 2],
    normal_mass: f64,
    tangent_mass: [f64; 2],
    twist_mass: f64,
    target: f64,
    friction: f64,
    torsional: f64,
    normal_impulse: f64,
    tangent_impulse: [f64; 2],
    twist_impulse: f64,
    report: ContactReport,
}

fn basis(n: DVec3) -> [DVec3; 2] {
    let t = if n.x.abs() < 0.57 {
        DVec3::X.cross(n)
    } else {
        DVec3::Y.cross(n)
    }
    .normalize();
    [t, n.cross(t)]
}

fn effective_mass(ma: &Motion, mb: &Motion, ra: DVec3, rb: DVec3, d: DVec3) -> f64 {
    let (ca, cb) = (ra.cross(d), rb.cross(d));
    let k = ma.inverse_mass
        + mb.inverse_mass
        + ca.dot(ma.inverse_inertia * ca)
        + cb.dot(mb.inverse_inertia * cb);
    if k > 0.0 { 1.0 / k } else { 0.0 }
}

impl World {
    /// Resolve `manifolds` by changing body velocities for a step of `dt`.
    /// Returns what each contact point did.
    pub(crate) fn solve_contacts(&mut self, manifolds: &[Manifold], dt: f64) -> Vec<ContactReport> {
        let settings = self.solver;
        let mut motions: Vec<Motion> = self
            .bodies()
            .iter()
            .map(|b| {
                let r = DMat3::from_quat(b.orientation);
                let inverse = if b.inverse_mass() > 0.0 {
                    r * DMat3::from_diagonal(b.inertia.recip()) * r.transpose()
                } else {
                    DMat3::ZERO
                };
                Motion {
                    vel: b.vel,
                    omega: b.omega_world(),
                    inverse_mass: b.inverse_mass(),
                    inverse_inertia: inverse,
                }
            })
            .collect();
        let mut rows = Vec::new();
        for manifold in manifolds {
            let (ca, cb) = (
                self.colliders()[manifold.a.0 as usize],
                self.colliders()[manifold.b.0 as usize],
            );
            let (a, b) = (ca.body.0 as usize, cb.body.0 as usize);
            let friction = (ca.material.friction * cb.material.friction).sqrt();
            let torsional = (ca.material.torsional * cb.material.torsional).sqrt();
            let restitution = ca.material.restitution.max(cb.material.restitution);
            for c in &manifold.points {
                let ra = c.point - self.bodies()[a].pos;
                let rb = c.point - self.bodies()[b].pos;
                let (ma, mb) = (&motions[a], &motions[b]);
                let closing = (mb.point_velocity(rb) - ma.point_velocity(ra)).dot(c.normal);
                let target = if c.separation > 0.0 {
                    // Speculative: allow closing until the surfaces meet.
                    -c.separation / dt
                } else {
                    let push = settings.erp * (-c.separation - settings.slop).max(0.0) / dt;
                    let bounce = if closing < -settings.bounce_threshold {
                        -restitution * closing
                    } else {
                        0.0
                    };
                    push.max(bounce)
                };
                let tangents = basis(c.normal);
                let twist_k = c.normal.dot(ma.inverse_inertia * c.normal)
                    + c.normal.dot(mb.inverse_inertia * c.normal);
                rows.push(Row {
                    a,
                    b,
                    ra,
                    rb,
                    normal: c.normal,
                    tangents,
                    normal_mass: effective_mass(ma, mb, ra, rb, c.normal),
                    tangent_mass: [
                        effective_mass(ma, mb, ra, rb, tangents[0]),
                        effective_mass(ma, mb, ra, rb, tangents[1]),
                    ],
                    twist_mass: if twist_k > 0.0 { 1.0 / twist_k } else { 0.0 },
                    target,
                    friction,
                    torsional,
                    normal_impulse: 0.0,
                    tangent_impulse: [0.0; 2],
                    twist_impulse: 0.0,
                    report: ContactReport {
                        a: manifold.a,
                        b: manifold.b,
                        body_a: ca.body,
                        body_b: cb.body,
                        point: c.point,
                        normal: c.normal,
                        separation: c.separation,
                        impulse: DVec3::ZERO,
                        twist: DVec3::ZERO,
                    },
                });
            }
        }
        for _ in 0..settings.iterations {
            for row in &mut rows {
                let (a, b) = (row.a, row.b);
                // Friction first, bounded by the current normal impulse.
                let relative =
                    motions[b].point_velocity(row.rb) - motions[a].point_velocity(row.ra);
                let limit = row.friction * row.normal_impulse;
                let old = row.tangent_impulse;
                let mut next = [
                    old[0] - relative.dot(row.tangents[0]) * row.tangent_mass[0],
                    old[1] - relative.dot(row.tangents[1]) * row.tangent_mass[1],
                ];
                let size = next[0].hypot(next[1]);
                if size > limit {
                    let scale = if size > 0.0 { limit / size } else { 0.0 };
                    next = [next[0] * scale, next[1] * scale];
                }
                row.tangent_impulse = next;
                let delta =
                    row.tangents[0] * (next[0] - old[0]) + row.tangents[1] * (next[1] - old[1]);
                motions[a].push(-delta, row.ra);
                motions[b].push(delta, row.rb);
                if row.torsional > 0.0 {
                    let spin = (motions[b].omega - motions[a].omega).dot(row.normal);
                    let limit = row.torsional * row.normal_impulse;
                    let old = row.twist_impulse;
                    let next = (old - spin * row.twist_mass).clamp(-limit, limit);
                    row.twist_impulse = next;
                    let delta = row.normal * (next - old);
                    motions[a].twist(-delta);
                    motions[b].twist(delta);
                }
                let relative =
                    motions[b].point_velocity(row.rb) - motions[a].point_velocity(row.ra);
                let old = row.normal_impulse;
                let next =
                    (old + (row.target - relative.dot(row.normal)) * row.normal_mass).max(0.0);
                row.normal_impulse = next;
                let delta = row.normal * (next - old);
                motions[a].push(-delta, row.ra);
                motions[b].push(delta, row.rb);
            }
        }
        for (body, motion) in self.bodies_mut().iter_mut().zip(&motions) {
            if motion.inverse_mass > 0.0 {
                body.vel = motion.vel;
                body.omega = body.orientation.inverse() * motion.omega;
            }
        }
        rows.into_iter()
            .map(|row| ContactReport {
                impulse: row.normal * row.normal_impulse
                    + row.tangents[0] * row.tangent_impulse[0]
                    + row.tangents[1] * row.tangent_impulse[1],
                twist: row.normal * row.twist_impulse,
                ..row.report
            })
            .collect()
    }
}
