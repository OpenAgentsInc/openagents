//! Control Water (SRD 5.2.1, level 4): water in a cube up to 100 feet
//! (30 m) on a side, Concentration up to 10 minutes, one of four modes at a
//! time (`docs/verse/water.md`, Control Water).
//!
//! - **Flood.** A body smaller than the cube rises by up to 6 m and falls
//!   back over 6 s when the mode ends (ours). In a larger body a solitary
//!   6 m wave crosses the cube in 6 s (ours), carrying rowboats, and each
//!   boat it strikes has the SRD's 25 percent chance to capsize.
//! - **Part Water.** A trench 3 m wide (ours) along the caster's facing,
//!   the cube's length, down to the bed; swimmers in its path move to the
//!   nearer wall; it refills over 6 s.
//! - **Redirect Flow.** The current in the cube runs the chosen way at the
//!   body's peak speed, at least 1 m/s (ours on still water).
//! - **Whirlpool.** Needs 50 feet square and 25 feet deep (15 m by 7.5 m);
//!   it pulls swimmers within 25 feet of it 10 feet a round, deals 2d8
//!   bludgeoning on entry and each round inside (half on a Strength save),
//!   and a swimmer leaves with a Strength (Athletics) check.
//!
//! Each mode is a `physics::water` event, so clients replicate it from the
//! tick; ice cells are skipped because ice isn't water (ice stills the
//! current under it).

use glam::{DVec2, DVec3};
use physics::water::{Effect, Kind, Outline};
use serde::{Deserialize, Serialize};

use super::{Basin, Wader, WaterSpells, ice::IceState, ticks};
use crate::spells::{Dice, FEET, Save};

/// The cube's largest side, m: 100 feet.
pub const SIDE: f64 = 100.0 * FEET;
/// The most a flood raises the level, m: 20 feet.
pub const FLOOD: f64 = 20.0 * FEET;
/// How long the flood takes to fall back, the wave to cross, and the
/// trench to refill, s: one round (ours, the SRD's "next round").
pub const SETTLE: f64 = 6.0;
/// The trench's width, m (ours).
pub const TRENCH: f64 = 3.0;
/// Redirect Flow's slowest current, m/s (ours).
pub const MIN_FLOW: f64 = 1.0;
/// The whirlpool's least area and depth, m (SRD 5.2.1: 50 feet square and
/// 25 feet deep).
pub const WHIRL_SQUARE: f64 = 50.0 * FEET;
pub const WHIRL_DEPTH: f64 = 25.0 * FEET;
/// Its width at the base and at the top, and how far from it it pulls, m.
pub const WHIRL_BASE: f64 = 5.0 * FEET;
pub const WHIRL_TOP: f64 = 50.0 * FEET;
pub const WHIRL_REACH: f64 = 25.0 * FEET;
/// How fast it pulls, m/s: 10 feet every 6 s.
pub const WHIRL_PULL: f64 = 10.0 * FEET / 6.0;
/// How long a swimmer holds the swim-away input to make the escape check,
/// s (ours).
pub const ESCAPE_HOLD: f64 = 1.0;
/// The chance a struck boat capsizes, percent (SRD 5.2.1).
pub const CAPSIZE: u32 = 25;
/// The longest the spell lasts, s.
pub const DURATION: f64 = 600.0;

/// The four modes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    /// Raise the water by `rise` m, at most 6 m.
    Flood {
        rise: f64,
    },
    Part,
    Redirect,
    Whirlpool,
}

impl Mode {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Flood { .. } => "Flood",
            Self::Part => "Part Water",
            Self::Redirect => "Redirect Flow",
            Self::Whirlpool => "Whirlpool",
        }
    }
}

/// Why a cast refuses, with the reason the caster reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No water at the cube's center.
    NoWater,
    /// The cube's center is ice.
    Ice,
    /// The water isn't 15 m square for a whirlpool.
    TooSmall,
    /// The water isn't 7.5 m deep for a whirlpool.
    TooShallow,
}

impl Refusal {
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::NoWater => "There is no water there to control",
            Self::Ice => "Ice isn't water; Control Water skips it",
            Self::TooSmall => "A whirlpool needs water at least 50 feet square",
            Self::TooShallow => "A whirlpool needs water at least 25 feet deep",
        }
    }
}

/// What the flood does in this body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Flood {
    /// The body is smaller than the cube: its level rises.
    Rise,
    /// The body is larger: a wave crosses the cube.
    Wave,
}

