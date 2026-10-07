//! The spells on Everglade's hotbar that need no enemy: Feather Fall, Wind
//! Wall, Reverse Gravity, and Wall of Stone.
//!
//! Each cast is admitted, and moves the player, through the chamber's
//! native rules in `verse_world`: Feather Fall's drag and landing, Wall of
//! Stone's panel layout and support check, Wind Wall's grounded path and
//! updraft, and Reverse Gravity's cylinder and hover spring. The player is
//! the only creature they act on. They draw as the chamber draws them: Wind
//! Wall's updraft streaks are the chamber's guide lines, and the panels are
//! granite slabs in the chamber's stone color. Reverse Gravity draws only
//! its particles, which fall upward and crowd its edge ([`motes`]).
//!
//! Everglade's hotbar has no cooldowns and no concentration: every press
//! casts ([`Spells::cast_ahead`]). Each Wind Wall stands beside the ones
//! before it, up to [`MAX_WALLS`]. Each Wall of Stone does too, up to
//! [`MAX_STONE_WALLS`], and crumbles after [`STONE_LIFETIME`] or when a
//! newer cast passes the cap: its panels break into chunks that tumble,
//! settle, and shrink away within a few seconds, in the demolition yard's
//! rigid-body debris ([`super::demolition::site`]) with its dust. A wall
//! that would cut through a building or its furniture is refused. Pressing
//! Feather Fall or Reverse Gravity while it holds ends it. The Grove keeps
//! the chamber's concentration rule through [`Spells::concentrate_ahead`].

use super::demolition::hammer;
use super::demolition::site::{Cuboid, Link, Matter, PieceSpec, Puff, Role, Site};
use super::hotbar::Slot;
use super::solids::Solids;
use crate::controller::{AVATAR_HEIGHT, Footprint, PlayerController, RADIUS};
use crate::mesh::{Mesh, Vertex};
use crate::zones::Intent;
use glam::{DVec2, DVec3, Mat4, Vec3};
use std::collections::VecDeque;
pub mod motes;

use verse_world::{
    feather_fall as feather, reverse_gravity as reverse, wall_of_stone as stone, wind_wall as wind,
};

/// The player's identity in the spells' rules.
const PLAYER: u32 = 1;
/// The player's mass for Feather Fall's energy accounting, kg.
const MASS: f64 = 75.0;
/// How far ahead of the caster the walls stand, m, as in the chamber.
pub const AHEAD: f64 = 4.0;
/// Half the stone slab the glade's ground is to Wall of Stone's support
/// check, m.
const SLAB: f64 = 20.0;
/// Spacing and half size of the posts that stand in for a panel's
/// footprint, m: the controller collides with axis-aligned boxes, so a
/// turned panel is a close row of small ones.
const POST_SPACING: f64 = 0.25;
const POST_HALF: f32 = 0.1;
/// How long a stone panel takes to rise into place, drawn only, s.
const RAISE: f64 = 0.35;
/// The longest step of Reverse Gravity's integration, s: its hover spring
/// is stiff.
const SUBSTEP: f64 = 1.0 / 120.0;
/// The chamber's granite color (`prop-stone`).
const GRANITE: [f32; 3] = [0.5, 0.49, 0.46];
/// The upward speed Wind Wall gives a player inside it, m/s: enough to
/// clear the wall's top and rise a few meters over it before falling.
const WIND_LIFT: f32 = 11.0;
/// Feather Fall's drifting feathers.
const FEATHERS: usize = 8;
const FEATHER: [f32; 3] = [0.95, 0.93, 0.86];
/// The most Wind Walls that stand at once; another cast takes down the
/// oldest.
pub const MAX_WALLS: usize = 12;
/// How long a Wall of Stone stands before it crumbles, s: long enough to
/// hide behind or climb, short enough that walls don't pile up in town.
pub const STONE_LIFETIME: f64 = 20.0;
/// The most Walls of Stone that stand at once; another cast crumbles the
/// oldest.
pub const MAX_STONE_WALLS: usize = 4;
/// How long a crumbled wall's chunks last, s, and up to a fifth longer.
pub const DEBRIS_LIFETIME: f64 = 4.0;
/// The end of a chunk's life over which it shrinks to nothing, s.
const DEBRIS_FADE: f64 = 1.2;
/// The most crumbling walls whose debris is kept; past it the oldest
/// debris goes at once.
const MAX_CRUMBLING: usize = MAX_STONE_WALLS;
/// How fast a crumbling panel's chunks slump off its face, m/s.
const SLUMP: f64 = 1.6;
/// Whether this build runs on a browser or a phone, whose debris budget is
/// smaller (`docs/verse/destructible-buildings.md`, Performance budgets).
const SMALL: bool = cfg!(any(
    target_arch = "wasm32",
    target_os = "ios",
    target_os = "android"
));
/// The chunks a panel breaks into, along it and up it.
const CHUNK_GRID: [usize; 2] = if SMALL { [3, 3] } else { [4, 4] };
/// The most dust puffs the crumbling walls draw in a frame, so they fit
/// within the frame's particle budget beside a busy field's spells.
const MAX_DUST: usize = 32;
/// Seed of the crumbling walls' debris spread.
const SEED: u64 = 0x570E_C2B1;

