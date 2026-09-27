//! Sensors and debug primitives, after Genesis `sensors/contact_force_go2.py`,
//! `sensors/imu_franka.py`, `sensors/lidar_teleop.py`, and
//! `tutorials/draw_debug.py`: a raycast against colliders, per-body contact
//! force from the last step, an inertial measurement unit, and lines that
//! show contacts and joints.

use glam::{DMat3, DVec3};
use serde::{Deserialize, Serialize};

use crate::body::Body;
use crate::collision::{Collider, ColliderId, Shape};
use crate::world::{BodyId, World};

/// Where a ray first meets a collider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub collider: ColliderId,
    pub body: BodyId,
    /// Distance along the unit ray direction, m.
    pub distance: f64,
    pub point: DVec3,
    /// Outward surface normal at the hit.
    pub normal: DVec3,
}

/// Smallest `t` in `[0, max]` where `origin + dir t` enters a sphere, with
/// the outward normal.
fn ray_sphere(
    origin: DVec3,
    dir: DVec3,
    center: DVec3,
    radius: f64,
    max: f64,
) -> Option<(f64, DVec3)> {
    let m = origin - center;
    let b = m.dot(dir);
    let c = m.length_squared() - radius * radius;
    if c <= 0.0 {
        // Starts inside: hit at the origin, facing back along the ray.
        return Some((0.0, -dir));
    }
    if b > 0.0 {
        return None;
    }
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    (t <= max).then(|| (t, (origin + dir * t - center) / radius))
}

fn ray_capsule(
    origin: DVec3,
    dir: DVec3,
    p: DVec3,
    q: DVec3,
    radius: f64,
    max: f64,
) -> Option<(f64, DVec3)> {
    let axis = q - p;
    let length = axis.length();
    let mut best = [p, q]
        .into_iter()
        .filter_map(|c| ray_sphere(origin, dir, c, radius, max))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if length > 1e-12 {
        // The cylinder between the end caps.
        let a = axis / length;
        let (d, m) = (dir - a * dir.dot(a), (origin - p) - a * (origin - p).dot(a));
        let (qa, qb, qc) = (
            d.length_squared(),
            2.0 * d.dot(m),
            m.length_squared() - radius * radius,
        );
        if qa > 1e-12 {
            let disc = qb * qb - 4.0 * qa * qc;
            if disc >= 0.0 {
                let t = (-qb - disc.sqrt()) / (2.0 * qa);
                let x = origin + dir * t;
                let along = (x - p).dot(a);
                if (0.0..=max).contains(&t)
                    && (0.0..=length).contains(&along)
                    && best.is_none_or(|b| t < b.0)
                {
                    let core = p + a * along;
                    best = Some((t, (x - core) / radius));
                }
            }
        }
    }
    best
}

fn ray_box(
    origin: DVec3,
    dir: DVec3,
    center: DVec3,
    axes: DMat3,
    half: DVec3,
    max: f64,
) -> Option<(f64, DVec3)> {
    let o = axes.transpose() * (origin - center);
    let d = axes.transpose() * dir;
    let (mut near, mut far) = (0.0_f64, max);
    let mut normal = DVec3::ZERO;
    for i in 0..3 {
        if d[i].abs() < 1e-15 {
            if o[i].abs() > half[i] {
                return None;
            }
            continue;
        }
        let (t1, t2) = ((-half[i] - o[i]) / d[i], (half[i] - o[i]) / d[i]);
        let (lo, hi) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
        if lo > near {
            near = lo;
            let mut n = DVec3::ZERO;
            n[i] = -d[i].signum();
            normal = n;
        }
        far = far.min(hi);
        if near > far {
            return None;
        }
    }
    if normal == DVec3::ZERO {
        // Starts inside.
        return Some((0.0, -dir));
    }
    Some((near, axes * normal))
}

