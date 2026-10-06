//! Everglade: the forest glade where the Agent Studio lives
//! (`docs/verse/everglade.md`).
//!
//! The ground is a heightfield computed in Rust: flat inside the clearing,
//! rising toward the tree ring. The glade, the workshop, and the small town
//! around them are placements of the pinned Everglade pack's models
//! (`layout`), drawn as textured,
//! alpha-tested cells on a lit stage, with the Task Wall, the desk
//! monitors, and the atrium's goal board drawn by Verse (`boards`, from
//! [`signals`]). The pack loads on portal entry.
//! The studio's stations have fixed standing points in [`STATIONS`]; the
//! Agent Studio's seats walk between them and the stations open its panels
//! ([`studio`]). The player walks the shared plaza controller over the
//! heightfield as the ritual chamber's outfitted character from the pack,
//! and the seats are the same character in their own colors and postures
//! ([`player`], [`pose`]); no companion follows.

pub mod boards;
pub mod demolition;
pub mod detail;
pub mod draw;
pub mod floaters;
pub mod hotbar;
pub mod layout;
pub mod player;
pub mod pose;
pub mod scene;
pub mod signals;
pub mod solids;
pub mod spells;
pub mod studio;
#[cfg(test)]
mod tests;
pub mod wildlife;

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
use std::sync::Arc;

use super::everglade_pack::ZonePack;

#[cfg(test)]
use verse_world::social::everglade::UNDULATION;
pub use verse_world::social::everglade::{
    CLEARING_RADIUS, HALF_EXTENT, HALL, MAX_HEIGHT, PATH_HALF_WIDTH, RING_RADIUS, RING_RISE,
    STATION_RANGE, STATIONS, STRONGROOM, Station, YARD, height, station_near,
};
use verse_world::social::everglade::{SPAWN, SPAWN_YAW};

/// Warm late-morning haze: the horizon of Everglade's daylight sky, which
/// fades the tree ring into it. The haze lies low: its density halves about
/// every 6 m of height, so the hollows fog over before the ring's high
/// ground, and it brightens toward the Sun. The Grove stands under
/// Everglade's sky, haze, and light. The city is about 270 m across: the fog
/// closes past its far districts, so the tree ring shows as haze from the
/// center, and the renderer skips every cell beyond it.
pub const ATMOSPHERE: super::Atmosphere = super::Atmosphere {
    color: [0.72, 0.66, 0.50],
    fog_start: 40.0,
    fog_end: 180.0,
    height_fog: Some(verse_engine::lighting::HeightFog {
        density: 0.005,
        base: 0.0,
        falloff: 0.12,
        start: 40.0,
        max_opacity: 0.92,
        sun_strength: 0.4,
        sun_exponent: 3.0,
    }),
};

/// The return portal, at the start of the approach path.
pub const RETURN_PORTAL: Vec3 = Vec3::new(0.0, 0.0, -32.0);
/// Spacing of the baked light probes characters sample, m. The town's
/// square is 510 m across, and the bake keeps at most 64 probes a side, so
/// the grid spans the square at 63 cells of 8.1 m.
const PROBE_CELL: f32 = 8.1;
/// How far the probe grid reaches above the highest ground, m: a
/// character's head on the ring.
const PROBE_HEADROOM: f32 = 3.0;

/// How fast a held Levitate, or a held descent, changes the altitude, m/s.
pub const CLIMB_RATE: f32 = 3.0;
/// How high levitation reaches over the ground, m.
pub const LEVITATE_CEILING: f32 = 18.0;
/// A Levitate press shorter than this is a tap, s: it rises nothing, and
/// on a player already levitating it ends the levitation.
pub const TAP: f32 = 0.25;

/// A held Levitate press: how long it has been held, s, and whether the
/// press started the levitation.
#[derive(Clone, Copy, Debug)]
struct Hold {
    held: f32,
    began: bool,
}

