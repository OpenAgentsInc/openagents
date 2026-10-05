//! Upload explicit material-role recipes without sRGB/linear view reinterpretation.
use verse_engine::mips::{Role, Variant};

pub(super) struct Image {
    #[cfg_attr(not(test), allow(dead_code))]
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}
pub(super) fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
    role: Role,
) -> Result<Image, String> {
    let levels = verse_engine::mips::cook(
        width,
        height,
        rgba,
        role,
        device.limits().max_texture_dimension_2d,
    )?;
    upload_levels(device, queue, label, &levels, role)
}
pub(super) fn upload_levels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    levels: &[verse_engine::mips::Level],
    role: Role,
) -> Result<Image, String> {
    if levels.is_empty()
        || levels.iter().any(|(width, height, bytes)| {
            *width == 0
                || *height == 0
                || *width > device.limits().max_texture_dimension_2d
                || *height > device.limits().max_texture_dimension_2d
                || bytes.len() as u64 != u64::from(*width) * u64::from(*height) * 4
        })
    {
        return Err("Invalid prepared GPU mip levels".into());
    }
    if levels.len() > 14
        || levels.windows(2).any(|pair| {
            pair[1].0 != (pair[0].0 / 2).max(1)
                || pair[1].1 != (pair[0].1 / 2).max(1)
                || pair[0].0 == 1 && pair[0].1 == 1
        })
        || levels.last().is_none_or(|l| l.0 != 1 || l.1 != 1)
    {
        return Err("Prepared GPU mip chain has inconsistent levels".into());
    }
    let format = if role.srgb() {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: levels[0].0,
            height: levels[0].1,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | if cfg!(test) {
                wgpu::TextureUsages::COPY_SRC
            } else {
                wgpu::TextureUsages::empty()
            },
        view_formats: &[],
    });
    for (level, (width, height, bytes)) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(*height),
            },
            wgpu::Extent3d {
                width: *width,
                height: *height,
                depth_or_array_layers: 1,
            },
        );
    }
    let view = texture.create_view(&Default::default());
    Ok(Image { texture, view })
}

impl super::material_gpu::Key {
    pub fn base_variant(self) -> Variant {
        Variant {
            texture: self.texture,
            role: self.base_role(),
        }
    }
    pub fn map_variant(self, channel: usize) -> Variant {
        self.maps[channel].map_or_else(
            || self.base_variant(),
            |texture| Variant {
                texture,
                role: match channel {
                    0 => Role::Normal,
                    3 => Role::Color,
                    _ => Role::Linear,
                },
            },
        )
    }
}
