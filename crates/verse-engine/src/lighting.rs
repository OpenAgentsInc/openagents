//! Portable local illumination, atmosphere, cube-shadow camera construction,
//! and the color grade every zone's output pass applies.
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
    /// Fog that thins with height, toward `fog`. Without it, fog is uniform
    /// at `density` per meter of distance.
    pub height_fog: Option<HeightFog>,
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
            height_fog: None,
        }
    }
}

impl Lighting {
    /// The fog the scene draws: its height fog, or uniform distance fog at
    /// `density`.
    #[must_use]
    pub fn fog_shape(&self) -> HeightFog {
        self.height_fog
            .unwrap_or_else(|| HeightFog::distance(self.density))
    }
}

/// Exponential height fog: density falls off exponentially with height, so
/// the optical depth along a straight view ray has a closed form (Wenzel,
/// "Real-Time Atmospheric Effects in Games Revisited", GDC 2007; Quílez,
/// "Better Fog"). Fog begins `start` meters from the eye, never exceeds
/// `max_opacity`, and scatters sunlight toward the eye in a lobe around the
/// Sun's direction.
///
/// The shaders evaluate the same functions; [`HeightFog::uniform`] packs the
/// parameters they read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeightFog {
    /// Extinction per meter at height `base`.
    pub density: f32,
    /// The height where the density is `density`, meters.
    pub base: f32,
    /// How fast density falls with height, per meter; zero is uniform fog.
    pub falloff: f32,
    /// Distance from the eye where fog begins, meters.
    pub start: f32,
    /// The most opacity fog reaches, 0 to 1.
    pub max_opacity: f32,
    /// In-scattered sunlight looking straight at the Sun, as a multiple of
    /// the Sun's tint.
    pub sun_strength: f32,
    /// The lobe's sharpness: the power of the cosine to the Sun.
    pub sun_exponent: f32,
}

/// The largest magnitude an exponent in the fog's closed form may take, so
/// single-precision exponentials stay finite.
const FOG_EXPONENT_LIMIT: f32 = 80.0;

impl HeightFog {
    /// Uniform distance fog: `density` per meter everywhere, from the eye,
    /// up to full opacity, without a sun lobe.
    #[must_use]
    pub const fn distance(density: f32) -> Self {
        Self {
            density,
            base: 0.0,
            falloff: 0.0,
            start: 0.0,
            max_opacity: 1.0,
            sun_strength: 0.0,
            sun_exponent: 1.0,
        }
    }

    /// Refuses non-finite or out-of-range parameters.
    pub fn validate(&self) -> Result<(), String> {
        let values = [
            self.density,
            self.base,
            self.falloff,
            self.start,
            self.max_opacity,
            self.sun_strength,
            self.sun_exponent,
        ];
        if values.iter().any(|v| !v.is_finite())
            || !(0.0..=1.0).contains(&self.density)
            || self.base.abs() > 10_000.0
            || !(0.0..=10.0).contains(&self.falloff)
            || !(0.0..=10_000.0).contains(&self.start)
            || !(0.0..=1.0).contains(&self.max_opacity)
            || !(0.0..=16.0).contains(&self.sun_strength)
            || !(1.0..=256.0).contains(&self.sun_exponent)
        {
            return Err("Invalid height fog".into());
        }
        Ok(())
    }

    /// The fog's optical depth between `eye` and `point`: the integral of
    /// `density × exp(−falloff × (y − base))` along the ray from where fog
    /// starts.
    #[must_use]
    pub fn optical_depth(&self, eye: Vec3, point: Vec3) -> f32 {
        let ray = point - eye;
        let length = ray.length();
        let travel = length - self.start;
        if travel <= 0.0 || self.density <= 0.0 {
            return 0.0;
        }
        // The ray from the start distance to the point.
        let first = eye.y + ray.y * (self.start / length);
        let rise = ray.y * (travel / length);
        let at_start = self.density
            * (-self.falloff * (first - self.base))
                .clamp(-FOG_EXPONENT_LIMIT, FOG_EXPONENT_LIMIT)
                .exp();
        // The mean density along the ray relative to its start:
        // (1 − e^(−k)) / k, which tends to 1 − k / 2 as k vanishes.
        let k = (self.falloff * rise).clamp(-FOG_EXPONENT_LIMIT, FOG_EXPONENT_LIMIT);
        let shape = if k.abs() > 1e-4 {
            (1.0 - (-k).exp()) / k
        } else {
            1.0 - 0.5 * k
        };
        at_start * travel * shape
    }

