//! Portable local illumination, atmosphere, and cube-shadow camera construction.
use crate::presentation::View;
use glam::{Mat4, Vec3};

pub const MAX_LIGHTS: usize = 32;

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

impl Lighting {
    pub fn shadow_count(&self) -> usize {
        self.lights.len().min(self.shadowed).min(4)
    }
    pub fn validate(&self, view: View) -> Result<(), String> {
        view.validate()?;
        if self.lights.len() > MAX_LIGHTS
            || !self.ambient.is_finite()
            || self.ambient.min_element() < 0.
            || !self.fog.is_finite()
            || self.fog.min_element() < 0.
            || !self.exposure.is_finite()
            || self.exposure <= 0.
            || !self.density.is_finite()
            || self.density < 0.
            || !self.time.is_finite()
        {
            return Err("Invalid scene illumination".into());
        }
        for (index, light) in self.lights.iter().enumerate() {
            light.sample(self.time, index)?;
            if index < self.shadow_count() {
                light.shadow_views()?;
            }
        }
        Ok(())
    }
}
impl Light {
    /// Preserve the authored two-frequency local-light flicker.
    pub fn sample(&self, time: f32, index: usize) -> Result<f32, String> {
        if !self.position.is_finite()
            || !self.color.is_finite()
            || self.color.min_element() < 0.
            || !self.intensity.is_finite()
            || self.intensity < 0.
            || !self.range.is_finite()
            || self.range <= 0.2
            || !time.is_finite()
        {
            return Err("Invalid point source".into());
        }
        let flicker = 1.0
            + 0.04 * (time * 13.0 + index as f32).sin()
            + 0.025 * (time * 19.0 + index as f32 * 2.0).sin();
        let intensity = self.intensity * flicker;
        if !intensity.is_finite() {
            return Err("Point source sampling overflow".into());
        }
        Ok(intensity)
    }
    /// Cube faces use +X, -X, +Y, -Y, +Z, -Z ordering and right-handed depth.
    pub fn shadow_views(&self) -> Result<[Mat4; 6], String> {
        self.sample(0., 0)?;
        let directions = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
        let ups = [-Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z, -Vec3::Y, -Vec3::Y];
        let projection = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.15, self.range);
        let views = std::array::from_fn(|face| {
            projection * Mat4::look_to_rh(self.position, directions[face], ups[face])
        });
        if views.iter().any(|matrix| !matrix.is_finite()) {
            return Err("Point source shadow projection overflow".into());
        }
        Ok(views)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> View {
        View {
            view_proj: Mat4::IDENTITY,
            eye: Vec3::ZERO,
        }
    }
    fn light() -> Light {
        Light {
            position: Vec3::ZERO,
            color: Vec3::ONE,
            intensity: 12.,
            range: 20.,
        }
    }
    #[test]
    fn every_cube_face_projects_outward_with_finite_depth() {
        let light = light();
        for (matrix, direction) in light.shadow_views().unwrap().iter().zip([
            Vec3::X,
            -Vec3::X,
            Vec3::Y,
            -Vec3::Y,
            Vec3::Z,
            -Vec3::Z,
        ]) {
            let p = *matrix * (direction * 2.).extend(1.);
            assert!(p.w > 0. && p.x.abs() < 1e-5 && p.y.abs() < 1e-5);
            assert!(p.z / p.w > 0. && p.z / p.w < 1.);
        }
    }
    #[test]
    fn capacity_and_derived_overflow_are_admitted_before_submission() {
        let mut lighting = Lighting::default();
        lighting.lights = vec![light(); MAX_LIGHTS];
        assert!(lighting.validate(view()).is_ok());
        assert_eq!(lighting.shadow_count(), 4);
        lighting.lights.push(light());
        assert!(lighting.validate(view()).is_err());
        lighting.lights.pop();
        lighting.time = f32::MAX;
        assert!(lighting.validate(view()).is_err());
        lighting.time = 0.;
        lighting.lights[0].intensity = f32::MAX;
        lighting.time = 0.1;
        assert!(lighting.validate(view()).is_err());
    }
    #[test]
    fn camera_atmosphere_and_point_sources_reject_invalid_values() {
        let mut v = view();
        v.eye.x = f32::NAN;
        assert!(Lighting::default().validate(v).is_err());
        v = view();
        v.view_proj.x_axis.x = f32::INFINITY;
        assert!(Lighting::default().validate(v).is_err());
        let mut lighting = Lighting::default();
        lighting.fog.x = -1.;
        assert!(lighting.validate(view()).is_err());
        let mut bad = light();
        bad.range = 0.2;
        assert!(bad.shadow_views().is_err());
        bad = light();
        bad.position = Vec3::splat(f32::MAX);
        assert!(bad.shadow_views().is_err());
    }
}