impl World {
    /// The first collider a ray from `origin` along `dir` meets within
    /// `max` meters, among colliders `accept` allows.
    #[must_use]
    pub fn raycast(
        &self,
        origin: DVec3,
        dir: DVec3,
        max: f64,
        accept: &dyn Fn(&Collider) -> bool,
    ) -> Option<RayHit> {
        let dir = dir.try_normalize()?;
        let mut best: Option<RayHit> = None;
        for (i, c) in self.colliders().iter().enumerate() {
            if !accept(c) {
                continue;
            }
            let (pos, rotation) = c.pose(self);
            let limit = best.map_or(max, |b| b.distance);
            let hit = match c.shape {
                Shape::Sphere { radius } => ray_sphere(origin, dir, pos, radius, limit),
                Shape::Capsule {
                    radius,
                    half_length,
                } => {
                    let axis = rotation * DVec3::Z * half_length;
                    ray_capsule(origin, dir, pos - axis, pos + axis, radius, limit)
                }
                Shape::Cuboid { half } => {
                    ray_box(origin, dir, pos, DMat3::from_quat(rotation), half, limit)
                }
            };
            if let Some((t, normal)) = hit
                && t <= limit
            {
                best = Some(RayHit {
                    collider: ColliderId(i as u32),
                    body: c.body,
                    distance: t,
                    point: origin + dir * t,
                    normal,
                });
            }
        }
        best
    }

    /// Force and torque about its center of mass that contacts applied to
    /// `body` in the last step, N and N m.
    #[must_use]
    pub fn contact_force(&self, body: BodyId) -> (DVec3, DVec3) {
        let center = self[body].pos;
        let mut force = DVec3::ZERO;
        let mut torque = DVec3::ZERO;
        for c in &self.contacts {
            let sign = if c.body_b == body {
                1.0
            } else if c.body_a == body {
                -1.0
            } else {
                continue;
            };
            force += c.impulse * sign;
            torque += (c.point - center).cross(c.impulse * sign) + c.twist * sign;
        }
        (force / self.dt, torque / self.dt)
    }

    /// Lines showing the last step's contacts and the joints: each contact
    /// point with its normal, the impulse it carried, and each joint's
    /// anchors.
    #[must_use]
    pub fn debug_lines(&self) -> Vec<DebugLine> {
        let mut lines = Vec::new();
        for c in &self.contacts {
            lines.push(DebugLine {
                from: c.point,
                to: c.point + c.normal * 0.3,
                kind: DebugKind::ContactNormal,
            });
            if c.impulse != DVec3::ZERO {
                // Scaled so a 1 N s impulse draws 10 cm.
                lines.push(DebugLine {
                    from: c.point,
                    to: c.point + c.impulse * 0.1,
                    kind: DebugKind::ContactImpulse,
                });
            }
        }
        for (_, joint) in self.joints() {
            let (a, b) = joint.anchors(self);
            lines.push(DebugLine {
                from: self[joint.a].pos,
                to: a,
                kind: DebugKind::Joint,
            });
            lines.push(DebugLine {
                from: a,
                to: b,
                kind: if joint.saturated {
                    DebugKind::Strained
                } else {
                    DebugKind::Joint
                },
            });
            lines.push(DebugLine {
                from: b,
                to: self[joint.b].pos,
                kind: DebugKind::Joint,
            });
        }
        lines
    }
}

/// What a debug line shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugKind {
    ContactNormal,
    ContactImpulse,
    Joint,
    /// A joint at its force or torque limit.
    Strained,
    /// A thruster's force, drawn by its owner.
    Thrust,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DebugLine {
    pub from: DVec3,
    pub to: DVec3,
    pub kind: DebugKind,
}

/// One inertial reading, body frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImuReading {
    /// Proper acceleration: what an accelerometer feels, the acceleration
    /// minus the field's, m/s^2. Zero in free fall.
    pub specific_force: DVec3,
    /// Angular rate, rad/s.
    pub angular_rate: DVec3,
}

/// An inertial measurement unit fixed at a body's center of mass. It
/// differences velocity between readings, so read it once per step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Imu {
    last_vel: Option<DVec3>,
    pub reading: ImuReading,
}

impl Imu {
    /// Read after a step of `dt` in which the field accelerated the body by
    /// `field`.
    pub fn read(&mut self, body: &Body, field: DVec3, dt: f64) -> ImuReading {
        let accel = self.last_vel.map_or(DVec3::ZERO, |v| (body.vel - v) / dt);
        let specific = if self.last_vel.is_some() {
            accel - field
        } else {
            DVec3::ZERO
        };
        self.last_vel = Some(body.vel);
        self.reading = ImuReading {
            specific_force: body.orientation.inverse() * specific,
            angular_rate: body.omega,
        };
        self.reading
    }
}
