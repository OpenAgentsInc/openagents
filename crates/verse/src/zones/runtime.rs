//! Zone transition admission and the shared forest simulation.

use super::{Control, Forest, Intent, LoadState, PortalProjection, Snapshot, ZoneId, assets};
use crate::{controller::PlayerController, runtime::WorldRuntime};
use glam::Vec3;

impl WorldRuntime {
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
        let forest = Forest::new(assets);
        self.world = forest.world();
        self.zone_state.forest = Some(forest);
        self.zone = ZoneId::Forest;
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.error = None;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(Vec3::new(0.0, 0.0, 8.0), std::f32::consts::PI);
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
            Intent::StartEncounter | Intent::ResetEncounter => {
                let forest = self
                    .zone_state
                    .forest
                    .as_mut()
                    .ok_or("Enter the forest first")?;
                let encounter =
                    super::rules::Encounter::new(&mut forest.dice).map_err(|e| e.to_string())?;
                let position = encounter.snapshot().wizard.position;
                forest.encounter = Some(encounter);
                forest.flash = 0.0;
                self.set_spawn(
                    Vec3::new(position[0], 0.0, position[1]),
                    std::f32::consts::PI,
                )?;
                self.zone_state.error = None;
            }
            Intent::Cast | Intent::EndTurn => {
                let forest = self
                    .zone_state
                    .forest
                    .as_mut()
                    .ok_or("Enter the forest first")?;
                let encounter = forest
                    .encounter
                    .as_mut()
                    .ok_or("Begin the encounter first")?;
                if intent == Intent::Cast {
                    encounter
                        .cast(&mut forest.dice)
                        .map_err(|e| e.to_string())?;
                    forest.flash = 0.35;
                } else {
                    encounter
                        .end_turn(&mut forest.dice)
                        .map_err(|e| e.to_string())?;
                }
                self.cancel_navigation();
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
        let encounter = self
            .zone_state
            .forest
            .as_ref()
            .and_then(|f| f.encounter.as_ref())
            .map(super::rules::Encounter::snapshot);
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
            if let Some(e) = &encounter {
                let active = e.status == super::rules::EncounterStatus::Active;
                add(
                    "cast",
                    "Fire Bolt",
                    Intent::Cast,
                    active && e.action_available,
                );
                add("end_turn", "End turn", Intent::EndTurn, active);
                add("reset_encounter", "Reset", Intent::ResetEncounter, true);
                add("return", "Plaza", Intent::Return, true);
                forest_caption(e)
            } else {
                add("start_encounter", "Encounter", Intent::StartEncounter, true);
                add("return", "Plaza", Intent::Return, true);
                "Atlantis forest · SRD 5.1".into()
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
            encounter,
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
            near: distance <= 6.0 && self.player.pos.y < 3.0,
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
                    && !crate::runtime::mesh_occludes(
                        &self.dynamic_mesh(),
                        view.eye,
                        direction,
                        length,
                    );
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
        if offset.x.hypot(offset.z) > 6.0 || self.player.pos.y >= 3.0 || self.zone_loading() {
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
            && !crate::runtime::mesh_occludes(&self.dynamic_mesh(), eye, direction, distance)
            && !crate::runtime::mesh_occludes(entities, eye, direction, distance)
    }
    pub(crate) fn forest_tick(&mut self, dt: f32, previous: PlayerController) {
        self.zone_state.elapsed = (self.zone_state.elapsed + dt) % 1000.0;
        if let Some(forest) = &mut self.zone_state.forest {
            forest.elapsed = (forest.elapsed + dt) % 1000.0;
            forest.flash = (forest.flash - dt).max(0.0);
            if let Some(encounter) = &mut forest.encounter {
                if encounter.snapshot().status != super::rules::EncounterStatus::Active {
                    return;
                }
                let position = [self.player.pos.x, self.player.pos.z];
                // An idle, jump-only, or camera-only frame must not erase a
                // refusal or create a synthetic encounter movement revision.
                if position != [previous.pos.x, previous.pos.z] {
                    match encounter.move_wizard_to(position) {
                        Ok(()) => self.zone_state.error = None,
                        Err(error) => {
                            self.player.pos.x = previous.pos.x;
                            self.player.pos.z = previous.pos.z;
                            self.player.speed = 0.0;
                            self.navigation.stop(crate::nav::NavigationStatus::Blocked);
                            self.zone_state.error = Some(error.to_string());
                        }
                    }
                }
            }
        }
    }
    pub(crate) fn zone_dynamic_mesh(&self) -> crate::mesh::Mesh {
        let elapsed = self.zone_state.elapsed;
        let mut mesh = super::portal_mesh(self.zone, elapsed);
        if let Some(forest) = &self.zone_state.forest {
            mesh.extend(&forest.dynamic(&self.player));
        }
        mesh
    }
}

fn forest_caption(encounter: &super::rules::EncounterSnapshot) -> String {
    if encounter.status != super::rules::EncounterStatus::Active {
        return format!(
            "You {} HP · Zombie {} HP · {}",
            encounter.wizard.hp, encounter.zombie.hp, encounter.last_notice
        );
    }
    let feet = (encounter.movement_remaining_m / super::rules::METERS_PER_FOOT).floor();
    format!(
        "R{} · You {} / Zombie {} HP · {feet:.0}ft · {} · {}",
        encounter.round,
        encounter.wizard.hp,
        encounter.zombie.hp,
        if encounter.action_available {
            "Action ready"
        } else {
            "Action spent"
        },
        encounter.last_notice
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Mesh;
    use crate::zones::rules::{Dice, Encounter, EncounterStatus, WIZARD_SPEED_M};
    use std::collections::VecDeque;

    struct Rolls(VecDeque<u8>);
    impl Dice for Rolls {
        fn roll(&mut self, _sides: u8) -> u8 {
            self.0.pop_front().expect("unexpected roll")
        }
    }
    fn rolls(values: &[u8]) -> Rolls {
        Rolls(values.iter().copied().collect())
    }
    fn assets() -> assets::LoadedAssets {
        let animation = || assets::AnimatedMesh {
            frames: vec![Mesh::default()],
            frame_seconds: 0.1,
        };
        assets::LoadedAssets {
            tree: Mesh::default(),
            wizard_still: Mesh::default(),
            wizard: animation(),
            zombie: animation(),
            zombie_walk: animation(),
        }
    }
    fn encounter_world() -> WorldRuntime {
        let mut world = WorldRuntime::new();
        world.install_forest(assets());
        world.zone_state.forest.as_mut().unwrap().encounter =
            Some(Encounter::new(&mut rolls(&[20, 1])).unwrap());
        world.set_spawn(Vec3::ZERO, std::f32::consts::PI).unwrap();
        world
    }

    #[test]
    fn entry_resets_camera_and_duplicate_completion_preserves_return_pose() {
        let mut world = WorldRuntime::new();
        world.set_spawn(Vec3::new(2.0, 0.0, 3.0), 0.5).unwrap();
        world.camera.distance = 40.0;
        world.install_forest(assets());
        assert_eq!(world.player.pos, Vec3::new(0.0, 0.0, 8.0));
        assert_eq!(
            world.camera.distance,
            crate::camera::FollowCamera::default().distance
        );
        let revision = world.zone_revision;
        world.player.pos = Vec3::new(1.0, 0.0, 2.0);
        world.install_forest(assets());
        assert_eq!(world.zone_revision, revision);
        assert_eq!(world.player.pos, Vec3::new(1.0, 0.0, 2.0));
        world.zone_intent(Intent::Return).unwrap();
        assert_eq!(world.player.pos, Vec3::new(2.0, 0.0, 3.0));
        assert!(world.zone_state.forest.is_none());
    }

    #[test]
    fn refused_movement_keeps_camera_and_jump_but_shows_reason_until_valid_move() {
        let mut world = encounter_world();
        let previous = world.player;
        world.player.pos = Vec3::new(10.1, 0.2, 0.0);
        world.player.yaw = 1.0;
        world.player.speed = 6.0;
        world.forest_tick(0.01, previous);
        assert_eq!(world.player.pos, Vec3::new(0.0, 0.2, 0.0));
        assert_eq!(world.player.yaw, 1.0);
        assert_eq!(world.player.speed, 0.0);
        assert_eq!(
            world.zone_snapshot(1.0).error.as_deref(),
            Some("Stay inside the encounter circle.")
        );
        let paused = world.player;
        let revision = world.zone_snapshot(1.0).encounter.unwrap().revision;
        world.forest_tick(0.01, paused);
        assert!(world.zone_snapshot(1.0).error.is_some());
        assert_eq!(
            world.zone_snapshot(1.0).encounter.unwrap().revision,
            revision
        );
        world.player.pos.x = 0.25;
        world.forest_tick(0.01, paused);
        assert!(world.zone_snapshot(1.0).error.is_none());
        assert_eq!(
            world.zone_snapshot(1.0).encounter.unwrap().wizard.position,
            [0.25, 0.0]
        );
    }

    #[test]
    fn caption_reports_action_movement_and_round_without_automatic_turns() {
        let mut world = encounter_world();
        let initial = world.zone_snapshot(1.0);
        assert!(initial.caption.contains("R1"));
        assert!(initial.caption.contains("30ft"));
        assert!(initial.caption.contains("Action ready"));
        let encounter = world
            .zone_state
            .forest
            .as_mut()
            .unwrap()
            .encounter
            .as_mut()
            .unwrap();
        encounter.cast(&mut rolls(&[10, 5])).unwrap();
        let previous = world.player;
        world.player.pos.x = 0.3048;
        world.forest_tick(0.01, previous);
        let cast = world.zone_snapshot(1.0);
        assert!(cast.caption.contains("R1"));
        assert!(cast.caption.contains("29ft"));
        assert!(cast.caption.contains("Action spent"));
        assert!(
            !cast
                .controls
                .iter()
                .find(|c| c.id == "cast")
                .unwrap()
                .enabled
        );
        assert!(
            cast.controls
                .iter()
                .find(|c| c.id == "end_turn")
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn finished_encounter_disables_combat_and_allows_exploration() {
        let mut world = encounter_world();
        let encounter = world
            .zone_state
            .forest
            .as_mut()
            .unwrap()
            .encounter
            .as_mut()
            .unwrap();
        encounter.cast(&mut rolls(&[20, 10, 10])).unwrap();
        encounter.end_turn(&mut rolls(&[1])).unwrap();
        encounter.cast(&mut rolls(&[20, 20, 10, 10])).unwrap();
        let finished = world.zone_snapshot(1.0);
        assert_eq!(finished.encounter.unwrap().status, EncounterStatus::Won);
        for control in finished.controls {
            assert_eq!(
                control.enabled,
                matches!(control.id, "reset_encounter" | "return")
            );
        }
        let previous = world.player;
        world.player.pos = Vec3::new(12.0, 0.0, 0.0);
        world.forest_tick(0.01, previous);
        assert_eq!(world.player.pos.x, 12.0);
    }

    #[test]
    fn reset_and_return_clear_refusals_and_restore_unblocked_movement() {
        let mut world = encounter_world();
        world.zone_state.error = Some("Movement spent. End your turn.".into());
        world.zone_intent(Intent::ResetEncounter).unwrap();
        let reset = world.zone_snapshot(1.0);
        assert!(reset.error.is_none());
        assert_eq!(world.player.pos, Vec3::ZERO);
        let encounter = reset.encounter.unwrap();
        if encounter.status == EncounterStatus::Active {
            assert_eq!(encounter.movement_remaining_m, WIZARD_SPEED_M);
        }
        let previous = world.player;
        world.player.pos.x = 0.1;
        world.forest_tick(0.01, previous);
        assert_eq!(world.player.pos.x, 0.1);
        world.zone_state.error = Some("Stay inside the encounter circle.".into());
        world.zone_intent(Intent::Return).unwrap();
        assert!(world.zone_snapshot(1.0).error.is_none());
        assert!(world.is_plaza());
        assert!(world.zone_state.forest.is_none());
    }
}
