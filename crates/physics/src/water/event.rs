//! Host events on water and the shared clock they are timed by.
//!
//! The gameplay surface is a pure function of a body, its seed, and the
//! world tick, so clients that agree on the tick agree on the water without
//! any stream of their own (`docs/verse/water.md`, the surface model and
//! determinism). What changes a body for a while (ice, a level change, a
//! dam) is an [`Event`] with a start tick and a schedule, sent once like any
//! other host event. A client that receives it late, or receives several in
//! another order, computes the same water at every tick, because an
//! [`Events`] list is kept in a canonical order and every effect is a pure
//! function of the tick.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use super::body::{Sample, WaterId};
use super::set::Water;

/// Physics steps a second: the world tick's rate.
pub const TICK_HZ: u64 = 120;

/// The world tick at `unix_ms` milliseconds after the Unix epoch: whole
/// physics steps since then. A zone with no host to hand out a tick (a
/// presence-only world) derives it from each client's clock, so clients
/// whose clocks agree within a step agree on the water.
#[must_use]
pub fn tick_at(unix_ms: u64) -> u64 {
    unix_ms / 1000 * TICK_HZ + unix_ms % 1000 * TICK_HZ / 1000
}

/// What an event does to its body while it lasts.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    /// The whole body's level rises by `rise` m (negative falls).
    Level { rise: f64 },
    /// Ice covers the disc: the water there stops moving.
    Ice { center: DVec2, radius: f64 },
    /// A wall from `a` to `b` dams the water: on its left side (looking
    /// from `a` to `b`), within `reach` m, the level rises by up to `rise`
    /// m, most at the wall.
    Dam {
        a: DVec2,
        b: DVec2,
        rise: f64,
        reach: f64,
    },
}

/// One host event on a body of water: an effect that ramps in over `ramp`
/// ticks from `start`, holds for `hold` ticks, and ramps out over `ramp`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Unique per zone; a repeated id is the same event.
    pub id: u64,
    pub body: WaterId,
    /// The world tick it starts at.
    pub start: u64,
    pub ramp: u64,
    /// [`u64::MAX`] holds forever.
    pub hold: u64,
    pub effect: Effect,
}

impl Event {
    /// How far the effect has set in at `tick`, 0 to 1.
    #[must_use]
    pub fn amount(&self, tick: u64) -> f64 {
        let Some(t) = tick.checked_sub(self.start) else {
            return 0.0;
        };
        let ramp = self.ramp.max(1);
        if t < ramp {
            return t as f64 / ramp as f64;
        }
        let t = t - ramp;
        if t < self.hold {
            return 1.0;
        }
        let t = t - self.hold;
        if t < ramp {
            1.0 - t as f64 / ramp as f64
        } else {
            0.0
        }
    }

    /// The first tick at which the effect is gone, if it ends.
    #[must_use]
    pub fn end(&self) -> Option<u64> {
        let ramp = self.ramp.max(1);
        self.start
            .checked_add(ramp)?
            .checked_add(self.hold)?
            .checked_add(ramp)
    }

    /// How much the level rises at `p` at `tick`, m.
    #[must_use]
    pub fn rise(&self, p: DVec2, tick: u64) -> f64 {
        match self.effect {
            Effect::Level { rise } => rise * self.amount(tick),
            Effect::Dam { a, b, rise, reach } => {
                let ab = b - a;
                let length = ab.length_squared();
                if length <= 0.0 || reach <= 0.0 || ab.perp_dot(p - a) <= 0.0 {
                    return 0.0;
                }
                let t = ((p - a).dot(ab) / length).clamp(0.0, 1.0);
                let d = p.distance(a + ab * t);
                if d >= reach {
                    return 0.0;
                }
                rise * (1.0 - d / reach) * self.amount(tick)
            }
            Effect::Ice { .. } => 0.0,
        }
    }

    /// How frozen the water at `p` is at `tick`, 0 to 1.
    #[must_use]
    pub fn ice(&self, p: DVec2, tick: u64) -> f64 {
        match self.effect {
            Effect::Ice { center, radius } if p.distance(center) <= radius => self.amount(tick),
            _ => 0.0,
        }
    }
}

/// The water at a point with the events on it applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eventful {
    pub sample: Sample,
    /// How frozen it is, 0 to 1; frozen water carries nothing.
    pub ice: f64,
}

/// A zone's water events, kept in the order of their start ticks and ids
/// whatever order they arrived in, so every client sums them alike.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Events {
    list: Vec<Event>,
}

impl Events {
    /// Adds `event`; false when its id is already here.
    pub fn insert(&mut self, event: Event) -> bool {
        if self.list.iter().any(|e| e.id == event.id) {
            return false;
        }
        let at = self
            .list
            .partition_point(|e| (e.start, e.id) < (event.start, event.id));
        self.list.insert(at, event);
        true
    }

    /// Drops the events that have ended by `tick`.
    pub fn prune(&mut self, tick: u64) {
        self.list.retain(|e| e.end().is_none_or(|end| end > tick));
    }

    #[must_use]
    pub fn list(&self) -> &[Event] {
        &self.list
    }

