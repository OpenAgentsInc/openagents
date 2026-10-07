//! The baked looping normal tile the low tier reads in place of analytic
//! detail waves: the slopes of a sum of short sine waves whose wave vectors
//! are whole cycles per tile, so it repeats without a seam, built once on
//! the CPU and uploaded with its mipmaps. Each texel holds the mean slope
//! (red and green) and, from the first mip on, the slope variance the
//! averaging lost (blue), which the shader turns into roughness (after
//! Toksvig, "Mipmapping Normal Maps", 2005). Normal maps from summed waves
//! follow Finch (*GPU Gems*, chapter 1, 2004).

use std::f32::consts::TAU;
use std::sync::OnceLock;

/// Texels along a side of the tile.
pub const TEXELS: usize = 64;
/// The tile's size, m (`WATER_TILE_METERS` in `water.wgsl`).
pub const METERS: f32 = 8.0;
/// The slope the texels' full range stands for (`WATER_TILE_SLOPE`).
pub const SLOPE: f32 = 0.5;
/// Mip levels, 64 down to 1.
pub const LEVELS: usize = 7;

/// The tile's levels, each `side × side` RGBA8 texels, largest first.
pub fn levels() -> &'static [Vec<[u8; 4]>] {
    static LEVELS_CELL: OnceLock<Vec<Vec<[u8; 4]>>> = OnceLock::new();
    LEVELS_CELL.get_or_init(build)
}

/// The slope at each texel of the base level, (dh/dx, dh/dz).
fn slopes() -> Vec<[f32; 2]> {
    // A fixed linear congruential sequence (Knuth's MMIX constants) picks
    // the waves, so the tile is the same on every machine.
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 40) as f32) / (1u64 << 24) as f32
    };
    let mut waves = Vec::new();
    while waves.len() < 28 {
        let m = (next() * 33.0) as i32 - 16;
        let n = (next() * 33.0) as i32 - 16;
        let cycles = ((m * m + n * n) as f32).sqrt();
        if !(2.0..=16.0).contains(&cycles) {
            continue;
        }
        let k = [TAU * m as f32 / METERS, TAU * n as f32 / METERS];
        let wavelength = METERS / cycles;
        // Every wave the same steepness, a little under 1/60 of a radian.
        let amplitude = wavelength * 0.0035;
        waves.push((k, amplitude, next() * TAU));
    }
    let mut out = vec![[0.0; 2]; TEXELS * TEXELS];
    for (i, slot) in out.iter_mut().enumerate() {
        let x = (i % TEXELS) as f32 / TEXELS as f32 * METERS;
        let z = (i / TEXELS) as f32 / TEXELS as f32 * METERS;
        for (k, a, phase) in &waves {
            let c = (k[0] * x + k[1] * z + phase).cos() * a;
            slot[0] += k[0] * c;
            slot[1] += k[1] * c;
        }
    }
    out
}

fn build() -> Vec<Vec<[u8; 4]>> {
    let encode = |v: f32| ((v / SLOPE * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
    // Mean slope and mean squared slope per texel, level by level.
    let mut mean: Vec<[f32; 2]> = slopes();
    let mut square: Vec<f32> = mean.iter().map(|s| s[0] * s[0] + s[1] * s[1]).collect();
    let mut side = TEXELS;
    let mut out = Vec::with_capacity(LEVELS);
    loop {
        out.push(
            mean.iter()
                .zip(&square)
                .map(|(m, sq)| {
                    let variance = (sq - (m[0] * m[0] + m[1] * m[1])).max(0.0);
                    let lost = (variance / (SLOPE * SLOPE * 0.25)).clamp(0.0, 1.0);
                    [
                        encode(m[0]),
                        encode(m[1]),
                        (lost * 255.0).round() as u8,
                        255,
                    ]
                })
                .collect(),
        );
        if side == 1 {
            break;
        }
        let half = side / 2;
        let mut next_mean = vec![[0.0; 2]; half * half];
        let mut next_square = vec![0.0; half * half];
        for r in 0..half {
            for c in 0..half {
                for (dc, dr) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let i = (r * 2 + dr) * side + c * 2 + dc;
                    next_mean[r * half + c][0] += mean[i][0] * 0.25;
                    next_mean[r * half + c][1] += mean[i][1] * 0.25;
                    next_square[r * half + c] += square[i] * 0.25;
                }
            }
        }
        mean = next_mean;
        square = next_square;
        side = half;
    }
    out
}

/// Uploads the tile with its mipmaps and returns a view of it.
pub fn texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Verse water normal tile"),
        size: wgpu::Extent3d {
            width: TEXELS as u32,
            height: TEXELS as u32,
            depth_or_array_layers: 1,
        },
        mip_level_count: LEVELS as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, texels) in levels().iter().enumerate() {
        let side = (TEXELS >> level) as u32;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(side * 4),
                rows_per_image: Some(side),
            },
            wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
        );
    }
    texture.create_view(&Default::default())
}

/// The tile's sampler: repeating and trilinear.
pub fn sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Verse water normal tile"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}

/// The tile's bytes on the GPU, all levels.
#[must_use]
pub fn bytes() -> u64 {
    levels().iter().map(|l| l.len() as u64 * 4).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tile repeats without a seam, its slopes fit the texels' range,
    /// and the mips keep the variance they average away.
    #[test]
    fn the_tile_loops_and_keeps_its_lost_variance() {
        let s = slopes();
        let max = s
            .iter()
            .map(|v| v[0].abs().max(v[1].abs()))
            .fold(0.0, f32::max);
        assert!(max < SLOPE, "{max}");
        let rms =
            (s.iter().map(|v| v[0] * v[0] + v[1] * v[1]).sum::<f32>() / s.len() as f32).sqrt();
        assert!(rms > 0.05, "{rms}");
        // The slope one texel past the last column is the first column's.
        let at = |x: f32, z: f32| {
            let i = ((z / METERS * TEXELS as f32).rem_euclid(TEXELS as f32)) as usize * TEXELS
                + ((x / METERS * TEXELS as f32).rem_euclid(TEXELS as f32)) as usize;
            s[i]
        };
        assert_eq!(at(METERS, 0.0), at(0.0, 0.0));
        let levels = levels();
        assert_eq!(levels.len(), LEVELS);
        assert_eq!(levels[LEVELS - 1].len(), 1);
        assert!(levels[0].iter().all(|t| t[2] == 0));
        // The last level has lost nearly all the detail to variance.
        assert!(levels[LEVELS - 1][0][2] > 15, "{:?}", levels[LEVELS - 1][0]);
        assert_eq!(
            bytes(),
            (0..LEVELS)
                .map(|l| ((TEXELS >> l) * (TEXELS >> l) * 4) as u64)
                .sum::<u64>()
        );
    }
}
