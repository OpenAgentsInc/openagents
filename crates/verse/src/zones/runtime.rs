//! Zone transition admission and the Lagrange 1, Physics Lab, and Everglade
//! simulations.

use super::{
    Control, Everglade, Intent, Lab, Lagrange, LoadState, PortalProjection, Snapshot, ZoneId,
    everglade::studio::{PanelKind, Source, Studio},
    everglade_pack,
};
use crate::{
    controller::{InputState, PlayerController},
    runtime::WorldRuntime,
};
use glam::{DVec3, Vec3};
use serde_json::{Value, json};
use verse_lagrange::{Input, PartState};

/// The display name of the OpenAgents app's bare world.
pub const GRID_LABEL: &str = "The Grid";

/// How fast a held Up or Down changes a levitating player's altitude, m/s.

impl WorldRuntime {
    pub(crate) fn update_player(&mut self, input: &InputState, dt: f32) {
        if self.is_hosted() {
            return;
        }
        if let Some(coast) = &mut self.zone_state.coast {
            let mut input = *input;
            if let Some(glade) = &mut self.zone_state.everglade {
                input.jump |= std::mem::take(&mut glade.jump);
                input.sprint |= glade.sprinting;
            }
            coast.move_player(&mut self.player, &input, self.camera.pitch, dt);
        } else if let Some(lagrange) = &mut self.zone_state.lagrange {
            lagrange.move_player(&mut self.player, input, self.camera.pitch, dt);
        } else if let Some(everglade) = &mut self.zone_state.everglade {
            // As the Grove's Giant Eagle, Jump climbs.
            let flying = self
                .zone_state
                .grove
                .as_deref()
                .and_then(super::grove::Grove::form)
                .is_some_and(super::grove::shape::Form::flies);
            if flying && input.jump {
                // The dragon lands as it takes shape; Jump takes off.
                if !everglade.levitating {
                    everglade.toggle_levitate(&self.player);
                }
                let ground = super::everglade::land(self.player.pos.x, self.player.pos.z);
                everglade.altitude = (everglade.altitude + everglade.climb_rate() * dt)
                    .clamp(ground, ground + everglade.ceiling());
            }
            // A swimmer strokes down or up along the camera's pitch.
            everglade.set_look_pitch(self.camera.pitch);
            everglade.move_controlled(&mut self.player, input, &self.world.blockers, dt);
            // In the Water Lab, the water under Water Walk, the swimming
            // level, and ice hold the character up; below them it rises.
            if let Some(floor) = self.zone_state.water.as_ref().and_then(|lab| lab.floor)
                && self.player.pos.y < floor
            {
                self.player.set_surface_height(floor);
                self.player.hold_altitude(floor);
            }
            if self.zone_state.crypt.is_some() {
                // The vault holds a levitating or jumping player under it.
                let top = super::crypt::feet_ceiling(self.player.pos.x);
                if self.player.pos.y > top {
                    self.player.hold_altitude(top);
                }
                everglade.altitude = everglade.altitude.min(top);
            }
        } else {
            self.player
                .update(input, dt, &self.world.blockers, self.zone_half());
        }
    }

    /// Configure a host-owned cache directory. This does not read or fetch
    /// assets. Everglade's pack is named by its digest.
    pub fn configure_zone_cache(&mut self, path: std::path::PathBuf) {
        self.zone_cancel_loading();
        self.zone_state.everglade_loader = Some(Box::new(everglade_pack::Loader::new(path)));
    }
    /// Whether an Everglade entry downloads the pinned medieval kit pack
    /// from the OpenAgents web origin when the cache lacks it
    /// (`everglade_pack::kit`). Off by default, so tests never reach the
    /// network; a desktop or phone build turns it on. Without the kit the
    /// town draws its committed proxies.
    pub fn download_zone_kit(&mut self, download: bool) {
        if let Some(loader) = &mut self.zone_state.everglade_loader {
            loader.download_kit(download);
        }
    }
    /// Fetch the phone tier's smaller kit and light files (#10908).
    pub fn zone_kit_phone_tier(&mut self) {
        if let Some(loader) = &mut self.zone_state.everglade_loader {
            loader.kit_tier(everglade_pack::kit::Tier::Phone);
        }
    }
    pub fn is_plaza(&self) -> bool {
        self.zone == ZoneId::Plaza
    }
    pub fn zone_half(&self) -> f32 {
        self.zone.half_extent()
    }
    pub fn zone_loading(&self) -> bool {
        self.zone_state.loading == LoadState::Loading
    }
    pub fn zone_cancel_loading(&mut self) {
        if let Some(loader) = &mut self.zone_state.everglade_loader {
            loader.cancel();
        }
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.progress = 0.0;
        self.zone_state.error = None;
    }
    /// Poll only while the native surface is active, before its presence tick.
    /// A true result invalidates the old world GPU buffers and pointer capture.
    pub fn zone_tick(&mut self) -> bool {
        if self.is_hosted() {
            return false;
        }
        let everglade = self
            .zone_state
            .everglade_loader
            .as_deref_mut()
            .and_then(everglade_pack::Loader::poll);
        #[cfg(not(target_arch = "wasm32"))]
        self.poll_private_assets();
        if !self.zone_loading() {
            return false;
        }
        match everglade {
            Some(everglade_pack::LoadEvent::Progress { received, total }) => {
                self.zone_load_progress(received, total);
            }
            Some(everglade_pack::LoadEvent::Ready(pack)) => {
                if self.zone_state.destination == ZoneId::MeteorStressTest {
                    self.install_meteor_stress_test(&pack);
                } else if self.zone_state.destination == ZoneId::MeteorShowcase {
                    self.install_meteor_showcase(&pack);
                } else if self.zone_state.destination == ZoneId::Grove {
                    self.install_grove(&pack);
                } else if self.zone_state.destination == ZoneId::Crypt {
                    self.install_crypt(&pack);
                } else if self.zone_state.destination == ZoneId::Coast {
                    self.install_coast(&pack);
                } else if self.zone_state.destination == ZoneId::WaterLab {
                    self.install_water_lab(&pack);
                } else {
                    self.install_everglade(&pack);
                }
                return self.zone_state.everglade.is_some();
            }
            Some(everglade_pack::LoadEvent::Failed(error)) => self.zone_load_failed(&error),
            None => {}
        }
        false
    }
    fn zone_load_progress(&mut self, received: u64, total: u64) {
        self.zone_state.progress = if total == 0 {
            0.0
        } else {
            (received as f32 / total as f32).clamp(0.0, 1.0)
        };
    }
    pub(super) fn zone_load_failed(&mut self, error: &str) {
        self.zone_state.error = Some(error.chars().take(180).collect());
        self.zone_state.loading = LoadState::Failed;
    }
    /// Enter the procedurally built L1 station. Nothing is downloaded.
    /// Finishes any pending zone light bake (offline captures).
    pub fn settle_zone_light(&mut self) {
        if let Some(lagrange) = &mut self.zone_state.lagrange {
            lagrange.settle_light();
        }
        if let Some(everglade) = &mut self.zone_state.everglade {
            everglade.settle_light();
        }
    }

