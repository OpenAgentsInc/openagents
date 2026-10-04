//! Everglade: the forest glade where the Agent Studio lives
//! (`docs/verse/everglade.md`).
//!
//! The ground is a heightfield computed in Rust: flat inside the clearing,
//! rising toward the tree ring. The glade and the workshop are placements
//! of the pinned Everglade pack's models (`layout`), drawn as textured,
//! alpha-tested cells on a lit stage, with the Task Wall, the desk
//! monitors, and the atrium's goal board drawn by Verse (`boards`, from
//! [`signals`]). The pack loads on portal entry, as the Ruins pack does.
//! The studio's stations have fixed standing points in [`STATIONS`]; the
//! Agent Studio's seats walk between them and the stations open its panels
//! ([`studio`]). The player walks the shared plaza controller over the
//! heightfield as the ritual chamber's outfitted character from the pack,
//! and the seats are the same character in their own colors and postures
//! ([`player`], [`pose`]); no companion follows.

mod boards;
mod draw;
pub mod hotbar;
pub mod layout;
pub mod player;
pub mod pose;
mod scene;
pub mod signals;
pub mod solids;
#[cfg(test)]
mod spell_tests;
pub mod spells;
pub mod studio;
#[cfg(test)]
mod tests;

use crate::{
    controller::{Footprint, InputState, PlayerController},
    mesh::Mesh,
    pbr::{
        Daylight, Key, Neon,
        textured::TexturedScene,
        textured_bake::{self, AmbientProbes, BakeJob, BakeLight, BakeSettings},
    },
    world::World,
};
use glam::Vec3;
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use super::everglade_pack::ZonePack;

/// Half the walkable square, m. The glade is about 120 m across; the square
/// runs past the tree ring so its rising ground closes the view.
pub const HALF_EXTENT: f32 = 75.0;
/// Radius of the flat clearing around the workshop, m.
pub const CLEARING_RADIUS: f32 = 34.0;
/// Radius of the tree ring, where the ground finishes its rise, m.
pub const RING_RADIUS: f32 = 58.0;
/// Height of the ground at the tree ring above the clearing, m.
pub const RING_RISE: f32 = 5.0;
/// Highest ground anywhere in the zone, m.
pub const MAX_HEIGHT: f32 = 10.0;
/// Amplitude of the low undulation on the rising ground, m.
const UNDULATION: f32 = 0.6;
/// Rise per meter beyond the tree ring.
const OUTER_SLOPE: f32 = 0.1;

/// The return portal, at the start of the approach path.
pub(crate) const RETURN_PORTAL: Vec3 = Vec3::new(0.0, 0.0, -32.0);
/// Where the player arrives: on the approach path, facing the workshop.
const SPAWN: Vec3 = Vec3::new(0.0, 0.0, -20.0);
const SPAWN_YAW: f32 = 0.0;
/// Distance from a station's standing point within which the caption names
/// the station, m.
pub const STATION_RANGE: f32 = 3.0;

/// The workshop hall's floor: center x and z, and half extents, m. The
/// hall's door is in its south wall, facing the yard and the approach.
pub const HALL: ([f32; 2], [f32; 2]) = ([0.0, 6.0], [8.0, 5.0]);
/// The strongroom annex east of the hall: center and half extents, m.
pub const STRONGROOM: ([f32; 2], [f32; 2]) = ([11.0, 6.0], [3.0, 4.0]);
/// The yard in front of the hall: center and half extents, m.
pub const YARD: ([f32; 2], [f32; 2]) = ([0.0, -6.5], [13.0, 7.5]);
/// Half the width of the approach path, m. The path runs along x = 0 from
/// the return portal to the yard.
pub const PATH_HALF_WIDTH: f32 = 1.6;
/// Spacing of the baked light probes characters sample, m.
const PROBE_CELL: f32 = 3.0;
/// How far the probe grid reaches above the highest ground, m: a
/// character's head on the ring.
const PROBE_HEADROOM: f32 = 3.0;

