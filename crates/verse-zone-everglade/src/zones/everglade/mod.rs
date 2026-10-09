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

pub mod baked;
pub mod boards;
pub mod boats;
pub mod compute;
pub mod demolition;
pub mod detail;
pub mod draw;
pub mod floaters;
pub mod guests;
pub mod hotbar;
pub mod house_lod;
pub mod layout;
pub mod npcs;
pub mod player;
pub mod pose;
pub mod sales_floor;
pub mod scene;
pub mod signals;
pub mod solids;
pub mod spells;
pub mod studio;
#[cfg(test)]
mod tests;
pub mod time_of_day;
pub mod townsfolk;
pub mod unstick;
pub mod water;
pub mod weather;
pub mod wildlife;
pub mod world_tree;

use crate::{
    controller::{Footprint, InputState, PlayerController},
    mesh::Mesh,
    pbr::{
        Daylight, Key, Neon, relight,
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
    STATION_RANGE, STATIONS, STRONGROOM, Station, YARD, height, land, station_near,
};
use verse_world::social::everglade::{SPAWN, SPAWN_YAW};

/// Warm late-morning haze: the horizon of Everglade's daylight sky, which
/// fades the tree ring into it. The haze lies low: its density halves about
/// every 6 m of height, so the hollows fog over before the ring's high
/// ground, and it brightens toward the Sun. The Grove stands under
/// Everglade's sky, haze, and light. The city is about 270 m across: the fog
/// closes past its far districts, so the tree ring shows as haze from the
/// center, and the renderer skips every cell beyond it. The time of day
/// recolors it and its glow ([`Everglade::atmosphere`]).
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

/// The quality tier the town's water effects are budgeted for until the
/// renderer says ([`Everglade::set_water_tier`]): a phone's or a browser's
/// Medium, a desktop's High.
pub const WATER_TIER: verse_engine::quality::Tier = if cfg!(any(
    target_arch = "wasm32",
    target_os = "ios",
    target_os = "android"
)) {
    verse_engine::quality::Tier::Medium
} else {
    verse_engine::quality::Tier::High
};

/// The start of the approach path, where the return portal to the plaza
/// stood. Everglade has no arch now; the layout still keeps this spot
/// clear so the pack and the path stay where they were. The zone panel's
/// return control leaves the zone.
pub const RETURN_PORTAL: Vec3 = Vec3::new(0.0, 0.0, -32.0);
/// Spacing of the baked light probes characters sample, m. The town's
/// square is 510 m across, and the bake keeps at most 64 probes a side, so
/// the grid spans the square at 63 cells of 8.1 m.
const PROBE_CELL: f32 = 8.1;
/// How far the probe grid reaches above the highest ground, m: a
/// character's head on the ring.
const PROBE_HEADROOM: f32 = 3.0;

/// How fast a held Levitate, or a held descent, changes the altitude, m/s:
/// faster in a `dev-destruction` build, so its higher ceiling takes about
/// seven seconds to reach.
pub const CLIMB_RATE: f32 = if cfg!(feature = "dev-destruction") {
    18.0
} else {
    3.0
};
/// How high levitation reaches over the ground, m: 120 m in a
/// `dev-destruction` build, for a view over the whole town, and 18 m in
/// every public build.
pub const LEVITATE_CEILING: f32 = if cfg!(feature = "dev-destruction") {
    120.0
} else {
    18.0
};
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
    /// A spell's cast playing on the player's character: seconds in, and
    /// the cast's length ([`Self::begin_spell`]).
    spell: Option<(f32, f32)>,
    pub sprinting: bool,
    pub altitude: f32,
    /// How many times levitation's climb rate and ceiling the flier has:
    /// one, or more for the Grove's dragon.
    lift: f32,
    pub jump: bool,
    landing: bool,
    /// The held Levitate press, if any.
    hold: Option<Hold>,
    /// Watches the walking player for a pocket they can't leave.
    watch: unstick::Watch,
    pub solids: solids::Solids,
    spells: spells::Spells,
    rendered: Mesh,
    cast: Option<player::Cast>,
    /// The static scene's light bake while it runs.
    bake: Option<BakeJob>,
    /// The bake's probes, which light the characters once it finishes.
    probes: Option<Arc<AmbientProbes>>,
    /// The town's offline-baked light layers, when it has them
    /// ([`baked`]).
    baked: Option<baked::BakedLight>,
    /// Whether the static scene's light follows what destruction hides
    /// ([`Self::relight_destruction`]), the bake it starts from once the
    /// bake finishes, and the relight itself.
    relights: bool,
    relight_from: Option<(Arc<TexturedScene>, BakeLight, BakeSettings)>,
    relight: Option<relight::Relight>,
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
    /// Private characters the owner placed, each from its own private pack
    /// ([`guests`]), added as their packs arrive.
    guests: Vec<wildlife::Wildlife>,
    /// Another zone's light on these placements, such as the Grove's dusk:
    /// its stage at a time, whose key also lights the bake. Everglade's
    /// own afternoon without it.
    look: Option<fn(f32) -> Neon>,
    /// The stage's bloom in place of the glade's own, such as the Meteor
    /// Showcase's stronger glow around fire.
    bloom: Option<f32>,
    /// Where the haze starts and where it closes, m, in place of the
    /// glade's own, such as the Meteor Showcase's clearer air.
    fog: Option<(f32, f32)>,
    /// The town clock the sky follows ([`time_of_day`]).
    clock: town_clock::Clock,
    /// Town time at the last tick, and the light it gives.
    now: town_clock::TownTime,
    light: time_of_day::Light,
    /// The Agora's bell (`layout::agora`), in the town only: the zone
    /// draws it hanging from its yoke and swings it when it rings.
    agora_bell: Option<layout::agora::Bell>,
    /// The player's medium and breath in the town's water ([`water`]), in
    /// the town only: a zone built from other placements, such as the
    /// Grove, has no water.
    swim: Option<Box<water::Swim>>,
    /// The rowboats and lily pads afloat ([`boats`]), in the town only.
    afloat: Option<Box<boats::Afloat>>,
    /// The water's splashes, spray, droplets, drips, and steam, within
    /// the tier's particle budget, in the town only.
    water_fx: Option<water::WaterFx>,
    /// Impacts on the water since the last frame, for the ripple field.
    pulses: Vec<verse_pbr::water::Source>,
    /// The town's weather ([`weather`]), in the town only.
    sky: Option<Box<weather::Sky>>,
    /// The rain's streaks, splash-back, and drips, in the town only.
    rainfall: Option<weather::Rainfall>,
    /// Where rain drips off the eaves.
    eaves: Vec<[f32; 3]>,
    /// How wet the player's character is, and until its next drip, s.
    drying: verse_pbr::water::rain::Drying,
    drip_wait: f32,
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
        let mut creatures = wildlife::creatures(pack, &placements);
        creatures.extend(npcs::creatures());
        zone.wildlife = Some(Box::new(wildlife::Wildlife::new(pack, creatures)?));
        zone.agora_bell = Some(layout::agora::Bell::default());
        zone.swim = Some(Box::default());
        // Mist and spray where Glade Run's weir lands in its pool.
        if let Some(smoke) = &mut zone.smoke {
            smoke.start(
                "water_falls_spray",
                crate::fx::Spawn::at(water::landing()).scaled(0.4),
            );
        }
        zone.afloat = Some(Box::new(boats::Afloat::new(pack, &layout::floats())?));
        let mut fx = water::WaterFx::new(WATER_TIER);
        // Spray where the run breaks over the weir's stones, at each end
        // of its lip.
        let run = verse_world::social::everglade_water::run();
        let ([lx, lz], _, half) = run.weir_lip();
        let [tx, tz] = run.tangent_at(run.weir);
        let level = run.level_at(run.weir - 0.5);
        for side in [-0.7_f32, 0.7] {
            let at = Vec3::new(lx - tz * half * side, level + 0.05, lz + tx * half * side);
            fx.start("water_crest_spray", crate::fx::Spawn::at(at).scaled(0.6));
        }
        zone.water_fx = Some(fx);
        zone.sky = Some(Box::new(weather::Sky::everglade()));
        zone.rainfall = Some(weather::Rainfall::new(WATER_TIER));
        zone.eaves = layout::city::eaves();
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
        // Halos at the owner's house's, the Civic Hall's, the
        // belvedere's, and the Agora's flames.
        for at in layout::estate::flames()
            .into_iter()
            .chain(layout::civic::flames())
            .chain(layout::belvedere::flames())
            .chain(layout::agora::flames())
        {
            smoke.start("greco_candle_glow", crate::fx::Spawn::at(at));
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
            spell: None,
            sprinting: false,
            altitude: 0.0,
            lift: 1.0,
            jump: false,
            landing: false,
            hold: None,
            watch: unstick::Watch::default(),
            solids,
            spells: spells::Spells::default(),
            rendered: Mesh {
                neon: Some(Self::glade_stage(0.0, &time_of_day::Light::at_hours(10.5))),
                ..Mesh::default()
            },
            cast: player::Cast::new(pack, at)?,
            bake: None,
            probes: None,
            baked: None,
            // Desktops relight what breaks; a phone keeps its baked light
            // rather than hold a second copy of the town's geometry.
            relights: !cfg!(any(target_os = "ios", target_os = "android")),
            relight_from: None,
            relight: None,
            extra_blocks: Vec::new(),
            demolition: None,
            town: None,
            figure_scene: None,
            smoke: None,
            look: None,
            bloom: None,
            fog: None,
            wildlife: None,
            guests: Vec::new(),
            clock: town_clock::Clock::DAYTIME,
            now: town_clock::TownTime::at_hour(0, 10.5),
            light: time_of_day::Light::at_hours(10.5),
            agora_bell: None,
            swim: None,
            afloat: None,
            water_fx: None,
            pulses: Vec::new(),
            sky: None,
            rainfall: None,
            eaves: Vec::new(),
            drying: verse_pbr::water::rain::Drying::default(),
            drip_wait: 0.0,
        })
    }

    /// Stands the private character in `pack` at `stand` ([`guests`]). The
    /// demolition yard takes no guests.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack holds no guest that can play.
    pub fn add_guest(&mut self, pack: &ZonePack, stand: guests::Stand) -> Result<(), String> {
        if self.demolition.is_some() {
            return Err("The demolition yard takes no guests".into());
        }
        self.guests.push(guests::guest(pack, stand)?);
        if let Some(block) = guests::block(stand) {
            self.extra_blocks.push(block);
            self.refresh_blocks();
        }
        Ok(())
    }

    /// The private characters, for tests and captures.
    #[must_use]
    pub fn guests(&self) -> &[wildlife::Wildlife] {
        &self.guests
    }

    /// How many private characters stand in the zone.
    #[must_use]
    pub fn guest_count(&self) -> usize {
        self.guests.len()
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
        // The dev bar's Meteor Swarm calls down the showcase's eight arcs.
        if hotbar::DEV_DESTRUCTION {
            town.set_volley(demolition::meteor::Volley::SHOWCASE);
        }
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
        self.start_wreckage_with_houses(pack, placements, scene, None)
    }

    /// As [`Everglade::start_wreckage`], with the zone's own kit houses
    /// claiming their lots in place of Everglade's town houses
    /// ([`demolition::town::Town::standalone_with_houses`]).
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a placed model.
    pub fn start_wreckage_with_houses(
        &mut self,
        pack: &ZonePack,
        placements: &[layout::Placement],
        scene: Arc<TexturedScene>,
        houses: Option<&[layout::kit_house::KitHouse]>,
    ) -> Result<(), String> {
        let mut town = match houses {
            Some(houses) => {
                demolition::town::Town::standalone_with_houses(pack, placements, scene, houses)?
            }
            None => demolition::town::Town::standalone(pack, placements, scene)?,
        };
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

    /// The town's broken chunks as GPU instances, lit by the probes, when
    /// the town draws them so ([`demolition::town::Town::set_instanced`]).
    #[must_use]
    pub fn town_instances(&self) -> Vec<crate::pbr::textured::Instances> {
        self.town
            .as_ref()
            .map(|town| town.instances(self.probes.as_deref()))
            .unwrap_or_default()
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

    /// Direct impact light from the yard, town, and staged casters.
    #[must_use]
    pub fn flash_lamps(&self, eye: Vec3) -> [crate::pbr::Lamp; crate::pbr::MAX_FLASH_CANDIDATES] {
        if let Some(yard) = &self.demolition {
            yard.swarm().flash_lamps(eye)
        } else if let Some(town) = &self.town {
            town.flash_lamps(eye)
        } else {
            [crate::pbr::Lamp::OFF; crate::pbr::MAX_FLASH_CANDIDATES]
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

    /// Sets where the haze starts and where it closes, m, in place of the
    /// glade's own, for a zone with clearer air.
    pub fn set_fog(&mut self, start: f32, end: f32) {
        let start = start.clamp(1.0, 2_000.0);
        self.fog = Some((start, end.clamp(start + 1.0, 4_000.0)));
        self.rendered = self.stage(self.elapsed);
    }

    /// Sets the stage's bloom, the glow that bright fire spreads into what
    /// surrounds it, in place of the glade's own.
    pub fn set_bloom(&mut self, bloom: f32) {
        self.bloom = Some(bloom.clamp(0.0, 0.3));
        self.rendered = self.stage(self.elapsed);
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
        // The kit town's offline-baked layers, when they were baked for this
        // scene; the job bakes at load otherwise.
        let choice = baked::choice(&self.light);
        self.baked = choice
            .as_ref()
            .map(|choice| baked::BakedLight::new(choice, scene.clone()));
        self.relight = None;
        self.relight_from = self.relights.then(|| {
            scene.baked.keep_delivered();
            (scene.clone(), light, settings)
        });
        self.bake = Some(BakeJob::start_layered(scene, light, settings, key, choice));
        self.probes = None;
    }

    /// Relights what destruction hides from the static scene from now on
    /// (`pbr::relight`): when the town's buildings break, the vertices and
    /// probes near the gaps are traced again without the broken pieces, so
    /// no baked shade floats where a wall stood and rubble takes the light
    /// of the open lot, and when they are restored the baked light returns.
    /// Call it before [`Self::bake_light`]. Desktops relight by default,
    /// the town over its offline-baked layers too (#10907); the Meteor
    /// Showcase asks explicitly (#10938).
    pub fn relight_destruction(&mut self) {
        self.relights = true;
    }

    /// Triangles hidden, vertices and probes relit by the last relight to
    /// land, once one has.
    #[must_use]
    pub fn relit(&self) -> Option<relight::RelightStats> {
        self.relight.as_ref().and_then(relight::Relight::last)
    }

    /// The physical stage: the time of day's sky ([`time_of_day`]), whose
    /// horizon haze is the zone's air and fog, a Sun or Moon that casts
    /// shadows over the clearing and stands in the sky where the shadows
    /// say it is, and the sky's own light as fill, with low height fog.
    /// Textured meshes draw only on a lit stage.
    fn stage(&self, time: f32) -> Mesh {
        let mut neon = self.look.map_or_else(
            || Neon {
                // The running clock moves the sky a step at a time, so its
                // light rebakes over frames rather than stalling one.
                sky_gradual: self.clock.pinned_hour().is_none(),
                ..Self::glade_stage(time, &self.light)
            },
            |look| look(time),
        );
        // The town's weather over its sky ([`weather`]).
        if let (None, Some(sky)) = (self.look, &self.sky) {
            weather::weather_stage(&mut neon, &sky.weather);
        }
        if let Some(bloom) = self.bloom {
            neon.bloom = bloom;
        }
        if self.look.is_none() && self.fog.is_some() {
            let air = self.atmosphere();
            neon.fog_start = air.fog_start;
            neon.fog_end = air.fog_end;
            neon.height_fog = air.height_fog;
        }
        neon.baked_lamps = self.baked_lamps();
        Mesh {
            neon: Some(neon),
            ..Mesh::default()
        }
    }

    /// The glade's stage under `light`: the late-morning stage's shape with
    /// the time of day's sky, haze, and key.
    fn glade_stage(time: f32, light: &time_of_day::Light) -> Neon {
        let air = Self::air(light);
        Neon {
            field: air.color,
            fog_start: air.fog_start,
            fog_end: air.fog_end,
            line_gain: 1.0,
            line_width: 1.4,
            bloom: 0.04,
            vignette: 0.15,
            time,
            key: Some(light.key(Self::afternoon())),
            daylight: Some(light.daylight(Daylight {
                zenith: [0.10, 0.30, 0.73],
                horizon: air.color,
                sun: [1.0, 0.8, 0.54],
                clouds: 0.38,
                // Grass and leaf litter: under the key and the sky it
                // returns about the irradiance the key's ground fill
                // gave surfaces facing down.
                ground: [0.10, 0.11, 0.07],
                glow: 0.0,
            })),
            height_fog: air.height_fog,
            key_color: light.key_color,
            rim_color: light.rim_color,
            ..Neon::plaza(time)
        }
    }

    /// The zone's air under `light`: the late-morning haze in the time of
    /// day's horizon color, glowing toward the key as strongly as the
    /// light says.
    fn air(light: &time_of_day::Light) -> super::Atmosphere {
        let mut air = ATMOSPHERE;
        air.color = light.horizon;
        if let Some(fog) = air.height_fog.as_mut() {
            fog.sun_strength = light.haze_glow;
        }
        air
    }

    /// The zone's air with the town clock pinned at `hour`, as a zone that
    /// pins it declares its atmosphere.
    #[must_use]
    pub fn air_at(hour: f32) -> super::Atmosphere {
        Self::air(&time_of_day::Light::at_hours(hour))
    }

    /// The zone's air now, which follows the town clock.
    #[must_use]
    pub fn atmosphere(&self) -> super::Atmosphere {
        let mut air = Self::air(&self.light);
        if let Some((start, end)) = self.fog {
            air.fog_start = start;
            air.fog_end = end;
            if let Some(fog) = air.height_fog.as_mut() {
                fog.start = start;
            }
        }
        air
    }

    /// Sets the town clock the sky follows, such as one with its hour
    /// pinned for a capture, and applies it at once.
    pub fn set_clock(&mut self, clock: town_clock::Clock) {
        self.clock = clock;
        self.advance_clock();
        self.rendered = self.stage(self.elapsed);
    }

    /// The town clock the sky follows.
    #[must_use]
    pub fn clock(&self) -> town_clock::Clock {
        self.clock
    }

    /// Town time at the last tick.
    #[must_use]
    pub fn town_time(&self) -> town_clock::TownTime {
        self.now
    }

    /// The time of day's light at the last tick, whose
    /// [`time_of_day::Light::lamps_lit`] says whether the lamps burn.
    #[must_use]
    pub fn light(&self) -> &time_of_day::Light {
        &self.light
    }

    /// Whether the installed scene matches and uses its offline light layers.
    #[must_use]
    pub fn uses_baked_light(&self) -> bool {
        self.baked.as_ref().is_some_and(|b| b.active)
    }

    /// Reads the real time into the town clock and its light.
    fn advance_clock(&mut self) {
        self.now = self.clock.at(unix_now());
        // A pinned hour never changes, so it lights exactly, unstepped:
        // the default daytime is the old fixed 10:30 light.
        self.light = match self.clock.pinned_hour() {
            Some(hour) => time_of_day::Light::at_hours(hour as f32),
            None => time_of_day::Light::at(self.now),
        };
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
        // The ponds and Glade Run: every pond is swimmable, so no bank
        // blocks walking into one.
        let surface = water::surface()?;
        world.mesh.water = Some(Arc::new(surface));
        world
            .blockers
            .extend(layout::city::blocks().into_iter().map(|(f, _)| f));
        world
            .blockers
            .extend(npcs::blocks().into_iter().map(|(f, _)| f));
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
            if self.swim.is_some() {
                player.set_pace(1.0);
            }
            let before = player.pos.y;
            self.move_on_solids(player, &input, dt);
            let floor = self.solids.floor(player.pos.x, player.pos.z, before);
            if self.landing {
                self.altitude = (before - 2.0 * dt).max(floor);
                self.landing = self.altitude > floor + 0.001;
            }
            self.altitude = self.altitude.max(floor);
            // The character keeps up with a held climb, and sinks two
            // thirds as fast.
            let rise = self.climb_rate() * dt;
            player.hold_altitude(before + (self.altitude - before).clamp(-rise * 2.0 / 3.0, rise));
        } else if self
            .afloat
            .as_mut()
            .is_some_and(|afloat| afloat.ride(player, &input))
        {
            // Aboard a boat: the keys row it, and the seat carries the
            // player.
        } else {
            if let Some(afloat) = &mut self.afloat
                && let Some(k) = afloat.take_dump()
            {
                afloat.drop_in(k, player);
            }
            let (feet, speed) = (player.pos.y, player.vertical_speed());
            if let Some(swim) = &self.swim {
                // Wading and swimming are half speed (SRD 5.2.1), and each
                // Exhaustion level takes a sixth.
                player.set_pace(swim.pace());
            }
            self.move_on_solids(player, &input, dt);
            if let Some(swim) = &mut self.swim {
                swim.after_step(player, &input, feet, &self.solids, dt);
            }
            self.spells
                .after_step(player, feet, speed, &self.solids, dt);
            // A player held in a pocket they can't walk out of, such as a
            // gap below a floor, is moved to the nearest place they can.
            let pressing = input.forward
                || input.backward
                || input.strafe_left
                || input.strafe_right
                || (input.mouse_look && (input.left || input.right));
            if let Some(free) =
                self.watch
                    .after_step(&self.solids, player.pos, pressing, !player.airborne(), dt)
            {
                player.pos = free;
                player.set_vertical_speed(0.0);
                player.set_surface_height(free.y);
            }
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
        if let Some(fx) = &self.water_fx {
            fx.particles.draw(&mut mesh.sprites);
        }
        if let Some(rainfall) = &self.rainfall {
            rainfall.draw_covered(&mut mesh.sprites, eye, |p| self.solids.rain_open(p));
        }
        if let Some(yard) = &self.demolition {
            mesh.extend(&yard.mesh(player, eye, hold));
        } else if let Some(bell) = &self.agora_bell {
            // The Agora's bell under its yoke, in the town.
            mesh.extend(&bell.mesh(eye));
        }
        mesh
    }

    /// Rings the Agora's bell: it swings and its clapper strikes for a
    /// few seconds (`layout::agora::Bell`). The sales floor rings it when
    /// the payment ledger records a settled deal; nothing calls this yet.
    /// Returns whether the zone has the bell: the town does, and a zone
    /// built from other placements, such as the Grove, doesn't.
    pub fn ring_agora_bell(&mut self) -> bool {
        self.agora_bell
            .as_mut()
            .map(layout::agora::Bell::ring)
            .is_some()
    }

    /// The Agora's bell, in the town.
    #[must_use]
    pub fn agora_bell(&self) -> Option<&layout::agora::Bell> {
        self.agora_bell.as_ref()
    }

    /// The player's medium and breath in the town's water, in the town.
    #[must_use]
    pub fn swim(&self) -> Option<&water::Swim> {
        self.swim.as_deref()
    }

    /// The HUD's breath bar, while the player's eye is under water or its
    /// breath is short.
    #[must_use]
    pub fn breath_bar(&self) -> Option<water::BreathBar> {
        self.swim.as_ref().and_then(|swim| swim.bar())
    }

    /// Sets the camera's pitch, which steers a swimmer's stroke up or down.
    pub fn set_look_pitch(&mut self, pitch: f32) {
        if let Some(swim) = &mut self.swim {
            swim.set_pitch(pitch);
        }
    }

    /// Marks the water body `eye` is in on `mesh`'s stage, so its surface
    /// shades from below.
    pub fn see_water_from(&self, mesh: &mut Mesh, eye: Vec3) {
        if let Some(water) = mesh.neon.as_mut().and_then(|neon| neon.water.as_mut())
            && self.swim.is_some()
        {
            water::see_from(water, eye);
            let motes = water::motes(water, eye);
            mesh.sprites.extend(motes);
        }
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
    /// Plays a spell's cast on the player's character: the hand draws back
    /// over `cast` seconds while an ember gathers, then throws
    /// ([`player::SpellPose`]). A `cast` of zero plays the throw alone.
    pub fn begin_spell(&mut self, cast: f32) {
        self.spell = Some((0.0, cast.max(0.0)));
    }

    /// Whether a spell's cast or throw is playing.
    #[must_use]
    pub fn casting_spell(&self) -> bool {
        self.spell.is_some()
    }

    /// The player's right hand, in the world, as last posed: where a cast's
    /// ember gathers.
    #[must_use]
    pub fn hand(&self) -> Option<Vec3> {
        self.cast.as_ref()?.hand()
    }

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
        self.advance_clock();
        if let Some(sky) = &mut self.sky {
            sky.update(glam::Vec2::new(at.pos.x, at.pos.z));
            // Rain raises the ponds and the run for every client alike.
            verse_world::social::everglade_water::set_rise(sky.rise, sky.flow_gain);
        }
        self.rendered = self.stage(self.elapsed);
        self.tick_water(dt, at);
        if self.look.is_none() {
            // The owner's house's, the Civic Hall's, the belvedere's, and
            // the Agora's candles and lamps near the player
            // (`layout::estate`, `layout::civic`, `layout::belvedere`,
            // `layout::agora`); nothing changes elsewhere.
            if let Some(neon) = self.rendered.neon.as_mut() {
                layout::estate::light(neon, at.pos, self.elapsed);
                layout::civic::light(neon, at.pos, self.elapsed);
                layout::belvedere::light(neon, at.pos, self.elapsed);
                layout::agora::light(neon, at.pos, self.elapsed);
            }
        }
        if let Some(bell) = &mut self.agora_bell {
            bell.tick(dt);
        }
        if let Some(smoke) = &mut self.smoke {
            smoke.tick(dt, height);
        }
        if self.spells.tick(dt) {
            self.refresh_blocks();
        }
        // A spell's cast plays over the clip until its throw ends.
        let pose = self
            .spell
            .and_then(|(t, cast)| player::SpellPose::at(t, cast));
        self.spell = self
            .spell
            .filter(|_| pose.is_some())
            .map(|(t, cast)| (t + dt, cast));
        if let Some(cast) = &mut self.cast {
            let chop = match (&self.demolition, &self.town) {
                (Some(yard), _) => yard.chop(),
                (None, Some(town)) => town.chop(),
                _ => None,
            };
            cast.set_swing(chop);
            cast.set_spell(pose);
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
        for guest in &mut self.guests {
            guest.tick(dt, at.pos);
            if let Some(scene) = &cast_scene {
                guest.prepare(scene);
                cast_scene = guest.joined(scene).or(cast_scene);
            }
        }
        if let (Some(afloat), Some(scene)) = (&mut self.afloat, &cast_scene) {
            afloat.prepare(scene);
            cast_scene = afloat.joined(scene).or(cast_scene);
        }
        let solids = self.town.as_mut().and_then(|town| {
            town.tick(dt, at);
            town.take_solids()
        });
        #[allow(unused_mut)]
        let mut solids_ms = 0.0;
        if let Some(solids) = solids {
            #[cfg(not(target_arch = "wasm32"))]
            let started = std::time::Instant::now();
            self.solids = solids;
            self.refresh_blocks();
            #[cfg(not(target_arch = "wasm32"))]
            {
                solids_ms = started.elapsed().as_secs_f32() * 1000.0;
            }
        }
        if let Some(town) = &mut self.town {
            town.note_solids(solids_ms);
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
        self.poll_bake();
        if let Some(town) = &mut self.town {
            town.settle_rubble(self.probes.as_deref());
        }
    }

    /// Takes a finished light bake's probes, and follows the key light with
    /// the baked layers once the town has them.
    fn poll_bake(&mut self) {
        if let Some(job) = &mut self.bake {
            if let Some(probes) = job.poll() {
                self.probes = Some(Arc::new(probes));
            }
            if job.finished() {
                if let Some(baked) = &mut self.baked {
                    baked.active = job.layered();
                    if baked.active {
                        eprintln!("verse: Everglade's light comes from its baked layers");
                    }
                }
                self.bake = None;
            }
        }
        if self.bake.is_none()
            && let Some((scene, light, settings)) = self.relight_from.take()
            && let (Some(base), Some(probes)) = (scene.baked.delivered(), &self.probes)
        {
            self.relight = Some(relight::Relight::start(
                scene,
                light,
                settings,
                base,
                probes.as_ref().clone(),
            ));
        }
        if let Some(probes) = self.relight.as_mut().and_then(relight::Relight::poll) {
            self.probes = Some(Arc::new(probes));
        }
        if self.bake.is_none()
            && let Some(combined) = self.baked.as_mut().and_then(|b| b.update(&self.light))
        {
            // What destruction opened stays relit over the new hour's light
            // (#10907); without a relight the light goes straight to the
            // renderer.
            if let Some(relight) = &mut self.relight {
                relight.rebase(Arc::new(combined.lights), combined.probes.clone());
            } else if let Some(baked) = &self.baked {
                baked.scene.baked.deliver_lights(combined.lights);
            }
            self.probes = Some(Arc::new(combined.probes));
        }
    }

    /// How brightly the baked lamp layer burns now: zero without one.
    fn baked_lamps(&self) -> f32 {
        if self.baked.as_ref().is_some_and(|b| b.active) {
            baked::lamp_intensity(&self.light)
        } else {
            0.0
        }
    }

    /// The water's part of a frame: the boats and pads, the effects of what
    /// struck the water, and the frame's water with every mover and impact
    /// as a source for the ripple field and the boats' hulls masked out.
    fn tick_water(&mut self, dt: f32, at: &PlayerController) {
        let Some(swim) = self.swim.as_deref_mut() else {
            return;
        };
        let mut frame = water::frame(self.elapsed);
        swim.ring(&mut frame);
        // The weather: rain on the water and the ground, the ponds and the
        // run risen, the rain's effects around the player, and the
        // player's character wet from the water or the rain.
        if let Some(sky) = &self.sky {
            frame.rain = sky.rain();
            for body in &mut frame.bodies[..frame.count] {
                body.level += sky.rise as f32;
            }
            let immersed = swim.medium.wet();
            let exposed = self.solids.rain_open(at.pos + Vec3::Y * 1.6);
            self.drying.tick(
                dt,
                immersed,
                if exposed {
                    sky.weather.rain as f32
                } else {
                    0.0
                },
            );
            if self.drying.wet > 0.0 {
                frame.rain.figure = [at.pos.x, at.pos.y, at.pos.z, self.drying.wet * 0.8];
            }
            self.drip_wait -= dt;
            if !immersed && self.drying.wet > 0.35 && self.drip_wait <= 0.0 {
                self.drip_wait = 1.2 / self.drying.wet;
                if let Some(fx) = &mut self.water_fx {
                    fx.start(
                        "water_drips",
                        crate::fx::Spawn::at(at.pos + Vec3::Y * 0.9).scaled(0.7),
                    );
                }
            }
            if let Some(rainfall) = &mut self.rainfall {
                let forward = at.forward();
                rainfall.tick_covered(
                    dt,
                    at.pos + Vec3::Y * 1.6,
                    forward,
                    &sky.weather,
                    &sky.ground,
                    |x, z| {
                        verse_world::social::everglade_water::surface(x, z)
                            .is_none()
                            .then(|| height(x, z))
                    },
                    verse_world::social::everglade_water::surface,
                    &self.eaves,
                    |p| self.solids.rain_open(p),
                );
                for source in &rainfall.sources {
                    frame.add_source(*source);
                }
            }
        }
        let mut fx_starts: Vec<(&'static str, crate::fx::Spawn)> = Vec::new();
        for event in swim.take_events() {
            match event {
                water::WaterEvent::Splash { at, speed } => {
                    let scale = water::splash_scale(speed, water::CHARACTER_MASS);
                    fx_starts.push(("water_entry_splash", crate::fx::Spawn::at(at).scaled(scale)));
                    self.pulses.push(verse_pbr::water::Source::impact(
                        glam::Vec2::new(at.x, at.z),
                        0.5 * scale,
                        0.05 * scale,
                    ));
                }
                water::WaterEvent::Stride { at, velocity } => {
                    fx_starts.push(("water_wade", crate::fx::Spawn::at(at).moving(velocity)))
                }
                water::WaterEvent::Drip { at } => {
                    fx_starts.push(("water_drips", crate::fx::Spawn::at(at)));
                }
            }
        }
        let mut movers: Vec<(glam::Vec2, f32)> = Vec::new();
        if let Some(source) = swim.mover() {
            movers.push((
                glam::Vec2::from(source.at),
                glam::Vec2::from(source.velocity).length(),
            ));
        }
        if let Some(afloat) = &mut self.afloat {
            afloat.tick(dt);
            for event in afloat.take_events() {
                match event {
                    verse_world::rowboat::Event::Stroke { at, .. } => {
                        // Each blade leaves the water off the beam.
                        fx_starts.push(("water_droplets", crate::fx::Spawn::at(at)));
                        self.pulses.push(verse_pbr::water::Source::impact(
                            glam::Vec2::new(at.x, at.z),
                            0.5,
                            0.015,
                        ));
                    }
                    verse_world::rowboat::Event::Capsized { boat, .. }
                    | verse_world::rowboat::Event::Broken { boat, .. } => {
                        let (keel, _) = afloat.fleet.frame(boat);
                        let p = keel.as_vec3();
                        let top =
                            verse_world::social::everglade_water::surface(p.x, p.z).unwrap_or(p.y);
                        let at = Vec3::new(p.x, top, p.z);
                        fx_starts
                            .push(("water_entry_splash", crate::fx::Spawn::at(at).scaled(1.6)));
                        self.pulses.push(verse_pbr::water::Source::impact(
                            glam::Vec2::new(at.x, at.z),
                            1.2,
                            0.08,
                        ));
                    }
                    _ => {}
                }
            }
            for source in afloat.sources() {
                movers.push((
                    glam::Vec2::from(source.at),
                    glam::Vec2::from(source.velocity).length(),
                ));
                frame.add_source(source);
            }
            for hull in afloat.hulls() {
                frame.add_hull(hull);
            }
        }
        if let Some(wildlife) = &mut self.wildlife {
            for &(p, v) in wildlife.swimmers() {
                let source = verse_pbr::water::Source::mover(
                    glam::Vec2::new(p.x, p.z),
                    glam::Vec2::new(v.x, v.z),
                    0.2,
                );
                movers.push((
                    glam::Vec2::new(p.x, p.z),
                    glam::Vec2::new(v.x, v.z).length(),
                ));
                frame.add_source(source);
            }
            // Ducks steer round the boats.
            let avoid = self.afloat.as_ref().map_or_else(Vec::new, |afloat| {
                (0..afloat.fleet.boats.len())
                    .filter(|&k| afloat.fleet.boats[k].state != verse_world::rowboat::State::Broken)
                    .map(|k| {
                        let (keel, _) = afloat.fleet.frame(k);
                        (glam::Vec2::new(keel.x as f32, keel.z as f32), 2.4)
                    })
                    .collect()
            });
            wildlife.set_avoid(avoid);
        }
        if let Some(town) = &mut self.town {
            for (p, v) in town.site().floating() {
                frame.add_source(verse_pbr::water::Source::mover(
                    glam::Vec2::new(p.x, p.z),
                    glam::Vec2::new(v.x, v.z),
                    0.25,
                ));
            }
            for splash in town.site_mut().take_splashes() {
                let scale = water::splash_scale(splash.speed, splash.mass);
                if splash.hiss {
                    fx_starts.push(("water_steam_puff", crate::fx::Spawn::at(splash.at)));
                }
                fx_starts.push((
                    "water_entry_splash",
                    crate::fx::Spawn::at(splash.at).scaled(scale),
                ));
                self.pulses.push(verse_pbr::water::Source::impact(
                    glam::Vec2::new(splash.at.x, splash.at.z),
                    0.4 * scale,
                    0.04 * scale,
                ));
            }
        }
        if let Some(afloat) = &mut self.afloat {
            afloat.set_movers(movers);
        }
        for pulse in self.pulses.drain(..) {
            frame.add_source(pulse);
        }
        if let Some(fx) = &mut self.water_fx {
            for (name, spawn) in fx_starts {
                fx.start(name, spawn);
            }
            fx.particles.tick(dt, height);
        }
        let _ = at;
        if let Some(neon) = self.rendered.neon.as_mut() {
            neon.water = Some(frame);
        }
    }

    /// The town's weather ([`weather`]), in the town.
    #[must_use]
    pub fn sky(&self) -> Option<&weather::Sky> {
        self.sky.as_deref()
    }

    /// The town's weather, to pin a state or fix the tick for a capture.
    pub fn sky_mut(&mut self) -> Option<&mut weather::Sky> {
        self.sky.as_deref_mut()
    }

    /// The rain's streaks, splash-back, and drips, in the town.
    #[must_use]
    pub fn rainfall(&self) -> Option<&weather::Rainfall> {
        self.rainfall.as_ref()
    }

    /// How wet the player's character is, 0 to 1.
    #[must_use]
    pub fn character_wetness(&self) -> f32 {
        self.drying.wet
    }

    /// The rowboats and lily pads afloat, in the town.
    #[must_use]
    pub fn afloat(&self) -> Option<&boats::Afloat> {
        self.afloat.as_deref()
    }

    /// The rowboats and lily pads, to act on.
    pub fn afloat_mut(&mut self) -> Option<&mut boats::Afloat> {
        self.afloat.as_deref_mut()
    }

    /// The water's effects, in the town.
    #[must_use]
    pub fn water_fx(&self) -> Option<&water::WaterFx> {
        self.water_fx.as_ref()
    }

    /// Budgets the water's effects for `tier`.
    pub fn set_water_tier(&mut self, tier: verse_engine::quality::Tier) {
        if let Some(fx) = &mut self.water_fx {
            fx.set_tier(tier);
        }
        if let Some(rainfall) = &mut self.rainfall {
            rainfall.set_tier(tier);
        }
    }

    /// The interact key by a boat: leaves the boat the player sits in,
    /// rights a capsized boat beside a swimmer, or boards the boat in
    /// reach. Returns what happened, or `None` with no boat in reach.
    pub fn interact_boat(&mut self, player: &mut PlayerController) -> Option<String> {
        let swimming = self.swim.as_ref().is_some_and(|s| s.medium.afloat());
        let line = self
            .afloat
            .as_mut()?
            .interact(player, swimming, &self.solids)?;
        Some(line)
    }

    /// Whether the interact key would do something with a boat for a
    /// player at `at`.
    #[must_use]
    pub fn boat_in_reach(&self, at: Vec3) -> bool {
        self.afloat.as_ref().is_some_and(|a| a.in_reach(at))
    }

    /// Waits for the light bake to finish and takes its result, for
    /// offline captures.
    pub fn settle_light(&mut self) {
        while self.bake.is_some() {
            self.poll_bake();
            if self.bake.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        self.poll_bake();
        // The baked layers combined for this hour, then relit over.
        for _ in 0..3_000 {
            if self.baked.as_ref().is_none_or(|b| b.settled(&self.light)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
            self.poll_bake();
        }
        if let Some(probes) = self.relight.as_mut().and_then(relight::Relight::settle) {
            self.probes = Some(Arc::new(probes));
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
                for guest in &self.guests {
                    figure = guest.figure(figure, self.probes.as_deref());
                }
                if let Some(afloat) = &self.afloat {
                    figure = afloat.figure(figure, self.probes.as_deref());
                }
                if let Some(yard) = &self.demolition {
                    figure = yard.figure(Some(figure));
                }
                if let Some(town) = &self.town {
                    figure = town.figure(figure, self.probes.as_deref());
                }
                Mesh {
                    figure: Some(figure),
                    instances: self.town_instances(),
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
                mesh.instances = self.town_instances();
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

/// The real time, Unix seconds, which the town clock reads. A clock before
/// 1970 reads as the epoch of Unix time.
fn unix_now() -> f64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}
