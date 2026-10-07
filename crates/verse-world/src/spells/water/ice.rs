//! Ice on water: the four states, the freezing table, staged thaw, and
//! what ice does to the creatures and bodies on, in, and under it
//! (`docs/verse/water.md`, Ice states and Which spells freeze water).
//!
//! The SRD 5.2.1 hazards Thin Ice (a 3d10 × 10 lb tolerance a 10-foot
//! square) and Slippery Ice (Difficult Terrain and a DC 10 Dexterity
//! check or Prone) are the SRD's; which spell makes which state, for how
//! long, and how it thaws are ours.

use std::collections::{BTreeMap, BTreeSet};

use glam::DVec2;
use physics::water::{Effect, Events, WaterId};
use serde::{Deserialize, Serialize};

use super::{Basin, Wader, seconds, ticks};
use crate::spells::{Dice, FEET, Save};

/// Ice on water, weakest first, so the stronger of two overlapping states
/// is the greater.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum IceState {
    /// Floating crystals: Difficult Terrain to swim through; holds no one.
    Slush,
    /// Broken ice drifting with the flow: a floe bears one Medium creature.
    Floes,
    /// Each 3 m cell bears its rolled tolerance, then breaks.
    Thin,
    /// A static surface that bears any Medium or smaller creature: Slippery
    /// Ice.
    Walkable,
}

impl IceState {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Slush => "slush",
            Self::Floes => "floes",
            Self::Thin => "thin ice",
            Self::Walkable => "walkable ice",
        }
    }
}

/// The side of a thin-ice cell, m: 10 feet.
pub const CELL: f64 = 10.0 * FEET;
/// One pound, kg.
pub const POUND: f64 = 0.453_592_37;
/// Water flowing faster than this only slushes, m/s (ours).
pub const FLOWING: f64 = 0.5;
/// On a swell with a greater amplitude, walkable ice forms as floes, m
/// (ours).
pub const SWELL: f64 = 0.3;
/// The most a floe bears: one Medium creature, kg (ours).
pub const FLOE_LOAD: f64 = 140.0;
/// Slippery Ice's Dexterity (Acrobatics) DC (SRD 5.2.1).
pub const SLIP_DC: i32 = 10;
/// How long a slip leaves a creature Prone, s (ours: the combat model's
/// knockdown).
pub const PRONE: f64 = 1.5;
/// An ice cell's Armor Class as an object (ours, from the SRD's object
/// table for a resilient Medium object).
pub const CELL_AC: i32 = 13;
/// The most patches a zone holds.
pub const MAX_PATCHES: usize = 64;

/// A spell that freezes water: every one on the Grove's and Everglade's
/// bars and the Water Lab's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Freeze {
    RayOfFrost,
    IceKnife,
    SleetStorm,
    IceStorm,
    ConeOfCold,
    StormOfVengeance,
}

/// One row of the freezing table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    /// The disc it freezes, m: the SRD area (Cone of Cold's is its 18 m
    /// length).
    pub radius: f64,
    /// Each stage and how long it lasts, s; a concentration spell's first
    /// stage lasts until the spell ends, at most this long.
    pub stages: &'static [(IceState, f64)],
    /// Whether the last stage is slush dissolving rather than floes.
    pub dissolves: bool,
    pub thickness_cm: f64,
    pub concentration: bool,
    /// How long after the cast the ice forms, s.
    pub delay: f64,
    /// How long hail roughens it before it turns slippery, s.
    pub hail: f64,
}

