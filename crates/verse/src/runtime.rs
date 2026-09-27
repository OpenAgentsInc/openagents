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
use crate::nav::{self, NavError, Navigation, NavigationStatus};
use crate::render::View;
use crate::world::{self, World};

/// Long pauses never become a large physics step.
pub const MAX_FRAME_SECONDS: f32 = rust_native::surface::MAX_FRAME_DELTA;
/// Maximum distance from the player's shoulder to a tappable companion.
pub const COMPANION_RANGE: f32 = 4.0;

/// The local companion's projected anchor and bounded reaction state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Companion {
    pub near: bool,
    /// Projection visibility only; picking also checks foreground geometry.
    pub visible: bool,
    pub screen_x: f32,
    pub screen_y: f32,
    /// Distance from the player's shoulder, in meters.
    pub distance: f32,
    pub reacting: bool,
    pub cooldown_seconds: f32,
}

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
    /// Incremental finger separation: a scale above one moves the camera closer.
    PinchZoom { scale: f32 },
}

/// State shared by every Verse surface. Services remain separate owners.
pub struct WorldRuntime {
    pub world: World,
    pub player: PlayerController,
    pub camera: FollowCamera,
    pub gait: Gait,
    pub agent: Agent,
    pub(crate) navigation: Navigation,
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
            navigation: Navigation::default(),
        }
    }

    /// Start ordinary walking to an exact clear ground position.
    /// A refused destination stops any earlier route.
    pub fn navigate_to(&mut self, destination: [f32; 2]) -> Result<(), NavError> {
        match nav::plan(
            [self.player.pos.x, self.player.pos.z],
            destination,
            &self.world.blockers,
            world::HALF,
        ) {
            Ok(route) => {
                self.navigation.start(route);
                Ok(())
            }
            Err(error) => {
                self.navigation = Navigation::default();
                self.navigation.stop(NavigationStatus::Blocked);
                Err(error)
            }
        }
    }

    /// Stop automatic walking without moving the player or changing the view.
    pub fn cancel_navigation(&mut self) {
        if self.navigation.is_active() {
            self.navigation.stop(NavigationStatus::Cancelled);
        }
    }

    #[must_use]
    pub fn navigation(&self) -> &Navigation {
        &self.navigation
    }

    /// Applies bounded, finite camera input. Invalid input changes no state.
    pub fn apply(&mut self, action: Action) -> Result<(), String> {
        let motion = |dx: f32, dy: f32| {
            dx.is_finite() && dy.is_finite() && dx.abs() <= 4096.0 && dy.abs() <= 4096.0
        };
        match action {
            Action::Orbit { dx, dy } if motion(dx, dy) => self.camera.orbit(dx, dy),
            Action::Look { dx, dy } if motion(dx, dy) => {
                if self.navigation.is_active() {
                    self.camera.orbit(dx, dy);
                } else {
                    self.player.yaw = wrap(self.player.yaw + self.camera.mouselook(dx, dy));
                }
            }
            Action::FaceCamera => {
                if !self.navigation.is_active() {
                    self.player.yaw = wrap(self.player.yaw + self.camera.take_offset());
                }
            }
            Action::Zoom { lines } if lines.is_finite() && lines.abs() <= 100.0 => {
                self.camera.zoom(lines);
            }
            Action::PinchZoom { scale } if scale.is_finite() && (0.1..=10.0).contains(&scale) => {
                self.camera.pinch(scale);
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
        if input.forward
            || input.backward
            || input.left
            || input.right
            || input.strafe_left
            || input.strafe_right
            || input.jump
        {
            self.cancel_navigation();
        }
        if self.navigation.is_active() {
            self.walk_route(dt);
        } else {
            self.player
                .update(input, dt, &self.world.blockers, world::HALF);
        }
        if self.player.speed > 0.1
            && !orbiting
            && !(self.navigation.is_active() && input.mouse_look)
        {
            self.camera.settle(dt);
        }
        self.gait
            .advance(self.player.speed, self.player.airborne(), dt);
        if follow_agent {
            self.agent.update(&self.player, dt);
        }
        dt
    }

    fn walk_route(&mut self, dt: f32) {
        let at = [self.player.pos.x, self.player.pos.z];
        while self
            .navigation
            .waypoints()
            .first()
            .is_some_and(|next| (next[0] - at[0]).hypot(next[1] - at[1]) <= 0.05)
        {
            self.navigation.next += 1;
        }
        let Some(&next) = self.navigation.waypoints().first() else {
            self.navigation.stop(NavigationStatus::Arrived);
            self.player.update(
                &InputState::default(),
                dt,
                &self.world.blockers,
                world::HALF,
            );
            return;
        };
        if !nav::segment_clear(at, next, &self.world.blockers, world::HALF) {
            self.navigation.stop(NavigationStatus::Blocked);
            self.player.update(
                &InputState::default(),
                dt,
                &self.world.blockers,
                world::HALF,
            );
            return;
        }
        let dx = next[0] - at[0];
        let dz = next[1] - at[1];
        let distance = dx.hypot(dz);
        let view_yaw = self.player.yaw + self.camera.yaw_offset;
        self.player.yaw = dx.atan2(dz);
        self.camera.yaw_offset = wrap(view_yaw - self.player.yaw);
        let step = dt.min(distance / crate::controller::RUN_SPEED);
        self.player.update(
            &InputState {
                forward: true,
                mouse_look: true,
                ..InputState::default()
            },
            step,
            &self.world.blockers,
            world::HALF,
        );
        if step < dt {
            self.player.update(
                &InputState::default(),
                dt - step,
                &self.world.blockers,
                world::HALF,
            );
        }
        let progress = (self.player.pos.x - at[0]).hypot(self.player.pos.z - at[1]);
        self.navigation.stalled = if progress < 0.0001 {
            self.navigation.stalled + dt
        } else {
            0.0
        };
        if self.navigation.stalled >= 1.0 {
            self.navigation.stop(NavigationStatus::Blocked);
        } else if (self.player.pos.x - next[0]).hypot(self.player.pos.z - next[1]) <= 0.05 {
            self.navigation.next += 1;
            if self.navigation.waypoints().is_empty() {
                self.navigation.stop(NavigationStatus::Arrived);
            }
        }
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

    /// Project the animated spade center into normalized viewport coordinates.
    #[must_use]
    pub fn companion(&self, aspect: f32) -> Companion {
        let center = self.agent.transform().transform_point3(Vec3::ZERO);
        let shoulder = self.player.pos + Vec3::Y * crate::agent::HOVER;
        let distance = center.distance(shoulder);
        let clip = self.view(aspect).view_proj * center.extend(1.0);
        let mut result = Companion {
            near: distance.is_finite() && distance <= COMPANION_RANGE,
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance: if distance.is_finite() {
                distance
            } else {
                f32::MAX
            },
            reacting: self.agent.emote() == Some(crate::agent::Emote::Wiggle),
            cooldown_seconds: self.agent.pet_cooldown(),
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

    /// Pick the spade's visible faces, excluding its decorative ground ring.
    #[must_use]
    pub fn companion_hit(&self, aspect: f32, x: f32, y: f32) -> bool {
        self.companion_hit_with_entities(aspect, x, y, &Mesh::default())
    }

    /// Pick against the presented static, local-player, and remote-entity faces.
    /// This reads animation state without advancing it or fetching remote state.
    #[must_use]
    pub fn companion_hit_with_entities(
        &self,
        aspect: f32,
        x: f32,
        y: f32,
        entities: &Mesh,
    ) -> bool {
        if !self.companion(aspect).near {
            return false;
        }
        let view = self.view(aspect);
        let Some((origin, direction)) = viewport_ray(&view, aspect, x, y) else {
            return false;
        };
        let spade = crate::agent::spade(self.agent.transform(), coder_ui::theme::Intensity::Full);
        let Some(distance) = mesh_hit(&spade, origin, direction) else {
            return false;
        };
        let clip = view.view_proj * (origin + direction * distance).extend(1.0);
        if !clip.is_finite() || clip.w <= 0.0 || !(0.0..=1.0).contains(&(clip.z / clip.w)) {
            return false;
        }
        !mesh_occludes(&self.world.mesh, origin, direction, distance)
            && !mesh_occludes(
                &avatar::mesh(&self.player, &self.gait),
                origin,
                direction,
                distance,
            )
            && !mesh_occludes(&world::computer_display(None), origin, direction, distance)
            && !mesh_occludes(entities, origin, direction, distance)
    }

    /// React after a host admits a visible tap or accessible action.
    /// The host owns gesture, visibility, replay, and surface-lifecycle admission.
    pub fn pet_companion(&mut self) -> bool {
        self.companion(1.0).near && self.agent.pet()
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
        let Some((_, direction)) = viewport_ray(&view, aspect, x, y) else {
            return false;
        };
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
        self.cancel_navigation();
        self.player = PlayerController::new(position, wrap(yaw));
        self.agent = Agent::new(&self.player);
        self.gait = Gait::default();
        Ok(())
    }
}

/// Unproject one finite normalized viewport point using the presented camera.
pub(crate) fn viewport_ray(view: &View, aspect: f32, x: f32, y: f32) -> Option<(Vec3, Vec3)> {
    if !aspect.is_finite()
        || aspect <= 0.0
        || !(0.0..=1.0).contains(&x)
        || !(0.0..=1.0).contains(&y)
        || !view.eye.is_finite()
    {
        return None;
    }
    let far = view.view_proj.inverse() * glam::Vec4::new(x * 2.0 - 1.0, 1.0 - y * 2.0, 1.0, 1.0);
    if !far.is_finite() || far.w.abs() < f32::EPSILON {
        return None;
    }
    let direction = (far.truncate() / far.w - view.eye).normalize_or_zero();
    (direction.is_finite() && direction.length_squared() > 0.5).then_some((view.eye, direction))
}

pub(crate) fn mesh_occludes(mesh: &Mesh, origin: Vec3, direction: Vec3, distance: f32) -> bool {
    // Ignore the destination plane itself, including its amber display strokes.
    mesh_hit(mesh, origin, direction).is_some_and(|hit| hit < distance - 0.003)
}

/// Nearest double-sided face intersection, matching the renderer's solid faces.
pub(crate) fn mesh_hit(mesh: &Mesh, origin: Vec3, direction: Vec3) -> Option<f32> {
    if !origin.is_finite() || !direction.is_finite() {
        return None;
    }
    mesh.faces
        .chunks_exact(3)
        .filter_map(|vertices| {
            let a = Vec3::from(vertices[0].pos);
            let edge_ab = Vec3::from(vertices[1].pos) - a;
            let edge_ac = Vec3::from(vertices[2].pos) - a;
            let cross = direction.cross(edge_ac);
            let determinant = edge_ab.dot(cross);
            if determinant.abs() < 1e-7 {
                return None;
            }
            let inverse = determinant.recip();
            let from_a = origin - a;
            let u = from_a.dot(cross) * inverse;
            if !(-1e-6..=1.0 + 1e-6).contains(&u) {
                return None;
            }
            let cross = from_a.cross(edge_ab);
            let v = direction.dot(cross) * inverse;
            if v < -1e-6 || u + v > 1.0 + 1e-6 {
                return None;
            }
            let hit = edge_ac.dot(cross) * inverse;
            (hit.is_finite() && hit > 0.001).then_some(hit)
        })
        .min_by(f32::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projected(runtime: &WorldRuntime, aspect: f32, at: Vec3) -> [f32; 2] {
        let clip = runtime.view(aspect).view_proj * at.extend(1.0);
        [clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5]
    }

    #[test]
    fn companion_picking_uses_the_spade_in_portrait_landscape_and_animated_views() {
        let mut runtime = WorldRuntime::new();
        for aspect in [0.46, 2.0 / 3.0, 1.0, 2.2] {
            for orbit in [-0.3, 0.0, 0.3] {
                runtime.camera.yaw_offset = orbit;
                let companion = runtime.companion(aspect);
                assert!(companion.visible && companion.near);
                assert!(
                    runtime.companion_hit(aspect, companion.screen_x, companion.screen_y),
                    "aspect {aspect}, orbit {orbit}"
                );
                let outside = runtime
                    .agent
                    .transform()
                    .transform_point3(Vec3::new(0.5, 0.3, 0.0));
                let [x, y] = projected(&runtime, aspect, outside);
                assert!(
                    !runtime.companion_hit(aspect, x, y),
                    "no rectangular button outside the silhouette"
                );
            }
        }
        runtime.camera.yaw_offset = 0.0;
        let ring = Vec3::new(runtime.agent.pos.x + 0.28, 0.02, runtime.agent.pos.z);
        let [x, y] = projected(&runtime, 1.0, ring);
        assert!(!runtime.companion_hit(1.0, x, y));
        assert!(runtime.pet_companion());
        for _ in 0..54 {
            runtime.tick(&InputState::default(), 1.0 / 60.0);
            let companion = runtime.companion(1.0);
            assert!(runtime.companion_hit(1.0, companion.screen_x, companion.screen_y));
        }
        assert_eq!(runtime.agent.pet_count(), 1);
    }

    #[test]
    fn companion_picking_refuses_foreground_static_player_and_remote_faces() {
        let mut runtime = WorldRuntime::new();
        let companion = runtime.companion(1.0);
        let view = runtime.view(1.0);
        let (eye, direction) =
            viewport_ray(&view, 1.0, companion.screen_x, companion.screen_y).unwrap();
        let center = runtime.agent.transform().transform_point3(Vec3::ZERO);
        let distance = center.distance(eye);
        let horizontal = direction.cross(Vec3::Y).normalize();
        let vertical = horizontal.cross(direction).normalize();
        let wall = |distance| {
            let at = eye + direction * distance;
            let mut mesh = Mesh::default();
            mesh.quad([
                at - horizontal - vertical,
                at + horizontal - vertical,
                at + horizontal + vertical,
                at - horizontal + vertical,
            ]);
            mesh
        };
        let foreground = wall(distance - 1.0);
        assert!(!runtime.companion_hit_with_entities(
            1.0,
            companion.screen_x,
            companion.screen_y,
            &foreground
        ));
        assert!(runtime.companion_hit_with_entities(
            1.0,
            companion.screen_x,
            companion.screen_y,
            &wall(distance + 1.0)
        ));
        let old_mesh = runtime.world.mesh.clone();
        runtime.world.mesh.extend(&foreground);
        assert!(!runtime.companion_hit(1.0, companion.screen_x, companion.screen_y));
        runtime.world.mesh = old_mesh;

        let torso = runtime.player.pos + Vec3::Y;
        let behind_player = torso + (torso - eye).normalize() * 0.8;
        runtime.agent = Agent::at(behind_player, 0.0);
        let companion = runtime.companion(1.0);
        assert!(companion.near && companion.visible);
        let (eye, direction) =
            viewport_ray(&view, 1.0, companion.screen_x, companion.screen_y).unwrap();
        assert!(
            mesh_hit(
                &avatar::mesh(&runtime.player, &runtime.gait),
                eye,
                direction
            )
            .is_some()
        );
        assert!(!runtime.companion_hit(1.0, companion.screen_x, companion.screen_y));
    }

    #[test]
    fn companion_refuses_invalid_projection_input_and_distant_reactions() {
        let mut runtime = WorldRuntime::new();
        for aspect in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let companion = runtime.companion(aspect);
            assert!(!companion.visible);
            assert!(companion.screen_x.is_finite() && companion.screen_y.is_finite());
            assert!(!runtime.companion_hit(aspect, companion.screen_x, companion.screen_y));
        }
        for (x, y) in [
            (f32::NAN, 0.5),
            (0.5, f32::INFINITY),
            (-0.1, 0.5),
            (0.5, 1.1),
        ] {
            assert!(!runtime.companion_hit(1.0, x, y));
        }
        for position in [runtime.player.pos + Vec3::new(6.0, 2.2, 0.0), Vec3::NAN] {
            runtime.agent = Agent::at(position, 0.0);
            let companion = runtime.companion(1.0);
            assert!(!companion.near);
            assert!(companion.distance.is_finite());
            assert!(!runtime.pet_companion());
            assert!(!runtime.companion_hit(1.0, companion.screen_x, companion.screen_y));
            assert_eq!(runtime.agent.pet_count(), 0);
        }
    }

    #[test]
    fn navigation_walks_around_a_building_without_teleporting_or_entering_it() {
        let mut runtime = WorldRuntime::new();
        runtime.world.blockers = vec![crate::controller::Footprint {
            min: [-2.0, -2.0],
            max: [2.0, 2.0],
        }];
        runtime.set_spawn(Vec3::new(-8.0, 0.0, 0.0), 0.0).unwrap();
        runtime.navigate_to([8.0, 0.0]).unwrap();
        let route = runtime.navigation().waypoints().to_vec();
        assert!(route.len() >= 2);
        for _ in 0..2000 {
            let previous = runtime.player.pos;
            runtime.tick(&InputState::default(), 1.0 / 60.0);
            assert!(
                runtime.player.pos.distance(previous)
                    <= crate::controller::RUN_SPEED / 60.0 + 0.001
            );
            assert!(!runtime.world.blockers[0].contains(
                runtime.player.pos.x,
                runtime.player.pos.z,
                crate::controller::RADIUS
            ));
            if !runtime.navigation().is_active() {
                break;
            }
        }
        assert_eq!(runtime.navigation().status(), NavigationStatus::Arrived);
        assert!(runtime.player.pos.distance(Vec3::new(8.0, 0.0, 0.0)) <= 0.05);
        assert_eq!(runtime.navigation().destination(), Some([8.0, 0.0]));
    }

    #[test]
    fn navigation_uses_the_seeded_gym_door_and_camera_control_does_not_cancel_it() {
        let mut runtime = WorldRuntime::new();
        runtime.navigate_to([48.0, 0.0]).unwrap();
        let camera = runtime.camera;
        runtime
            .apply(Action::Look {
                dx: 30.0,
                dy: -20.0,
            })
            .unwrap();
        assert_ne!(runtime.camera, camera);
        assert!(runtime.navigation().is_active());
        for _ in 0..3000 {
            runtime.tick(
                &InputState {
                    mouse_look: true,
                    ..InputState::default()
                },
                1.0 / 60.0,
            );
            assert!(!runtime.world.blockers.iter().any(|b| b.contains(
                runtime.player.pos.x,
                runtime.player.pos.z,
                crate::controller::RADIUS
            )));
            if !runtime.navigation().is_active() {
                break;
            }
        }
        assert_eq!(runtime.navigation().status(), NavigationStatus::Arrived);
        assert!(runtime.gym(1.0).inside);
    }

    #[test]
    fn manual_movement_jump_and_explicit_cancel_stop_routes() {
        for input in [
            InputState {
                forward: true,
                ..InputState::default()
            },
            InputState {
                left: true,
                ..InputState::default()
            },
            InputState {
                strafe_right: true,
                ..InputState::default()
            },
            InputState {
                jump: true,
                ..InputState::default()
            },
        ] {
            let mut runtime = WorldRuntime::new();
            runtime.navigate_to([8.0, -10.0]).unwrap();
            runtime.tick(&input, 0.02);
            assert_eq!(runtime.navigation().status(), NavigationStatus::Cancelled);
        }
        let mut runtime = WorldRuntime::new();
        runtime.navigate_to([8.0, -10.0]).unwrap();
        let position = runtime.player.pos;
        runtime.cancel_navigation();
        runtime.tick(&InputState::default(), 0.05);
        assert_eq!(runtime.player.pos, position);
        assert_eq!(runtime.navigation().status(), NavigationStatus::Cancelled);
    }

    #[test]
    fn changed_obstacles_stop_navigation_before_the_player_crosses_them() {
        let mut runtime = WorldRuntime::new();
        runtime.world.blockers.clear();
        runtime.navigate_to([10.0, -10.0]).unwrap();
        runtime.world.blockers.push(crate::controller::Footprint {
            min: [2.0, -12.0],
            max: [3.0, -8.0],
        });
        let position = runtime.player.pos;
        runtime.tick(&InputState::default(), 0.05);
        assert_eq!(runtime.navigation().status(), NavigationStatus::Blocked);
        assert_eq!(runtime.player.pos, position);
        assert!(runtime.navigate_to([f32::NAN, 0.0]).is_err());
        assert!(!runtime.navigation().is_active());
    }

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
