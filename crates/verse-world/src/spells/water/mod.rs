//! Spells and water, by `docs/verse/water.md`'s Spells and water section:
//! SRD 5.2.1 where it has a rule, and **ours**, marked in comments, where
//! it is silent.
//!
//! - [`ice`]: the four ice states (slush, thin ice, walkable ice, floes),
//!   the freezing table, staged thaw, thin ice's rolled tolerance,
//!   Slippery Ice, creatures caught at the surface, breakable cells, and
//!   fire melting ice.
//! - [`control`]: Control Water's flood (a level rise, or a 20-foot wave in
//!   a large body), part water, redirect flow, and whirlpool.
//! - [`lightning`]: our conduction rule.
//! - [`effects`]: everything else a spell does to water: Wind Wall, Gust of
//!   Wind, Thunderwave, Meteor Swarm and the fire spells, Reverse Gravity,
//!   Wall of Stone's dam, Create or Destroy Water, Water Walk, Water
//!   Breathing, plants, webs, fog, and steam.
//! - [`weather`]: SRD 5.2.1's Heavy Precipitation and Strong Wind over the
//!   zone's weather schedule, Call Lightning's storm bonus, and the spells
//!   that make weather as local overlays on it.
//!
//! [`WaterSpells`] is the authoritative state. It lives in
//! [`super::SpellWorld`], so a checkpoint saves it; what changes the water
//! itself (ice, a level, a trench, a current, a vortex, a dam) is a
//! `physics::water` [`Event`] with a start tick, so every client computes
//! the same water from the tick without a stream of its own. Times are world
//! ticks at [`TICK_HZ`].

pub mod control;
pub mod effects;
pub mod ice;
pub mod lightning;
pub mod weather;

use std::collections::{BTreeMap, BTreeSet};

use glam::{DVec2, DVec3};
use physics::water::{Effect, Event, Events, Sample, TICK_HZ, Water, WaterId, WaterSet};
use serde::{Deserialize, Serialize};

pub use control::{Control, Mode, Refusal};
pub use effects::{Rain, Vapor, VaporKind};
pub use ice::{Freeze, Ice, IceState, Shape};
pub use lightning::{CONDUCTION, Conducted, Delivery};

/// Water spell events start their ids here, above any other source of
/// water events in a zone.
pub const EVENT_BASE: u64 = 1 << 40;
/// The most live water events a zone keeps.
pub const MAX_EVENTS: usize = 128;

/// Whole ticks in `seconds`.
#[must_use]
pub fn ticks(seconds: f64) -> u64 {
    (seconds.max(0.0) * TICK_HZ as f64).round() as u64
}

/// Seconds in `ticks`.
#[must_use]
pub fn seconds(ticks: u64) -> f64 {
    ticks as f64 / TICK_HZ as f64
}

/// A zone's water as the rules read it: its bodies, its bed (the ground
/// height under any point), and its seed.
#[derive(Clone, Copy)]
pub struct Basin<'a> {
    pub water: &'a WaterSet,
    pub bed: &'a dyn Fn(DVec2) -> f64,
    pub seed: u64,
}

impl Basin<'_> {
    /// The water over `p` at `tick` with `events` applied.
    #[must_use]
    pub fn sample(&self, events: &Events, p: DVec2, tick: u64) -> Option<Sample> {
        events.over(self.water).sample(p.x, p.y, tick)
    }

    /// The water over `p` at `tick` before any spell touched it.
    #[must_use]
    pub fn sample_base(&self, p: DVec2, tick: u64) -> Option<Sample> {
        self.water.sample(p.x, p.y, tick)
    }

    /// How deep the water over `p` is at rest, m.
    #[must_use]
    pub fn depth(&self, p: DVec2) -> Option<f64> {
        let s = self.water.sample(p.x, p.y, 0)?;
        Some(s.height - (self.bed)(p))
    }

    /// The swell's amplitude on `body`, m: its Gerstner terms' amplitudes
    /// summed, plus half an ocean spectrum's significant height.
    #[must_use]
    pub fn swell(&self, body: WaterId) -> f64 {
        let Some(b) = self.water.get(body) else {
            return 0.0;
        };
        let terms: f64 = b.waves.waves.iter().map(|w| w.amplitude.abs()).sum();
        terms
            + b.waves
                .spectrum
                .as_ref()
                .map_or(0.0, |s| 0.5 * s.significant_height())
    }

    /// The fastest current anywhere in `body`, m/s.
    #[must_use]
    pub fn peak_flow(&self, body: WaterId) -> f64 {
        self.water
            .get(body)
            .and_then(|b| b.flow.as_ref())
            .map_or(0.0, |f| {
                f.velocity.iter().map(|v| v.length()).fold(0.0, f64::max)
            })
    }
}

/// A creature the water rules act on, where it stands this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wader {
    pub id: u64,
    pub feet: DVec3,
    /// Its weight with what it carries, kg.
    pub kg: f64,
    /// It sits in a boat.
    pub boat: bool,
}

impl Wader {
    #[must_use]
    pub fn new(id: u64, feet: DVec3) -> Self {
        Self {
            id,
            feet,
            kg: 75.0,
            boat: false,
        }
    }

    #[must_use]
    pub fn xz(&self) -> DVec2 {
        DVec2::new(self.feet.x, self.feet.z)
    }

    /// Its head: the top of the chamber's capsule.
    #[must_use]
    pub fn head(&self) -> f64 {
        self.feet.y + super::CHARACTER_HEIGHT
    }
}