impl Freeze {
    pub const ALL: [Self; 6] = [
        Self::RayOfFrost,
        Self::IceKnife,
        Self::SleetStorm,
        Self::IceStorm,
        Self::ConeOfCold,
        Self::StormOfVengeance,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::RayOfFrost => "Ray of Frost",
            Self::IceKnife => "Ice Knife",
            Self::SleetStorm => "Sleet Storm",
            Self::IceStorm => "Ice Storm",
            Self::ConeOfCold => "Cone of Cold",
            Self::StormOfVengeance => "Storm of Vengeance",
        }
    }

    /// Its row of the freezing table.
    #[must_use]
    pub fn row(self) -> Row {
        use IceState::{Floes, Slush, Thin, Walkable};
        let row = |radius_ft: f64, stages, dissolves, thickness_cm| Row {
            radius: radius_ft * FEET,
            stages,
            dissolves,
            thickness_cm,
            concentration: false,
            delay: 0.0,
            hail: 0.0,
        };
        match self {
            // A disc of 1.5 m where the ray meets water: slush for the
            // SRD's one round, dissolving over 3 s.
            Self::RayOfFrost => row(5.0, &[(Slush, 6.0), (Slush, 3.0)], true, 2.0),
            // The 5-foot burst: thin ice for a round, floes for 3 s.
            Self::IceKnife => row(5.0, &[(Thin, 6.0), (Floes, 3.0)], false, 3.0),
            // SRD 5.2.1's 20-foot-radius cylinder: slush while it lasts,
            // up to a minute, dissolving over 20 s.
            Self::SleetStorm => Row {
                concentration: true,
                ..row(20.0, &[(Slush, 60.0), (Slush, 20.0)], true, 5.0)
            },
            // Walkable ice for 30 s, the hail's Difficult Terrain for the
            // first round, then thin for 10 s and floes for 10 s.
            Self::IceStorm => Row {
                hail: 6.0,
                ..row(
                    20.0,
                    &[(Walkable, 30.0), (Thin, 10.0), (Floes, 10.0)],
                    false,
                    10.0,
                )
            },
            Self::ConeOfCold => row(
                60.0,
                &[(Walkable, 60.0), (Thin, 15.0), (Floes, 15.0)],
                false,
                15.0,
            ),
            // A glaze of thin ice from round 5 (24 s after the cast) until
            // the spell ends, then floes for 15 s.
            Self::StormOfVengeance => Row {
                concentration: true,
                delay: 24.0,
                ..row(60.0, &[(Thin, 36.0), (Floes, 15.0)], false, 2.0)
            },
        }
    }
}

/// The water a freeze covers, in the (x, z) plane.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Disc {
        center: DVec2,
        radius: f64,
    },
    /// The SRD cone: as wide at each distance as it is long there.
    Cone {
        apex: DVec2,
        dir: DVec2,
        length: f64,
    },
}

impl Shape {
    #[must_use]
    pub fn contains(&self, p: DVec2) -> bool {
        match *self {
            Self::Disc { center, radius } => p.distance(center) <= radius,
            Self::Cone { apex, dir, length } => {
                let dir = dir.normalize_or(DVec2::X);
                let local = p - apex;
                let along = local.dot(dir);
                along >= 0.0 && along <= length && local.perp_dot(dir).abs() <= along * 0.5
            }
        }
    }

    /// Discs that cover it, for the flow-stilling ice events.
    #[must_use]
    pub fn discs(&self) -> Vec<(DVec2, f64)> {
        match *self {
            Self::Disc { center, radius } => vec![(center, radius)],
            Self::Cone { apex, dir, length } => {
                let dir = dir.normalize_or(DVec2::X);
                (0..4)
                    .map(|k| {
                        let along = length * (k as f64 + 0.5) / 4.0;
                        (apex + dir * along, along * 0.5 + length / 8.0)
                    })
                    .collect()
            }
        }
    }

    fn validate(&self) -> bool {
        match *self {
            Self::Disc { center, radius } => center.is_finite() && (0.0..=200.0).contains(&radius),
            Self::Cone { apex, dir, length } => {
                apex.is_finite() && dir.is_finite() && (0.0..=200.0).contains(&length)
            }
        }
    }
}

/// One freeze: its area, its stages, and the cells broken out of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub id: u64,
    pub spell: Freeze,
    pub cast: u64,
    pub body: WaterId,
    pub shape: Shape,
    /// The tick it was cast.
    pub start: u64,
    /// Each stage and the tick it ends.
    pub stages: Vec<(IceState, u64)>,
    pub thickness_cm: f64,
    /// The caster's spell save DC, for the Strength saves of the caught.
    pub dc: i32,
    /// The flow-stilling event, if any.
    pub event: Option<u64>,
    /// 3 m cells broken open.
    pub broken: BTreeSet<(i32, i32)>,
    /// Damage dealt to cells, hit points.
    pub damage: BTreeMap<(i32, i32), f64>,
}

impl Patch {
    /// When the ice forms.
    #[must_use]
    pub fn formed(&self) -> u64 {
        self.start + ticks(self.spell.row().delay)
    }

    /// The stage at `tick`, before the water's own rules.
    #[must_use]
    pub fn stage(&self, tick: u64) -> Option<IceState> {
        if tick < self.formed() {
            return None;
        }
        self.stages
            .iter()
            .find(|(_, end)| tick < *end)
            .map(|(state, _)| *state)
    }

