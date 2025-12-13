//! Text rendering system using cosmic-text
//!
//! Provides text shaping and glyph atlas management for GPU rendering.

use crate::color::Hsla;
use crate::scene::TextQuad;
use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent,
};
use std::collections::HashMap;

/// Embedded Berkeley Mono font
const BERKELEY_MONO_REGULAR: &[u8] =
    include_bytes!("../../commander/assets/fonts/BerkeleyMono-Regular.ttf");
const BERKELEY_MONO_BOLD: &[u8] =
    include_bytes!("../../commander/assets/fonts/BerkeleyMono-Bold.ttf");

/// Cached glyph info
#[derive(Clone, Debug)]
struct GlyphEntry {
    uv_origin: [f32; 2],
    uv_size: [f32; 2],
    size: [f32; 2],
    offset: [f32; 2],
}

/// Text rendering system
pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
    glyph_cache: HashMap<CacheKey, GlyphEntry>,
    atlas_data: Vec<u8>,
    atlas_size: u32,
    atlas_cursor_x: u32,
    atlas_cursor_y: u32,
    atlas_row_height: u32,
    pub atlas_texture: Option<wgpu::Texture>,
    pub atlas_bind_group: Option<wgpu::BindGroup>,
    dirty: bool,
    scale_factor: f32,
}

impl TextSystem {
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();

        // Load Berkeley Mono fonts
        font_system
            .db_mut()
            .load_font_data(BERKELEY_MONO_REGULAR.to_vec());
        font_system
            .db_mut()
            .load_font_data(BERKELEY_MONO_BOLD.to_vec());

        let atlas_size = 1024u32;