/// The zone's live state: its clock, the lit stage its frames draw on, and
/// the characters: the player's and the studio's seats.
pub struct Everglade {
    elapsed: f32,
    pub levitating: bool,
    pub sprinting: bool,
    pub altitude: f32,
    /// How many times levitation's climb rate and ceiling the flier has:
    /// one, or more for the Grove's dragon.
    lift: f32,
    pub jump: bool,
    landing: bool,
    /// The held Levitate press, if any.
    hold: Option<Hold>,
    pub solids: solids::Solids,
    spells: spells::Spells,
    rendered: Mesh,
    cast: Option<player::Cast>,
    /// The static scene's light bake while it runs.
    bake: Option<BakeJob>,
    /// The bake's probes, which light the characters once it finishes.
    probes: Option<Arc<AmbientProbes>>,
    /// Blocks another zone's rules add to the spells' own, such as the
    /// Grove's training dummies, each a footprint and its top, m.
    extra_blocks: Vec<(crate::controller::Footprint, f32)>,
    /// The demolition yard, when Everglade opened as one (`--demolition`).
    demolition: Option<Box<demolition::Demolition>>,
    /// The town's destructible buildings, once the zone's static scene is
    /// in place ([`Self::start_town`]).
    town: Option<Box<demolition::town::Town>>,
    /// The figure scene the town's chunks join when another zone draws its
    /// own characters, such as the Grove's dummies; the character's
    /// otherwise.
    figure_scene: Option<Arc<TexturedScene>>,
    /// Wood smoke rising from the town's chimneys (`layout::details`), and
    /// butterflies over its flower drifts.
    smoke: Option<crate::fx::Particles>,
    /// The town's ambient creatures ([`wildlife`]).
    wildlife: Option<Box<wildlife::Wildlife>>,
    /// Another zone's light on these placements, such as the Grove's dusk:
    /// its stage at a time, whose key also lights the bake. Everglade's
    /// own afternoon without it.
    look: Option<fn(f32) -> Neon>,
}

impl Everglade {
    /// The zone with `pack`'s player character standing at `at`. A pack
    /// without a character leaves the player as the plaza's avatar.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack's character cannot play.
    pub fn new(pack: &ZonePack, at: &PlayerController) -> Result<Self, String> {
        let placements = layout::placements();
        let mut zone = Self::with_solids(pack, at, solids::build(pack, &placements)?)?;
        zone.smoke = Some(Self::chimney_smoke(&placements));
        let creatures = wildlife::creatures(pack, &placements);
        zone.wildlife = Some(Box::new(wildlife::Wildlife::new(pack, creatures)?));
        Ok(zone)
    }

    /// One plume of smoke over each of the town's chimneys, already risen,
    /// so the town is not seen lighting its fires, and butterflies over
    /// every other spring and summer flower drift among `placements`.
    fn chimney_smoke(placements: &[layout::Placement]) -> crate::fx::Particles {
        let mut smoke = crate::fx::Particles::new(0x5E0C_E1AD);
        for [x, y, z] in layout::details::chimneys() {
            smoke.start("chimney_smoke", crate::fx::Spawn::at(Vec3::new(x, y, z)));
        }
        // And over each campfire in the woods.
        for p in placements.iter().filter(|p| p.model == "foliage/campfire") {
            let [x, z] = p.at;
            smoke.start(
                "chimney_smoke",
                crate::fx::Spawn::at(Vec3::new(x, height(x, z) + 0.5, z)),
            );
        }
        let drifts = placements.iter().filter(|p| {
            matches!(
                p.model,
                "generated/flower_patch_spring" | "generated/flower_patch_summer"
            )
        });
        for p in drifts.step_by(2) {
            let [x, z] = p.at;
            smoke.start(
                "butterflies",
                crate::fx::Spawn::at(Vec3::new(x, height(x, z) + 0.5, z)),
            );
        }
        for _ in 0..32 {
            smoke.tick(0.25, height);
        }
        smoke
    }

