//! Thrusters fixed to a body and the allocator that turns a wanted force
//! and torque into throttles, after the rotor mixer in Genesis
//! `examples/drone/quadcopter_controller.py`, generalized to any layout.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::body::Body;

/// A thruster fixed to a body.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thruster {
    /// Mounting point relative to the center of mass, body frame, m.
    pub pos: DVec3,
    /// Direction of the force on the body, body frame, unit length. The
    /// exhaust leaves the other way.
    pub dir: DVec3,
    /// Force at full throttle, N.
    pub max_force: f64,
}

/// The force and torque a set of throttles produces, body frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wrench {
    pub force: DVec3,
    pub torque: DVec3,
}

/// Thrusters on one body.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ThrusterSet {
    pub thrusters: Vec<Thruster>,
    /// Length that converts torque to force when the allocator weighs them
    /// against each other, m; about the thruster arm.
    pub arm: f64,
}

/// Coordinate-descent sweeps per allocation phase. Fixed, so the result is
/// deterministic.
const SWEEPS: usize = 256;
/// Linear fuel penalty in the first allocation phase, relative to the
/// largest thruster column's squared norm.
const FUEL_WEIGHT: f64 = 0.01;

impl ThrusterSet {
    /// Three thrusters at each corner of a box of half-extents `half`
    /// around the center of mass, each pushing inward along one axis: a
    /// 24-thruster layout like the Manned Maneuvering Unit's that gives
    /// pure force and pure torque about every axis.
    #[must_use]
    pub fn box_corners(half: DVec3, max_force: f64) -> Self {
        let mut thrusters = Vec::with_capacity(24);
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    let pos = DVec3::new(sx, sy, sz) * half;
                    for axis in [DVec3::X * sx, DVec3::Y * sy, DVec3::Z * sz] {
                        thrusters.push(Thruster {
                            pos,
                            dir: -axis,
                            max_force,
                        });
                    }
                }
            }
        }
        Self {
            thrusters,
            arm: half.length(),
        }
    }

    /// Body-frame wrench for `throttles` in [0, 1].
    #[must_use]
    pub fn wrench(&self, throttles: &[f64]) -> Wrench {
        self.thrusters
            .iter()
            .zip(throttles)
            .fold(Wrench::default(), |w, (t, u)| {
                let f = t.dir * (t.max_force * u);
                Wrench {
                    force: w.force + f,
                    torque: w.torque + t.pos.cross(f),
                }
            })
    }

    /// Throttles in [0, 1] whose wrench is closest to the wanted body-frame
    /// `force` and `torque`, with torque weighted by `arm` so both are in
    /// newtons. Infeasible requests saturate rather than fail.
    ///
    /// Two phases of projected coordinate descent, each a fixed number of
    /// sweeps so the result is deterministic. The first adds a linear fuel
    /// penalty, which drives thrusters that would fight each other to zero
    /// and leaves a sparse set; the second fits the request exactly on that
    /// set without the penalty's bias.
    #[must_use]
    pub fn allocate(&self, force: DVec3, torque: DVec3) -> Vec<f64> {
        let arm = if self.arm > 0.0 { self.arm } else { 1.0 };
        let columns: Vec<[DVec3; 2]> = self
            .thrusters
            .iter()
            .map(|t| {
                let f = t.dir * t.max_force;
                [f, t.pos.cross(f) / arm]
            })
            .collect();
        let biggest = columns
            .iter()
            .map(|c| c[0].length_squared() + c[1].length_squared())
            .fold(0.0, f64::max);
        let mut throttles = vec![0.0; columns.len()];
        let mut residual = [force, torque / arm];
        let mut active = vec![true; columns.len()];
        for penalty in [FUEL_WEIGHT * biggest, 0.0] {
            for _ in 0..SWEEPS {
                let mut moved: f64 = 0.0;
                for ((u, c), on) in throttles.iter_mut().zip(&columns).zip(&active) {
                    let norm = c[0].length_squared() + c[1].length_squared();
                    if !on || norm == 0.0 {
                        continue;
                    }
                    let gradient = c[0].dot(residual[0]) + c[1].dot(residual[1]) - penalty;
                    let next = (*u + gradient / norm).clamp(0.0, 1.0);
                    let change = next - *u;
                    if change != 0.0 {
                        residual[0] -= c[0] * change;
                        residual[1] -= c[1] * change;
                        *u = next;
                        moved = moved.max(change.abs());
                    }
                }
                // Converged; stopping on a fixed threshold stays deterministic.
                if moved < 1e-13 {
                    break;
                }
            }
            for (on, u) in active.iter_mut().zip(&throttles) {
                *on = *u > 0.0;
            }
        }
        throttles
    }

    /// Apply `throttles` to `body` as forces at the thruster mounts for the
    /// next world step. Returns the world-frame force at each mount, N,
    /// with the mount's world position, for exhaust and plume bookkeeping.
    pub fn apply(&self, body: &mut Body, throttles: &[f64]) -> Vec<(DVec3, DVec3)> {
        self.thrusters
            .iter()
            .zip(throttles)
            .filter(|(_, u)| **u > 0.0)
            .map(|(t, u)| {
                let at = body.to_world(t.pos);
                let force = body.orientation * (t.dir * (t.max_force * u));
                body.apply_force_at(force, at);
                (force, at)
            })
            .collect()
    }
}

