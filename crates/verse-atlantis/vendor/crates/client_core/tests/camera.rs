use client_core::facade::controller::ControllerState;
use client_core::systems::camera::{CameraRigCfg, update_camera_pose};

#[test]
fn boom_places_camera_behind_look_dir() {
    let mut s = ControllerState::default();
    s.camera.look_dir = glam::Vec3::Z;
    let cfg = CameraRigCfg {
        boom_len: 5.0,
        boom_height: 1.5,
    };

    update_camera_pose(&cfg, &mut s, glam::Vec3::ZERO);

    assert!((s.camera.eye.y - cfg.boom_height).abs() < 1e-3);
    assert!((s.camera.eye.z + cfg.boom_len).abs() < 1e-3);
}