    pub fn install_lagrange(&mut self) {
        if self.is_hosted() || !self.is_plaza() {
            return;
        }
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.zone_cancel_loading();
        let mut zone = Lagrange::new();
        // From the Grid, the station's guides and overlays are neutral.
        zone.neutral = self.is_bare();
        zone.tick();
        self.world = if self.is_bare() {
            Lagrange::neutral_world()
        } else {
            Lagrange::world()
        };
        self.zone_state.lagrange = Some(Box::new(zone));
        self.zone = ZoneId::Lagrange1;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(Lagrange::spawn(), Lagrange::spawn_yaw());
        self.camera = crate::camera::FollowCamera::default();
    }
    /// Enter the generated Physics Lab. Nothing is downloaded.
    pub fn install_lab(&mut self) {
        if self.is_hosted() || !self.is_plaza() {
            return;
        }
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.zone_cancel_loading();
        self.world = Lab::world();
        self.zone_state.lab = Some(Box::new(Lab::new()));
        self.zone = ZoneId::PhysicsLab;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(Lab::spawn(), Lab::spawn_yaw());
        self.camera = crate::camera::FollowCamera::default();
    }
    /// Enter Everglade with its verified pack. Offline tools use the same
    /// decoder ([`everglade_pack::ZonePack::load_local`]).
    pub fn install_everglade(&mut self, pack: &everglade_pack::ZonePack) {
        // A repeated completion cannot replace the saved plaza return pose.
        if self.is_hosted() || !self.is_plaza() {
            return;
        }
        let world = if self.zone_state.demolition {
            Everglade::demolition_world(pack)
        } else {
            Everglade::world(pack)
        };
        let world = match world {
            Ok(world) => world,
            Err(error) => {
                self.zone_load_failed(&error);
                return;
            }
        };
        let mut spawn = self.player;
        spawn.pos = Everglade::spawn();
        spawn.yaw = Everglade::spawn_yaw();
        let mut everglade = match Everglade::new(pack, &spawn) {
            Ok(everglade) => everglade,
            Err(error) => {
                self.zone_load_failed(&error);
                return;
            }
        };
        everglade.set_clock(self.zone_state.town_clock);
        if self.zone_state.demolition
            && let Err(error) = everglade.start_demolition(pack)
        {
            self.zone_load_failed(&error);
            return;
        }
        if let Some(scene) = &world.mesh.textured {
            everglade.bake_light(scene.clone());
            // The town's buildings break where the sledgehammer and
            // Meteor Swarm reach them; a town that can't is still a town.
            if !self.zone_state.demolition
                && let Err(error) = everglade.start_town(pack, scene.clone())
            {
                eprintln!("verse: Everglade's buildings stay whole: {error}");
            }
        }
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.world = world;
        self.zone_state.everglade = Some(Box::new(everglade));
        self.zone = ZoneId::Everglade;
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.error = None;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(Everglade::spawn(), Everglade::spawn_yaw());
        self.player.set_surface_height(super::everglade::height(
            self.player.pos.x,
            self.player.pos.z,
        ));
        self.camera = crate::camera::FollowCamera::default();
        // The owner's private characters join as their packs arrive.
        #[cfg(not(target_arch = "wasm32"))]
        if !self.zone_state.demolition {
            self.start_private_assets();
        }
    }
    /// Enter Everglade from pack bytes the caller already holds, such as a
    /// browser's own download. The bytes must be the pinned pack: their
    /// length and SHA-256 are checked before anything is decoded.
    pub fn install_everglade_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.install_everglade_bytes_with_kit(bytes, None)
    }
    /// [`Self::install_everglade_bytes`], with the medieval kit pack's
    /// bytes when the caller fetched them too. The kit must be the pinned
    /// kit pack; without it, or when it fails its checks, the town draws
    /// its committed proxies.
    pub fn install_everglade_bytes_with_kit(
        &mut self,
        bytes: &[u8],
        kit: Option<&[u8]>,
    ) -> Result<(), String> {
        let mut pack = everglade_pack::ZonePack::decode_pinned(bytes)?;
        if let Some(kit) = kit {
            match everglade_pack::kit::decode_pinned(kit) {
                Ok(pieces) => {
                    everglade_pack::kit::install(&mut pack, Some(&pieces));
                }
                Err(error) => eprintln!("verse: Everglade draws the kit's proxies: {error}"),
            }
        }
        self.install_everglade(&pack);
        if self.zone == ZoneId::Everglade {
            Ok(())
        } else {
            Err(self
                .zone_state
                .error
                .clone()
                .unwrap_or_else(|| "Everglade enters only from the plaza".into()))
        }
    }
    /// Installs the standalone castle with five autonomous meteor casters.
    pub fn install_meteor_stress_test(&mut self, pack: &everglade_pack::ZonePack) {
        if self.is_hosted() || !self.is_plaza() {
            return;
        }
        let mut spawn = self.player;
        spawn.pos = super::meteor_stress::SPAWN;
        spawn.yaw = 0.0;
        let (world, glade) = match super::meteor_stress::build(pack, &spawn) {
            Ok(built) => built,
            Err(error) => {
                self.zone_load_failed(&error);
                return;
            }
        };
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.world = world;
        self.zone_state.everglade = Some(Box::new(glade));
        self.zone = ZoneId::MeteorStressTest;
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.error = None;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(super::meteor_stress::SPAWN, 0.0);
        self.player
            .set_surface_height(super::everglade::land(self.player.pos.x, self.player.pos.z));
        self.camera = crate::camera::FollowCamera::default();
        self.camera.pitch = -0.2;
        self.camera.distance = 6.0;
    }

    /// Installs the Meteor Showcase: two kit houses at golden hour and a
    /// caster who calls an eight-meteor swarm down on them
    /// ([`super::meteor_showcase`]).
    pub fn install_meteor_showcase(&mut self, pack: &everglade_pack::ZonePack) {
        use super::meteor_showcase as showcase;
        if self.is_hosted() || !self.is_plaza() {
            return;
        }
        let mut spawn = self.player;
        spawn.pos = showcase::SPAWN;
        spawn.yaw = 0.0;
        let (world, glade) = match showcase::build(pack, &spawn) {
            Ok(built) => built,
            Err(error) => {
                self.zone_load_failed(&error);
                return;
            }
        };
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.world = world;
        self.zone_state.everglade = Some(Box::new(glade));
        self.zone = ZoneId::MeteorShowcase;
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.error = None;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        // Facing north up the path, at the houses.
        let _ = self.set_spawn(showcase::SPAWN, 0.0);
        self.player
            .set_surface_height(super::everglade::land(self.player.pos.x, self.player.pos.z));
        self.camera = crate::camera::FollowCamera::default();
        self.camera.pitch = -0.1;
        self.camera.distance = 7.0;
    }

    /// Stands the Meteor Showcase's caster and has them cast the eight
    /// meteors `delay` seconds from now, as its film stages the cast
    /// ([`super::meteor_showcase::stage`]). Returns whether it was staged.
    pub fn stage_meteor_showcase(&mut self, delay: f32) -> bool {
        if self.zone != ZoneId::MeteorShowcase {
            return false;
        }
        self.zone_state
            .everglade
            .as_deref_mut()
            .is_some_and(|glade| super::meteor_showcase::stage(glade, delay).is_ok())
    }

    /// Triangles hidden, vertices and probes relit by the last relighting
    /// of what destruction broke, in a zone that relights.
    #[must_use]
    pub fn everglade_relit(&self) -> Option<crate::pbr::relight::RelightStats> {
        self.zone_state.everglade.as_deref()?.relit()
    }

    /// What Everglade's town's last tick cost, in a zone with one.
    #[must_use]
    pub fn everglade_town_profile(
        &self,
    ) -> Option<super::everglade::demolition::town::TownProfile> {
        Some(self.zone_state.everglade.as_deref()?.town()?.profile())
    }

    /// Loads the Meteor Showcase from the plaza.
    pub fn enter_meteor_showcase(&mut self) -> Result<(), String> {
        if !self.is_plaza() || self.zone_loading() {
            return Err("Meteor Showcase enters only from the plaza".into());
        }
        self.zone_state.destination = ZoneId::MeteorShowcase;
        self.start_zone_load(ZoneId::MeteorShowcase)
    }

    /// Loads the standalone meteor stress test from the plaza.
    pub fn enter_meteor_stress_test(&mut self) -> Result<(), String> {
        if !self.is_plaza() || self.zone_loading() {
            return Err("Meteor Stress Test enters only from the plaza".into());
        }
        self.zone_state.destination = ZoneId::MeteorStressTest;
        self.start_zone_load(ZoneId::MeteorStressTest)
    }

    /// Enter the Grove with Everglade's verified pack: the meadow, its
    /// dummies, and Everglade's character and movement
    /// ([`super::grove`]).
    pub fn install_grove(&mut self, pack: &everglade_pack::ZonePack) {
        if self.is_hosted() || !self.is_plaza() {
            return;
        }
        let world = match super::grove::world(pack) {
            Ok(world) => world,
            Err(error) => {
                self.zone_load_failed(&error);
                return;
            }
        };
        let mut spawn = self.player;
        spawn.pos = super::grove::SPAWN;
        spawn.yaw = super::grove::SPAWN_YAW;
        let built = super::grove::glade(pack, &spawn).and_then(|mut glade| {
            let grove = super::grove::Grove::new(pack, &glade)?;
            // The tower's chunks draw in the field's figure.
            glade.set_figure_scene(Some(grove.figure_scene()));
            glade.set_extra_blocks(
                grove
                    .dummies
                    .iter()
                    .map(super::grove::dummies::Dummy::block)
                    .collect(),
            );
            Ok((glade, grove))
        });
        let (mut glade, grove) = match built {
            Ok(built) => built,
            Err(error) => {
                self.zone_load_failed(&error);
                return;
            }
        };
        if let Some(scene) = &world.mesh.textured {
            glade.bake_light(scene.clone());
            // The concrete tower breaks under Meteor Swarm and the
            // Thunderbolt; a Grove whose tower can't is still a Grove.
            if let Err(error) =
                glade.start_wreckage(pack, &super::grove::placements(), scene.clone())
            {
                eprintln!("verse: the Grove's tower stays whole: {error}");
            }
        }
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.world = world;
        self.zone_state.everglade = Some(Box::new(glade));
        self.zone_state.grove = Some(Box::new(grove));
        self.zone = ZoneId::Grove;
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.error = None;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(super::grove::SPAWN, super::grove::SPAWN_YAW);
        self.player
            .set_surface_height(super::everglade::land(self.player.pos.x, self.player.pos.z));
        self.camera = crate::camera::FollowCamera::default();
    }
    /// Enter the Grove from Everglade's pack bytes the caller already
    /// holds, as [`Self::install_everglade_bytes`] does.
    pub fn install_grove_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let pack = everglade_pack::ZonePack::decode_pinned(bytes)?;
        self.install_grove(&pack);
        if self.zone == ZoneId::Grove {
            Ok(())
        } else {
            Err(self
                .zone_state
                .error
                .clone()
                .unwrap_or_else(|| "The Grove enters only from the plaza".into()))
        }
    }
    /// Enter the crypt lab: the hall from the models built into this
    /// binary, walked with Everglade's character, movement, and spells from
    /// its verified pack ([`super::crypt`]).
    pub fn install_crypt(&mut self, pack: &everglade_pack::ZonePack) {
        if self.is_hosted() || !self.is_plaza() {
            return;
        }
        match super::crypt::Hall::embedded() {
            Ok(hall) => self.install_crypt_hall(pack, hall),
            Err(error) => self.zone_load_failed(&error),
        }
    }

    /// Enter the crypt lab built from `hall`, as [`Self::install_crypt`]
    /// does with the built-in models.
    pub fn install_crypt_hall(
        &mut self,
        pack: &everglade_pack::ZonePack,
        hall: super::crypt::Hall,
    ) {
        use super::crypt;
        if self.is_hosted() || !self.is_plaza() {
            return;
        }
        let mut spawn = self.player;
        spawn.pos = crypt::SPAWN;
        spawn.yaw = crypt::SPAWN_YAW;
        let built = crypt::solids()
            .and_then(|solids| Everglade::with_solids(pack, &spawn, solids))
            .and_then(|glade| Ok((glade, crypt::blocks()?)));
        let (glade, blocks) = match built {
            Ok(built) => built,
            Err(error) => {
                self.zone_load_failed(&error);
                return;
            }
        };
        let live = crypt::Crypt::new(&hall);
        let mut world = crate::world::World::default();
        // Routes go around what a step can't climb.
        world.blockers = blocks
            .into_iter()
            .filter(|&(_, top)| top > super::everglade::solids::STEP)
            .map(|(footprint, _)| footprint)
            .collect();
        world.mesh.textured = Some(std::sync::Arc::new(hall.scene));
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.world = world;
        self.zone_state.everglade = Some(Box::new(glade));
        self.zone_state.crypt = Some(Box::new(live));
        self.zone = ZoneId::Crypt;
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.error = None;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(crypt::SPAWN, crypt::SPAWN_YAW);
        self.player.set_surface_height(0.0);
        self.camera = crate::camera::FollowCamera::default();
    }

    /// Enter the coastal shell using the verified pack's character.
    pub fn install_coast(&mut self, pack: &everglade_pack::ZonePack) {
        use super::coast;
        if self.is_hosted() || !self.is_plaza() {
            return;
        }
        let mut spawn = self.player;
        spawn.pos = coast::SPAWN;
        spawn.yaw = coast::SPAWN_YAW;
        let built = (|| {
            let glade = Everglade::with_solids(
                pack,
                &spawn,
                super::everglade::solids::Solids::over(coast::ground),
            )?;
            let world = coast::world()?;
            let live = coast::Coast::new(unix_water_tick())?;
            Ok::<_, String>((glade, world, live))
        })();
        let (glade, world, live) = match built {
            Ok(built) => built,
            Err(error) => {
                self.zone_load_failed(&error);
                return;
            }
        };
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.world = world;
        self.zone_state.everglade = Some(Box::new(glade));
        self.zone_state.coast = Some(Box::new(live));
        self.zone = ZoneId::Coast;
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.error = None;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(coast::SPAWN, coast::SPAWN_YAW);
        self.player.set_surface_height(coast::SPAWN.y);
        self.camera = crate::camera::FollowCamera::default();
    }

    /// Enter the coast from an already downloaded, pinned character pack.
    pub fn install_coast_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let pack = everglade_pack::ZonePack::decode_pinned(bytes)?;
        self.install_coast(&pack);
        if self.zone == ZoneId::Coast {
            Ok(())
        } else {
            Err(self
                .zone_state
                .error
                .clone()
                .unwrap_or_else(|| "The coast enters only from the plaza".into()))
        }
    }

    /// Start loading the coast from the plaza.
    pub fn enter_coast(&mut self) -> Result<(), String> {
        if !self.is_plaza() || self.zone_loading() {
            return Err("The coast enters only from the plaza".into());
        }
        self.zone_state.destination = ZoneId::Coast;
        self.start_zone_load(ZoneId::Coast)
    }

    /// Enter the Water Lab: the cove built on Everglade's verified pack,
    /// walked with its character and movement ([`super::water`]).
    pub fn install_water_lab(&mut self, pack: &everglade_pack::ZonePack) {
        use super::water;
        if self.is_hosted() || !self.is_plaza() {
            return;
        }
        let mut spawn = self.player;
        spawn.pos = water::spawn();
        spawn.yaw = water::SPAWN_YAW;
        let built = water::solids(pack)
            .and_then(|solids| Everglade::with_solids(pack, &spawn, solids))
            .and_then(|glade| Ok((glade, water::world(pack)?)));
        let (glade, world) = match built {
            Ok(built) => built,
            Err(error) => {
                self.zone_load_failed(&error);
                return;
            }
        };
        let mut lab = water::WaterLab::new();
        // The dummies join the character's figure; without the pack's
        // dummy they still take hits, with bars and numbers.
        if let Err(error) = lab.set_model(pack, glade.cast_figure().as_ref()) {
            eprintln!("verse: the Water Lab's dummies have no model: {error}");
        }
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.world = world;
        self.zone_state.everglade = Some(Box::new(glade));
        self.zone_state.water = Some(Box::new(lab));
        self.zone = ZoneId::WaterLab;
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.error = None;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(water::spawn(), water::SPAWN_YAW);
        self.player.set_surface_height(water::spawn().y);
        self.camera = crate::camera::FollowCamera::default();
    }

    /// Enter the Water Lab from Everglade's pack bytes the caller already
    /// holds, as [`Self::install_grove_bytes`] does for the Grove.
    ///
    /// # Errors
    /// The bytes are not the pinned pack, or the lab enters only from the
    /// plaza.
    pub fn install_water_lab_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let pack = everglade_pack::ZonePack::decode_pinned(bytes)?;
        self.install_water_lab(&pack);
        if self.zone == ZoneId::WaterLab {
            Ok(())
        } else {
            Err(self
                .zone_state
                .error
                .clone()
                .unwrap_or_else(|| "The Water Lab enters only from the plaza".into()))
        }
    }

    /// The Water Lab, while the player is in it.
    #[must_use]
    pub fn water_lab(&self) -> Option<&super::water::WaterLab> {
        self.zone_state.water.as_deref()
    }

    /// The Water Lab, to change it, as a capture or a test does.
    pub fn water_lab_mut(&mut self) -> Option<&mut super::water::WaterLab> {
        self.zone_state.water.as_deref_mut()
    }

    /// Casts the Water Lab's demonstration of `spell` from `caster` toward
    /// `forward`, as the capture does, the character playing the cast: a
    /// Fireball draws back for its cast, then throws.
    pub fn water_cast_demo(
        &mut self,
        spell: super::water::Demo,
        caster: Vec3,
        forward: Vec3,
    ) -> String {
        let state = &mut self.zone_state;
        let (Some(glade), Some(lab)) = (state.everglade.as_deref_mut(), state.water.as_deref_mut())
        else {
            return String::new();
        };
        let cast = match spell {
            super::water::Demo::Fireball => super::water::FIREBALL_CAST,
            _ => 0.0,
        };
        glade.begin_spell(cast);
        lab.hand = glade.hand();
        lab.cast_demo(spell, caster, forward)
    }

    /// Presses the Water Lab's hotbar slot `index` (0-based); `alternate`
    /// (Shift) ends Control Water or casts Destroy Water.
    ///
    /// # Errors
    /// The player is not in the Water Lab, or the slot is empty.
    pub fn water_press(&mut self, index: usize, alternate: bool) -> Result<String, String> {
        let slot = *super::water::Slot::ALL
            .get(index)
            .ok_or("That slot is empty")?;
        let (at, forward, yaw) = (self.player.pos, self.player.forward(), self.player.yaw);
        let lab = self
            .zone_state
            .water
            .as_deref_mut()
            .ok_or("Enter the Water Lab first")?;
        Ok(lab.press(slot, alternate, at, forward, yaw))
    }

    /// Lets go of the Water Lab's hotbar slot `index` (0-based): the Water
    /// Orb's throws its orb, or with `alternate` (Shift) holds it in place.
    /// Returns what happened, if anything did.
    ///
    /// # Errors
    /// The player is not in the Water Lab, or the slot is empty.
    pub fn water_release(
        &mut self,
        index: usize,
        alternate: bool,
    ) -> Result<Option<String>, String> {
        let slot = *super::water::Slot::ALL
            .get(index)
            .ok_or("That slot is empty")?;
        let (at, forward) = (self.player.pos, self.player.forward());
        let lab = self
            .zone_state
            .water
            .as_deref_mut()
            .ok_or("Enter the Water Lab first")?;
        Ok(lab.release(slot, alternate, at, forward))
    }

    /// Aims the Water Lab's throws and bolts along the ray through the
    /// normalized viewport point `(x, y)` of a view with `aspect`, or ahead
    /// of the player with `None`.
    pub fn water_aim(&mut self, aspect: f32, point: Option<(f32, f32)>) {
        let ray =
            point.and_then(|(x, y)| crate::runtime::viewport_ray(&self.view(aspect), aspect, x, y));
        if let Some(lab) = self.zone_state.water.as_deref_mut() {
            lab.set_aim(ray);
        }
    }

    /// Turns the Water Lab's hour between golden hour and noon.
    ///
    /// # Errors
    /// The player is not in the Water Lab.
    pub fn water_hour(&mut self) -> Result<String, String> {
        let lab = self
            .zone_state
            .water
            .as_deref_mut()
            .ok_or("Enter the Water Lab first")?;
        Ok(lab.turn_hour())
    }

    /// Turns the Water Lab's sea from calm to moderate to storm.
    ///
    /// # Errors
    /// The player is not in the Water Lab.
    pub fn water_sea(&mut self) -> Result<String, String> {
        let lab = self
            .zone_state
            .water
            .as_deref_mut()
            .ok_or("Enter the Water Lab first")?;
        Ok(lab.turn_sea())
    }

    /// Pins the Water Lab's weather to the next state, or back to the
    /// coast's schedule.
    ///
    /// # Errors
    /// The player is not in the Water Lab.
    pub fn water_weather(&mut self) -> Result<String, String> {
        let lab = self
            .zone_state
            .water
            .as_deref_mut()
            .ok_or("Enter the Water Lab first")?;
        Ok(lab.turn_weather())
    }

    /// The Water Lab's hotbar: each slot's icon and state.
    #[must_use]
    pub fn water_bar(&self) -> Option<Vec<(&'static str, super::everglade::hotbar::Slot)>> {
        let lab = self.zone_state.water.as_deref()?;
        Some(
            super::water::Slot::ALL
                .iter()
                .map(|&slot| {
                    (
                        slot.icon(),
                        super::everglade::hotbar::Slot {
                            enabled: true,
                            active: lab.active(slot),
                            cooldown: 0.0,
                        },
                    )
                })
                .collect(),
        )
    }

    /// Start loading the Water Lab from the plaza, as `verse --water-lab`
    /// asks at launch; it loads Everglade's pack for the character.
    ///
    /// # Errors
    /// The player is not in the plaza, a load is under way, or the pack
    /// cannot be requested.
    pub fn enter_water_lab(&mut self) -> Result<(), String> {
        if !self.is_plaza() || self.zone_loading() {
            return Err("The Water Lab enters only from the plaza".into());
        }
        self.zone_state.destination = ZoneId::WaterLab;
        let result = self.start_zone_load(ZoneId::WaterLab);
        if let Err(error) = &result {
            self.zone_state.error = Some(error.chars().take(180).collect());
        }
        result
    }

    /// Whether the player stands in reach of the crypt's door.
    #[must_use]
    pub fn crypt_door_near(&self) -> bool {
        self.zone == ZoneId::Crypt && super::crypt::near_door(self.player.pos)
    }

    /// The nearest portal in this zone and its destination, or `None` in a
    /// zone without one, such as Everglade.
    fn nearest_portal(&self) -> Option<(ZoneId, Vec3)> {
        let at = self.player.pos;
        self.zone.portals().into_iter().min_by(|a, b| {
            let da = (a.1 - at).x.hypot((a.1 - at).z);
            let db = (b.1 - at).x.hypot((b.1 - at).z);
            da.total_cmp(&db)
        })
    }
    fn portal_in_reach(&self, at: Vec3) -> bool {
        // The crypt's door opens from closer than an arch.
        if self.zone == ZoneId::Crypt {
            return super::crypt::near_door(self.player.pos);
        }
        if self.zone == ZoneId::WaterLab {
            return super::water::near_exit(self.player.pos);
        }
        let offset = self.player.pos - at;
        offset.x.hypot(offset.z) <= 6.0
            && (offset.y.abs() < 3.0 || self.is_plaza() && offset.y < 3.0)
    }
    pub fn zone_intent(&mut self, intent: Intent) -> Result<(), String> {
        if self.is_hosted() {
            return Err("Hosted transitions require destination admission".into());
        }
        let result = self.apply_zone_intent(intent);
        if let Err(error) = &result {
            self.zone_state.error = Some(error.chars().take(180).collect());
        }
        result
    }
    fn apply_zone_intent(&mut self, intent: Intent) -> Result<(), String> {
        if self.zone == ZoneId::Grove
            && let Some(spell) = self
                .zone_state
                .grove
                .as_deref()
                .and_then(|grove| grove.resolve(intent))
        {
            let state = &mut self.zone_state;
            let (Some(grove), Some(glade)) =
                (state.grove.as_deref_mut(), state.everglade.as_deref_mut())
            else {
                return Err("Enter the Grove first".into());
            };
            grove.cast(spell, &mut self.player, glade)?;
            self.cancel_navigation();
            self.zone_state.error = None;
            return Ok(());
        }
        match intent {
            Intent::Enter | Intent::Retry => {
                let walked_in = intent == Intent::Retry && self.grid_retry_allowed();
                if !self.is_plaza()
                    || self.zone_loading()
                    || !(walked_in || self.zone_portal(1.0).near)
                {
                    return Err("Approach a portal".into());
                }
                let destination = if intent == Intent::Retry {
                    self.zone_state.destination
                } else {
                    self.nearest_portal().ok_or("Approach a portal")?.0
                };
                self.zone_state.destination = destination;
                if destination == ZoneId::Lagrange1 {
                    self.cancel_navigation();
                    self.doors.cancel_transient();
                    self.install_lagrange();
                    return Ok(());
                }
                if destination == ZoneId::PhysicsLab {
                    self.cancel_navigation();
                    self.doors.cancel_transient();
                    self.install_lab();
                    return Ok(());
                }
                self.start_zone_load(destination)?;
            }
            Intent::Cancel => self.zone_cancel_loading(),
            Intent::Return => {
                if self.is_plaza() {
                    return Err("You are already in the plaza".into());
                }
                let left = self.zone;
                self.zone_cancel_loading();
                // Leaving Everglade stops the studio's observation.
                self.zone_state.studio.set_active(false);
                // Dropping a zone releases its decoded frames and simulation.
                self.zone_state.lagrange = None;
                self.zone_state.lab = None;
                self.zone_state.everglade = None;
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.zone_state.private_loader = None;
                }
                self.zone_state.grove = None;
                self.zone_state.crypt = None;
                self.zone_state.water = None;
                self.zone_state.coast = None;
                // A Wild Shape's pace ends with the Grove.
                self.player.set_pace(1.0);
                self.zone = ZoneId::Plaza;
                self.zone_revision = self.zone_revision.saturating_add(1);
                if self.is_bare() {
                    // Back on the Grid in front of the portal the player
                    // left by, facing away, with the ball and blocks where
                    // they were left. With that portal hidden, back where the
                    // player left the Grid.
                    self.world = if self.has_gym() {
                        crate::world::bare()
                    } else {
                        crate::world::bare_ground()
                    };
                    let pose = self.zone_state.plaza_pose.take();
                    let gate = if left == ZoneId::Everglade {
                        self.everglade_gate()
                    } else {
                        self.grid_gate()
                    };
                    let (pos, yaw) = gate.map_or_else(
                        || pose.unwrap_or((crate::world::SPAWN, 0.0)),
                        |gate| gate.front(),
                    );
                    self.place_player(pos, yaw)?;
                } else {
                    self.world = crate::world::build();
                    let (pos, yaw) = self
                        .zone_state
                        .plaza_pose
                        .take()
                        .unwrap_or((crate::world::SPAWN, 0.0));
                    self.set_spawn(pos, yaw)?;
                }
                self.zone_state.gate_cooldown = super::gate::COOLDOWN;
                self.camera = crate::camera::FollowCamera::default();
            }
            // The Grove casts these through its kit above.
            Intent::Firebolt | Intent::MagicMissile | Intent::Fireball => {
                return Err("Enter the Grove first".into());
            }
            Intent::Forces => {
                let lagrange = self
                    .zone_state
                    .lagrange
                    .as_deref_mut()
                    .ok_or("Enter Lagrange 1 first")?;
                lagrange.overlay = !lagrange.overlay;
                lagrange.tick();
            }
            Intent::Camera => {
                let lagrange = self
                    .zone_state
                    .lagrange
                    .as_deref_mut()
                    .ok_or("Enter Lagrange 1 first")?;
                lagrange.art = !lagrange.art;
                lagrange.tick();
            }
            Intent::Grab | Intent::Release => {
                let lagrange = self
                    .zone_state
                    .lagrange
                    .as_deref_mut()
                    .ok_or("Enter Lagrange 1 first")?;
                lagrange.station.apply(if intent == Intent::Grab {
                    Input::Grab
                } else {
                    Input::Release
                })?;
                lagrange.tick();
                self.zone_state.error = None;
            }
            Intent::Tether => {
                let lagrange = self
                    .zone_state
                    .lagrange
                    .as_deref_mut()
                    .ok_or("Enter Lagrange 1 first")?;
                let station = &mut lagrange.station;
                station.apply(if station.tethered() {
                    Input::Unclip
                } else {
                    Input::Clip
                })?;
                lagrange.tick();
                self.zone_state.error = None;
            }
            Intent::KnobPrev
            | Intent::KnobNext
            | Intent::Decrease
            | Intent::Increase
            | Intent::Reset
            | Intent::Pause
            | Intent::Step => {
                let lab = self
                    .zone_state
                    .lab
                    .as_deref_mut()
                    .ok_or("Enter the Physics Lab first")?;
                match intent {
                    Intent::KnobPrev => lab.cycle_knob(false),
                    Intent::KnobNext => lab.cycle_knob(true),
                    Intent::Decrease => lab.adjust(false),
                    Intent::Increase => lab.adjust(true),
                    Intent::Reset => lab.reset(),
                    Intent::Pause => lab.toggle_pause(),
                    _ => lab.single_step(),
                }
                self.zone_state.error = None;
            }
            Intent::Jump | Intent::Sprint | Intent::Levitate | Intent::Rise | Intent::Lower => {
                let glade = self
                    .zone_state
                    .everglade
                    .as_deref_mut()
                    .ok_or("Enter Everglade first")?;
                match intent {
                    Intent::Jump => glade.jump = true,
                    Intent::Sprint => glade.sprinting = !glade.sprinting,
                    Intent::Levitate => glade.toggle_levitate(&self.player),
                    Intent::Rise | Intent::Lower if glade.levitating => {
                        let ground = super::everglade::land(self.player.pos.x, self.player.pos.z);
                        glade.altitude = (glade.altitude
                            + if intent == Intent::Rise { 1.5 } else { -1.5 })
                        .clamp(ground, ground + glade.ceiling());
                    }
                    _ => return Err("Levitate before changing altitude".into()),
                }
                self.cancel_navigation();
                self.zone_state.error = None;
            }
            Intent::Thunderwave
            | Intent::GustOfWind
            | Intent::MistyStep
            | Intent::Web
            | Intent::LongRest
            | Intent::GroveSlot(_) => return Err("Enter the Grove first".into()),
            Intent::FeatherFall
            | Intent::WallOfStone
            | Intent::WindWall
            | Intent::ReverseGravity => {
                let spell =
                    super::everglade::spells::Spell::of(intent).ok_or("Not an Everglade spell")?;
                self.zone_state
                    .everglade
                    .as_deref_mut()
                    .ok_or("Enter Everglade first")?
                    .cast_hotbar_spell(spell, &self.player)?;
                self.zone_state.error = None;
            }
            Intent::Swing | Intent::Rebuild => {
                if !self.casts_destruction() {
                    return Err("Everglade has no offensive spells".into());
                }
                self.zone_state
                    .everglade
                    .as_deref_mut()
                    .ok_or("Enter Everglade first")?
                    .demolish(intent == Intent::Rebuild)?;
                self.zone_state.error = None;
            }
            Intent::MeteorSwarm => {
                if !self.casts_destruction() {
                    return Err("Everglade has no offensive spells".into());
                }
                let player = self.player.clone();
                self.zone_state
                    .everglade
                    .as_deref_mut()
                    .ok_or("Enter Everglade first")?
                    .meteor_swarm(&player)?;
                self.zone_state.error = None;
            }
            Intent::Thunderbolt | Intent::MegaThunderbolt => {
                use super::everglade::demolition::meteor::Strike;
                if !self.casts_destruction() {
                    return Err("Everglade has no offensive spells".into());
                }
                let player = self.player.clone();
                let strike = if intent == Intent::MegaThunderbolt {
                    Strike::MegaLightning
                } else {
                    Strike::Lightning
                };
                self.zone_state
                    .everglade
                    .as_deref_mut()
                    .ok_or("Enter Everglade first")?
                    .target_strike(strike, &player)?;
                self.zone_state.error = None;
            }
            Intent::Interact if self.zone == ZoneId::Crypt => {
                // The heavy door is the way out.
                if !super::crypt::near_door(self.player.pos) {
                    return Err("Walk up to the door".into());
                }
                return self.apply_zone_intent(Intent::Return);
            }
            Intent::Interact => {
                if self.studio_panel_here().is_none() {
                    return Err("Walk up to a station".into());
                }
                self.zone_state.error = None;
            }
        }
        Ok(())
    }
    /// Applies a NIP-MV zone command from an authorized operator to the
    /// loaded simulation and returns what changed.
    ///
    /// Verbs: `fly X,Y,Z` or `fly LANDMARK`, `grab`, `release`, `unclip`,
    /// `clip`, `stop`, `status`, and `parts`. `install` and `wait` are headless-only, since
    /// the desktop simulation runs in real time. Anything else is refused.
    ///
    /// # Errors
    ///
    /// Returns a message when the command names another zone, no zone is
    /// loaded, or the simulation refuses the verb.
    pub fn zone_command(&mut self, command: &crate::mv::Command) -> Result<Value, String> {
        use crate::mv::Arg;
        if command.zone != self.zone.world_id() {
            return Err(format!(
                "zone `{}` is not loaded; this operator is in `{}`",
                command.zone,
                self.zone.world_id()
            ));
        }
        let lagrange = self
            .zone_state
            .lagrange
            .as_deref_mut()
            .ok_or("no simulation zone is loaded")?;
        let station = &mut lagrange.station;
        let result = match command.cmd.as_str() {
            "fly" => {
                let target = match command.args.as_slice() {
                    [Arg::Text(name)] => station
                        .landmark(name)
                        .ok_or_else(|| format!("`{name}` is not a landmark or part"))?,
                    [Arg::Number(x), Arg::Number(y), Arg::Number(z)] => DVec3::new(*x, *y, *z),
                    _ => return Err("fly takes X,Y,Z or a landmark name".into()),
                };
                let target = if station.snapshot().carrying.is_some() {
                    target - (station.hands() - station.astronaut().pos)
                } else {
                    target
                };
                station.apply(Input::FlyTo { target })?;
                json!({ "flying_to": target.to_array(), "from": station.astronaut().pos.to_array() })
            }
            "grab" => {
                let kind = station.apply(Input::Grab)?.ok_or("nothing was grabbed")?;
                json!({ "grabbed": kind.name() })
            }
            "release" => {
                let kind = station
                    .apply(Input::Release)?
                    .ok_or("nothing was released")?;
                let installed = station
                    .parts
                    .iter()
                    .any(|part| part.kind == kind && part.state == PartState::Installed);
                json!({ "released": kind.name(), "installed": installed })
            }
            "stop" => {
                station.apply(Input::Stop)?;
                json!({ "stopped": true })
            }
            "unclip" => {
                station.apply(Input::Unclip)?;
                json!({ "tethered": false })
            }
            "clip" => {
                station.apply(Input::Clip)?;
                json!({ "tethered": true })
            }
            "status" => serde_json::to_value(station.snapshot()).map_err(|e| e.to_string())?,
            "parts" => json!(
                station
                    .parts
                    .iter()
                    .map(|part| json!({
                        "kind": part.kind.name(),
                        "state": format!("{:?}", part.state).to_lowercase(),
                        "pos": station.body(part).pos.to_array(),
                    }))
                    .collect::<Vec<_>>()
            ),
            "install" | "wait" => {
                return Err(format!(
                    "`{}` runs only in the headless simulator; send fly, grab, release, and stop",
                    command.cmd
                ));
            }
            other => return Err(format!("unknown zone command `{other}`")),
        };
        lagrange.tick();
        self.zone_state.error = None;
        Ok(result)
    }
    /// Map status and marker while an EVA pack autopilot owns map taps.
    #[must_use]
    pub fn eva_map_status(&self) -> Option<(&'static str, Option<[f32; 2]>)> {
        let station = &self.zone_state.lagrange.as_deref()?.station;
        Some(match station.target {
            Some(t) => ("Flying", Some([t.x as f32, t.z as f32])),
            None => ("Choose a point to fly to", None),
        })
    }
    /// Fly the EVA pack toward a map point at the current altitude.
    pub(crate) fn lagrange_fly_to(&mut self, destination: [f32; 2]) -> Option<Result<(), String>> {
        let lagrange = self.zone_state.lagrange.as_deref_mut()?;
        let y = lagrange.station.astronaut().pos.y;
        Some(
            lagrange
                .station
                .apply(Input::FlyTo {
                    target: glam::DVec3::new(
                        f64::from(destination[0]),
                        y,
                        f64::from(destination[1]),
                    ),
                })
                .map(|_| ()),
        )
    }
    /// Source slot indices in the zone's displayed hotbar order: displayed
    /// slot `i` is `SLOTS[order[i]]`. The Meteor Stress Test puts Meteor
    /// Swarm first and Levitate sixth; Everglade with dev destruction on
    /// puts Meteor Swarm, the Thunderbolt, the Mega Thunderbolt, then
    /// Levitate first
    /// ([`super::everglade::hotbar::DEV_ORDER`]); elsewhere the bar keeps
    /// [`super::everglade::hotbar::SLOTS`] order. It has one entry per slot
    /// [`Self::everglade_hotbar`] shows.
    #[must_use]
    pub fn everglade_hotbar_order(&self) -> Vec<usize> {
        use super::everglade::hotbar::{COUNT, DEV_ORDER, in_order};
        if self.zone.meteor_stage() {
            vec![5, 1, 2, 3, 4, 0, 8]
        } else if self.everglade_destruction() {
            DEV_ORDER.to_vec()
        } else {
            in_order(COUNT)
        }
    }

    /// The intent of the hotbar slot displayed at `index`, in the zone's
    /// order ([`Self::everglade_hotbar_order`]).
    #[must_use]
    pub fn everglade_slot_intent(&self, index: usize) -> Option<Intent> {
        let order = self.everglade_hotbar_order();
        super::everglade::hotbar::SLOTS
            .get(*order.get(index)?)
            .map(|(intent, ..)| *intent)
    }

    /// The player's breath for the HUD's breath bar in Everglade's town,
    /// while the eye is under water or breath is short.
    #[must_use]
    pub fn everglade_breath(&self) -> Option<super::everglade::water::BreathBar> {
        if self.zone != ZoneId::Everglade {
            return None;
        }
        self.zone_state.everglade.as_deref()?.breath_bar()
    }

    /// Everglade's hotbar of movement and utility spells, in displayed
    /// order ([`Self::everglade_hotbar_order`]), or `None` outside Everglade.
    /// It has [`super::everglade::hotbar::COUNT`] slots, and Meteor Swarm,
    /// the Thunderbolt, and the sledgehammer too only in the Meteor Stress
    /// Test, or while
    /// [`Self::dev_destruction`] is on in Everglade.
    #[must_use]
    pub fn everglade_hotbar(&self) -> Option<Vec<super::everglade::hotbar::Slot>> {
        use super::everglade::{hotbar::Slot, spells::Spell};
        // The crypt is walked with Everglade's bar.
        if !matches!(
            self.zone,
            ZoneId::Everglade | ZoneId::Crypt | ZoneId::MeteorStressTest | ZoneId::MeteorShowcase
        ) {
            return None;
        }
        let glade = self.zone_state.everglade.as_deref()?;
        if glade.demolition().is_some() {
            return None;
        }
        let [feather, wind, reverse, stone] =
            Spell::ALL.map(|spell| glade.spell_slot(spell, &self.player));
        let levitate = Slot {
            enabled: true,
            active: glade.levitating,
            cooldown: 0.0,
        };
        let mut bar = vec![levitate, feather, wind, reverse, stone];
        if !self.everglade_destruction() {
            return Some(bar);
        }
        // Meteor Swarm and the sledgehammer in the Meteor Stress Test, or
        // in Everglade's town with the dev build's destruction on, once the
        // buildings can break: free, without a cooldown.
        let on = |enabled, active| Slot {
            enabled,
            active,
            cooldown: 0.0,
        };
        let (wielding, swarm) = glade.town().map_or((false, None), |town| {
            let (wielding, swarm) = town.bar();
            (wielding, Some(swarm))
        });
        use super::everglade::demolition::meteor::Strike;
        let strike = |kind| {
            swarm.map_or(on(false, false), |s| {
                on(
                    s.ready || s.targeting,
                    s.strike == kind && (s.targeting || s.casting.is_some()),
                )
            })
        };
        bar.extend([
            strike(Strike::Meteors),
            strike(Strike::Lightning),
            strike(Strike::MegaLightning),
            on(glade.town().is_some(), wielding),
        ]);
        Some(
            self.everglade_hotbar_order()
                .into_iter()
                .map(|index| bar[index])
                .collect(),
        )
    }

    /// Calls Meteor Swarm down on the ground ahead of the player in
    /// Everglade's town without the hotbar, for a scripted run such as the
    /// terminal stress run; no input reaches it. Returns whether the cast
    /// began.
    pub fn scripted_meteor_swarm(&mut self) -> bool {
        if self.zone != ZoneId::Everglade {
            return false;
        }
        let player = self.player.clone();
        let Some(glade) = self.zone_state.everglade.as_deref_mut() else {
            return false;
        };
        glade.meteor_swarm(&player).is_ok() && glade.confirm_swarm(&player)
    }

    /// Whether Everglade's hotbar carries Meteor Swarm and the sledgehammer:
    /// only in a `dev-destruction` build, and only after
    /// [`Self::set_dev_destruction`] switched them on.
    #[must_use]
    pub fn dev_destruction(&self) -> bool {
        super::everglade::hotbar::DEV_DESTRUCTION && self.zone_state.dev_destruction
    }

    /// Switches Meteor Swarm and the sledgehammer on Everglade's hotbar on
    /// or off, for a local test of destruction (`verse --dev-destruction`).
    ///
    /// # Errors
    ///
    /// Returns a message when asked to switch them on in a build without
    /// the `dev-destruction` feature, which every web, phone, and release
    /// build is.
    pub fn set_dev_destruction(&mut self, on: bool) -> Result<(), String> {
        if on && !super::everglade::hotbar::DEV_DESTRUCTION {
            return Err(
                "This build has no dev destruction; build Verse with --features dev-destruction"
                    .into(),
            );
        }
        self.zone_state.dev_destruction = on;
        Ok(())
    }

    /// How far Meteor Swarm's blasts shake the camera this frame, in the
    /// demolition yard or Everglade's town.
    #[must_use]
    pub(crate) fn demolition_shake(&self) -> Vec3 {
        if !self.breaks_things() {
            return Vec3::ZERO;
        }
        self.zone_state
            .everglade
            .as_deref()
            .map_or(Vec3::ZERO, Everglade::shake)
    }

    /// Whether Meteor Swarm's circle follows the cursor, in the demolition
    /// yard or Everglade's town.
    #[must_use]
    pub fn demolition_targeting(&self) -> bool {
        self.casts_destruction()
            && self
                .zone_state
                .everglade
                .as_deref()
                .and_then(Everglade::swarm)
                .is_some_and(super::everglade::demolition::meteor::Swarm::targeting)
    }

    /// How much of Everglade's town is in the rules now, for a frame log:
    /// the raised buildings, their pieces, and the chunks alive; `None`
    /// outside the town.
    #[must_use]
    pub fn everglade_wreckage(&self) -> Option<[usize; 3]> {
        if !matches!(
            self.zone,
            ZoneId::Everglade | ZoneId::MeteorStressTest | ZoneId::MeteorShowcase
        ) {
            return None;
        }
        let town = self.zone_state.everglade.as_deref()?.town()?;
        let site = town.site();
        let chunks = site
            .pieces()
            .iter()
            .flat_map(|p| &p.chunks)
            .filter(|c| !c.gone)
            .count();
        Some([town.raised().len(), site.pieces().len(), chunks])
    }

    /// Meteor Swarm's state in Everglade's town, for the overlay that
    /// draws its help and cast bar over the hotbar; `None` elsewhere and
    /// in the demolition yard, whose own hotbar draws them.
    #[must_use]
    pub fn everglade_swarm(&self) -> Option<super::everglade::demolition::meteor::Status> {
        if !self.everglade_destruction() {
            return None;
        }
        let glade = self.zone_state.everglade.as_deref()?;
        Some(glade.town()?.swarm().status())
    }

    /// Puts Meteor Swarm's circle on the ground under the normalized
    /// viewport point `(x, y)` of a view with `aspect`. Returns whether it
    /// moved.
    pub fn demolition_aim(&mut self, aspect: f32, x: f32, y: f32) -> bool {
        if !self.demolition_targeting() {
            return false;
        }
        let Some((origin, direction)) =
            crate::runtime::viewport_ray(&self.view(aspect), aspect, x, y)
        else {
            return false;
        };
        let player = self.player.clone();
        self.zone_state
            .everglade
            .as_deref_mut()
            .is_some_and(|glade| glade.aim_swarm(origin, direction, &player))
    }

    /// Casts Meteor Swarm at its circle. Returns whether the cast began.
    pub fn demolition_confirm(&mut self) -> bool {
        if !self.casts_destruction() {
            return false;
        }
        let player = self.player.clone();
        self.zone_state
            .everglade
            .as_deref_mut()
            .is_some_and(|glade| glade.confirm_swarm(&player))
    }

    /// Leaves Meteor Swarm's targeting or stops its cast, spending
    /// nothing. Returns whether there was either to stop.
    pub fn demolition_cancel(&mut self) -> bool {
        if !self.casts_destruction() {
            return false;
        }
        self.zone_state
            .everglade
            .as_deref_mut()
            .is_some_and(Everglade::cancel_swarm)
    }

    /// Whether this zone has things Meteor Swarm breaks: Everglade's town
    /// or yard, or the Grove's tower.
    fn breaks_things(&self) -> bool {
        matches!(
            self.zone,
            ZoneId::Everglade | ZoneId::Grove | ZoneId::MeteorStressTest | ZoneId::MeteorShowcase
        )
    }

    /// Whether the player may cast Meteor Swarm or swing the sledgehammer
    /// here: in the Grove, the Meteor Stress Test, the demolition yard, and
    /// in Everglade's town
    /// only while [`Self::dev_destruction`] is on. Everglade's buildings
    /// still break; the player just has no offensive spell there.
    fn casts_destruction(&self) -> bool {
        match self.zone {
            ZoneId::Grove | ZoneId::MeteorStressTest | ZoneId::MeteorShowcase => true,
            ZoneId::Everglade => self.in_demolition() || self.dev_destruction(),
            _ => false,
        }
    }

    /// Whether Everglade's hotbar carries Meteor Swarm and the sledgehammer
    /// here: in the Meteor Stress Test, and in Everglade's town only while
    /// [`Self::dev_destruction`] is on.
    fn everglade_destruction(&self) -> bool {
        match self.zone {
            ZoneId::MeteorStressTest | ZoneId::MeteorShowcase => true,
            ZoneId::Everglade => self.dev_destruction(),
            _ => false,
        }
    }

    /// How many tall buildings' tops are toppling now in Everglade's town
    /// or the Grove, and the chunks alive; `None` where nothing breaks.
    #[must_use]
    pub fn toppling(&self) -> Option<(usize, usize)> {
        if !self.breaks_things() {
            return None;
        }
        let site = self.zone_state.everglade.as_deref()?.town()?.site();
        let chunks = site
            .pieces()
            .iter()
            .flat_map(|p| &p.chunks)
            .filter(|c| !c.gone)
            .count();
        Some((site.toppling(), chunks))
    }

    /// Meteor Swarm's or the Thunderbolt's state in the Grove, for the
    /// line of help and the cast bar over its bar; `None` elsewhere.
    #[must_use]
    pub fn grove_swarm(&self) -> Option<super::everglade::demolition::meteor::Status> {
        if self.zone != ZoneId::Grove {
            return None;
        }
        let glade = self.zone_state.everglade.as_deref()?;
        Some(glade.town()?.swarm().status())
    }

    /// The demolition yard's hotbar, or `None` outside the yard.
    #[must_use]
    pub fn demolition_bar(&self) -> Option<super::everglade::demolition::hotbar::Bar> {
        if self.zone != ZoneId::Everglade {
            return None;
        }
        let glade = self.zone_state.everglade.as_deref()?;
        Some(glade.demolition()?.bar())
    }

    /// Presses (`down`) or lets go of the Grove hotbar key that sends
    /// `intent`. A press casts at once, every time, and a held key recasts
    /// six times a second until it is let go. Returns whether the Grove took
    /// the key, with a refused cast's reason.
    ///
    /// # Errors
    ///
    /// Returns why a press's cast was refused, such as no dummy ahead.
    pub fn grove_key(&mut self, intent: Intent, down: bool) -> Result<bool, String> {
        if self.zone != ZoneId::Grove {
            return Ok(false);
        }
        let Some(slot) = self
            .zone_state
            .grove
            .as_deref()
            .and_then(|grove| grove.slot_of(intent))
        else {
            return Ok(false);
        };
        let result = if down {
            self.zone_intent(intent)
        } else {
            Ok(())
        };
        if let Some(grove) = self.zone_state.grove.as_deref_mut() {
            grove.hold(slot, down);
        }
        result.map(|()| true)
    }

    /// How many times its usual distance the camera stands back in the
    /// Grove, to fit the dragon; one elsewhere.
    pub(crate) fn grove_camera(&self) -> f32 {
        self.zone_state
            .grove
            .as_deref()
            .map_or(1.0, super::grove::Grove::camera)
    }

    /// The camera's jolt from the Grove's newest Thunderwave, m.
    pub(crate) fn grove_shake(&self) -> Vec3 {
        self.zone_state
            .grove
            .as_deref()
            .map_or(Vec3::ZERO, super::grove::Grove::shake)
    }

    /// Lets go of every held Grove hotbar key, as when the window loses
    /// focus.
    pub fn grove_release(&mut self) {
        if let Some(grove) = self.zone_state.grove.as_deref_mut() {
            grove.release();
        }
    }

    /// The Grove's combat log: its heading (the land and the form) and its
    /// newest lines, oldest first, or `None` outside the Grove.
    #[must_use]
    pub fn grove_log(&self) -> Option<(String, Vec<String>)> {
        let grove = self.zone_state.grove.as_deref()?;
        Some((grove.status(), grove.log.clone()))
    }

    /// The Grove's hotbar, or `None` outside the Grove.
    #[must_use]
    pub fn grove_bar(&self) -> Option<super::grove::hotbar::Bar> {
        let grove = self.zone_state.grove.as_deref()?;
        let glade = self.zone_state.everglade.as_deref()?;
        Some(grove.bar(&self.player, glade))
    }

    /// Whether the player is in the Grove as a form that flies, such as the
    /// Giant Eagle, whose Space climbs while held.
    #[must_use]
    pub fn grove_form_flies(&self) -> bool {
        self.zone_state
            .grove
            .as_deref()
            .and_then(super::grove::Grove::form)
            .is_some_and(super::grove::shape::Form::flies)
    }

    /// While levitating in Everglade, climbs (`direction` 1) or descends
    /// (-1) for `dt` seconds of a held key, such as X to descend.
    pub fn everglade_climb(&mut self, direction: f32, dt: f32) {
        let (x, z) = (self.player.pos.x, self.player.pos.z);
        // A flying form on the ground, such as the Grove's dragon, takes
        // off as the climb begins.
        let takes_off = direction > 0.0 && self.grove_form_flies();
        if let Some(glade) = self.zone_state.everglade.as_deref_mut()
            && takes_off
            && !glade.levitating
        {
            glade.toggle_levitate(&self.player);
        }
        if let Some(glade) = self.zone_state.everglade.as_deref_mut()
            && glade.levitating
        {
            let ground = super::everglade::land(x, z);
            glade.altitude = (glade.altitude
                + direction.clamp(-1.0, 1.0) * glade.climb_rate() * dt)
                .clamp(ground, ground + glade.ceiling());
            self.cancel_navigation();
        }
    }

    /// Presses (`pressed`) or lets go of Everglade's Levitate slot or key.
    /// Holding it starts levitating if needed and rises while held;
    /// letting go holds the altitude; a quick tap while levitating ends
    /// the levitation, so the player falls.
    ///
    /// # Errors
    ///
    /// Returns a message outside Everglade's open glade.
    pub fn everglade_levitate(&mut self, pressed: bool) -> Result<(), String> {
        let glade = self
            .zone_state
            .everglade
            .as_deref_mut()
            .filter(|glade| glade.demolition().is_none())
            .ok_or("Enter Everglade first")?;
        if pressed {
            glade.press_levitate(&self.player);
            self.cancel_navigation();
        } else {
            glade.release_levitate(&self.player);
        }
        self.zone_state.error = None;
        Ok(())
    }

    /// Whether the player is levitating in Everglade.
    #[must_use]
    pub fn everglade_levitating(&self) -> bool {
        self.zone_state
            .everglade
            .as_deref()
            .is_some_and(|glade| glade.levitating)
    }

    /// Rings the Agora's bell in Everglade (`layout::agora::Bell`): the
    /// hook the sales floor calls when the payment ledger records a
    /// settled deal. Returns whether the zone has the bell.
    pub fn everglade_ring_agora_bell(&mut self) -> bool {
        self.zone_state
            .everglade
            .as_deref_mut()
            .is_some_and(|glade| glade.ring_agora_bell())
    }

    pub fn zone_snapshot(&self, aspect: f32) -> Snapshot {
        let portal = self.zone_portal(aspect);
        let mut controls = Vec::new();
        let mut add = |id, label, action, enabled| {
            controls.push(Control {
                id,
                label: String::from(label),
                action,
                enabled,
            })
        };
        let caption = if self.zone_loading() {
            add("cancel", "Cancel", Intent::Cancel, true);
            format!(
                "Loading {} · {}%",
                self.zone_state.destination.label(),
                (self.zone_state.progress * 100.0) as u32
            )
        } else if self.zone_state.loading == LoadState::Failed {
            add(
                "retry",
                "Retry",
                Intent::Retry,
                portal.near || self.grid_retry_allowed(),
            );
            add("cancel", "Dismiss", Intent::Cancel, true);
            format!("{} could not load", self.zone_state.destination.label())
        } else if let Some(lagrange) = &self.zone_state.lagrange {
            let s = lagrange.station.snapshot();
            if s.carrying.is_some() {
                add(
                    "release",
                    if s.latch_ready { "Latch" } else { "Release" },
                    Intent::Release,
                    true,
                );
            } else {
                add("grab", "Grab", Intent::Grab, s.can_grab);
            }
            add(
                "tether",
                if s.tethered { "Unclip" } else { "Clip" },
                Intent::Tether,
                s.tethered
                    || s.clip_distance_m
                        .is_some_and(|d| d <= verse_lagrange::station::CLIP_RANGE),
            );
            add(
                "forces",
                if lagrange.overlay {
                    "Hide forces"
                } else {
                    "Forces"
                },
                Intent::Forces,
                true,
            );
            add(
                "camera",
                if lagrange.art { "Photo" } else { "Art" },
                Intent::Camera,
                true,
            );
            add("return", self.return_label(), Intent::Return, true);
            let status = if let Some(kind) = s.carrying {
                match s.latch_distance_m {
                    Some(_) if s.latch_ready => format!("{} aligned · latch", kind.name()),
                    Some(d) => format!("{} · {:.0} kg · jig {:.1} m", kind.name(), kind.mass(), d),
                    None => kind.name().to_owned(),
                }
            } else if let Some(message) = &s.message {
                message.clone()
            } else if let Some(d) = s.clip_distance_m {
                format!("Unclipped · tether clip {d:.0} m away")
            } else if let Some(next) = s.next_part {
                format!("Next: {} at the depot", next.name().to_lowercase())
            } else {
                "Keel frame complete".into()
            };
            let status = if s.keeping_active && s.carrying.is_none() {
                format!(
                    "Day {:.0} · keeping burn · {:.2} m/s total",
                    s.orbit.mission_days, s.orbit.keeping_dv_m_s
                )
            } else {
                status
            };
            let mut sensed = format!("{:.2} g · spin {:.0}°/s", s.g_load, s.spin_deg_s);
            if let Some(ahead) = s.proximity_m {
                sensed.push_str(&format!(" · {ahead:.1} m ahead"));
            }
            if lagrange.overlay {
                sensed.push_str(&format!(
                    " · step {:.2} ms · {} awake",
                    s.step_ms, s.awake_bodies
                ));
            }
            if s.impact_n > 1.0 {
                sensed.push_str(&format!(" · impact {:.0} N", s.impact_n));
            }
            format!(
                "Earth {:.2}M km · N2 {:.1} kg · {:.1} m/s\n{sensed}\n{}",
                s.orbit.earth_distance_km / 1.0e6,
                s.propellant_kg,
                s.speed_m_s,
                status
            )
        } else if let Some(lab) = &self.zone_state.lab {
            add("knob_prev", "Prev", Intent::KnobPrev, true);
            add("knob_next", "Next", Intent::KnobNext, true);
            add("decrease", "-", Intent::Decrease, true);
            add("increase", "+", Intent::Increase, true);
            add("reset", "Reset", Intent::Reset, true);
            add(
                "pause",
                if lab.paused { "Run" } else { "Pause" },
                Intent::Pause,
                true,
            );
            add("step", "Step", Intent::Step, true);
            add("return", "Plaza", Intent::Return, true);
            Lab::caption(&lab.snapshot())
        } else if self.zone == ZoneId::MeteorStressTest {
            add("meteor_swarm", "Meteor Swarm", Intent::MeteorSwarm, true);
            add("rebuild", "Rebuild castle", Intent::Rebuild, true);
            add("levitate", "Levitate", Intent::Levitate, true);
            add("return", self.return_label(), Intent::Return, true);
            let counts = self
                .zone_state
                .everglade
                .as_deref()
                .and_then(Everglade::town)
                .map_or([0, 0], |town| town.bombardment());
            format!(
                "Meteor Stress Test · {} casters · {} meteors in flight · 1: cast · R: rebuild · automatic rebuild every 3 min",
                counts[0], counts[1]
            )
        } else if self.zone == ZoneId::MeteorShowcase {
            add("meteor_swarm", "Meteor Swarm", Intent::MeteorSwarm, true);
            add("rebuild", "Rebuild houses", Intent::Rebuild, true);
            add("levitate", "Levitate", Intent::Levitate, true);
            add("return", self.return_label(), Intent::Return, true);
            let flying = self
                .zone_state
                .everglade
                .as_deref()
                .and_then(Everglade::town)
                .map_or(0, |town| town.bombardment()[1]);
            format!(
                "Meteor Showcase · {flying} meteors in flight · 1: cast eight · R: rebuild · the caster casts again after each rebuild"
            )
        } else if let Some(coast) = &self.zone_state.coast {
            add("jump", "Jump", Intent::Jump, !self.player.airborne());
            add("return", self.return_label(), Intent::Return, true);
            format!(
                "Coast · {} · tide {:+.2} m",
                coast.medium.name(),
                super::coast::water::tide(coast.tick)
            )
        } else if let Some(lab) = &self.zone_state.water {
            add("jump", "Jump", Intent::Jump, !self.player.airborne());
            add("return", self.return_label(), Intent::Return, true);
            lab.caption(self.player.pos)
        } else if self.zone_state.crypt.is_some() {
            add("jump", "Jump", Intent::Jump, !self.player.airborne());
            add("return", self.return_label(), Intent::Return, true);
            let door = if !super::crypt::near_door(self.player.pos) {
                "The heavy door by the entrance leads out"
            } else if self.interact_hint == crate::runtime::InteractHint::Tap {
                "Tap Plaza to open the door and leave"
            } else {
                "F opens the door and leaves"
            };
            format!("Crypt\n{door}")
        } else if let Some(grove) = &self.zone_state.grove {
            add("jump", "Jump", Intent::Jump, !self.player.airborne());
            add("long_rest", "Long Rest", Intent::LongRest, true);
            add("return", self.return_label(), Intent::Return, true);
            grove.caption(&self.player)
        } else if let Some(yard) = self
            .zone_state
            .everglade
            .as_deref()
            .and_then(Everglade::demolition)
        {
            // The yard's hotbar ([`Self::demolition_bar`]) draws these;
            // Space jumps.
            add("swing", "Swing", Intent::Swing, true);
            add(
                "meteor_swarm",
                "Meteor Swarm",
                Intent::MeteorSwarm,
                yard.swarm().status().ready,
            );
            add("rebuild", "Rebuild", Intent::Rebuild, true);
            yard.caption()
        } else if let Some(glade) = &self.zone_state.everglade {
            add("jump", "Jump", Intent::Jump, !self.player.airborne());
            add(
                "sprint",
                if glade.sprinting { "Run" } else { "Sprint" },
                Intent::Sprint,
                true,
            );
            add(
                "levitate",
                if glade.levitating { "Land" } else { "Levitate" },
                Intent::Levitate,
                true,
            );
            add("return", self.return_label(), Intent::Return, true);
            if self.interact_hint != crate::runtime::InteractHint::None
                && let Some(panel) = self.studio_panel_here()
            {
                let label = crate::zones::everglade::button_label(&panel);
                add("interact", label, Intent::Interact, true);
            }
            let mut caption = Everglade::caption(self.player.pos, self.interact_hint);
            let agent = crate::zones::everglade::studio::WORKSHOP_AGENT;
            if self.interact_hint == crate::runtime::InteractHint::Key
                && let Some(at) = self.zone_state.studio.seat_position(agent)
                && (at.x - self.player.pos.x).hypot(at.z - self.player.pos.z)
                    <= crate::zones::everglade::studio::TALK_REACH
            {
                caption = if self.zone_state.workshop_owner {
                    format!("Everglade\nF talks to {agent}, your workshop agent")
                } else {
                    format!(
                        "Everglade\n{agent} is the owner's workshop agent; she answers only her owner"
                    )
                };
            }
            if self.interact_hint == crate::runtime::InteractHint::Key
                && self.studio_panel_here().is_none()
                && let Some((_, name)) = self.villager_in_reach()
            {
                caption = format!("Everglade\nF talks to {name}, a character");
            }
            if self.zone == ZoneId::Everglade
                && let Some(text) = self
                    .zone_state
                    .compute
                    .inspect([self.player.pos.x, self.player.pos.z])
            {
                caption = text;
            }
            match &self.zone_state.studio_notice {
                Some(notice) if caption.is_empty() => notice.clone(),
                Some(notice) => format!("{notice} · {caption}"),
                None => caption,
            }
        } else if portal.near && portal.visible {
            let nearest = self.nearest_portal().map(|(zone, _)| zone);
            if nearest == Some(ZoneId::Lagrange1) {
                add("enter", "Enter L1", Intent::Enter, true);
                "Lagrange 1 · Sun–Earth L1 station".into()
            } else if nearest == Some(ZoneId::PhysicsLab) {
                add("enter", "Enter Lab", Intent::Enter, true);
                "Physics Lab · live rigid-body sandbox".into()
            } else if nearest == Some(ZoneId::Coast) {
                add(
                    "enter",
                    "Enter Coast",
                    Intent::Enter,
                    self.zone_state.everglade_loader.is_some(),
                );
                "Coast · a tidal bay and open sea".into()
            } else if nearest == Some(ZoneId::WaterLab) {
                add(
                    "enter",
                    "Enter Water Lab",
                    Intent::Enter,
                    self.zone_state.everglade_loader.is_some(),
                );
                "Water Lab · a cove of waves, falls, and floating things".into()
            } else if nearest == Some(ZoneId::Crypt) {
                add(
                    "enter",
                    "Enter Crypt",
                    Intent::Enter,
                    self.zone_state.everglade_loader.is_some(),
                );
                "Crypt · a candlelit laboratory".into()
            } else {
                add(
                    "enter",
                    "Enter Everglade",
                    Intent::Enter,
                    self.zone_state.everglade_loader.is_some(),
                );
                "Everglade · the Agent Studio's forest glade".into()
            }
        } else {
            String::new()
        };
        Snapshot {
            id: self.zone,
            label: self.zone_label(),
            state: self.zone_state.loading,
            progress: self.zone_state.progress,
            error: self.zone_state.error.clone(),
            portal,
            controls,
            station: self
                .zone_state
                .lagrange
                .as_deref()
                .map(|l| l.station.snapshot()),
            lab: self.zone_state.lab.as_deref().map(Lab::snapshot),
            caption,
        }
    }
    fn zone_portal(&self, aspect: f32) -> PortalProjection {
        let Some((_, at)) = self.nearest_portal() else {
            // No arch here to approach or tap.
            return PortalProjection {
                near: false,
                visible: false,
                screen_x: 0.5,
                screen_y: 0.5,
                distance: f32::MAX,
            };
        };
        let offset = self.player.pos - at;
        let distance = offset.x.hypot(offset.z);
        let anchor = at + Vec3::new(0.0, 2.5, -0.3);
        let view = self.view(aspect);
        let clip = view.view_proj * anchor.extend(1.0);
        let mut p = PortalProjection {
            // The Grid's own portal is walked through, not tapped.
            near: !(self.is_bare() && self.is_plaza()) && self.portal_in_reach(at),
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance,
        };
        if aspect.is_finite() && aspect > 0.0 && clip.is_finite() && clip.w > 0.0 {
            let ndc = clip.truncate() / clip.w;
            p.screen_x = (ndc.x * 0.5 + 0.5).clamp(0.0, 1.0);
            p.screen_y = (0.5 - ndc.y * 0.5).clamp(0.0, 1.0);
            p.visible = (-1.0..=1.0).contains(&ndc.x)
                && (-1.0..=1.0).contains(&ndc.y)
                && (0.0..=1.0).contains(&ndc.z);
        }
        if p.visible && p.near {
            let delta = anchor - view.eye;
            let length = delta.length();
            let direction = delta.normalize_or_zero();
            p.visible =
                !crate::runtime::mesh_occludes(&self.world.mesh, view.eye, direction, length)
                    && !self.zone_dynamic_occludes(view.eye, direction, length);
        }
        p
    }
    /// Pick a nearby visible portal opening; an occluding surface blocks entry.
    pub fn zone_hit(&self, aspect: f32, x: f32, y: f32) -> bool {
        self.zone_hit_with_entities(aspect, x, y, &crate::mesh::Mesh::default())
    }
    /// Include the currently presented remote entities when admitting a tap.
    pub fn zone_hit_with_entities(
        &self,
        aspect: f32,
        x: f32,
        y: f32,
        entities: &crate::mesh::Mesh,
    ) -> bool {
        let Some((_, portal)) = self.nearest_portal() else {
            return false;
        };
        // The Grid's portal is walked through; nothing on the Grid is tapped.
        if self.is_bare() && self.is_plaza() || !self.portal_in_reach(portal) || self.zone_loading()
        {
            return false;
        }
        let view = self.view(aspect);
        let Some((eye, direction)) = crate::runtime::viewport_ray(&view, aspect, x, y) else {
            return false;
        };
        let at = portal + Vec3::new(0.0, 2.25, -0.3);
        if direction.z.abs() < 0.0001 {
            return false;
        }
        let distance = (at.z - eye.z) / direction.z;
        let hit = eye + direction * distance;
        let clip = view.view_proj * hit.extend(1.0);
        distance > 0.0
            && (hit.x - at.x).abs() <= 1.75
            && (hit.y - at.y).abs() <= 2.1
            && clip.w > 0.0
            && (0.0..=1.0).contains(&(clip.z / clip.w))
            && !crate::runtime::mesh_occludes(&self.world.mesh, eye, direction, distance)
            && !self.zone_dynamic_occludes(eye, direction, distance)
            && !crate::runtime::mesh_occludes(entities, eye, direction, distance)
    }
    fn zone_dynamic_occludes(&self, eye: Vec3, direction: Vec3, distance: f32) -> bool {
        if let Some(lagrange) = &self.zone_state.lagrange {
            crate::runtime::mesh_occludes(lagrange.dynamic(), eye, direction, distance)
        } else if let Some(lab) = &self.zone_state.lab {
            crate::runtime::mesh_occludes(lab.dynamic(), eye, direction, distance)
        } else if let Some(everglade) = &self.zone_state.everglade {
            crate::runtime::mesh_occludes(everglade.dynamic(), eye, direction, distance)
        } else {
            crate::runtime::mesh_occludes(&self.dynamic_mesh(), eye, direction, distance)
        }
    }
    /// The Grid's walk-in portal to Lagrange 1 in the bare world; none
    /// elsewhere, and none while the portal is hidden
    /// ([`super::gate::GRID_PORTAL_OPEN`]).
    #[must_use]
    pub fn grid_gate(&self) -> Option<super::Gate> {
        (self.is_bare() && self.zone_state.grid_portal)
            .then(|| super::Gate::grid(&crate::blocks::Layout::grid()))
    }

    /// The Grid's walk-in portal to Everglade in the bare world
    /// ([`super::gate::GRID_EVERGLADE_OPEN`]). A world without zone storage
    /// cannot load Everglade's pack, so it has no such portal.
    #[must_use]
    pub fn everglade_gate(&self) -> Option<super::Gate> {
        (self.is_bare()
            && super::gate::GRID_EVERGLADE_OPEN
            && self.zone_state.everglade_loader.is_some())
        .then(|| super::Gate::everglade(&crate::blocks::Layout::grid()))
    }

    /// The Grid's RITUAL arch to the pinned chamber, when the desktop has one
    /// ([`Self::set_ritual`]); none elsewhere.
    #[must_use]
    pub fn ritual_gate(&self) -> Option<super::Gate> {
        (self.is_bare() && self.zone_state.ritual.is_some())
            .then(|| super::Gate::ritual(&crate::blocks::Layout::grid()))
    }

    /// Pins the chamber configuration the Grid's RITUAL arch joins, or
    /// removes the arch.
    pub fn set_ritual(&mut self, config: Option<std::path::PathBuf>) {
        self.zone_state.ritual = config;
        self.zone_state.ritual_crossed = false;
    }

    /// The pinned chamber configuration, once per walk through the RITUAL
    /// arch. The application joins the chamber and, on return, calls
    /// [`Self::return_from_ritual`].
    pub fn take_ritual_crossing(&mut self) -> Option<std::path::PathBuf> {
        std::mem::take(&mut self.zone_state.ritual_crossed)
            .then(|| self.zone_state.ritual.clone())
            .flatten()
    }

    /// Puts the player back in front of the RITUAL arch, facing away from it.
    ///
    /// # Errors
    /// The Grid has no RITUAL arch, or the player cannot be placed.
    pub fn return_from_ritual(&mut self) -> Result<(), String> {
        let gate = self.ritual_gate().ok_or("The Grid has no RITUAL arch")?;
        let (pos, yaw) = gate.front();
        self.zone_state.gate_cooldown = super::gate::COOLDOWN;
        self.place_player(pos, yaw)
    }

    /// The Grid's shown walk-in portals with their destinations.
    fn grid_gates(&self) -> Vec<(ZoneId, super::Gate)> {
        [
            (ZoneId::Lagrange1, self.grid_gate()),
            (ZoneId::Everglade, self.everglade_gate()),
        ]
        .into_iter()
        .filter_map(|(zone, gate)| gate.map(|gate| (zone, gate)))
        .collect()
    }

    /// Whether the zone panel's Retry may restart a load the player started
    /// by walking through a Grid portal, which has no button to approach.
    fn grid_retry_allowed(&self) -> bool {
        self.is_plaza()
            && self.zone_state.loading == LoadState::Failed
            && self
                .grid_gates()
                .iter()
                .any(|&(zone, _)| zone == self.zone_state.destination)
    }

    /// Where the zone loader stands: idle, loading, or failed.
    #[must_use]
    pub fn zone_load_state(&self) -> LoadState {
        self.zone_state.loading
    }

    /// Starts loading Everglade's pack for `destination`. The
    /// world stays where it is until [`Self::zone_tick`] installs the zone.
    fn start_zone_load(&mut self, destination: ZoneId) -> Result<(), String> {
        if destination == ZoneId::Crypt && !super::crypt::EMBEDDED {
            return Err("This build does not carry the crypt".into());
        }
        // The Grove and the crypt walk with Everglade's character.
        if !matches!(
            destination,
            ZoneId::Everglade
                | ZoneId::Grove
                | ZoneId::Crypt
                | ZoneId::MeteorStressTest
                | ZoneId::MeteorShowcase
                | ZoneId::WaterLab
                | ZoneId::Coast
        ) {
            return Err("This zone has no pack to load".into());
        }
        let requested = self
            .zone_state
            .everglade_loader
            .as_deref_mut()
            .ok_or("Zone storage is unavailable")?
            .request();
        if !requested {
            return Err("Finishing the previous load; try again".into());
        }
        self.cancel_navigation();
        self.doors.cancel_transient();
        self.zone_state.loading = LoadState::Loading;
        self.zone_state.progress = 0.0;
        self.zone_state.error = None;
        Ok(())
    }

    /// Show the Grid's portal on this world regardless of
    /// [`super::gate::GRID_PORTAL_OPEN`], so tests keep the hidden portal's
    /// path working until it is restored. No app build calls this.
    #[doc(hidden)]
    pub fn open_grid_portal_for_tests(&mut self) {
        self.zone_state.grid_portal = true;
    }

    /// The label of the control that leaves a zone.
    fn return_label(&self) -> &'static str {
        if self.is_bare() { GRID_LABEL } else { "Plaza" }
    }

    /// The display name of where the player is. The bare world's grid is
    /// "The Grid"; Coder's plaza and every zone keep their own names.
    #[must_use]
    pub fn zone_label(&self) -> &'static str {
        if self.is_bare() && self.is_plaza() {
            GRID_LABEL
        } else if self.in_demolition() {
            "Demolition yard"
        } else {
            self.zone.label()
        }
    }

    /// The walk-in arches in the neutral palette: the Grid's shown portals,
    /// or a zone's return arch lettered for the Grid.
    pub(crate) fn grid_portal_mesh(&self) -> crate::mesh::Mesh {
        let elapsed = self.zone_state.elapsed;
        let mut mesh = crate::mesh::Mesh::default();
        if self.is_plaza() {
            for (zone, gate) in self.grid_gates() {
                mesh.extend(&gate.mesh(ZoneId::Plaza, zone.sign(), elapsed));
            }
            if let Some(gate) = self.ritual_gate() {
                mesh.extend(&gate.mesh(ZoneId::Plaza, super::gate::RITUAL_SIGN, elapsed));
            }
            return mesh;
        }
        // The crypt's door and the Water Lab's lantern are their ways out;
        // they have no arch.
        if matches!(self.zone, ZoneId::Crypt | ZoneId::WaterLab) {
            return mesh;
        }
        for (_, at) in self.zone.portals() {
            mesh.extend(&super::Gate::fixed(at).mesh(self.zone, "THE GRID", elapsed));
        }
        mesh
    }

    /// On the Grid, walking through a portal enters its zone: Lagrange 1 at
    /// once, Everglade once its pack loads. In the zone, walking (or flying)
    /// through the return arch comes back. Feet moved from `from` to the
    /// player's position this frame. Coder's plaza keeps its tapped arches
    /// and buttons.
    pub(crate) fn walk_through_portals(&mut self, from: Vec3, dt: f32) {
        if !self.is_bare() {
            return;
        }
        self.zone_state.gate_cooldown = (self.zone_state.gate_cooldown - dt).max(0.0);
        if self.zone_state.gate_cooldown > 0.0 || self.zone_loading() {
            return;
        }
        let to = self.player.pos;
        if self.is_plaza() {
            if self
                .ritual_gate()
                .is_some_and(|gate| gate.crossed(from, to))
            {
                self.cancel_navigation();
                self.doors.cancel_transient();
                self.zone_state.ritual_crossed = true;
                self.zone_state.gate_cooldown = super::gate::COOLDOWN;
                return;
            }
            let Some((zone, _)) = self
                .grid_gates()
                .into_iter()
                .find(|(_, gate)| gate.crossed(from, to))
            else {
                return;
            };
            self.cancel_navigation();
            self.doors.cancel_transient();
            self.zone_state.destination = zone;
            self.zone_state.gate_cooldown = super::gate::COOLDOWN;
            if zone == ZoneId::Lagrange1 {
                self.install_lagrange();
            } else if let Err(error) = self.start_zone_load(zone) {
                // The zone panel offers Retry and Dismiss.
                self.zone_load_failed(&error);
            }
        } else if self
            .zone
            .portals()
            .iter()
            .any(|&(_, at)| super::Gate::fixed(at).crossed(from, to))
        {
            let _ = self.zone_intent(Intent::Return);
        }
    }

    /// Start loading Everglade from the plaza without walking to its
    /// portal, as `verse --everglade` asks at launch. [`Self::zone_tick`]
    /// installs it once its pack loads, as after a portal entry.
    ///
    /// # Errors
    /// The player is not in the plaza, a load is under way, or the pack
    /// cannot be requested.
    /// Whether Everglade's loader could start a load now, so a caller can
    /// wait out a canceled load instead of failing on it.
    pub fn everglade_loader_idle(&mut self) -> bool {
        self.zone_state
            .everglade_loader
            .as_deref_mut()
            .is_some_and(everglade_pack::Loader::idle)
    }

    /// Start loading the Grove from the plaza, as `verse --grove` asks at
    /// launch; it loads Everglade's pack. [`Self::zone_tick`] installs it.
    ///
    /// # Errors
    /// The player is not in the plaza, a load is under way, or the pack
    /// cannot be requested.
    pub fn enter_grove(&mut self) -> Result<(), String> {
        if !self.is_plaza() || self.zone_loading() {
            return Err("The Grove enters only from the plaza".into());
        }
        self.zone_state.destination = ZoneId::Grove;
        let result = self.start_zone_load(ZoneId::Grove);
        if let Err(error) = &result {
            self.zone_state.error = Some(error.chars().take(180).collect());
        }
        result
    }

    /// Start loading the crypt lab from the plaza, as `verse --crypt` asks
    /// at launch; it loads Everglade's pack for the character.
    /// [`Self::zone_tick`] installs it.
    ///
    /// # Errors
    /// The player is not in the plaza, a load is under way, or the pack
    /// cannot be requested.
    pub fn enter_crypt(&mut self) -> Result<(), String> {
        if !self.is_plaza() || self.zone_loading() {
            return Err("The crypt enters only from the plaza".into());
        }
        self.zone_state.destination = ZoneId::Crypt;
        let result = self.start_zone_load(ZoneId::Crypt);
        if let Err(error) = &result {
            self.zone_state.error = Some(error.chars().take(180).collect());
        }
        result
    }

    /// Opens Everglade as the demolition yard from now on (`verse
    /// --demolition`): two kit cottages to knock down in place of its
    /// layout and the studio. Takes effect at the next entry.
    pub fn set_demolition(&mut self, on: bool) {
        self.zone_state.demolition = on;
    }

    /// Sets the town clock Everglade's sky follows, such as one with its
    /// hour pinned for a capture. It applies to Everglade now and to every
    /// later entry.
    pub fn set_town_clock(&mut self, clock: town_clock::Clock) {
        self.zone_state.town_clock = clock;
        if let Some(everglade) = self.zone_state.everglade.as_deref_mut() {
            everglade.set_clock(clock);
        }
    }

    /// Everglade's air under its town clock, while the player is in
    /// Everglade itself rather than a zone built on it.
    pub(crate) fn everglade_atmosphere(&self) -> Option<crate::zones::Atmosphere> {
        matches!(self.zone, ZoneId::Everglade | ZoneId::MeteorShowcase)
            .then_some(self.zone_state.everglade.as_deref())
            .flatten()
            .map(Everglade::atmosphere)
    }

    /// Everglade's town time at its last tick, while the player is in
    /// Everglade.
    #[must_use]
    pub fn town_time(&self) -> Option<town_clock::TownTime> {
        (self.zone == ZoneId::Everglade)
            .then_some(self.zone_state.everglade.as_deref())
            .flatten()
            .map(Everglade::town_time)
    }

    /// Whether the player is in the demolition yard.
    #[must_use]
    pub fn in_demolition(&self) -> bool {
        self.zone_state
            .everglade
            .as_deref()
            .is_some_and(|glade| glade.demolition().is_some())
    }

    pub fn enter_everglade(&mut self) -> Result<(), String> {
        if !self.is_plaza() || self.zone_loading() {
            return Err("Everglade enters only from the plaza".into());
        }
        self.zone_state.destination = ZoneId::Everglade;
        let result = self.start_zone_load(ZoneId::Everglade);
        if let Err(error) = &result {
            self.zone_state.error = Some(error.chars().take(180).collect());
        }
        result
    }

    /// Lead Everglade's caption with `notice`, or with nothing for `None`.
    /// A blank notice counts as none, and a long one is cut to 180
    /// characters.
    /// Says whether this window's own host answers for the workshop agent:
    /// only then does her caption offer F, and does F open her panel.
    pub fn set_workshop_owner(&mut self, owner: bool) {
        self.zone_state.workshop_owner = owner;
    }

    pub fn set_studio_notice(&mut self, notice: Option<String>) {
        self.zone_state.studio_notice = notice
            .map(|text| text.trim().chars().take(180).collect::<String>())
            .filter(|text| !text.is_empty());
    }

    /// Joins a hosted Everglade instance whose viewer's NIP-HOST grant holds
    /// `rights`, or leaves it with `None`. The `world` right admits walking
    /// only; the studio's panels need its studio rights ([`Studio::access`]).
    pub fn set_studio_grant(&mut self, rights: Option<Vec<coder_access::Right>>) {
        self.zone_state.studio.set_grant(rights);
    }

    /// Draws the studio's seats where a hosted instance's authority places
    /// them, or walks them here again with `None`.
    pub fn follow_studio_authority(
        &mut self,
        poses: Option<Vec<crate::zones::everglade::studio::SeatPose>>,
    ) {
        self.zone_state.studio.follow_authority(poses);
    }

    /// Where Everglade's Agent Studio comes from. Nothing is read until the
    /// player is in Everglade and [`Self::update_studio`] is called active.
    pub fn set_studio_source(&mut self, source: Box<dyn Source>) {
        self.zone_state.studio.set_source(source);
        // A world that opened no panel (the OpenAgents app's bare world)
        // opens a station's panel by tap once a studio is connected.
        if self.interact_hint == crate::runtime::InteractHint::None {
            self.interact_hint = crate::runtime::InteractHint::Tap;
        }
    }

    /// Stops local observation before entering an authoritative social world.
    #[cfg(feature = "hosted-social")]
    pub(crate) fn stop_local_studio(&mut self) {
        self.zone_state.studio.set_active(false);
        self.zone_state.sales_floor.poll(false, 0.0);
    }

    /// Polls the Studio source only while the local Everglade surface is active.
    pub fn update_studio(&mut self, surface_active: bool, dt: f32) {
        if self.is_hosted() {
            self.zone_state.sales_floor.poll(false, 0.0);
            return;
        }
        let active = surface_active
            && self.zone == ZoneId::Everglade
            && self.zone_state.everglade.is_some()
            && !self.zone_state.demolition;
        let studio = &mut self.zone_state.studio;
        studio.set_active(active);
        studio.poll(dt, &self.world.blockers);
        self.zone_state.sales_floor.poll(active, dt);
        if self.zone_state.sales_floor.take_rings() > 0 {
            self.everglade_ring_agora_bell();
        }
    }

    /// Seats this computer draws in the studio beside the host's, such as
    /// the workshop agent ([`Studio::set_resident`]).
    pub fn set_studio_resident(&mut self, seats: Vec<coder_access::studio::Seat>) {
        self.zone_state.studio.set_resident(seats);
    }

    /// Sets the workshop agent's day plan for the plan board in the great
    /// room ([`Studio::set_plan`]).
    pub fn set_studio_plan(&mut self, plan: Option<coder_access::day_plan::DayPlan>) {
        self.zone_state.studio.set_plan(plan);
    }

    /// The Agent Studio as Everglade draws it.
    #[must_use]
    pub fn studio(&self) -> &Studio {
        &self.zone_state.studio
    }

    /// Feeds Everglade's Pylon Field from `source`, or from nothing, which
    /// leaves it dormant (`everglade::compute`).
    pub fn set_compute_source(
        &mut self,
        source: Option<Box<dyn crate::zones::everglade::compute::ComputeSource>>,
    ) {
        self.zone_state.compute.set_source(source);
    }

    /// Configures a private sales reader independently of Studio observation.
    pub fn set_sales_source(
        &mut self,
        source: Option<Box<dyn crate::zones::everglade::sales_floor::Source>>,
    ) {
        self.zone_state.sales_floor.set_source(source);
    }

    /// Current observations exist only on the active local Everglade surface.
    pub fn private_sales_snapshot(
        &self,
    ) -> Option<&crate::zones::everglade::sales_floor::Snapshot> {
        if self.is_hosted() || self.zone != ZoneId::Everglade || self.zone_state.demolition {
            return None;
        }
        self.zone_state.sales_floor.snapshot()
    }

    /// Everglade's Pylon Field.
    pub fn compute(&self) -> &crate::zones::everglade::compute::Compute {
        &self.zone_state.compute
    }

    /// The studio's signals since the last call: new decisions, finished
    /// tasks, and finished goals ([`Studio::take_events`]).
    pub fn take_studio_events(&mut self) -> Vec<crate::zones::everglade::signals::Event> {
        self.zone_state.studio.take_events()
    }

    /// The review the studio's source holds of `task`, while in Everglade.
    pub fn studio_review(&mut self, task: &str) -> Option<coder_access::review::TaskReview> {
        self.zone_state.studio.review(task)
    }

    /// Sends a studio intent through the studio's source, while in
    /// Everglade ([`Studio::send`]).
    ///
    /// # Errors
    /// The studio's refusal before anything is sent.
    pub fn studio_send(
        &mut self,
        operation: coder_access::Operation,
    ) -> Result<u64, coder_access::Error> {
        if self.is_hosted() {
            return Err(coder_access::Error::new(
                coder_access::Code::Forbidden,
                "World admission grants no Studio execution authority",
            ));
        }
        self.zone_state.studio.send(operation)
    }

    /// Turns the studio's sounds on or off for the session (`/sound`).
    pub fn studio_sounds(&mut self, on: bool) {
        self.zone_state.studio.set_sounds(on);
    }

    /// The villager in talking reach in Everglade, by ID and name.
    #[must_use]
    pub fn villager_in_reach(&self) -> Option<(String, String)> {
        if self.zone_state.everglade.is_none()
            || self.zone != ZoneId::Everglade
            || self.zone_state.demolition
            || self.zone_loading()
        {
            return None;
        }
        self.zone_state
            .townsfolk
            .in_reach(self.player.pos)
            .map(|p| (p.id.clone(), p.name.clone()))
    }

    /// Everglade's zone, while the player is in the town, for captures and
    /// tools that act on its water.
    pub fn everglade_zone_mut(&mut self) -> Option<&mut crate::zones::everglade::Everglade> {
        if self.zone != ZoneId::Everglade || self.zone_state.demolition {
            return None;
        }
        self.zone_state.everglade.as_deref_mut()
    }

    /// Everglade's rowboats, while the player is in the town.
    pub fn everglade_afloat_mut(&mut self) -> Option<&mut crate::zones::everglade::boats::Afloat> {
        if self.zone != ZoneId::Everglade || self.zone_state.demolition {
            return None;
        }
        self.zone_state.everglade.as_deref_mut()?.afloat_mut()
    }

    /// The interact key by one of Everglade's rowboats: boards it, leaves
    /// it, or rights it capsized. Returns what happened, or `None` with no
    /// boat in reach.
    pub fn boat_interact(&mut self) -> Option<String> {
        if self.zone != ZoneId::Everglade || self.zone_state.demolition || self.zone_loading() {
            return None;
        }
        let glade = self.zone_state.everglade.as_deref_mut()?;
        glade.interact_boat(&mut self.player)
    }

    /// Shows `text` in a bubble over villager `id`.
    pub fn villager_say(&mut self, id: &str, text: &str) {
        self.zone_state.townsfolk.say(id, text);
    }

    /// The panel the interact key opens where the player stands, in
    /// Everglade.
    #[must_use]
    pub fn studio_panel_here(&self) -> Option<PanelKind> {
        // In a hosted instance the `world` right admits walking only; a
        // panel opens under the grant's `observe`.
        if self.zone_state.everglade.is_none()
            || self.zone != ZoneId::Everglade
            || self.zone_state.demolition
            || self.zone_loading()
            || !self.zone_state.studio.access().read
        {
            return None;
        }
        Studio::panel_at(self.player.pos)
    }

    /// The panel a click at `(x, y)`, as fractions of the viewport from its
    /// top-left, selects in Everglade: a seat opens its panel, a desk's
    /// monitor its seat's, a Task Wall card its task's details, and the
    /// Task Wall, the podium, the merge station, and the library theirs.
    /// The nearest target within a small screen radius wins.
    #[must_use]
    pub fn studio_pick(&self, aspect: f32, x: f32, y: f32) -> Option<PanelKind> {
        /// Screen radius a target takes clicks in, as a fraction of the
        /// viewport's height.
        const RADIUS: f32 = 0.06;
        /// Targets farther from the camera than this are not selected, m.
        const REACH: f32 = 40.0;
        if self.zone_state.everglade.is_none()
            || self.zone != ZoneId::Everglade
            || self.zone_state.demolition
            || self.zone_loading()
            || !self.zone_state.studio.access().read
            || !aspect.is_finite()
            || aspect <= 0.0
        {
            return None;
        }
        let view = self.view(aspect);
        let mut targets: Vec<(Vec3, PanelKind)> = self
            .zone_state
            .studio
            .seats()
            .map(|(name, at)| (at + Vec3::Y * 1.2, PanelKind::Seat(name.to_owned())))
            .collect();
        for (i, desk) in super::everglade::layout::DESKS.iter().enumerate() {
            targets.push((desk.monitor.center, PanelKind::Desk(i as u32)));
        }
        targets.push((
            super::everglade::layout::TASK_WALL.center,
            PanelKind::Console,
        ));
        // A card opens its task's details; nearer the click than the
        // wall's middle, it wins over the console.
        targets.extend(
            self.zone_state
                .studio
                .cards()
                .into_iter()
                .map(|(at, task)| (at, PanelKind::Task(task))),
        );
        for station in &super::everglade::STATIONS {
            if let Some(panel) = PanelKind::at_station(station.id)
                && panel != PanelKind::Console
            {
                targets.push((station.position() + Vec3::Y, panel));
            }
        }
        targets
            .into_iter()
            .filter_map(|(at, panel)| {
                if at.distance(view.eye) > REACH {
                    return None;
                }
                let clip = view.view_proj * at.extend(1.0);
                if clip.w <= 0.0 {
                    return None;
                }
                let sx = (clip.x / clip.w + 1.0) / 2.0;
                let sy = (1.0 - clip.y / clip.w) / 2.0;
                let d = ((sx - x) * aspect).hypot(sy - y);
                (d <= RADIUS).then_some((d, panel))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, panel)| panel)
    }

    pub(crate) fn zone_simulation_tick(&mut self, dt: f32, _previous: PlayerController) {
        self.zone_state.elapsed = (self.zone_state.elapsed + dt) % 1000.0;
        if let Some(lagrange) = &mut self.zone_state.lagrange {
            lagrange.tick();
        }
        if let Some(lab) = &mut self.zone_state.lab {
            lab.tick(dt);
        }
        let state = &mut self.zone_state;
        if let (Some(glade), Some(coast)) = (&mut state.everglade, &mut state.coast) {
            coast.tick = unix_water_tick();
            glade.tick(dt, &self.player, &[]);
        } else if let (Some(glade), Some(lab)) = (&mut state.everglade, &mut state.water) {
            glade.tick(dt, &self.player, &[]);
            // A Fireball's ember gathers in the character's hand.
            lab.hand = glade.hand();
            // The lab says what the character stands on: water under Water
            // Walk, the swimming level, or ice; and how fast it goes.
            let feet = lab.tick(dt, self.player.pos, self.player.forward());
            self.player.set_pace(feet.pace);
            lab.floor = feet.floor;
            // The next step stands on it, as on a block.
            let p = self.player.pos;
            glade.set_extra_blocks(
                feet.floor
                    .map(|top| {
                        let square = crate::controller::Footprint {
                            min: [p.x - 0.9, p.z - 0.9],
                            max: [p.x + 0.9, p.z + 0.9],
                        };
                        vec![(square, top)]
                    })
                    .unwrap_or_default(),
            );
        } else if let (Some(glade), Some(crypt)) = (&mut state.everglade, &mut state.crypt) {
            glade.tick(dt, &self.player, &[]);
            crypt.tick(dt);
        } else if let (Some(glade), Some(grove)) = (&mut state.everglade, &mut state.grove) {
            glade.tick(dt, &self.player, &[]);
            grove.tick(dt, glade, &self.player);
            grove.keep_clear(glade, &mut self.player);
            // The shape's pace holds however the player is placed.
            self.player.set_pace(grove.pace(glade, &self.player));
            // Held hotbar keys recast at their fixed rate, each casting
            // what its slot holds now.
            for slot in grove.due() {
                let Some(spell) = grove.slot_spell(slot) else {
                    continue;
                };
                if let Err(error) = grove.cast(spell, &mut self.player, glade) {
                    state.error = Some(error.chars().take(180).collect());
                }
            }
        } else if self.zone == ZoneId::MeteorStressTest {
            if let Some(glade) = &mut state.everglade {
                glade.tick(dt, &self.player, &super::meteor_stress::figures());
            }
        } else if self.zone == ZoneId::MeteorShowcase {
            if let Some(glade) = &mut state.everglade {
                // The caster stands only while a film stages the cast.
                let staged = glade.town().is_some_and(|t| t.bombardment()[0] > 0);
                let figures = if staged {
                    super::meteor_showcase::figures()
                } else {
                    Vec::new()
                };
                glade.tick(dt, &self.player, &figures);
            }
        } else if let Some(everglade) = &mut state.everglade {
            // The seats move first, so the characters pose where they stand.
            state.studio.set_player(Some(self.player.pos));
            state.studio.tick(dt);
            if self.zone == ZoneId::Everglade && !state.demolition {
                state.compute.tick(dt, unix_seconds());
            }
            let mut seats = if state.demolition {
                Vec::new()
            } else {
                state.studio.figures()
            };
            // The townsfolk stand and walk where the town clock puts them,
            // drawn as seats are.
            if !state.demolition && self.zone == ZoneId::Everglade {
                state
                    .townsfolk
                    .tick(everglade.town_time(), &self.world.blockers, self.player.pos);
                state.townsfolk.step(dt);
                seats.extend(state.townsfolk.figures());
            }
            everglade.tick(dt, &self.player, &seats);
        }
    }
    pub(crate) fn zone_dynamic_mesh(&self) -> crate::mesh::Mesh {
        let elapsed = self.zone_state.elapsed;
        let mut mesh = if self.is_bare() {
            self.grid_portal_mesh()
        } else {
            super::portal_mesh(self.zone, elapsed)
        };
        if let Some(lagrange) = &self.zone_state.lagrange {
            mesh.extend(lagrange.dynamic());
        }
        if let Some(lab) = &self.zone_state.lab {
            mesh.extend(lab.dynamic());
            // The lab has no suit of its own; the plaza character walks it.
            mesh.extend(&crate::avatar::mesh(&self.player, &self.gait));
        }
        if let (Some(glade), Some(coast)) = (&self.zone_state.everglade, &self.zone_state.coast) {
            mesh.extend(&coast.mesh(self.view(1.0).eye));
            mesh.extend(&glade.player_mesh(&self.player, &self.gait, self.hides_avatar()));
        } else if let (Some(glade), Some(lab)) =
            (&self.zone_state.everglade, &self.zone_state.water)
        {
            // The cove's stage and water, the floating bodies and the
            // particles, the character (not in first person), and its spells.
            let eye = self.view(1.0).eye;
            mesh.extend(&lab.mesh(eye));
            let mut player = glade.player_mesh(&self.player, &self.gait, self.hides_avatar());
            // The dummies stand in the character's figure, lit by the
            // probes as the character is.
            if let Some((mut figure, start)) = lab.figure(player.figure.as_ref()) {
                let mut vertices = figure.vertices.as_ref().clone();
                if let Some(dummies) = vertices.get_mut(start..) {
                    glade.shade(dummies);
                }
                figure.vertices = std::sync::Arc::new(vertices);
                player.figure = Some(figure);
            }
            mesh.extend(&player);
            mesh.extend(&glade.spell_mesh_from(&self.player, eye));
        } else if let (Some(glade), Some(crypt)) =
            (&self.zone_state.everglade, &self.zone_state.crypt)
        {
            // The hall's candlelit stage, moonbeam, and effects, the
            // character (not in first person), and the glade's spells.
            let eye = self.view(1.0).eye;
            mesh.extend(&crypt.mesh(eye));
            mesh.extend(&glade.player_mesh(&self.player, &self.gait, self.hides_avatar()));
            mesh.extend(&glade.spell_mesh_from(&self.player, eye));
        } else if let (Some(glade), Some(grove)) =
            (&self.zone_state.everglade, &self.zone_state.grove)
        {
            // Everglade's lit stage, the character and the dummies in one
            // figure, the glade's spells, and the field's bars and effects.
            mesh.extend(glade.dynamic());
            mesh.extend(&crate::mesh::Mesh {
                figure: Some(glade.with_town(grove.figure(glade))),
                instances: glade.town_instances(),
                ..crate::mesh::Mesh::default()
            });
            mesh.extend(&glade.spell_mesh(&self.player));
            let eye = self.view(1.0).eye;
            mesh.extend(&grove.mesh(eye, &self.player));
            // The field's fires and fireflies come last and take what the
            // spells leave of one system's particles, the dust of a
            // crumbling Wall of Stone included.
            mesh.sprites.truncate(crate::fx::system::MAX_PARTICLES);
            // The dusk stage, lit by the fires and the spells.
            let lights = grove.lights(glade, &self.player, eye);
            mesh.neon = Some(grove.lit_stage(&lights, glade.elapsed()));
        } else if let Some(everglade) = &self.zone_state.everglade {
            // Carries the lit stage the textured glade draws on, with its
            // water seen from the eye: from below when the eye is in it.
            mesh.extend(everglade.dynamic());
            everglade.see_water_from(&mut mesh, self.view(1.0).eye);
            // The player, and the seats when the pack's character draws them.
            // In first person the player's own character is not drawn.
            mesh.extend(&everglade.player_mesh(&self.player, &self.gait, self.hides_avatar()));
            let eye = self.view(1.0).eye;
            // The live spells: stone panels, wind, the cylinder and its
            // rising particles, feathers.
            mesh.extend(&everglade.spell_mesh_from(&self.player, eye));
            // The studio's nameplates, lamps, marks, bubbles, particles, and
            // live boards, and boxy seats when there is no character.
            if self.zone == ZoneId::Everglade && everglade.demolition().is_none() {
                mesh.extend(
                    &self
                        .zone_state
                        .studio
                        .draw(eye, !everglade.has_characters()),
                );
                // The townsfolk's nameplates.
                mesh.extend(&self.zone_state.townsfolk.draw(eye));
                // The Pylon Field and the Wellspring, with the beam to
                // Alice's workstation while her seat works.
                let alice = crate::zones::everglade::compute::seat_working(
                    self.zone_state.studio.view(),
                    crate::zones::everglade::studio::WORKSHOP_AGENT,
                );
                mesh.extend(
                    &self
                        .zone_state
                        .compute
                        .mesh(eye, alice, everglade.light().night),
                );
                // The basin's and the serving crystals' lamps, near the
                // field: they light the stones and the ground at night.
                if let Some(neon) = mesh.neon.as_mut() {
                    self.zone_state.compute.light(neon, self.player.pos);
                }
                if !self.is_hosted() {
                    mesh.extend(&self.zone_state.sales_floor.mesh());
                }
            }
        }
        if let (Some(glade), Some(neon)) = (&self.zone_state.everglade, mesh.neon.as_mut()) {
            neon.flash_lamps = glade.flash_lamps(self.view(1.0).eye);
        }
        mesh
    }
}

