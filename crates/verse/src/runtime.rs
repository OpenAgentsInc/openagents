//! The shared Verse world simulation. Platform adapters supply intent and time.
//!
//! This module opens no windows, files, sockets, or model connections. Desktop
//! and native surfaces advance the same player, camera, gait, and follower.

use glam::Vec3;

use crate::agent::Agent;
use crate::avatar::{self, Gait};
use crate::camera::FollowCamera;
use crate::controller::{InputState, PlayerController, wrap};
use crate::mesh::Mesh;
use crate::render::View;
use crate::world::{self, World};

/// Long pauses never become a large physics step.
pub const MAX_FRAME_SECONDS: f32 = rust_native::surface::MAX_FRAME_DELTA;

/// The shared computer's proximity and viewport projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Computer {
    /// Within the interaction radius in the ground plane.
    pub near: bool,
    /// The monitor anchor is in front of the camera and inside the viewport.
    /// This is projection visibility, not an occlusion test.
    pub visible: bool,
    /// Horizontal anchor in [0, 1], measured from the viewport's left edge.
    pub screen_x: f32,
    /// Vertical anchor in [0, 1], measured from the viewport's top edge.
    pub screen_y: f32,
    /// Ground-plane distance to the desk's center, in meters.
    pub distance: f32,
}

/// Spatial state for the Gym. Occupancy alone grants no execution authority.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gym {
    /// The player's feet are inside the hall and below its upper structure.
    pub inside: bool,
    /// Inside and within six ground-plane meters of the central board.
    pub near: bool,
    /// The board anchor projects into the viewport, without an occlusion test.
    pub visible: bool,
    /// Horizontal normalized anchor from the viewport's left edge.
    pub screen_x: f32,
    /// Vertical normalized anchor from the viewport's top edge.
    pub screen_y: f32,
    /// Ground-plane distance to the central board, in meters.
    pub distance: f32,
}

/// Camera intent, independent of a mouse, touchscreen, or gamepad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Orbit without turning the player, in logical input pixels.
    Orbit { dx: f32, dy: f32 },
    /// Turn the player and adjust camera pitch, in logical input pixels.
    Look { dx: f32, dy: f32 },
    /// Transfer the current camera orbit to the player's heading.
    FaceCamera,
    /// Positive lines move the camera closer.
    Zoom { lines: f32 },
}

/// State shared by every Verse surface. Services remain separate owners.
pub struct WorldRuntime {
    pub world: World,
    pub player: PlayerController,
    pub camera: FollowCamera,
    pub gait: Gait,
    pub agent: Agent,
}

impl Default for WorldRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldRuntime {
    #[must_use]
    pub fn new() -> Self {
        let world = world::build();
        let player = PlayerController::new(world::SPAWN, 0.0);
        let agent = Agent::new(&player);
        Self {
            world,
            player,
            camera: FollowCamera::default(),
            gait: Gait::default(),
            agent,
        }
    }

    /// Applies bounded, finite camera input. Invalid input changes no state.
    pub fn apply(&mut self, action: Action) -> Result<(), String> {
        let motion = |dx: f32, dy: f32| {
            dx.is_finite() && dy.is_finite() && dx.abs() <= 4096.0 && dy.abs() <= 4096.0
        };
        match action {
            Action::Orbit { dx, dy } if motion(dx, dy) => self.camera.orbit(dx, dy),
            Action::Look { dx, dy } if motion(dx, dy) => {
                self.player.yaw = wrap(self.player.yaw + self.camera.mouselook(dx, dy));
            }
            Action::FaceCamera => {
                self.player.yaw = wrap(self.player.yaw + self.camera.take_offset());
            }
            Action::Zoom { lines } if lines.is_finite() && lines.abs() <= 100.0 => {
                self.camera.zoom(lines);
            }
            _ => return Err("camera input is nonfinite or exceeds its bound".into()),
        }
        Ok(())
    }

