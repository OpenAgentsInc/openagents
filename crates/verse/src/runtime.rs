//! The shared Verse world simulation. Platform adapters supply intent and time.
//!
//! Desktop and native surfaces advance the same player, camera, gait, and
//! follower. Explicit zone entry delegates asset I/O to a bounded background
//! loader; ordinary simulation starts no network or model calls.

use glam::Vec3;

use crate::agent::Agent;
use crate::avatar::{self, Gait};
use crate::camera::FollowCamera;
use crate::controller::{InputState, PlayerController, wrap};
use crate::doors::{self, DemoItem, DoorId, Doors, TapResult};
use crate::mesh::Mesh;
use crate::nav::{self, NavError, Navigation, NavigationStatus};
use crate::render::View;
use crate::world::{self, World};

/// Long pauses never become a large physics step.
pub const MAX_FRAME_SECONDS: f32 = rust_native::surface::MAX_FRAME_DELTA;
/// Maximum distance from the player's shoulder to a tappable companion.
pub const COMPANION_RANGE: f32 = 4.0;

/// A local demo door's anchor. Projection visibility does not imply admission.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct DoorProjection {
    pub id: DoorId,
    pub near: bool,
    pub visible: bool,
    pub screen_x: f32,
    pub screen_y: f32,
    pub distance: f32,
}

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
pub use verse_core::zone::InteractHint;

pub struct WorldRuntime {
    #[cfg(feature = "hosted-social")]
    pub(crate) hosted: Option<crate::hosted::Projection>,
    pub world: World,
    pub player: PlayerController,
    pub camera: FollowCamera,
    pub gait: Gait,
    pub agent: Agent,
    pub doors: Doors,
    pub zone: crate::zones::ZoneId,
    pub zone_revision: u64,
    /// Boxed so the creation path's frames stay small: the zone states sum
    /// to tens of kilobytes, and a by-value chain of them overflowed a
    /// phone's main-thread stack mounting the world (#10928).
    pub(crate) zone_state: Box<crate::zones::State>,
    pub(crate) navigation: Navigation,
    /// The bare world: the plaza grid alone, with no objects, companion,
    /// portals, or interactions, drawn in the neutral palette.
    bare: bool,
    /// How a station's panel opens on this device, which the zone's caption
    /// says; see [`InteractHint`].
    pub interact_hint: InteractHint,
    /// The bare world watched from above with nobody playing here
    /// ([`Self::unoccupied`]): no local avatar is drawn and the player never
    /// moves or touches a body.
    unoccupied: bool,
    /// The bare world's ball, which the player pushes.
    pub(crate) ball: Option<Box<crate::ball::Ball>>,
    /// Other players' avatars where they are drawn, feet positions. The
    /// player cannot walk through them.
    avatars: Vec<Vec3>,
    /// A published trace's ghost in the Grid's Gym, where the results
    /// panel's replay puts it; drawn only in the bare world.
    pub trace_ghost: Option<Vec3>,
    /// Seconds the Grid robots have patrolled and danced
    /// ([`crate::grid_robot::patroller`]).
    robot_clock: f64,
    /// A director's camera in place of the follow camera, for a capture:
    /// its eye and the point it looks at ([`Self::set_shot`]).
    shot: Option<(Vec3, Vec3)>,
    /// The bare world's Gym stands on the Grid; [`Self::remove_gym`] takes
    /// it, its boards, and its walls away.
    gym: bool,
}

impl Default for WorldRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldRuntime {
    /// The shared construction behind [`Self::new`], [`Self::bare`], and
    /// [`Self::unoccupied`]. `#[inline(never)]` keeps this frame — the
    /// largest on the creation path — out of its callers (#10928).
    #[inline(never)]
    fn with_world(world: World, bare: bool, unoccupied: bool, interact_hint: InteractHint) -> Self {
        let player = PlayerController::new(world::SPAWN, 0.0);
        let agent = Agent::new(&player);
        Self {
            #[cfg(feature = "hosted-social")]
            hosted: None,
            world,
            player,
            camera: FollowCamera::default(),
            gait: Gait::default(),
            agent,
            doors: Doors::default(),
            zone: crate::zones::ZoneId::Plaza,
            zone_revision: 0,
            zone_state: Box::new(crate::zones::State::default()),
            navigation: Navigation::default(),
            bare,
            interact_hint,
            unoccupied,
            ball: None,
            avatars: Vec::new(),
            trace_ghost: None,
            robot_clock: 0.0,
            shot: None,
            gym: true,
        }
    }

    /// Takes the Gym off the bare world's Grid: its hall, boards, and
    /// walls are gone, and the world has no Gym site, so no board opens or
    /// loads. The plain Grid of the OpenAgents app's normal builds.
    pub fn remove_gym(&mut self) {
        if !self.bare {
            return;
        }
        self.gym = false;
        if self.is_plaza() {
            self.world = world::bare_ground();
        }
    }

    /// Whether this world has its Gym ([`Self::remove_gym`]).
    #[must_use]
    pub fn has_gym(&self) -> bool {
        self.gym
    }

    #[must_use]
    #[inline(never)]
    pub fn new() -> Self {
        Self::with_world(world::build(), false, false, InteractHint::Key)
    }

    pub fn is_hosted(&self) -> bool {
        #[cfg(feature = "hosted-social")]
        {
            self.hosted.is_some()
        }
        #[cfg(not(feature = "hosted-social"))]
        {
            false
        }
    }

    /// The bare world: the plaza's ground grid in the neutral palette, with
    /// the same player, controller, and camera, the shared ball and blocks,
    /// and the Gym at [`world::GymSite::GRID`]. It has no computer, doors,
    /// companion, or tapped portals.
    #[must_use]
    #[inline(never)]
    pub fn bare() -> Self {
        let mut world = world::bare();
        // The Grid is the pinned engine pack: what the player cannot walk
        // through comes from its placements, not from the line mesh.
        if let Ok(pack) = crate::grid_pack::embedded() {
            world.blockers = crate::grid_pack::blockers(&pack);
        }
        // The OpenAgents app's Grid opens no studio panel. The ball, the
        // blocks (cubes and dominoes), and the pedestal are off for now
        // (owner, 2026-10-01): the Grid keeps only the Gym.
        Self::with_world(world, true, false, InteractHint::None)
    }

