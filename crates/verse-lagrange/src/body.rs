//! Torque-free rigid bodies: Newton for the center of mass, Euler's equations
//! for rotation about principal axes, and quaternion attitude.

use glam::{DQuat, DVec3};

/// A rigid body in the station frame, SI units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigidBody {
    /// Mass, kg.
    pub mass: f64,
    /// Principal moments of inertia in the body frame, kg m^2.
    pub inertia: DVec3,
    /// Center of mass, m.
    pub pos: DVec3,
    /// Velocity, m/s.
    pub vel: DVec3,
    /// Body-to-station rotation.
    pub orientation: DQuat,
    /// Angular velocity in the body frame, rad/s.
    pub omega: DVec3,
}

impl RigidBody {
    /// A body at rest with the given mass properties.
    #[must_use]
    pub fn new(mass: f64, inertia: DVec3, pos: DVec3) -> Self {
        Self {
            mass,
            inertia,
            pos,
            vel: DVec3::ZERO,
            orientation: DQuat::IDENTITY,
            omega: DVec3::ZERO,
        }
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

    /// Angular momentum in the station frame, kg m^2/s.
    #[must_use]
    pub fn angular_momentum(&self) -> DVec3 {
        self.orientation * (self.inertia * self.omega)
    }

    /// Rotational kinetic energy, J.
    #[must_use]
    pub fn rotational_energy(&self) -> f64 {
        0.5 * (self.inertia * self.omega).dot(self.omega)
    }

    /// Advance rotation without torque. Euler's equations use RK4; attitude
    /// uses the exact exponential map of the step-averaged body rate.
    pub fn rotate(&mut self, dt: f64) {
        let i = self.inertia;
        let f = |w: DVec3| {
            DVec3::new(
                (i.y - i.z) * w.y * w.z / i.x,
                (i.z - i.x) * w.z * w.x / i.y,
                (i.x - i.y) * w.x * w.y / i.z,
            )
        };
        let w0 = self.omega;
        let k1 = f(w0);
        let k2 = f(w0 + k1 * (dt / 2.0));
        let k3 = f(w0 + k2 * (dt / 2.0));
        let k4 = f(w0 + k3 * dt);
        let w1 = w0 + (k1 + 2.0 * k2 + 2.0 * k3 + k4) * (dt / 6.0);
        let mid = w0 + k1 * (dt / 2.0) + (k2 - k1) * (dt / 4.0);
        let angle = mid.length() * dt;
        if angle > 0.0 {
            self.orientation =
                (self.orientation * DQuat::from_axis_angle(mid.normalize(), angle)).normalize();
        }
        self.omega = w1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_tumble_conserves_momentum_and_energy() {
        let size = DVec3::new(1.2, 1.2, 4.0);
        let mut body = RigidBody::new(180.0, RigidBody::box_inertia(180.0, size), DVec3::ZERO);
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
        let inertia = RigidBody::box_inertia(100.0, DVec3::new(3.0, 2.0, 1.0));
        let mut intermediate = RigidBody::new(100.0, inertia, DVec3::ZERO);
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
}