    /// Advances ordinary movement and the following agent; returns applied dt.
    pub fn tick(&mut self, input: &InputState, dt: f32) -> f32 {
        self.tick_with_mode(input, dt, false, true)
    }

    /// As [`Self::tick`], with platform orbit capture and external agent motion.
    /// Replays set `follow_agent` false, then advance the shared replay timeline.
    pub fn tick_with_mode(
        &mut self,
        input: &InputState,
        dt: f32,
        orbiting: bool,
        follow_agent: bool,
    ) -> f32 {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, MAX_FRAME_SECONDS)
        } else {
            0.0
        };
        if dt == 0.0 {
            return 0.0;
        }
        self.player
            .update(input, dt, &self.world.blockers, world::HALF);
        if self.player.speed > 0.1 && !orbiting {
            self.camera.settle(dt);
        }
        self.gait
            .advance(self.player.speed, self.player.airborne(), dt);
        if follow_agent {
            self.agent.update(&self.player, dt);
        }
        dt
    }

    #[must_use]
    pub fn view(&self, aspect: f32) -> View {
        let aspect = if aspect.is_finite() {
            aspect.clamp(0.01, 100.0)
        } else {
            1.0
        };
        View {
            view_proj: self
                .camera
                .view_proj(self.player.pos, self.player.yaw, aspect),
            eye: self.camera.eye(self.player.pos, self.player.yaw),
        }
    }

    /// Projects the monitor into the same viewport used by the renderer.
    /// Invalid aspects produce a hidden, finite anchor rather than NaNs.
    #[must_use]
    pub fn computer(&self, aspect: f32) -> Computer {
        let offset = self.player.pos - world::COMPUTER;
        let distance = offset.x.hypot(offset.z);
        let clip = self.view(aspect).view_proj * world::COMPUTER_SCREEN.extend(1.0);
        let mut result = Computer {
            near: distance <= world::COMPUTER_RANGE,
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance,
        };
        if aspect.is_finite() && aspect > 0.0 && clip.is_finite() && clip.w > 0.0 {
            let ndc = clip.truncate() / clip.w;
            result.screen_x = (ndc.x * 0.5 + 0.5).clamp(0.0, 1.0);
            result.screen_y = (0.5 - ndc.y * 0.5).clamp(0.0, 1.0);
            result.visible = (-1.0..=1.0).contains(&ndc.x)
                && (-1.0..=1.0).contains(&ndc.y)
                && (0.0..=1.0).contains(&ndc.z);
        }
        result
    }

    /// Tests a normalized viewport point against the physical monitor.
    /// A tap must reach the front display from a nearby player and pass the
    /// same static and local-entity faces that hide it in the scene renderer.
    #[must_use]
    pub fn computer_hit(&self, aspect: f32, x: f32, y: f32) -> bool {
        self.computer_hit_with_entities(aspect, x, y, &Mesh::default())
    }

    /// As [`Self::computer_hit`], including externally owned entity faces.
    /// Supply the same remote-entity mesh used for the last presented frame;
    /// picking must not advance crowd interpolation or animation on its own.
    #[must_use]
    pub fn computer_hit_with_entities(&self, aspect: f32, x: f32, y: f32, entities: &Mesh) -> bool {
        if !aspect.is_finite()
            || aspect <= 0.0
            || !(0.0..=1.0).contains(&x)
            || !(0.0..=1.0).contains(&y)
            || !self.computer(aspect).near
        {
            return false;
        }
        let view = self.view(aspect);
        // The monitor faces -Z. Reject a camera behind the display even if
        // its projected rectangle happens to cover this viewport point.
        if view.eye.z >= world::COMPUTER_SCREEN.z {
            return false;
        }
        let far =
            view.view_proj.inverse() * glam::Vec4::new(x * 2.0 - 1.0, 1.0 - y * 2.0, 1.0, 1.0);
        if !far.is_finite() || far.w.abs() < f32::EPSILON {
            return false;
        }
        let direction = (far.truncate() / far.w - view.eye).normalize_or_zero();
        if direction.z <= f32::EPSILON {
            return false;
        }
        let distance = (world::COMPUTER_SCREEN.z - view.eye.z) / direction.z;
        let target = view.eye + direction * distance;
        let offset = target - world::COMPUTER_SCREEN;
        if !target.is_finite()
            || offset.x.abs() > world::COMPUTER_SCREEN_HALF[0]
            || offset.y.abs() > world::COMPUTER_SCREEN_HALF[1]
        {
            return false;
        }
        let clip = view.view_proj * target.extend(1.0);
        if clip.w <= 0.0 || !(0.0..=1.0).contains(&(clip.z / clip.w)) {
            return false;
        }
        !mesh_occludes(&self.world.mesh, view.eye, direction, distance)
            && !mesh_occludes(&self.dynamic_mesh(), view.eye, direction, distance)
            && !mesh_occludes(entities, view.eye, direction, distance)
    }

    /// Reports occupancy and the central Gym board's normalized projection.
    /// Hosts gate Gym reads on `inside` and their own active-surface state.
    #[must_use]
    pub fn gym(&self, aspect: f32) -> Gym {
        let position = self.player.pos;
        let inside = (36.5..59.5).contains(&position.x)
            && (-8.5..8.5).contains(&position.z)
            && (0.0..=4.5).contains(&position.y);
        let offset = position - world::GYM_BOARD;
        let distance = offset.x.hypot(offset.z);
        let clip = self.view(aspect).view_proj * world::GYM_BOARD.extend(1.0);
        let mut result = Gym {
            inside,
            near: inside && distance <= 6.0,
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance,
        };
        if aspect.is_finite() && aspect > 0.0 && clip.is_finite() && clip.w > 0.0 {
            let ndc = clip.truncate() / clip.w;
            result.screen_x = (ndc.x * 0.5 + 0.5).clamp(0.0, 1.0);
            result.screen_y = (0.5 - ndc.y * 0.5).clamp(0.0, 1.0);
            result.visible = (-1.0..=1.0).contains(&ndc.x)
                && (-1.0..=1.0).contains(&ndc.y)
                && (0.0..=1.0).contains(&ndc.z);
        }
        result
    }

    /// Local geometry with a neutral monitor. Services append remote entities.
    #[must_use]
    pub fn dynamic_mesh(&self) -> Mesh {
        self.mesh_with_computer_display(None)
    }

    /// Local geometry with the mobile computer's proximity and tap prompt.
    /// Use only when the host implements picking and opens the computer.
    #[must_use]
    pub fn dynamic_mesh_with_computer_interaction(&self) -> Mesh {
        let offset = self.player.pos - world::COMPUTER;
        self.mesh_with_computer_display(Some(offset.x.hypot(offset.z) <= world::COMPUTER_RANGE))
    }

    fn mesh_with_computer_display(&self, interaction: Option<bool>) -> Mesh {
        let mut dynamic = avatar::mesh(&self.player, &self.gait);
        dynamic.extend(&self.agent.mesh());
        dynamic.extend(&world::computer_display(interaction));
        dynamic
    }

    /// Sets a finite spawn inside the world. Source admission checks placement.
    pub fn set_spawn(&mut self, position: Vec3, yaw: f32) -> Result<(), String> {
        if !position.is_finite()
            || !yaw.is_finite()
            || position.x.abs() >= world::HALF
            || position.z.abs() >= world::HALF
            || position.y < 0.0
            || position.y > 100.0
        {
            return Err("spawn is nonfinite or outside the world".into());
        }
        self.player = PlayerController::new(position, wrap(yaw));
        self.agent = Agent::new(&self.player);
        self.gait = Gait::default();
        Ok(())
    }
}

