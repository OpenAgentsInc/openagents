//! The spells on Everglade's hotbar that need no enemy: Feather Fall, Wall
//! of Stone, Wind Wall, and Reverse Gravity.
//!
//! Each cast is admitted, and moves the player, through the chamber's
//! native rules in `verse_world`: Feather Fall's drag and landing, Wall of
//! Stone's panel layout and support check, Wind Wall's grounded path and
//! updraft, and Reverse Gravity's cylinder and hover spring. The player is
//! the only creature they act on. They draw as the chamber draws them: Wind
//! Wall's updraft streaks and Reverse Gravity's cylinder are the chamber's
//! guide lines, and the panels are granite slabs in the chamber's stone
//! color.
//!
//! Wall of Stone, Wind Wall, and Reverse Gravity need concentration, so
//! casting one ends the one before it, and pressing a live one's slot ends
//! it. Everglade's Levitate stays a movement toggle outside that rule.

use super::hotbar::Slot;
use super::solids::Solids;
use crate::controller::{AVATAR_HEIGHT, Footprint, PlayerController, RADIUS};
use crate::mesh::{Mesh, Vertex};
use crate::zones::Intent;
use glam::{DVec2, DVec3, Vec3};
use verse_world::{
    feather_fall as feather, reverse_gravity as reverse, wall_of_stone as stone, wind_wall as wind,
};

/// The player's identity in the spells' rules.
const PLAYER: u32 = 1;
/// The player's mass for Feather Fall's energy accounting, kg.
const MASS: f64 = 75.0;
/// How far ahead of the caster the walls stand, m, as in the chamber.
const AHEAD: f64 = 4.0;
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
/// Feather Fall's drifting feathers.
const FEATHERS: usize = 8;
const FEATHER: [f32; 3] = [0.95, 0.93, 0.86];

/// A spell on the hotbar, after Levitate, Up, and Down.
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
        Self::WallOfStone,
        Self::WindWall,
        Self::ReverseGravity,
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

    /// The chamber's cooldown, s.
    #[must_use]
    pub fn cooldown(self) -> f32 {
        verse_world::spells::CATALOG
            .iter()
            .find(|s| s.key == self.key())
            .map_or(verse_world::spells::ROUND, |s| s.cooldown)
    }

    const fn index(self) -> usize {
        self as usize
    }
}

/// The live concentration spell.
#[derive(Clone, Debug)]
enum Concentration {
    Stone {
        panels: Vec<stone::Placement>,
        at: f64,
    },
    Wind {
        wall: wind::Wall,
        at: f64,
    },
    Reverse {
        gravity: reverse::Gravity,
        at: f64,
    },
}

impl Concentration {
    fn spell(&self) -> Spell {
        match self {
            Self::Stone { .. } => Spell::WallOfStone,
            Self::Wind { .. } => Spell::WindWall,
            Self::Reverse { .. } => Spell::ReverseGravity,
        }
    }

    fn ends_at(&self) -> f64 {
        match self {
            Self::Stone { at, .. } => at + stone::DURATION,
            Self::Wind { at, .. } => at + wind::DURATION,
            Self::Reverse { at, .. } => at + reverse::DURATION,
        }
    }
}

/// The player's spells: the clock, each spell's cooldown, the Feather Fall
/// ward, and the concentration spell.
#[derive(Clone, Debug, Default)]
pub(crate) struct Spells {
    time: f64,
    ready: [f64; 4],
    feather: Option<feather::FeatherFall>,
    concentration: Option<Concentration>,
}