    /// The bare world with nobody playing in it here: a spectator's view of
    /// the Grid. Other players, the ball, the blocks, and the Gym are drawn,
    /// but no local avatar; advance it with [`Self::tick_unoccupied`], never
    /// with player input.
    #[must_use]
    pub fn unoccupied() -> Self {
        let mut runtime = Self::bare();
        runtime.unoccupied = true;
        runtime
    }

    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.bare
    }

    /// Whether nobody plays in this world here ([`Self::unoccupied`]).
    #[must_use]
    pub fn is_unoccupied(&self) -> bool {
        self.unoccupied
    }

    /// Advances the shared ball and blocks by `dt` seconds with nobody
    /// playing here: no player moves, walks through a portal, or touches a
    /// body. Returns the applied dt.
    pub fn tick_unoccupied(&mut self, dt: f32) -> f32 {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, MAX_FRAME_SECONDS)
        } else {
            0.0
        };
        if dt > 0.0
            && let Some(ball) = &mut self.ball
        {
            ball.advance_unoccupied(dt);
        }
        self.robot_clock += f64::from(dt);
        dt
    }

    /// The Grid robots in the bare world's plaza: the four patrollers in
    /// route order, then the dancer by the Everglade arch. None elsewhere.
    #[must_use]
    pub fn robots(&self) -> Vec<crate::grid_robot::Pose> {
        if !(self.bare && self.is_plaza()) {
            return Vec::new();
        }
        (0..crate::grid_robot::ROUTES.len())
            .map(|route| crate::grid_robot::patroller(route, self.robot_clock))
            .chain([crate::grid_robot::dancer(self.robot_clock)])
            .collect()
    }

    /// The bare world's ball; other worlds have none.
    #[must_use]
    pub fn ball(&self) -> Option<&crate::ball::Ball> {
        self.ball.as_deref()
    }

    /// Zooming in past the nearest orbit enters first person on the bare
    /// world's grid, in Everglade, and in the crypt. Coder's plaza and the
    /// other zones keep the third-person orbit.
    #[must_use]
    pub fn first_person_allowed(&self) -> bool {
        (self.bare && self.is_plaza())
            || matches!(
                self.zone,
                crate::zones::ZoneId::Everglade
                    | crate::zones::ZoneId::Crypt
                    | crate::zones::ZoneId::MeteorStressTest
                    | crate::zones::ZoneId::MeteorShowcase
                    | crate::zones::ZoneId::WaterLab
                    | crate::zones::ZoneId::Coast
            )
    }

    /// The camera is at, or gliding to, the player's head.
    #[must_use]
    pub fn first_person(&self) -> bool {
        self.camera.first_person && self.first_person_allowed()
    }

    /// The eye is at or near the player's head, so the local avatar is not
    /// drawn: in first person, or where a wall pulls the camera in close.
    #[must_use]
    pub fn hides_avatar(&self) -> bool {
        (self.camera.hides_avatar() && self.first_person_allowed()) || self.framing().close
    }

    /// The plaza with its objects: not a zone and not the bare world.
    pub(crate) fn furnished_plaza(&self) -> bool {
        self.is_plaza() && !self.bare
    }

    /// The fog and clear color of the current world.
    #[must_use]
    pub fn atmosphere(&self) -> crate::zones::Atmosphere {
        let mut atmosphere = crate::zones::atmosphere(self.zone);
        // Everglade's air follows its town clock.
        if let Some(air) = self.everglade_atmosphere() {
            atmosphere = air;
        }
        if self.bare {
            atmosphere.color = crate::palette::neutral(atmosphere.color);
        }
        if self.bare && self.is_plaza() {
            atmosphere.fog_start = crate::render::BARE_FOG_START;
            atmosphere.fog_end = crate::render::BARE_FOG_END;
        }
        atmosphere
    }

    /// Other players' avatars, by feet position, as the host draws them
    /// this frame. The next ticks keep the player out of each; an empty
    /// list removes them. Only the plaza collides, where players share a
    /// world.
    pub fn set_avatars(&mut self, avatars: impl IntoIterator<Item = Vec3>) {
        self.avatars.clear();
        self.avatars
            .extend(avatars.into_iter().filter(|p| p.is_finite()).take(512));
    }

    /// Start ordinary walking to an exact clear ground position.
    /// A refused destination stops any earlier route.
    pub fn navigate_to(&mut self, destination: [f32; 2]) -> Result<(), NavError> {
        if self.is_hosted() {
            return Err(NavError::WorldBounds);
        }
        self.doors.route_owner = None;
        // In Lagrange 1 a map point becomes an EVA pack autopilot target.
        if let Some(result) = self.lagrange_fly_to(destination) {
            self.navigation = Navigation::default();
            return result.map_err(|_| NavError::WorldBounds);
        }
        match nav::plan(
            [self.player.pos.x, self.player.pos.z],
            destination,
            &self.world.blockers,
            self.zone_half(),
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
        self.doors.route_owner = None;
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
                let allow = self.first_person_allowed();
                self.camera.zoom_by(0.88f32.powf(-lines), allow);
            }
            Action::PinchZoom { scale } if scale.is_finite() && (0.1..=10.0).contains(&scale) => {
                let allow = self.first_person_allowed();
                self.camera.zoom_by(scale, allow);
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
        #[cfg(feature = "hosted-social")]
        if self.is_hosted() {
            self.restore_hosted_player();
            return dt;
        }
        let previous = self.player;
        if self.camera.first_person && !self.first_person_allowed() {
            self.camera.leave_first_person();
        }
        self.camera.advance(dt);
        self.doors.tick(dt);
        let moving = input.forward
            || input.backward
            || input.left
            || input.right
            || input.strafe_left
            || input.strafe_right
            || input.jump;
        if moving {
            self.cancel_navigation();
            // The player walks where the camera looks: after an orbit (a
            // left drag), a key that moves the character turns the body to
            // the view rather than walking the way it last faced. In first
            // person any movement does; turning keys alone keep turning.
            let travels =
                input.forward || input.backward || input.strafe_left || input.strafe_right;
            if self.first_person() || travels {
                self.player.yaw = wrap(self.player.yaw + self.camera.take_offset());
            }
        }
        if self.navigation.is_active() {
            self.walk_route(dt);
        } else {
            self.update_player(input, dt);
        }
        if self.is_plaza() && !self.avatars.is_empty() {
            self.player
                .separate(&self.avatars, &self.world.blockers, self.zone_half());
        }
        self.robot_clock += f64::from(dt);
        // Nobody walks through the Grid robots.
        for robot in self.robots() {
            crate::grid_robot::keep_clear(&mut self.player.pos, robot.pos);
        }
        // The ball waits on the Grid while the player visits a zone.
        if self.is_plaza()
            && let Some(ball) = &mut self.ball
        {
            ball.advance(previous.pos, &mut self.player, dt);
        }
        if self.player.speed > 0.1
            && !orbiting
            && !(self.navigation.is_active() && input.mouse_look)
        {
            self.camera.settle(dt);
        }
        self.zone_simulation_tick(dt, previous);
        self.walk_through_portals(previous.pos, dt);
        self.track_camera(dt);
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
            self.update_player(&InputState::default(), dt);
            return;
        };
        if !nav::segment_clear(at, next, &self.world.blockers, self.zone_half()) {
            self.navigation.stop(NavigationStatus::Blocked);
            self.update_player(&InputState::default(), dt);
            return;
        }
        let dx = next[0] - at[0];
        let dz = next[1] - at[1];
        let distance = dx.hypot(dz);
        let view_yaw = self.player.yaw + self.camera.yaw_offset;
        self.player.yaw = dx.atan2(dz);
        self.camera.yaw_offset = wrap(view_yaw - self.player.yaw);
        let step = dt.min(distance / crate::controller::RUN_SPEED);
        self.update_player(
            &InputState {
                forward: true,
                mouse_look: true,
                ..InputState::default()
            },
            step,
        );
        if step < dt {
            self.update_player(&InputState::default(), dt - step);
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

    /// Looks from `eye` at `target` in place of the follow camera, as a
    /// capture's director does, until it is set to `None`. Everything that
    /// faces the camera, such as a glow or a nameplate, faces this eye, and
    /// Meteor Swarm's blasts still shake it.
    pub fn set_shot(&mut self, shot: Option<(Vec3, Vec3)>) {
        self.shot = shot.filter(|(eye, target)| {
            eye.is_finite() && target.is_finite() && eye.distance(*target) > 0.01
        });
    }

    #[must_use]
    pub fn view(&self, aspect: f32) -> View {
        let aspect = if aspect.is_finite() {
            aspect.clamp(0.01, 100.0)
        } else {
            1.0
        };
        if let Some((eye, target)) = self.shot {
            // A blast jolts the eye and, more, where it looks, so the shake
            // reads in a wide shot as well as a close one.
            let shake = match self.zone {
                crate::zones::ZoneId::Everglade
                | crate::zones::ZoneId::MeteorStressTest
                | crate::zones::ZoneId::MeteorShowcase => self.demolition_shake(),
                _ => Vec3::ZERO,
            };
            let eye = eye + shake;
            let ground = if self.zone == crate::zones::ZoneId::Coast {
                crate::zones::coast::ground(eye.x, eye.z)
            } else {
                crate::zones::everglade::land(eye.x, eye.z)
            } + 0.3;
            let eye = Vec3::new(eye.x, eye.y.max(ground), eye.z);
            let look = target + shake * (1.0 + 0.04 * eye.distance(target));
            let view = glam::Mat4::look_at_rh(eye, look, Vec3::Y);
            let proj =
                glam::Mat4::perspective_rh(crate::camera::FOV_Y, aspect, 0.3, crate::camera::FAR);
            return View {
                view_proj: proj * view,
                eye,
            };
        }
        // One camera-collision step for every zone: the eye stops short of
        // the zone's solids (`zones::sight`).
        let mut framing = self.framing();
        // Meteor Swarm's blasts and a Thunderwave shake the camera.
        framing.eye += match self.zone {
            crate::zones::ZoneId::Grove => self.grove_shake(),
            crate::zones::ZoneId::Everglade
            | crate::zones::ZoneId::MeteorStressTest
            | crate::zones::ZoneId::MeteorShowcase => self.demolition_shake(),
            _ => Vec3::ZERO,
        };
        if self.zone.meteor_stage() {
            // Simultaneous impacts must not shake the low camera below terrain.
            framing.eye.y = framing
                .eye
                .y
                .max(crate::zones::everglade::land(framing.eye.x, framing.eye.z) + 0.3);
        }
        View {
            view_proj: self
                .camera
                .view_proj_framed(framing, self.player.yaw, aspect),
            eye: framing.eye,
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
            near: self.furnished_plaza() && distance <= world::COMPUTER_RANGE,
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance,
        };
        if aspect.is_finite() && aspect > 0.0 && clip.is_finite() && clip.w > 0.0 {
            let ndc = clip.truncate() / clip.w;
            result.screen_x = (ndc.x * 0.5 + 0.5).clamp(0.0, 1.0);
            result.screen_y = (0.5 - ndc.y * 0.5).clamp(0.0, 1.0);
            result.visible = self.furnished_plaza()
                && (-1.0..=1.0).contains(&ndc.x)
                && (-1.0..=1.0).contains(&ndc.y)
                && (0.0..=1.0).contains(&ndc.z);
        }
        result
    }

    /// Whether the spade companion follows the player here: in the
    /// furnished plaza and the zones entered from it, except Everglade,
    /// where the player walks as the outfitted character alone.
    #[must_use]
    pub fn companion_present(&self) -> bool {
        !self.bare
            && !matches!(
                self.zone,
                crate::zones::ZoneId::Everglade
                    | crate::zones::ZoneId::Grove
                    | crate::zones::ZoneId::Crypt
                    | crate::zones::ZoneId::MeteorStressTest
                    | crate::zones::ZoneId::MeteorShowcase
                    | crate::zones::ZoneId::WaterLab
                    | crate::zones::ZoneId::Coast
            )
    }

    /// Project the animated spade center into normalized viewport coordinates.
    #[must_use]
    pub fn companion(&self, aspect: f32) -> Companion {
        let center = self.agent.transform().transform_point3(Vec3::ZERO);
        let shoulder = self.player.pos + Vec3::Y * crate::agent::HOVER;
        let distance = center.distance(shoulder);
        let clip = self.view(aspect).view_proj * center.extend(1.0);
        let mut result = Companion {
            near: self.companion_present() && distance.is_finite() && distance <= COMPANION_RANGE,
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
        let mut local = self.zone_dynamic_mesh();
        if self.is_plaza() {
            local.extend(&avatar::mesh(&self.player, &self.gait));
            local.extend(&doors::held_mesh(self.doors.held(), &self.player));
            local.extend(&world::computer_display(None));
            local.extend(&world::gym_display(world::GymSite::PLAZA, None));
        }
        !mesh_occludes(&self.world.mesh, origin, direction, distance)
            && !mesh_occludes(&local, origin, direction, distance)
            && !mesh_occludes(entities, origin, direction, distance)
    }

    /// React after a host admits a visible tap or accessible action.
    /// The host owns gesture, visibility, replay, and surface-lifecycle admission.
    pub fn pet_companion(&mut self) -> bool {
        self.companion(1.0).near && self.agent.pet()
    }

    #[must_use]
    pub fn door(&self, id: DoorId, aspect: f32) -> DoorProjection {
        let offset = self.player.pos - id.position();
        let distance = offset.x.hypot(offset.z);
        // The upper opening stays above the avatar when approaching head-on.
        let anchor = id.plane() + Vec3::Y * 0.8;
        let clip = self.view(aspect).view_proj * anchor.extend(1.0);
        let mut result = DoorProjection {
            id,
            near: self.furnished_plaza()
                && distance.is_finite()
                && distance <= doors::RANGE
                && (0.0..=3.0).contains(&self.player.pos.y),
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance: if distance.is_finite() {
                distance
            } else {
                f32::MAX
            },
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

    #[must_use]
    pub fn nearest_door(&self, aspect: f32) -> Option<DoorId> {
        DoorId::ALL
            .into_iter()
            .map(|id| self.door(id, aspect))
            .filter(|door| {
                door.near
                    && door.visible
                    && self.door_hit(door.id, aspect, door.screen_x, door.screen_y)
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
            .map(|door| door.id)
    }

    #[must_use]
    pub fn door_hit(&self, id: DoorId, aspect: f32, x: f32, y: f32) -> bool {
        self.door_hit_with_entities(id, aspect, x, y, &Mesh::default())
    }

    /// Admit the bounded inner plane from the front, through no foreground faces.
    #[must_use]
    pub fn door_hit_with_entities(
        &self,
        id: DoorId,
        aspect: f32,
        x: f32,
        y: f32,
        entities: &Mesh,
    ) -> bool {
        if !self.door(id, aspect).near {
            return false;
        }
        let view = self.view(aspect);
        let plane = id.plane();
        if view.eye.z >= plane.z {
            return false;
        }
        let Some((eye, direction)) = viewport_ray(&view, aspect, x, y) else {
            return false;
        };
        if direction.z <= f32::EPSILON {
            return false;
        }
        let distance = (plane.z - eye.z) / direction.z;
        let target = eye + direction * distance;
        let offset = target - plane;
        if !target.is_finite()
            || offset.x.abs() > doors::PLANE_HALF[0]
            || offset.y.abs() > doors::PLANE_HALF[1]
        {
            return false;
        }
        let clip = view.view_proj * target.extend(1.0);
        if !clip.is_finite() || clip.w <= 0.0 || !(0.0..=1.0).contains(&(clip.z / clip.w)) {
            return false;
        }
        !mesh_occludes(&self.world.mesh, eye, direction, distance)
            && !mesh_occludes(
                &avatar::mesh(&self.player, &self.gait),
                eye,
                direction,
                distance,
            )
            && !mesh_occludes(&self.agent.mesh(), eye, direction, distance)
            && !mesh_occludes(
                &doors::held_mesh(self.doors.held(), &self.player),
                eye,
                direction,
                distance,
            )
            && !mesh_occludes(&world::computer_display(None), eye, direction, distance)
            && !mesh_occludes(
                &world::gym_display(world::GymSite::PLAZA, None),
                eye,
                direction,
                distance,
            )
            && !mesh_occludes(entities, eye, direction, distance)
    }

    /// Hosts admit visibility and tap intent; this rechecks proximity and runs no service.
    pub fn tap_door(&mut self, id: DoorId) -> Result<bool, String> {
        if !self.door(id, 1.0).near {
            return Err("Walk closer to the door".into());
        }
        match self.doors.tap(id) {
            TapResult::Walk(destination) => match self.navigate_to(destination.point()) {
                Ok(()) => {
                    self.doors.route_owner = Some(id);
                    Ok(true)
                }
                Err(error) => {
                    self.doors.route_failed(id);
                    Err(error.to_string())
                }
            },
            TapResult::Reacted => Ok(true),
            TapResult::Refused | TapResult::Ignored => Ok(false),
        }
    }

    pub fn hold_door_item(&mut self, item: DemoItem) {
        if self.doors.held() != item {
            if self.doors.route_owner.is_some() {
                self.cancel_navigation();
            }
            self.doors.hold(item);
        }
    }
    pub fn reset_door(&mut self, id: DoorId) {
        if self.doors.route_owner == Some(id) {
            self.cancel_navigation();
        }
        self.doors.reset(id);
    }
    /// Cancel transient door input and its walk without clearing remembered choices.
    pub fn cancel_door_interactions(&mut self) {
        if self.doors.route_owner.is_some() {
            self.cancel_navigation();
        }
        self.doors.cancel_transient();
    }
    pub fn restore_door_state(&mut self, document: &str) -> Result<(), String> {
        let mut restored = self.doors.clone();
        restored.restore(document)?;
        self.cancel_door_interactions();
        self.doors = restored;
        Ok(())
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

    /// Where this world's Gym stands: Coder's plaza has it east of the
    /// plaza, the Grid straight ahead of the spawn, and a zone has none.
    #[must_use]
    pub fn gym_site(&self) -> Option<world::GymSite> {
        if !self.is_plaza() || !self.gym {
            None
        } else if self.bare {
            Some(world::GymSite::GRID)
        } else {
            Some(world::GymSite::PLAZA)
        }
    }

    /// Reports occupancy and the central Gym board's normalized projection.
    /// Hosts gate Gym reads on `inside` and their own active-surface state.
    #[must_use]
    pub fn gym(&self, aspect: f32) -> Gym {
        self.board_state(aspect, self.gym_site(), world::GYM_BOARD)
    }

    /// Where the Grid's RESULTS board stands: the Grid's Gym, and nowhere
    /// else. Coder's plaza has none.
    fn results_site(&self) -> Option<world::GymSite> {
        self.gym_site().filter(|_| self.bare)
    }

    /// As [`Self::gym`], for the Grid's RESULTS board beside the central
    /// board. Outside the Grid it is never inside, near, or visible.
    #[must_use]
    pub fn results(&self, aspect: f32) -> Gym {
        self.board_state(aspect, self.results_site(), world::GYM_RESULTS_BOARD)
    }

    /// As [`Self::results`], for the Grid's EVALS board on the other side of
    /// the central board.
    #[must_use]
    pub fn evals(&self, aspect: f32) -> Gym {
        self.board_state(aspect, self.results_site(), world::GYM_EVALS_BOARD)
    }

    fn board_state(&self, aspect: f32, site: Option<world::GymSite>, local: Vec3) -> Gym {
        let position = self.player.pos;
        let inside = site.is_some_and(|site| site.inside(position));
        let board = site.unwrap_or(world::GymSite::PLAZA).point(local);
        let offset = position - board;
        let distance = offset.x.hypot(offset.z);
        let clip = self.view(aspect).view_proj * board.extend(1.0);
        let mut result = Gym {
            inside,
            near: inside && distance <= world::GYM_BOARD_RANGE,
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance,
        };
        if aspect.is_finite() && aspect > 0.0 && clip.is_finite() && clip.w > 0.0 {
            let ndc = clip.truncate() / clip.w;
            result.screen_x = (ndc.x * 0.5 + 0.5).clamp(0.0, 1.0);
            result.screen_y = (0.5 - ndc.y * 0.5).clamp(0.0, 1.0);
            result.visible = site.is_some()
                && (-1.0..=1.0).contains(&ndc.x)
                && (-1.0..=1.0).contains(&ndc.y)
                && (0.0..=1.0).contains(&ndc.z);
        }
        result
    }

    /// Pick the central Gym board's front surface from inside its interaction range.
    #[must_use]
    pub fn gym_hit(&self, aspect: f32, x: f32, y: f32) -> bool {
        self.gym_hit_with_entities(aspect, x, y, &Mesh::default())
    }

    /// Include remote faces from the last presented frame, without advancing them.
    /// Opening a board is separate from granting observation or run authority.
    #[must_use]
    pub fn gym_hit_with_entities(&self, aspect: f32, x: f32, y: f32, entities: &Mesh) -> bool {
        self.gym(aspect).near
            && self.board_hit(
                aspect,
                [x, y],
                self.gym_site(),
                world::GYM_BOARD_SCREEN,
                world::GYM_BOARD_HALF,
                entities,
            )
    }

    /// Pick the Grid's RESULTS board front from inside its reach, with the
    /// same unobstructed-view rule as the central board.
    #[must_use]
    pub fn results_hit_with_entities(&self, aspect: f32, x: f32, y: f32, entities: &Mesh) -> bool {
        self.results(aspect).near
            && self.board_hit(
                aspect,
                [x, y],
                self.results_site(),
                world::GYM_RESULTS_SCREEN,
                world::GYM_RESULTS_HALF,
                entities,
            )
    }

    /// Pick the Grid's EVALS board front from inside its reach, with the
    /// same unobstructed-view rule as the central board.
    #[must_use]
    pub fn evals_hit_with_entities(&self, aspect: f32, x: f32, y: f32, entities: &Mesh) -> bool {
        self.evals(aspect).near
            && self.board_hit(
                aspect,
                [x, y],
                self.results_site(),
                world::GYM_EVALS_SCREEN,
                world::GYM_EVALS_HALF,
                entities,
            )
    }

    fn board_hit(
        &self,
        aspect: f32,
        [x, y]: [f32; 2],
        site: Option<world::GymSite>,
        plane: Vec3,
        half: [f32; 2],
        entities: &Mesh,
    ) -> bool {
        let Some(site) = site else {
            return false;
        };
        let view = self.view(aspect);
        // The boards face the doorway (west in the Gym's frame). Their
        // backs never open a panel.
        if site.local(view.eye).x >= plane.x {
            return false;
        }
        let Some((eye, direction)) = viewport_ray(&view, aspect, x, y) else {
            return false;
        };
        let (local_eye, local_direction) = (site.local(eye), site.local_direction(direction));
        if local_direction.x <= f32::EPSILON {
            return false;
        }
        // Rotation keeps lengths, so the distance along the ray is the same
        // in either frame.
        let distance = (plane.x - local_eye.x) / local_direction.x;
        let offset = local_eye + local_direction * distance - plane;
        let target = eye + direction * distance;
        if !target.is_finite() || offset.z.abs() > half[0] || offset.y.abs() > half[1] {
            return false;
        }
        let clip = view.view_proj * target.extend(1.0);
        if !clip.is_finite() || clip.w <= 0.0 || !(0.0..=1.0).contains(&(clip.z / clip.w)) {
            return false;
        }
        !mesh_occludes(&self.world.mesh, eye, direction, distance)
            && !mesh_occludes(&self.dynamic_mesh(), eye, direction, distance)
            && !mesh_occludes(entities, eye, direction, distance)
    }

    /// Local geometry with neutral displays. Services append remote entities.
    #[must_use]
    pub fn dynamic_mesh(&self) -> Mesh {
        self.dynamic_mesh_with_interactions(false, false)
    }

    /// Local geometry with the mobile computer's proximity and tap prompt.
    /// Use only when the host implements picking and opens the computer.
    #[must_use]
    pub fn dynamic_mesh_with_computer_interaction(&self) -> Mesh {
        self.dynamic_mesh_with_interactions(true, false)
    }

    /// Advertise only the physical display actions implemented by this host.
    /// This changes presentation alone, without admitting a tap or any service.
    #[must_use]
    pub fn dynamic_mesh_with_interactions(&self, computer: bool, gym: bool) -> Mesh {
        self.dynamic_mesh_with_panels(computer, gym, false)
    }

    /// As [`Self::dynamic_mesh_with_interactions`], with the Grid's RESULTS
    /// board's tap cue for a host that opens its panel.
    #[must_use]
    pub fn dynamic_mesh_with_panels(&self, computer: bool, gym: bool, results: bool) -> Mesh {
        self.dynamic_mesh_with_boards(computer, gym, results, false)
    }

    /// As [`Self::dynamic_mesh_with_panels`], with the Grid's EVALS board's
    /// tap cue for a host that opens its panel.
    #[must_use]
    pub fn dynamic_mesh_with_boards(
        &self,
        computer: bool,
        gym: bool,
        results: bool,
        evals: bool,
    ) -> Mesh {
        #[cfg(feature = "hosted-social")]
        if self.is_hosted() {
            return self.hosted_mesh();
        }
        if self.bare && self.is_plaza() {
            // The player, the ball and blocks, and the walk-in portals (to
            // Everglade, and to Lagrange 1, which is hidden for now; see
            // `zones::gate`), on the neutral stage; in first person the camera is inside the
            // avatar, which is hidden, and an unoccupied world has none.
            let mut player = if self.hides_avatar() || self.unoccupied {
                Mesh::default()
            } else {
                avatar::mesh(&self.player, &self.gait)
            };
            player.neutralize();
            player.neon = Some(crate::pbr::Neon::neutral(0.0));
            if let Some(ball) = &self.ball {
                ball.draw(&mut player);
            }
            player.extend(&self.grid_portal_mesh());
            // The Gym board's lettering, and its tap cue for a host that
            // opens the board.
            let mut display =
                world::gym_display(world::GymSite::GRID, gym.then_some(self.gym(1.0).near));
            display.extend(&world::results_display(
                world::GymSite::GRID,
                results.then_some(self.results(1.0).near),
            ));
            display.extend(&world::evals_display(
                world::GymSite::GRID,
                evals.then_some(self.evals(1.0).near),
            ));
            display.neutralize();
            player.extend(&display);
            if let Some(at) = self.trace_ghost {
                player.extend(&crate::gym_replay::ghost_mesh(at));
            }
            return player;
        }
        if self.bare {
            // A zone entered from the Grid, with its guides and return arch
            // in the neutral palette and no companion.
            return self.zone_dynamic_mesh();
        }
        let mut dynamic = if self.is_plaza() {
            avatar::mesh(&self.player, &self.gait)
        } else {
            Mesh::default()
        };
        dynamic.extend(&self.zone_dynamic_mesh());
        if self.companion_present() {
            dynamic.extend(&self.agent.mesh());
        }
        if self.is_plaza() {
            dynamic.extend(&self.doors.mesh(&self.player));
            let offset = self.player.pos - world::COMPUTER;
            dynamic.extend(&world::computer_display(
                computer.then_some(offset.x.hypot(offset.z) <= world::COMPUTER_RANGE),
            ));
            dynamic.extend(&world::gym_display(
                world::GymSite::PLAZA,
                gym.then_some(self.gym(1.0).near),
            ));
            dynamic.neon = Some(crate::pbr::Neon::plaza(0.0));
        }
        dynamic
    }

    /// Sets a finite spawn inside the world. Source admission checks placement.
    pub fn set_spawn(&mut self, position: Vec3, yaw: f32) -> Result<(), String> {
        self.place_player(position, yaw)?;
        // The bare world's bodies stay where everyone left them; only the
        // player's capsule moves.
        if self.is_plaza()
            && let Some(ball) = &mut self.ball
        {
            ball.place_player(self.player.pos);
        }
        Ok(())
    }

    /// Stands the player at a finite position inside the world, leaving the
    /// ball and blocks where they are.
    pub fn place_player(&mut self, position: Vec3, yaw: f32) -> Result<(), String> {
        if self.is_hosted() {
            return Err("Hosted placement requires an authoritative snapshot".into());
        }
        if !position.is_finite()
            || !yaw.is_finite()
            || position.x.abs() >= self.zone_half()
            || position.z.abs() >= self.zone_half()
            || position.y < if self.is_plaza() { 0.0 } else { -100.0 }
            || position.y > 100.0
        {
            return Err("spawn is nonfinite or outside the world".into());
        }
        self.cancel_navigation();
        self.doors.cancel_transient();
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
    let faces = mesh
        .faces
        .chunks_exact(3)
        .map(|v| [v[0].pos, v[1].pos, v[2].pos]);
    let lit = mesh
        .lit
        .chunks_exact(3)
        .map(|v| [v[0].pos, v[1].pos, v[2].pos]);
    faces
        .chain(lit)
        .filter_map(|vertices| {
            let a = Vec3::from(vertices[0]);
            let edge_ab = Vec3::from(vertices[1]) - a;
            let edge_ac = Vec3::from(vertices[2]) - a;
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

    #[test]
    fn walking_after_an_orbit_heads_where_the_camera_looks() {
        let mut runtime = WorldRuntime::new();
        runtime.camera.yaw_offset = 1.2;
        let view_yaw = wrap(runtime.player.yaw + runtime.camera.yaw_offset);
        let start = runtime.player.pos;
        let walk = InputState {
            forward: true,
            ..InputState::default()
        };
        for _ in 0..10 {
            runtime.tick(&walk, 0.05);
        }
        assert!(wrap(runtime.player.yaw - view_yaw).abs() < 1e-4);
        assert!(runtime.camera.yaw_offset.abs() < 1e-6);
        let moved = runtime.player.pos - start;
        let heading = crate::controller::forward(view_yaw);
        assert!(moved.x * heading.x + moved.z * heading.z > 1.0, "{moved}");
        // A turning key alone keeps the orbit.
        runtime.camera.yaw_offset = 0.7;
        let turn = InputState {
            left: true,
            ..InputState::default()
        };
        runtime.tick(&turn, 0.05);
        assert!((runtime.camera.yaw_offset - 0.7).abs() < 1e-6);
    }

    fn projected(runtime: &WorldRuntime, aspect: f32, at: Vec3) -> [f32; 2] {
        let clip = runtime.view(aspect).view_proj * at.extend(1.0);
        [clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5]
    }

    #[test]
    #[ignore = "the Grid's ball and blocks are off for now (2026-10-01)"]
    fn the_bare_world_is_a_neutral_grid_with_only_the_player() {
        let mut runtime = WorldRuntime::bare();
        assert!(runtime.is_bare() && runtime.is_plaza());
        // The Gym's walls are the only blockers, and its hall the only
        // geometry off the ground.
        assert_eq!(
            runtime.world.blockers,
            world::GymSite::GRID.walls().to_vec()
        );
        let full = WorldRuntime::new();
        assert!(runtime.world.mesh.lines.len() < full.world.mesh.lines.len() / 4);
        let gray = |v: &crate::mesh::Vertex| v.color[0] == v.color[1] && v.color[1] == v.color[2];
        let gym = |v: &crate::mesh::Vertex| {
            let local = world::GymSite::GRID.local(Vec3::from(v.pos));
            (35.0..=60.1).contains(&local.x) && local.z.abs() <= 9.1
        };
        assert!(
            runtime
                .world
                .mesh
                .lines
                .iter()
                .chain(&runtime.world.mesh.faces)
                .all(|v| gray(v) && (v.pos[1] == 0.0 || gym(v)))
        );
        let dynamic = runtime.dynamic_mesh_with_interactions(true, true);
        assert!(!dynamic.lines.is_empty());
        assert!(dynamic.lines.iter().chain(&dynamic.faces).all(gray));
        // The ball and the blocks are the only lit geometry, gray
        // lacquer under a studio key.
        assert!(!dynamic.lit.is_empty() && dynamic.glow.is_empty());
        assert!(
            dynamic
                .lit
                .iter()
                .all(|v| v.color.iter().all(|c| (c - v.color[0]).abs() < 0.03))
        );
        let neon = dynamic.neon.expect("the neutral stage");
        assert!(neon.key.is_some());
        assert_eq!(
            crate::pbr::Neon { key: None, ..neon },
            crate::pbr::Neon::neutral(0.0)
        );
        assert!(WorldRuntime::new().ball().is_none());
        assert!(
            WorldRuntime::new()
                .dynamic_mesh()
                .neon
                .is_some_and(|n| n.key.is_none())
        );
        let [r, g, b] = runtime.atmosphere().color;
        assert!(r == g && g == b);
        // The bare grid fades from near the player toward the horizon, on
        // both render paths; Coder's plaza keeps its own distances.
        let atmosphere = runtime.atmosphere();
        assert_eq!(
            (atmosphere.fog_start, atmosphere.fog_end),
            (crate::render::BARE_FOG_START, crate::render::BARE_FOG_END)
        );
        assert!(atmosphere.fog_end < world::HALF);
        assert!(atmosphere.validate().is_ok());
        assert_eq!(
            (neon.fog_start, neon.fog_end),
            (atmosphere.fog_start, atmosphere.fog_end)
        );
        let plaza = WorldRuntime::new().atmosphere();
        assert_eq!(
            (plaza.fog_start, plaza.fog_end),
            (crate::render::FOG_START, crate::render::FOG_END)
        );
        // Standing where the plaza's objects would be reaches none of them.
        runtime
            .set_spawn(world::COMPUTER + Vec3::Z * -2.0, 0.0)
            .unwrap();
        let computer = runtime.computer(1.0);
        assert!(!computer.near && !computer.visible);
        assert!(!runtime.computer_hit(1.0, computer.screen_x, computer.screen_y));
        assert!(!runtime.companion(1.0).near && !runtime.pet_companion());
        runtime
            .set_spawn(world::GYM_BOARD.with_y(0.0) - Vec3::X * 3.0, 0.0)
            .unwrap();
        assert!(!runtime.gym(1.0).inside);
        for id in DoorId::ALL {
            runtime
                .set_spawn(id.position() - Vec3::Z * 3.0, 0.0)
                .unwrap();
            assert!(!runtime.door(id, 1.0).near);
            assert!(runtime.nearest_door(1.0).is_none());
        }
        for (_, portal) in crate::zones::ZoneId::Plaza.portals() {
            runtime.set_spawn(portal - Vec3::Z * 2.0, 0.0).unwrap();
            assert!(runtime.zone_intent(crate::zones::Intent::Enter).is_err());
            assert!(runtime.is_plaza());
        }
        // The player still walks and jumps through the shared controller,
        // straight through where the computer's desk would block it.
        runtime.set_spawn(world::SPAWN, 0.0).unwrap();
        let start = runtime.player.pos;
        let walk = InputState {
            forward: true,
            ..InputState::default()
        };
        for _ in 0..60 {
            runtime.tick(&walk, 1.0 / 60.0);
        }
        assert!(
            runtime.player.pos.z > world::COMPUTER.z + 1.0,
            "{:?}",
            runtime.player.pos
        );
        assert!(runtime.player.pos.distance(start) > 3.0);
        runtime.tick(
            &InputState {
                jump: true,
                ..InputState::default()
            },
            1.0 / 60.0,
        );
        runtime.tick(&InputState::default(), 0.1);
        assert!(runtime.player.airborne());
    }

    #[test]
    fn players_cannot_walk_through_each_others_avatars() {
        for (mut runtime, start) in [
            (WorldRuntime::bare(), Vec3::new(40.0, 0.0, -60.0)),
            (WorldRuntime::new(), world::SPAWN),
        ] {
            runtime.set_spawn(start, 0.0).unwrap();
            // Another player stands 4 m to the right, away from the ball
            // and the blocks ahead; strafing right walks at them (-X).
            let other = start + Vec3::NEG_X * 4.0;
            runtime.set_avatars([other]);
            let walk = InputState {
                strafe_right: true,
                ..InputState::default()
            };
            for _ in 0..120 {
                runtime.tick(&walk, 1.0 / 60.0);
                let offset = runtime.player.pos - other;
                assert!(
                    offset.x.hypot(offset.z) >= crate::controller::RADIUS * 2.0 - 1e-3,
                    "{:?}",
                    runtime.player.pos
                );
            }
            // It stopped against them instead of passing through.
            assert!(runtime.player.pos.x > other.x);
            // With nobody there, the way is open again.
            runtime.set_avatars([]);
            for _ in 0..60 {
                runtime.tick(&walk, 1.0 / 60.0);
            }
            assert!(runtime.player.pos.x < other.x - 1.0);
        }
        // An avatar that walks onto the player pushes the player aside.
        let mut runtime = WorldRuntime::bare();
        runtime.set_spawn(Vec3::new(40.0, 0.0, -60.0), 0.0).unwrap();
        runtime.set_avatars([Vec3::new(40.2, 0.0, -60.0)]);
        runtime.tick(&InputState::default(), 1.0 / 60.0);
        assert!(runtime.player.pos.x < 40.2 - crate::controller::RADIUS * 2.0 + 1e-3);
        // Nonfinite positions are ignored.
        runtime.set_avatars([Vec3::NAN]);
        assert!(runtime.avatars.is_empty());
    }

    #[test]
    #[ignore = "the Grid's ball and blocks are off for now (2026-10-01)"]
    fn zooming_all_the_way_in_on_the_grid_looks_through_the_players_eyes() {
        let mut runtime = WorldRuntime::bare();
        let third_person = runtime.dynamic_mesh().lines.len();
        for _ in 0..40 {
            runtime.apply(Action::PinchZoom { scale: 1.1 }).unwrap();
        }
        assert!(runtime.first_person());
        runtime.tick(&InputState::default(), 1.0);
        let view = runtime.view(0.5);
        let head = crate::camera::head(runtime.player.pos);
        assert!(view.eye.distance(head) < 1e-5, "{:?}", view.eye);
        // The avatar is hidden; the ball, blocks, portal (while shown), and
        // the Gym board's lettering still draw.
        let mesh = runtime.dynamic_mesh();
        assert!(mesh.lines.len() < third_person);
        assert_eq!(
            mesh.faces.len(),
            runtime.grid_portal_mesh().faces.len()
                + world::gym_display(world::GymSite::GRID, None).faces.len()
                + world::results_display(world::GymSite::GRID, None)
                    .faces
                    .len()
                + world::evals_display(world::GymSite::GRID, None).faces.len()
        );
        assert!(!mesh.lit.is_empty());
        // Walking keeps the eye on the head.
        let walk = InputState {
            forward: true,
            ..InputState::default()
        };
        for _ in 0..30 {
            runtime.tick(&walk, 1.0 / 60.0);
        }
        let head = crate::camera::head(runtime.player.pos);
        assert!(runtime.view(0.5).eye.distance(head) < 1e-5);
        // Zooming out returns to third person with the avatar.
        for _ in 0..3 {
            runtime.apply(Action::PinchZoom { scale: 0.9 }).unwrap();
        }
        runtime.tick(&InputState::default(), 1.0);
        assert!(!runtime.first_person());
        assert_eq!(runtime.dynamic_mesh().lines.len(), third_person);
        assert!(runtime.camera.distance >= crate::camera::MIN_DISTANCE);
        // Coder's plaza stops at the nearest orbit.
        let mut plaza = WorldRuntime::new();
        for _ in 0..40 {
            plaza.apply(Action::PinchZoom { scale: 1.1 }).unwrap();
            plaza.apply(Action::Zoom { lines: 3.0 }).unwrap();
        }
        assert!(!plaza.first_person() && !plaza.camera.first_person);
        assert_eq!(plaza.camera.distance, crate::camera::MIN_DISTANCE);
    }

    #[test]
    fn door_anchors_are_hittable_on_approach_and_reject_front_obstructions() {
        let mut runtime = WorldRuntime::new();
        for id in DoorId::ALL {
            runtime
                .set_spawn(id.position() + Vec3::new(0.0, 0.0, -3.0), 0.0)
                .unwrap();
            for aspect in [0.46, 1.0, 2.2] {
                let door = runtime.door(id, aspect);
                assert!(door.near && door.visible);
                assert!(
                    runtime.door_hit(id, aspect, door.screen_x, door.screen_y),
                    "{id:?} {aspect}"
                );
                assert_eq!(runtime.nearest_door(aspect), Some(id));
                let (eye, direction) =
                    viewport_ray(&runtime.view(aspect), aspect, door.screen_x, door.screen_y)
                        .unwrap();
                let distance = (id.plane().z - eye.z) / direction.z;
                let at = eye + direction * (distance - 1.0);
                let mut foreground = Mesh::default();
                foreground.cube(
                    glam::Mat4::from_translation(at),
                    coder_ui::theme::Intensity::Full,
                );
                assert!(!runtime.door_hit_with_entities(
                    id,
                    aspect,
                    door.screen_x,
                    door.screen_y,
                    &foreground
                ));
                let old = runtime.world.mesh.clone();
                runtime.world.mesh.extend(&foreground);
                assert!(!runtime.door_hit(id, aspect, door.screen_x, door.screen_y));
                runtime.world.mesh = old;
                let outside = projected(&runtime, aspect, id.plane() + Vec3::X * 1.4);
                assert!(!runtime.door_hit(id, aspect, outside[0], outside[1]));
            }
            for (aspect, x, y) in [
                (f32::NAN, 0.5, 0.5),
                (0.0, 0.5, 0.5),
                (1.0, f32::NAN, 0.5),
                (1.0, 0.5, 1.1),
            ] {
                assert!(!runtime.door_hit(id, aspect, x, y));
            }
            runtime
                .set_spawn(
                    id.position() + Vec3::new(0.0, 0.0, 3.0),
                    std::f32::consts::PI,
                )
                .unwrap();
            let door = runtime.door(id, 1.0);
            assert!(!runtime.door_hit(id, 1.0, door.screen_x, door.screen_y));
            runtime.set_spawn(world::SPAWN, 0.0).unwrap();
            assert!(runtime.tap_door(id).is_err());
        }
    }

    #[test]
    fn doors_preview_then_walk_and_reset_cancels_only_their_own_route() {
        let mut runtime = WorldRuntime::new();
        let id = DoorId::Spark;
        runtime
            .set_spawn(id.position() + Vec3::new(0.0, 0.0, -3.0), 0.0)
            .unwrap();
        let start = runtime.player.pos;
        assert!(runtime.tap_door(id).unwrap());
        assert!(!runtime.navigation().is_active());
        for _ in 0..60 {
            runtime.tick(&InputState::default(), 1.0 / 60.0);
        }
        assert_eq!(runtime.player.pos, start);
        assert!(runtime.tap_door(id).unwrap());
        assert_eq!(
            runtime.navigation().destination(),
            Some(doors::Destination::Library.point())
        );
        runtime.reset_door(DoorId::Halo);
        assert!(runtime.navigation().is_active());
        runtime.reset_door(id);
        assert_eq!(runtime.navigation().status(), NavigationStatus::Cancelled);
        runtime.navigate_to([-12.0, -12.0]).unwrap();
        runtime.reset_door(id);
        assert!(
            runtime.navigation().is_active(),
            "reset must preserve an unrelated map route"
        );
        runtime.cancel_navigation();
        runtime.tap_door(id).unwrap();
        runtime.cancel_door_interactions();
        assert_eq!(runtime.doors.state(id).phase, doors::DoorPhase::Idle);
        assert_eq!(runtime.doors.state(id).last, Some(DemoItem::Prism));
    }

    #[test]
    fn blocked_door_route_is_visible_and_restoration_never_restarts_it() {
        let mut runtime = WorldRuntime::new();
        let id = DoorId::Spark;
        runtime
            .set_spawn(id.position() + Vec3::new(0.0, 0.0, -3.0), 0.0)
            .unwrap();
        runtime.tap_door(id).unwrap();
        for _ in 0..60 {
            runtime.tick(&InputState::default(), 1.0 / 60.0);
        }
        let target = doors::Destination::Library.point();
        runtime.world.blockers.push(crate::controller::Footprint {
            min: [target[0] - 1.0, target[1] - 1.0],
            max: [target[0] + 1.0, target[1] + 1.0],
        });
        assert!(runtime.tap_door(id).is_err());
        assert!(!runtime.navigation().is_active());
        assert_eq!(runtime.doors.state(id).caption(), "No route found");
        for _ in 0..120 {
            runtime.tick(&InputState::default(), 1.0 / 60.0);
        }
        assert_eq!(runtime.doors.state(id).caption(), "No route found");
        let document = runtime.doors.document();
        runtime.restore_door_state(&document).unwrap();
        assert_eq!(runtime.doors.state(id).phase, doors::DoorPhase::Idle);
        assert_eq!(runtime.doors.state(id).selected, None);
        assert!(!runtime.navigation().is_active());
    }

    #[test]
    fn all_door_destinations_remain_reachable_and_gym_ends_outside_the_hall() {
        let mut runtime = WorldRuntime::new();
        for id in DoorId::ALL {
            for item in DemoItem::ALL {
                if let Some(target) = doors::destination(id, item) {
                    runtime
                        .set_spawn(id.position() + Vec3::new(0.0, 0.0, -3.0), 0.0)
                        .unwrap();
                    runtime.navigate_to(target.point()).unwrap();
                    for _ in 0..3000 {
                        runtime.tick(&InputState::default(), 1.0 / 60.0);
                        assert!(!runtime.world.blockers.iter().any(|b| b.contains(
                            runtime.player.pos.x,
                            runtime.player.pos.z,
                            crate::controller::RADIUS
                        )));
                        if !runtime.navigation().is_active() {
                            break;
                        }
                    }
                    assert_eq!(
                        runtime.navigation().status(),
                        NavigationStatus::Arrived,
                        "{id:?} {item:?}"
                    );
                    if target == doors::Destination::GymApproach {
                        assert!(!runtime.gym(1.0).inside);
                    }
                }
            }
        }
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
    fn gym_action_prompt_requires_an_interactive_host_and_tracks_proximity() {
        let mut runtime = WorldRuntime::new();
        let neutral = runtime.dynamic_mesh();
        let distant = runtime.dynamic_mesh_with_interactions(false, true);
        assert!(distant.faces.len() > neutral.faces.len());
        assert_eq!(
            neutral.faces,
            runtime.dynamic_mesh_with_interactions(false, false).faces
        );
        runtime
            .set_spawn(Vec3::new(54.0, 0.0, 0.0), std::f32::consts::FRAC_PI_2)
            .unwrap();
        let nearby = runtime.dynamic_mesh_with_interactions(false, true);
        assert!(nearby.faces.len() > runtime.dynamic_mesh().faces.len());
        assert_ne!(distant.faces.len(), nearby.faces.len());
    }

    #[test]
    fn the_grids_results_board_picks_apart_from_the_live_board_and_only_in_the_grid() {
        let mut runtime = WorldRuntime::bare();
        let site = world::GymSite::GRID;
        // Inside the hall, facing the boards from between them.
        let stand = site.point(Vec3::new(54.0, 0.0, 2.8));
        runtime
            .set_spawn(stand, site.yaw_of(std::f32::consts::FRAC_PI_2))
            .unwrap();
        let screen = site.point(world::GYM_RESULTS_SCREEN);
        for aspect in [0.46, 1.0, 2.2] {
            let results = runtime.results(aspect);
            assert!(results.inside && results.near, "{results:?}");
            let [x, y] = projected(&runtime, aspect, screen);
            if !((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)) {
                continue;
            }
            assert!(runtime.results_hit_with_entities(aspect, x, y, &Mesh::default()));
            assert!(
                !runtime.gym_hit(aspect, x, y),
                "the RESULTS board isn't the live board"
            );
            let [x, y] = projected(&runtime, aspect, site.point(world::GYM_BOARD_SCREEN));
            assert!(!runtime.results_hit_with_entities(aspect, x, y, &Mesh::default()));
        }
        // Its lettering and cue turn with the building, on its face.
        let display = world::results_display(site, Some(true));
        assert!(!display.faces.is_empty());
        for vertex in &display.faces {
            let local = site.local(Vec3::from(vertex.pos)) - world::GYM_RESULTS_SCREEN;
            assert!(local.x.abs() < 1e-3 && local.z.abs() < world::GYM_RESULTS_HALF[0]);
            assert!(local.y.abs() < world::GYM_RESULTS_HALF[1]);
        }
        // Outside the hall it is neither near nor tappable.
        runtime.set_spawn(world::SPAWN, 0.0).unwrap();
        assert!(!runtime.results(1.0).near);
        // Coder's plaza has no RESULTS board.
        let mut plaza = WorldRuntime::new();
        plaza
            .set_spawn(Vec3::new(54.0, 0.0, 2.8), std::f32::consts::FRAC_PI_2)
            .unwrap();
        assert!(!plaza.results(1.0).inside);
        let [x, y] = projected(&plaza, 1.0, world::GYM_RESULTS_SCREEN);
        assert!(!plaza.results_hit_with_entities(1.0, x, y, &Mesh::default()));
    }

    #[test]
    fn the_grids_evals_board_picks_apart_from_the_other_boards_and_only_in_the_grid() {
        let mut runtime = WorldRuntime::bare();
        let site = world::GymSite::GRID;
        // Inside the hall, facing the boards from the EVALS side.
        let stand = site.point(Vec3::new(54.0, 0.0, -2.8));
        runtime
            .set_spawn(stand, site.yaw_of(std::f32::consts::FRAC_PI_2))
            .unwrap();
        let screen = site.point(world::GYM_EVALS_SCREEN);
        let mut picked = 0;
        for aspect in [0.46, 1.0, 2.2] {
            let evals = runtime.evals(aspect);
            assert!(evals.inside && evals.near, "{evals:?}");
            let [x, y] = projected(&runtime, aspect, screen);
            if !((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)) {
                continue;
            }
            picked += 1;
            assert!(runtime.evals_hit_with_entities(aspect, x, y, &Mesh::default()));
            assert!(
                !runtime.gym_hit(aspect, x, y),
                "the EVALS board isn't the live board"
            );
            assert!(!runtime.results_hit_with_entities(aspect, x, y, &Mesh::default()));
            let [x, y] = projected(&runtime, aspect, site.point(world::GYM_BOARD_SCREEN));
            assert!(!runtime.evals_hit_with_entities(aspect, x, y, &Mesh::default()));
        }
        assert!(picked > 0, "the board is on screen in some aspect");
        // Its lettering and cue stand on its face, and the tap cue shows
        // only for a host that opens it.
        let display = world::evals_display(site, Some(true));
        assert!(!display.faces.is_empty());
        // Every letter of its label draws: EVALS once missed its V.
        for letter in world::GYM_BOARD_LABELS.iter().flat_map(|l| l.bytes()) {
            assert!(
                letter == b' ' || world::has_glyph(letter),
                "{}",
                letter as char
            );
        }
        for vertex in &display.faces {
            let local = site.local(Vec3::from(vertex.pos)) - world::GYM_EVALS_SCREEN;
            assert!(local.x.abs() < 1e-3 && local.z.abs() < world::GYM_EVALS_HALF[0]);
            assert!(local.y.abs() < world::GYM_EVALS_HALF[1]);
        }
        assert!(
            runtime
                .dynamic_mesh_with_boards(false, false, false, true)
                .faces
                .len()
                > runtime
                    .dynamic_mesh_with_panels(false, false, false)
                    .faces
                    .len()
        );
        runtime.set_spawn(world::SPAWN, 0.0).unwrap();
        assert!(!runtime.evals(1.0).near);
        let mut plaza = WorldRuntime::new();
        plaza
            .set_spawn(Vec3::new(54.0, 0.0, -2.8), std::f32::consts::FRAC_PI_2)
            .unwrap();
        assert!(!plaza.evals(1.0).inside);
        let [x, y] = projected(&plaza, 1.0, world::GYM_EVALS_SCREEN);
        assert!(!plaza.evals_hit_with_entities(1.0, x, y, &Mesh::default()));
    }

    #[test]
    fn gym_picking_tracks_the_physical_front_in_portrait_landscape_and_oblique_views() {
        let mut runtime = WorldRuntime::new();
        runtime
            .set_spawn(Vec3::new(54.0, 0.0, 0.0), std::f32::consts::FRAC_PI_2)
            .unwrap();
        for aspect in [0.46, 2.0 / 3.0, 1.0, 2.2] {
            for orbit in [-0.35, 0.0, 0.35] {
                runtime.camera.yaw_offset = orbit;
                for offset in [
                    Vec3::ZERO,
                    Vec3::new(0.0, 1.3, -2.3),
                    Vec3::new(0.0, 1.3, 2.3),
                ] {
                    let [x, y] = projected(&runtime, aspect, world::GYM_BOARD_SCREEN + offset);
                    let in_viewport = (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y);
                    assert_eq!(
                        runtime.gym_hit(aspect, x, y),
                        in_viewport,
                        "aspect {aspect}, orbit {orbit}, offset {offset}"
                    );
                }
                // The side plot panels and space above the board are not an
                // enlarged native button around the central board's anchor.
                for offset in [Vec3::new(0.0, 0.0, 2.65), Vec3::new(0.0, 1.7, 0.0)] {
                    let [x, y] = projected(&runtime, aspect, world::GYM_BOARD_SCREEN + offset);
                    assert!(!runtime.gym_hit(aspect, x, y));
                }
            }
        }
    }

    #[test]
    fn gym_picking_respects_static_player_and_presented_remote_faces() {
        let mut runtime = WorldRuntime::new();
        runtime
            .set_spawn(Vec3::new(54.0, 0.0, 0.0), std::f32::consts::FRAC_PI_2)
            .unwrap();
        let [x, y] = projected(&runtime, 1.0, world::GYM_BOARD_SCREEN);
        assert!(runtime.gym_hit(1.0, x, y));
        let (eye, direction) = viewport_ray(&runtime.view(1.0), 1.0, x, y).unwrap();
        let distance = (world::GYM_BOARD_SCREEN.x - eye.x) / direction.x;
        let mut remote = Mesh::default();
        remote.cube(
            glam::Mat4::from_translation(eye + direction * (distance - 1.0)),
            coder_ui::theme::Intensity::Full,
        );
        let before = remote.faces.clone();
        assert!(!runtime.gym_hit_with_entities(1.0, x, y, &remote));
        assert_eq!(before, remote.faces);
        runtime.world.mesh.extend(&remote);
        assert!(!runtime.gym_hit(1.0, x, y));
        runtime.world = world::build();
        for vertex in &mut remote.faces {
            vertex.pos = (Vec3::from(vertex.pos) + direction * 3.0).to_array();
        }
        assert!(runtime.gym_hit_with_entities(1.0, x, y, &remote));
        runtime.camera.pitch = 0.0;
        let [x, y] = projected(&runtime, 1.0, world::GYM_BOARD_SCREEN - Vec3::Y * 1.2);
        let (eye, direction) = viewport_ray(&runtime.view(1.0), 1.0, x, y).unwrap();
        let distance = (world::GYM_BOARD_SCREEN.x - eye.x) / direction.x;
        assert!(mesh_occludes(
            &avatar::mesh(&runtime.player, &runtime.gait),
            eye,
            direction,
            distance
        ));
        assert!(!runtime.gym_hit(1.0, x, y));
    }

    #[test]
    fn gym_picking_refuses_back_faces_distance_outside_invalid_input_and_other_zones() {
        let mut runtime = WorldRuntime::new();
        let test = |runtime: &WorldRuntime| {
            let [x, y] = projected(runtime, 1.0, world::GYM_BOARD_SCREEN);
            runtime.gym_hit(1.0, x, y)
        };
        for position in [
            Vec3::new(48.0, 0.0, 0.0),
            Vec3::new(60.0, 0.0, 0.0),
            Vec3::new(54.0, 0.0, 8.6),
            Vec3::new(54.0, 4.6, 0.0),
        ] {
            runtime
                .set_spawn(position, std::f32::consts::FRAC_PI_2)
                .unwrap();
            assert!(!test(&runtime));
        }
        runtime
            .set_spawn(Vec3::new(54.0, 0.0, 0.0), std::f32::consts::FRAC_PI_2)
            .unwrap();
        assert!(test(&runtime));
        for (aspect, x, y) in [
            (0.0, 0.5, 0.5),
            (-1.0, 0.5, 0.5),
            (f32::NAN, 0.5, 0.5),
            (f32::INFINITY, 0.5, 0.5),
            (1.0, f32::NAN, 0.5),
            (1.0, 0.5, f32::INFINITY),
            (1.0, -0.1, 0.5),
            (1.0, 0.5, 1.1),
        ] {
            assert!(!runtime.gym_hit(aspect, x, y));
        }
        runtime.camera.yaw_offset = std::f32::consts::PI;
        assert!(!test(&runtime), "the camera is behind the board");
        runtime.camera.yaw_offset = 0.0;
        runtime.zone = crate::zones::ZoneId::Lagrange1;
        assert!(!test(&runtime));
    }

    #[test]
    fn the_grids_gym_is_entered_through_its_doorway_and_its_board_picks_like_the_plazas() {
        let site = world::GymSite::GRID;
        let mut runtime = WorldRuntime::bare();
        assert_eq!(runtime.gym_site(), Some(site));
        assert_eq!(WorldRuntime::new().gym_site(), Some(world::GymSite::PLAZA));
        // From the spawn the board is ahead but out of reach.
        let spawn = runtime.gym(0.46);
        assert!(!spawn.inside && !spawn.near);
        // Walk in from just outside the doorway, heading into the hall.
        let into = site.yaw_of(std::f32::consts::FRAC_PI_2);
        runtime
            .set_spawn(site.point(Vec3::new(33.0, 0.0, 0.0)), into)
            .unwrap();
        assert!(!runtime.gym(1.0).inside);
        let forward = InputState {
            forward: true,
            ..InputState::default()
        };
        for _ in 0..220 {
            runtime.tick(&forward, 1.0 / 60.0);
        }
        let board = runtime.gym(1.0);
        assert!(board.inside && board.near && board.visible, "{board:?}");
        assert!((board.screen_x - 0.5).abs() < 0.001);
        assert!(site.local(runtime.player.pos).x > 55.0);
        // A jamb blocks walking beside the doorway.
        runtime
            .set_spawn(site.point(Vec3::new(33.0, 0.0, 6.0)), into)
            .unwrap();
        for _ in 0..120 {
            runtime.tick(&forward, 1.0 / 60.0);
        }
        assert!(site.local(runtime.player.pos).x < 36.0);
        assert!(!runtime.gym(1.0).inside);
        // The board picks in portrait and landscape, and not beside it.
        runtime
            .set_spawn(site.point(Vec3::new(54.0, 0.0, 0.0)), into)
            .unwrap();
        let screen = site.point(world::GYM_BOARD_SCREEN);
        let across = site.direction(Vec3::Z);
        for aspect in [0.46, 1.0, 2.2] {
            for orbit in [-0.35, 0.0, 0.35] {
                runtime.camera.yaw_offset = orbit;
                for offset in [
                    Vec3::ZERO,
                    Vec3::Y * 1.3 - across * 2.3,
                    Vec3::Y * 1.3 + across * 2.3,
                ] {
                    let [x, y] = projected(&runtime, aspect, screen + offset);
                    let in_viewport = (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y);
                    assert_eq!(
                        runtime.gym_hit(aspect, x, y),
                        in_viewport,
                        "{aspect} {orbit} {offset}"
                    );
                }
                for offset in [across * 2.65, Vec3::Y * 1.7] {
                    let [x, y] = projected(&runtime, aspect, screen + offset);
                    assert!(!runtime.gym_hit(aspect, x, y));
                }
            }
        }
        runtime.camera.yaw_offset = std::f32::consts::PI;
        let [x, y] = projected(&runtime, 1.0, screen);
        assert!(
            !runtime.gym_hit(1.0, x, y),
            "the camera is behind the board"
        );
        runtime.camera.yaw_offset = 0.0;
        // The tap cue shows only for a host that opens the board, and the
        // whole Gym stays in the neutral palette.
        let cue = runtime.dynamic_mesh_with_interactions(true, true);
        assert!(cue.faces.len() > runtime.dynamic_mesh().faces.len());
        assert!(
            cue.lines
                .iter()
                .chain(&cue.faces)
                .all(|v| v.color[0] == v.color[1] && v.color[1] == v.color[2])
        );
        // Lagrange 1 has no Gym.
        runtime.zone = crate::zones::ZoneId::Lagrange1;
        assert_eq!(runtime.gym_site(), None);
        assert!(!runtime.gym(1.0).inside);
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