    /// The zone's movement, spells, and characters over `solids`, for a
    /// zone built from other placements of the same pack (the Grove).
    ///
    /// # Errors
    ///
    /// Returns a message when the pack's character cannot play.
    pub fn with_solids(
        pack: &ZonePack,
        at: &PlayerController,
        solids: solids::Solids,
    ) -> Result<Self, String> {
        Ok(Self {
            elapsed: 0.0,
            levitating: false,
            sprinting: false,
            altitude: 0.0,
            lift: 1.0,
            jump: false,
            landing: false,
            hold: None,
            solids,
            spells: spells::Spells::default(),
            rendered: Mesh {
                neon: Some(Self::glade_stage(0.0)),
                ..Mesh::default()
            },
            cast: player::Cast::new(pack, at)?,
            bake: None,
            probes: None,
            extra_blocks: Vec::new(),
            demolition: None,
            town: None,
            figure_scene: None,
            smoke: None,
            look: None,
            wildlife: None,
        })
    }

    /// Lets the sledgehammer and Meteor Swarm break the town's buildings,
    /// drawn in `scene`, the static scene [`Self::world`] built from
    /// `pack` ([`demolition::town`]).
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a placed model.
    pub fn start_town(&mut self, pack: &ZonePack, scene: Arc<TexturedScene>) -> Result<(), String> {
        let mut town = demolition::town::Town::new(pack, &layout::placements(), scene)?;
        town.set_track(
            self.cast
                .as_ref()
                .and_then(player::Cast::swing_track)
                .cloned(),
        );
        if let Some(solids) = town.take_solids() {
            self.solids = solids;
            self.refresh_blocks();
        }
        self.town = Some(Box::new(town));
        Ok(())
    }

    /// Lets Meteor Swarm, the Thunderbolt, and the sledgehammer break the
    /// destructible models among `placements` outside Everglade's town,
    /// drawn in `scene`, such as the Grove's concrete tower
    /// ([`demolition::town::Town::standalone`]).
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a placed model.
    pub fn start_wreckage(
        &mut self,
        pack: &ZonePack,
        placements: &[layout::Placement],
        scene: Arc<TexturedScene>,
    ) -> Result<(), String> {
        let mut town = demolition::town::Town::standalone(pack, placements, scene)?;
        town.set_track(
            self.cast
                .as_ref()
                .and_then(player::Cast::swing_track)
                .cloned(),
        );
        if let Some(solids) = town.take_solids() {
            self.solids = solids;
            self.refresh_blocks();
        }
        self.town = Some(Box::new(town));
        Ok(())
    }

    /// The town's destructible buildings, once started.
    #[must_use]
    pub fn town(&self) -> Option<&demolition::town::Town> {
        self.town.as_deref()
    }

    /// Joins the town's chunks to `scene`'s figures from now on rather
    /// than the character's, for a zone that draws its own figure.
    pub fn set_figure_scene(&mut self, scene: Option<Arc<TexturedScene>>) {
        self.figure_scene = scene;
    }

    /// `figure`, a figure of the scene [`Self::set_figure_scene`] named,
    /// followed by the town's drawn chunks, lit by the baked probes.
    #[must_use]
    pub fn with_town(&self, figure: crate::pbr::textured::Figure) -> crate::pbr::textured::Figure {
        match &self.town {
            Some(town) => town.figure(figure, self.probes.as_deref()),
            None => figure,
        }
    }

    /// The town's destructible buildings to act on, once started.
    pub fn town_mut(&mut self) -> Option<&mut demolition::town::Town> {
        self.town.as_deref_mut()
    }

    /// Enters `strike`'s targeting for `player` in the town, switches to
    /// it, or leaves it.
    ///
    /// # Errors
    ///
    /// Returns why the spell can't be cast now, or that there is nothing
    /// here to cast it at.
    pub fn target_strike(
        &mut self,
        strike: demolition::meteor::Strike,
        player: &PlayerController,
    ) -> Result<(), String> {
        match (&mut self.demolition, &mut self.town) {
            (None, Some(town)) => town.target(strike, player),
            _ => Err(format!("{} needs something to break", strike.name())),
        }
    }

    /// Meteor Swarm's state, in the demolition yard or the town.
    #[must_use]
    pub fn swarm(&self) -> Option<&demolition::meteor::Swarm> {
        match (&self.demolition, &self.town) {
            (Some(yard), _) => Some(yard.swarm()),
            (None, Some(town)) => Some(town.swarm()),
            _ => None,
        }
    }