fn mesh_occludes(mesh: &Mesh, origin: Vec3, direction: Vec3, distance: f32) -> bool {
    // Möller–Trumbore, double-sided like the renderer's face pipeline. Ignore
    // the destination plane itself, including its amber display strokes.
    mesh.faces.chunks_exact(3).any(|vertices| {
        let a = Vec3::from(vertices[0].pos);
        let edge_ab = Vec3::from(vertices[1].pos) - a;
        let edge_ac = Vec3::from(vertices[2].pos) - a;
        let cross = direction.cross(edge_ac);
        let determinant = edge_ab.dot(cross);
        if determinant.abs() < 1e-7 {
            return false;
        }
        let inverse = determinant.recip();
        let from_a = origin - a;
        let u = from_a.dot(cross) * inverse;
        if !(0.0..=1.0).contains(&u) {
            return false;
        }
        let cross = from_a.cross(edge_ab);
        let v = direction.dot(cross) * inverse;
        if v < 0.0 || u + v > 1.0 {
            return false;
        }
        let hit = edge_ac.dot(cross) * inverse;
        hit > 0.001 && hit < distance - 0.003
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gym_entry_requires_the_real_doorway_and_excludes_walls_and_roof() {
        let mut runtime = WorldRuntime::new();
        runtime
            .set_spawn(Vec3::new(33.0, 0.0, 0.0), std::f32::consts::FRAC_PI_2)
            .unwrap();
        assert!(!runtime.gym(1.0).inside);
        for _ in 0..220 {
            runtime.tick(
                &InputState {
                    forward: true,
                    ..InputState::default()
                },
                1.0 / 60.0,
            );
        }
        let board = runtime.gym(1.0);
        assert!(board.inside && board.near && board.visible);
        assert!((board.screen_x - 0.5).abs() < 0.001);
        assert!(runtime.player.pos.x > 55.0);
        for _ in 0..360 {
            runtime.tick(
                &InputState {
                    backward: true,
                    ..InputState::default()
                },
                1.0 / 60.0,
            );
        }
        assert!(!runtime.gym(1.0).inside);
        runtime
            .set_spawn(Vec3::new(33.0, 0.0, 6.0), std::f32::consts::FRAC_PI_2)
            .unwrap();
        for _ in 0..120 {
            runtime.tick(
                &InputState {
                    forward: true,
                    ..InputState::default()
                },
                1.0 / 60.0,
            );
        }
        assert!(runtime.player.pos.x < 36.0);
        assert!(!runtime.gym(1.0).inside);
        for position in [
            Vec3::new(36.49, 0.0, 0.0),
            Vec3::new(59.5, 0.0, 0.0),
            Vec3::new(48.0, 0.0, -8.51),
            Vec3::new(48.0, 0.0, 8.5),
            Vec3::new(58.0, 4.51, 0.0),
            Vec3::new(58.0, 6.0, 0.0),
        ] {
            runtime
                .set_spawn(position, std::f32::consts::FRAC_PI_2)
                .unwrap();
            assert!(!runtime.gym(1.0).inside, "{position:?}");
            assert!(!runtime.gym(1.0).near);
        }
        runtime
            .set_spawn(Vec3::new(54.0, 0.0, 0.0), std::f32::consts::FRAC_PI_2)
            .unwrap();
        assert!(runtime.gym(1.0).inside && runtime.gym(1.0).near);
        assert!(!runtime.gym(0.0).visible);
        assert!(!runtime.gym(f32::NAN).visible);
    }

    #[test]
    fn the_computer_is_visible_then_reachable_without_walking_through_it() {
        let mut runtime = WorldRuntime::new();
        let initial = runtime.computer(2.0 / 3.0);
        assert!(!initial.near);
        assert!(initial.visible);
        assert_eq!(initial.distance, 5.0);
        assert!((initial.screen_x - 0.5).abs() < 0.001);
        for _ in 0..120 {
            runtime.tick(
                &InputState {
                    forward: true,
                    ..InputState::default()
                },
                1.0 / 60.0,
            );
        }
        let reached = runtime.computer(2.0 / 3.0);
        assert!(reached.near && reached.visible);
        assert!(runtime.player.pos.z <= world::COMPUTER.z - 0.8 - crate::controller::RADIUS);
        assert!((0.0..=1.0).contains(&reached.screen_y));
        runtime
            .set_spawn(world::SPAWN, std::f32::consts::PI)
            .unwrap();
        runtime.camera.distance = crate::camera::MIN_DISTANCE;
        assert!(!runtime.computer(2.0 / 3.0).visible);
        for aspect in [0.0, f32::NAN, f32::INFINITY] {
            let hidden = runtime.computer(aspect);
            assert!(!hidden.visible);
            assert!(hidden.screen_x.is_finite() && hidden.screen_y.is_finite());
        }
    }

    #[test]
    fn computer_action_prompt_requires_an_interactive_host() {
        let mut runtime = WorldRuntime::new();
        let neutral = runtime.dynamic_mesh();
        let distant = runtime.dynamic_mesh_with_computer_interaction();
        assert!(distant.faces.len() > neutral.faces.len());
        runtime.set_spawn(Vec3::new(0.0, 0.0, -7.0), 0.0).unwrap();
        let neutral = runtime.dynamic_mesh();
        let nearby = runtime.dynamic_mesh_with_computer_interaction();
        assert!(nearby.faces.len() > neutral.faces.len());
        assert_ne!(distant.faces.len(), nearby.faces.len());
    }

    #[test]
    fn computer_picking_tracks_the_monitor_in_portrait_landscape_and_oblique_views() {
        let mut runtime = WorldRuntime::new();
        runtime.set_spawn(Vec3::new(0.0, 0.0, -7.0), 0.0).unwrap();
        for aspect in [0.46, 2.0 / 3.0, 1.0, 2.2] {
            for orbit in [-0.35, 0.0, 0.35] {
                runtime.camera.yaw_offset = orbit;
                let screen = runtime.computer(aspect);
                assert!(screen.visible && screen.near);
                assert!(
                    runtime.computer_hit(aspect, screen.screen_x, screen.screen_y),
                    "aspect {aspect}, orbit {orbit}"
                );
                // A point just outside the real monitor does not become a
                // hittable axis-aligned native button when the camera turns.
                let outside = world::COMPUTER_SCREEN + Vec3::new(1.4, 0.0, 0.0);
                let clip = runtime.view(aspect).view_proj * outside.extend(1.0);
                assert!(!runtime.computer_hit(
                    aspect,
                    clip.x / clip.w * 0.5 + 0.5,
                    0.5 - clip.y / clip.w * 0.5
                ));
            }
        }
    }

    #[test]
    fn computer_picking_respects_presented_remote_geometry_without_advancing_it() {
        let mut runtime = WorldRuntime::new();
        runtime.set_spawn(Vec3::new(0.0, 0.0, -7.0), 0.0).unwrap();
        let screen = runtime.computer(1.0);
        let test = |entities: &Mesh| {
            runtime.computer_hit_with_entities(1.0, screen.screen_x, screen.screen_y, entities)
        };
        assert!(test(&Mesh::default()));
        // A remote entity's solid geometry hides the monitor even though
        // neither its collider nor its identity belongs to this runtime.
        let mut remote = Mesh::default();
        remote.cube(
            glam::Mat4::from_translation(Vec3::new(0.0, 3.0, -6.0)),
            coder_ui::theme::Intensity::Full,
        );
        let before = remote.faces.clone();
        assert!(!test(&remote));
        assert_eq!(before, remote.faces);
        // Geometry beyond the display does not hide or disable it.
        for vertex in &mut remote.faces {
            vertex.pos[2] += 3.0;
        }
        assert!(test(&remote));
    }

    #[test]
    fn computer_picking_rejects_distance_back_faces_invalid_input_and_occlusion() {
        let mut runtime = WorldRuntime::new();
        let screen = runtime.computer(1.0);
        assert!(!runtime.computer_hit(1.0, screen.screen_x, screen.screen_y));
        runtime.set_spawn(Vec3::new(0.0, 0.0, -7.0), 0.0).unwrap();
        let screen = runtime.computer(1.0);
        for aspect in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(!runtime.computer_hit(aspect, screen.screen_x, screen.screen_y));
        }
        for (x, y) in [
            (f32::NAN, 0.5),
            (0.5, f32::INFINITY),
            (-0.1, 0.5),
            (0.5, 1.1),
        ] {
            assert!(!runtime.computer_hit(1.0, x, y));
        }
        // Insert an actual solid face in the line of sight, rather than a
        // footprint-only blocker that would not hide a rendered display.
        runtime.world.mesh.quad([
            Vec3::new(-3.0, 0.0, -6.0),
            Vec3::new(3.0, 0.0, -6.0),
            Vec3::new(3.0, 6.0, -6.0),
            Vec3::new(-3.0, 6.0, -6.0),
        ]);
        assert!(!runtime.computer_hit(1.0, screen.screen_x, screen.screen_y));
        let mut runtime = WorldRuntime::new();
        runtime
            .set_spawn(Vec3::new(0.0, 0.0, -3.0), std::f32::consts::PI)
            .unwrap();
        let screen = runtime.computer(1.0);
        assert!(screen.near && screen.visible);
        assert!(!runtime.computer_hit(1.0, screen.screen_x, screen.screen_y));
        runtime
            .set_spawn(Vec3::new(0.0, 0.0, -7.0), std::f32::consts::PI)
            .unwrap();
        runtime.camera.distance = crate::camera::MIN_DISTANCE;
        assert!(!runtime.computer_hit(1.0, 0.5, 0.5));
    }

    #[test]
    fn semantic_input_advances_the_same_world_deterministically() {
        let mut desktop = WorldRuntime::new();
        let mut native = WorldRuntime::new();
        for world in [&mut desktop, &mut native] {
            world.apply(Action::Orbit { dx: 25.0, dy: 10.0 }).unwrap();
            world.apply(Action::FaceCamera).unwrap();
            for _ in 0..120 {
                world.tick(
                    &InputState {
                        forward: true,
                        sprint: true,
                        ..InputState::default()
                    },
                    1.0 / 60.0,
                );
            }
        }
        assert_eq!(desktop.player, native.player);
        assert_eq!(desktop.camera, native.camera);
        assert_eq!(desktop.agent.pos, native.agent.pos);
        assert!(desktop.player.pos.distance(world::SPAWN) > 1.0);
        assert_eq!(desktop.dynamic_mesh().lines, native.dynamic_mesh().lines);
    }

    #[test]
    fn bad_input_and_resume_gaps_cannot_poison_the_simulation() {
        let mut world = WorldRuntime::new();
        let before = world.player;
        assert_eq!(world.tick(&InputState::default(), f32::NAN), 0.0);
        assert_eq!(world.player, before);
        let camera = world.camera;
        assert!(
            world
                .apply(Action::Look {
                    dx: f32::INFINITY,
                    dy: 0.0
                })
                .is_err()
        );
        assert!(world.apply(Action::Zoom { lines: f32::NAN }).is_err());
        assert_eq!(world.camera, camera);
        assert_eq!(
            world.tick(
                &InputState {
                    forward: true,
                    ..InputState::default()
                },
                3600.0
            ),
            MAX_FRAME_SECONDS
        );
        assert!(world.player.pos.distance(before.pos) < 1.0);
        assert!(world.view(f32::NAN).view_proj.is_finite());
    }

    #[test]
    fn external_agent_motion_is_not_advanced_as_a_follower() {
        let mut world = WorldRuntime::new();
        let position = world.agent.pos;
        world.tick_with_mode(
            &InputState {
                forward: true,
                ..InputState::default()
            },
            0.1,
            false,
            false,
        );
        assert_eq!(world.agent.pos, position);
        assert_ne!(world.player.pos, world::SPAWN);
    }
}