/// One Agent Studio station: where a seat or the player stands to use it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    /// Stable identifier, also the map landmark ID.
    pub id: &'static str,
    /// The place in the glade, from the layout table.
    pub place: &'static str,
    /// The studio station it hosts.
    pub studio: &'static str,
    /// Marker lettering: A–Z, 0–9, and space.
    pub sign: &'static str,
    /// Standing point, x and z, m. The ground there is part of the flat
    /// clearing, so its height is zero.
    pub at: [f32; 2],
    /// Heading from the standing point toward the station's furniture, as
    /// the controller's yaw.
    pub facing: f32,
}

impl Station {
    /// The standing point on the ground.
    #[must_use]
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.at[0], height(self.at[0], self.at[1]), self.at[1])
    }
}

/// The stations of the layout table in `docs/verse/everglade.md`, in the
/// table's order. Each station's furniture stands ahead of its point, in
/// the direction it faces; layout changes never move these points.
pub const STATIONS: [Station; 10] = [
    Station {
        id: "approach",
        place: "Approach path",
        studio: "Spawn and return",
        sign: "APPROACH",
        at: [-3.0, -27.0],
        facing: FRAC_PI_2,
    },
    Station {
        id: "task_wall",
        place: "Yard notice board",
        studio: "Task Wall",
        sign: "TASK WALL",
        at: [-7.0, -9.0],
        facing: -FRAC_PI_2,
    },
    Station {
        id: "desks",
        place: "Workshop hall",
        studio: "Desks",
        sign: "DESKS",
        at: [0.0, 5.0],
        facing: 0.0,
    },
    Station {
        id: "library",
        place: "Hall gallery",
        studio: "Library",
        sign: "LIBRARY",
        at: [-5.0, 9.0],
        facing: -FRAC_PI_2,
    },
    Station {
        id: "oracle",
        place: "Hearth corner",
        studio: "Oracle",
        sign: "ORACLE",
        at: [5.0, 9.0],
        facing: FRAC_PI_2,
    },
    Station {
        id: "proving",
        place: "Yard ring",
        studio: "Proving ground",
        sign: "PROVING GROUND",
        at: [8.0, -8.0],
        facing: FRAC_PI_2,
    },
    Station {
        id: "podium",
        place: "Lectern by the door",
        studio: "Podium",
        sign: "PODIUM",
        at: [-3.0, -2.5],
        facing: 0.0,
    },
    Station {
        id: "merge",
        place: "Strongroom",
        studio: "Merge station",
        sign: "MERGE",
        at: [10.5, 5.0],
        facing: FRAC_PI_2,
    },
    Station {
        id: "lounge",
        place: "Bench under the trees",
        studio: "Lounge",
        sign: "LOUNGE",
        at: [-24.0, -20.0],
        facing: -FRAC_PI_2,
    },
    Station {
        id: "workbench",
        place: "Wagon by the gate",
        studio: "Workbench",
        sign: "WORKBENCH",
        at: [9.0, -24.0],
        facing: FRAC_PI_2,
    },
];

/// Ground height at `(x, z)`, m: zero inside the clearing, rising smoothly to
/// [`RING_RISE`] at the tree ring with a low undulation, then climbing
/// gently to the edge of the zone. Always finite and within
/// `0..=MAX_HEIGHT`; a nonfinite coordinate reads as the clearing.
#[must_use]
pub fn height(x: f32, z: f32) -> f32 {
    if !x.is_finite() || !z.is_finite() {
        return 0.0;
    }
    let r = x.hypot(z);
    let t = ((r - CLEARING_RADIUS) / (RING_RADIUS - CLEARING_RADIUS)).clamp(0.0, 1.0);
    let rise = t * t * (3.0 - 2.0 * t);
    // The undulation scales with the rise, so the clearing stays flat and
    // the ground never dips below it.
    let wave = (x * 0.13 + 0.4).sin() * (z * 0.11 - 0.7).cos();
    let outer = (r - RING_RADIUS).max(0.0) * OUTER_SLOPE;
    (rise * (RING_RISE + UNDULATION * wave) + outer).clamp(0.0, MAX_HEIGHT)
}