    /// How much of the view toward `point` the fog covers, 0 to
    /// `max_opacity`.
    #[must_use]
    pub fn opacity(&self, eye: Vec3, point: Vec3) -> f32 {
        (1.0 - (-self.optical_depth(eye, point)).exp()).min(self.max_opacity)
    }

    /// The fog's opacity on a stage whose world ends at `end` meters: full
    /// fog over the last 30% of that horizontal distance, so the zone's fog
    /// range still hides the world's edge.
    #[must_use]
    pub fn stage_opacity(&self, eye: Vec3, point: Vec3, end: f32) -> f32 {
        let distance = glam::Vec2::new(point.x - eye.x, point.z - eye.z).length();
        let x = ((distance - 0.7 * end) / (0.3 * end).max(1e-3)).clamp(0.0, 1.0);
        self.opacity(eye, point).max(x * x * (3.0 - 2.0 * x))
    }

    /// In-scattered sunlight along unit view direction `view` toward the
    /// unit Sun direction `sun`, as a multiple of the Sun's tint.
    #[must_use]
    pub fn sun_lobe(&self, view: Vec3, sun: Vec3) -> f32 {
        self.sun_strength * view.dot(sun).max(0.0).powf(self.sun_exponent)
    }

    /// The parameters as the shaders read them: density, base, falloff, and
    /// start; then the opacity cap, the lobe's strength and exponent, and 1
    /// to mark the fog present.
    #[must_use]
    pub fn uniform(&self) -> [[f32; 4]; 2] {
        [
            [self.density, self.base, self.falloff, self.start],
            [self.max_opacity, self.sun_strength, self.sun_exponent, 1.0],
        ]
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
        if let Some(fog) = &self.height_fog {
            fog.validate()?;
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

/// Edge length of the color-grading lookup table, in texels.
pub const GRADE_LUT_SIZE: usize = 32;
/// The table's shaper is linear below this scene value and logarithmic above.
pub const GRADE_LUT_FLOOR: f32 = 1.0 / 1024.0;
/// The scene value the table's last texel holds; brighter values clamp to it.
pub const GRADE_LUT_TOP: f32 = 256.0;

/// Rec. 709 luminance weights for linear scene color.
const LUMA: Vec3 = Vec3::new(0.2126, 0.7152, 0.0722);
/// Mid-grey, the pivot of a contrast change.
const MID_GREY: f32 = 0.18;

/// The shaper's span: the number of stops between the floor and the top.
#[must_use]
pub fn grade_lut_range() -> f32 {
    (GRADE_LUT_TOP / GRADE_LUT_FLOOR + 1.0).log2()
}

/// A linear scene value as a table coordinate in `[0, 1]`.
///
/// The shaper is `log2(x / floor + 1)`, normalized so the top maps to 1. Zero
/// maps to zero, so black stays black, and the table spends its texels
/// evenly across stops above the floor.
#[must_use]
pub fn grade_lut_shape(x: f32) -> f32 {
    ((x.max(0.0) / GRADE_LUT_FLOOR + 1.0).log2() / grade_lut_range()).clamp(0.0, 1.0)
}

/// The linear scene value at table coordinate `u`, the inverse of
/// [`grade_lut_shape`].
#[must_use]
pub fn grade_lut_unshape(u: f32) -> f32 {
    GRADE_LUT_FLOOR * ((u.clamp(0.0, 1.0) * grade_lut_range()).exp2() - 1.0)
}

/// The tone curve that maps graded scene color to display color.
///
/// Both curves scale a color by its peak channel, so they keep its hue. A
/// table the size of [`GRADE_LUT_SIZE`] interpolates such a curve poorly at
/// the shoulder, so the output pass evaluates the curve analytically after
/// the table; a per-channel curve could be baked into the table instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    /// Khronos PBR Neutral, its shoulder generalized to approach `ceiling`.
    Neutral,
    /// A hue-preserving shoulder alone, for neon stages: a bright amber core
    /// compresses along its own hue instead of washing toward white.
    HueShoulder,
}

impl Curve {
    /// The display color for a graded scene color, at most `ceiling`: 1 on a
    /// standard display, the headroom over reference white on an
    /// extended-range one. The output pass's WGSL is the GPU form of this.
    #[must_use]
    pub fn apply(self, color: Vec3, ceiling: f32) -> Vec3 {
        let color = color.max(Vec3::ZERO);
        let start = 0.8 - 0.04;
        match self {
            Self::Neutral => {
                let x = color.min_element();
                let offset = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
                let c = color - Vec3::splat(offset);
                let peak = c.max_element();
                if peak < start {
                    return c;
                }
                let d = ceiling - start;
                let new_peak = ceiling - d * d / (peak + d - start);
                let c = c * (new_peak / peak);
                let g = 1.0 - 1.0 / (0.15 * (peak - new_peak) + 1.0);
                c.lerp(Vec3::splat(new_peak), g)
            }
            Self::HueShoulder => {
                let peak = color.max_element();
                if peak < start {
                    return color;
                }
                let d = ceiling - start;
                color * ((ceiling - d * d / (peak + d - start)) / peak)
            }
        }
    }
}

/// A zone's color grade: what the output pass does to exposed scene color
/// before it reaches the display.
///
/// The scene-referred part (exposure offset, white balance, saturation,
/// contrast, and color gain) is baked into a 3D lookup table, so a new look
/// is data rather than shader code. The curve and its ceiling are evaluated
/// analytically after the table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grade {
    /// Exposure offset, stops, applied first.
    pub exposure: f32,
    /// Per-channel white-balance gains on linear scene color.
    pub balance: Vec3,
    /// Saturation about Rec. 709 luminance: 1 keeps color, 0 is grey.
    pub saturation: f32,
    /// Contrast about mid-grey, per channel in log space: 1 keeps it.
    pub contrast: f32,
    /// A per-channel color filter after contrast.
    pub gain: Vec3,
    pub curve: Curve,
    /// The display's highest value: 1 in standard range, the headroom over
    /// reference white on an extended-range display.
    pub ceiling: f32,
}

impl Grade {
    /// No grade: the Neutral curve on exposed scene color.
    pub const NEUTRAL: Self = Self {
        exposure: 0.0,
        balance: Vec3::ONE,
        saturation: 1.0,
        contrast: 1.0,
        gain: Vec3::ONE,
        curve: Curve::Neutral,
        ceiling: 1.0,
    };

