//! Player controller integration (movement + camera-facing) — v0.
//!
//! This module centralizes player movement updates so the renderer can
//! delegate transform math here and only upload GPU buffers.

use crate::controller::PlayerController;
use crate::input::InputState;
use glam::Vec3;

/// Update the player controller position/yaw based on input and camera forward.
/// Implements WoW-style rules: RMB=mouse look (A/D strafe), A/D turn otherwise.
pub fn update(player: &mut PlayerController, input: &InputState, dt: f32, cam_forward: Vec3) {
    player.update(input, dt, cam_forward);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wow_turn_strafe_and_forward() {
        let mut pc = PlayerController::new(Vec3::ZERO);
        let mut input = InputState::default();
        let dt = 1.0;
        let cam = Vec3::Z; // looking +Z => forward is +Z
        // W moves +Z
        input.forward = true;
        update(&mut pc, &input, dt, cam);
        assert!(pc.pos.z > 0.9);
        // Reset and test A without mouselook: should TURN, not translate
        pc.pos = Vec3::ZERO;
        input = InputState::default();
        input.turn_left = true;
        update(&mut pc, &input, dt, cam);
        assert!(pc.pos.length() < 1e-3);
        assert!(pc.yaw > 0.0);
        // Now hold RMB (mouselook) and press A: should STRAFE left
        pc.pos = Vec3::ZERO;
        pc.yaw = 0.0;
        input = InputState::default();
        input.mouse_look = true;
        input.strafe_left = true;
        update(&mut pc, &input, dt, cam);
        assert!(pc.pos.x < -0.9);
    }
}