/// The station whose standing point is nearest `(x, z)` within
/// [`STATION_RANGE`].
#[must_use]
pub fn station_near(x: f32, z: f32) -> Option<&'static Station> {
    STATIONS
        .iter()
        .map(|s| (s, (s.at[0] - x).hypot(s.at[1] - z)))
        .filter(|(_, d)| *d <= STATION_RANGE)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(s, _)| s)
}

/// The zone's live state: its clock, the lit stage its frames draw on, and
/// the characters: the player's and the studio's seats.
pub(crate) struct Everglade {
    elapsed: f32,
    pub levitating: bool,
    pub sprinting: bool,
    pub altitude: f32,
    pub jump: bool,
    landing: bool,
    solids: solids::Solids,
    spells: spells::Spells,
    rendered: Mesh,
    cast: Option<player::Cast>,
    /// The static scene's light bake while it runs.
    bake: Option<BakeJob>,
    /// The bake's probes, which light the characters once it finishes.
    probes: Option<Arc<AmbientProbes>>,
}

impl Everglade {
    /// The zone with `pack`'s player character standing at `at`. A pack
    /// without a character leaves the player as the plaza's avatar.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack's character cannot play.
    pub fn new(pack: &ZonePack, at: &PlayerController) -> Result<Self, String> {
        Ok(Self {
            elapsed: 0.0,
            levitating: false,
            sprinting: false,
            altitude: 0.0,
            jump: false,
            landing: false,
            solids: solids::Solids::build(pack, &layout::placements())?,
            spells: spells::Spells::default(),
            rendered: Self::stage(0.0),
            cast: player::Cast::new(pack, at)?,
            bake: None,
            probes: None,
        })
    }

    /// The afternoon light: a warm sun from behind the approach that casts
    /// shadows over the clearing, a cool rim, and sky and ground fill.
    fn key() -> Key {
        Key {
            dir: Vec3::new(-0.35, 0.8, -0.45).normalize(),
            illuminance: 4_000.0,
            angular_radius: 0.03,
            rim_dir: Vec3::new(0.5, 0.35, 0.6).normalize(),
            rim_illuminance: 900.0,
            rim_angular_radius: 0.1,
            sky: 1_200.0,
            ground: 450.0,
            ev100: 10.0,
            shadow_center: Vec3::new(0.0, 0.0, -4.0),
            shadow_half: 40.0,
        }
    }

    /// Starts baking `scene`'s ambient light under the zone's sun: sky
    /// visibility and one bounce for every static vertex, and the probes
    /// the characters sample. The bake runs off the main thread where the
    /// target has threads, keyed by the pinned pack's digest and the light.
    pub fn bake_light(&mut self, scene: Arc<TexturedScene>) {
        // Zone tests install the full pack many times over; the bake's own
        // tests in `pbr::textured_bake` cover it without the pack.
        if cfg!(test) {
            return;
        }
        let light = BakeLight::from_key(&Self::key());
        let settings = BakeSettings::new(
            Vec3::new(-HALF_EXTENT, 0.0, -HALF_EXTENT),
            Vec3::new(HALF_EXTENT, MAX_HEIGHT + PROBE_HEADROOM, HALF_EXTENT),
            PROBE_CELL,
        );
        let key = textured_bake::bake_key(
            super::everglade_pack::PACK_SHA256,
            &scene,
            &light,
            &settings,
        );
        self.bake = Some(BakeJob::start(scene, light, settings, key));
        self.probes = None;
    }

