//! Scene-controlled local illumination, cube shadow views, and atmosphere.
use crate::render::View;
use bytemuck::{Pod, Zeroable};
#[cfg(test)]
use glam::{Mat4, Vec3};

pub use verse_engine::lighting::{Light, Lighting, MAX_LIGHTS};
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct Frame {
    pub view: [[f32; 4]; 4],
    pub eye: [f32; 4],
    pub ambient: [f32; 4],
    pub fog: [f32; 4],
    pub meta: [f32; 4],
    pub lights: [[f32; 4]; MAX_LIGHTS * 2],
    pub shadow: [[[f32; 4]; 4]; 24],
}
pub(super) fn frame(view: View, lighting: &Lighting) -> Result<Frame, String> {
    lighting.validate(view)?;
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
    for (i, l) in lighting.lights.iter().enumerate() {
        f.lights[i * 2] = [l.position.x, l.position.y, l.position.z, l.range];
        f.lights[i * 2 + 1] = [l.color.x, l.color.y, l.color.z, l.sample(lighting.time, i)?];
        if i < 4 {
            for (face, matrix) in l.shadow_views()?.iter().enumerate() {
                f.shadow[i * 6 + face] = matrix.to_cols_array_2d();
            }
        }
    }
    Ok(f)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uniform_capacity_admits_all_slots_and_refuses_overflow() {
        let light = Light {
            position: Vec3::ZERO,
            color: Vec3::ONE,
            intensity: 1.0,
            range: 5.0,
        };
        let mut lighting = Lighting::default();
        lighting.lights = vec![light; MAX_LIGHTS];
        let view = View {
            view_proj: Mat4::IDENTITY,
            eye: Vec3::ZERO,
        };
        let frame_data = frame(view, &lighting).unwrap();
        assert_eq!(frame_data.meta[0], MAX_LIGHTS as f32);
        assert_eq!(
            frame_data.lights[MAX_LIGHTS * 2 - 1][3],
            1.0 + 0.04 * ((MAX_LIGHTS - 1) as f32).sin()
                + 0.025 * (((MAX_LIGHTS - 1) * 2) as f32).sin()
        );
        lighting.lights.push(light);
        assert!(frame(view, &lighting).is_err());
    }
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
