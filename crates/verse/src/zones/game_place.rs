//! The Game Lab entrance shared by the Grid's native and web renderers.

use glam::Vec3;

pub const MODEL: &str = "grid/game-lab";
pub const LABEL: &str = "GAME LAB";
pub const POSITION: Vec3 = Vec3::new(-8.0, 0.0, 3.0);

#[must_use]
pub fn in_reach(player: Vec3, already_near: bool) -> bool {
    let delta = player - POSITION;
    player.is_finite()
        && delta.y.abs() < 2.5
        && delta.x.hypot(delta.z) < if already_near { 4.5 } else { 3.5 }
}

pub(crate) fn mesh() -> crate::mesh::Mesh {
    let mut mesh = crate::mesh::Mesh::default();
    super::arch_frame(&mut mesh, LABEL, POSITION);
    mesh.neutralize();
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approach_requires_the_same_floor_and_has_hysteresis() {
        assert!(in_reach(POSITION + Vec3::Z * 3.0, false));
        assert!(!in_reach(POSITION + Vec3::Z * 4.0, false));
        assert!(in_reach(POSITION + Vec3::Z * 4.0, true));
        assert!(!in_reach(POSITION + Vec3::Y * 5.0, true));
        assert!(!in_reach(Vec3::NAN, true));
    }
}