/// A spell on the hotbar, after Levitate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spell {
    FeatherFall,
    WallOfStone,
    WindWall,
    ReverseGravity,
}

impl Spell {
    /// Every spell, in hotbar order.
    pub const ALL: [Self; 4] = [
        Self::FeatherFall,
        Self::WindWall,
        Self::ReverseGravity,
        Self::WallOfStone,
    ];

    /// The chamber catalog's key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::FeatherFall => "feather-fall",
            Self::WallOfStone => "wall-of-stone",
            Self::WindWall => "wind-wall",
            Self::ReverseGravity => "reverse-gravity",
        }
    }

    /// The zone intent that casts it.
    #[must_use]
    pub const fn intent(self) -> Intent {
        match self {
            Self::FeatherFall => Intent::FeatherFall,
            Self::WallOfStone => Intent::WallOfStone,
            Self::WindWall => Intent::WindWall,
            Self::ReverseGravity => Intent::ReverseGravity,
        }
    }

    /// The spell `intent` casts, if any.
    #[must_use]
    pub fn of(intent: Intent) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.intent() == intent)
    }
}

/// One standing wall or field and when it was cast, s.
#[derive(Clone, Debug)]
struct Cast<T> {
    spell: T,
    at: f64,
}

/// A crumbled Wall of Stone: its panels' chunks and dust in the demolition
/// yard's rules, in a frame whose origin is `origin`, the foot of the wall
/// on the lowest ground under it.
#[derive(Clone, Debug)]
struct Crumble {
    site: Site,
    origin: Vec3,
}

impl Crumble {
    /// `panels` broken into their chunks at once, slumping off their faces.
    fn new(panels: &[stone::Placement], seed: u64) -> Self {
        let count = panels.len().max(1) as f64;
        let center = panels.iter().map(|p| p.center).sum::<DVec3>() / count;
        let base = panels
            .iter()
            .map(|p| p.center.y - p.half().y)
            .fold(f64::INFINITY, f64::min);
        let base = if base.is_finite() { base } else { center.y };
        let origin = DVec3::new(center.x, base, center.z);
        let specs = panels
            .iter()
            .map(|panel| {
                let half = panel.half();
                let [along, up] = CHUNK_GRID;
                let step = DVec3::new(2.0 * half.x / along as f64, 2.0 * half.y / up as f64, 0.0);
                let chunks = (0..along)
                    .flat_map(|i| (0..up).map(move |j| (i, j)))
                    .map(|(i, j)| {
                        let min = DVec3::new(
                            -half.x + step.x * i as f64,
                            -half.y + step.y * j as f64,
                            -half.z,
                        );
                        let max = DVec3::new(min.x + step.x, min.y + step.y, half.z);
                        Cuboid::between(min, max)
                    })
                    .collect();
                PieceSpec {
                    building: 0,
                    role: Role::Block { level: 0 },
                    matter: Matter::Plaster,
                    center: panel.center - origin,
                    orientation: panel.orientation,
                    mass: panel.form.mass(),
                    size: half * 2.0,
                    hit_points: 1,
                    colliders: vec![Cuboid::between(-half, half)],
                    chunks,
                    blocks: false,
                    link: Link {
                        footing: true,
                        ..Link::default()
                    },
                }
            })
            .collect();
        let mut site = Site::new(specs, seed);
        site.set_debris_lifetime(DEBRIS_LIFETIME);
        for (index, panel) in panels.iter().enumerate() {
            site.crumble(index, panel.normal(), SLUMP);
        }
        Self {
            site,
            origin: origin.as_vec3(),
        }
    }

    /// How many chunks are left.
    fn chunks(&self) -> usize {
        self.site
            .pieces()
            .iter()
            .flat_map(|p| &p.chunks)
            .filter(|c| !c.gone)
            .count()
    }

    /// The chunks as granite blocks, each shrinking over the end of its
    /// life.
    fn draw(&self, mesh: &mut Mesh) {
        let time = self.site.time();
        let to_world = Mat4::from_translation(self.origin);
        for (spec, piece) in self.site.specs().iter().zip(self.site.pieces()) {
            for (cuboid, chunk) in spec.chunks.iter().zip(&piece.chunks) {
                if chunk.gone {
                    continue;
                }
                let left = ((chunk.until - time) / DEBRIS_FADE).clamp(0.0, 1.0) as f32;
                let pose = to_world * self.site.body_pose(chunk.body);
                let half = cuboid.half.as_vec3() * left;
                shaded_box(mesh, |i| {
                    pose.transform_point3(Vec3::new(
                        if i & 1 == 0 { -half.x } else { half.x },
                        if i & 2 == 0 { -half.y } else { half.y },
                        if i & 4 == 0 { -half.z } else { half.z },
                    ))
                });
            }
        }
    }

