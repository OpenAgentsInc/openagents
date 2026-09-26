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

    /// Local player and follower geometry. Services append remote entities.
    #[must_use]
    pub fn dynamic_mesh(&self) -> Mesh {
        let mut dynamic = avatar::mesh(&self.player, &self.gait);
        dynamic.extend(&self.agent.mesh());
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

#[cfg(test)]
mod tests {
    use super::*;

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