    /// Enters Meteor Swarm's targeting for `player`, or leaves it.
    ///
    /// # Errors
    ///
    /// Returns why the spell can't be cast now, or that there is nothing
    /// here to cast it at.
    pub fn meteor_swarm(&mut self, player: &PlayerController) -> Result<(), String> {
        match (&mut self.demolition, &mut self.town) {
            (Some(yard), _) => yard.meteor_swarm(),
            (None, Some(town)) => town.meteor_swarm(player),
            _ => Err("Meteor Swarm needs the town's buildings".into()),
        }
    }

    /// Puts Meteor Swarm's circle where the ray from `origin` along
    /// `direction` meets the ground. Returns whether it moved.
    pub fn aim_swarm(&mut self, origin: Vec3, direction: Vec3, player: &PlayerController) -> bool {
        match (&mut self.demolition, &mut self.town) {
            (Some(yard), _) => yard.aim(origin, direction, player),
            (None, Some(town)) => town.aim(origin, direction, player),
            _ => false,
        }
    }

    /// Casts Meteor Swarm at its circle. Returns whether the cast began.
    pub fn confirm_swarm(&mut self, player: &PlayerController) -> bool {
        match (&mut self.demolition, &mut self.town) {
            (Some(yard), _) => yard.confirm(player),
            (None, Some(town)) => town.confirm(player),
            _ => false,
        }
    }

    /// Leaves Meteor Swarm's targeting or stops its cast. Returns whether
    /// there was either to stop.
    pub fn cancel_swarm(&mut self) -> bool {
        let busy = self
            .swarm()
            .is_some_and(|swarm| swarm.targeting() || swarm.casting());
        match (&mut self.demolition, &mut self.town) {
            (Some(yard), _) => yard.cancel(),
            (None, Some(town)) => town.cancel(),
            _ => {}
        }
        busy
    }

    /// How far the meteors' blasts shake the camera this frame.
    #[must_use]
    pub fn shake(&self) -> Vec3 {
        match (&self.demolition, &self.town) {
            (Some(yard), _) => yard.shake(),
            (None, Some(town)) => town.shake(),
            _ => Vec3::ZERO,
        }
    }

    /// Everglade's ground and sky with none of its layout: the demolition
    /// yard's static world, a ring of trees around an open field. The
    /// cottages draw with the player ([`Self::player_mesh`]).
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a tree or the scene exceeds
    /// the renderer's bounds.
    pub fn demolition_world(pack: &ZonePack) -> Result<World, String> {
        let mut world = World::default();
        let (mut scene, _) = scene::build(pack, &demolition::trees())?;
        // Grass only: the town's roads and ponds belong to its layout.
        draw::ground_with(&mut scene, false);
        scene.validate()?;
        world.mesh.textured = Some(Arc::new(scene));
        Ok(world)
    }

    /// Turns this zone into the demolition yard: two kit cottages from
    /// `pack` stand in the clearing and nothing else blocks the player.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a kit model.
    pub fn start_demolition(&mut self, pack: &ZonePack) -> Result<(), String> {
        // The yard has no ponds, trees, or hives for the town's creatures.
        self.wildlife = None;
        let mut yard = demolition::Demolition::new(pack)?;
        yard.set_track(
            self.cast
                .as_ref()
                .and_then(player::Cast::swing_track)
                .cloned(),
        );
        self.solids = demolition::solids();
        let blocks = yard.take_blocks().unwrap_or_default();
        self.demolition = Some(Box::new(yard));
        self.town = None;
        self.set_extra_blocks(blocks);
        Ok(())
    }

    /// The demolition yard, when this zone is one.
    #[must_use]
    pub fn demolition(&self) -> Option<&demolition::Demolition> {
        self.demolition.as_deref()
    }

    /// Swings the sledgehammer, or with `rebuild` rebuilds the yard's
    /// cottages or restores the town's buildings.
    ///
    /// # Errors
    ///
    /// Returns a message where there is nothing to swing at.
    pub fn demolish(&mut self, rebuild: bool) -> Result<(), String> {
        match (&mut self.demolition, &mut self.town) {
            (Some(yard), _) if rebuild => yard.reset(),
            (Some(yard), _) => {
                yard.swing();
            }
            (None, Some(town)) if rebuild => town.restore(),
            (None, Some(town)) => {
                town.swing();
            }
            _ => return Err("The sledgehammer needs the town's buildings".into()),
        }
        Ok(())
    }