    /// The dust puffs, in the world.
    fn puffs(&self) -> impl Iterator<Item = Puff> + '_ {
        self.site.puffs().iter().map(|p| Puff {
            at: p.at + self.origin,
            ..*p
        })
    }
}

/// The player's spells: the clock, the Feather Fall ward, the standing
/// Wind Walls and Walls of Stone, oldest first, the crumbling walls'
/// debris, and Reverse Gravity.
#[derive(Clone, Debug, Default)]
pub struct Spells {
    time: f64,
    /// Whether the Grove asked for free casting; every cast is free now,
    /// so this only round-trips through a long rest.
    free: bool,
    feather: Option<feather::FeatherFall>,
    winds: VecDeque<Cast<wind::Wall>>,
    stones: VecDeque<Cast<Vec<stone::Placement>>>,
    crumbles: VecDeque<Crumble>,
    /// Walls crumbled so far, which seeds each one's debris.
    crumbled: u64,
    reverse: Option<Cast<reverse::Gravity>>,
}

impl Spells {
    /// Lets every cast skip its cooldown, for a demo field such as the
    /// Grove.
    pub fn set_free(&mut self, free: bool) {
        self.free = free;
    }

    /// Whether casts skip their cooldowns.
    #[must_use]
    pub fn free(&self) -> bool {
        self.free
    }

