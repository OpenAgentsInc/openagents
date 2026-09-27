#![deny(warnings, clippy::all, clippy::pedantic)]

use client_core::systems::move_intent::{make_basis_from, move_vec_xz, pack_dx_dz};
use client_core::systems::pc_controller::ResolvedIntents;
use glam::Vec2;

fn cam_fwd() -> Vec2 {
    Vec2::new(0.0, 1.0)
} // +Z

#[test]
fn forward_is_dz_positive_camera_basis() {
    let b = make_basis_from(cam_fwd(), 0.0, true);
    let mut intents = ResolvedIntents::default();
    intents.forward = true;
    let (dx, dz) = pack_dx_dz(move_vec_xz(intents, b));
    assert!((dx - 0.0).abs() < 1e-6);
    assert!(dz > 0.99);
}

#[test]
fn yaw_basis_rotates_forward_to_x() {
    // yaw = +90° => facing +X
    let yaw = std::f32::consts::FRAC_PI_2;
    let b = make_basis_from(cam_fwd(), yaw, false);
    let mut intents = ResolvedIntents::default();
    intents.forward = true;
    let (dx, dz) = pack_dx_dz(move_vec_xz(intents, b));
    assert!(dx > 0.99);
    assert!(dz.abs() < 1e-6);
}

#[test]
fn diagonals_are_normalized() {
    let b = make_basis_from(cam_fwd(), 0.0, true);
    let mut intents = ResolvedIntents::default();
    intents.forward = true;
    intents.strafe_right = true;
    let v = move_vec_xz(intents, b);
    assert!((v.length() - 1.0).abs() < 1e-6);
}