impl Spells {
    /// Advances the clock `dt` seconds and ends what has run out. Returns
    /// whether the solids the spells raise changed.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.time += f64::from(dt.max(0.0));
        if self.feather.as_ref().is_some_and(|f| f.ended(self.time)) {
            self.feather = None;
        }
        if self
            .concentration
            .as_ref()
            .is_some_and(|c| self.time >= c.ends_at())
        {
            let stone = matches!(self.concentration, Some(Concentration::Stone { .. }));
            self.concentration = None;
            return stone;
        }
        false
    }

    /// Whether `spell` is live on the player.
    #[must_use]
    pub fn active(&self, spell: Spell) -> bool {
        match spell {
            Spell::FeatherFall => self
                .feather
                .as_ref()
                .is_some_and(|f| f.holds(PLAYER, self.time)),
            _ => self
                .concentration
                .as_ref()
                .is_some_and(|c| c.spell() == spell),
        }
    }

    /// Seconds until `spell` can be cast again.
    fn cooling(&self, spell: Spell) -> f64 {
        (self.ready[spell.index()] - self.time).max(0.0)
    }

    /// `spell`'s hotbar slot for `player` standing on `solids`.
    #[must_use]
    pub fn slot(&self, spell: Spell, player: &PlayerController, solids: &Solids) -> Slot {
        let active = self.active(spell);
        let ready = self.cooling(spell) <= 0.0;
        let enabled = (active && spell != Spell::FeatherFall)
            || (ready && !active && self.admit(spell, player, solids).is_ok());
        Slot {
            enabled,
            active,
            cooldown: (self.cooling(spell) / f64::from(spell.cooldown()).max(1e-3)) as f32,
        }
    }

    /// Casts `spell` for `player`, or ends it when it is a live
    /// concentration spell.
    ///
    /// # Errors
    ///
    /// Returns why the cast was refused: a cooldown, or the rule it breaks.
    pub fn cast(
        &mut self,
        spell: Spell,
        player: &PlayerController,
        solids: &Solids,
    ) -> Result<(), String> {
        if spell != Spell::FeatherFall && self.active(spell) {
            self.concentration = None;
            return Ok(());
        }
        if self.cooling(spell) > 0.0 {
            return Err(format!("{} is not ready", label(spell)));
        }
        match self.admit(spell, player, solids)? {
            Admitted::Feather(effect) => self.feather = Some(effect),
            Admitted::Concentration(c) => self.concentration = Some(c),
        }
        self.ready[spell.index()] = self.time + f64::from(spell.cooldown());
        Ok(())
    }

    /// The rules' admission of `spell` cast now by `player`.
    fn admit(
        &self,
        spell: Spell,
        player: &PlayerController,
        solids: &Solids,
    ) -> Result<Admitted, String> {
        let feet = player.pos.as_dvec3();
        let forward = player.forward().as_dvec3();
        let at = self.time;
        match spell {
            Spell::FeatherFall => {
                if self.active(spell) {
                    return Err("Feather Fall already holds you".into());
                }
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
                let center = feet + forward * AHEAD;
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
                Ok(Admitted::Concentration(Concentration::Stone {
                    panels: plan.panels,
                    at,
                }))
            }
            Spell::WindWall => {
                let facing = DVec2::new(forward.x, forward.z);
                let center = DVec2::new(feet.x, feet.z) + facing * AHEAD;
                let side = DVec2::new(-facing.y, facing.x);
                let path = wind::Wall::straight(center, side, 30.0 * wind::FOOT);
                let wall = wind::Wall::new(path, feet, |p| {
                    Some(f64::from(super::height(p.x as f32, p.y as f32)))
                })
                .map_err(|refusal| match refusal {
                    wind::Refusal::NotGrounded => "Wind Wall needs level ground ahead".to_string(),
                    refusal => format!("Wind Wall placement refused: {refusal:?}"),
                })?;
                Ok(Admitted::Concentration(Concentration::Wind { wall, at }))
            }
            Spell::ReverseGravity => {
                let floor = solids.floor(player.pos.x, player.pos.z, player.pos.y);
                let point = DVec3::new(feet.x, f64::from(floor), feet.z);
                Ok(Admitted::Concentration(Concentration::Reverse {
                    gravity: reverse::Gravity {
                        cylinder: reverse::Cylinder::at(point),
                        active: true,
                    },
                    at,
                }))
            }
        }
    }

    /// Wall of Stone's panels as footprints with their tops, m.
    #[must_use]
    pub fn blocks(&self) -> Vec<(Footprint, f32)> {
        let Some(Concentration::Stone { panels, .. }) = &self.concentration else {
            return Vec::new();
        };
        let mut blocks = Vec::new();
        for panel in panels {
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
    /// Wind Wall's updraft lifts the airborne player inside it, and Feather
    /// Fall's drag caps the descent until the player lands.
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
        if let Some(Concentration::Reverse { gravity, .. }) = &self.concentration
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
        if !player.airborne() {
            // A warded landing ends the spell on the player.
            if let Some(effect) = &mut self.feather {
                effect.land(PLAYER, self.time);
            }
            return;
        }
        let after = player.vertical_speed();
        let mut v = after;
        if let Some(Concentration::Wind { wall, .. }) = &self.concentration
            && wall.in_area(
                player.pos.as_dvec3(),
                f64::from(RADIUS),
                f64::from(AVATAR_HEIGHT),
            )
        {
            v += wind::HEAVY_UPDRAFT as f32 * dt;
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

    /// The live spells as drawn around `player`.
    #[must_use]
    pub fn mesh(&self, player: &PlayerController) -> Mesh {
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
        match &self.concentration {
            Some(Concentration::Wind { wall, .. }) => {
                for (a, b, color) in verse_world::spells::wind_wall::wall_lines(wall, time) {
                    line(a, b, color);
                }
            }
            Some(Concentration::Reverse { gravity, .. }) => {
                for (a, b, color) in
                    verse_world::spells::reverse_gravity::cylinder_lines(&gravity.cylinder)
                {
                    line(a, b, color);
                }
            }
            Some(Concentration::Stone { panels, at }) => {
                let rise = ((self.time - at) / RAISE).clamp(0.0, 1.0);
                for panel in panels {
                    slab(&mut mesh, panel, rise);
                }
            }
            None => {}
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
    Concentration(Concentration),
}

fn label(spell: Spell) -> &'static str {
    verse_world::spells::CATALOG
        .iter()
        .find(|s| s.key == spell.key())
        .map_or("The spell", |s| s.label)
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
        let at = Vec3::new(0.0, super::super::height(0.0, 40.0), 40.0);
        let player = PlayerController::new(at, std::f32::consts::FRAC_PI_2);
        let Err(refusal) = Spells::default().admit(Spell::WindWall, &player, &Solids::default())
        else {
            panic!("a wall up a slope was admitted");
        };
        assert!(refusal.contains("level ground"), "{refusal}");
    }
}