    /// The tick the last stage ends: open water from then.
    #[must_use]
    pub fn open(&self) -> u64 {
        self.stages.last().map_or(self.start, |(_, end)| *end)
    }

    /// How set in the ice is at `tick`, 0 to 1, for drawing: dissolving
    /// slush fades over its last stage.
    #[must_use]
    pub fn amount(&self, tick: u64) -> f64 {
        let row = self.spell.row();
        let Some(state) = self.stage(tick) else {
            return 0.0;
        };
        let last = self.stages.len() - 1;
        let index = self.stages.iter().position(|(_, end)| tick < *end);
        let base = match state {
            IceState::Slush => 0.35,
            IceState::Floes => 0.6,
            IceState::Thin => 0.8,
            IceState::Walkable => 1.0,
        };
        if row.dissolves && index == Some(last) {
            let begin = if last == 0 {
                self.formed()
            } else {
                self.stages[last - 1].1
            };
            let end = self.stages[last].1;
            let t = (tick - begin) as f64 / (end - begin).max(1) as f64;
            return base * (1.0 - t);
        }
        base
    }
}

/// A hole fire melted at a tick, which opens every older patch under it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hole {
    pub center: DVec2,
    pub radius: f64,
    pub tick: u64,
}

/// A creature the ice holds at the surface: Restrained until a Strength
/// save or until the ice turns thin.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Held {
    pub patch: u64,
    pub dc: i32,
    /// The tick of its next save.
    pub next: u64,
}

/// What ice under a load does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bearing {
    /// No ice here.
    Open,
    /// Slush or a floe too small: the load is in the water.
    Sinks,
    Holds,
    /// The load broke the cell; it is open water now.
    Breaks,
}

/// What damage an ice cell takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Damage {
    Fire,
    Poison,
    Psychic,
    Other,
}

/// The zone's ice and the creatures it holds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ice {
    pub patches: Vec<Patch>,
    pub holes: Vec<Hole>,
    /// Creatures caught at the surface.
    pub held: BTreeMap<u64, Held>,
    /// Creatures and the tick they last made a Slippery Ice check.
    pub slipped: BTreeMap<u64, u64>,
    /// Creatures Prone from a slip, until a tick.
    pub prone: BTreeMap<u64, u64>,
    /// Creatures Cone of Cold killed in the water: frozen statues held by
    /// a patch until it thaws.
    pub statues: BTreeMap<u64, u64>,
}

/// The 3 m cell holding `p`.
#[must_use]
pub fn cell(p: DVec2) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

/// Thin ice's tolerance for `cell` in a zone of `seed`, kg: 3d10 × 10 lb
/// (SRD 5.2.1 Thin Ice), rolled from the seed and the cell so every client
/// and every later freeze agree.
#[must_use]
pub fn tolerance(seed: u64, cell: (i32, i32)) -> f64 {
    let mut state = seed
        ^ (cell.0 as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (cell.1 as u32 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    let mut roll = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) % 10 + 1
    };
    let dice = roll() + roll() + roll();
    dice as f64 * 10.0 * POUND
}

impl Ice {
    /// Freezes `shape` on `body` with `spell` at `tick`; `catch` are the
    /// creatures in the water there. Returns the patch's id.
    #[allow(clippy::too_many_arguments)]
    pub fn freeze(
        &mut self,
        basin: &Basin,
        events: &mut Events,
        event_id: u64,
        spell: Freeze,
        shape: Shape,
        cast: u64,
        dc: i32,
        tick: u64,
    ) -> Result<u64, String> {
        if !shape.validate() {
            return Err("Invalid freeze area".into());
        }
        if self.patches.len() >= MAX_PATCHES {
            return Err("Too much ice".into());
        }
        let probe = match shape {
            Shape::Disc { center, .. } => center,
            Shape::Cone { apex, dir, length } => apex + dir.normalize_or(DVec2::X) * length * 0.5,
        };
        let body = basin
            .sample_base(probe, tick)
            .map(|s| s.body)
            .or_else(|| {
                shape
                    .discs()
                    .iter()
                    .find_map(|(c, _)| basin.sample_base(*c, tick).map(|s| s.body))
            })
            .ok_or("No water there to freeze")?;
        let row = spell.row();
        let mut at = tick + ticks(row.delay);
        let stages = row
            .stages
            .iter()
            .map(|(state, s)| {
                at += ticks(*s);
                (*state, at)
            })
            .collect::<Vec<_>>();
        // The flow stills where the ice is solid: thin or walkable, on
        // water that isn't flowing.
        let solid: f64 = row
            .stages
            .iter()
            .take_while(|(s, _)| matches!(s, IceState::Walkable | IceState::Thin))
            .map(|(_, s)| *s)
            .sum();
        let still = basin
            .sample_base(probe, tick)
            .is_none_or(|s| s.flow.length() <= FLOWING);
        let mut event = None;
        if solid > 0.0 && still {
            for (k, (center, radius)) in shape.discs().into_iter().enumerate() {
                let id = event_id + k as u64 * (1 << 20);
                events.insert(physics::water::Event {
                    id,
                    body,
                    start: tick + ticks(row.delay),
                    ramp: ticks(1.0),
                    hold: if row.concentration {
                        u64::MAX
                    } else {
                        ticks(solid - 1.0)
                    },
                    effect: Effect::Ice { center, radius },
                });
                event.get_or_insert(id);
            }
        }
        let patch = Patch {
            id: event_id,
            spell,
            cast,
            body,
            shape,
            start: tick,
            stages,
            thickness_cm: row.thickness_cm,
            dc,
            event,
            broken: BTreeSet::new(),
            damage: BTreeMap::new(),
        };
        self.patches.push(patch);
        Ok(event_id)
    }