/// Every spell's hold on the zone's water.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WaterSpells {
    /// The water events the spells started, for every client.
    pub events: Events,
    pub ice: Ice,
    pub control: Option<Control>,
    /// Wall of Stone dams: the cast and its event.
    pub dams: Vec<(u64, u64)>,
    /// Water Walk and Water Breathing: creature to the tick each ends.
    pub walkers: BTreeMap<u64, u64>,
    pub breathers: BTreeMap<u64, u64>,
    pub rains: Vec<Rain>,
    pub vapors: Vec<Vapor>,
    /// Objects and the last tick they were in water, so fire can't light
    /// them for a minute after (ours).
    pub wet: BTreeMap<u64, u64>,
    /// Lightning that already conducted: (bolt, creature).
    pub conducted: BTreeSet<(u64, u64)>,
    next: u64,
}

impl WaterSpells {
    /// A fresh event id.
    pub(crate) fn event_id(&mut self) -> u64 {
        self.next += 1;
        EVENT_BASE + self.next
    }

    /// Starts an event on `body` and returns its id.
    pub(crate) fn start(
        &mut self,
        body: WaterId,
        start: u64,
        ramp: u64,
        hold: u64,
        effect: Effect,
    ) -> Result<u64, String> {
        if self.events.list().len() >= MAX_EVENTS {
            return Err("Too many water spell events".into());
        }
        let id = self.event_id();
        self.events.insert(Event {
            id,
            body,
            start,
            ramp,
            hold,
            effect,
        });
        Ok(id)
    }

    /// Casts `spell` at `center` (Cone of Cold from its apex along `dir`)
    /// at `tick`: the ice forms by the freezing table, and creatures in
    /// `waders` whose heads are above the water where it turns walkable
    /// are caught. Returns the patch's id.
    #[allow(clippy::too_many_arguments)]
    pub fn freeze(
        &mut self,
        basin: &Basin,
        spell: Freeze,
        center: DVec2,
        dir: DVec2,
        cast: u64,
        dc: i32,
        waders: &[Wader],
        tick: u64,
    ) -> Result<u64, String> {
        let shape = match spell {
            Freeze::ConeOfCold => Shape::Cone {
                apex: center,
                dir,
                length: spell.row().radius,
            },
            _ => Shape::Disc {
                center,
                radius: spell.row().radius,
            },
        };
        self.freeze_shape(basin, spell, shape, cast, dc, waders, tick)
    }

    /// As [`Self::freeze`], over an explicit area, such as the Grove's
    /// 60-foot Storm of Vengeance.
    #[allow(clippy::too_many_arguments)]
    pub fn freeze_shape(
        &mut self,
        basin: &Basin,
        spell: Freeze,
        shape: Shape,
        cast: u64,
        dc: i32,
        waders: &[Wader],
        tick: u64,
    ) -> Result<u64, String> {
        if self.events.list().len() + 4 > MAX_EVENTS {
            return Err("Too many water spell events".into());
        }
        let id = self.event_id();
        let id = self
            .ice
            .freeze(basin, &mut self.events, id, spell, shape, cast, dc, tick)?;
        let formed = tick + ticks(spell.row().delay);
        if formed == tick {
            self.ice.catch(basin, waders, tick);
        }
        Ok(id)
    }

    /// Advances the spells to `tick`: thawed ice, spent rain and vapor,
    /// ended Water Walk and Water Breathing, and finished events leave.
    pub fn tick(&mut self, tick: u64) {
        self.ice.prune(tick);
        self.walkers.retain(|_, until| *until > tick);
        self.breathers.retain(|_, until| *until > tick);
        self.rains.retain(|r| r.until > tick);
        self.vapors.retain(|v| v.until > tick);
        let minute = ticks(60.0);
        self.wet.retain(|_, at| at.saturating_add(minute) > tick);
        if let Some(control) = &self.control
            && control.until <= tick
        {
            let control = self.control.take();
            if let Some(control) = control {
                control.end(&mut self.events, tick);
            }
        }
        self.events.prune(tick);
    }

    /// Ends what `cast` holds by concentration at `tick`.
    pub fn end_cast(&mut self, cast: u64, tick: u64) {
        let events = &mut self.events;
        self.ice.end_cast(cast, tick, events);
        if self.control.as_ref().is_some_and(|c| c.cast == cast)
            && let Some(control) = self.control.take()
        {
            control.end(events, tick);
        }
        for (_, id) in self.dams.iter().filter(|(c, _)| *c == cast) {
            events.end(*id, tick);
        }
        self.dams.retain(|(c, _)| *c != cast);
    }

    /// The water over `p` at `tick` as the spells leave it.
    #[must_use]
    pub fn sample(&self, basin: &Basin, p: DVec2, tick: u64) -> Option<Sample> {
        basin.sample(&self.events, p, tick)
    }

    /// Whether `creature` breathes water at `tick` (Water Breathing).
    #[must_use]
    pub fn breathes(&self, creature: u64, tick: u64) -> bool {
        self.breathers
            .get(&creature)
            .is_some_and(|until| *until > tick)
    }

    /// Whether `creature` walks on water at `tick` (Water Walk).
    #[must_use]
    pub fn walks(&self, creature: u64, tick: u64) -> bool {
        self.walkers
            .get(&creature)
            .is_some_and(|until| *until > tick)
    }

    /// Whether no spell holds the water, so a checkpoint without water
    /// spells keeps its old shape.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Bounds a checkpoint may hold.
    pub fn validate(&self) -> Result<(), String> {
        if self.events.list().len() > MAX_EVENTS
            || self.dams.len() > 32
            || self.walkers.len() > 256
            || self.breathers.len() > 256
            || self.rains.len() > 32
            || self.vapors.len() > 128
            || self.wet.len() > 1024
            || self.conducted.len() > 4096
        {
            return Err("Invalid water spell checkpoint".into());
        }
        self.ice.validate()?;
        if let Some(control) = &self.control {
            control.validate()?;
        }
        Ok(())
    }
}
