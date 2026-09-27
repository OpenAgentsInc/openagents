//! Zone transition admission and the shared forest simulation.

use super::{Control, Forest, Intent, LoadState, PortalProjection, Snapshot, ZoneId, assets};
use crate::{
    controller::{InputState, PlayerController},
    runtime::WorldRuntime,
};
use glam::Vec3;

impl WorldRuntime {
    pub(crate) fn update_player(&mut self, input: &InputState, dt: f32) {
        if let Some(forest) = &mut self.zone_state.forest {
            forest.move_player(&mut self.player, input, dt);
        } else {
            self.player
                .update(input, dt, &self.world.blockers, self.zone_half());
        }
    }

    /// Configure a host-owned cache directory. This does not read or fetch assets.
    pub fn configure_zone_cache(&mut self, path: std::path::PathBuf) {
        self.zone_cancel_loading();
        self.zone_state.loader = Some(assets::Loader::new(path));
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
        if let Some(loader) = &mut self.zone_state.loader {
            loader.cancel();
        }
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.progress = 0.0;
        self.zone_state.error = None;
    }
    /// Poll only while the native surface is active, before its presence tick.
    /// A true result invalidates the old world GPU buffers and pointer capture.
    pub fn zone_tick(&mut self) -> bool {
        let event = self
            .zone_state
            .loader
            .as_mut()
            .and_then(assets::Loader::poll);
        if !self.zone_loading() {
            return false;
        }
        match event {
            Some(assets::LoadEvent::Progress { received, total }) => {
                self.zone_state.progress = if total == 0 {
                    0.0
                } else {
                    (received as f32 / total as f32).clamp(0.0, 1.0)
                };
            }
            Some(assets::LoadEvent::Ready(assets)) => {
                self.install_forest(*assets);
                return true;
            }
            Some(assets::LoadEvent::Failed(error)) => {
                self.zone_state.error = Some(error.chars().take(180).collect());
                self.zone_state.loading = LoadState::Failed;
            }
            None => {}
        }
        false
    }
    /// Install already verified artwork. Offline tools use the same decoder.
    pub fn install_forest(&mut self, assets: assets::LoadedAssets) {
        // A repeated completion cannot replace the saved plaza return pose.
        if !self.is_plaza() {
            return;
        }
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        let forest = match Forest::new(assets) {
            Ok(forest) => forest,
            Err(error) => {
                self.zone_state.error = Some(error);
                self.zone_state.loading = LoadState::Failed;
                return;
            }
        };
        self.world = forest.world();
        self.zone_state.forest = Some(forest);
        self.zone = ZoneId::Forest;
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.error = None;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(Forest::spawn(), 0.0);
        self.camera = crate::camera::FollowCamera::default();
    }
    pub fn zone_intent(&mut self, intent: Intent) -> Result<(), String> {
        let result = self.apply_zone_intent(intent);
        if let Err(error) = &result {
            self.zone_state.error = Some(error.chars().take(180).collect());
        }
        result
    }
    fn apply_zone_intent(&mut self, intent: Intent) -> Result<(), String> {
        match intent {
            Intent::Enter | Intent::Retry => {
                if !self.is_plaza() || self.zone_loading() || !self.zone_portal(1.0).near {
                    return Err("Approach the forest portal".into());
                }
                super::Manifest::forest()?;
                let loader = self
                    .zone_state
                    .loader
                    .as_mut()
                    .ok_or("Zone storage is unavailable")?;
                if !loader.request() {
                    return Err("Finishing the previous load; try again".into());
                }
                self.cancel_navigation();
                self.doors.cancel_transient();
                self.zone_state.loading = LoadState::Loading;
                self.zone_state.progress = 0.0;
                self.zone_state.error = None;
            }
            Intent::Cancel => self.zone_cancel_loading(),
            Intent::Return => {
                if self.is_plaza() {
                    return Err("You are already in the plaza".into());
                }
                self.zone_cancel_loading();
                // Dropping the forest releases its decoded animation frames.
                self.zone_state.forest = None;
                self.world = crate::world::build();
                self.zone = ZoneId::Plaza;
                self.zone_revision = self.zone_revision.saturating_add(1);
                let (pos, yaw) = self
                    .zone_state
                    .plaza_pose
                    .take()
                    .unwrap_or((crate::world::SPAWN, 0.0));
                self.set_spawn(pos, yaw)?;
                self.camera = crate::camera::FollowCamera::default();
            }
            Intent::Firebolt | Intent::MagicMissile | Intent::Fireball => {
                let spell = match intent {
                    Intent::Firebolt => verse_atlantis::Spell::Firebolt,
                    Intent::MagicMissile => verse_atlantis::Spell::MagicMissile,
                    _ => verse_atlantis::Spell::Fireball,
                };
                let direction = self.player.forward();
                let origin = self.player.pos + Vec3::Y * 1.4 + direction * 0.25;
                let forest = self
                    .zone_state
                    .forest
                    .as_mut()
                    .ok_or("Enter the forest first")?;
                forest
                    .simulation
                    .cast(spell, origin.to_array(), direction.to_array())?;
                self.zone_state.error = None;
            }
        }
        Ok(())
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
        let combat = self.zone_state.forest.as_ref().map(|f| f.snapshot.clone());
        let caption = if self.zone_loading() {
            add("cancel", "Cancel", Intent::Cancel, true);
            format!(
                "Loading Atlantis forest · {}%",
                (self.zone_state.progress * 100.0) as u32
            )
        } else if self.zone_state.loading == LoadState::Failed {
            add("retry", "Retry", Intent::Retry, portal.near);
            add("cancel", "Dismiss", Intent::Cancel, true);
            "Forest could not load".into()
        } else if self.zone == ZoneId::Forest {
            if let Some(c) = &combat {
                for ability in &c.abilities {
                    let (id, label, intent) = match ability.id {
                        verse_atlantis::Spell::Firebolt => {
                            ("firebolt", "Firebolt", Intent::Firebolt)
                        }
                        verse_atlantis::Spell::MagicMissile => {
                            ("magic_missile", "Missile", Intent::MagicMissile)
                        }
                        verse_atlantis::Spell::Fireball => {
                            ("fireball", "Fireball", Intent::Fireball)
                        }
                    };
                    add(id, label, intent, ability.ready);
                }
                add("return", "Plaza", Intent::Return, true);
                if c.player.hp <= 0 {
                    "Defeated · return to Plaza".into()
                } else {
                    format!(
                        "HP {} / {}   Mana {} / {}",
                        c.player.hp, c.player.max_hp, c.player.mana, c.player.max_mana
                    )
                }
            } else {
                String::new()
            }
        } else if portal.near && portal.visible {
            add(
                "enter",
                "Enter forest",
                Intent::Enter,
                self.zone_state.loader.is_some(),
            );
            "Atlantis forest · load this zone".into()
        } else {
            String::new()
        };
        Snapshot {
            id: self.zone,
            label: self.zone.label(),
            state: self.zone_state.loading,
            progress: self.zone_state.progress,
            error: self.zone_state.error.clone(),
            portal,
            controls,
            combat,
            caption,
        }
    }
    fn zone_portal(&self, aspect: f32) -> PortalProjection {
        let at = self.zone.portal();
        let offset = self.player.pos - at;
        let distance = offset.x.hypot(offset.z);
        let anchor = at + Vec3::new(0.0, 2.5, -0.3);
        let view = self.view(aspect);
        let clip = view.view_proj * anchor.extend(1.0);
        let mut p = PortalProjection {
            near: distance <= 6.0 && self.player.pos.y - at.y < 3.0,
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
        let offset = self.player.pos - self.zone.portal();
        if offset.x.hypot(offset.z) > 6.0
            || self.player.pos.y - self.zone.portal().y >= 3.0
            || self.zone_loading()
        {
            return false;
        }
        let view = self.view(aspect);
        let Some((eye, direction)) = crate::runtime::viewport_ray(&view, aspect, x, y) else {
            return false;
        };
        let at = self.zone.portal() + Vec3::new(0.0, 2.25, -0.3);
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
        if let Some(forest) = &self.zone_state.forest {
            crate::runtime::mesh_occludes(forest.dynamic(), eye, direction, distance)
        } else {
            crate::runtime::mesh_occludes(&self.dynamic_mesh(), eye, direction, distance)
        }
    }
    pub(crate) fn forest_tick(&mut self, dt: f32, _previous: PlayerController) {
        self.zone_state.elapsed = (self.zone_state.elapsed + dt) % 1000.0;
        if let Some(forest) = &mut self.zone_state.forest
            && let Err(error) = forest.tick(dt, &self.player)
        {
            self.zone_state.error = Some(error);
        }
    }
    pub(crate) fn zone_dynamic_mesh(&self) -> crate::mesh::Mesh {
        let elapsed = self.zone_state.elapsed;
        let mut mesh = super::portal_mesh(self.zone, elapsed);
        if let Some(forest) = &self.zone_state.forest {
            mesh.extend(forest.dynamic());
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{controller::InputState, mesh::Mesh};

    fn forest_world() -> WorldRuntime {
        let animation = || assets::AnimatedMesh {
            frames: vec![Mesh::default()],
            frame_seconds: 0.1,
        };
        let mut world = WorldRuntime::new();
        world.install_forest(assets::LoadedAssets {
            tree: Mesh::default(),
            wizard_still: Mesh::default(),
            wizard: animation(),
            zombie: animation(),
            zombie_walk: animation(),
        });
        world
    }

    #[test]
    fn forest_runs_immediately_and_fireball_spends_mana_while_moving() {
        let mut world = forest_world();
        let initial = world.zone_snapshot(1.0).combat.unwrap();
        assert_eq!(
            initial.actors.iter().filter(|a| a.kind == "zombie").count(),
            35
        );
        assert_eq!(world.player.pos, Forest::spawn());
        let moving = InputState {
            forward: true,
            ..Default::default()
        };
        world.zone_intent(Intent::Fireball).unwrap();
        world.tick(&moving, 0.05);
        let cast = world.zone_snapshot(1.0).combat.unwrap();
        assert!(cast.player.mana < initial.player.mana);
        assert!(
            cast.abilities
                .iter()
                .find(|a| a.id == verse_atlantis::Spell::Fireball)
                .unwrap()
                .cooldown_remaining
                > 0.0
        );
        assert!(world.player.pos.z > 0.0);
        assert!(
            cast.projectiles
                .iter()
                .any(|p| p.kind == verse_atlantis::Spell::Fireball)
        );
        for _ in 0..80 {
            world.tick(&moving, 0.05);
        }
        let later = world.zone_snapshot(1.0).combat.unwrap();
        assert!(world.player.pos.z > 20.0, "no per-turn movement allowance");
        assert!(
            initial
                .actors
                .iter()
                .filter(|a| a.kind == "zombie")
                .any(|a| later.actors.iter().any(|b| b.id == a.id && b.pos != a.pos))
        );
        assert!(
            later
                .abilities
                .iter()
                .find(|a| a.id == verse_atlantis::Spell::Fireball)
                .unwrap()
                .ready
        );
        assert!(
            !world
                .zone_snapshot(1.0)
                .controls
                .iter()
                .any(|c| c.id.contains("turn") && c.id != "return")
        );
    }

    #[test]
    fn forest_uses_source_strafe_and_terrain_camera_and_stops_defeated_players() {
        let mut world = forest_world();
        let moving = InputState {
            left: true,
            mouse_look: true,
            ..Default::default()
        };
        world.tick(&moving, 0.05);
        assert!(world.player.pos.x.abs() > 0.1);
        assert_eq!(world.player.yaw, 0.0);
        world.camera.distance = crate::camera::MIN_DISTANCE;
        world.camera.pitch = -0.6;
        let view = world.view(1.0);
        let floor = verse_atlantis::scene::Terrain::bundled().height(view.eye.x, view.eye.z);
        assert!(view.eye.y < 0.0, "camera follows negative terrain");
        assert!(view.eye.y >= floor + 0.399);
        world.zone_state.forest.as_mut().unwrap().snapshot.player.hp = 0;
        let before = world.player.pos;
        world.update_player(&moving, 0.05);
        assert_eq!(world.player.pos, before);
        assert!(world.zone_snapshot(1.0).caption.contains("Defeated"));
        assert!(
            world
                .zone_snapshot(1.0)
                .controls
                .iter()
                .find(|c| c.id == "return")
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn return_drops_combat_and_restores_the_plaza_pose() {
        let mut world = WorldRuntime::new();
        let original = world.player;
        let mut forest = forest_world();
        world.install_forest(forest.zone_state.forest.take().unwrap().assets);
        world.zone_intent(Intent::Firebolt).unwrap();
        world.tick(&InputState::default(), 0.05);
        world.zone_intent(Intent::Return).unwrap();
        assert_eq!(world.player, original);
        assert!(world.zone_snapshot(1.0).combat.is_none());
        assert!(world.zone_state.forest.is_none());
    }
}
