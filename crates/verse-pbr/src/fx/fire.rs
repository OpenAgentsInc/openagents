//! Fire Pro's spectral blackbody integration, ported to Rust.
//! Upstream: dgreenheck/threejs-fire-pro at c284f0b13ed2234752087b8dce24f0cb38854147.
//! Copyright 2026 Daniel Greenheck, MIT; see assets/verse/fx/LICENSE-fire-pro.txt.
use std::sync::OnceLock;

pub const TABLE_SIZE: usize = 512;
pub const MIN_KELVIN: f64 = 500.0;
pub const MAX_KELVIN: f64 = 10_000.0;

/// Linear RGB, normalized to its brightest channel, and log2 radiance relative to 1800 K.
pub fn blackbody_table() -> &'static [[f32; 4]; TABLE_SIZE] {
    static TABLE: OnceLock<[[f32; 4]; TABLE_SIZE]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let reference = rgb(1800.0).into_iter().fold(0.0, f64::max);
        std::array::from_fn(|i| {
            let color =
                rgb(MIN_KELVIN + (MAX_KELVIN - MIN_KELVIN) * i as f64 / (TABLE_SIZE - 1) as f64);
            let peak = color.into_iter().fold(0.0, f64::max);
            [
                (color[0] / peak) as f32,
                (color[1] / peak) as f32,
                (color[2] / peak) as f32,
                (peak / reference).log2().max(-32.0) as f32,
            ]
        })
    })
}

fn rgb(kelvin: f64) -> [f64; 3] {
    // Wyman, Sloan and Shirley's analytic CIE 1931 observer fit (JCGT 2013).
    const LOBES: [&[[f64; 4]]; 3] = [
        &[
            [0.362, 442.0, 0.0624, 0.0374],
            [1.056, 599.8, 0.0264, 0.0323],
            [-0.065, 501.1, 0.049, 0.0382],
        ],
        &[
            [0.821, 568.8, 0.0213, 0.0247],
            [0.286, 530.9, 0.0613, 0.0322],
        ],
        &[
            [1.217, 437.0, 0.0845, 0.0278],
            [0.681, 459.0, 0.0385, 0.0725],
        ],
    ];
    let mut xyz = [0.0; 3];
    for i in 0..95 {
        let nm = 360.0 + i as f64 * 5.0;
        let wavelength = nm * 1e-9;
        let radiance = (2.0 * 6.62607015e-34 * 299792458.0_f64.powi(2))
            / (wavelength.powi(5)
                * ((6.62607015e-34 * 299792458.0) / (wavelength * 1.380649e-23 * kelvin)).exp_m1());
        for c in 0..3 {
            let observer: f64 = LOBES[c]
                .iter()
                .map(|&[height, center, left, right]| {
                    let d = (nm - center) * if nm < center { left } else { right };
                    height * (-0.5 * d * d).exp()
                })
                .sum();
            xyz[c] += radiance * 5e-9 * observer;
        }
    }
    let [x, y, z] = xyz;
    [
        (3.2406 * x - 1.5372 * y - 0.4986 * z).max(0.0),
        (-0.9689 * x + 1.8758 * y + 0.0415 * z).max(0.0),
        (0.0557 * x - 0.204 * y + 1.057 * z).max(0.0),
    ]
}

/// Fixed, deterministic 32³ density noise. Trilinear GPU sampling supplies smooth turbulence.
pub fn noise() -> Vec<u8> {
    (0..32u32.pow(3))
        .map(|i| {
            let mut n = i.wrapping_mul(1664525).wrapping_add(1013904223);
            n ^= n >> 16;
            n = n.wrapping_mul(2246822519);
            (n >> 24) as u8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spectral_heat_has_finite_radiance_and_warms_from_red_to_white() {
        let table = blackbody_table();
        assert!(table.iter().flatten().all(|v| v.is_finite()));
        let cool = table[((1200.0 - MIN_KELVIN) / (MAX_KELVIN - MIN_KELVIN) * 511.0) as usize];
        let hot = table[((4000.0 - MIN_KELVIN) / (MAX_KELVIN - MIN_KELVIN) * 511.0) as usize];
        assert!(cool[0] > 0.99 && cool[1] < 0.2 && cool[2] < 0.02);
        assert!(hot[1] > cool[1] && hot[2] > cool[2] && hot[3] > cool[3]);
    }
}

pub(crate) fn textures(device: &wgpu::Device, queue: &wgpu::Queue) -> [wgpu::TextureView; 2] {
    let upload = |name, size: wgpu::Extent3d, dimension, format, bytes: &[u8], row| {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(size.height),
            },
            size,
        );
        texture.create_view(&Default::default())
    };
    let data: Vec<u16> = blackbody_table()
        .iter()
        .flatten()
        .map(|&v| crate::pbr::gpu::half(v))
        .collect();
    [
        upload(
            "Verse fire turbulence",
            wgpu::Extent3d {
                width: 32,
                height: 32,
                depth_or_array_layers: 32,
            },
            wgpu::TextureDimension::D3,
            wgpu::TextureFormat::R8Unorm,
            &noise(),
            32,
        ),
        upload(
            "Fire Pro spectral blackbody",
            wgpu::Extent3d {
                width: 512,
                height: 1,
                depth_or_array_layers: 1,
            },
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba16Float,
            bytemuck::cast_slice(&data),
            4096,
        ),
    ]
}