    /// The neon stages (the plaza and Everglade): the hue-preserving shoulder,
    /// standard range even on an extended-range display.
    pub const STAGE: Self = Self {
        curve: Curve::HueShoulder,
        ..Self::NEUTRAL
    };

    /// The summoning chamber. Its fragments used to tone-map themselves with
    /// an ACES curve fit, which lifts mid-grey to 0.267; Neutral maps 0.18 to
    /// 0.14. The 0.77-stop offset keeps the chamber's mid-tones where the
    /// retired curve put them, so its authored light levels still read.
    pub const CHAMBER: Self = Self {
        exposure: 0.77,
        ..Self::NEUTRAL
    };

    /// Refuses non-finite or out-of-range values before a table is baked.
    pub fn validate(&self) -> Result<(), String> {
        let finite = self.exposure.is_finite()
            && self.balance.is_finite()
            && self.saturation.is_finite()
            && self.contrast.is_finite()
            && self.gain.is_finite()
            && self.ceiling.is_finite();
        if !finite
            || self.exposure.abs() > 16.0
            || self.balance.min_element() <= 0.0
            || self.gain.min_element() < 0.0
            || self.saturation < 0.0
            || self.contrast <= 0.0
            || self.ceiling < 1.0
        {
            return Err("Invalid color grade".into());
        }
        Ok(())
    }

    /// Whether `other` bakes the same table: the curve and ceiling are
    /// uniforms, not table contents.
    #[must_use]
    pub fn same_table(&self, other: &Self) -> bool {
        self.exposure == other.exposure
            && self.balance == other.balance
            && self.saturation == other.saturation
            && self.contrast == other.contrast
            && self.gain == other.gain
    }

