//! The imported renderer's water pass: the shared water shader
//! (`crate::water`) over this renderer's frame, drawn inside the world pass
//! after opaque surfaces and before blended ones, in two halves (transmit,
//! then emit), with no extra pass or render target.

use std::sync::Arc;

use crate::water::{SurfaceGpu, Water, WaterSurface, WaterUniform};

/// The water pass's GPU state.
pub(super) struct WaterPass {
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    emit: wgpu::RenderPipeline,
    transmit: wgpu::RenderPipeline,
    surface: Option<SurfaceGpu>,
    /// What [`Self::surface`] was uploaded from, kept to upload again after
    /// a device loss.
    pub source: Option<Arc<WaterSurface>>,
}

impl WaterPass {
    /// The pipelines over `shader` (the expanded scene shader), group 0 the
    /// frame and group 1 the water's uniform and normal tile.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shader: &wgpu::ShaderModule,
        frame_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        samples: u32,
    ) -> Self {
        let [tile, tile_sampler] = crate::water::tile_entries(8);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Verse imported water"),
            entries: &[crate::water::uniform_entry(7), tile, tile_sampler],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Verse imported water"),
            size: std::mem::size_of::<WaterUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let tile_view = crate::water::tile::texture(device, queue);
        let tile_sampler = crate::water::tile::sampler(device);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Verse imported water"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&tile_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&tile_sampler),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Verse imported water"),
            bind_group_layouts: &[Some(frame_layout), Some(&layout)],
            immediate_size: 0,
        });
        let vertices = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::water::WaterVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &crate::water::frame::VERTEX_ATTRIBUTES,
        }];
        let make = |label: &str, entry: &str, blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_water"),
                    compilation_options: Default::default(),
                    buffers: &vertices,
                },
                // Seen from above and below, so neither face is culled.
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            emit: make("Verse imported water", "fs_water", crate::water::EMIT_BLEND),
            transmit: make(
                "Verse imported water transmit",
                "fs_water_transmit",
                crate::water::TRANSMIT_BLEND,
            ),
            buffer,
            group,
            surface: None,
            source: None,
        }
    }

    /// Uploads `surface` for `tier`, or drops the one drawn.
    pub fn set_surface(
        &mut self,
        device: &wgpu::Device,
        surface: Option<Arc<WaterSurface>>,
        tier: verse_engine::quality::Tier,
    ) {
        self.surface = surface
            .as_deref()
            .map(|s| SurfaceGpu::upload(device, s, tier));
        self.source = surface;
    }

    /// Writes this frame's water seen through `view` on a target `height`
    /// pixels tall, and returns whether there is any to draw.
    pub fn prepare(
        &self,
        queue: &wgpu::Queue,
        water: Option<&Water>,
        view: verse_engine::presentation::View,
        height: u32,
    ) -> bool {
        let Some(water) = water.filter(|w| w.valid()) else {
            return false;
        };
        if self.surface.as_ref().is_none_or(|s| s.count() == 0) {
            return false;
        }
        let mut uniform = water.uniform();
        uniform.look[1] = crate::water::pixel_angle(view.view_proj, view.eye, height);
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&uniform));
        true
    }

    /// Draws the surface; the frame's group is already bound at 0.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if let Some(surface) = &self.surface {
            pass.set_bind_group(1, &self.group, &[]);
            surface.draw(pass, &self.transmit, &self.emit);
        }
    }

    /// The bytes the surface and the normal tile hold on the GPU.
    pub fn bytes(&self) -> u64 {
        self.surface.as_ref().map_or(0, SurfaceGpu::bytes) + crate::water::tile::bytes()
    }
}
