//! Deterministic RGBA8 mip recipes, independent of GPU formats and source I/O.
pub mod archive;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const RECIPE_VERSION: u32 = 1;
pub const MAX_DIMENSION: u32 = 8192;

/// Color RGB uses sRGB; alpha and data channels remain linear.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case", deny_unknown_fields)]
pub enum Role {
    Color,
    Mask {
        #[serde(rename = "cutoff_bits")]
        cutoff: u32,
    },
    Normal,
    Linear,
}
impl Role {
    /// Match the shader's normalized-byte alpha comparison, including the material factor.
    pub fn masked(cutoff: f32, opacity: f32) -> Result<Self, String> {
        if [cutoff, opacity]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err("Invalid mip alpha comparison".into());
        }
        let effective = cutoff / opacity;
        Ok(if cutoff > 0. && opacity > 0. && effective <= 1. {
            Self::Mask {
                cutoff: effective.to_bits(),
            }
        } else {
            Self::Color
        })
    }

    pub fn srgb(self) -> bool {
        matches!(self, Self::Color | Self::Mask { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Variant {
    pub texture: usize,
    pub role: Role,
}

pub fn surface_role(surface: &crate::assets::Surface) -> Result<Role, String> {
    if surface.blend == 1 {
        Role::masked(surface.material.alpha_cutoff, surface.material.opacity)
    } else {
        Ok(Role::Color)
    }
}

/// A source slot can have separate color, mask, normal, and scalar variants.
pub fn pack_variants(pack: &crate::assets::Pack) -> Result<BTreeSet<Variant>, String> {
    let mut variants = BTreeSet::new();
    let mut used = BTreeSet::new();
    for surface in pack.models.values().flat_map(|model| &model.surfaces) {
        surface.material.validate(pack.textures.len())?;
        if surface.texture >= pack.textures.len() {
            return Err("Invalid mip base texture slot".into());
        }
        variants.insert(Variant {
            texture: surface.texture,
            role: surface_role(surface)?,
        });
        used.insert(surface.texture);
        for (slot, role) in [
            (surface.material.normal_texture, Role::Normal),
            (surface.material.metallic_roughness_texture, Role::Linear),
            (surface.material.occlusion_texture, Role::Linear),
            (surface.material.emissive_texture, Role::Color),
        ] {
            if let Some(texture) = slot {
                variants.insert(Variant { texture, role });
                used.insert(texture);
            }
        }
        if variants.len() > 16_384 {
            return Err("Mip variant metadata exceeds its bound".into());
        }
    }
    for texture in 0..pack.textures.len() {
        if !used.contains(&texture) {
            variants.insert(Variant {
                texture,
                role: Role::Color,
            });
        }
    }
    if variants.len() > 16_384 {
        return Err("Mip variant metadata exceeds its bound".into());
    }
    Ok(variants)
}

pub type Level = (u32, u32, Vec<u8>);

pub fn bytes(mut width: u32, mut height: u32, max: u32) -> Result<u64, String> {
    validate(width, height, max)?;
    let mut bytes = 0;
    loop {
        if width <= max && height <= max {
            bytes += u64::from(width) * u64::from(height) * 4;
        }
        if width == 1 && height == 1 {
            break;
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    Ok(bytes)
}
fn validate(width: u32, height: u32, max: u32) -> Result<(), String> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION || max == 0 {
        return Err("Invalid mip image extent".into());
    }
    Ok(())
}

pub fn srgb_to_linear() -> &'static [f64; 256] {
    static TABLE: std::sync::OnceLock<[f64; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        std::array::from_fn(|i| {
            let c = i as f64 / 255.;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
    })
}
fn encode(c: f64) -> u8 {
    let c = c.clamp(0., 1.);
    let c = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1. / 2.4) - 0.055
    };
    (c * 255.).round() as u8
}
fn normal(rgb: [f64; 3]) -> [f64; 3] {
    let length = rgb.iter().map(|v| v * v).sum::<f64>().sqrt();
    if length > 1e-8 {
        rgb.map(|v| v / length)
    } else {
        [0., 0., 1.]
    }
}
fn normalize_texels(rgba: &mut [u8]) {
    for p in rgba.chunks_exact_mut(4) {
        let n = normal([p[0], p[1], p[2]].map(|v| f64::from(v) / 127.5 - 1.));
        for c in 0..3 {
            p[c] = ((n[c] + 1.) * 127.5).round().clamp(0., 255.) as u8;
        }
    }
}

/// Area weights include every source texel when either dimension is odd.
fn halve(width: u32, height: u32, rgba: &[u8], role: Role) -> Level {
    let (w, h) = ((width / 2).max(1), (height / 2).max(1));
    let table = srgb_to_linear();
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        let y0 = f64::from(y) * f64::from(height) / f64::from(h);
        let y1 = f64::from(y + 1) * f64::from(height) / f64::from(h);
        for x in 0..w {
            let x0 = f64::from(x) * f64::from(width) / f64::from(w);
            let x1 = f64::from(x + 1) * f64::from(width) / f64::from(w);
            let mut sum = [0.; 4];
            let mut weight = 0.;
            for sy in y0.floor() as u32..y1.ceil() as u32 {
                for sx in x0.floor() as u32..x1.ceil() as u32 {
                    let a = (y1.min(f64::from(sy + 1)) - y0.max(f64::from(sy)))
                        * (x1.min(f64::from(sx + 1)) - x0.max(f64::from(sx)));
                    let p = &rgba[((sy * width + sx) * 4) as usize..][..4];
                    let rgb = if role == Role::Normal {
                        normal([p[0], p[1], p[2]].map(|v| f64::from(v) / 127.5 - 1.))
                    } else if role.srgb() {
                        [
                            table[p[0] as usize],
                            table[p[1] as usize],
                            table[p[2] as usize],
                        ]
                    } else {
                        [p[0], p[1], p[2]].map(|v| f64::from(v) / 255.)
                    };
                    for c in 0..3 {
                        sum[c] += rgb[c] * a;
                    }
                    sum[3] += f64::from(p[3]) / 255. * a;
                    weight += a;
                }
            }
            for n in &mut sum {
                *n /= weight;
            }
            if role == Role::Normal {
                out.extend(
                    normal(sum[..3].try_into().unwrap())
                        .map(|v| ((v + 1.) * 127.5).round().clamp(0., 255.) as u8),
                );
            } else if role.srgb() {
                out.extend(sum[..3].iter().map(|v| encode(*v)));
            } else {
                out.extend(
                    sum[..3]
                        .iter()
                        .map(|v| (v * 255.).round().clamp(0., 255.) as u8),
                );
            }
            out.push((sum[3] * 255.).round().clamp(0., 255.) as u8);
        }
    }
    (w, h, out)
}

/// Bilinear repeat sampling, on a fixed grid, matching the runtime's spatial filter.
fn filtered_coverage(
    mask: &[bool],
    width: u32,
    height: u32,
    high: u8,
    low: u8,
    cutoff: f32,
) -> f64 {
    let mut kept = 0;
    const SAMPLES: u32 = 32;
    for y in 0..SAMPLES {
        for x in 0..SAMPLES {
            let px = (f64::from(x) + 0.5) * f64::from(width) / f64::from(SAMPLES) - 0.5;
            let py = (f64::from(y) + 0.5) * f64::from(height) / f64::from(SAMPLES) - 0.5;
            let (ix, iy) = (px.floor() as i32, py.floor() as i32);
            let (fx, fy) = (px - px.floor(), py - py.floor());
            let sample = |dx: i32, dy: i32| {
                let sx = (ix + dx).rem_euclid(width as i32) as u32;
                let sy = (iy + dy).rem_euclid(height as i32) as u32;
                f64::from(if mask[(sy * width + sx) as usize] {
                    high
                } else {
                    low
                }) / 255.
            };
            let alpha = (sample(0, 0) * (1. - fx) + sample(1, 0) * fx) * (1. - fy)
                + (sample(0, 1) * (1. - fx) + sample(1, 1) * fx) * fy;
            kept += usize::from(alpha >= f64::from(cutoff));
        }
    }
    kept as f64 / f64::from(SAMPLES * SAMPLES)
}

/// Rank alpha to retain the nearest texel count, with one surviving texel for nonempty masks.
/// Tune alpha contrast against bilinear coverage; fully uniform coarse levels cannot retain fractions.
fn preserve(
    width: u32,
    height: u32,
    rgba: &mut [u8],
    cutoff: f32,
    root_kept: usize,
    root_texels: usize,
) {
    let count = rgba.len() / 4;
    let wanted =
        ((root_kept as u64 * count as u64 + root_texels as u64 / 2) / root_texels as u64) as usize;
    let wanted = if root_kept > 0 { wanted.max(1) } else { 0 };
    let threshold = (0u16..=255)
        .find(|v| f32::from(*v) / 255. >= cutoff)
        .unwrap() as u8;
    let mut histogram = [0usize; 256];
    for p in rgba.chunks_exact(4) {
        histogram[p[3] as usize] += 1;
    }
    let mut selected = [0usize; 256];
    let mut remaining = wanted;
    for alpha in (0..256).rev() {
        selected[alpha] = histogram[alpha].min(remaining);
        remaining -= selected[alpha];
    }
    let mask: Vec<bool> = rgba
        .chunks_exact(4)
        .map(|p| {
            let alpha = p[3] as usize;
            if selected[alpha] > 0 {
                selected[alpha] -= 1;
                true
            } else {
                false
            }
        })
        .collect();
    let target = root_kept as f64 / root_texels as f64;
    let mut high = 255;
    let mut low = 0;
    if wanted > 0 && wanted < count {
        let current = filtered_coverage(&mask, width, height, high, low, cutoff);
        let mut best = (current - target).abs();
        // Coverage is monotone in each endpoint. Search integer alpha values without copying the image.
        let (mut a, mut b) = if current < target {
            (0u16, u16::from(threshold.saturating_sub(1)))
        } else {
            (u16::from(threshold), 255)
        };
        while a <= b {
            let middle = (a + b) / 2;
            let (candidate_high, candidate_low) = if current < target {
                (255, middle as u8)
            } else {
                (middle as u8, 0)
            };
            let coverage =
                filtered_coverage(&mask, width, height, candidate_high, candidate_low, cutoff);
            let error = (coverage - target).abs();
            if error < best {
                best = error;
                high = candidate_high;
                low = candidate_low;
            }
            if coverage < target {
                a = middle + 1;
            } else if middle == 0 {
                break;
            } else {
                b = middle - 1;
            }
        }
    }
    for (p, keep) in rgba.chunks_exact_mut(4).zip(mask) {
        p[3] = if keep { high } else { low };
    }
}

/// Cook the complete chain, retaining only levels that fit the target device.
/// Masks use the original image's coverage; corrections never feed the next reduction.
pub fn cook(
    width: u32,
    height: u32,
    rgba: &[u8],
    role: Role,
    max: u32,
) -> Result<Vec<Level>, String> {
    validate(width, height, max)?;
    if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4
        || matches!(role, Role::Mask { cutoff } if !f32::from_bits(cutoff).is_finite() || !(0.0..=1.0).contains(&f32::from_bits(cutoff)) || f32::from_bits(cutoff) <= 0.)
    {
        return Err("Invalid mip payload or mask threshold".into());
    }
    let kept = match role {
        Role::Mask { cutoff } => rgba
            .chunks_exact(4)
            .filter(|p| f32::from(p[3]) / 255. >= f32::from_bits(cutoff))
            .count(),
        _ => 0,
    };
    let root = (width, height);
    let mut current = (width, height, rgba.to_vec());
    if role == Role::Normal {
        normalize_texels(&mut current.2);
    }
    let mut levels = Vec::new();
    loop {
        let (width, height) = (current.0, current.1);
        let next = (width > 1 || height > 1).then(|| halve(width, height, &current.2, role));
        if width <= max && height <= max {
            if (width, height) != root {
                if let Role::Mask { cutoff } = role {
                    preserve(
                        width,
                        height,
                        &mut current.2,
                        f32::from_bits(cutoff),
                        kept,
                        rgba.len() / 4,
                    );
                }
            }
            levels.push(current);
        }
        match next {
            Some(value) => current = value,
            None => break,
        }
    }
    Ok(levels)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn color_and_scalar_recipes_have_different_real_mip_bytes() {
        let pixels = [[0, 0, 0, 0], [255, 255, 255, 255]].repeat(2).concat();
        let color = cook(2, 2, &pixels, Role::Color, MAX_DIMENSION).unwrap();
        let scalar = cook(2, 2, &pixels, Role::Linear, MAX_DIMENSION).unwrap();
        assert_eq!(color[0].2, pixels);
        assert_eq!(color[1].2, [188, 188, 188, 128]);
        assert_eq!(scalar[1].2, [128; 4]);
        assert_eq!(bytes(2, 2, MAX_DIMENSION).unwrap(), 20);
    }
    #[test]
    fn odd_edges_contribute_to_every_reduction_and_device_fitting() {
        let pixels: Vec<_> = (0..15)
            .flat_map(|i| [if i % 5 == 4 || i / 5 == 2 { 255 } else { 0 }; 4])
            .collect();
        let levels = cook(5, 3, &pixels, Role::Linear, MAX_DIMENSION).unwrap();
        assert_eq!(
            levels.iter().map(|l| (l.0, l.1)).collect::<Vec<_>>(),
            [(5, 3), (2, 1), (1, 1)]
        );
        assert_eq!(levels[2].2, [119; 4]);
        let fitted = cook(5, 3, &pixels, Role::Linear, 2).unwrap();
        assert_eq!(fitted, levels[1..]);
        assert_eq!(bytes(5, 3, 2).unwrap(), 12);
    }
    #[test]
    fn normal_mips_are_finite_normalized_and_cancellation_has_a_forward_fallback() {
        let pixels = [[255, 128, 128, 255], [128, 255, 128, 255]]
            .repeat(8)
            .concat();
        let levels = cook(4, 4, &pixels, Role::Normal, MAX_DIMENSION).unwrap();
        for (_, _, data) in levels {
            for p in data.chunks_exact(4) {
                let length = p[..3]
                    .iter()
                    .map(|v| (f64::from(*v) / 127.5 - 1.).powi(2))
                    .sum::<f64>()
                    .sqrt();
                assert!((length - 1.).abs() < 0.009, "normal {p:?}, length {length}");
            }
        }
        let opposite = [[255, 128, 128, 255], [0, 127, 127, 255]].concat();
        assert_eq!(
            cook(2, 1, &opposite, Role::Normal, 2).unwrap()[1].2,
            [128, 128, 255, 255]
        );
    }
    #[test]
    fn cutout_mips_keep_point_and_bilinear_density_without_filling_tied_levels() {
        let pixels: Vec<_> = (0..256)
            .flat_map(|i| [40, 120, 30, if i % 16 % 4 == 0 { 255 } else { 0 }])
            .collect();
        let cutoff = 0.6;
        let masked = cook(16, 16, &pixels, Role::masked(cutoff, 1.).unwrap(), 16).unwrap();
        let color = cook(16, 16, &pixels, Role::Color, 16).unwrap();
        assert_eq!(masked[0].2, pixels);
        for (index, (width, height, data)) in masked.iter().enumerate().skip(1) {
            let kept = data
                .chunks_exact(4)
                .filter(|p| f32::from(p[3]) / 255. >= cutoff)
                .count();
            assert_eq!(kept, ((width * height) / 4).max(1) as usize);
            for (m, c) in data.chunks_exact(4).zip(color[index].2.chunks_exact(4)) {
                assert_eq!(&m[..3], &c[..3]);
            }
            if *width > 1 {
                let mask: Vec<_> = data
                    .chunks_exact(4)
                    .map(|p| f32::from(p[3]) / 255. >= cutoff)
                    .collect();
                let high = data.chunks_exact(4).map(|p| p[3]).max().unwrap();
                let low = data.chunks_exact(4).map(|p| p[3]).min().unwrap();
                assert!(
                    (filtered_coverage(&mask, *width, *height, high, low, cutoff) - 0.25).abs()
                        < 0.04
                );
            }
        }
        assert_ne!(
            Role::masked(0.6, 1.).unwrap(),
            Role::masked(0.601, 1.).unwrap()
        );
        assert_ne!(
            Role::masked(0.6, 1.).unwrap(),
            Role::masked(0.6, 0.75).unwrap()
        );
    }
    #[test]
    fn invalid_extents_payloads_and_cutoffs_are_refused_before_allocating() {
        assert!(cook(0, 1, &[], Role::Color, 1).is_err());
        assert!(cook(u32::MAX, u32::MAX, &[], Role::Normal, 1).is_err());
        assert!(cook(1, 1, &[], Role::Color, 1).is_err());
        assert!(
            cook(
                1,
                1,
                &[0; 4],
                Role::Mask {
                    cutoff: f32::NAN.to_bits()
                },
                1
            )
            .is_err()
        );
        assert!(Role::masked(f32::NAN, 1.).is_err());
        assert_eq!(Role::masked(0.5, 0.).unwrap(), Role::Color);
        assert_eq!(Role::masked(0., 1.).unwrap(), Role::Color);
    }
}