    /// The physical stage: a late-morning daylight sky whose horizon haze is
    /// the zone's air and fog, a warm sun from behind the approach that
    /// casts shadows over the clearing and stands in the sky where the
    /// shadows say it is, and sky and ground fill. Textured meshes draw only
    /// on a lit stage.
    fn stage(time: f32) -> Mesh {
        let air = super::atmosphere(super::ZoneId::Everglade);
        Mesh {
            neon: Some(Neon {
                field: air.color,
                fog_start: air.fog_start,
                fog_end: air.fog_end,
                line_gain: 1.0,
                line_width: 1.4,
                bloom: 0.04,
                vignette: 0.15,
                time,
                key: Some(Self::key()),
                daylight: Some(Daylight {
                    zenith: [0.10, 0.30, 0.73],
                    horizon: air.color,
                    sun: [1.0, 0.86, 0.62],
                    clouds: 0.38,
                }),
            }),
            ..Mesh::default()
        }
    }

    pub fn spawn() -> Vec3 {
        Vec3::new(SPAWN.x, height(SPAWN.x, SPAWN.z), SPAWN.z)
    }

    pub fn spawn_yaw() -> f32 {
        SPAWN_YAW
    }

    /// The ground, the textured glade and workshop from `pack` with their
    /// blockers, and the boards.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a placed model or the scene
    /// exceeds the renderer's bounds.
    pub fn world(pack: &ZonePack) -> Result<World, String> {
        let mut world = World::default();
        let (mut scene, blockers) = scene::build(pack, &layout::placements())?;
        draw::ground(&mut scene);
        scene.validate()?;
        world.mesh.textured = Some(Arc::new(scene));
        world.blockers = blockers;
        world.blockers.extend(layout::board_blockers());
        boards::draw(&mut world.mesh);
        Ok(world)
    }

    /// Walk the shared plaza controller over the heightfield. The controller
    /// sees flat ground at the terrain under the character: feet are moved
    /// into height above ground before the step and back after it, so walking,
    /// jumping, and landing follow the slope.
    pub fn move_player(
        player: &mut PlayerController,
        input: &InputState,
        blockers: &[Footprint],
        dt: f32,
    ) {
        player.pos.y -= height(player.pos.x, player.pos.z);
        player.set_surface_height(0.0);
        player.update(input, dt, blockers, HALF_EXTENT);
        let ground = height(player.pos.x, player.pos.z);
        player.pos.y += ground;
        player.set_surface_height(ground);
    }

    /// Apply movement controls while preserving the shared wall collision.
    pub fn move_controlled(
        &mut self,
        player: &mut PlayerController,
        input: &InputState,
        blockers: &[Footprint],
        dt: f32,
    ) {
        // The solids carry every blocker with its height; `blockers` is
        // the same set without heights.
        let _ = blockers;
        let mut input = *input;
        input.sprint |= self.sprinting;
        input.jump |= std::mem::take(&mut self.jump);
        if self.levitating || self.landing {
            input.jump = false;
            let before = player.pos.y;
            self.move_on_solids(player, &input, dt);
            let floor = self.solids.floor(player.pos.x, player.pos.z, before);
            if self.landing {
                self.altitude = (before - 2.0 * dt).max(floor);
                self.landing = self.altitude > floor + 0.001;
            }
            self.altitude = self.altitude.max(floor);
            player.hold_altitude(before + (self.altitude - before).clamp(-2.0 * dt, 3.0 * dt));
        } else {
            let (feet, speed) = (player.pos.y, player.vertical_speed());
            self.move_on_solids(player, &input, dt);
            self.spells
                .after_step(player, feet, speed, &self.solids, dt);
        }
    }