    /// Advances the clock `dt` seconds and ends what has run out. Returns
    /// whether the solids the spells raise changed.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.time += f64::from(dt.max(0.0));
        let now = self.time;
        if self.feather.as_ref().is_some_and(|f| f.ended(now)) {
            self.feather = None;
        }
        self.winds.retain(|c| now < c.at + wind::DURATION);
        if self
            .reverse
            .as_ref()
            .is_some_and(|c| now >= c.at + reverse::DURATION)
        {
            self.reverse = None;
        }
        let mut changed = false;
        while self
            .stones
            .front()
            .is_some_and(|c| now >= c.at + STONE_LIFETIME)
        {
            if let Some(wall) = self.stones.pop_front() {
                self.crumble(&wall.spell);
            }
            changed = true;
        }
        for crumble in &mut self.crumbles {
            crumble.site.tick(dt);
        }
        self.crumbles.retain(|c| !c.site.cleared());
        changed
    }

    /// Breaks the Wall of Stone of `panels` into tumbling chunks.
    fn crumble(&mut self, panels: &[stone::Placement]) {
        self.crumbled += 1;
        let seed = SEED ^ self.crumbled.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.crumbles.push_back(Crumble::new(panels, seed));
        while self.crumbles.len() > MAX_CRUMBLING {
            self.crumbles.pop_front();
        }
    }

    /// How many chunks of crumbled walls are left.
    #[must_use]
    pub fn debris(&self) -> usize {
        self.crumbles.iter().map(Crumble::chunks).sum()
    }

    /// Whether `spell` is live on the player.
    #[must_use]
    pub fn active(&self, spell: Spell) -> bool {
        self.count(spell) > 0
    }

    /// How many of `spell` stand now.
    #[must_use]
    pub fn count(&self, spell: Spell) -> usize {
        match spell {
            Spell::FeatherFall => usize::from(
                self.feather
                    .as_ref()
                    .is_some_and(|f| f.holds(PLAYER, self.time)),
            ),
            Spell::WallOfStone => self.stones.len(),
            Spell::WindWall => self.winds.len(),
            Spell::ReverseGravity => usize::from(self.reverse.is_some()),
        }
    }

    /// `spell`'s hotbar slot for `player` standing on `solids`: enabled
    /// when a press would do something, which ending a live Feather Fall
    /// or Reverse Gravity is.
    #[must_use]
    pub fn slot(&self, spell: Spell, player: &PlayerController, solids: &Solids) -> Slot {
        let active = self.active(spell);
        let ends = active && matches!(spell, Spell::FeatherFall | Spell::ReverseGravity);
        Slot {
            enabled: ends || self.admit(spell, player, solids).is_ok(),
            active,
            cooldown: 0.0,
        }
    }

    /// Casts `spell` for `player` as Everglade's hotbar does.
    ///
    /// # Errors
    ///
    /// Returns the rule the cast breaks.
    #[cfg(test)]
    pub fn cast(
        &mut self,
        spell: Spell,
        player: &PlayerController,
        solids: &Solids,
    ) -> Result<(), String> {
        self.cast_ahead(spell, player, solids, AHEAD)
    }

    /// Casts `spell` for `player` as Everglade's hotbar does, with Wall of
    /// Stone and Wind Wall standing `ahead` meters in front: with no
    /// cooldown, each wall beside the ones standing (the oldest past
    /// [`MAX_WALLS`] goes), and a press on a live Feather Fall or Reverse
    /// Gravity ending it.
    ///
    /// # Errors
    ///
    /// Returns the rule the cast breaks.
    pub fn cast_ahead(
        &mut self,
        spell: Spell,
        player: &PlayerController,
        solids: &Solids,
        ahead: f64,
    ) -> Result<(), String> {
        match spell {
            Spell::FeatherFall if self.active(spell) => self.feather = None,
            Spell::ReverseGravity if self.active(spell) => self.reverse = None,
            _ => {
                let admitted = self.admit_ahead(spell, player, solids, ahead)?;
                self.add(admitted);
            }
        }
        Ok(())
    }

    /// Casts `spell` under the chamber's concentration rule, as the Grove
    /// does: a press on a live Wall of Stone, Wind Wall, or Reverse Gravity
    /// ends it, and casting one ends the others.
    ///
    /// # Errors
    ///
    /// Returns the rule the cast breaks.
    pub fn concentrate_ahead(
        &mut self,
        spell: Spell,
        player: &PlayerController,
        solids: &Solids,
        ahead: f64,
    ) -> Result<(), String> {
        if spell == Spell::FeatherFall {
            if self.active(spell) {
                return Err("Feather Fall already holds you".into());
            }
            let admitted = self.admit_ahead(spell, player, solids, ahead)?;
            self.add(admitted);
            return Ok(());
        }
        if self.active(spell) {
            self.end_concentration();
            return Ok(());
        }
        let admitted = self.admit_ahead(spell, player, solids, ahead)?;
        self.end_concentration();
        self.add(admitted);
        Ok(())
    }

    fn end_concentration(&mut self) {
        self.winds.clear();
        while let Some(wall) = self.stones.pop_front() {
            self.crumble(&wall.spell);
        }
        self.reverse = None;
    }

    fn add(&mut self, admitted: Admitted) {
        let at = self.time;
        match admitted {
            Admitted::Feather(effect) => self.feather = Some(effect),
            Admitted::Wind(spell) => {
                self.winds.push_back(Cast { spell, at });
                while self.winds.len() > MAX_WALLS {
                    self.winds.pop_front();
                }
            }
            Admitted::Stone(spell) => {
                self.stones.push_back(Cast { spell, at });
                while self.stones.len() > MAX_STONE_WALLS {
                    if let Some(wall) = self.stones.pop_front() {
                        self.crumble(&wall.spell);
                    }
                }
            }
            Admitted::Reverse(spell) => self.reverse = Some(Cast { spell, at }),
        }
    }

    /// The rules' admission of `spell` cast now by `player`.
    fn admit(
        &self,
        spell: Spell,
        player: &PlayerController,
        solids: &Solids,
    ) -> Result<Admitted, String> {
        self.admit_ahead(spell, player, solids, AHEAD)
    }

    fn admit_ahead(
        &self,
        spell: Spell,
        player: &PlayerController,
        solids: &Solids,
        ahead: f64,
    ) -> Result<Admitted, String> {
        let feet = player.pos.as_dvec3();
        let forward = player.forward().as_dvec3();
        let at = self.time;
        match spell {
            Spell::FeatherFall => {
                let center = feet + DVec3::Y * (f64::from(AVATAR_HEIGHT) * 0.5);
                let candidate = feather::Candidate {
                    id: PLAYER,
                    kind: feather::Kind::Creature,
                    position: center,
                    velocity: DVec3::Y * f64::from(player.vertical_speed()),
                    supported: !player.airborne(),
                };
                feather::FeatherFall::cast(PLAYER, center, &[candidate], &[PLAYER], at)
                    .map(Admitted::Feather)
                    .map_err(|refusal| match refusal {
                        feather::Refusal::NotFalling(_) => {
                            "Feather Fall is a reaction: cast it while falling".into()
                        }
                        refusal => format!("Feather Fall refused: {refusal:?}"),
                    })
            }
            Spell::WallOfStone => {
                let side = DVec3::new(forward.z, 0.0, -forward.x);
                let size = stone::Form::Thick.size();
                let center = feet + forward * ahead;
                // The panels stand on the lowest ground under them, so no
                // gap opens beneath the wall on a slope.
                let samples = (2.0 * size.x / 0.5).ceil() as usize;
                let base = (0..=samples)
                    .map(|i| {
                        let p =
                            center + side * (-size.x + 2.0 * size.x * i as f64 / samples as f64);
                        f64::from(super::height(p.x as f32, p.z as f32))
                    })
                    .fold(f64::INFINITY, f64::min);
                let start = DVec3::new(center.x, base, center.z) - side * size.x;
                let panels = stone::shapes::straight(start, side, 2, stone::Form::Thick);
                let ground = stone::validate::Stone::aabb(
                    DVec3::new(center.x - SLAB, base - 1.0, center.z - SLAB),
                    DVec3::new(center.x + SLAB, base, center.z + SLAB),
                );
                let plan = stone::validate::validate(&panels, &[ground], feet)
                    .map_err(|e| format!("Wall of Stone placement refused: {e:?}"))?;
                // Nothing built may stand in the wall's way: a building's
                // walls, floors, or furniture. Walls already raised, and
                // what another zone's rules add, such as the Grove's
                // dummies, don't count.
                let raised: Vec<Footprint> = solids.spell_blocks().map(|(f, _)| f).collect();
                let blocked = plan.panels.iter().flat_map(posts).any(|p| {
                    solids
                        .blocking_near(p.x as f32, p.z as f32, POST_HALF, base as f32)
                        .iter()
                        .any(|f| !raised.contains(f))
                });
                if blocked {
                    return Err(
                        "Wall of Stone needs clear ground: something built is in the way".into(),
                    );
                }
                Ok(Admitted::Stone(plan.panels))
            }
            Spell::WindWall => {
                let facing = DVec2::new(forward.x, forward.z);
                let center = DVec2::new(feet.x, feet.z) + facing * ahead;
                let side = DVec2::new(-facing.y, facing.x);
                let path = wind::Wall::straight(center, side, 30.0 * wind::FOOT);
                let wall = wind::Wall::new(path, feet, |p| {
                    Some(f64::from(super::height(p.x as f32, p.y as f32)))
                })
                .map_err(|refusal| match refusal {
                    wind::Refusal::NotGrounded => "Wind Wall needs level ground ahead".to_string(),
                    refusal => format!("Wind Wall placement refused: {refusal:?}"),
                })?;
                Ok(Admitted::Wind(wall))
            }
            Spell::ReverseGravity => {
                let floor = solids.floor(player.pos.x, player.pos.z, player.pos.y);
                let point = DVec3::new(feet.x, f64::from(floor), feet.z);
                Ok(Admitted::Reverse(reverse::Gravity {
                    cylinder: reverse::Cylinder::at(point),
                    active: true,
                }))
            }
        }
    }

    /// The live Reverse Gravity, if any.
    #[must_use]
    pub fn reverse_gravity(&self) -> Option<&reverse::Gravity> {
        self.reverse.as_ref().map(|c| &c.spell)
    }

    /// The newest Wind Wall, if any.
    #[must_use]
    pub fn wind_wall(&self) -> Option<&wind::Wall> {
        self.winds.back().map(|c| &c.spell)
    }

    /// Every standing Wind Wall, oldest first.
    pub fn wind_walls(&self) -> impl Iterator<Item = &wind::Wall> {
        self.winds.iter().map(|c| &c.spell)
    }

    /// The newest Wall of Stone's panels, if any.
    #[must_use]
    pub fn stone_panels(&self) -> Option<&[stone::Placement]> {
        self.stones.back().map(|c| c.spell.as_slice())
    }

    /// Every standing Wall of Stone's panels as footprints with their
    /// tops, m.
    #[must_use]
    pub fn blocks(&self) -> Vec<(Footprint, f32)> {
        let mut blocks = Vec::new();
        for panel in self.stones.iter().flat_map(|c| &c.spell) {
            let top = (panel.center.y + panel.half().y) as f32;
            for p in posts(panel) {
                let (x, z) = (p.x as f32, p.z as f32);
                let footprint = Footprint {
                    min: [x - POST_HALF, z - POST_HALF],
                    max: [x + POST_HALF, z + POST_HALF],
                };
                blocks.push((footprint, top));
            }
        }
        blocks
    }

    /// Applies the spells to `player`'s vertical motion after a step of
    /// `dt` seconds that started with the feet at `feet` moving at `speed`,
    /// m/s. Inside Reverse Gravity's cylinder its field replaces the
    /// controller's gravity and a roof overhead stops the rise; elsewhere
    /// Wind Wall's updraft throws the player who walks into it upward, on
    /// the ground or in the air, and Feather Fall's drag caps the descent
    /// until the player lands.
    pub fn after_step(
        &mut self,
        player: &mut PlayerController,
        feet: f32,
        speed: f32,
        solids: &Solids,
        dt: f32,
    ) {
        let (x, z) = (player.pos.x, player.pos.z);
        // A player hovering at the top plane crosses it by a hair; the
        // rules' field governs that overshoot too, rather than the
        // controller's stronger gravity.
        let governed = |cylinder: &reverse::Cylinder| {
            let at = DVec3::new(f64::from(x), f64::from(feet), f64::from(z));
            cylinder.axis_distance(at) <= cylinder.radius
                && (cylinder.base.y..=cylinder.top() + reverse::HOVER_BAND).contains(&at.y)
        };
        if let Some(Cast { spell: gravity, .. }) = &self.reverse
            && governed(&gravity.cylinder)
        {
            let steps = (f64::from(dt) / SUBSTEP).ceil().max(1.0);
            let h = f64::from(dt) / steps;
            let (mut y, mut v) = (f64::from(feet), f64::from(speed));
            for _ in 0..steps as usize {
                let at = DVec3::new(f64::from(x), y, f64::from(z));
                v += reverse::creature_accel(gravity, at, v) * h;
                y += v * h;
            }
            let (mut y, mut v) = (y as f32, v as f32);
            if let Some(roof) = solids.ceiling(x, z, feet + AVATAR_HEIGHT)
                && y + AVATAR_HEIGHT > roof
            {
                y = roof - AVATAR_HEIGHT;
                v = v.min(0.0);
            }
            let floor = solids.floor(x, z, y.max(feet));
            if y <= floor {
                y = floor;
                v = v.max(0.0);
            }
            player.pos.y = y;
            player.set_vertical_speed(v);
            player.set_surface_height(floor);
            return;
        }
        let in_wind = self.wind_walls().any(|wall| {
            wall.in_area(
                player.pos.as_dvec3(),
                f64::from(RADIUS),
                f64::from(AVATAR_HEIGHT),
            )
        });
        if !player.airborne() && !in_wind {
            // A warded landing ends the spell on the player.
            if let Some(effect) = &mut self.feather {
                effect.land(PLAYER, self.time);
            }
            return;
        }
        let after = player.vertical_speed();
        let mut v = after;
        if in_wind {
            // The chamber's heavy-creature updraft is gentler than this
            // controller's gravity; in the glade the wall throws the player
            // up and keeps lifting while they stay in it.
            v = v.max(WIND_LIFT);
        }
        if let Some(effect) = &mut self.feather
            && let Some(drag) = effect.drag(
                PLAYER,
                MASS,
                f64::from(speed),
                f64::from(v),
                f64::from(dt),
                self.time,
            )
        {
            v = drag.vertical_speed as f32;
        }
        player.pos.y += (v - after) * dt;
        player.set_vertical_speed(v);
    }

    /// The live spells as drawn around `player`, seen from `eye`.
    #[must_use]
    pub fn mesh(&self, player: &PlayerController, eye: Vec3) -> Mesh {
        let mut mesh = Mesh::default();
        let time = self.time as f32;
        let mut line = |a: Vec3, b: Vec3, [r, g, bl, alpha]: [f32; 4]| {
            let color = [r * alpha, g * alpha, bl * alpha];
            for p in [a, b] {
                mesh.lines.push(Vertex {
                    pos: p.to_array(),
                    color,
                    fog: 1.0,
                });
            }
        };
        for wall in self.wind_walls() {
            for (a, b, color) in verse_world::spells::wind_wall::wall_lines(wall, time) {
                line(a, b, color);
            }
        }
        // Reverse Gravity draws no guide lines, only its particles.
        if let Some(Cast { spell: gravity, at }) = &self.reverse {
            let age = (self.time - at) as f32;
            motes::draw(&mut mesh, &gravity.cylinder, age, eye);
        }
        for Cast { spell: panels, at } in &self.stones {
            let rise = ((self.time - at) / RAISE).clamp(0.0, 1.0);
            for panel in panels {
                slab(&mut mesh, panel, rise);
            }
        }
        for crumble in &self.crumbles {
            crumble.draw(&mut mesh);
        }
        // The dust, thinned evenly to its sprite budget.
        let dust: usize = self.crumbles.iter().map(|c| c.site.puffs().len()).sum();
        let stride = dust.div_ceil(MAX_DUST).max(1);
        let puffs: Vec<Puff> = self
            .crumbles
            .iter()
            .flat_map(Crumble::puffs)
            .step_by(stride)
            .collect();
        hammer::dust(&mut mesh, &puffs);
        if self.active(Spell::FeatherFall) {
            feathers(&mut mesh, player.pos, time);
        }
        mesh
    }
}

