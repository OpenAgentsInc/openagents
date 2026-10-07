//! The Water Lab's training dummies: the Grove's dummies
//! (`verse_zone_grove::zones::grove::dummies`), with their health, saves,
//! and resistances, drawn as the pack's `props/Dummy` in the character's
//! figure ([`verse_zone_grove::zones::grove::draw::Model`]), with the
//! Grove's health bars and floating combat numbers.
//!
//! Three stand on the beach, two wade in the shallows, and one wades in
//! the river, so lightning's conduction through the sea and the river
//! shows. A Water Orb that engulfs one carries it, its weight borne by the
//! orb's water; a burst throws it back, and it falls to the ground or the
//! bed. A dummy nothing touches for [`RESET_AFTER`] s stands back up at
//! full health where it started, as in the Grove.

use glam::{Vec2, Vec3};
use verse_zone_everglade::zones::everglade::floaters::Floater;
use verse_zone_grove::zones::grove::dummies::{Dummy, Kind};

use crate::{SPAWN, ground};

pub use verse_zone_grove::zones::grove::dummies::HEIGHT;

/// Seconds a dummy waits untouched before it stands back up at home.
pub const RESET_AFTER: f32 = 10.0;
/// How stiffly an orb pulls a dummy toward its middle, 1/s², and matches
/// it to the water, 1/s.
const PULL: f32 = 9.0;
const DRAG: f32 = 5.0;
const GRAVITY: f32 = 9.81;

/// The field: each dummy's kind and where it stands, x and z.
#[must_use]
pub fn field() -> Vec<(Kind, [f32; 2])> {
    vec![
        (Kind::Straw, [-3.0, 9.0]),
        (Kind::Armored, [11.0, 8.0]),
        (Kind::Big, [-10.0, 12.5]),
        (Kind::Straw, [2.0, -4.0]),
        (Kind::Warded, [8.0, -6.0]),
        // In the lower river, on its centerline between the rocks.
        (Kind::Straw, [-36.1, 19.0]),
    ]
}

/// One dummy in the lab.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub dummy: Dummy,
    /// Its velocity while thrown or carried, m/s.
    pub vel: Vec3,
    /// When anything last touched it, s.
    pub touched: f32,
}

/// What a dummy stands on at (x, z): the ground, or the bed under water.
#[must_use]
pub fn support(x: f32, z: f32) -> f32 {
    ground(x, z)
}

impl Target {
    /// A dummy of `kind` standing at `at`, facing the spawn.
    #[must_use]
    pub fn new(kind: Kind, at: [f32; 2]) -> Self {
        let mut dummy = Dummy::new(kind, at);
        let home = Vec3::new(at[0], support(at[0], at[1]), at[1]);
        dummy.home = home;
        dummy.pos = home;
        dummy.yaw = (SPAWN.x - at[0]).atan2(SPAWN.z - at[1]);
        Self {
            dummy,
            vel: Vec3::ZERO,
            touched: f32::NEG_INFINITY,
        }
    }

    /// The whole field, at rest.
    #[must_use]
    pub fn field() -> Vec<Self> {
        field()
            .into_iter()
            .map(|(kind, at)| Self::new(kind, at))
            .collect()
    }

    /// Back at home, at full health.
    pub fn reset(&mut self) {
        let yaw = self.dummy.yaw;
        let home = self.dummy.home;
        self.dummy.reset();
        self.dummy.home = home;
        self.dummy.pos = home;
        self.dummy.yaw = yaw;
        self.vel = Vec3::ZERO;
    }

    /// Advances it `dt` seconds at `now`: carried toward `hold`'s center
    /// at its velocity while an orb holds it, else falling to what it
    /// stands on and sliding to a stop.
    pub fn tick(&mut self, dt: f32, now: f32, hold: Option<(Vec3, Vec3)>) {
        if let Some((center, velocity)) = hold {
            self.touched = now;
            let at = self.dummy.center();
            let accel = (center - at) * PULL + (velocity - self.vel) * DRAG;
            self.vel += accel * dt;
            self.dummy.pos += self.vel * dt;
            // It turns slowly in the water.
            self.dummy.yaw += 0.5 * dt;
            return;
        }
        let floor = support(self.dummy.pos.x, self.dummy.pos.z);
        let airborne = self.dummy.pos.y > floor + 0.01 || self.vel.length_squared() > 1e-4;
        if airborne {
            self.vel.y -= GRAVITY * dt;
            self.dummy.pos += self.vel * dt;
            let floor = support(self.dummy.pos.x, self.dummy.pos.z);
            if self.dummy.pos.y <= floor {
                self.dummy.pos.y = floor;
                self.vel.y = 0.0;
                let slow = (-6.0 * dt).exp();
                self.vel.x *= slow;
                self.vel.z *= slow;
                if Vec2::new(self.vel.x, self.vel.z).length() < 0.05 {
                    self.vel = Vec3::ZERO;
                }
            }
        }
        let idle = now - self.touched.max(self.dummy.hit_at);
        let away = self.dummy.pos.distance(self.dummy.home) > 0.3;
        if idle > RESET_AFTER && (away || self.dummy.hp < self.dummy.kind.max_hp()) {
            self.reset();
        }
    }
}

/// A floating label over a dummy whose top is at `top` and feet at `feet`.
#[must_use]
pub fn floater(top: f32, feet: Vec3, text: &str, color: [f32; 3], now: f32) -> Floater {
    Floater {
        at: Vec3::new(feet.x, top + 0.9, feet.z),
        text: text.into(),
        color,
        start: now,
    }
}