    /// Casts `spell` for `player`, or ends it when it is the live
    /// concentration spell. Reverse Gravity ends levitation: the player
    /// falls upward instead.
    ///
    /// # Errors
    ///
    /// Returns why the spell's rules refused the cast.
    pub fn cast_spell(
        &mut self,
        spell: spells::Spell,
        player: &PlayerController,
    ) -> Result<(), String> {
        self.spells.cast(spell, player, &self.solids)?;
        if spell == spells::Spell::ReverseGravity && self.spells.active(spell) {
            self.levitating = false;
            self.landing = false;
        }
        self.solids.set_spell_blocks(self.spells.blocks());
        Ok(())
    }

    /// `spell`'s hotbar slot for `player`.
    #[must_use]
    pub fn spell_slot(&self, spell: spells::Spell, player: &PlayerController) -> hotbar::Slot {
        self.spells.slot(spell, player, &self.solids)
    }

    /// The live spells as drawn around `player`.
    #[must_use]
    pub fn spell_mesh(&self, player: &PlayerController) -> Mesh {
        self.spells.mesh(player)
    }

    /// One step of the shared controller over the solids: the blockers the
    /// feet are not above, standing on the highest surface under them.
    fn move_on_solids(&self, player: &mut PlayerController, input: &InputState, dt: f32) {
        let feet = player.pos.y;
        let grounded = !player.airborne();
        let floor = self.solids.floor(player.pos.x, player.pos.z, feet);
        let blockers = self.solids.blocking(feet);
        player.pos.y -= floor;
        player.set_surface_height(0.0);
        player.update(input, dt, &blockers, HALF_EXTENT);
        player.pos.y += floor;
        let landed = self.solids.floor(player.pos.x, player.pos.z, player.pos.y);
        player.pos.y = player.pos.y.max(landed);
        if grounded && !self.levitating {
            // A small step down keeps walking instead of falling.
            player.settle_onto(landed, solids::STEP);
        } else {
            player.set_surface_height(landed);
        }
    }

    /// Levitate, or stop: the character then falls under gravity, as from
    /// a jump.
    pub fn toggle_levitate(&mut self, player: &PlayerController) {
        self.levitating = !self.levitating;
        self.landing = false;
        self.altitude = if self.levitating {
            (player.pos.y + 1.5).min(height(player.pos.x, player.pos.z) + 18.0)
        } else {
            player.pos.y
        };
    }

    /// Advances the clock and the spells, poses the player's character for
    /// `at`, and poses each of the studio's `seats`.
    pub fn tick(&mut self, dt: f32, at: &PlayerController, seats: &[studio::SeatFigure]) {
        self.elapsed = (self.elapsed + dt) % 1000.0;
        self.rendered = Self::stage(self.elapsed);
        if self.spells.tick(dt) {
            self.solids.set_spell_blocks(self.spells.blocks());
        }
        if let Some(cast) = &mut self.cast {
            cast.advance(at, seats, dt);
        }
        if let Some(job) = &mut self.bake {
            if let Some(probes) = job.poll() {
                self.probes = Some(Arc::new(probes));
            }
            if job.finished() {
                self.bake = None;
            }
        }
    }

    /// Whether the pack's character draws the player and the seats, so the
    /// studio draws no boxy figures.
    #[must_use]
    pub fn has_characters(&self) -> bool {
        self.cast.is_some()
    }

    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// The player and the seats as drawn: the posed characters, or the
    /// plaza's avatar when the pack has no character.
    pub fn player_mesh(&self, at: &PlayerController, gait: &crate::avatar::Gait) -> Mesh {
        match &self.cast {
            Some(cast) => {
                let mut figure = cast.figure();
                // Characters take the baked probes' light, so they darken
                // under the roof and the canopy as the ground does.
                if let Some(probes) = &self.probes {
                    let mut vertices = figure.vertices.as_ref().clone();
                    probes.shade(&mut vertices);
                    figure.vertices = Arc::new(vertices);
                }
                Mesh {
                    figure: Some(figure),
                    ..Mesh::default()
                }
            }
            None => crate::avatar::mesh(at, gait),
        }
    }

