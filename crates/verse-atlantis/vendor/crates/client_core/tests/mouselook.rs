use client_core::facade::controller::ControllerState;
use client_core::systems::mouselook::{MouselookConfig, apply_mouse_delta};
use ecs_core::components::ControllerMode;

fn mk_state() -> ControllerState {
    ControllerState {
        mode: ControllerMode::Mouselook,
        ..Default::default()
    }
}

#[test]
fn pitch_is_clamped() {
    let mut s = mk_state();
    let cfg = MouselookConfig {
        min_pitch_deg: -30.0,
        max_pitch_deg: 30.0,
        ..Default::default()
    };

    apply_mouse_delta(&cfg, &mut s, 0.0, -10_000.0);
    assert!(s.camera.pitch <= cfg.max_pitch_deg.to_radians() + 1e-6);

    apply_mouse_delta(&cfg, &mut s, 0.0, 10_000.0);
    assert!(s.camera.pitch >= cfg.min_pitch_deg.to_radians() - 1e-6);
}

#[test]
fn invert_y_flips_sign() {
    let mut s1 = mk_state();
    let mut s2 = mk_state();
    let mut cfg = MouselookConfig {
        sensitivity_deg_per_count: 0.5,
        ..Default::default()
    };

    apply_mouse_delta(&cfg, &mut s1, 0.0, 5.0);

    cfg.invert_y = true;
    apply_mouse_delta(&cfg, &mut s2, 0.0, 5.0);

    assert!((s1.camera.pitch.abs() - s2.camera.pitch.abs()).abs() < 1e-4);
    assert!(s1.camera.pitch.signum() == -s2.camera.pitch.signum());
}

#[test]
fn yaw_accumulates() {
    let mut s = mk_state();
    let cfg = MouselookConfig {
        sensitivity_deg_per_count: 1.0,
        ..Default::default()
    };
    let yaw0 = s.camera.yaw;
    apply_mouse_delta(&cfg, &mut s, 10.0, 0.0);
    assert!(s.camera.yaw != yaw0);
}