    /// Lights these placements with `look`'s stage instead of Everglade's
    /// afternoon, before the light is baked.
    pub fn set_look(&mut self, look: fn(f32) -> Neon) {
        self.look = Some(look);
        self.rendered = self.stage(self.elapsed);
    }

    /// The key light the stage casts shadows with and the bake reads.
    fn key(&self) -> Key {
        self.look
            .and_then(|look| look(0.0).key)
            .unwrap_or_else(Self::afternoon)
    }

    /// The afternoon light: a warm sun from behind the approach that casts
    /// shadows over the clearing, a cool rim, and sky and ground fill.
    fn afternoon() -> Key {
        Key {
            dir: Vec3::new(-0.42, 0.6, -0.56).normalize(),
            illuminance: 4_000.0,
            angular_radius: 0.03,
            rim_dir: Vec3::new(0.5, 0.35, 0.6).normalize(),
            rim_illuminance: 900.0,
            rim_angular_radius: 0.1,
            sky: 1_200.0,
            ground: 450.0,
            ev100: 10.0,
            shadow_center: Vec3::new(0.0, 0.0, 0.0),
            shadow_half: 75.0,
            // Across the town: a house 120 m off still casts, and the
            // last cascade fades out before the fog closes at 180 m.
            shadow_distance: Some(150.0),
            // The glade and the workshop are the world mesh; the seats and
            // the player stay near the camera.
            cache_far_shadows: true,
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
        let light = BakeLight::from_key(&self.key());
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
    /// shadows say it is, and the sky's own light as fill, with low height
    /// fog. Textured meshes draw only on a lit stage.
    fn stage(&self, time: f32) -> Mesh {
        Mesh {
            neon: Some(
                self.look
                    .map_or_else(|| Self::glade_stage(time), |look| look(time)),
            ),
            ..Mesh::default()
        }
    }

    fn glade_stage(time: f32) -> Neon {
        let air = ATMOSPHERE;
        Neon {
            field: air.color,
            fog_start: air.fog_start,
            fog_end: air.fog_end,
            line_gain: 1.0,
            line_width: 1.4,
            bloom: 0.04,
            vignette: 0.15,
            time,
            key: Some(Self::afternoon()),
            daylight: Some(Daylight {
                zenith: [0.10, 0.30, 0.73],
                horizon: air.color,
                sun: [1.0, 0.8, 0.54],
                clouds: 0.38,
                // Grass and leaf litter: under the key and the sky it
                // returns about the irradiance the key's ground fill
                // gave surfaces facing down.
                ground: [0.10, 0.11, 0.07],
                glow: 0.0,
            }),
            height_fog: air.height_fog,
            ..Neon::plaza(time)
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
        let (mut scene, blockers) =
            scene::build_painted(pack, &layout::placements(), layout::paint)?;
        draw::ground(&mut scene);
        scene.validate()?;
        world.mesh.textured = Some(Arc::new(scene));
        world.blockers = blockers;
        world.blockers.extend(layout::board_blockers());
        world.blockers.extend(layout::pond_blockers());
        world
            .blockers
            .extend(layout::city::blocks().into_iter().map(|(f, _)| f));
        boards::draw(&mut world.mesh);
        Ok(world)
    }

    /// Everglade under the social rules profile, for a hosted instance: the
    /// same heightfield and blockers the zone walks on, built from the
    /// pinned `pack`, whose digest is the instance's content identity.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a placed model.
    pub fn social_profile(pack: &ZonePack) -> Result<verse_world::social::world::Profile, String> {
        Ok(verse_world::social::world::Profile::everglade(
            super::everglade_pack::content_digest()?,
            solids::build(pack, &layout::placements())?,
        ))
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
        if let Some(hold) = &mut self.hold {
            hold.held += dt;
            if hold.held >= TAP && self.levitating {
                let ground = height(player.pos.x, player.pos.z);
                self.altitude =
                    (self.altitude + self.climb_rate() * dt).clamp(ground, ground + self.ceiling());
            }
        }
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
            let lift = self.lift;
            player.hold_altitude(
                before + (self.altitude - before).clamp(-2.0 * lift * dt, 3.0 * lift * dt),
            );
        } else {
            let (feet, speed) = (player.pos.y, player.vertical_speed());
            self.move_on_solids(player, &input, dt);
            self.spells
                .after_step(player, feet, speed, &self.solids, dt);
        }
    }

    /// Casts `spell` for `player` from Everglade's hotbar: no cooldown,
    /// walls beside the ones standing, and a press on a live Feather Fall
    /// or Reverse Gravity ending it ([`spells::Spells::cast_ahead`]).
    /// Reverse Gravity ends levitation: the player falls upward instead.
    ///
    /// # Errors
    ///
    /// Returns why the spell's rules refused the cast.
    pub fn cast_hotbar_spell(
        &mut self,
        spell: spells::Spell,
        player: &PlayerController,
    ) -> Result<(), String> {
        if self.demolition.is_some() {
            return Err("The demolition yard has only the sledgehammer".into());
        }
        self.spells
            .cast_ahead(spell, player, &self.solids, spells::AHEAD)?;
        self.after_cast(spell);
        Ok(())
    }

    /// Casts `spell` for `player` under the chamber's concentration rule,
    /// as the Grove does, or ends it when it is the live concentration
    /// spell. Reverse Gravity ends levitation: the player falls upward
    /// instead.
    ///
    /// # Errors
    ///
    /// Returns why the spell's rules refused the cast.
    pub fn cast_spell(
        &mut self,
        spell: spells::Spell,
        player: &PlayerController,
    ) -> Result<(), String> {
        if self.demolition.is_some() {
            return Err("The demolition yard has only the sledgehammer".into());
        }
        self.cast_spell_ahead(spell, player, spells::AHEAD)
    }

    /// Casts `spell` as [`Self::cast_spell`] does, with Wall of Stone and
    /// Wind Wall standing `ahead` meters in front of the player.
    ///
    /// # Errors
    ///
    /// Returns why the spell's rules refused the cast.
    pub fn cast_spell_ahead(
        &mut self,
        spell: spells::Spell,
        player: &PlayerController,
        ahead: f64,
    ) -> Result<(), String> {
        if self.demolition.is_some() {
            return Err("The demolition yard has only the sledgehammer".into());
        }
        self.spells
            .concentrate_ahead(spell, player, &self.solids, ahead)?;
        self.after_cast(spell);
        Ok(())
    }

    fn after_cast(&mut self, spell: spells::Spell) {
        if spell == spells::Spell::ReverseGravity && self.spells.active(spell) {
            self.levitating = false;
            self.landing = false;
        }
        self.refresh_blocks();
    }

    /// Whether `spell` is live on the player.
    #[must_use]
    pub fn spell_active(&self, spell: spells::Spell) -> bool {
        self.spells.active(spell)
    }

    /// Lets the glade's spells cast without cooldowns, as the Grove does.
    pub fn set_free_casting(&mut self) {
        self.spells.set_free(true);
    }

    /// The live spells, for rules that act on more than the player.
    #[must_use]
    pub fn spells(&self) -> &spells::Spells {
        &self.spells
    }

    /// Ends every spell, as a long rest does.
    pub fn long_rest(&mut self) {
        let free = self.spells.free();
        self.spells = spells::Spells::default();
        self.spells.set_free(free);
        self.refresh_blocks();
    }

    /// Replaces the blocks another zone's rules add, each a footprint and
    /// its top, m.
    pub fn set_extra_blocks(&mut self, blocks: Vec<(crate::controller::Footprint, f32)>) {
        self.extra_blocks = blocks;
        self.refresh_blocks();
    }

    fn refresh_blocks(&mut self) {
        let mut blocks = self.spells.blocks();
        blocks.extend(self.extra_blocks.iter().copied());
        self.solids.set_spell_blocks(blocks);
    }

    /// `spell`'s hotbar slot for `player`.
    #[must_use]
    pub fn spell_slot(&self, spell: spells::Spell, player: &PlayerController) -> hotbar::Slot {
        self.spells.slot(spell, player, &self.solids)
    }

    /// The live spells as drawn around `player`, seen from its head.
    #[must_use]
    pub fn spell_mesh(&self, player: &PlayerController) -> Mesh {
        self.spell_mesh_from(player, player.pos + Vec3::Y * 1.6)
    }

    /// The live spells as drawn around `player`, seen from `eye`: the
    /// particles turn toward it. The demolition yard adds its hammer,
    /// broken faces, cracks, and dust.
    #[must_use]
    pub fn spell_mesh_from(&self, player: &PlayerController, eye: Vec3) -> Mesh {
        let hold = self.cast.as_ref().and_then(player::Cast::hold);
        // The town's targeting circle and meteors come first, so a frame
        // that has to drop glow drops the spells' particles first.
        let mut mesh = self
            .town
            .as_ref()
            .map(|town| town.mesh(player, eye, hold))
            .unwrap_or_default();
        mesh.extend(&self.spells.mesh(player, eye));
        if let Some(smoke) = &self.smoke {
            smoke.draw(&mut mesh.sprites);
        }
        if let Some(yard) = &self.demolition {
            mesh.extend(&yard.mesh(player, eye, hold));
        }
        mesh
    }

    /// What a character runs into and stands on here, which the camera
    /// also sees through.
    #[must_use]
    pub fn solids(&self) -> &solids::Solids {
        &self.solids
    }

    /// One step of the shared controller over the solids: the blockers the
    /// feet are not above, standing on the highest surface under them.
    fn move_on_solids(&self, player: &mut PlayerController, input: &InputState, dt: f32) {
        self.solids
            .step(player, input, dt, HALF_EXTENT, !self.levitating);
    }

    /// Presses Levitate and keeps it held: the player starts levitating if
    /// not already, and rises while it stays held past a tap. A repeated
    /// press while held is ignored.
    pub fn press_levitate(&mut self, player: &PlayerController) {
        if self.hold.is_some() {
            return;
        }
        let began = !self.levitating;
        if began {
            self.toggle_levitate(player);
        }
        self.hold = Some(Hold { held: 0.0, began });
    }

    /// Lets go of Levitate: the player holds the altitude reached, or, after
    /// a tap that did not start the levitation, stops levitating and falls.
    pub fn release_levitate(&mut self, player: &PlayerController) {
        if let Some(hold) = self.hold.take()
            && !hold.began
            && hold.held < TAP
            && self.levitating
        {
            self.toggle_levitate(player);
        }
    }

    /// Sets how many times levitation's climb rate and ceiling the flier
    /// has, such as the Grove's dragon's two.
    pub fn set_lift(&mut self, lift: f32) {
        self.lift = if lift.is_finite() {
            lift.clamp(0.5, 4.0)
        } else {
            1.0
        };
    }

    /// How fast a held climb or descent changes the altitude, m/s.
    #[must_use]
    pub fn climb_rate(&self) -> f32 {
        CLIMB_RATE * self.lift
    }

    /// How high levitation reaches over the ground, m.
    #[must_use]
    pub fn ceiling(&self) -> f32 {
        LEVITATE_CEILING * self.lift
    }

    /// Levitate, or stop: the character then falls under gravity, as from
    /// a jump.
    pub fn toggle_levitate(&mut self, player: &PlayerController) {
        self.levitating = !self.levitating;
        self.landing = false;
        self.altitude = if self.levitating {
            (player.pos.y + 1.5).min(height(player.pos.x, player.pos.z) + LEVITATE_CEILING)
        } else {
            player.pos.y
        };
    }

    /// Advances the clock and the spells, poses the player's character for
    /// `at`, and poses each of the studio's `seats`.
    pub fn tick(&mut self, dt: f32, at: &PlayerController, seats: &[studio::SeatFigure]) {
        self.elapsed = (self.elapsed + dt) % 1000.0;
        self.rendered = self.stage(self.elapsed);
        if let Some(smoke) = &mut self.smoke {
            smoke.tick(dt, height);
        }
        if self.spells.tick(dt) {
            self.refresh_blocks();
        }
        if let Some(cast) = &mut self.cast {
            let chop = match (&self.demolition, &self.town) {
                (Some(yard), _) => yard.chop(),
                (None, Some(town)) => town.chop(),
                _ => None,
            };
            cast.set_swing(chop);
            cast.advance(at, seats, dt);
        }
        // The characters' scene, joined to the creatures' once they draw.
        let mut cast_scene = self.cast.as_ref().map(|cast| cast.figure().scene);
        if let Some(wildlife) = &mut self.wildlife {
            wildlife.tick(dt, at.pos);
            if let Some(scene) = &cast_scene {
                wildlife.prepare(scene);
                cast_scene = wildlife.joined(scene).or(cast_scene);
            }
        }
        let solids = self.town.as_mut().and_then(|town| {
            town.tick(dt, at);
            town.take_solids()
        });
        if let Some(solids) = solids {
            self.solids = solids;
            self.refresh_blocks();
        }
        if let Some(town) = &mut self.town {
            town.prepare(self.figure_scene.as_ref().or(cast_scene.as_ref()));
        }
        let blocks = self.demolition.as_mut().and_then(|yard| {
            yard.tick(dt, at);
            yard.take_blocks()
        });
        if let Some(blocks) = blocks {
            self.set_extra_blocks(blocks);
        }
        if let Some(yard) = &mut self.demolition {
            yard.prepare(self.cast.as_ref().map(|cast| cast.figure().scene).as_ref());
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

    /// Waits for the light bake to finish and takes its result, for
    /// offline captures.
    pub fn settle_light(&mut self) {
        while let Some(job) = &mut self.bake {
            if let Some(probes) = job.poll() {
                self.probes = Some(Arc::new(probes));
            }
            if job.finished() {
                self.bake = None;
            } else {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }

    /// Whether the pack's character draws the player and the seats, so the
    /// studio draws no boxy figures.
    #[must_use]
    pub fn has_characters(&self) -> bool {
        self.cast.is_some()
    }

    /// The zone's clock, s, which its stage animates by.
    #[must_use]
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// The player and the seats as drawn: the posed characters, or the
    /// plaza's avatar when the pack has no character. With `hide_player`,
    /// the player's own character is left out, as first person needs.
    pub fn player_mesh(
        &self,
        at: &PlayerController,
        gait: &crate::avatar::Gait,
        hide_player: bool,
    ) -> Mesh {
        match &self.cast {
            Some(cast) => {
                let mut figure = if hide_player {
                    cast.figure_without_player()
                } else {
                    cast.figure()
                };
                // Characters take the baked probes' light, so they darken
                // under the roof and the canopy as the ground does.
                if let Some(probes) = &self.probes {
                    let mut vertices = figure.vertices.as_ref().clone();
                    probes.shade(&mut vertices);
                    figure.vertices = Arc::new(vertices);
                }
                if let Some(wildlife) = &self.wildlife {
                    figure = wildlife.figure(figure, self.probes.as_deref());
                }
                if let Some(yard) = &self.demolition {
                    figure = yard.figure(Some(figure));
                }
                if let Some(town) = &self.town {
                    figure = town.figure(figure, self.probes.as_deref());
                }
                Mesh {
                    figure: Some(figure),
                    ..Mesh::default()
                }
            }
            None => {
                let mut mesh = if hide_player {
                    Mesh::default()
                } else {
                    crate::avatar::mesh(at, gait)
                };
                if let Some(yard) = &self.demolition {
                    mesh.figure = Some(yard.figure(None));
                }
                if let Some(town) = &self.town {
                    mesh.figure = town.own_figure();
                }
                mesh
            }
        }
    }

    /// The pack's characters posed for this frame, without the probes'
    /// light, or `None` when the pack has no character.
    #[must_use]
    pub fn cast_figure(&self) -> Option<crate::pbr::textured::Figure> {
        self.cast.as_ref().map(player::Cast::figure)
    }

    /// Lights `vertices` with the baked probes, once the bake has them.
    pub fn shade(&self, vertices: &mut [crate::pbr::textured::TexturedVertex]) {
        if let Some(probes) = &self.probes {
            probes.shade(vertices);
        }
    }

    /// What the player's character is doing, when the pack has one.
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