    /// Catches the creatures `waders` whose heads are above water where
    /// the ice is walkable at `tick` (ours): Restrained until a Strength
    /// save every 6 s, or until it turns thin. Returns those caught.
    pub fn catch(&mut self, basin: &Basin, waders: &[Wader], tick: u64) -> Vec<u64> {
        let mut caught = Vec::new();
        for w in waders {
            let p = w.xz();
            let Some(top) = basin.sample_base(p, tick) else {
                continue;
            };
            let in_water = w.feet.y < top.height - 0.05 && w.head() > top.height;
            if !in_water || w.boat {
                continue;
            }
            if let Some((IceState::Walkable, patch)) = self.state_with(basin, p, tick)
                && !self.held.contains_key(&w.id)
            {
                let dc = self
                    .patches
                    .iter()
                    .find(|q| q.id == patch)
                    .map_or(crate::spells::SPELL_SAVE_DC, |q| q.dc);
                self.held.insert(
                    w.id,
                    Held {
                        patch,
                        dc,
                        next: tick + ticks(crate::spells::ROUND.into()),
                    },
                );
                caught.push(w.id);
            }
        }
        caught
    }

    /// The ice state over `p` at `tick` and the patch that sets it: the
    /// strongest of the patches there, after the water's rules (flowing
    /// water only slushes; walkable ice on a swell forms as floes).
    #[must_use]
    pub fn state_with(&self, basin: &Basin, p: DVec2, tick: u64) -> Option<(IceState, u64)> {
        let mut best: Option<(IceState, u64, u64)> = None;
        for patch in &self.patches {
            if !patch.shape.contains(p) || patch.broken.contains(&cell(p)) {
                continue;
            }
            if self.holes.iter().any(|h| {
                h.tick >= patch.start && h.tick <= tick && p.distance(h.center) <= h.radius
            }) {
                continue;
            }
            let Some(mut state) = patch.stage(tick) else {
                continue;
            };
            let Some(water) = basin.sample_base(p, tick) else {
                continue;
            };
            if water.body != patch.body {
                continue;
            }
            if DVec2::new(water.flow.x, water.flow.z).length() > FLOWING {
                state = IceState::Slush;
            } else if state == IceState::Walkable && basin.swell(patch.body) > SWELL {
                state = IceState::Floes;
            }
            let end = patch.open();
            if best.is_none_or(|(s, e, _)| (state, end) > (s, e)) {
                best = Some((state, end, patch.id));
            }
        }
        best.map(|(s, _, id)| (s, id))
    }

    /// The ice state over `p` at `tick`, if any.
    #[must_use]
    pub fn state(&self, basin: &Basin, p: DVec2, tick: u64) -> Option<IceState> {
        self.state_with(basin, p, tick).map(|(s, _)| s)
    }

    /// Whether `p` is Slippery Ice at `tick`: walkable, past any hail.
    #[must_use]
    pub fn slippery(&self, basin: &Basin, p: DVec2, tick: u64) -> bool {
        match self.state_with(basin, p, tick) {
            Some((IceState::Walkable, id)) => self
                .patches
                .iter()
                .find(|q| q.id == id)
                .is_none_or(|q| tick >= q.formed() + ticks(q.spell.row().hail)),
            _ => false,
        }
    }

