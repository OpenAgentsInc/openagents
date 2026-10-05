//! A rasterized panel composited over a finished world frame.
//!
//! An application paints a panel on the CPU (on desktop, a Rust Native view
//! through `rust-native-desktop`) and hands the pixels here as an
//! [`OverlayImage`]. The renderer uploads them to a texture once per
//! revision and draws one alpha-blended quad over the frame after the world
//! and the HUD, with one sample and no depth test. The world's own geometry,
//! palette, and HUD are drawn exactly as without a panel.

use crate::ui::UiVertex;

/// The widest or tallest panel, in pixels.
pub const MAX_EXTENT: u32 = 4096;

/// Panel pixels at a position in the frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayImage {
    /// Changes whenever `rgba` does; an unchanged revision is not uploaded again.
    pub revision: u64,
    /// The top-left corner, in frame pixels.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// sRGB-encoded RGBA with straight (not premultiplied) alpha, rows top
    /// to bottom.
    pub rgba: Vec<u8>,
}

impl OverlayImage {
    /// An image from premultiplied RGBA, as `rust-native-desktop` paints
    /// into a transparent frame.
    ///
    /// # Errors
    ///
    /// Returns a message when the extent is out of bounds or the pixel count
    /// does not match it.
    pub fn from_premultiplied(
        revision: u64,
        (x, y): (i32, i32),
        (width, height): (u32, u32),
        premultiplied: &[u8],
    ) -> Result<Self, String> {
        let image = Self {
            revision,
            x,
            y,
            width,
            height,
            rgba: premultiplied
                .chunks_exact(4)
                .flat_map(|p| match p[3] {
                    0 => [0, 0, 0, 0],
                    255 => [p[0], p[1], p[2], 255],
                    a => {
                        let straight =
                            |c: u8| ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)) as u8;
                        [straight(p[0]), straight(p[1]), straight(p[2]), a]
                    }
                })
                .collect(),
        };
        image.validate()?;
        Ok(image)
    }

    /// Checks the extent and the pixel count.
    ///
    /// # Errors
    ///
    /// Returns a message naming the bound the image breaks.
    pub fn validate(&self) -> Result<(), String> {
        if self.width == 0 || self.height == 0 {
            return Err("an overlay needs a nonzero extent".into());
        }
        if self.width > MAX_EXTENT || self.height > MAX_EXTENT {
            return Err(format!("an overlay is at most {MAX_EXTENT} pixels a side"));
        }
        if self.rgba.len() != self.width as usize * self.height as usize * 4 {
            return Err("overlay pixels do not match its extent".into());
        }
        Ok(())
    }

    /// The two triangles covering the image in a frame, with texture
    /// coordinates over the whole image and white vertex color.
    #[must_use]
    pub fn quad(&self) -> [UiVertex; 6] {
        let (x0, y0) = (self.x as f32, self.y as f32);
        let (x1, y1) = (x0 + self.width as f32, y0 + self.height as f32);
        let v = |x, y, u, w| UiVertex {
            pos: [x, y],
            uv: [u, w],
            color: [1.0; 4],
        };
        [
            v(x0, y0, 0.0, 0.0),
            v(x1, y0, 1.0, 0.0),
            v(x0, y1, 0.0, 1.0),
            v(x0, y1, 0.0, 1.0),
            v(x1, y0, 1.0, 0.0),
            v(x1, y1, 1.0, 1.0),
        ]
    }
}

/// The uploaded texture of one revision.
struct Uploaded {
    revision: u64,
    size: (u32, u32),
    group: wgpu::BindGroup,
}

/// The GPU half: a pipeline in the frame's format, the uploaded image, and
/// its quad.
pub struct Overlay {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    screen: wgpu::Buffer,
    quad: wgpu::Buffer,
    uploaded: Option<Uploaded>,
    shown: bool,
}

impl Overlay {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse overlay"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse overlay"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        // The HUD's shader: texture times vertex color, in screen pixels.
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("verse overlay"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("verse overlay"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4,
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        // Panel pixels map one to one onto frame pixels.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("verse overlay"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let screen = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse overlay screen"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let quad = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse overlay quad"),
            size: std::mem::size_of::<[UiVertex; 6]>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            sampler,
            screen,
            quad,
            uploaded: None,
            shown: false,
        }
    }

    /// Shows `image` from the next encoded frame, or nothing for `None`.
    /// The pixels upload only when the revision or the extent changed.
    pub fn set(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: Option<&OverlayImage>,
    ) -> Result<(), String> {
        let Some(image) = image else {
            self.shown = false;
            return Ok(());
        };
        image.validate()?;
        let size = (image.width, image.height);
        if self
            .uploaded
            .as_ref()
            .is_none_or(|u| u.revision != image.revision || u.size != size)
        {
            let extent = wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("verse overlay"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
                &image.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width * 4),
                    rows_per_image: Some(image.height),
                },
                extent,
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("verse overlay"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.screen.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.uploaded = Some(Uploaded {
                revision: image.revision,
                size,
                group,
            });
        }
        queue.write_buffer(&self.quad, 0, bytemuck::cast_slice(&image.quad()));
        self.shown = true;
        Ok(())
    }

    /// Draws the shown image over `output`, a `size` pixel frame that
    /// already holds the world.
    pub fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        size: [f32; 2],
    ) {
        let Some(uploaded) = self.uploaded.as_ref().filter(|_| self.shown) else {
            return;
        };
        queue.write_buffer(
            &self.screen,
            0,
            bytemuck::cast_slice(&[size[0], size[1], 0.0, 0.0]),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("verse overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &uploaded.group, &[]);
        pass.set_vertex_buffer(0, self.quad.slice(..));
        pass.draw(0..6, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiplied_pixels_become_straight_alpha_and_bounds_hold() {
        let image = OverlayImage::from_premultiplied(
            1,
            (10, 20),
            (3, 1),
            &[0, 0, 0, 0, 200, 100, 50, 255, 64, 32, 0, 128],
        )
        .unwrap();
        assert_eq!(
            image.rgba,
            vec![0, 0, 0, 0, 200, 100, 50, 255, 128, 64, 0, 128]
        );
        let quad = image.quad();
        assert_eq!(quad[0].pos, [10.0, 20.0]);
        assert_eq!(quad[5].pos, [13.0, 21.0]);
        assert_eq!(quad[5].uv, [1.0, 1.0]);
        assert!(OverlayImage::from_premultiplied(1, (0, 0), (2, 2), &[0; 4]).is_err());
        assert!(OverlayImage::from_premultiplied(1, (0, 0), (0, 1), &[]).is_err());
        assert!(OverlayImage::from_premultiplied(1, (0, 0), (MAX_EXTENT + 1, 1), &[]).is_err());
    }
}
