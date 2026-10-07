//! Water's forces on the bodies in a world: buoyancy, drag relative to the
//! current, and angular damping.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use super::set::Water;
use super::submerge::{submerged, volume};
use crate::ledger::{Ledger, Momentum};
use crate::world::{BodyId, World};

/// Drag and damping, and the gravity buoyancy opposes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// m/s². Buoyancy pushes against it.
    pub gravity: DVec3,
    /// Linear drag per wetted area, N s/m³: skin friction plus the energy
    /// a bobbing body radiates as waves.
    pub linear: f64,
    /// Quadratic drag coefficient `C_d` on the projected area, taken as a
    /// quarter of the wetted area (Cauchy's mean projection for a convex
    /// body).
    pub quadratic: f64,
    /// Angular damping at full wetting, 1/s.
    pub angular: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gravity: DVec3::new(0.0, -9.81, 0.0),
            linear: 60.0,
            quadratic: 1.0,
            angular: 1.5,
        }
    }
}

/// The ledger term each force enters under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Term {
    Buoyancy,
    Drag,
}

impl Term {
    /// The term's name in a [`Ledger`].
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Buoyancy => "water.buoyancy",
            Self::Drag => "water.drag",
        }
    }
}

/// One force water put on a body for the next step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Push {
    pub body: BodyId,
    pub term: Term,
    /// N, world frame.
    pub force: DVec3,
    /// Where the force acts, m.
    pub at: DVec3,
    /// A pure couple added besides the force's own moment, N m.
    pub couple: DVec3,
}

impl Push {
    /// The momentum this push adds over a step of `dt` about `origin`.
    #[must_use]
    pub fn momentum(&self, dt: f64, origin: DVec3) -> Momentum {
        let mut m = Momentum::impulse(self.force * dt, self.at, origin);
        m.angular += self.couple * dt;
        m
    }
}

/// Enter `pushes` into `ledger` under their terms, for a step of `dt`.
pub fn record(pushes: &[Push], ledger: &mut Ledger, dt: f64) {
    for push in pushes {
        let m = push.momentum(dt, ledger.origin);
        ledger.add(push.term.name(), m);
    }
}

/// Add water's forces to every awake dynamic body with a collider in
/// `water` at `tick`, with default [`Settings`], for the next step of `dt`.
pub fn apply<W: Water + ?Sized>(world: &mut World, water: &W, tick: u64, dt: f64) -> Vec<Push> {
    apply_with(world, water, tick, dt, &Settings::default())
}

/// As [`apply`], with `settings`.
pub fn apply_with<W: Water + ?Sized>(
    world: &mut World,
    water: &W,
    tick: u64,
    dt: f64,
    settings: &Settings,
) -> Vec<Push> {
    apply_where(world, water, tick, dt, settings, |_| true)
}

/// As [`apply_with`], only for the bodies `which` accepts.
pub fn apply_where<W: Water + ?Sized>(
    world: &mut World,
    water: &W,
    tick: u64,
    dt: f64,
    settings: &Settings,
    which: impl Fn(BodyId) -> bool,
) -> Vec<Push> {
    apply_scaled(world, water, tick, dt, settings, |id| {
        which(id).then_some(1.0)
    })
}

/// As [`apply_where`], with each accepted body's buoyancy scaled by the
/// factor `which` gives it. A game body whose mass was tuned for how it
/// moves rather than what it is made of floats as its material does when
/// the factor is its mass over its material's density times its collider
/// volume: buoyancy then holds it at the material's own draft.
pub fn apply_scaled<W: Water + ?Sized>(
    world: &mut World,
    water: &W,
    tick: u64,
    dt: f64,
    settings: &Settings,
    which: impl Fn(BodyId) -> Option<f64>,
) -> Vec<Push> {
    let mut pushes = Vec::new();
    let g = settings.gravity.length();
    let up = if g > 0.0 {
        -settings.gravity / g
    } else {
        DVec3::Y
    };
    // Each body's collider volume, to share its damping among colliders.
    let mut volumes = vec![0.0; world.bodies().len()];
    for c in world.colliders() {
        volumes[c.body.0 as usize] += volume(&c.shape);
    }
    // Per body: touched by water, and whether all of it was still.
    let mut wet = vec![(false, true); world.bodies().len()];
    for index in 0..world.colliders().len() {
        let collider = world.colliders()[index];
        let id = collider.body;
        let body = world[id];
        if !body.responds() {
            continue;
        }
        let Some(lift) = which(id).filter(|k| k.is_finite() && *k >= 0.0) else {
            continue;
        };
        let pose = collider.pose(world);
        let sub = submerged(&collider.shape, pose, water, tick);
        let Some(sample) = sub.water else {
            continue;
        };
        if sub.volume <= 0.0 {
            continue;
        }
        let slot = &mut wet[id.0 as usize];
        slot.0 = true;
        slot.1 &= sample.still();
        let rho = sample.density;
        let mut out = [Push {
            body: id,
            term: Term::Buoyancy,
            force: up * (rho * g * sub.volume * lift),
            at: sub.centroid,
            couple: DVec3::ZERO,
        }; 2];
        // Drag on the velocity relative to the water at the center of
        // buoyancy.
        let arm = sub.centroid - body.pos;
        let relative = body.vel + body.omega_world().cross(arm) - sample.velocity();
        let speed = relative.length();
        let area = sub.wetted_area;
        let mut drag = -relative
            * (settings.linear * area + 0.5 * rho * settings.quadratic * 0.25 * area * speed);
        let share = if volumes[id.0 as usize] > 0.0 {
            volume(&collider.shape) / volumes[id.0 as usize]
        } else {
            1.0
        };
        // Never reverse the relative motion within one step: the most this
        // collider may push is its share of the body's effective mass at
        // the point, along the drag, so the colliders of one body together
        // never overshoot, nor does the spin an off-center push gives.
        let size = drag.length();
        if size > 0.0 {
            let n = drag / size;
            let rn = arm.cross(n);
            let inverse = body.inverse_mass() + rn.dot(body.inverse_inertia_world(rn));
            let most = if inverse > 0.0 {
                share * speed / (dt.max(1e-9) * inverse)
            } else {
                0.0
            };
            if size > most {
                drag *= most / size;
            }
        }
        let rate = (settings.angular * sub.wetted_fraction() * share).min(1.0 / dt.max(1e-9));
        let spin = body.inertia_world() * body.omega_world();
        out[1] = Push {
            body: id,
            term: Term::Drag,
            force: drag,
            at: sub.centroid,
            couple: -spin * rate,
        };
        let target = &mut world[id];
        for push in &out {
            target.force += push.force;
            target.torque += (push.at - target.pos).cross(push.force) + push.couple;
        }
        pushes.extend(out);
    }
    for (i, (touched, still)) in wet.into_iter().enumerate() {
        if !touched {
            continue;
        }
        if still {
            // Floating at rest on still water counts as support for sleep.
            world.support(BodyId(i as u32));
        } else {
            // Waves and currents keep a floating body awake.
            world.bodies_mut()[i].sleep_time = 0.0;
        }
    }
    pushes
}