    /// Whether moving over `p` at `tick` is Difficult Terrain: slush to
    /// swim through, hail, or Slippery Ice.
    #[must_use]
    pub fn difficult(&self, basin: &Basin, p: DVec2, tick: u64) -> bool {
        matches!(
            self.state(basin, p, tick),
            Some(IceState::Slush | IceState::Walkable)
        )
    }

    /// Whether a floating body at `p` is held kinematic at `tick`: until
    /// its ice turns thin.
    #[must_use]
    pub fn pins(&self, basin: &Basin, p: DVec2, tick: u64) -> bool {
        self.state(basin, p, tick) == Some(IceState::Walkable)
    }

    /// Whether a creature under water at `p` meets ice above it at `tick`
    /// and can't surface there.
    #[must_use]
    pub fn ceiling(&self, basin: &Basin, p: DVec2, tick: u64) -> bool {
        self.pins(basin, p, tick)
    }

    /// A load of `kg` on the ice at `p` at `tick`: thin ice breaks over its
    /// cell's rolled tolerance, a floe bears one Medium creature, and slush
    /// never holds anyone.
    pub fn bear(&mut self, basin: &Basin, p: DVec2, kg: f64, tick: u64) -> Bearing {
        let Some((state, id)) = self.state_with(basin, p, tick) else {
            return Bearing::Open;
        };
        match state {
            IceState::Slush => Bearing::Sinks,
            IceState::Floes if kg > FLOE_LOAD => Bearing::Sinks,
            IceState::Floes | IceState::Walkable => Bearing::Holds,
            IceState::Thin => {
                if kg <= tolerance(basin.seed, cell(p)) {
                    Bearing::Holds
                } else {
                    if let Some(patch) = self.patches.iter_mut().find(|q| q.id == id) {
                        patch.broken.insert(cell(p));
                    }
                    Bearing::Breaks
                }
            }
        }
    }

    /// An attack on the ice cell at `p` that totals `attack` and deals
    /// `damage` of `kind`: an object with AC 13 and 1 hit point a
    /// centimeter, Vulnerability to Fire, and Immunity to Poison and
    /// Psychic (ours). True when it broke the cell open.
    pub fn strike(
        &mut self,
        basin: &Basin,
        p: DVec2,
        attack: i32,
        damage: f64,
        kind: Damage,
        tick: u64,
    ) -> bool {
        let Some((state, id)) = self.state_with(basin, p, tick) else {
            return false;
        };
        if !matches!(state, IceState::Thin | IceState::Walkable) || attack < CELL_AC {
            return false;
        }
        let dealt = match kind {
            Damage::Fire => damage * 2.0,
            Damage::Poison | Damage::Psychic => 0.0,
            Damage::Other => damage,
        };
        let Some(patch) = self.patches.iter_mut().find(|q| q.id == id) else {
            return false;
        };
        let c = cell(p);
        let taken = patch.damage.entry(c).or_insert(0.0);
        *taken += dealt;
        if *taken >= patch.thickness_cm {
            patch.broken.insert(c);
            patch.damage.remove(&c);
            return true;
        }
        false
    }

    /// Fire whose area is the disc at `center` touches the ice at `tick`:
    /// the ice there turns to water at once (ours). Returns how many
    /// patches it melted into.
    pub fn melt(&mut self, center: DVec2, radius: f64, tick: u64) -> usize {
        let touched = self
            .patches
            .iter()
            .filter(|p| {
                p.start <= tick
                    && p.shape
                        .discs()
                        .iter()
                        .any(|(c, r)| c.distance(center) <= r + radius)
            })
            .count();
        if touched > 0 && self.holes.len() < 64 {
            self.holes.push(Hole {
                center,
                radius,
                tick,
            });
        }
        touched
    }

    /// Each held creature's Strength save comes due every 6 s; a success
    /// frees it, and ice that is no longer walkable frees it at once.
    /// `strength` gives each creature's Strength save modifier.
    pub fn tick_held(
        &mut self,
        basin: &Basin,
        dice: &mut Dice,
        at: impl Fn(u64) -> Option<DVec2>,
        strength: impl Fn(u64) -> i32,
        tick: u64,
    ) -> Vec<(u64, Option<Save>)> {
        let mut freed = Vec::new();
        let ids: Vec<u64> = self.held.keys().copied().collect();
        for id in ids {
            let held = self.held[&id];
            let still = at(id).is_some_and(|p| self.pins(basin, p, tick));
            if !still {
                self.held.remove(&id);
                freed.push((id, None));
                continue;
            }
            if tick >= held.next {
                let save = dice.save(id, "Strength", strength(id), held.dc);
                if save.success {
                    self.held.remove(&id);
                    freed.push((id, Some(save)));
                } else if let Some(h) = self.held.get_mut(&id) {
                    h.next = tick + ticks(crate::spells::ROUND.into());
                }
            }
        }
        freed
    }