    /// How much events raise `body`'s level at `p` at `tick`, m.
    #[must_use]
    pub fn rise(&self, body: WaterId, p: DVec2, tick: u64) -> f64 {
        self.list
            .iter()
            .filter(|e| e.body == body)
            .map(|e| e.rise(p, tick))
            .sum()
    }

    /// How frozen `body` is at `p` at `tick`, 0 to 1.
    #[must_use]
    pub fn ice(&self, body: WaterId, p: DVec2, tick: u64) -> f64 {
        self.list
            .iter()
            .filter(|e| e.body == body)
            .map(|e| e.ice(p, tick))
            .fold(0.0, f64::max)
    }

    /// `water` over `(x, z)` at `tick` with these events applied: the
    /// level raised, and frozen water still.
    #[must_use]
    pub fn sample(&self, water: &impl Water, x: f64, z: f64, tick: u64) -> Option<Eventful> {
        let mut sample = water.sample(x, z, tick)?;
        let p = DVec2::new(x, z);
        sample.height += self.rise(sample.body, p, tick);
        let ice = self.ice(sample.body, p, tick);
        if ice > 0.0 {
            sample.surface_velocity *= 1.0 - ice;
            sample.flow *= 1.0 - ice;
        }
        Some(Eventful { sample, ice })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::water::{WaterBody, WaterSet};

    fn events() -> Vec<Event> {
        vec![
            Event {
                id: 7,
                body: WaterId(0),
                start: 600,
                ramp: 240,
                hold: 1200,
                effect: Effect::Level { rise: 0.3 },
            },
            Event {
                id: 3,
                body: WaterId(0),
                start: 900,
                ramp: 120,
                hold: u64::MAX,
                effect: Effect::Ice {
                    center: DVec2::new(4.0, 0.0),
                    radius: 6.0,
                },
            },
            Event {
                id: 9,
                body: WaterId(0),
                start: 300,
                ramp: 60,
                hold: 2400,
                effect: Effect::Dam {
                    a: DVec2::new(-5.0, -5.0),
                    b: DVec2::new(5.0, -5.0),
                    rise: 0.25,
                    reach: 12.0,
                },
            },
        ]
    }

    /// The clock counts whole steps: a millisecond's skew moves the tick by
    /// at most one.
    #[test]
    fn the_clock_counts_whole_steps() {
        assert_eq!(tick_at(0), 0);
        assert_eq!(tick_at(1000), TICK_HZ);
        assert_eq!(tick_at(1_759_000_000_123), 1_759_000_000 * TICK_HZ + 14);
        let t = 1_759_000_000_000u64;
        assert!(tick_at(t + 3) - tick_at(t) <= 1);
    }

    /// An event ramps in from its start tick, holds, and ramps out.
    #[test]
    fn an_event_follows_its_schedule() {
        let e = events()[0];
        assert_eq!(e.amount(599), 0.0);
        assert_eq!(e.amount(600), 0.0);
        assert_eq!(e.amount(720), 0.5);
        assert_eq!(e.amount(900), 1.0);
        assert_eq!(e.amount(600 + 240 + 1200 + 120), 0.5);
        assert_eq!(e.amount(e.end().unwrap()), 0.0);
        assert_eq!(events()[1].end(), None);
    }

    /// Two clients that receive the same events in different orders, one
    /// of them only after the events started, sample the same water at
    /// every shared tick, bit for bit.
    #[test]
    fn clients_agree_whatever_order_events_arrive_in() {
        let water = WaterSet::new(
            vec![WaterBody::pond(
                WaterId(0),
                vec![
                    DVec2::new(-20.0, -20.0),
                    DVec2::new(20.0, -20.0),
                    DVec2::new(20.0, 20.0),
                    DVec2::new(-20.0, 20.0),
                ],
                1.0,
            )],
            4.0,
        );
        let mut early = Events::default();
        for e in events() {
            assert!(early.insert(e));
        }
        assert!(!early.insert(events()[0]));
        let mut late = Events::default();
        for e in events().into_iter().rev() {
            late.insert(e);
        }
        assert_eq!(early, late);
        for tick in [0, 299, 330, 700, 960, 1500, 4000] {
            for (x, z) in [
                (0.0, 0.0),
                (4.0, 1.0),
                (0.0, -1.0),
                (0.0, -9.0),
                (12.0, 12.0),
            ] {
                let a = early.sample(&water, x, z, tick).unwrap();
                let b = late.sample(&water, x, z, tick).unwrap();
                assert_eq!(a, b, "tick {tick} at ({x}, {z})");
            }
        }
        // The level rose, the dam raised the water on its left side only,
        // and the ice holds.
        let at = |x, z, tick| early.sample(&water, x, z, tick).unwrap();
        assert!((at(-15.0, 15.0, 1000).sample.height - 1.3).abs() < 1e-12);
        assert!(at(0.0, -1.0, 400).sample.height > at(0.0, -9.0, 400).sample.height);
        assert_eq!(at(4.0, 0.0, 1100).ice, 1.0);
        assert_eq!(at(15.0, 0.0, 1100).ice, 0.0);
        early.prune(10_000);
        assert_eq!(early.list().len(), 1);
    }
}
