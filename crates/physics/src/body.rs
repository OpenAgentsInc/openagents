//! Rigid bodies: Newton for the center of mass, Euler's equations for
//! rotation about principal axes, and quaternion attitude.

use glam::{DMat3, DQuat, DVec3, DVec4};
use serde::{Deserialize, Serialize};

/// How the world moves a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyKind {
    /// Integrated under the field and, later, forces, contacts, and joints.
    Dynamic,
    /// Never moves; infinite mass.
    Static,
    /// Moves only by its own velocity, which the owner sets; infinite mass.
    Kinematic,
}

/// A rigid body in the world frame, SI units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub kind: BodyKind,
    /// Mass, kg.
    pub mass: f64,
    /// Principal moments of inertia in the body frame, kg m^2.
    pub inertia: DVec3,
    /// Center of mass, m.
    pub pos: DVec3,
    /// Velocity, m/s.
    pub vel: DVec3,
    /// Body-to-world rotation.
    pub orientation: DQuat,
    /// Angular velocity in the body frame, rad/s.
    pub omega: DVec3,
    /// Pose before the last world step, for render interpolation.
    pub prev_pos: DVec3,
    pub prev_orientation: DQuat,
    /// Force accumulated for the next step, world frame, N. Cleared by the
    /// step.
    #[serde(default)]
    pub force: DVec3,
    /// Torque about the center of mass accumulated for the next step,
    /// world frame, N m. Cleared by the step.
    #[serde(default)]
    pub torque: DVec3,
    /// Asleep: at rest in a settled island, skipped by integration and
    /// treated as fixed until woken.
    #[serde(default)]
    pub sleeping: bool,
    /// How long the body has been slow enough to sleep, s.
    #[serde(default)]
    pub sleep_time: f64,
}

impl Body {
    /// A dynamic body at rest with the given mass properties.
    #[must_use]
    pub fn new(mass: f64, inertia: DVec3, pos: DVec3) -> Self {
        Self {
            kind: BodyKind::Dynamic,
            mass,
            inertia,
            pos,
            vel: DVec3::ZERO,
            orientation: DQuat::IDENTITY,
            omega: DVec3::ZERO,
            prev_pos: pos,
            prev_orientation: DQuat::IDENTITY,
            force: DVec3::ZERO,
            torque: DVec3::ZERO,
            sleeping: false,
            sleep_time: 0.0,
        }
    }

    /// This body with another kind.
    #[must_use]
    pub fn with_kind(mut self, kind: BodyKind) -> Self {
        self.kind = kind;
        self
    }

    /// Whether the world ever moves this body.
    #[must_use]
    pub fn moves(&self) -> bool {
        self.kind != BodyKind::Static
    }

    /// Whether contacts and joints can move this body: dynamic and awake.
    #[must_use]
    pub fn responds(&self) -> bool {
        self.kind == BodyKind::Dynamic && !self.sleeping
    }

    /// Wake the body and restart its sleep timer.
    pub fn wake(&mut self) {
        self.sleeping = false;
        self.sleep_time = 0.0;
    }

    /// Inverse mass seen by contacts and joints; zero unless dynamic and
    /// awake.
    #[must_use]
    pub fn inverse_mass(&self) -> f64 {
        if self.responds() && self.mass > 0.0 {
            1.0 / self.mass
        } else {
            0.0
        }
    }

    /// Angular velocity in the world frame, rad/s.
    #[must_use]
    pub fn omega_world(&self) -> DVec3 {
        self.orientation * self.omega
    }

    /// Linear momentum, kg m/s.
    #[must_use]
    pub fn momentum(&self) -> DVec3 {
        self.vel * self.mass
    }

    /// Inertia tensor about the center of mass in the world frame, kg m^2.
    #[must_use]
    pub fn inertia_world(&self) -> DMat3 {
        let r = DMat3::from_quat(self.orientation);
        r * DMat3::from_diagonal(self.inertia) * r.transpose()
    }

    /// Inverse inertia tensor in the world frame applied to `v`.
    #[must_use]
    pub fn inverse_inertia_world(&self, v: DVec3) -> DVec3 {
        if !self.responds() {
            return DVec3::ZERO;
        }
        let local = self.orientation.inverse() * v;
        self.orientation * (local / self.inertia)
    }

