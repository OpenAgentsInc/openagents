//! Camera-centered orthographic rain depth map, using shadow mapping from
//! directly above rather than from the sun. No commercial shader code.

use glam::{Mat4, Vec3};
use verse_engine::quality::Tier;

/// Resolution and world width of the bounded rain map.
#[must_use]
pub fn dimensions(tier: Tier) -> (u32, f32) {
    match tier {
        Tier::Low => (256, 64.0),
        Tier::Medium => (256, 80.0),
        Tier::High => (512, 96.0),
    }
}

/// Snap to an 8 m world cell. A straight-down view keeps x/z columns
/// aligned, with a fixed vertical range from −256 to +256 m.
#[must_use]
pub fn matrix(eye: Vec3, tier: Tier) -> Mat4 {
    let (_, width) = dimensions(tier);
    let center = Vec3::new(
        (eye.x / 8.0).floor() * 8.0,
        0.0,
        (eye.z / 8.0).floor() * 8.0,
    );
    Mat4::orthographic_rh(
        -width / 2.0,
        width / 2.0,
        -width / 2.0,
        width / 2.0,
        0.0,
        512.0,
    ) * Mat4::look_at_rh(center + Vec3::Y * 256.0, center, Vec3::NEG_Z)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_map_is_vertical_and_rebuilds_only_on_cell_crossings() {
        for tier in [Tier::Low, Tier::Medium, Tier::High] {
            let a = matrix(Vec3::new(1.0, 2.0, 1.0), tier);
            assert_eq!(a, matrix(Vec3::new(7.9, 12.0, 7.9), tier));
            assert_ne!(a, matrix(Vec3::new(8.0, 2.0, 1.0), tier));
            let roof = a.transform_point3(Vec3::Y * 5.0);
            let floor = a.transform_point3(Vec3::ZERO);
            assert_eq!(roof.truncate(), floor.truncate());
            assert!(roof.z < floor.z);
        }
    }
}