    /// A creature moves onto `p` at `tick`: the first time in each 6 s on
    /// Slippery Ice, a DC 10 Dexterity (Acrobatics) check or Prone.
    pub fn step_onto(
        &mut self,
        basin: &Basin,
        dice: &mut Dice,
        creature: u64,
        dexterity: i32,
        p: DVec2,
        tick: u64,
    ) -> Option<Save> {
        if !self.slippery(basin, p, tick) {
            return None;
        }
        let round = ticks(crate::spells::ROUND.into());
        if self
            .slipped
            .get(&creature)
            .is_some_and(|last| tick < last + round)
        {
            return None;
        }
        self.slipped.insert(creature, tick);
        let check = dice.save(creature, "Acrobatics", dexterity, SLIP_DC);
        if !check.success {
            self.prone.insert(creature, tick + ticks(PRONE));
        }
        Some(check)
    }

    /// Whether `creature` is Prone from a slip at `tick`.
    #[must_use]
    pub fn is_prone(&self, creature: u64, tick: u64) -> bool {
        self.prone.get(&creature).is_some_and(|until| tick < *until)
    }

    /// A creature Cone of Cold killed in the water at `p` becomes a frozen
    /// statue held by the ice there.
    pub fn statue(&mut self, basin: &Basin, creature: u64, p: DVec2, tick: u64) -> bool {
        match self.state_with(basin, p, tick) {
            Some((IceState::Walkable, id)) => {
                self.statues.insert(creature, id);
                true
            }
            _ => false,
        }
    }

    /// Ends the concentration ice of `cast` at `tick`: its held stage ends
    /// now and the thaw follows; a glaze that never formed never does.
    pub fn end_cast(&mut self, cast: u64, tick: u64, events: &mut Events) {
        for patch in &mut self.patches {
            if patch.cast != cast || !patch.spell.row().concentration {
                continue;
            }
            let formed = patch.formed();
            if tick < formed {
                patch.stages.clear();
                patch.stages.push((IceState::Slush, patch.start));
            } else if let Some(first) = patch.stages.first().map(|(_, end)| *end)
                && tick < first
            {
                let cut = first - tick;
                for stage in &mut patch.stages {
                    stage.1 -= cut;
                }
            }
            if let Some(id) = patch.event {
                let mut k = 0;
                while events.end(id + k * (1 << 20), tick.max(formed)) {
                    k += 1;
                }
            }
        }
    }

    /// Drops thawed patches and frees what they held.
    pub fn prune(&mut self, tick: u64) {
        self.patches.retain(|p| p.open() > tick);
        let live: BTreeSet<u64> = self.patches.iter().map(|p| p.id).collect();
        self.held.retain(|_, h| live.contains(&h.patch));
        self.statues.retain(|_, p| live.contains(p));
        self.prone.retain(|_, until| *until > tick);
        let round = ticks(crate::spells::ROUND.into());
        self.slipped.retain(|_, at| at.saturating_add(round) > tick);
        let first = self.patches.iter().map(|p| p.start).min();
        self.holes.retain(|h| first.is_some_and(|f| h.tick >= f));
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.patches.len() > MAX_PATCHES
            || self.holes.len() > 64
            || self.held.len() > 256
            || self.slipped.len() > 256
            || self.prone.len() > 256
            || self.statues.len() > 256
            || self.patches.iter().any(|p| {
                !p.shape.validate()
                    || p.stages.is_empty()
                    || p.stages.len() > 4
                    || p.broken.len() > 4096
                    || p.damage.len() > 4096
                    || !p.thickness_cm.is_finite()
            })
        {
            return Err("Invalid ice checkpoint".into());
        }
        Ok(())
    }
}

/// How long `spell`'s ice lasts from cast to open water, s, when a
/// concentration spell is held to its end.
#[must_use]
pub fn lifetime(spell: Freeze) -> f64 {
    let row = spell.row();
    row.delay + row.stages.iter().map(|(_, s)| s).sum::<f64>()
}

/// Seconds since `tick0` at `tick`, for logs.
#[must_use]
pub fn since(tick0: u64, tick: u64) -> f64 {
    seconds(tick.saturating_sub(tick0))
}