/// A proportional-integral-derivative controller over vectors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pid {
    pub kp: f64,
    pub ki: f64,
    pub kd: f64,
    pub integral: DVec3,
    pub prev_error: Option<DVec3>,
}

impl Pid {
    #[must_use]
    pub fn new(kp: f64, ki: f64, kd: f64) -> Self {
        Self {
            kp,
            ki,
            kd,
            ..Self::default()
        }
    }

    /// Output for `error` after `dt` seconds. The first update has no
    /// derivative term.
    pub fn update(&mut self, error: DVec3, dt: f64) -> DVec3 {
        self.integral += error * dt;
        let derivative = self
            .prev_error
            .map_or(DVec3::ZERO, |prev| (error - prev) / dt);
        self.prev_error = Some(error);
        error * self.kp + self.integral * self.ki + derivative * self.kd
    }

    pub fn reset(&mut self) {
        self.integral = DVec3::ZERO;
        self.prev_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{NoField, World};
    use glam::DQuat;

    fn pack() -> ThrusterSet {
        ThrusterSet::box_corners(DVec3::new(0.35, 0.45, 0.3), 10.0)
    }

    #[test]
    fn one_off_center_thruster_pushes_and_turns() {
        let mut world = World::new(0.01);
        let mut body = Body::new(
            100.0,
            DVec3::new(10.0, 20.0, 30.0),
            DVec3::new(1.0, 2.0, 3.0),
        );
        body.orientation = DQuat::from_rotation_y(0.5);
        let id = world.add(body);
        let set = ThrusterSet {
            thrusters: vec![Thruster {
                pos: DVec3::new(0.0, 0.5, 0.0),
                dir: DVec3::X,
                max_force: 10.0,
            }],
            arm: 0.5,
        };
        let applied = set.apply(&mut world[id], &[1.0]);
        let force = applied[0].0;
        assert!(
            (force - DQuat::from_rotation_y(0.5) * DVec3::new(10.0, 0.0, 0.0)).length() < 1e-12
        );
        world.step(&NoField);
        let body = world[id];
        // a = F / m; body-frame torque r x F = (0, 0, -5) N m about z.
        assert!((body.vel - force / 100.0 * 0.01).length() < 1e-12);
        let spin = body.orientation.inverse() * body.omega_world();
        assert!(
            (spin - DVec3::new(0.0, 0.0, -5.0 / 30.0 * 0.01)).length() < 1e-9,
            "{spin}"
        );
        assert_eq!(body.force, DVec3::ZERO, "accumulators clear");
    }

    #[test]
    fn pure_force_and_pure_torque_requests_are_met() {
        let set = pack();
        for force in [
            DVec3::new(40.0, 0.0, 0.0),
            DVec3::new(-10.0, 25.0, 5.0),
            DVec3::new(0.0, 0.0, -30.0),
        ] {
            let u = set.allocate(force, DVec3::ZERO);
            let w = set.wrench(&u);
            assert!((w.force - force).length() < 1e-4, "{force}: {:?}", w);
            assert!(w.torque.length() < 1e-4, "{force}: {:?}", w);
            assert!(u.iter().all(|u| (0.0..=1.0).contains(u)));
            // No thruster fights another: fuel equals the least possible.
            let fuel: f64 = u.iter().sum::<f64>() * 10.0;
            let least = force.abs().element_sum();
            assert!(
                (fuel - least).abs() < 1e-3,
                "{force}: fuel {fuel} vs {least}"
            );
        }
        let torque = DVec3::new(0.0, 4.0, -2.0);
        let w = set.wrench(&set.allocate(DVec3::ZERO, torque));
        assert!(
            (w.torque - torque).length() < 1e-6 && w.force.length() < 1e-6,
            "{w:?}"
        );
    }

    #[test]
    fn an_infeasible_request_saturates_toward_it() {
        let set = pack();
        let wanted = DVec3::new(500.0, 0.0, 0.0);
        let u = set.allocate(wanted, DVec3::ZERO);
        let w = set.wrench(&u);
        assert!(u.iter().all(|u| (0.0..=1.0).contains(u)));
        assert!(
            (w.force.x - 40.0).abs() < 1e-6,
            "four thrusters at full: {w:?}"
        );
        assert!(w.torque.length() < 1e-4);
        assert_eq!(u, set.allocate(wanted, DVec3::ZERO), "deterministic");
    }

    #[test]
    fn pid_terms_add() {
        let mut pid = Pid::new(2.0, 1.0, 0.5);
        let first = pid.update(DVec3::X, 0.1);
        assert!((first - DVec3::X * (2.0 + 0.1)).length() < 1e-12);
        let second = pid.update(DVec3::X * 2.0, 0.1);
        assert!((second - DVec3::X * (4.0 + 0.3 + 5.0)).length() < 1e-12);
    }
}
