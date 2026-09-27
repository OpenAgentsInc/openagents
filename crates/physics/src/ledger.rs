//! Momentum bookkeeping, after Genesis `examples/ipc/ipc_momentum.py`: sum
//! a system's momentum every step and check it against the start plus every
//! external impulse. A closed system that changes its own momentum has a bug.

use std::collections::BTreeMap;
use std::ops::{Add, AddAssign, Sub};

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::body::Body;
use crate::world::World;

/// Linear momentum and angular momentum about a fixed origin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Momentum {
    /// kg m/s.
    pub linear: DVec3,
    /// Orbital plus spin angular momentum, kg m^2/s.
    pub angular: DVec3,
}

impl Momentum {
    pub const ZERO: Self = Self {
        linear: DVec3::ZERO,
        angular: DVec3::ZERO,
    };

    /// A body's momentum about `origin`, counting its full mass. Callers
    /// decide which bodies belong to the system.
    #[must_use]
    pub fn of(body: &Body, origin: DVec3) -> Self {
        let linear = body.momentum();
        Self {
            linear,
            angular: (body.pos - origin).cross(linear) + body.angular_momentum(),
        }
    }

    /// An impulse `impulse` applied at `at`, about `origin`.
    #[must_use]
    pub fn impulse(impulse: DVec3, at: DVec3, origin: DVec3) -> Self {
        Self {
            linear: impulse,
            angular: (at - origin).cross(impulse),
        }
    }
}

impl Add for Momentum {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            linear: self.linear + other.linear,
            angular: self.angular + other.angular,
        }
    }
}

impl AddAssign for Momentum {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl Sub for Momentum {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            linear: self.linear - other.linear,
            angular: self.angular - other.angular,
        }
    }
}

impl World {
    /// Momentum of every non-static body about `origin`.
    #[must_use]
    pub fn momentum(&self, origin: DVec3) -> Momentum {
        self.bodies()
            .iter()
            .filter(|b| b.moves())
            .fold(Momentum::ZERO, |sum, b| sum + Momentum::of(b, origin))
    }
}

/// Relative ledger error: residual size over the largest momentum involved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedgerError {
    pub linear: f64,
    pub angular: f64,
}

/// A system's starting momentum and the named external impulses it has
/// received since, such as thruster exhaust or contact with fixed structure.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    pub origin: DVec3,
    pub start: Momentum,
    pub external: BTreeMap<String, Momentum>,
}

impl Ledger {
    #[must_use]
    pub fn new(origin: DVec3, start: Momentum) -> Self {
        Self {
            origin,
            start,
            external: BTreeMap::new(),
        }
    }

    /// Record an external impulse `impulse` applied at `at` under `term`.
    pub fn add_impulse(&mut self, term: &str, impulse: DVec3, at: DVec3) {
        self.add(term, Momentum::impulse(impulse, at, self.origin));
    }

    /// Record external momentum under `term`.
    pub fn add(&mut self, term: &str, momentum: Momentum) {
        *self.external.entry(term.to_owned()).or_default() += momentum;
    }

    /// Sum of every external term.
    #[must_use]
    pub fn external_total(&self) -> Momentum {
        self.external
            .values()
            .fold(Momentum::ZERO, |sum, m| sum + *m)
    }

    /// What `now` has that the start and the external terms do not explain.
    #[must_use]
    pub fn residual(&self, now: Momentum) -> Momentum {
        now - (self.start + self.external_total())
    }

    /// The residual relative to the largest momentum in play. A floor of
    /// 1e-12 keeps a system at rest from dividing by zero.
    #[must_use]
    pub fn error(&self, now: Momentum) -> LedgerError {
        let residual = self.residual(now);
        let scale = |f: fn(&Momentum) -> DVec3| {
            let terms = self.external.values().map(|m| f(m).length()).sum::<f64>();
            (f(&self.start).length() + terms)
                .max(f(&now).length())
                .max(1e-12)
        };
        LedgerError {
            linear: residual.linear.length() / scale(|m| m.linear),
            angular: residual.angular.length() / scale(|m| m.angular),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::BodyKind;
    use crate::world::NoField;

    fn tumbling_pair() -> World {
        let mut world = World::new(1.0 / 120.0);
        let size = DVec3::new(1.2, 2.0, 4.0);
        let mut truss = Body::new(
            180.0,
            Body::box_inertia(180.0, size),
            DVec3::new(3.0, 1.0, -2.0),
        );
        truss.vel = DVec3::new(0.4, -0.1, 0.25);
        truss.omega = DVec3::new(0.3, 0.05, -0.2);
        world.add(truss);
        let mut tank = Body::new(320.0, Body::shell_inertia(320.0, 1.3, 3.6), DVec3::ZERO);
        tank.vel = DVec3::new(-0.2, 0.0, 0.1);
        tank.omega = DVec3::new(0.0, 0.8, 0.02);
        world.add(tank);
        world.add(
            Body::new(1e6, DVec3::ONE, DVec3::new(50.0, 0.0, 0.0)).with_kind(BodyKind::Static),
        );
        world
    }

    #[test]
    fn a_coasting_system_keeps_its_momentum() {
        let mut world = tumbling_pair();
        let origin = DVec3::new(1.0, 2.0, 3.0);
        let ledger = Ledger::new(origin, world.momentum(origin));
        let mut worst = LedgerError {
            linear: 0.0,
            angular: 0.0,
        };
        for _ in 0..(120 * 20) {
            world.step(&NoField);
            let error = ledger.error(world.momentum(origin));
            worst.linear = worst.linear.max(error.linear);
            worst.angular = worst.angular.max(error.angular);
        }
        assert!(worst.linear < 1e-12, "{worst:?}");
        assert!(worst.angular < 1e-12, "{worst:?}");
    }

    #[test]
    fn recorded_impulses_balance_and_unrecorded_ones_show() {
        let mut world = tumbling_pair();
        let origin = DVec3::ZERO;
        let mut ledger = Ledger::new(origin, world.momentum(origin));
        let kick = DVec3::new(30.0, -5.0, 12.0);
        let at = world.bodies()[0].pos + DVec3::new(0.0, 0.9, 1.5);
        world.bodies_mut()[0].apply_impulse_at(kick, at);
        let before = ledger.error(world.momentum(origin));
        assert!(before.linear > 1e-2 && before.angular > 1e-2, "{before:?}");
        ledger.add_impulse("kick", kick, at);
        let after = ledger.error(world.momentum(origin));
        assert!(after.linear < 1e-12 && after.angular < 1e-12, "{after:?}");
        for _ in 0..600 {
            world.step(&NoField);
        }
        let later = ledger.error(world.momentum(origin));
        assert!(later.linear < 1e-12 && later.angular < 1e-12, "{later:?}");
    }
}