        Self {
            font_system,
            swash_cache: SwashCache::new(),
            glyph_cache: HashMap::new(),
            atlas_data: vec![0u8; (atlas_size * atlas_size) as usize],
            atlas_size,
            atlas_cursor_x: 0,
            atlas_cursor_y: 0,
            atlas_row_height: 0,
            atlas_texture: None,
            atlas_bind_group: None,
            dirty: true,
            scale_factor: 1.0,
        }
    }

    /// Set the scale factor for high-DPI rendering
    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        if (self.scale_factor - scale_factor).abs() > 0.001 {
            self.glyph_cache.clear();
            self.atlas_cursor_x = 0;
            self.atlas_cursor_y = 0;
            self.atlas_row_height = 0;
            self.atlas_data.fill(0);
            self.dirty = true;
        }
        self.scale_factor = scale_factor;
    }

    /// Initialize GPU resources
    pub fn init_gpu(
        &mut self,
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width: self.atlas_size,
                height: self.atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_bind_group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        self.atlas_texture = Some(texture);
        self.atlas_bind_group = Some(bind_group);
    }

    /// Upload atlas to GPU if dirty
    pub fn update_atlas(&mut self, queue: &wgpu::Queue) {
        if self.dirty {
            if let Some(ref texture) = self.atlas_texture {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &self.atlas_data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(self.atlas_size),
                        rows_per_image: Some(self.atlas_size),
                    },
                    wgpu::Extent3d {
                        width: self.atlas_size,
                        height: self.atlas_size,
                        depth_or_array_layers: 1,
                    },
                );
                self.dirty = false;
            }
        }
    }

    /// Measure text width without rendering (returns logical pixels)
    pub fn measure(&mut self, text: &str, font_size: f32) -> f32 {
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(10000.0), Some(font_size * 2.0));
        buffer.set_text(
            &mut self.font_system,
            text,
            Attrs::new().family(Family::Name("Berkeley Mono")),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut width = 0.0f32;
        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                width = width.max(glyph.x + glyph.w);
            }
        }
        width
    }

    /// Layout and rasterize text, returning TextQuads for rendering
    pub fn layout(
        &mut self,
        text: &str,
        position: [f32; 2],
        font_size: f32,
        color: Hsla,
    ) -> Vec<TextQuad> {
        let scale = self.scale_factor;
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(10000.0), Some(font_size * 2.0));
        buffer.set_text(
            &mut self.font_system,
            text,
            Attrs::new().family(Family::Name("Berkeley Mono")),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut quads = Vec::new();

        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), scale);
                let cache_key = physical.cache_key;

                let entry = if let Some(entry) = self.glyph_cache.get(&cache_key) {
                    entry.clone()
                } else {
                    if let Some(image) = self
                        .swash_cache
                        .get_image(&mut self.font_system, cache_key)
                    {
                        let width = image.placement.width as u32;
                        let height = image.placement.height as u32;

                        if width > 0 && height > 0 {
                            if self.atlas_cursor_x + width > self.atlas_size {
                                self.atlas_cursor_x = 0;
                                self.atlas_cursor_y += self.atlas_row_height;
                                self.atlas_row_height = 0;
                            }

                            if self.atlas_cursor_y + height <= self.atlas_size {
                                let atlas_x = self.atlas_cursor_x;
                                let atlas_y = self.atlas_cursor_y;

                                match image.content {
                                    SwashContent::Mask => {
                                        for y in 0..height {
                                            for x in 0..width {
                                                let src_idx = (y * width + x) as usize;
                                                let dst_idx = ((atlas_y + y) * self.atlas_size
                                                    + atlas_x
                                                    + x)
                                                    as usize;
                                                if src_idx < image.data.len()
                                                    && dst_idx < self.atlas_data.len()
                                                {
                                                    self.atlas_data[dst_idx] = image.data[src_idx];
                                                }
                                            }
                                        }
                                    }
                                    SwashContent::Color => {
                                        for y in 0..height {
                                            for x in 0..width {
                                                let src_idx = ((y * width + x) * 4 + 3) as usize;
                                                let dst_idx = ((atlas_y + y) * self.atlas_size
                                                    + atlas_x
                                                    + x)
                                                    as usize;
                                                if src_idx < image.data.len()
                                                    && dst_idx < self.atlas_data.len()
                                                {
                                                    self.atlas_data[dst_idx] = image.data[src_idx];
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }

                                self.dirty = true;
                                self.atlas_cursor_x += width + 1;
                                self.atlas_row_height = self.atlas_row_height.max(height + 1);

                                let entry = GlyphEntry {
                                    uv_origin: [
                                        atlas_x as f32 / self.atlas_size as f32,
                                        atlas_y as f32 / self.atlas_size as f32,
                                    ],
                                    uv_size: [
                                        width as f32 / self.atlas_size as f32,
                                        height as f32 / self.atlas_size as f32,
                                    ],
                                    size: [width as f32, height as f32],
                                    offset: [
                                        image.placement.left as f32,
                                        image.placement.top as f32,
                                    ],
                                };
                                self.glyph_cache.insert(cache_key, entry.clone());
                                entry
                            } else {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                };

                let x = position[0] + glyph.x + entry.offset[0] / scale;
                let y = position[1] + run.line_y - entry.offset[1] / scale;

                quads.push(TextQuad::new(
                    x,
                    y,
                    entry.size[0] / scale,
                    entry.size[1] / scale,
                    entry.uv_origin[0],
                    entry.uv_origin[1],
                    entry.uv_size[0],
                    entry.uv_size[1],
                    color,
                ));
            }
        }

        quads
    }
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Create bind group layout for text rendering
pub fn create_text_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("text_bind_group_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Text shader WGSL source
pub const TEXT_SHADER: &str = r#"
struct GlobalParams {
    viewport_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> globals: GlobalParams;

@group(1) @binding(0)
var glyph_atlas: texture_2d<f32>;

@group(1) @binding(1)
var atlas_sampler: sampler;

struct TextInstance {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_origin: vec2<f32>,
    @location(3) uv_size: vec2<f32>,
    @location(4) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

fn hsla_to_rgba(hsla: vec4<f32>) -> vec4<f32> {
    let h = hsla.x * 6.0;
    let s = hsla.y;
    let l = hsla.z;
    let a = hsla.w;

    let c = (1.0 - abs(2.0 * l - 1.0)) * s;
    let x = c * (1.0 - abs(h % 2.0 - 1.0));
    let m = l - c / 2.0;

    var rgb: vec3<f32>;
    if (h < 1.0) { rgb = vec3<f32>(c, x, 0.0); }
    else if (h < 2.0) { rgb = vec3<f32>(x, c, 0.0); }
    else if (h < 3.0) { rgb = vec3<f32>(0.0, c, x); }
    else if (h < 4.0) { rgb = vec3<f32>(0.0, x, c); }
    else if (h < 5.0) { rgb = vec3<f32>(x, 0.0, c); }
    else { rgb = vec3<f32>(c, 0.0, x); }

    return vec4<f32>(rgb + m, a);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_id: u32,
    instance: TextInstance,
) -> VertexOutput {
    let x = f32(vertex_id & 1u);
    let y = f32((vertex_id >> 1u) & 1u);
    let unit_pos = vec2<f32>(x, y);

    let world_pos = instance.position + unit_pos * instance.size;
    let ndc = (world_pos / globals.viewport_size) * 2.0 - 1.0;

    var output: VertexOutput;
    output.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    output.uv = instance.uv_origin + unit_pos * instance.uv_size;
    output.color = instance.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(glyph_atlas, atlas_sampler, input.uv).r;
    let color = hsla_to_rgba(input.color);
    return vec4<f32>(color.rgb * color.a * alpha, color.a * alpha);
}
"#;