/// A live Control Water.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Control {
    pub cast: u64,
    pub caster: u64,
    pub mode: Mode,
    pub body: physics::water::WaterId,
    /// The cube's center, on the surface, and its side.
    pub center: DVec3,
    pub side: f64,
    /// The caster's facing, unit, in (x, z).
    pub dir: DVec2,
    pub dc: i32,
    pub start: u64,
    /// When the spell runs out.
    pub until: u64,
    pub flood: Option<Flood>,
    /// The mode's water events.
    pub events: Vec<u64>,
    /// Boats a wave already struck.
    pub struck: Vec<u64>,
    /// Swimmers inside the whirlpool, and the tick of their next 2d8.
    pub inside: Vec<(u64, u64)>,
    /// Swimmers holding the swim-away input, since a tick.
    pub escaping: Vec<(u64, u64)>,
    /// Swimmers who escaped, free of the pull until a tick.
    pub freed: Vec<(u64, u64)>,
}

impl Control {
    /// The cube's footprint contains `p`.
    #[must_use]
    pub fn covers(&self, p: DVec2) -> bool {
        let d = (p - DVec2::new(self.center.x, self.center.z)).abs();
        d.x <= self.side * 0.5 && d.y <= self.side * 0.5
    }

    /// Ends the mode at `tick`: its events ramp out over [`SETTLE`].
    pub fn end(&self, events: &mut physics::water::Events, tick: u64) {
        for id in &self.events {
            events.end(*id, tick);
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if !self.center.is_finite()
            || !self.dir.is_finite()
            || !(0.0..=SIDE + 1e-9).contains(&self.side)
            || self.events.len() > 8
            || self.struck.len() > 64
            || self.inside.len() > 64
            || self.escaping.len() > 64
            || self.freed.len() > 64
        {
            return Err("Invalid Control Water checkpoint".into());
        }
        Ok(())
    }

    /// The whirlpool's radius at depth `below` m under the surface: 5 feet
    /// at its base 25 feet down, 50 feet across at the top.
    #[must_use]
    pub fn funnel(below: f64) -> f64 {
        let t = (below / WHIRL_DEPTH).clamp(0.0, 1.0);
        let top = WHIRL_TOP * 0.5;
        let base = WHIRL_BASE * 0.5;
        top + (base - top) * t
    }
}

/// What a step of the whirlpool did to one swimmer.
#[derive(Clone, Debug, PartialEq)]
pub enum Whirl {
    /// Pulled toward the eye: its new feet.
    Pulled { creature: u64, to: DVec3 },
    /// 2d8 bludgeoning, half on a Strength save.
    Battered {
        creature: u64,
        damage: u32,
        save: Save,
    },
    /// The Strength (Athletics) check to leave.
    Escape { creature: u64, check: Save },
}

/// The outline bounds of `body`, if it has any.
fn bounded(basin: &Basin, body: physics::water::WaterId) -> Option<(DVec2, DVec2)> {
    let b = basin.water.get(body)?;
    if b.kind == Kind::Ocean {
        return None;
    }
    match &b.outline {
        Outline::Everywhere => None,
        outline => outline.bounds(),
    }
}

impl WaterSpells {
    /// Casts Control Water's `mode` in a cube of `side` m centered on
    /// `center`, the caster facing `dir`, at `tick`. Switching modes ends
    /// the previous one. `swimmers` are moved out of a trench's path.
    #[allow(clippy::too_many_arguments)]
    pub fn control_water(
        &mut self,
        basin: &Basin,
        cast: u64,
        caster: u64,
        mode: Mode,
        center: DVec2,
        side: f64,
        dir: DVec2,
        dc: i32,
        tick: u64,
    ) -> Result<Control, Refusal> {
        let side = side.clamp(1.0, SIDE);
        let dir = dir.normalize_or(DVec2::Y);
        let water = basin.sample_base(center, tick).ok_or(Refusal::NoWater)?;
        if matches!(
            self.ice.state(basin, center, tick),
            Some(IceState::Thin | IceState::Walkable)
        ) {
            return Err(Refusal::Ice);
        }
        if mode == Mode::Whirlpool {
            whirlpool_fits(basin, center, water.body)?;
        }
        let previous = self.control.take();
        if let Some(previous) = &previous {
            previous.end(&mut self.events, tick);
        }
        let (start, until) = match previous {
            Some(p) if p.cast == cast => (p.start, p.until),
            _ => (tick, tick + ticks(DURATION)),
        };
        let mut control = Control {
            cast,
            caster,
            mode,
            body: water.body,
            center: DVec3::new(center.x, water.height, center.y),
            side,
            dir,
            dc,
            start,
            until,
            flood: None,
            events: Vec::new(),
            struck: Vec::new(),
            inside: Vec::new(),
            escaping: Vec::new(),
            freed: Vec::new(),
        };
        let settle = ticks(SETTLE);
        let half = side * 0.5;
        let effect = match mode {
            Mode::Flood { rise } => {
                let rise = rise.clamp(0.0, FLOOD);
                let small = bounded(basin, water.body).is_some_and(|(lo, hi)| {
                    let size = hi - lo;
                    size.x <= side && size.y <= side
                });
                if small {
                    control.flood = Some(Flood::Rise);
                    (settle, u64::MAX, Effect::Level { rise })
                } else {
                    control.flood = Some(Flood::Wave);
                    (
                        1,
                        settle,
                        Effect::Surge {
                            origin: center - dir * half,
                            dir,
                            half_width: half,
                            length: side,
                            height: FLOOD,
                        },
                    )
                }
            }
            Mode::Part => (
                ticks(1.0),
                u64::MAX,
                Effect::Trench {
                    a: center - dir * half,
                    b: center + dir * half,
                    half_width: TRENCH * 0.5,
                    depth: side,
                },
            ),
            Mode::Redirect => {
                let speed = basin.peak_flow(water.body).max(MIN_FLOW);
                (
                    ticks(1.0),
                    u64::MAX,
                    Effect::Current {
                        center,
                        half,
                        velocity: dir * speed,
                    },
                )
            }
            Mode::Whirlpool => (
                ticks(2.0),
                u64::MAX,
                Effect::Vortex {
                    center,
                    radius: WHIRL_TOP * 0.5 + WHIRL_REACH,
                    pull: WHIRL_PULL,
                    swirl: 2.0,
                    depth: 3.0,
                },
            ),
        };
        let (ramp, hold, effect) = effect;
        // A ramp of one round, so the water falls back or refills over 6 s
        // when the mode ends.
        let ramp = if matches!(mode, Mode::Part | Mode::Redirect) {
            settle.max(ramp)
        } else {
            ramp
        };
        if let Ok(id) = self.start(water.body, tick, ramp, hold, effect) {
            control.events.push(id);
        }
        self.control = Some(control.clone());
        Ok(control)
    }

