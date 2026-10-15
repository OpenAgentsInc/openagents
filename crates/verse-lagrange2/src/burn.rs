//! The finished ship's test burn: a rigid body whose mass properties come
//! from its parts as the tanks drain, a gimballed engine that tracks the
//! center of mass, and an attitude hold, integrated momentum-exactly.
//!
//! The burn lasts a couple of minutes, during which the Earth–Moon field
//! changes the ship's velocity by far less than a millimeter per second, so
//! it runs in the station's local frame; its velocity change is then handed
//! to the three-body integrator (`verse_lagrange::earth_moon`) to show the
//! ship leaving L2.
//!
//! Each step drains `dm = mdot dt` and changes the ship's velocity by
//! `dv = v_e dm / (m - dm / 2)` along the thrust: the exhaust leaves at
//! `v_e` relative to the ship's mean velocity over the step, so momentum
//! balances exactly against the exhaust, and the sum of the steps matches
//! the rocket equation `v_e ln(m0 / mf)` to the midpoint rule's
//! second-order error.

use glam::{DMat3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::ship::{G0, PartKind, Spacecraft};

/// Burn step, s.
pub const DT: f64 = 1.0 / 30.0;
/// Time constant of the gimbal's estimate of the center of mass, s: the
/// propellant gauges and the gimbal actuators lag the real drain.
pub const GIMBAL_LAG: f64 = 1.0;
/// Reaction-control torque limit, N m.
pub const RCS_TORQUE: f64 = 4_000.0;

/// The ship during its burn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Burn {
    pub ship: Spacecraft,
    /// Propellant left in each part (zero for non-tanks), kg.
    pub propellant: Vec<f64>,
    /// Center of mass in the local frame, m, and its velocity, m/s.
    pub pos: DVec3,
    pub vel: DVec3,
    pub orientation: DQuat,
    /// Angular momentum about the center of mass, local frame.
    pub spin: DVec3,
    /// The gimbal's estimate of the center of mass, ship frame.
    pub com_estimate: DVec3,
    /// Velocity change still to fly, m/s.
    pub target_dv: f64,
    /// Scalar sum of the velocity changes so far, m/s.
    pub flown: f64,
    pub time: f64,
    /// Momentum carried off by the exhaust, local frame.
    pub exhaust: DVec3,
    /// Angular impulse from the reaction-control thrusters.
    pub rcs: DVec3,
    /// Largest body rate seen, rad/s.
    pub peak_rate: f64,
    /// Current thrust direction, ship frame.
    pub thrust_dir: DVec3,
    pub firing: bool,
    pub initial_mass: f64,
}

impl Burn {
    /// The ship at rest at the origin, ready to fly `dv` m/s forward.
    #[must_use]
    pub fn new(ship: Spacecraft, dv: f64) -> Self {
        let propellant = ship
            .parts
            .iter()
            .map(crate::ship::Part::propellant)
            .collect();
        let mass = ship.mass_properties();
        Self {
            com_estimate: mass.com,
            initial_mass: mass.mass,
            ship,
            propellant,
            pos: DVec3::ZERO,
            vel: DVec3::ZERO,
            orientation: DQuat::IDENTITY,
            spin: DVec3::ZERO,
            target_dv: dv,
            flown: 0.0,
            time: 0.0,
            exhaust: DVec3::ZERO,
            rcs: DVec3::ZERO,
            peak_rate: 0.0,
            thrust_dir: DVec3::X,
            firing: true,
        }
    }

    #[must_use]
    pub fn mass(&self) -> f64 {
        self.ship
            .parts
            .iter()
            .zip(&self.propellant)
            .map(|(p, prop)| p.dry_mass_kg + prop)
            .sum()
    }

    /// Center of mass, ship frame, and inertia about it, ship frame.
    #[must_use]
    pub fn mass_properties(&self) -> crate::ship::MassProperties {
        self.ship.mass_properties_with(|p| {
            let i = self
                .ship
                .parts
                .iter()
                .position(|q| q.id == p.id)
                .unwrap_or(0);
            p.dry_mass_kg + self.propellant[i]
        })
    }

