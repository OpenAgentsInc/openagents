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
//!
//! Spells write their water through these effects
//! (`docs/verse/water.md`, Spells and water): a flood is a [`Effect::Level`]
//! or a [`Effect::Surge`], Part Water a [`Effect::Trench`], Redirect Flow a
//! [`Effect::Current`], a whirlpool a [`Effect::Vortex`], Destroy Water's
//! dimple a [`Effect::Bowl`], Wall of Stone a [`Effect::Dam`], and ice an
//! [`Effect::Ice`]. [`Evented`] is a zone's water with its events applied,
//! so buoyancy and swimming read the same water every client draws.

use glam::{DVec2, DVec3};
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
    /// A trench from `a` to `b`, `half_width` m to each side, where the
    /// surface drops by `depth` m: the water there is gone to the bed.
    Trench {
        a: DVec2,
        b: DVec2,
        half_width: f64,
        depth: f64,
    },
    /// The current in the square of half side `half` about `center` runs
    /// at `velocity`, m/s, in place of the body's own.
    Current {
        center: DVec2,
        half: f64,
        velocity: DVec2,
    },
    /// A whirlpool `radius` m wide at the surface: the current turns about
    /// `center` at `swirl` m/s and pulls in at `pull` m/s, and the surface
    /// sinks by up to `depth` m at the eye.
    Vortex {
        center: DVec2,
        radius: f64,
        pull: f64,
        swirl: f64,
        depth: f64,
    },
    /// A smooth dimple `depth` m deep and `radius` m wide.
    Bowl {
        center: DVec2,
        radius: f64,
        depth: f64,
    },
    /// A solitary wave `height` m high that starts on the line through
    /// `origin` square to `dir` and crosses `length` m along `dir` over the
    /// event's hold, `half_width` m to each side of `origin`'s line.
    Surge {
        origin: DVec2,
        dir: DVec2,
        half_width: f64,
        length: f64,
        height: f64,
    },
}

