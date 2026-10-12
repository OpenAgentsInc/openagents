//! Draws a take's pictures offscreen with wgpu: paths are flattened on the
//! CPU, filled on the GPU by stencil-then-cover (nonzero winding) under 4×
//! MSAA, stroked as quads per segment, and images drawn as textured quads;
//! records draw back to front by depth. The result is sRGB RGBA8 pixels.

use std::collections::BTreeMap;

use wgpu::util::DeviceExt;

use crate::take::{Frame, Kind, Record, Take, Upload, View};

const SAMPLES: u32 = 4;

const SHADER: &str = r"
struct Vertex { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32> }
struct Out { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32> }
struct Paint { color: vec4<f32>, textured: u32, pad0: u32, pad1: u32, pad2: u32 }

@group(0) @binding(0) var<uniform> paint: Paint;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var sampling: sampler;

@vertex
fn vs(v: Vertex) -> Out {
    var out: Out;
    out.clip = vec4<f32>(v.position, 0.0, 1.0);
    out.uv = v.uv;
    return out;
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    if paint.textured != 0u {
        return textureSample(picture, sampling, in.uv) * paint.color;
    }
    return paint.color;
}
";

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Paint {
    color: [f32; 4],
    textured: u32,
    pad: [u32; 3],
}

/// Flattened geometry of an uploaded path, in scene units.
struct Flat {
    /// Each subpath as a polyline (the path is one subpath here).
    points: Vec<[f32; 2]>,
    closed: bool,
}