/// What a cast's rules admitted.
enum Admitted {
    Feather(feather::FeatherFall),
    Stone(Vec<stone::Placement>),
    Wind(wind::Wall),
    Reverse(reverse::Gravity),
}

/// Where the posts that stand in for `panel`'s footprint stand, along its
/// middle line at its center's height.
fn posts(panel: &stone::Placement) -> impl Iterator<Item = DVec3> + '_ {
    let half = panel.half();
    let along = panel.orientation * DVec3::X;
    let count = (2.0 * half.x / POST_SPACING).ceil() as usize;
    (0..=count)
        .map(move |i| panel.center + along * (-half.x + 2.0 * half.x * i as f64 / count as f64))
}

/// One granite panel risen `rise` of its height, as shaded faces.
fn slab(mesh: &mut Mesh, panel: &stone::Placement, rise: f64) {
    let half = panel.half();
    shaded_box(mesh, |i| {
        let x = if i & 1 == 0 { -half.x } else { half.x };
        let y = if i & 2 == 0 {
            -half.y
        } else {
            -half.y + 2.0 * half.y * rise
        };
        let z = if i & 4 == 0 { -half.z } else { half.z };
        panel.to_world(DVec3::new(x, y, z)).as_vec3()
    });
}

/// A granite box as shaded faces, from its eight corners: bit 0 of the
/// index picks the high x side, bit 1 the high y, and bit 2 the high z.
fn shaded_box(mesh: &mut Mesh, corner: impl Fn(usize) -> Vec3) {
    let c: [Vec3; 8] = std::array::from_fn(corner);
    for [a, b, cc, d] in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        let color = super::draw::shade(GRANITE, c[a], c[b], c[cc]);
        for p in [c[a], c[b], c[cc], c[a], c[cc], c[d]] {
            mesh.faces.push(Vertex {
                pos: p.to_array(),
                color,
                fog: 1.0,
            });
        }
    }
}