    /// Where Part Water moves a swimmer at `feet` in its trench's path: to
    /// the nearer wall, half a meter past it. None outside the path.
    #[must_use]
    pub fn part_push(&self, feet: DVec3) -> Option<DVec3> {
        let control = self.control.as_ref()?;
        if control.mode != Mode::Part {
            return None;
        }
        let local = DVec2::new(feet.x - control.center.x, feet.z - control.center.z);
        if local.dot(control.dir).abs() > control.side * 0.5 {
            return None;
        }
        let across = local.perp_dot(control.dir);
        let half = TRENCH * 0.5;
        if across.abs() > half {
            return None;
        }
        let side = if across >= 0.0 { 1.0 } else { -1.0 };
        // The way `across` grows.
        let normal = DVec2::new(control.dir.y, -control.dir.x);
        let shift = normal * side * (half + 0.5 - across.abs());
        Some(DVec3::new(feet.x + shift.x, feet.y, feet.z + shift.y))
    }

    /// The flood wave strikes the boats at `boats` (id and position) at
    /// `tick`: each one it reaches for the first time is carried, and has a
    /// 25 percent chance to capsize. Returns (boat, capsized, the wave's
    /// velocity to carry it at).
    pub fn wave_strikes(
        &mut self,
        basin: &Basin,
        dice: &mut Dice,
        boats: &[(u64, DVec3)],
        tick: u64,
    ) -> Vec<(u64, bool, DVec2)> {
        let Some(control) = self.control.as_mut() else {
            return Vec::new();
        };
        if control.flood != Some(Flood::Wave) {
            return Vec::new();
        }
        let speed = control.side / SETTLE;
        let carry = control.dir * speed;
        let mut out = Vec::new();
        for &(boat, at) in boats {
            let p = DVec2::new(at.x, at.z);
            let lifted = self
                .events
                .list()
                .iter()
                .filter(|e| control.events.contains(&e.id))
                .map(|e| e.rise(p, tick))
                .sum::<f64>();
            if lifted < 1.0 {
                continue;
            }
            if control.struck.contains(&boat) {
                out.push((boat, false, carry));
                continue;
            }
            if control.struck.len() < 64 {
                control.struck.push(boat);
            }
            let capsized = dice.roll(100) <= CAPSIZE;
            out.push((boat, capsized, carry));
        }
        let _ = basin;
        out
    }