/// The distance from `p` to the segment from `a` to `b`, and how far along
/// it the nearest point lies, 0 to 1.
fn segment(p: DVec2, a: DVec2, b: DVec2) -> (f64, f64) {
    let ab = b - a;
    let length = ab.length_squared();
    let t = if length > 0.0 {
        ((p - a).dot(ab) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.distance(a + ab * t), t)
}

/// How far a dam turns the current aside, m from its wall.
pub const DAM_SHADOW: f64 = 1.5;
/// How wide a surge's crest is, m: the sech² profile's length scale.
pub const SURGE_WIDTH: f64 = 3.0;

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
            Effect::Trench {
                a,
                b,
                half_width,
                depth,
            } => {
                let (d, _) = segment(p, a, b);
                // The walls slope over a meter.
                let edge = (1.0 - (d - half_width).max(0.0)).clamp(0.0, 1.0);
                -depth * edge * self.amount(tick)
            }
            Effect::Vortex {
                center,
                radius,
                depth,
                ..
            } => {
                let r = p.distance(center) / radius.max(0.1);
                -depth * (-(r * 2.2) * (r * 2.2)).exp() * self.amount(tick)
            }
            Effect::Bowl {
                center,
                radius,
                depth,
            } => {
                let r = p.distance(center) / radius.max(0.01);
                if r >= 1.0 {
                    0.0
                } else {
                    -depth * (1.0 - r * r) * self.amount(tick)
                }
            }
            Effect::Surge {
                origin,
                dir,
                half_width,
                length,
                height,
            } => {
                let Some(t) = tick.checked_sub(self.start) else {
                    return 0.0;
                };
                let dir = dir.normalize_or_zero();
                let local = p - origin;
                if local.perp_dot(dir).abs() > half_width {
                    return 0.0;
                }
                let travel = self.hold.max(1);
                if t > travel {
                    return 0.0;
                }
                let front = length * t as f64 / travel as f64;
                let x = (local.dot(dir) - front) / SURGE_WIDTH;
                let sech = 1.0 / x.cosh();
                height * sech * sech
            }
            Effect::Ice { .. } | Effect::Current { .. } => 0.0,
        }
    }

    /// The current at `p` at `tick` once this event acts on `flow`, m/s.
    #[must_use]
    pub fn flow(&self, flow: DVec2, p: DVec2, tick: u64) -> DVec2 {
        let k = self.amount(tick);
        if k <= 0.0 {
            return flow;
        }
        match self.effect {
            Effect::Current {
                center,
                half,
                velocity,
            } => {
                let d = (p - center).abs();
                if d.x <= half && d.y <= half {
                    flow.lerp(velocity, k)
                } else {
                    flow
                }
            }
            Effect::Vortex {
                center,
                radius,
                pull,
                swirl,
                ..
            } => {
                let to = center - p;
                let r = to.length();
                if r >= radius || r < 1e-6 {
                    return flow;
                }
                let inward = to / r;
                let around = DVec2::new(-inward.y, inward.x);
                flow + (inward * pull + around * swirl) * k
            }
            Effect::Dam { a, b, .. } => {
                let (d, _) = segment(p, a, b);
                if d >= DAM_SHADOW {
                    return flow;
                }
                // Turn the current along the wall: take away its part
                // across the wall, the more the nearer.
                let along = (b - a).normalize_or_zero();
                let normal = DVec2::new(-along.y, along.x);
                let across = flow.dot(normal);
                flow - normal * across * (1.0 - d / DAM_SHADOW) * k
            }
            Effect::Trench {
                a, b, half_width, ..
            } => {
                let (d, _) = segment(p, a, b);
                if d <= half_width {
                    flow * (1.0 - k)
                } else {
                    flow
                }
            }
            Effect::Level { .. }
            | Effect::Ice { .. }
            | Effect::Bowl { .. }
            | Effect::Surge { .. } => flow,
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

    /// Ends event `id` at `tick`: it ramps out from there, or from the end
    /// of its ramp in. False when there is no such event.
    pub fn end(&mut self, id: u64, tick: u64) -> bool {
        let Some(event) = self.list.iter_mut().find(|e| e.id == id) else {
            return false;
        };
        let held = tick
            .saturating_sub(event.start)
            .saturating_sub(event.ramp.max(1));
        event.hold = event.hold.min(held);
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
        let sample = self.over(water).sample(x, z, tick)?;
        let ice = self.ice(sample.body, DVec2::new(x, z), tick);
        Some(Eventful { sample, ice })
    }

    /// `water` with these events applied, as [`Water`].
    #[must_use]
    pub fn over<'a, W: Water + ?Sized>(&'a self, water: &'a W) -> Evented<'a, W> {
        Evented {
            water,
            events: self,
        }
    }
}

/// A zone's water with its events applied, as [`Water`]: what floating
/// bodies and swimmers feel while spells act on it.
#[derive(Clone, Copy, Debug)]
pub struct Evented<'a, W: ?Sized> {
    pub water: &'a W,
    pub events: &'a Events,
}