/// Pale feathers drifting up around a warded player whose feet are at
/// `feet`: the player falls slowly past them.
fn feathers(mesh: &mut Mesh, feet: Vec3, time: f32) {
    for i in 0..FEATHERS {
        let phase = i as f32 / FEATHERS as f32;
        let angle = std::f32::consts::TAU * phase + time * 1.3;
        let lift = (time * 0.6 + phase * 1.7).rem_euclid(1.0);
        let at = feet
            + Vec3::new(
                angle.cos() * 0.7,
                0.2 + lift * AVATAR_HEIGHT,
                angle.sin() * 0.7,
            );
        let tilt = time * 2.0 + phase * 5.0;
        let along = Vec3::new(tilt.cos(), 0.35, tilt.sin()).normalize() * 0.12;
        let across = Vec3::new(-tilt.sin(), 0.0, tilt.cos()) * 0.035;
        let [a, b, c, d] = [at - along, at - across, at + along, at + across];
        for p in [a, b, c, a, c, d] {
            mesh.faces.push(Vertex {
                pos: p.to_array(),
                color: FEATHER,
                fog: 1.0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 60.0;

    /// Runs `spells` for `seconds` in frames, and returns whether the
    /// solids they raise changed in any.
    fn run(spells: &mut Spells, seconds: f64) -> bool {
        let mut changed = false;
        for _ in 0..(seconds / f64::from(FRAME)).round() as usize {
            changed |= spells.tick(FRAME);
        }
        changed
    }

    #[test]
    fn wall_of_stone_crumbles_after_its_lifetime_and_stops_blocking() {
        let mut spells = Spells::default();
        let player = PlayerController::new(Vec3::ZERO, 0.0);
        let solids = Solids::default();
        spells.cast(Spell::WallOfStone, &player, &solids).unwrap();
        let blocks = spells.blocks();
        // Two ten-foot panels four meters ahead, each a row of posts as
        // tall as a panel.
        assert!(blocks.len() > 20);
        let top = stone::Form::Thick.size().y as f32;
        for (footprint, height) in &blocks {
            assert!((height - top).abs() < 1e-4);
            let z = (footprint.min[1] + footprint.max[1]) / 2.0;
            assert!((z - 4.0).abs() < 0.1, "{z}");
        }
        assert!(!spells.tick(STONE_LIFETIME as f32 - 1.0));
        assert!(spells.active(Spell::WallOfStone));
        assert_eq!(spells.debris(), 0);
        assert!(spells.tick(1.0));
        assert!(!spells.active(Spell::WallOfStone));
        assert!(spells.blocks().is_empty());
        // It broke into chunks, which draw.
        let chunks = 2 * CHUNK_GRID[0] * CHUNK_GRID[1];
        assert_eq!(spells.debris(), chunks);
        let mesh = spells.mesh(&player, Vec3::Y);
        assert!(!mesh.faces.is_empty());
        // With dust, held to its budget.
        assert!(
            (1..=MAX_DUST).contains(&mesh.sprites.len()),
            "{}",
            mesh.sprites.len()
        );
    }

    #[test]
    fn crumbled_chunks_fall_and_settle_on_the_ground() {
        let mut spells = Spells::default();
        let player = PlayerController::new(Vec3::ZERO, 0.0);
        spells
            .cast(Spell::WallOfStone, &player, &Solids::default())
            .unwrap();
        spells.tick(STONE_LIFETIME as f32);
        let ground = super::super::height(0.0, 4.0);
        let top = ground + stone::Form::Thick.size().y as f32;
        run(&mut spells, 2.0);
        let crumble = &spells.crumbles[0];
        let heights: Vec<f32> = crumble
            .site
            .pieces()
            .iter()
            .flat_map(|p| &p.chunks)
            .filter(|c| !c.gone)
            .map(|c| crumble.origin.y + crumble.site.body_pose(c.body).w_axis.y)
            .collect();
        assert!(!heights.is_empty());
        // Nothing stands as high as the wall did, and nothing sinks far
        // through the ground.
        let highest = heights.iter().copied().fold(f32::MIN, f32::max);
        assert!(highest < top - 1.0, "{highest} against {top}");
        assert!(heights.iter().all(|&y| y > ground - 0.5), "{heights:?}");
    }

    #[test]
    fn no_debris_is_left_after_the_fade() {
        let mut spells = Spells::default();
        let player = PlayerController::new(Vec3::ZERO, 0.0);
        spells
            .cast(Spell::WallOfStone, &player, &Solids::default())
            .unwrap();
        spells.tick(STONE_LIFETIME as f32);
        assert!(spells.debris() > 0);
        // Chunks shrink before they go.
        run(&mut spells, DEBRIS_LIFETIME - 0.5 * DEBRIS_FADE);
        run(&mut spells, DEBRIS_LIFETIME * 0.2 + 2.0 * DEBRIS_FADE + 3.0);
        assert_eq!(spells.debris(), 0);
        assert!(spells.crumbles.is_empty());
        assert!(spells.mesh(&player, Vec3::Y).faces.is_empty());
        assert!(spells.mesh(&player, Vec3::Y).sprites.is_empty());
    }

    #[test]
    fn the_cap_crumbles_the_oldest_wall() {
        let mut spells = Spells::default();
        let solids = Solids::default();
        let mut player = PlayerController::new(Vec3::ZERO, 0.0);
        for n in 0..MAX_STONE_WALLS {
            player.pos = Vec3::X * (n as f32 * 8.0);
            spells.cast(Spell::WallOfStone, &player, &solids).unwrap();
            spells.tick(0.5);
        }
        assert_eq!(spells.count(Spell::WallOfStone), MAX_STONE_WALLS);
        assert_eq!(spells.debris(), 0);
        let oldest = spells.stones[0].spell.clone();
        let second = spells.stones[1].spell.clone();
        player.pos = Vec3::X * -8.0;
        spells.cast(Spell::WallOfStone, &player, &solids).unwrap();
        assert_eq!(spells.count(Spell::WallOfStone), MAX_STONE_WALLS);
        // The first wall crumbled; the second is the oldest standing.
        assert_eq!(
            format!("{:?}", spells.stones[0].spell),
            format!("{second:?}")
        );
        assert!(
            spells
                .stones
                .iter()
                .all(|c| format!("{:?}", c.spell) != format!("{oldest:?}"))
        );
        assert_eq!(spells.debris(), 2 * CHUNK_GRID[0] * CHUNK_GRID[1]);
        // Its posts, across x = 0, went with it.
        assert!(
            spells
                .blocks()
                .iter()
                .all(|(f, _)| (f.min[0] + f.max[0]).abs() / 2.0 > 3.2)
        );
        // The wall cast next to go still lasts its own lifetime.
        assert!(!run(&mut spells, STONE_LIFETIME - 2.0));
        assert!(run(&mut spells, 1.0));
    }

    #[test]
    fn a_wall_through_something_built_is_refused() {
        let player = PlayerController::new(Vec3::ZERO, 0.0);
        let mut solids = Solids::default();
        // A table across the wall's line, four meters ahead.
        solids.add_block(
            Footprint {
                min: [0.5, 3.6],
                max: [1.5, 4.4],
            },
            super::super::height(1.0, 4.0) + 0.8,
        );
        let Err(refusal) = Spells::default().admit(Spell::WallOfStone, &player, &solids) else {
            panic!("a wall through a table was admitted");
        };
        assert!(refusal.contains("in the way"), "{refusal}");
        // A wall already raised there doesn't count: another stands beside it.
        let mut spells = Spells::default();
        let mut solids = Solids::default();
        spells.cast(Spell::WallOfStone, &player, &solids).unwrap();
        solids.set_spell_blocks(spells.blocks());
        spells.cast(Spell::WallOfStone, &player, &solids).unwrap();
        assert_eq!(spells.count(Spell::WallOfStone), 2);
    }

    #[test]
    fn wind_wall_and_reverse_gravity_last_a_minute() {
        for spell in [Spell::WindWall, Spell::ReverseGravity] {
            let mut spells = Spells::default();
            let player = PlayerController::new(Vec3::ZERO, 0.0);
            spells.cast(spell, &player, &Solids::default()).unwrap();
            assert!(!spells.tick(59.0));
            assert!(spells.active(spell));
            assert!(!spells.tick(1.0));
            assert!(!spells.active(spell));
        }
    }

    #[test]
    fn wind_wall_needs_level_ground() {
        // Walking around the clearing's edge, so the wall would run up the
        // rising ground toward the tree ring.
        let at = Vec3::new(0.0, super::super::height(0.0, 148.0), 148.0);
        let player = PlayerController::new(at, std::f32::consts::FRAC_PI_2);
        let Err(refusal) = Spells::default().admit(Spell::WindWall, &player, &Solids::default())
        else {
            panic!("a wall up a slope was admitted");
        };
        assert!(refusal.contains("level ground"), "{refusal}");
    }
}
