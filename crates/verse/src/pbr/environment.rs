//! Sky light for daylight stages: the [`Daylight`] sky as diffuse and glossy
//! ambient light.
//!
//! [`air`] is the CPU form of `daylight_air` in `photo.wgsl`, the sky pass's
//! own color along a direction, and [`sky`] adds the clouds at their expected
//! cover in place of their noise. [`SkyLight::bake`] puts the stage's lit
//! ground below the horizon, projects the result onto order-two spherical
//! harmonics, which the lit shader evaluates for diffuse light, and
//! prefilters it into a small cube that glossy surfaces sample at their
//! roughness's level of detail ([`verse_engine::environment`]). The key's
//! `sky` illuminance sets the light's level on surfaces facing up; the sky
//! sets its color and gradient.
//!
//! Both are computed on the CPU, and only when the sky or the Sun changes,
//! so every quality tier, WebGL2 included, draws them without compute
//! shaders or storage buffers. The tier sets only the cube's size.

use std::f32::consts::PI;

use glam::Vec3;
use verse_engine::environment::{Cube, Sh9};
use verse_engine::quality::Quality;

use super::Daylight;

/// Texels along each cube edge of the spherical-harmonic projection.
const PROJECTION_SIZE: u32 = 32;
/// The cloud octaves' sum in `fs_daylight` as a normal distribution: its
/// median is near 0.38 and its 95th percentile near 0.62.
const CLOUD_MEAN: f32 = 0.38;
const CLOUD_SPREAD: f32 = 0.146;
/// The mean of `fs_daylight`'s cloud lighting, `0.62 + 3 (n − lee)`, whose
/// noise difference averages zero.
const CLOUD_LIT: f32 = 0.62;
/// How far below the horizon the ground takes over from the haze, as a
/// direction's downward component.
const GROUND_BAND: f32 = 0.1;

/// What a sky light depends on: the stage's sky and its key light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyInputs {
    pub daylight: Daylight,
    /// Unit direction toward the Sun, which is the key light.
    pub sun: Vec3,
    /// The key's pre-exposed illuminance on a surface facing it.
    pub sun_illuminance: f32,
    /// The pre-exposed irradiance the sky light delivers to a surface facing
    /// straight up: the key's `sky` times its exposure.
    pub level: f32,
}

/// A daylight sky's light: diffuse irradiance and a prefiltered reflection
/// cube, both in pre-exposed luminance.
#[derive(Clone, Debug, PartialEq)]
pub struct SkyLight {
    pub sh: Sh9,
    pub cube: Cube,
}

impl SkyLight {
    /// Bakes the sky light of `inputs` with a reflection cube of `cube_size`
    /// texels and `samples` GGX samples per blurred texel.
    #[must_use]
    pub fn bake(inputs: &SkyInputs, cube_size: u32, samples: u32) -> Self {
        let radiance = radiance(inputs);
        Self {
            sh: Sh9::project(PROJECTION_SIZE, &radiance),
            cube: Cube::prefilter(cube_size, samples, &radiance),
        }
    }
}

/// The radiance the sky light holds along each unit direction: the sky
/// above the horizon, scaled to the key's level, and the ground below it,
/// lit by the key and that sky.
pub fn radiance(inputs: &SkyInputs) -> impl Fn(Vec3) -> Vec3 {
    let day = inputs.daylight;
    let sun = inputs.sun.normalize_or(Vec3::Y);
    let coverage = cloud_coverage(day.clouds);
    let open = Sh9::project(PROJECTION_SIZE, |d| {
        if d.y >= 0.0 {
            sky(&day, sun, coverage, d)
        } else {
            Vec3::ZERO
        }
    })
    .irradiance(Vec3::Y);
    let scale = if luminance(open) > 1e-6 {
        inputs.level.max(0.0) / luminance(open)
    } else {
        0.0
    };
    let lit = Vec3::splat(inputs.sun_illuminance.max(0.0) * sun.y.max(0.0)) + open * scale;
    let ground = Vec3::from(day.ground) * lit / PI;
    move |d: Vec3| {
        let above = sky(&day, sun, coverage, d) * scale;
        if d.y >= 0.0 {
            above
        } else {
            above.lerp(ground, (-d.y / GROUND_BAND).min(1.0))
        }
    }
}

/// Rec. 709 luminance of linear color.
fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The shape of the Cornette-Shanks phase function, as `mie_lobe` in
/// `photo.wgsl`.
fn mie_lobe(mu: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = (1.0 + g2 - 2.0 * g * mu).max(1e-4).powf(1.5);
    (1.0 - g2) * (1.0 + mu * mu) / (2.0 * denom)
}