/// A renderer that keeps its device, pipelines, and the take's uploads
/// between pictures, so a whole take costs one upload.
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    width: u32,
    height: u32,
    color: wgpu::TextureView,
    resolve: wgpu::Texture,
    resolve_view: wgpu::TextureView,
    stencil: wgpu::TextureView,
    readback: wgpu::Buffer,
    row: u32,
    layout: wgpu::BindGroupLayout,
    stencil_pipeline: wgpu::RenderPipeline,
    cover_pipeline: wgpu::RenderPipeline,
    plain_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    white: wgpu::TextureView,
    flats: BTreeMap<u64, Flat>,
    images: BTreeMap<u64, wgpu::TextureView>,
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    label: &str,
    write_color: bool,
    stencil: wgpu::StencilState,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
            }],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Stencil8,
            depth_write_enabled: None,
            depth_compare: None,
            stencil,
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: SAMPLES,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: if write_color {
                    wgpu::ColorWrites::ALL
                } else {
                    wgpu::ColorWrites::empty()
                },
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Renderer {
    /// A renderer for `take`'s size, with its uploads on the GPU.
    pub fn new(take: &Take) -> Result<Self, String> {
        let (width, height) = (take.width.max(1), take.height.max(1));
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|e| format!("no graphics adapter: {e}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("verse-film"),
            ..Default::default()
        }))
        .map_err(|e| format!("no graphics device: {e}"))?;

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = |label, format, samples, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = texture(
            "color",
            wgpu::TextureFormat::Rgba8UnormSrgb,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        let resolve = texture(
            "resolve",
            wgpu::TextureFormat::Rgba8UnormSrgb,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let resolve_view = resolve.create_view(&Default::default());
        let stencil = texture(
            "stencil",
            wgpu::TextureFormat::Stencil8,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("film"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("paint"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
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
            label: Some("film"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let wind = |pass| wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: pass,
        };
        let stencil_pipeline = pipeline(
            &device,
            &pipeline_layout,
            &shader,
            "stencil",
            false,
            wgpu::StencilState {
                front: wind(wgpu::StencilOperation::IncrementWrap),
                back: wind(wgpu::StencilOperation::DecrementWrap),
                read_mask: 0xff,
                write_mask: 0xff,
            },
        );
        let cover = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::NotEqual,
            fail_op: wgpu::StencilOperation::Zero,
            depth_fail_op: wgpu::StencilOperation::Zero,
            pass_op: wgpu::StencilOperation::Zero,
        };
        let cover_pipeline = pipeline(
            &device,
            &pipeline_layout,
            &shader,
            "cover",
            true,
            wgpu::StencilState {
                front: cover,
                back: cover,
                read_mask: 0xff,
                write_mask: 0xff,
            },
        );
        let plain_pipeline = pipeline(
            &device,
            &pipeline_layout,
            &shader,
            "plain",
            true,
            wgpu::StencilState::default(),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let upload_image = |w: u32, h: u32, pixels: &[u8]| {
            device
                .create_texture_with_data(
                    &queue,
                    &wgpu::TextureDescriptor {
                        label: Some("image"),
                        size: wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    pixels,
                )
                .create_view(&Default::default())
        };
        let white = upload_image(1, 1, &[255; 4]);

        let mut flats = BTreeMap::new();
        let mut images = BTreeMap::new();
        for (&key, upload) in &take.uploads {
            match upload {
                Upload::Path { closed, points } => {
                    flats.insert(
                        key,
                        Flat {
                            points: flatten(points),
                            closed: *closed,
                        },
                    );
                }
                Upload::Image {
                    width,
                    height,
                    pixels,
                } => {
                    if *width > 0 && *height > 0 {
                        images.insert(key, upload_image(*width, *height, pixels));
                    }
                }
            }
        }

        Ok(Self {
            device,
            queue,
            width,
            height,
            color,
            resolve,
            resolve_view,
            stencil,
            readback,
            row,
            layout,
            stencil_pipeline,
            cover_pipeline,
            plain_pipeline,
            sampler,
            white,
            flats,
            images,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Draws `frame` and returns its sRGB RGBA8 pixels, row-major.
    pub fn draw(&mut self, frame: &Frame, background: [f32; 4]) -> Result<Vec<u8>, String> {
        let mut records: Vec<&Record> = frame.records.iter().collect();
        records.sort_by(|a, b| a.depth.total_cmp(&b.depth).reverse());

        struct Draw {
            vertices: wgpu::Buffer,
            count: u32,
            bind: wgpu::BindGroup,
            fill: bool,
        }
        let mut draws: Vec<Draw> = Vec::new();
        let mut push =
            |verts: &[Vertex], color: [f32; 4], texture: Option<&wgpu::TextureView>, fill: bool| {
                if verts.is_empty() {
                    return;
                }
                let paint = Paint {
                    color,
                    textured: u32::from(texture.is_some()),
                    pad: [0; 3],
                };
                let uniform = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("paint"),
                        contents: bytemuck::bytes_of(&paint),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("paint"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniform.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(
                                texture.unwrap_or(&self.white),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                });
                let vertices = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("vertices"),
                        contents: bytemuck::cast_slice(verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                draws.push(Draw {
                    vertices,
                    count: verts.len() as u32,
                    bind,
                    fill,
                });
            };

        let to_clip = |view: View, t: [f32; 6], p: [f32; 2]| -> [f32; 2] {
            let x = t[0] * p[0] + t[2] * p[1] + t[4];
            let y = t[1] * p[0] + t[3] * p[1] + t[5];
            [
                (x - view.x) / view.width * 2.0 - 1.0,
                (y - view.y) / view.height * 2.0 - 1.0,
            ]
        };
        let aspect = frame.view.width / frame.view.height.max(1e-6);
        for record in records {
            match record.kind {
                Kind::Path => {
                    let Some(flat) = self.flats.get(&record.key) else {
                        continue;
                    };
                    let shown = reveal(&flat.points, flat.closed, record.reveal);
                    let clip: Vec<[f32; 2]> = shown
                        .iter()
                        .map(|p| to_clip(frame.view, record.transform, *p))
                        .collect();
                    if record.fill[3] > 0.0 && clip.len() >= 3 {
                        let mut fan = Vec::with_capacity((clip.len() - 2) * 3);
                        for i in 1..clip.len() - 1 {
                            for p in [clip[0], clip[i], clip[i + 1]] {
                                fan.push(Vertex {
                                    position: p,
                                    uv: [0.0, 0.0],
                                });
                            }
                        }
                        push(&fan, record.fill, None, true);
                    }
                    if record.stroke[3] > 0.0 && record.stroke_width > 0.0 && clip.len() >= 2 {
                        let closed = flat.closed && record.reveal == [0.0, 1.0];
                        let hw = record.stroke_width / frame.view.height;
                        let mut quads = Vec::new();
                        let n = clip.len();
                        let segments = if closed { n } else { n - 1 };
                        for i in 0..segments {
                            let a = clip[i];
                            let b = clip[(i + 1) % n];
                            let dx = (b[0] - a[0]) * aspect;
                            let dy = b[1] - a[1];
                            let len = (dx * dx + dy * dy).sqrt();
                            if len <= 0.0 {
                                continue;
                            }
                            let nx = -dy / len * hw / aspect;
                            let ny = dx / len * hw;
                            let corners = [
                                [a[0] + nx, a[1] + ny],
                                [a[0] - nx, a[1] - ny],
                                [b[0] - nx, b[1] - ny],
                                [b[0] + nx, b[1] + ny],
                            ];
                            for p in [
                                corners[0], corners[1], corners[2], corners[0], corners[2],
                                corners[3],
                            ] {
                                quads.push(Vertex {
                                    position: p,
                                    uv: [0.0, 0.0],
                                });
                            }
                        }
                        push(&quads, record.stroke, None, false);
                    }
                }
                Kind::Image => {
                    let Some(texture) = self.images.get(&record.key) else {
                        continue;
                    };
                    // The image fills the unit square from (-1,-1) to (1,1)
                    // in its own space, as the demo's square does.
                    let corners = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
                    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
                    let mut quad = Vec::with_capacity(6);
                    for i in [0, 1, 2, 0, 2, 3] {
                        quad.push(Vertex {
                            position: to_clip(frame.view, record.transform, corners[i]),
                            uv: uvs[i],
                        });
                    }
                    let tint = if record.fill[3] > 0.0 {
                        record.fill
                    } else {
                        [1.0; 4]
                    };
                    push(&quad, tint, Some(texture), false);
                }
            }
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("film"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("picture"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.color,
                    depth_slice: None,
                    resolve_target: Some(&self.resolve_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(background[0]),
                            g: f64::from(background[1]),
                            b: f64::from(background[2]),
                            a: f64::from(background[3]),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.stencil,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for draw in &draws {
                pass.set_bind_group(0, &draw.bind, &[]);
                pass.set_vertex_buffer(0, draw.vertices.slice(..));
                if draw.fill {
                    pass.set_pipeline(&self.stencil_pipeline);
                    pass.draw(0..draw.count, 0..1);
                    pass.set_pipeline(&self.cover_pipeline);
                    pass.set_stencil_reference(0);
                    pass.draw(0..draw.count, 0..1);
                } else {
                    pass.set_pipeline(&self.plain_pipeline);
                    pass.draw(0..draw.count, 0..1);
                }
            }
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.resolve,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|e| format!("the GPU did not finish: {e}"))?;
        let mut pixels = Vec::with_capacity((self.width * self.height * 4) as usize);
        {
            let mapped = slice.get_mapped_range();
            let stride = self.row as usize;
            let line = (self.width * 4) as usize;
            // Scene y goes up; pixel rows go down.
            for y in (0..self.height as usize).rev() {
                pixels.extend_from_slice(&mapped[y * stride..y * stride + line]);
            }
        }
        self.readback.unmap();
        Ok(pixels)
    }
}

/// Flattens cubic Bézier curves into a polyline, 16 steps per curve.
fn flatten(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut out = Vec::new();
    if points.is_empty() {
        return out;
    }
    out.push(points[0]);
    for curve in points[1..].chunks_exact(3) {
        let p0 = *out.last().unwrap();
        let [p1, p2, p3] = [curve[0], curve[1], curve[2]];
        let straight = p1 == p0 && p2 == p3 || (p1 == p0 && p2 == p0) || (p1 == p3 && p2 == p3);
        let steps = if straight { 1 } else { 16 };
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let u = 1.0 - t;
            let x = u * u * u * p0[0]
                + 3.0 * u * u * t * p1[0]
                + 3.0 * u * t * t * p2[0]
                + t * t * t * p3[0];
            let y = u * u * u * p0[1]
                + 3.0 * u * u * t * p1[1]
                + 3.0 * u * t * t * p2[1]
                + t * t * t * p3[1];
            out.push([x, y]);
        }
    }
    out
}

/// The part of a polyline between fractions `reveal[0]` and `reveal[1]` of
/// its length.
fn reveal(points: &[[f32; 2]], closed: bool, reveal: [f32; 2]) -> Vec<[f32; 2]> {
    if reveal == [0.0, 1.0] || points.len() < 2 {
        return points.to_vec();
    }
    let mut line = points.to_vec();
    if closed {
        line.push(points[0]);
    }
    let lengths: Vec<f32> = line
        .windows(2)
        .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
        .collect();
    let total: f32 = lengths.iter().sum();
    let (from, to) = (
        reveal[0].clamp(0.0, 1.0) * total,
        reveal[1].clamp(0.0, 1.0) * total,
    );
    let at = |d: f32| -> [f32; 2] {
        let mut run = 0.0;
        for (i, len) in lengths.iter().enumerate() {
            if run + len >= d || i == lengths.len() - 1 {
                let t = if *len > 0.0 {
                    ((d - run) / len).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (a, b) = (line[i], line[i + 1]);
                return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            }
            run += len;
        }
        line[0]
    };
    let mut out = vec![at(from)];
    let mut run = 0.0;
    for (i, len) in lengths.iter().enumerate() {
        run += len;
        if run > from && run < to {
            out.push(line[i + 1]);
        }
    }
    out.push(at(to));
    out
}

/// Writes sRGB RGBA8 pixels as a PNG.
pub fn write_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<(), String> {
    let file =
        std::fs::File::create(path).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_straight_curve_flattens_to_its_ends() {
        let flat = flatten(&[[0.0, 0.0], [0.0, 0.0], [1.0, 0.0], [1.0, 0.0]]);
        assert_eq!(flat, vec![[0.0, 0.0], [1.0, 0.0]]);
    }

    #[test]
    fn reveal_takes_the_middle_of_a_line() {
        let shown = reveal(&[[0.0, 0.0], [4.0, 0.0]], false, [0.25, 0.75]);
        assert_eq!(shown, vec![[1.0, 0.0], [3.0, 0.0]]);
    }

    #[test]
    fn the_demo_square_lands_in_the_middle_of_its_last_picture() {
        let take = crate::read(&crate::take::demo(3, 1)).unwrap();
        let Ok(mut renderer) = Renderer::new(&take) else {
            eprintln!("no GPU here; skipped");
            return;
        };
        let pixels = renderer
            .draw(&take.frames[1], [0.0, 0.0, 0.0, 1.0])
            .unwrap();
        let (w, h) = renderer.size();
        let at =
            |x: u32, y: u32| &pixels[((y * w + x) * 4) as usize..((y * w + x) * 4 + 4) as usize];
        let center = at(w / 2, h / 2);
        assert!(
            center[2] > 200 && center[0] < 200 && center[0] > 100,
            "center {center:?}"
        );
        let corner = at(2, 2);
        assert_eq!(&corner[..3], &[0, 0, 0], "corner {corner:?}");
    }
}