/// Now, in Unix seconds, for the Pylon Field's freshness rule.
fn unix_seconds() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(zone: &str, cmd: &str, args: Vec<crate::mv::Arg>) -> crate::mv::Command {
        crate::mv::Command {
            v: 1,
            zone: zone.into(),
            cmd: cmd.into(),
            args,
            t: 0,
            id: "c1".into(),
        }
    }

    #[test]
    fn zone_command_refuses_without_a_simulation_or_in_another_zone() {
        let mut world = WorldRuntime::new();
        let error = world
            .zone_command(&command("verse-lagrange-1", "status", vec![]))
            .unwrap_err();
        assert!(error.contains("not loaded"), "{error}");
        world.install_lagrange();
        let error = world
            .zone_command(&command("plaza", "status", vec![]))
            .unwrap_err();
        assert!(error.contains("not loaded"), "{error}");
    }

    #[test]
    fn zone_command_flies_grabs_and_refuses_headless_verbs() {
        use crate::mv::Arg;
        let mut world = WorldRuntime::new();
        world.install_lagrange();
        let zone = "verse-lagrange-1";
        let before = world
            .zone_state
            .lagrange
            .as_deref()
            .unwrap()
            .station
            .astronaut()
            .pos;
        let flown = world
            .zone_command(&command(zone, "fly", vec![Arg::Text("depot".into())]))
            .unwrap();
        assert!(flown["flying_to"].is_array());
        assert!(
            world
                .zone_state
                .lagrange
                .as_deref()
                .unwrap()
                .station
                .target
                .is_some()
        );
        world.zone_command(&command(zone, "stop", vec![])).unwrap();
        assert!(
            world
                .zone_state
                .lagrange
                .as_deref()
                .unwrap()
                .station
                .target
                .is_none()
        );
        let parts = world.zone_command(&command(zone, "parts", vec![])).unwrap();
        assert_eq!(parts.as_array().unwrap().len(), 6);
        let status = world
            .zone_command(&command(zone, "status", vec![]))
            .unwrap();
        assert!(status.is_object());
        for verb in ["install", "wait", "explode"] {
            let error = world
                .zone_command(&command(zone, verb, vec![]))
                .unwrap_err();
            assert!(!error.is_empty(), "{verb}");
        }
        let error = world
            .zone_command(&command(zone, "fly", vec![Arg::Text("nowhere".into())]))
            .unwrap_err();
        assert!(error.contains("nowhere"), "{error}");
        assert!(
            world
                .zone_state
                .lagrange
                .as_deref()
                .unwrap()
                .station
                .astronaut()
                .pos
                .is_finite(),
            "refused commands leave the station intact (started at {before})"
        );
    }
}

/// Shared 120 Hz Unix clock, retaining subsecond precision on native and web.
fn unix_water_tick() -> u64 {
    let elapsed = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap_or_default();
    elapsed
        .as_secs()
        .saturating_mul(physics::water::TICK_HZ)
        .saturating_add(u64::from(elapsed.subsec_nanos()) * physics::water::TICK_HZ / 1_000_000_000)
}