impl<W: Water + ?Sized> Water for Evented<'_, W> {
    fn sample(&self, x: f64, z: f64, tick: u64) -> Option<Sample> {
        let mut sample = self.water.sample(x, z, tick)?;
        let p = DVec2::new(x, z);
        sample.height += self.events.rise(sample.body, p, tick);
        let mut flow = DVec2::new(sample.flow.x, sample.flow.z);
        for event in self.events.list.iter().filter(|e| e.body == sample.body) {
            flow = event.flow(flow, p, tick);
        }
        sample.flow = DVec3::new(flow.x, 0.0, flow.y);
        let ice = self.events.ice(sample.body, p, tick);
        if ice > 0.0 {
            sample.surface_velocity *= 1.0 - ice;
            sample.flow *= 1.0 - ice;
        }
        Some(sample)
    }

    fn bodies_overlapping(&self, min: DVec2, max: DVec2) -> Vec<WaterId> {
        self.water.bodies_overlapping(min, max)
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

    fn ev(id: u64, start: u64, ramp: u64, hold: u64, effect: Effect) -> Event {
        Event {
            id,
            body: WaterId(0),
            start,
            ramp,
            hold,
            effect,
        }
    }

    /// The spells' effects shape the level and the current, and an ended
    /// event ramps out from the tick it ended.
    #[test]
    fn spell_effects_shape_level_and_current() {
        let water = WaterSet::new(vec![WaterBody::ocean(WaterId(0), 0.0)], 8.0);
        let mut events = Events::default();
        events.insert(ev(
            1,
            0,
            1,
            u64::MAX,
            Effect::Trench {
                a: DVec2::new(-10.0, 0.0),
                b: DVec2::new(10.0, 0.0),
                half_width: 1.5,
                depth: 5.0,
            },
        ));
        events.insert(ev(
            2,
            0,
            1,
            u64::MAX,
            Effect::Current {
                center: DVec2::new(0.0, 20.0),
                half: 5.0,
                velocity: DVec2::new(1.0, 0.0),
            },
        ));
        events.insert(ev(
            3,
            0,
            1,
            u64::MAX,
            Effect::Vortex {
                center: DVec2::new(0.0, -20.0),
                radius: 7.5,
                pull: 0.5,
                swirl: 2.0,
                depth: 2.0,
            },
        ));
        let at = |events: &Events, x, z, tick| events.sample(&water, x, z, tick).unwrap().sample;
        assert!((at(&events, 0.0, 0.0, 10).height + 5.0).abs() < 1e-9);
        assert!(at(&events, 0.0, 5.0, 10).height.abs() < 1e-9);
        assert!((at(&events, 2.0, 21.0, 10).flow.x - 1.0).abs() < 1e-12);
        assert_eq!(at(&events, 9.0, 21.0, 10).flow.x, 0.0);
        // The vortex pulls toward its eye and turns about it.
        let v = at(&events, 4.0, -20.0, 10).flow;
        assert!(v.x < -0.49 && v.z.abs() > 1.0);
        // Ended at tick 120, the trench is back to level after its ramp.
        assert!(events.end(1, 120));
        assert!(at(&events, 0.0, 0.0, 120).height < -4.9);
        assert!(at(&events, 0.0, 0.0, 125).height.abs() < 1e-9);
    }

    /// A surge crosses its length over its hold, and a dam turns the
    /// current along its wall.
    #[test]
    fn a_surge_crosses_and_a_dam_turns_the_current() {
        let surge = ev(
            1,
            100,
            1,
            720,
            Effect::Surge {
                origin: DVec2::ZERO,
                dir: DVec2::X,
                half_width: 15.0,
                length: 30.0,
                height: 6.0,
            },
        );
        assert!((surge.rise(DVec2::new(0.0, 0.0), 100) - 6.0).abs() < 1e-9);
        assert!((surge.rise(DVec2::new(15.0, 3.0), 460) - 6.0).abs() < 1e-9);
        assert!(surge.rise(DVec2::new(0.0, 0.0), 460) < 0.01);
        assert_eq!(surge.rise(DVec2::new(15.0, 16.0), 460), 0.0);
        let dam = ev(
            2,
            0,
            1,
            u64::MAX,
            Effect::Dam {
                a: DVec2::new(0.0, -5.0),
                b: DVec2::new(0.0, 5.0),
                rise: 0.3,
                reach: 6.0,
            },
        );
        let turned = dam.flow(DVec2::new(1.0, 0.5), DVec2::new(-0.1, 0.0), 10);
        assert!(turned.x.abs() < 0.1 && (turned.y - 0.5).abs() < 1e-12);
        assert_eq!(
            dam.flow(DVec2::new(1.0, 0.5), DVec2::new(-3.0, 0.0), 10),
            DVec2::new(1.0, 0.5)
        );
    }
}