    fn engine(&self) -> (DVec3, f64, f64) {
        self.ship
            .parts
            .iter()
            .find_map(|p| match p.kind {
                PartKind::Engine { thrust_n, isp_s } => Some((p.position, thrust_n, isp_s)),
                _ => None,
            })
            .unwrap_or((DVec3::ZERO, 0.0, 1.0))
    }

    /// Body rate, local frame, rad/s.
    #[must_use]
    pub fn omega(&self) -> DVec3 {
        let mp = self.mass_properties();
        let r = DMat3::from_quat(self.orientation);
        (r * mp.inertia * r.transpose()).inverse() * self.spin
    }

    /// Momentum of the ship plus everything its exhaust carried away.
    #[must_use]
    pub fn system_momentum(&self) -> DVec3 {
        self.vel * self.mass() + self.exhaust
    }

    /// One step.
    pub fn step(&mut self) {
        let dt = DT;
        let (pivot, thrust, isp) = self.engine();
        let ve = isp * G0;
        let mp = self.mass_properties();
        // The gimbal aims through its estimate of the center of mass.
        let k = dt / (GIMBAL_LAG + dt);
        self.com_estimate += (mp.com - self.com_estimate) * k;
        let aim = (self.com_estimate - pivot).normalize_or(DVec3::X);
        self.thrust_dir = aim;
        let r = DMat3::from_quat(self.orientation);
        let mut torque = DVec3::ZERO;
        let left: f64 = self.propellant.iter().sum();
        if self.firing && left > 0.0 {
            let mass = mp.mass;
            let want = self.target_dv - self.flown;
            // The last step drains only what the remaining velocity needs.
            let mut dm = (thrust / ve * dt).min(left);
            let full = ve * dm / (mass - dm / 2.0);
            if full > want {
                // Solve ve dm / (m - dm/2) = want for dm.
                dm = want * mass / (ve + want / 2.0);
            }
            let dv = ve * dm / (mass - dm / 2.0);
            let dir = self.orientation * aim;
            let v_mid = self.vel + dir * dv / 2.0;
            self.exhaust += (v_mid - dir * ve) * dm;
            self.vel += dir * dv;
            self.flown += dv;
            // Drain the tanks in proportion to what they hold.
            for p in &mut self.propellant {
                *p -= dm * *p / left;
            }
            // The thrust's moment about the true center of mass.
            let force = dir * (dm * ve / dt);
            torque += (r * (pivot - mp.com)).cross(force);
            if self.target_dv - self.flown <= 1e-9 {
                self.firing = false;
            }
        }
        // Attitude hold on reaction-control thrusters.
        let mut error = self.orientation;
        if error.w < 0.0 {
            error = -error;
        }
        let (axis, angle) = error.to_axis_angle();
        let inertia = r * mp.inertia * r.transpose();
        let omega = inertia.inverse() * self.spin;
        let rcs = (-(axis * angle) * (inertia * DVec3::splat(0.25)).length()
            - omega * (inertia * DVec3::splat(1.0)).length())
        .clamp_length_max(RCS_TORQUE);
        self.rcs += rcs * dt;
        self.spin += (torque + rcs) * dt;
        let omega = inertia.inverse() * self.spin;
        self.peak_rate = self.peak_rate.max(omega.length());
        let turn = omega * dt;
        if turn.length_squared() > 0.0 {
            self.orientation = (DQuat::from_scaled_axis(turn) * self.orientation).normalize();
        }
        self.pos += self.vel * dt;
        self.time += dt;
    }

    /// Fly the whole burn and coast `coast` seconds after it.
    pub fn run(&mut self, coast: f64) {
        while self.firing {
            self.step();
        }
        let end = self.time + coast;
        while self.time < end {
            self.step();
        }
    }

    /// The rocket equation's velocity change for the mass burned so far.
    #[must_use]
    pub fn rocket_equation(&self) -> f64 {
        let (_, _, isp) = self.engine();
        isp * G0 * (self.initial_mass / self.mass()).ln()
    }
}
