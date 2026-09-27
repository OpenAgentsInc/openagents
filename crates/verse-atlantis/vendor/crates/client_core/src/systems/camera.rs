//! Camera integration system to update a simple third-person pose.

use crate::facade::controller::ControllerState;

#[derive(Clone, Copy, Debug)]
pub struct CameraRigCfg {
    pub boom_len: f32,
    pub boom_height: f32,
}
impl Default for CameraRigCfg {
    fn default() -> Self {
        Self {
            boom_len: 8.5,
            boom_height: 1.6,
        }
    }
}

pub fn update_camera_pose(cfg: &CameraRigCfg, state: &mut ControllerState, target: glam::Vec3) {
    let look = state.camera.look_dir.normalize_or_zero();
    let up = glam::Vec3::Y;
    let eye = target + up * cfg.boom_height - look * cfg.boom_len;
    state.camera.eye = eye;
    state.camera.up = up;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eye_moves_with_boom() {
        let mut s = ControllerState::default();
        s.camera.look_dir = glam::Vec3::Z;
        update_camera_pose(&CameraRigCfg::default(), &mut s, glam::Vec3::ZERO);
        assert!(s.camera.eye.length() > 0.0);
    }
}