    /// Add a force, N, through the center of mass for the next step.
    pub fn apply_force(&mut self, force: DVec3) {
        self.wake();
        self.force += force;
    }

    /// Add a force, N, applied at world point `at` for the next step: the
    /// force plus its torque about the center of mass (Genesis
    /// `apply_links_external_wrench`).
    pub fn apply_force_at(&mut self, force: DVec3, at: DVec3) {
        self.wake();
        self.force += force;
        self.torque += (at - self.pos).cross(force);
    }

    /// Add a torque, N m, world frame, for the next step.
    pub fn apply_torque(&mut self, torque: DVec3) {
        self.wake();
        self.torque += torque;
    }

    /// Transform a body-frame point to the world frame.
    #[must_use]
    pub fn to_world(&self, local: DVec3) -> DVec3 {
        self.pos + self.orientation * local
    }

    /// Apply an impulse `impulse`, N s, at world point `at`. Only dynamic
    /// bodies respond.
    pub fn apply_impulse_at(&mut self, impulse: DVec3, at: DVec3) {
        if self.kind != BodyKind::Dynamic {
            return;
        }
        self.wake();
        self.vel += impulse * self.inverse_mass();
        self.apply_angular_impulse((at - self.pos).cross(impulse));
    }

    /// Apply an angular impulse, N m s, in the world frame.
    pub fn apply_angular_impulse(&mut self, impulse: DVec3) {
        if self.kind != BodyKind::Dynamic {
            return;
        }
        self.wake();
        self.omega += (self.orientation.inverse() * impulse) / self.inertia;
    }

    /// Pose between the previous and current step; `alpha` in [0, 1].
    #[must_use]
    pub fn interpolated(&self, alpha: f64) -> (DVec3, DQuat) {
        let alpha = alpha.clamp(0.0, 1.0);
        (
            self.prev_pos.lerp(self.pos, alpha),
            self.prev_orientation.slerp(self.orientation, alpha),
        )
    }

    /// Principal moments of a uniform box with full edge lengths `size`.
    #[must_use]
    pub fn box_inertia(mass: f64, size: DVec3) -> DVec3 {
        let s = size * size;
        DVec3::new(s.y + s.z, s.x + s.z, s.x + s.y) * (mass / 12.0)
    }

    /// Principal moments of a thin-walled cylinder along z.
    #[must_use]
    pub fn shell_inertia(mass: f64, radius: f64, length: f64) -> DVec3 {
        let across = mass * (radius * radius / 2.0 + length * length / 12.0);
        DVec3::new(across, across, mass * radius * radius)
    }

    /// Spin angular momentum about the center of mass, world frame, kg m^2/s.
    #[must_use]
    pub fn angular_momentum(&self) -> DVec3 {
        self.orientation * (self.inertia * self.omega)
    }

    /// Rotational kinetic energy, J.
    #[must_use]
    pub fn rotational_energy(&self) -> f64 {
        0.5 * (self.inertia * self.omega).dot(self.omega)
    }

    /// Advance rotation without torque. World-frame angular momentum is
    /// held exactly; attitude follows the body rate it implies, integrated
    /// with RK4 on the quaternion, and the body rate is recovered from the
    /// new attitude. Momentum is conserved to rounding; energy to the
    /// integrator's accuracy.
    pub fn rotate(&mut self, dt: f64) {
        if self.omega == DVec3::ZERO {
            return;
        }
        let inertia = self.inertia;
        let momentum = self.orientation * (inertia * self.omega);
        let rate = |q: DVec4| {
            let q = DQuat::from_vec4(q);
            let w = (q.inverse() * momentum) / inertia;
            DVec4::from(q * DQuat::from_xyzw(w.x, w.y, w.z, 0.0)) * 0.5
        };
        let q0 = DVec4::from(self.orientation);
        let k1 = rate(q0);
        let k2 = rate(q0 + k1 * (dt / 2.0));
        let k3 = rate(q0 + k2 * (dt / 2.0));
        let k4 = rate(q0 + k3 * dt);
        let q1 = q0 + (k1 + 2.0 * k2 + 2.0 * k3 + k4) * (dt / 6.0);
        self.orientation = DQuat::from_vec4(q1).normalize();
        self.omega = (self.orientation.inverse() * momentum) / inertia;
    }
}