/// The sky without its Sun disc or clouds along unit direction `d`, with the
/// Sun toward unit `sun`: `daylight_air` in `photo.wgsl`. Keep the two in
/// step, so the sky light matches the sky the stage draws.
#[must_use]
pub fn air(day: &Daylight, sun: Vec3, d: Vec3) -> Vec3 {
    let zenith = Vec3::from(day.zenith);
    let horizon = Vec3::from(day.horizon);
    let tint = Vec3::from(day.sun);
    let up = d.y.max(0.0);
    let haze = (1.0 - up).powi(5);
    let mut c = zenith.lerp(horizon, haze);
    let flat_d = (Vec3::new(d.x, 0.0, d.z) + Vec3::new(1e-4, 0.0, 0.0)).normalize();
    let flat_s = (Vec3::new(sun.x, 0.0, sun.z) + Vec3::new(1e-4, 0.0, 0.0)).normalize();
    let toward = flat_d.dot(flat_s) * 0.5 + 0.5;
    c *= 1.0 + 0.12 * haze * (toward - 0.5);
    c = c.lerp(c * tint * 1.08, haze * toward * 0.35);
    let below = (-d.y * 4.0).clamp(0.0, 1.0);
    c = c.lerp(horizon * 0.94, below);
    let mu = d.dot(sun);
    let halo = mie_lobe(mu, 0.76);
    c + tint * (0.025 * halo + 0.22 * mu.max(0.0).powf(48.0))
}

/// The share of the sky `fs_daylight` covers with cloud at cover `cover`:
/// the expected value of its density threshold over the octaves' sum.
#[must_use]
pub fn cloud_coverage(cover: f32) -> f32 {
    let low = 0.56 + (0.24 - 0.56) * cover.clamp(0.0, 1.0);
    let steps = 200;
    let (mut sum, mut weights) = (0.0, 0.0);
    for i in 0..=steps {
        let z = -5.0 + 10.0 * i as f32 / steps as f32;
        let weight = (-0.5 * z * z).exp();
        sum += weight * smoothstep(low, low + 0.2, CLOUD_MEAN + CLOUD_SPREAD * z);
        weights += weight;
    }
    sum / weights
}

/// The daylight sky along unit direction `d` with its clouds at their
/// expected `coverage`: `fs_daylight` without the Sun's disc, the clouds'
/// silver lining, or their noise.
#[must_use]
pub fn sky(day: &Daylight, sun: Vec3, coverage: f32, d: Vec3) -> Vec3 {
    let c = air(day, sun, d);
    let up = d.y.max(0.0);
    let density = coverage * smoothstep(0.02, 0.3, up);
    if density <= 0.0 {
        return c;
    }
    let horizon = Vec3::from(day.horizon);
    let tint = Vec3::from(day.sun);
    let shadowed = horizon * Vec3::new(0.80, 0.84, 0.95);
    let sunlit = Vec3::new(0.97, 0.95, 0.92) * Vec3::ONE.lerp(tint, 0.35);
    let shade = shadowed.lerp(sunlit, CLOUD_LIT);
    let cloud = shade.lerp(c, (1.0 - up).powi(6) * 0.6);
    c.lerp(cloud, density * 0.92)
}

/// The sky light on the GPU: the prefiltered cube, and the irradiance
/// coefficients the frame uniform carries.
pub(crate) struct SkyLightGpu {
    pub view: wgpu::TextureView,
    /// [`Sh9::uniform`] of the baked light.
    pub sh: [[f32; 4]; 9],
    /// The cube's last level, which roughness 1 reads.
    pub max_lod: f32,
    /// What the uploaded light was baked from, with the cube's size and
    /// sample count.
    built: Option<(SkyInputs, u32, u32)>,
}

impl SkyLightGpu {
    /// A black one-texel cube, so the scene's bind group is complete before
    /// any sky lights it.
    pub fn empty(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let black = Cube {
            size: 1,
            levels: vec![vec![Vec3::ZERO; 6]],
        };
        Self {
            view: upload(device, queue, &black),
            sh: [[0.0; 4]; 9],
            max_lod: 0.0,
            built: None,
        }
    }

    /// Bakes and uploads the sky light of `inputs` at `quality`'s cube size,
    /// unless it already holds that light. Returns whether the cube's view
    /// changed, so the caller rebuilds the bind group that holds it.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        inputs: &SkyInputs,
        quality: &Quality,
    ) -> bool {
        let (size, samples) = quality.sky_cube();
        let key = (*inputs, size, samples);
        if self.built == Some(key) {
            return false;
        }
        let light = SkyLight::bake(inputs, size, samples);
        self.view = upload(device, queue, &light.cube);
        self.sh = light.sh.uniform();
        self.max_lod = light.cube.levels.len().saturating_sub(1) as f32;
        self.built = Some(key);
        true
    }
}

