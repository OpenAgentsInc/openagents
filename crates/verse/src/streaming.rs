//! Native source workers and incremental GPU residency for cooked static chunks.
use std::{collections::BTreeMap, path::Path, time::Instant};
use verse_engine::streaming::store::{Store, Workers};
use verse_engine::streaming::{Budget, Kind, Manifest, Metrics, Residency};

#[derive(Clone, Copy, Default, Debug, serde::Serialize)]
pub struct Frame {
    pub source_completions: usize,
    pub source_starts: usize,
    pub upload_bytes: usize,
    pub upload_calls: usize,
    pub cpu_ms: f64,
    pub time_budget_exceeded: bool,
    pub visible_geometry: usize,
}
enum GpuChunk {
    Geometry(wgpu::Buffer),
    Image {
        texture: wgpu::Texture,
        group: wgpu::BindGroup,
    },
}
struct Pipelines {
    ordinary: [wgpu::RenderPipeline; 2],
    physical: [wgpu::RenderPipeline; 2],
    images: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    white: wgpu::BindGroup,
}
pub struct Source {
    residency: Residency,
    source: Store,
    workers: Workers,
    chunks: BTreeMap<String, GpuChunk>,
    pipelines: Pipelines,
    pub last_frame: Frame,
}
impl Source {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        root: &Path,
        manifest: Manifest,
        budget: Budget,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        globals: &wgpu::BindGroupLayout,
        ordinary: wgpu::TextureFormat,
        physical: wgpu::TextureFormat,
        samples: u32,
    ) -> Result<Self, String> {
        let residency = Residency::new(manifest, budget)?;
        validate_device(residency.manifest(), device)?;
        let source = Store::open(root)?;
        let workers = Workers::new(budget.source_jobs)?;
        let pipelines = Pipelines::new(device, queue, globals, ordinary, physical, samples);
        Ok(Self {
            residency,
            source,
            workers,
            chunks: BTreeMap::new(),
            pipelines,
            last_frame: Frame::default(),
        })
    }
    pub fn request(&mut self, roots: &[String]) -> Result<(), String> {
        self.residency.request(roots)
    }
    pub fn cancel_view(&mut self) -> Result<(), String> {
        self.residency
            .change_zone(self.residency.manifest().clone())?;
        self.chunks.clear();
        Ok(())
    }
    pub fn retry_failed(&mut self, id: &str) {
        self.residency.retry_failed(id);
    }
    pub fn metrics(&self) -> Metrics {
        self.residency.metrics()
    }
    pub fn budget(&self) -> Budget {
        self.residency.budget()
    }
    pub fn manifest_identity(&self) -> Result<String, String> {
        self.residency.manifest().identity()
    }
    pub fn visible_geometry(&self) -> usize {
        self.residency
            .visible()
            .into_iter()
            .filter(|id| {
                !matches!(
                    self.residency.manifest().chunks[*id].payload,
                    Kind::Image { .. }
                )
            })
            .count()
    }
    pub fn change_zone(
        &mut self,
        root: &Path,
        manifest: Manifest,
        device: &wgpu::Device,
    ) -> Result<(), String> {
        manifest.validate()?;
        validate_device(&manifest, device)?;
        let source = Store::open(root)?;
        self.residency.change_zone(manifest)?;
        self.source = source;
        self.chunks.clear();
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rebind(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        globals: &wgpu::BindGroupLayout,
        ordinary: wgpu::TextureFormat,
        physical: wgpu::TextureFormat,
        samples: u32,
    ) -> Result<(), String> {
        validate_device(self.residency.manifest(), device)?;
        self.residency.device_lost()?;
        self.chunks.clear();
        self.pipelines = Pipelines::new(device, queue, globals, ordinary, physical, samples);
        Ok(())
    }
    /// At most the admitted number of source completions and upload bytes per frame.
    /// A time budget stops before the next driver call; a call already in progress is measured.
    pub(crate) fn pump(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), String> {
        let start = Instant::now();
        let budget = self.residency.budget();
        let mut frame = Frame::default();
        while frame.source_completions < budget.source_jobs {
            let Some((ticket, result)) = self.workers.poll() else {
                break;
            };
            self.residency.source_result(ticket, result)?;
            frame.source_completions += 1;
        }
        while let Some(ticket) = self.residency.next_source()? {
            if let Err(error) = self.workers.submit(
                ticket.clone(),
                self.residency.manifest(),
                self.source.clone(),
            ) {
                self.residency.source_result(ticket, Err(error))?;
                break;
            }
            frame.source_starts += 1;
        }
        self.chunks.retain(|id, _| self.residency.gpu_allocated(id));
        while frame.upload_bytes < budget.upload_bytes_per_frame
            && start.elapsed().as_secs_f64() * 1000. < budget.upload_ms_per_frame
        {
            let Some(job) = self
                .residency
                .next_upload(budget.upload_bytes_per_frame - frame.upload_bytes)
            else {
                break;
            };
            // Release evicted resources before reserving storage for the replacement chunk.
            self.chunks.retain(|id, _| job.gpu_allocated(id));
            let id = job.ticket.id().to_owned();
            if job.allocate {
                let gpu = match job.descriptor.payload {
                    Kind::Triangles { .. } | Kind::Lines { .. } => {
                        GpuChunk::Geometry(device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("Verse cooked geometry"),
                            size: job.descriptor.gpu_bytes(),
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        }))
                    }
                    Kind::Image { width, height } => {
                        let texture = image(device, width, height);
                        let group = group(
                            device,
                            &self.pipelines.images,
                            &self.pipelines.sampler,
                            &texture,
                        );
                        GpuChunk::Image { texture, group }
                    }
                };
                self.chunks.insert(id.clone(), gpu);
            }
            let written = match self
                .chunks
                .get(&id)
                .ok_or("Admitted GPU chunk is missing")?
            {
                GpuChunk::Geometry(buffer) => {
                    queue.write_buffer(buffer, job.offset as u64, job.bytes);
                    job.bytes.len()
                }
                GpuChunk::Image { texture, .. } => {
                    let Kind::Image { width, .. } = job.descriptor.payload else {
                        return Err("GPU image descriptor differs".into());
                    };
                    let row = width as usize * 4;
                    let x = job.offset % row;
                    let y = job.offset / row;
                    let (bytes, w, h) = if x > 0 || job.bytes.len() < row {
                        let bytes = job.bytes.len().min(row - x);
                        (bytes, (bytes / 4) as u32, 1)
                    } else {
                        let rows = job.bytes.len() / row;
                        (rows * row, width, rows as u32)
                    };
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: (x / 4) as u32,
                                y: y as u32,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &job.bytes[..bytes],
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(w * 4),
                            rows_per_image: Some(h),
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                    bytes
                }
            };
            let ticket = job.ticket.clone();
            let offset = job.offset;
            self.residency.uploaded(&ticket, offset, written)?;
            frame.upload_bytes += written;
            frame.upload_calls += 1;
        }
        // Eviction during admission can remove another GPU entry after the first prune.
        self.chunks.retain(|id, _| self.residency.gpu_allocated(id));
        frame.cpu_ms = start.elapsed().as_secs_f64() * 1000.;
        frame.time_budget_exceeded = frame.cpu_ms > budget.upload_ms_per_frame;
        frame.visible_geometry = self.visible_geometry();
        self.last_frame = frame;
        Ok(())
    }
    pub(crate) fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        globals: &'a wgpu::BindGroup,
        physical: bool,
    ) {
        let pipelines = if physical {
            &self.pipelines.physical
        } else {
            &self.pipelines.ordinary
        };
        for id in self.residency.visible() {
            let d = &self.residency.manifest().chunks[id];
            let (vertices, lines) = match d.payload {
                Kind::Triangles { vertices } => (vertices, false),
                Kind::Lines { vertices } => (vertices, true),
                Kind::Image { .. } => continue,
            };
            let Some(GpuChunk::Geometry(buffer)) = self.chunks.get(id) else {
                continue;
            };
            let material = d
                .image
                .as_ref()
                .and_then(|id| self.chunks.get(id))
                .and_then(|gpu| match gpu {
                    GpuChunk::Image { group, .. } => Some(group),
                    _ => None,
                })
                .unwrap_or(&self.pipelines.white);
            pass.set_pipeline(&pipelines[usize::from(lines)]);
            pass.set_bind_group(0, globals, &[]);
            pass.set_bind_group(1, material, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..vertices, 0..1);
        }
    }
}
fn validate_device(manifest: &Manifest, device: &wgpu::Device) -> Result<(), String> {
    if cfg!(target_endian = "big") {
        return Err("Cooked GPU geometry requires a little-endian target".into());
    }
    for d in manifest.chunks.values() {
        match d.payload {
            Kind::Image { width, height }
                if width > device.limits().max_texture_dimension_2d
                    || height > device.limits().max_texture_dimension_2d =>
            {
                return Err("Cooked image exceeds device extent".into());
            }
            Kind::Triangles { .. } | Kind::Lines { .. }
                if d.gpu_bytes() > device.limits().max_buffer_size =>
            {
                return Err("Cooked geometry exceeds device buffer extent".into());
            }
            _ => {}
        }
    }
    Ok(())
}
fn image(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Verse cooked sRGB image"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}
fn group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    texture: &wgpu::Texture,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Verse cooked image binding"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &texture.create_view(&Default::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}
impl Pipelines {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        globals: &wgpu::BindGroupLayout,
        ordinary: wgpu::TextureFormat,
        physical: wgpu::TextureFormat,
        samples: u32,
    ) -> Self {
        let images = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Verse cooked image layout"),
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
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Verse cooked image sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let texture = image(device, 1, 1);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255; 4],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white = group(device, &images, &sampler, &texture);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Verse cooked static shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("streaming.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Verse cooked pipeline layout"),
            bind_group_layouts: &[Some(globals), Some(&images)],
            immediate_size: 0,
        });
        let attributes =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32];
        let pipeline = |format, reverse, lines| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Verse cooked static pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(if reverse { "vs_reverse" } else { "vs" }),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 36,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    }],
                },
                primitive: wgpu::PrimitiveState {
                    topology: if lines {
                        wgpu::PrimitiveTopology::LineList
                    } else {
                        wgpu::PrimitiveTopology::TriangleList
                    },
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(if reverse {
                        wgpu::CompareFunction::GreaterEqual
                    } else {
                        wgpu::CompareFunction::LessEqual
                    }),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            ordinary: [
                pipeline(ordinary, false, false),
                pipeline(ordinary, false, true),
            ],
            physical: [
                pipeline(physical, true, false),
                pipeline(physical, true, true),
            ],
            images,
            sampler,
            white,
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn cooked_shader_validates_both_depth_conventions() {
        let module = naga::front::wgsl::parse_str(include_str!("streaming.wgsl")).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        let mut options = naga::back::glsl::Options {
            version: naga::back::glsl::Version::Embedded {
                version: 300,
                is_webgl: true,
            },
            writer_flags: naga::back::glsl::WriterFlags::empty(),
            binding_map: Default::default(),
            zero_initialize_workgroup_memory: true,
        };
        options.binding_map.insert(
            naga::ResourceBinding {
                group: 0,
                binding: 0,
            },
            0,
        );
        options.binding_map.insert(
            naga::ResourceBinding {
                group: 1,
                binding: 0,
            },
            1,
        );
        options.binding_map.insert(
            naga::ResourceBinding {
                group: 1,
                binding: 1,
            },
            2,
        );
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        for (entry, stage) in [
            ("vs", naga::ShaderStage::Vertex),
            ("vs_reverse", naga::ShaderStage::Vertex),
            ("fs", naga::ShaderStage::Fragment),
        ] {
            let mut text = String::new();
            let pipeline_options = naga::back::glsl::PipelineOptions {
                shader_stage: stage,
                entry_point: entry.into(),
                multiview: None,
            };
            let mut writer = naga::back::glsl::Writer::new(
                &mut text,
                &module,
                &info,
                &options,
                &pipeline_options,
                naga::proc::BoundsCheckPolicies::default(),
            )
            .unwrap();
            writer.write().unwrap();
        }
    }
    #[test]
    #[ignore = "Explicit scratch GPU: partial image rows, both depth conventions, retained rebuild"]
    fn gpu_partial_uploads_draw_and_rebuild_without_source_files() {
        use super::*;
        use verse_engine::streaming::{Vertex, cook_geometry, cook_image, store::install};
        use wgpu::util::DeviceExt;
        fn open(instance: &wgpu::Instance) -> (wgpu::Device, wgpu::Queue) {
            let adapter =
                pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_limits: adapter.limits(),
                ..Default::default()
            }))
            .unwrap()
        }
        fn globals(device: &wgpu::Device) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Cooked test globals"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
            let mut values = [0f32; 24];
            values[..16].copy_from_slice(&glam::Mat4::IDENTITY.to_cols_array());
            values[23] = 100.;
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cooked test camera"),
                contents: bytemuck::cast_slice(&values),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            (layout, group)
        }
        fn uploaded(source: &mut Source, device: &wgpu::Device, queue: &wgpu::Queue) {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while source.visible_geometry() == 0 {
                assert!(std::time::Instant::now() < deadline);
                source.pump(device, queue).unwrap();
                assert!(source.last_frame.upload_bytes <= 4);
                queue.submit([]);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        fn pixels(
            source: &Source,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            globals: &wgpu::BindGroup,
            reverse: bool,
        ) -> Vec<u8> {
            let make = |format, usage| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: 16,
                        height: 16,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
            };
            let color = make(
                wgpu::TextureFormat::Rgba8UnormSrgb,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            );
            let depth = make(
                wgpu::TextureFormat::Depth32Float,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
            let color_view = color.create_view(&Default::default());
            let depth_view = depth.create_view(&Default::default());
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &color_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(if reverse { 0. } else { 1. }),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                source.draw(&mut pass, globals, reverse);
            }
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 256 * 16,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &color,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(16),
                    },
                },
                wgpu::Extent3d {
                    width: 16,
                    height: 16,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit([encoder.finish()]);
            let (send, receive) = std::sync::mpsc::channel();
            buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = send.send(r);
            });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(5)),
                })
                .unwrap();
            receive
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .unwrap();
            let data = buffer.slice(..).get_mapped_range().to_vec();
            buffer.unmap();
            data
        }
        let root = tempfile::tempdir().unwrap();
        let (image, bytes) = cook_image(3, 2, &[0, 255, 0, 255].repeat(6)).unwrap();
        install(root.path(), &image, &bytes).unwrap();
        let vertices: Vec<_> = [[-0.8, -0.8, 0.25], [0.8, -0.8, 0.25], [0., 0.8, 0.25]]
            .into_iter()
            .map(|pos| Vertex {
                pos,
                color: [1.; 3],
                uv: [0.5; 2],
                fog: 0.,
            })
            .collect();
        let (geometry, bytes) = cook_geometry(
            &vertices,
            false,
            vec![image.sha256.clone()],
            Some(image.sha256.clone()),
            None,
        )
        .unwrap();
        install(root.path(), &geometry, &bytes).unwrap();
        let id = geometry.sha256.clone();
        let manifest = Manifest {
            version: 1,
            chunks: BTreeMap::from([(image.sha256.clone(), image), (id.clone(), geometry)]),
        };
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let (mut device, mut queue) = open(&instance);
        let (mut layout, mut group) = globals(&device);
        let budget = Budget {
            cpu_bytes: 1024,
            gpu_bytes: 1024,
            source_jobs: 1,
            upload_bytes_per_frame: 4,
            upload_ms_per_frame: 1.,
        };
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let mut source = Source::new(
            root.path(),
            manifest,
            budget,
            &device,
            &queue,
            &layout,
            format,
            format,
            1,
        )
        .unwrap();
        source.request(&[id]).unwrap();
        uploaded(&mut source, &device, &queue);
        let ordinary = pixels(&source, &device, &queue, &group, false);
        assert_eq!(
            &ordinary[8 * 256 + 8 * 4..8 * 256 + 8 * 4 + 4],
            &[0, 255, 0, 255]
        );
        assert_eq!(ordinary, pixels(&source, &device, &queue, &group, true));
        let starts = source.metrics().source_starts;
        std::fs::remove_dir_all(root.path()).unwrap();
        device.destroy();
        (device, queue) = open(&instance);
        (layout, group) = globals(&device);
        source
            .rebind(&device, &queue, &layout, format, format, 1)
            .unwrap();
        uploaded(&mut source, &device, &queue);
        assert_eq!(starts, source.metrics().source_starts);
        assert_eq!(ordinary, pixels(&source, &device, &queue, &group, false));
        println!(
            "Partial image rows and both depth conventions match; source-independent replacement device retains {} CPU bytes / {} GPU bytes",
            source.metrics().cpu_bytes,
            source.metrics().gpu_bytes
        );
    }
}