    /// What the player's character is doing, when the pack has one.
    #[cfg(test)]
    pub fn player_motion(&self) -> Option<player::Motion> {
        self.cast.as_ref().map(player::Cast::motion)
    }

    /// The HUD caption for a player standing at `at`: the station in
    /// reach, and how this device opens the panel there.
    pub fn caption(at: Vec3, hint: crate::runtime::InteractHint) -> String {
        use crate::runtime::InteractHint;
        let near = station_near(at.x, at.z);
        let mut caption = match (near, hint) {
            (Some(station), _) => format!("Everglade\n{} · {}", station.studio, station.place),
            (None, InteractHint::None) => "Everglade\nWalk the glade and the workshop".into(),
            (None, _) => "Everglade\nWalk up to a station".into(),
        };
        if let Some(panel) = studio::Studio::panel_at(at) {
            match hint {
                InteractHint::Key => {
                    caption.push_str("\nF opens ");
                    caption.push_str(panel_name(&panel));
                }
                InteractHint::Tap => {
                    caption.push_str("\nTap ");
                    caption.push_str(button_label(&panel));
                    caption.push_str(" to open ");
                    caption.push_str(panel_name(&panel));
                }
                InteractHint::None => {}
            }
        }
        caption
    }
}

/// The zone panel's button for a station's panel.
#[must_use]
pub fn button_label(panel: &studio::PanelKind) -> &'static str {
    match panel {
        studio::PanelKind::Console => "Console",
        studio::PanelKind::Desk(_) | studio::PanelKind::Seat(_) => "Seat",
        studio::PanelKind::Decisions => "Decisions",
        studio::PanelKind::Review => "Review",
        studio::PanelKind::Task(_) => "Task",
        studio::PanelKind::Library => "Library",
    }
}

/// What a panel is called in a caption or a control.
#[must_use]
pub fn panel_name(panel: &studio::PanelKind) -> &'static str {
    match panel {
        studio::PanelKind::Console => "the console",
        studio::PanelKind::Desk(_) | studio::PanelKind::Seat(_) => "the seat's panel",
        studio::PanelKind::Decisions => "the decisions",
        studio::PanelKind::Review => "the diff review",
        studio::PanelKind::Task(_) => "the task's details",
        studio::PanelKind::Library => "the memory library",
    }
}

/// The hall's interior, inset from its walls, and the camera's height
/// limit there, m: the third-person camera stays inside while the player
/// does, at about head height so it looks along the hall rather than up
/// into the roof slopes.
const INTERIOR: ([f32; 2], [f32; 2]) = ([-7.4, 1.5], [7.4, 10.5]);
const INTERIOR_TOP: f32 = 2.2;

/// Pulls the camera's `eye` toward the player's `focus` so it stays inside
/// the hall while the focus is inside, instead of looking through a wall or
/// the roof. Elsewhere `eye` is returned unchanged.
#[must_use]
pub fn keep_eye_inside(focus: Vec3, eye: Vec3) -> Vec3 {
    let (min, max) = INTERIOR;
    let within = |p: Vec3| (min[0]..=max[0]).contains(&p.x) && (min[1]..=max[1]).contains(&p.z);
    if !focus.is_finite() || !eye.is_finite() || !within(focus) {
        return eye;
    }
    let delta = eye - focus;
    let mut t = 1.0_f32;
    let limits = [
        (focus.x, delta.x, min[0], max[0]),
        (focus.z, delta.z, min[1], max[1]),
        (
            focus.y,
            delta.y,
            f32::NEG_INFINITY,
            INTERIOR_TOP.max(focus.y),
        ),
    ];
    for (start, step, low, high) in limits {
        if step > 0.0 && start + step > high {
            t = t.min((high - start) / step);
        } else if step < 0.0 && start + step < low {
            t = t.min((low - start) / step);
        }
    }
    if t >= 1.0 {
        return eye;
    }
    focus + delta * t.max(0.0)
}