/// Uploads a cube's levels as one half-float cube texture.
fn upload(device: &wgpu::Device, queue: &wgpu::Queue, cube: &Cube) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("verse sky light"),
        size: wgpu::Extent3d {
            width: cube.size,
            height: cube.size,
            depth_or_array_layers: 6,
        },
        mip_level_count: cube.levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let half = super::gpu::half;
    for (level, texels) in cube.levels.iter().enumerate() {
        let side = cube.side(level);
        let data: Vec<u16> = texels
            .iter()
            .flat_map(|t| [half(t.x), half(t.y), half(t.z), half(1.0)])
            .collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&data),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(side * 8),
                rows_per_image: Some(side),
            },
            wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 6,
            },
        );
    }
    texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("verse sky light"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glade() -> SkyInputs {
        SkyInputs {
            daylight: Daylight {
                zenith: [0.10, 0.30, 0.73],
                horizon: [0.72, 0.66, 0.50],
                sun: [1.0, 0.86, 0.62],
                clouds: 0.38,
                ground: [0.12, 0.1, 0.05],
            },
            sun: Vec3::new(-0.35, 0.8, -0.45).normalize(),
            sun_illuminance: 3.2,
            level: 0.98,
        }
    }

    /// The sky light's level is the key's, and its shape is the sky's: blue
    /// from above, the warmer ground from below, the haze from the side.
    #[test]
    fn the_sky_light_takes_the_keys_level_and_the_skys_gradient() {
        let inputs = glade();
        let light = SkyLight::bake(&inputs, 8, 16);
        let up = light.sh.irradiance(Vec3::Y);
        let down = light.sh.irradiance(-Vec3::Y);
        let side = light.sh.irradiance(Vec3::X);
        // Ground and sky together: within ringing of the sky alone's level.
        assert!(
            (luminance(up) - inputs.level).abs() < 0.1 * inputs.level,
            "{up}"
        );
        assert!(up.z > up.x, "a blue sky lights from above: {up}");
        assert!(down.x > down.z, "a warm ground lights from below: {down}");
        assert!(luminance(up) > luminance(side) && luminance(side) > luminance(down));
        // Without the key's level the sky lights nothing.
        let dark = SkyLight::bake(
            &SkyInputs {
                level: 0.0,
                sun_illuminance: 0.0,
                ..inputs
            },
            4,
            4,
        );
        assert_eq!(dark.sh.irradiance(Vec3::Y), Vec3::ZERO);
    }

    /// Glossy reflections see the sky the stage draws: the sharpest level
    /// holds the sky's color, scaled to the key's level, and the zenith is
    /// bluer than the horizon.
    #[test]
    fn the_reflection_cube_holds_the_drawn_sky() {
        let inputs = glade();
        let light = SkyLight::bake(&inputs, 8, 16);
        let open = Sh9::project(PROJECTION_SIZE, |d| {
            if d.y >= 0.0 {
                sky(&inputs.daylight, inputs.sun, cloud_coverage(0.38), d)
            } else {
                Vec3::ZERO
            }
        })
        .irradiance(Vec3::Y);
        let scale = inputs.level / luminance(open);
        // +Y face's center texel looks almost straight up.
        let side = light.cube.side(0) as usize;
        let zenith = light.cube.levels[0][2 * side * side + (side / 2) * side + side / 2];
        let d = verse_engine::environment::texel_direction(2, side as u32, 4, 4);
        let expected = sky(&inputs.daylight, inputs.sun, cloud_coverage(0.38), d) * scale;
        assert!(
            (zenith - expected).abs().max_element() < 1e-5,
            "{zenith} {expected}"
        );
        assert!(zenith.z > zenith.x);
        assert_eq!(light.cube.levels.len(), 4);
    }

    #[test]
    fn clouds_cover_more_of_the_sky_as_cover_grows() {
        let clear = cloud_coverage(0.0);
        let some = cloud_coverage(0.38);
        let overcast = cloud_coverage(1.0);
        assert!(clear < some && some < overcast, "{clear} {some} {overcast}");
        assert!((0.0..=1.0).contains(&clear) && overcast <= 1.0);
    }

    /// The CPU sky keeps `daylight_air`'s landmarks: the zenith color
    /// straight up away from the Sun, the haze color below the horizon, and
    /// a brighter sky toward the Sun.
    #[test]
    fn the_cpu_sky_matches_the_sky_pass_landmarks() {
        let inputs = glade();
        let day = inputs.daylight;
        let sun = inputs.sun;
        let up = air(&day, sun, Vec3::Y);
        // Straight up, only the halo's faint wide lobe adds to the zenith.
        assert!(
            (up - Vec3::from(day.zenith)).abs().max_element() < 0.1,
            "{up}"
        );
        let below = air(&day, sun, Vec3::new(0.0, -0.5, 0.866).normalize());
        let haze = Vec3::from(day.horizon) * 0.94;
        assert!((below - haze).abs().max_element() < 0.05, "{below}");
        let flat = Vec3::new(sun.x, 0.0, sun.z).normalize();
        assert!(luminance(air(&day, sun, flat)) > luminance(air(&day, sun, -flat)));
    }
}