/// Several bodies treated as one: total mass, center of mass, its velocity,
/// the inertia tensor about it, and the angular momentum about it. For
/// controlling a group held together by joints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Composite {
    pub mass: f64,
    pub com: DVec3,
    pub vel: DVec3,
    /// World frame, about `com`, kg m^2.
    pub inertia: DMat3,
    /// About `com`, kg m^2/s.
    pub angular_momentum: DVec3,
}

impl Composite {
    #[must_use]
    pub fn of<'a>(bodies: impl IntoIterator<Item = &'a Body> + Clone) -> Self {
        let mass: f64 = bodies.clone().into_iter().map(|b| b.mass).sum();
        let com = bodies
            .clone()
            .into_iter()
            .map(|b| b.pos * b.mass)
            .sum::<DVec3>()
            / mass;
        let vel = bodies
            .clone()
            .into_iter()
            .map(|b| b.vel * b.mass)
            .sum::<DVec3>()
            / mass;
        let mut inertia = DMat3::ZERO;
        let mut angular_momentum = DVec3::ZERO;
        for b in bodies {
            let d = b.pos - com;
            // Parallel-axis theorem.
            inertia += b.inertia_world()
                + (DMat3::IDENTITY * d.length_squared()
                    - DMat3::from_cols(d * d.x, d * d.y, d * d.z))
                    * b.mass;
            angular_momentum += b.angular_momentum() + d.cross((b.vel - vel) * b.mass);
        }
        Self {
            mass,
            com,
            vel,
            inertia,
            angular_momentum,
        }
    }

    /// Angular velocity of the group as if rigid, rad/s.
    #[must_use]
    pub fn omega(&self) -> DVec3 {
        self.inertia.inverse() * self.angular_momentum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_tumble_conserves_momentum_and_energy() {
        let size = DVec3::new(1.2, 1.2, 4.0);
        let mut body = Body::new(180.0, Body::box_inertia(180.0, size), DVec3::ZERO);
        body.omega = DVec3::new(0.3, 0.05, 0.2);
        let momentum = body.angular_momentum();
        let energy = body.rotational_energy();
        for _ in 0..(120 * 240) {
            body.rotate(1.0 / 240.0);
        }
        assert!((body.angular_momentum() - momentum).length() / momentum.length() < 1e-6);
        assert!((body.rotational_energy() - energy).abs() / energy < 1e-6);
    }

    #[test]
    fn intermediate_axis_spin_flips_but_major_axis_spin_is_stable() {
        // Distinct moments: x minor, y intermediate, z major.
        let inertia = Body::box_inertia(100.0, DVec3::new(3.0, 2.0, 1.0));
        let mut intermediate = Body::new(100.0, inertia, DVec3::ZERO);
        intermediate.omega = DVec3::new(1e-3, 1.0, 1e-3);
        let mut major = intermediate;
        major.omega = DVec3::new(1e-3, 1e-3, 1.0);
        let mut flipped = false;
        let mut major_drift: f64 = 0.0;
        for _ in 0..(60 * 240) {
            intermediate.rotate(1.0 / 240.0);
            major.rotate(1.0 / 240.0);
            flipped |= intermediate.omega.y < -0.9;
            major_drift = major_drift.max((major.omega.z - 1.0).abs());
        }
        assert!(flipped, "the Dzhanibekov flip appears");
        assert!(major_drift < 1e-3);
    }

    #[test]
    fn a_composite_of_two_bodies_uses_the_parallel_axis_theorem() {
        let a = Body::new(2.0, DVec3::splat(1.0), DVec3::new(-1.0, 0.0, 0.0));
        let mut b = Body::new(2.0, DVec3::splat(1.0), DVec3::new(1.0, 0.0, 0.0));
        b.vel = DVec3::new(0.0, 0.0, 2.0);
        let c = Composite::of([&a, &b]);
        assert_eq!(c.mass, 4.0);
        assert_eq!(c.com, DVec3::ZERO);
        assert_eq!(c.vel, DVec3::new(0.0, 0.0, 1.0));
        // About y: 1 + 1 + 2 * 2 * 1^2 = 6.
        assert!((c.inertia.y_axis.y - 6.0).abs() < 1e-12);
        // Each body carries 2 kg * 1 m/s at 1 m: 4 kg m^2/s about -y.
        assert!((c.omega() - DVec3::new(0.0, -4.0 / 6.0, 0.0)).length() < 1e-12);
    }
}
