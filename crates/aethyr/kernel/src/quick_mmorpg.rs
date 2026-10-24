//! Quick_3D_MMORPG's longitudinal controller, adapted to fixed-step collision.
//! See third_party/quick-mmorpg/NOTICE for source and coordinate conventions.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    pub profile: String,
    pub world_scale: f32,
}

impl Configuration {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.profile != "quick-mmorpg-v1"
            || !self.world_scale.is_finite()
            || !(0.01..=1.0).contains(&self.world_scale)
        {
            return Err("Choose a supported character controller and world scale.");
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Controller {
    pub configuration: Configuration,
    pub velocity: f32,
}

impl Controller {
    pub fn new(configuration: Configuration) -> Self {
        Self {
            configuration,
            velocity: 0.0,
        }
    }

    /// Return signed distance and runtime yaw change. Positive turn means right.
    pub fn advance(
        &mut self,
        forward: f32,
        turn: f32,
        sprint: bool,
        movement_locked: bool,
        dt: f32,
    ) -> (f32, f32) {
        af_mmorpg_core::longitudinal(
            &mut self.velocity,
            af_mmorpg_core::Controls {
                forward,
                turn,
                sprint,
                ..Default::default()
            },
            self.configuration.world_scale,
            movement_locked,
            dt,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CameraMode, DT, Engine, Input, Request};

    fn controller() -> Controller {
        Controller::new(Configuration {
            profile: "quick-mmorpg-v1".into(),
            world_scale: 1.0 / 6.0,
        })
    }

    fn engine() -> Engine {
        let mut engine = Engine::default();
        assert_eq!(
            engine.dispatch(Request::SetCamera {
                position: [0.0; 3],
                yaw: 0.0,
                pitch: 0.0,
                mode: CameraMode::Walk,
            })["ok"],
            true
        );
        assert_eq!(
            engine.dispatch(Request::SetCharacterController {
                controller: Some(controller().configuration),
            })["ok"],
            true
        );
        engine
    }

    #[test]
    fn acceleration_coasting_reversal_and_shift_match_the_source_recurrence() {
        let mut value = controller();
        let mut source_velocity = 0.0f32;
        for (forward, sprint, frames) in [(1.0, false, 60), (0.0, false, 20), (-1.0, true, 90)] {
            for _ in 0..frames {
                let deceleration = (-5.0 * source_velocity * DT)
                    .clamp(-source_velocity.abs(), source_velocity.abs());
                source_velocity += deceleration;
                source_velocity += forward * 100.0 * if sprint { 2.0 } else { 1.0 } * DT;
                let (distance, _) = value.advance(forward, 0.0, sprint, false, DT);
                assert!((value.velocity - source_velocity / 6.0).abs() < 0.00001);
                assert!((distance - source_velocity / 6.0 * DT).abs() < 0.000001);
            }
        }
        assert!(value.velocity < -6.0);
    }

    #[test]
    fn shift_does_not_change_turning_and_actions_preserve_momentum() {
        let mut value = controller();
        let (_, normal) = value.advance(1.0, 1.0, false, false, DT);
        let (_, sprint) = value.advance(1.0, 1.0, true, false, DT);
        assert_eq!(normal, sprint);
        assert!((normal * 60.0 - std::f32::consts::FRAC_PI_2).abs() < 0.00001);
        let saved = value.velocity;
        assert_eq!(value.advance(1.0, -1.0, true, true, DT), (0.0, 0.0));
        assert_eq!(saved, value.velocity);
        assert!(value.advance(0.0, 0.0, false, false, DT).0 > 0.0);
    }

    #[test]
    fn profile_retains_collision_ignores_jump_and_does_not_strafe_or_mouse_turn() {
        let mut value = engine();
        value.world.colliders.push(crate::Collider {
            id: "wall".into(),
            min: [-3.0, 0.0, -2.0],
            max: [3.0, 3.0, -1.9],
        });
        value.collision = crate::Shared(std::sync::Arc::new(value.world.colliders.clone()));
        for _ in 0..120 {
            value.step(
                DT,
                Input {
                    forward: 1.0,
                    right: 1.0,
                    jump: true,
                    yaw_delta: 0.3,
                    ..Input::default()
                },
            );
        }
        assert!(value.camera.position[2] >= -1.601);
        assert_eq!(value.camera.position[0], 0.0);
        assert_eq!(value.camera.position[1], 0.0);
        assert_eq!(value.camera.yaw, 0.0);
        assert!(value.quick_controller.as_ref().unwrap().velocity > 3.3);
        assert!(value.velocity[2].abs() < 0.001);
    }

    #[test]
    fn profile_selection_is_atomic_runtime_only_and_legacy_is_the_default() {
        let mut value = engine();
        let before = value.camera.position;
        let invalid = value.dispatch(Request::SetCharacterController {
            controller: Some(Configuration {
                profile: "quick-mmorpg-v1".into(),
                world_scale: f32::NAN,
            }),
        });
        assert_eq!(invalid["ok"], false);
        assert!(value.quick_controller.is_some());
        assert_eq!(value.revision, 0);
        assert_eq!(value.camera.position, before);
        value.dispatch(Request::SetCharacterController { controller: None });
        value.step(
            DT,
            Input {
                right: 1.0,
                ..Input::default()
            },
        );
        assert!(value.camera.position[0] > 0.1);
        assert!(Engine::default().quick_controller.is_none());
    }

    #[test]
    fn one_shot_lock_preserves_position_heading_and_the_velocity_that_resumes() {
        let mut value = engine();
        for _ in 0..60 {
            value.step(
                DT,
                Input {
                    forward: 1.0,
                    ..Input::default()
                },
            );
        }
        let position = value.camera.position;
        let velocity = value.quick_controller.as_ref().unwrap().velocity;
        for _ in 0..60 {
            value.step(
                DT,
                Input {
                    forward: -1.0,
                    turn: 1.0,
                    movement_locked: true,
                    ..Input::default()
                },
            );
        }
        assert_eq!(value.camera.position, position);
        assert_eq!(value.camera.yaw, 0.0);
        assert_eq!(value.quick_controller.as_ref().unwrap().velocity, velocity);
        value.step(DT, Input::default());
        assert!(value.camera.position[2] < position[2]);
        assert!(value.quick_controller.as_ref().unwrap().velocity < velocity);
    }
}