    /// The scene-referred grade of one linear color: the function the table
    /// holds.
    #[must_use]
    pub fn scene(&self, color: Vec3) -> Vec3 {
        // Identity terms are skipped, so an identity grade is exact.
        let mut c = color.max(Vec3::ZERO) * self.balance;
        if self.exposure != 0.0 {
            c *= self.exposure.exp2();
        }
        if self.saturation != 1.0 {
            let luma = Vec3::splat(c.dot(LUMA));
            c = (luma + (c - luma) * self.saturation).max(Vec3::ZERO);
        }
        let c = if self.contrast == 1.0 {
            c
        } else {
            let pivot = |v: f32| {
                if v > 0.0 {
                    MID_GREY * (v / MID_GREY).powf(self.contrast)
                } else {
                    0.0
                }
            };
            Vec3::new(pivot(c.x), pivot(c.y), pivot(c.z))
        };
        c * self.gain
    }

    /// The full transform of one exposed scene color to display color,
    /// without the table's interpolation.
    #[must_use]
    pub fn display(&self, color: Vec3) -> Vec3 {
        self.curve.apply(self.scene(color), self.ceiling)
    }

    /// Bakes the scene-referred grade into a [`GRADE_LUT_SIZE`]³ table of
    /// RGBA texels, red fastest, then green, then blue.
    ///
    /// Each texel holds the grade's change to its shaped input, not the graded
    /// color, so an identity grade is exact under trilinear filtering and a
    /// mild grade carries only a small interpolation error.
    #[must_use]
    pub fn bake(&self) -> Vec<[f32; 4]> {
        let n = GRADE_LUT_SIZE;
        let axis: Vec<f32> = (0..n)
            .map(|i| grade_lut_unshape(i as f32 / (n - 1) as f32))
            .collect();
        let mut texels = Vec::with_capacity(n * n * n);
        for &b in &axis {
            for &g in &axis {
                for &r in &axis {
                    let input = Vec3::new(r, g, b);
                    let change = self.scene(input) - input;
                    texels.push([change.x, change.y, change.z, 0.0]);
                }
            }
        }
        texels
    }
}

/// The graded scene color as the output pass reads it from a baked table:
/// trilinear filtering between texel centers, as the GPU samples it.
///
/// # Panics
///
/// Panics when `table` is not [`GRADE_LUT_SIZE`]³ texels.
#[must_use]
pub fn sample_grade_lut(table: &[[f32; 4]], color: Vec3) -> Vec3 {
    let n = GRADE_LUT_SIZE;
    assert_eq!(table.len(), n * n * n, "grade table size");
    let color = color.max(Vec3::ZERO);
    let mut base = [0usize; 3];
    let mut t = [0.0f32; 3];
    for (k, value) in color.to_array().into_iter().enumerate() {
        let position = grade_lut_shape(value) * (n - 1) as f32;
        base[k] = (position.floor() as usize).min(n - 2);
        t[k] = position - base[k] as f32;
    }
    let mut change = Vec3::ZERO;
    for corner in 0..8 {
        let step = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
        let mut weight = 1.0;
        for (s, f) in step.iter().zip(t) {
            weight *= if *s == 1 { f } else { 1.0 - f };
        }
        let index = (base[0] + step[0]) + n * ((base[1] + step[1]) + n * (base[2] + step[2]));
        let texel = table[index];
        change += Vec3::new(texel[0], texel[1], texel[2]) * weight;
    }
    color + change
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

    /// A mild grade with every table term away from identity.
    fn graded() -> Grade {
        Grade {
            exposure: 0.77,
            balance: Vec3::new(1.1, 1.0, 0.85),
            saturation: 0.9,
            contrast: 1.05,
            gain: Vec3::new(1.0, 0.97, 0.92),
            ..Grade::NEUTRAL
        }
    }

    /// Colors from deep shadow to bright highlights in three hues.
    fn test_colors() -> Vec<Vec3> {
        (0..200)
            .flat_map(|step| {
                let x = 0.0005 * (step as f32 / 200.0 * 18.0).exp2();
                [
                    Vec3::splat(x),
                    Vec3::new(x, x * 0.5, x * 0.1),
                    Vec3::new(x * 0.2, x, x * 0.6),
                    Vec3::new(0.0, x, 0.0),
                ]
            })
            .collect()
    }

    #[test]
    fn the_table_shaper_keeps_black_and_round_trips() {
        assert_eq!(grade_lut_shape(0.0), 0.0);
        assert_eq!(grade_lut_shape(-1.0), 0.0);
        assert_eq!(grade_lut_unshape(0.0), 0.0);
        assert!((grade_lut_shape(GRADE_LUT_TOP) - 1.0).abs() < 1e-6);
        assert!((grade_lut_unshape(1.0) - GRADE_LUT_TOP).abs() < GRADE_LUT_TOP * 1e-5);
        assert_eq!(grade_lut_shape(GRADE_LUT_TOP * 4.0), 1.0);
        let mut previous = -1.0;
        for step in 0..400 {
            let x = 1e-4 * (step as f32 / 400.0 * 21.0).exp2();
            let u = grade_lut_shape(x);
            assert!(u > previous, "the shaper rises at {x}");
            previous = u;
            if x <= GRADE_LUT_TOP {
                let back = grade_lut_unshape(u);
                assert!(
                    (back - x).abs() <= x * 1e-4 + 1e-7,
                    "{x} came back as {back}"
                );
            }
        }
    }

    #[test]
    fn an_identity_grade_bakes_an_exact_table() {
        let table = Grade::NEUTRAL.bake();
        assert_eq!(
            table.len(),
            GRADE_LUT_SIZE * GRADE_LUT_SIZE * GRADE_LUT_SIZE
        );
        assert!(table.iter().all(|texel| *texel == [0.0; 4]));
        for color in test_colors() {
            assert_eq!(sample_grade_lut(&table, color), color);
        }
        assert_eq!(Grade::STAGE.bake(), table);
    }

    #[test]
    fn a_baked_grade_matches_the_analytic_grade() {
        let grade = graded();
        let table = grade.bake();
        let n = GRADE_LUT_SIZE;
        // At texel centers the table holds the grade itself.
        for (r, g, b) in [
            (0, 0, 0),
            (5, 9, 2),
            (17, 17, 17),
            (24, 30, 11),
            (31, 31, 31),
        ] {
            let at = |i: usize| grade_lut_unshape(i as f32 / (n - 1) as f32);
            let color = Vec3::new(at(r), at(g), at(b));
            let expected = grade.scene(color);
            let sampled = sample_grade_lut(&table, color);
            let tolerance = expected.max_element() * 1e-4 + 1e-6;
            assert!(
                (sampled - expected).abs().max_element() <= tolerance,
                "{color} graded to {sampled}, not {expected}"
            );
        }
        // Between texels, trilinear filtering stays within 3% of the
        // brightest channel.
        for color in test_colors() {
            let expected = grade.scene(color);
            let sampled = sample_grade_lut(&table, color);
            let tolerance = expected.max_element() * 0.03 + 1e-4;
            assert!(
                (sampled - expected).abs().max_element() <= tolerance,
                "{color} graded to {sampled}, not {expected}"
            );
        }
    }

    #[test]
    fn grade_terms_do_what_they_name() {
        let grey = Vec3::splat(0.18);
        let brighter = Grade {
            exposure: 1.0,
            ..Grade::NEUTRAL
        };
        assert!(
            (brighter.scene(grey) - Vec3::splat(0.36))
                .abs()
                .max_element()
                < 1e-6
        );
        let flat = Grade {
            saturation: 0.0,
            ..Grade::NEUTRAL
        };
        let red = flat.scene(Vec3::new(1.0, 0.0, 0.0));
        assert!((red - Vec3::splat(0.2126)).abs().max_element() < 1e-6);
        // Contrast pivots on mid-grey.
        let punchy = Grade {
            contrast: 1.5,
            ..Grade::NEUTRAL
        };
        assert!((punchy.scene(grey) - grey).abs().max_element() < 1e-6);
        assert!(punchy.scene(Vec3::splat(0.36)).x > 0.36);
        assert!(punchy.scene(Vec3::splat(0.09)).x < 0.09);
        // White balance is a per-channel gain, and black stays black.
        let warm = Grade {
            balance: Vec3::new(1.2, 1.0, 0.8),
            ..Grade::NEUTRAL
        };
        assert!(
            (warm.scene(Vec3::ONE) - Vec3::new(1.2, 1.0, 0.8))
                .abs()
                .max_element()
                < 1e-6
        );
        assert_eq!(graded().scene(Vec3::ZERO), Vec3::ZERO);
    }

    #[test]
    fn output_curves_keep_black_hue_and_their_ceiling() {
        for curve in [Curve::Neutral, Curve::HueShoulder] {
            assert_eq!(curve.apply(Vec3::ZERO, 1.0), Vec3::ZERO);
            for ceiling in [1.0, 4.0, 16.0] {
                let mut previous = -1.0;
                for step in 0..300 {
                    let x = 1e-3 * (step as f32 / 300.0 * 20.0).exp2();
                    let y = curve.apply(Vec3::splat(x), ceiling).x;
                    assert!(y >= previous, "{curve:?} falls at {x}");
                    assert!(y <= ceiling, "{curve:?} passes its ceiling at {x}");
                    previous = y;
                }
            }
        }
        // Below the shoulder, Neutral subtracts its small toe offset.
        let mid = Curve::Neutral.apply(Vec3::splat(0.5), 1.0);
        assert!((mid - Vec3::splat(0.46)).abs().max_element() < 1e-6);
        // The hue-preserving shoulder keeps channel ratios.
        let amber = Curve::HueShoulder.apply(Vec3::new(8.0, 4.0, 1.0), 1.0);
        assert!((amber.x / amber.y - 2.0).abs() < 1e-5);
        assert!((amber.x / amber.z - 8.0).abs() < 1e-4);
        assert!(amber.x < 1.0);
    }

    #[test]
    fn the_chamber_grade_keeps_the_retired_curves_mid_grey() {
        // The ACES curve fit the chamber's fragments used to apply.
        let aces = |x: f32| (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
        let retired = aces(0.18);
        let graded = Grade::CHAMBER.display(Vec3::splat(0.18));
        assert!(
            (graded.x - retired).abs() < 0.003,
            "{graded} against {retired}"
        );
        let plain = Grade::NEUTRAL.display(Vec3::splat(0.18));
        assert!((plain.x - 0.14).abs() < 1e-6);
    }

    #[test]
    fn grades_refuse_invalid_values_and_compare_tables() {
        for grade in [Grade::NEUTRAL, Grade::STAGE, Grade::CHAMBER, graded()] {
            assert!(grade.validate().is_ok());
        }
        let bad = [
            Grade {
                exposure: f32::NAN,
                ..Grade::NEUTRAL
            },
            Grade {
                balance: Vec3::new(1.0, 0.0, 1.0),
                ..Grade::NEUTRAL
            },
            Grade {
                saturation: -0.5,
                ..Grade::NEUTRAL
            },
            Grade {
                contrast: 0.0,
                ..Grade::NEUTRAL
            },
            Grade {
                ceiling: 0.5,
                ..Grade::NEUTRAL
            },
            Grade {
                gain: Vec3::new(1.0, f32::INFINITY, 1.0),
                ..Grade::NEUTRAL
            },
        ];
        for grade in bad {
            assert!(grade.validate().is_err(), "{grade:?}");
        }
        let hdr = Grade {
            ceiling: 4.0,
            ..Grade::STAGE
        };
        assert!(hdr.same_table(&Grade::NEUTRAL));
        assert!(!Grade::CHAMBER.same_table(&Grade::NEUTRAL));
    }

    fn glade_fog() -> HeightFog {
        HeightFog {
            density: 0.005,
            base: 0.0,
            falloff: 0.12,
            start: 40.0,
            max_opacity: 0.92,
            sun_strength: 0.4,
            sun_exponent: 3.0,
        }
    }

    /// The trapezoid rule along the ray, against which the closed form is
    /// checked.
    fn integrated_depth(fog: &HeightFog, eye: Vec3, point: Vec3) -> f32 {
        let ray = point - eye;
        let length = ray.length();
        let density = |t: f32| {
            if t * length < fog.start {
                0.0
            } else {
                let y = eye.y + ray.y * t;
                fog.density * (-fog.falloff * (y - fog.base)).exp()
            }
        };
        let steps = 4_000;
        let mut depth = 0.0;
        for i in 0..steps {
            let a = i as f32 / steps as f32;
            let b = (i + 1) as f32 / steps as f32;
            depth += (density(a) + density(b)) * 0.5 * (b - a) * length;
        }
        depth
    }

    #[test]
    fn height_fog_has_the_closed_form_of_its_density() {
        let fog = glade_fog();
        let eye = Vec3::new(0.0, 1.7, 0.0);
        for point in [
            Vec3::new(120.0, 0.0, 0.0),
            Vec3::new(0.0, 9.0, 150.0),
            Vec3::new(60.0, 40.0, 60.0),
            Vec3::new(-30.0, -5.0, 80.0),
            Vec3::new(100.0, 1.7, 0.0),
        ] {
            let closed = fog.optical_depth(eye, point);
            let numeric = integrated_depth(&fog, eye, point);
            assert!(
                (closed - numeric).abs() < 1e-3 * numeric.max(1.0),
                "{point}: {closed} against {numeric}"
            );
        }
    }

    #[test]
    fn uniform_distance_fog_is_the_chambers_exponential() {
        let fog = HeightFog::distance(0.012);
        let eye = Vec3::new(1.0, 2.0, 3.0);
        for point in [Vec3::new(30.0, 2.0, 3.0), Vec3::new(1.0, 50.0, -40.0)] {
            let expected = 1.0 - (-(point - eye).length() * 0.012).exp();
            assert!((fog.opacity(eye, point) - expected).abs() < 1e-6);
        }
        let mut lighting = Lighting::default();
        assert_eq!(lighting.fog_shape(), HeightFog::distance(lighting.density));
        lighting.height_fog = Some(glade_fog());
        assert_eq!(lighting.fog_shape(), glade_fog());
    }

    #[test]
    fn height_fog_starts_late_caps_and_thins_with_height() {
        let fog = glade_fog();
        let eye = Vec3::new(0.0, 1.7, 0.0);
        // Nothing nearer than the start distance.
        assert_eq!(fog.opacity(eye, Vec3::new(0.0, 0.0, 39.0)), 0.0);
        assert!(fog.opacity(eye, Vec3::new(0.0, 0.0, 80.0)) > 0.0);
        // The cap holds however far the ray runs through dense fog.
        let dense = HeightFog {
            density: 0.5,
            ..fog
        };
        assert_eq!(dense.opacity(eye, Vec3::new(0.0, 0.0, 900.0)), 0.92);
        // At equal distance, a hilltop is clearer than the valley floor, and
        // an eye up on the hill sees less fog than one in the hollow.
        let floor = fog.opacity(eye, Vec3::new(0.0, 0.0, 120.0));
        let hill = fog.opacity(eye, Vec3::new(0.0, 10.0, 120.0));
        assert!(hill < floor, "{hill} {floor}");
        let high_eye = Vec3::new(0.0, 11.7, 0.0);
        let from_hill = fog.opacity(high_eye, Vec3::new(0.0, 10.0, 120.0));
        assert!(from_hill < hill, "{from_hill} {hill}");
        // The stage's range is still the limit: fog is total at its end.
        let edge = fog.stage_opacity(eye, Vec3::new(0.0, 0.0, 180.0), 170.0);
        assert_eq!(edge, 1.0);
        let near = Vec3::new(0.0, 0.0, 100.0);
        assert_eq!(fog.stage_opacity(eye, near, 170.0), fog.opacity(eye, near));
    }

    #[test]
    fn the_sun_lobe_glows_toward_the_sun_only() {
        let fog = glade_fog();
        let sun = Vec3::new(-0.35, 0.8, -0.45).normalize();
        let toward = Vec3::new(sun.x, 0.0, sun.z).normalize();
        assert!((fog.sun_lobe(sun, sun) - fog.sun_strength).abs() < 1e-5);
        assert!(fog.sun_lobe(toward, sun) > fog.sun_lobe(-toward, sun));
        assert_eq!(fog.sun_lobe(-sun, sun), 0.0);
        let packed = fog.uniform();
        assert_eq!(packed[0], [0.005, 0.0, 0.12, 40.0]);
        assert_eq!(packed[1], [0.92, 0.4, 3.0, 1.0]);
    }

    #[test]
    fn invalid_height_fog_is_refused() {
        assert!(glade_fog().validate().is_ok());
        assert!(HeightFog::distance(0.008).validate().is_ok());
        let bad = [
            HeightFog {
                density: f32::NAN,
                ..glade_fog()
            },
            HeightFog {
                falloff: -0.1,
                ..glade_fog()
            },
            HeightFog {
                max_opacity: 1.5,
                ..glade_fog()
            },
            HeightFog {
                sun_exponent: 0.0,
                ..glade_fog()
            },
        ];
        for fog in bad {
            assert!(fog.validate().is_err(), "{fog:?}");
        }
        let lighting = Lighting {
            height_fog: Some(HeightFog {
                start: -1.0,
                ..glade_fog()
            }),
            ..Lighting::default()
        };
        assert!(lighting.validate(view()).is_err());
    }
}
