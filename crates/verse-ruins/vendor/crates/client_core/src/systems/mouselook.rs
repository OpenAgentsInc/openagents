//! Mouselook integration helper for controller state.

use crate::facade::controller::ControllerState;

#[derive(Clone, Copy, Debug)]
pub struct MouselookConfig {
    pub sensitivity_deg_per_count: f32,
    pub invert_y: bool,
    pub min_pitch_deg: f32,
    pub max_pitch_deg: f32,
}

impl Default for MouselookConfig {
    fn default() -> Self {
        Self {
            sensitivity_deg_per_count: 0.15,
            invert_y: false,
            min_pitch_deg: -80.0,
            max_pitch_deg: 80.0,
        }
    }
}

pub fn apply_mouse_delta(cfg: &MouselookConfig, state: &mut ControllerState, dx: f32, dy: f32) {
    use ecs_core::components::ControllerMode;
    if state.mode != ControllerMode::Mouselook {
        return;
    }
    let to_rad = cfg.sensitivity_deg_per_count.to_radians();
    let yaw = state.camera.yaw + dx * to_rad;
    let mut pitch = state.camera.pitch + (if cfg.invert_y { dy } else { -dy }) * to_rad;
    pitch = pitch.clamp(
        cfg.min_pitch_deg.to_radians(),
        cfg.max_pitch_deg.to_radians(),
    );
    let dir = glam::Vec3::new(
        pitch.cos() * yaw.cos(),
        pitch.sin(),
        pitch.cos() * yaw.sin(),
    )
    .normalize();
    state.camera.yaw = yaw;
    state.camera.pitch = pitch;
    state.camera.look_dir = dir;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pitch_is_clamped() {
        let mut s = ControllerState {
            mode: ecs_core::components::ControllerMode::Mouselook,
            ..Default::default()
        };
        let cfg = MouselookConfig {
            sensitivity_deg_per_count: 1.0,
            invert_y: false,
            min_pitch_deg: -30.0,
            max_pitch_deg: 30.0,
        };
        apply_mouse_delta(&cfg, &mut s, 0.0, -1000.0);
        assert!(s.camera.pitch <= cfg.max_pitch_deg.to_radians() + 1e-6);
        apply_mouse_delta(&cfg, &mut s, 0.0, 1000.0);
        assert!(s.camera.pitch >= cfg.min_pitch_deg.to_radians() - 1e-6);
    }
}
