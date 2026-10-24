//! Cameras, jitter, and frustum planes.

use glam::{Mat4, Vec3, Vec4};

pub const FOV_Y: f32 = std::f32::consts::FRAC_PI_3;
pub const NEAR: f32 = 0.1;
pub const FAR: f32 = 2000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CameraMode {
    /// Eye height along a street: dense occlusion, the occlusion benchmark.
    Street,
    /// A high orbit over the district: wide view, the LOD and crowd benchmark.
    Overview,
    /// Interactive orbit driven by host input.
    Orbit,
}

#[derive(Clone, Copy, Debug)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Orbit { yaw: 0.6, pitch: 0.35, distance: 120.0 }
    }
}

/// Eye position and forward direction at time `t` seconds.
pub fn pose(mode: CameraMode, t: f32, world_side: f32, orbit: &Orbit) -> (Vec3, Vec3) {
    match mode {
        CameraMode::Street => {
            let half = world_side / 2.0;
            // Centre of the street nearest the origin (see scene::place_instances).
            let street_x = -half + (half / 40.0).round() * 40.0 + 3.5;
            let span = (half * 0.6).min(600.0);
            let z = -span + (t * 6.0).rem_euclid(2.0 * span);
            let sway = (t * 0.4).sin() * 0.35;
            (Vec3::new(street_x, 1.8, z), Vec3::new(sway.sin(), -0.02, sway.cos()).normalize())
        }
        CameraMode::Overview => {
            let r = (world_side * 0.25).min(500.0);
            let a = t * 0.05;
            let eye = Vec3::new(a.cos() * r, 60.0, a.sin() * r);
            let target = Vec3::new(0.0, 0.0, 0.0);
            (eye, (target - eye).normalize())
        }
        CameraMode::Orbit => {
            let dir = Vec3::new(orbit.yaw.cos() * orbit.pitch.cos(), orbit.pitch.sin(), orbit.yaw.sin() * orbit.pitch.cos());
            let eye = dir * orbit.distance + Vec3::Y * 2.0;
            (eye, -dir)
        }
    }
}

pub fn view(eye: Vec3, forward: Vec3) -> Mat4 {
    Mat4::look_to_rh(eye, forward, Vec3::Y)
}

pub fn projection(aspect: f32) -> Mat4 {
    Mat4::perspective_rh(FOV_Y, aspect, NEAR, FAR)
}

/// Halton(2, 3) sample `index`, centred on zero, in pixels.
pub fn halton_jitter(index: u32) -> (f32, f32) {
    fn halton(mut i: u32, base: u32) -> f32 {
        let (mut f, mut r) = (1.0, 0.0);
        while i > 0 {
            f /= base as f32;
            r += f * (i % base) as f32;
            i /= base;
        }
        r
    }
    let i = index % 8 + 1;
    (halton(i, 2) - 0.5, halton(i, 3) - 0.5)
}

/// Frustum planes as (normal, distance), inside when `n·p + d >= 0`.
pub fn planes(view_proj: Mat4) -> [Vec4; 6] {
    let r = |i| view_proj.row(i);
    let raw = [r(3) + r(0), r(3) - r(0), r(3) + r(1), r(3) - r(1), r(2), r(3) - r(2)];
    raw.map(|p| p / p.truncate().length())
}

pub fn sphere_visible(planes: &[Vec4; 6], center: Vec3, radius: f32) -> bool {
    planes.iter().all(|p| p.truncate().dot(center) + p.w >= -radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frustum_accepts_ahead_and_rejects_behind() {
        let vp = projection(16.0 / 9.0) * view(Vec3::ZERO, Vec3::Z);
        let p = planes(vp);
        assert!(sphere_visible(&p, Vec3::new(0.0, 0.0, 10.0), 0.5));
        assert!(!sphere_visible(&p, Vec3::new(0.0, 0.0, -10.0), 0.5));
        assert!(!sphere_visible(&p, Vec3::new(0.0, 0.0, FAR + 10.0), 0.5));
    }

    #[test]
    fn jitter_stays_within_a_pixel() {
        for i in 0..16 {
            let (x, y) = halton_jitter(i);
            assert!(x.abs() <= 0.5 && y.abs() <= 0.5);
        }
    }
}