    /// Steps the whirlpool for `dt` s at `tick`: swimmers within reach of
    /// it are pulled toward its eye at 10 feet a round; those inside take
    /// 2d8 bludgeoning on entry and every round (half on a Strength save);
    /// a swimmer holding swim-away (`away`) for a second makes the
    /// Strength (Athletics) check to leave. `strength` gives each
    /// creature's modifier.
    #[allow(clippy::too_many_arguments)]
    pub fn whirl(
        &mut self,
        basin: &Basin,
        dice: &mut Dice,
        swimmers: &[Wader],
        away: &[u64],
        strength: impl Fn(u64) -> i32,
        dt: f64,
        tick: u64,
    ) -> Vec<Whirl> {
        let mut out = Vec::new();
        let ice = self.ice.clone();
        let Some(control) = self.control.as_mut() else {
            return out;
        };
        if control.mode != Mode::Whirlpool {
            return out;
        }
        let eye = DVec2::new(control.center.x, control.center.z);
        let round = ticks(crate::spells::ROUND.into());
        control.freed.retain(|(_, until)| *until > tick);
        control
            .escaping
            .retain(|(id, _)| away.contains(id) && swimmers.iter().any(|s| s.id == *id));
        for s in swimmers {
            let p = s.xz();
            let Some(water) = basin.sample_base(p, tick) else {
                continue;
            };
            if water.body != control.body
                || s.boat
                || s.feet.y >= water.height - 0.05
                || ice
                    .state(basin, p, tick)
                    .is_some_and(|i| i >= IceState::Floes)
            {
                continue;
            }
            if control.freed.iter().any(|(id, _)| *id == s.id) {
                continue;
            }
            let below = (water.height - s.feet.y).max(0.0);
            let r = p.distance(eye);
            let funnel = Control::funnel(below);
            if r > funnel + WHIRL_REACH {
                control.inside.retain(|(id, _)| *id != s.id);
                continue;
            }
            // Escape: a held swim-away input makes the check.
            if away.contains(&s.id) {
                let since = match control.escaping.iter().find(|(id, _)| *id == s.id) {
                    Some((_, since)) => *since,
                    None => {
                        control.escaping.push((s.id, tick));
                        tick
                    }
                };
                if tick >= since + ticks(ESCAPE_HOLD) {
                    control.escaping.retain(|(id, _)| *id != s.id);
                    let check = dice.save(s.id, "Athletics", strength(s.id), control.dc);
                    if check.success {
                        control.inside.retain(|(id, _)| *id != s.id);
                        control.freed.push((s.id, tick + round));
                    }
                    out.push(Whirl::Escape {
                        creature: s.id,
                        check,
                    });
                    continue;
                }
            }
            if r > 1e-6 {
                let step = (WHIRL_PULL * dt).min(r);
                let to = p + (eye - p) / r * step;
                out.push(Whirl::Pulled {
                    creature: s.id,
                    to: DVec3::new(to.x, s.feet.y, to.y),
                });
            }
            if r <= funnel {
                let due = match control.inside.iter().find(|(id, _)| *id == s.id) {
                    Some((_, next)) => tick >= *next,
                    None => true,
                };
                if due {
                    control.inside.retain(|(id, _)| *id != s.id);
                    control.inside.push((s.id, tick + round));
                    let rolled = dice.sum(2, 8);
                    let save = dice.save(s.id, "Strength", strength(s.id), control.dc);
                    let damage = if save.success { rolled / 2 } else { rolled };
                    out.push(Whirl::Battered {
                        creature: s.id,
                        damage,
                        save,
                    });
                }
            }
        }
        out
    }
}

/// Whether a whirlpool fits at `center` in `body`: every point of a 15 m
/// square about it in the same body and at least 7.5 m deep.
pub fn whirlpool_fits(
    basin: &Basin,
    center: DVec2,
    body: physics::water::WaterId,
) -> Result<(), Refusal> {
    let half = WHIRL_SQUARE * 0.5;
    let mut shallow = false;
    for j in 0..=6 {
        for i in 0..=6 {
            let p = center
                + DVec2::new(
                    -half + i as f64 / 6.0 * 2.0 * half,
                    -half + j as f64 / 6.0 * 2.0 * half,
                );
            let Some(water) = basin.sample_base(p, 0) else {
                return Err(Refusal::TooSmall);
            };
            if water.body != body {
                return Err(Refusal::TooSmall);
            }
            if water.height - (basin.bed)(p) < WHIRL_DEPTH {
                shallow = true;
            }
        }
    }
    if shallow {
        return Err(Refusal::TooShallow);
    }
    Ok(())
}
