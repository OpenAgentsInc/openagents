//! Scene-controlled local illumination, cube shadow views, and atmosphere.
use crate::render::View;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// A point source in meters. The first four sources receive cube shadow maps.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
}
/// Linear scene lighting, independent of the UI layer.
#[derive(Clone, Debug)]
pub struct Lighting {
    pub ambient: Vec3,
    pub exposure: f32,
    pub fog: Vec3,
    pub density: f32,
    pub time: f32,
    pub lights: Vec<Light>,
    pub shadowed: usize,
}
impl Default for Lighting {
    fn default() -> Self {
        Self {
            ambient: Vec3::splat(0.025),
            exposure: 1.0,
            fog: Vec3::new(0.009, 0.012, 0.016),
            density: 0.008,
            time: 0.0,
            lights: vec![],
            shadowed: 4,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct Frame {
    pub view: [[f32; 4]; 4],
    pub eye: [f32; 4],
    pub ambient: [f32; 4],
    pub fog: [f32; 4],
    pub meta: [f32; 4],
    pub lights: [[f32; 4]; 16],
    pub shadow: [[[f32; 4]; 4]; 24],
}
pub(super) fn frame(view: View, lighting: &Lighting) -> Result<Frame, String> {
    if lighting.lights.len() > 8
        || !lighting.ambient.is_finite()
        || !lighting.fog.is_finite()
        || !lighting.exposure.is_finite()
        || lighting.exposure <= 0.0
        || !lighting.density.is_finite()
        || lighting.density < 0.0
        || !lighting.time.is_finite()
    {
        return Err("Invalid imported lighting".into());
    }
    let mut f = Frame::zeroed();
    f.view = view.view_proj.to_cols_array_2d();
    f.eye = [view.eye.x, view.eye.y, view.eye.z, 0.0];
    f.ambient = [
        lighting.ambient.x,
        lighting.ambient.y,
        lighting.ambient.z,
        lighting.exposure,
    ];
    f.fog = [
        lighting.fog.x,
        lighting.fog.y,
        lighting.fog.z,
        lighting.density,
    ];
    f.meta = [
        lighting.lights.len() as f32,
        lighting.time,
        lighting.lights.len().min(lighting.shadowed).min(4) as f32,
        0.0,
    ];
    let directions = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
    let ups = [-Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z, -Vec3::Y, -Vec3::Y];
    for (i, l) in lighting.lights.iter().enumerate() {
        if !l.position.is_finite()
            || !l.color.is_finite()
            || !l.intensity.is_finite()
            || l.intensity < 0.0
            || !l.range.is_finite()
            || l.range <= 0.2
        {
            return Err("Invalid point source".into());
        }
        f.lights[i * 2] = [l.position.x, l.position.y, l.position.z, l.range];
        let flicker = 1.0
            + 0.04 * (lighting.time * 13.0 + i as f32).sin()
            + 0.025 * (lighting.time * 19.0 + i as f32 * 2.0).sin();
        f.lights[i * 2 + 1] = [l.color.x, l.color.y, l.color.z, l.intensity * flicker];
        if i < 4 {
            for face in 0..6 {
                f.shadow[i * 6 + face] =
                    (Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.15, l.range)
                        * Mat4::look_to_rh(l.position, directions[face], ups[face]))
                    .to_cols_array_2d();
            }
        }
    }
    Ok(f)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_cube_faces_look_outward_and_bad_lights_are_refused() {
        let mut lighting = Lighting::default();
        lighting.lights.push(Light {
            position: Vec3::ZERO,
            color: Vec3::ONE,
            intensity: 12.0,
            range: 20.0,
        });
        let view = View {
            view_proj: Mat4::IDENTITY,
            eye: Vec3::ZERO,
        };
        let f = frame(view, &lighting).unwrap();
        for (i, d) in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z]
            .iter()
            .enumerate()
        {
            let p = Mat4::from_cols_array_2d(&f.shadow[i]) * (*d * 2.0).extend(1.0);
            assert!(p.w > 0.0);
            assert!(p.x.abs() < 1e-5 && p.y.abs() < 1e-5);
            assert!(p.z / p.w > 0.0 && p.z / p.w < 1.0);
        }
        lighting.lights[0].range = 0.0;
        assert!(frame(view, &lighting).is_err());
    }
}
