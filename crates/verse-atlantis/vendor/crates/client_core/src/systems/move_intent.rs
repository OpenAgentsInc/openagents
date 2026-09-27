#![allow(clippy::module_name_repetitions)]

use super::pc_controller::ResolvedIntents;
use glam::Vec2;

/// Movement basis in XZ plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BasisXZ {
    pub fwd: Vec2,
    pub right: Vec2,
}

/// Build basis:
/// - `use_camera`: use camera forward; else use player yaw.
#[must_use]
pub fn make_basis_from(camera_fwd_xz: Vec2, player_yaw: f32, use_camera: bool) -> BasisXZ {
    let fwd = if use_camera {
        camera_fwd_xz.normalize_or_zero()
    } else {
        // Facing vector from yaw (CCW, +Z forward)
        Vec2::new(player_yaw.sin(), player_yaw.cos())
    };
    let right = Vec2::new(fwd.y, -fwd.x);
    BasisXZ { fwd, right }
}

/// Resolve world-space XZ move vector from intents + basis.
#[must_use]
pub fn move_vec_xz(intents: ResolvedIntents, basis: BasisXZ) -> Vec2 {
    let mut mx = 0.0;
    let mut mz = 0.0;
    if intents.strafe_right {
        mx += 1.0;
    }
    if intents.strafe_left {
        mx -= 1.0;
    }
    if intents.forward {
        mz += 1.0;
    }
    if intents.backward {
        mz -= 1.0;
    }
    let mut v = basis.right * mx + basis.fwd * mz;
    if v.length_squared() > 1.0 {
        v = v.normalize();
    }
    v
}

/// Pack to network coords: RIGHT‑positive `dx`, FORWARD‑positive `dz`.
#[must_use]
pub fn pack_dx_dz(v: Vec2) -> (f32, f32) {
    (v.x, v.y)
}
