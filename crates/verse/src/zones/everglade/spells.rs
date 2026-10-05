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
//! casts ([`Spells::cast_ahead`]). Each Wind Wall and Wall of Stone stands
//! beside the ones before it, up to [`MAX_WALLS`] of each. Pressing Feather
//! Fall or Reverse Gravity while it holds ends it. The Grove keeps the
//! chamber's concentration rule through [`Spells::concentrate_ahead`].

use super::hotbar::Slot;
use super::solids::Solids;
use crate::controller::{AVATAR_HEIGHT, Footprint, PlayerController, RADIUS};
use crate::mesh::{Mesh, Vertex};
use crate::zones::Intent;
use glam::{DVec2, DVec3, Vec3};
use std::collections::VecDeque;
pub(crate) mod motes;

use verse_world::{
    feather_fall as feather, reverse_gravity as reverse, wall_of_stone as stone, wind_wall as wind,
};

/// The player's identity in the spells' rules.
const PLAYER: u32 = 1;
/// The player's mass for Feather Fall's energy accounting, kg.
const MASS: f64 = 75.0;
/// How far ahead of the caster the walls stand, m, as in the chamber.
pub(crate) const AHEAD: f64 = 4.0;
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
/// The most Wind Walls, and the most Walls of Stone, that stand at once;
/// another cast takes down the oldest.
pub(crate) const MAX_WALLS: usize = 12;

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

/// The player's spells: the clock, the Feather Fall ward, the standing
/// Wind Walls and Walls of Stone, oldest first, and Reverse Gravity.
#[derive(Clone, Debug, Default)]
pub(crate) struct Spells {
    time: f64,
    /// Whether the Grove asked for free casting; every cast is free now,
    /// so this only round-trips through a long rest.
    free: bool,
    feather: Option<feather::FeatherFall>,
    winds: VecDeque<Cast<wind::Wall>>,
    stones: VecDeque<Cast<Vec<stone::Placement>>>,
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
        let stones = self.stones.len();
        self.stones.retain(|c| now < c.at + stone::DURATION);
        self.stones.len() != stones
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
        self.stones.clear();
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
                while self.stones.len() > MAX_WALLS {
                    self.stones.pop_front();
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
            let half = panel.half();
            let along = panel.orientation * DVec3::X;
            let top = (panel.center.y + half.y) as f32;
            let count = (2.0 * half.x / POST_SPACING).ceil() as usize;
            for i in 0..=count {
                let p = panel.center + along * (-half.x + 2.0 * half.x * i as f64 / count as f64);
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

/// One granite panel risen `rise` of its height, as shaded faces.
fn slab(mesh: &mut Mesh, panel: &stone::Placement, rise: f64) {
    let half = panel.half();
    let corner = |i: usize| {
        let x = if i & 1 == 0 { -half.x } else { half.x };
        let y = if i & 2 == 0 {
            -half.y
        } else {
            -half.y + 2.0 * half.y * rise
        };
        let z = if i & 4 == 0 { -half.z } else { half.z };
        panel.to_world(DVec3::new(x, y, z)).as_vec3()
    };
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

    #[test]
    fn wall_of_stone_stands_for_ten_minutes_then_its_blocks_go() {
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
        assert!(!spells.tick(stone::DURATION as f32 - 1.0));
        assert!(spells.active(Spell::WallOfStone));
        assert!(spells.tick(1.0));
        assert!(!spells.active(Spell::WallOfStone));
        assert!(spells.blocks().is_empty());
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
