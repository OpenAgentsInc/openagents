//! CPU side of the crowd: the walk cycle used to bake the VAT. It mirrors
//! `animate` in `shaders/crowd.wgsl`; a test checks a few known poses.

use glam::Vec3;

pub const VAT_FRAMES: u32 = 16;
pub const ATLAS_SIZE: (u32, u32) = (640, 1024);

fn rot_x(p: Vec3, pivot: Vec3, a: f32) -> Vec3 {
    let q = p - pivot;
    let (s, c) = a.sin_cos();
    pivot + Vec3::new(q.x, q.y * c - q.z * s, q.y * s + q.z * c)
}

pub fn animate(p: Vec3, bone: u32, phase: f32) -> Vec3 {
    let tau = std::f32::consts::TAU;
    let swing = (phase * tau).sin();
    let side = if matches!(bone, 3 | 5 | 7 | 9) { -1.0 } else { 1.0 };
    let mut q = p;
    if matches!(bone, 9 | 10) {
        q = rot_x(q, Vec3::new(side * 0.1, 0.47, 0.0), 0.6 * (side * swing).max(0.0));
    }
    if matches!(bone, 7..=10) {
        q = rot_x(q, Vec3::new(side * 0.1, 0.9, 0.0), side * 0.5 * swing);
    }
    if matches!(bone, 5 | 6) {
        q = rot_x(q, Vec3::new(side * 0.3, 1.25, 0.0), -0.3);
    }
    if matches!(bone, 3..=6) {
        q = rot_x(q, Vec3::new(side * 0.3, 1.55, 0.0), -side * 0.4 * swing);
    }
    if !matches!(bone, 7..=10) {
        q.y += 0.04 * (phase * tau).cos().abs();
    }
    q
}

/// Bakes positions (rows 0..16) and normals (rows 16..32) of the low mesh as
/// RGBA32F texels, one column per vertex.
pub fn bake_vat(low: &crate::scene::MeshData, bones: &[u32]) -> Vec<f32> {
    let width = low.vertices.len();
    let mut out = vec![0.0f32; width * (VAT_FRAMES as usize * 2) * 4];
    for frame in 0..VAT_FRAMES {
        let phase = frame as f32 / VAT_FRAMES as f32;
        for (i, v) in low.vertices.iter().enumerate() {
            let p = animate(v.position.into(), bones[i], phase);
            let n = (animate(v.normal.into(), bones[i], phase) - animate(Vec3::ZERO, bones[i], phase)).normalize_or_zero();
            let pi = ((frame as usize) * width + i) * 4;
            out[pi..pi + 3].copy_from_slice(&p.to_array());
            let ni = (((frame + VAT_FRAMES) as usize) * width + i) * 4;
            out[ni..ni + 3].copy_from_slice(&n.to_array());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_pose_only_bobs() {
        // At phase 0 the swing is zero, so limbs stay put except fixed elbows.
        let hip = animate(Vec3::new(0.1, 0.6, 0.0), 8, 0.0);
        assert!((hip - Vec3::new(0.1, 0.6, 0.0)).length() < 1e-5);
        let head = animate(Vec3::new(0.0, 1.7, 0.0), 2, 0.0);
        assert!((head.y - 1.74).abs() < 1e-5);
    }

    #[test]
    fn legs_swing_in_opposite_directions() {
        let l = animate(Vec3::new(-0.1, 0.5, 0.0), 7, 0.25);
        let r = animate(Vec3::new(0.1, 0.5, 0.0), 8, 0.25);
        assert!(l.z * r.z < 0.0, "{l} {r}");
    }
}
