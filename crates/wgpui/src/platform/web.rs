//! Web platform implementation using wgpu + web-sys
//!
//! Provides browser-based rendering with wgpu and event handling via web-sys.

use crate::element::{Element, LayoutContext, PaintContext};
use crate::layout::{LayoutEngine, Size};
use crate::scene::{Quad, Scene, TextQuad};
use crate::text::TextSystem;

use bytemuck::{Pod, Zeroable};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;

/// Global uniforms for shaders
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Globals {
    viewport_size: [f32; 2],
    _pad: [f32; 2],
}

/// WGSL shader for rendering quads with rounded corners and borders
const QUAD_SHADER: &str = r#"
struct GlobalParams {
    viewport_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> globals: GlobalParams;

struct QuadInstance {
    @location(0) origin: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) background: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_widths: vec4<f32>,
    @location(5) corner_radii: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) background: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_widths: vec4<f32>,
    @location(5) corner_radii: vec4<f32>,
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
    instance: QuadInstance,
) -> VertexOutput {
    let x = f32(vertex_id & 1u);
    let y = f32((vertex_id >> 1u) & 1u);
    let unit_pos = vec2<f32>(x, y);
    let world_pos = instance.origin + unit_pos * instance.size;
    let ndc = (world_pos / globals.viewport_size) * 2.0 - 1.0;

    var output: VertexOutput;
    output.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    output.local_pos = unit_pos * instance.size;
    output.size = instance.size;
    output.background = instance.background;
    output.border_color = instance.border_color;
    output.border_widths = instance.border_widths;
    output.corner_radii = instance.corner_radii;
    return output;
}

fn rounded_box_sdf(pos: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let half_size = size * 0.5;
    let center_pos = pos - half_size;
    let q = abs(center_pos) - half_size + radius;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

fn pick_corner_radius(pos: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let half = size * 0.5;
    let center = pos - half;
    if (center.x < 0.0) {
        if (center.y < 0.0) { return radii.x; }
        else { return radii.w; }
    } else {
        if (center.y < 0.0) { return radii.y; }
        else { return radii.z; }
    }
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let bg_color = hsla_to_rgba(input.background);
    let border_col = hsla_to_rgba(input.border_color);
    let radius = pick_corner_radius(input.local_pos, input.size, input.corner_radii);
    let outer_dist = rounded_box_sdf(input.local_pos, input.size, radius);

    let border_width = max(max(input.border_widths.x, input.border_widths.y),
                          max(input.border_widths.z, input.border_widths.w));
    let inner_size = input.size - vec2<f32>(border_width * 2.0);
    let inner_pos = input.local_pos - vec2<f32>(border_width);
    let inner_radius = max(0.0, radius - border_width);
    let inner_dist = rounded_box_sdf(inner_pos, inner_size, inner_radius);

    let aa = 1.0;
    if (outer_dist > aa) { discard; }

    let outer_alpha = 1.0 - smoothstep(-aa, aa, outer_dist);
    var color: vec4<f32>;
    if (border_width > 0.0 && inner_dist > -aa) {
        let border_alpha = 1.0 - smoothstep(-aa, aa, inner_dist);
        color = mix(bg_color, border_col, border_alpha);
    } else {
        color = bg_color;
    }
    color.a *= outer_alpha;
    return vec4<f32>(color.rgb * color.a, color.a);
}
"#;

/// Text shader WGSL source
const TEXT_SHADER: &str = r#"
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

/// Web platform state for running wgpui in the browser
pub struct WebPlatform {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    quad_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    text_system: TextSystem,
    layout_engine: LayoutEngine,
    scene: Scene,
    scale_factor: f32,
    logical_width: f32,
    logical_height: f32,
}

impl WebPlatform {
    /// Create a new web platform with the given canvas
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let logical_width = canvas.client_width() as u32;
        let logical_height = canvas.client_height() as u32;

        // Set canvas size with device pixel ratio
        let dpr = web_sys::window().unwrap().device_pixel_ratio() as f32;
        let physical_width = (logical_width as f32 * dpr) as u32;
        let physical_height = (logical_height as f32 * dpr) as u32;
        canvas.set_width(physical_width);
        canvas.set_height(physical_height);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| format!("Failed to create surface: {e}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or("No adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("wgpui"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to get device: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: physical_width,
            height: physical_height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create globals buffer and bind group
        let globals = Globals {
            viewport_size: [logical_width as f32, logical_height as f32],
            _pad: [0.0, 0.0],
        };
        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::bytes_of(&globals),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let globals_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("globals_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals_bind_group"),
            layout: &globals_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        // Create quad pipeline
        let quad_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad_shader"),
            source: wgpu::ShaderSource::Wgsl(QUAD_SHADER.into()),
        });

        let quad_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad_pipeline_layout"),
            bind_group_layouts: &[&globals_bind_group_layout],
            push_constant_ranges: &[],
        });

        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad_pipeline"),
            layout: Some(&quad_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &quad_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Quad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 64,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &quad_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create text system and pipeline
        let mut text_system = TextSystem::new();
        text_system.set_scale_factor(dpr);
        let text_bind_group_layout = create_text_bind_group_layout(&device);
        text_system.init_gpu(&device, &text_bind_group_layout);

        let text_pipeline =
            create_text_pipeline(&device, format, &globals_bind_group_layout, &text_bind_group_layout);

        Ok(Self {
            device,
            queue,
            surface,
            config,
            quad_pipeline,
            text_pipeline,
            globals_buffer,
            globals_bind_group,
            text_system,
            layout_engine: LayoutEngine::new(),
            scene: Scene::new(),
            scale_factor: dpr,
            logical_width: logical_width as f32,
            logical_height: logical_height as f32,
        })
    }

    /// Get the current viewport size
    pub fn viewport_size(&self) -> Size {
        Size::new(self.logical_width, self.logical_height)
    }

    /// Get the scale factor (DPR)
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// Resize the viewport
    pub fn resize(&mut self, logical_width: u32, logical_height: u32, scale_factor: f32) {
        if logical_width > 0 && logical_height > 0 {
            self.scale_factor = scale_factor;
            self.logical_width = logical_width as f32;
            self.logical_height = logical_height as f32;

            // Update text system scale factor for crisp text
            self.text_system.set_scale_factor(scale_factor);

            // Physical size for the surface
            let physical_width = (logical_width as f32 * scale_factor) as u32;
            let physical_height = (logical_height as f32 * scale_factor) as u32;
            self.config.width = physical_width;
            self.config.height = physical_height;
            self.surface.configure(&self.device, &self.config);

            // Use LOGICAL dimensions for viewport
            let globals = Globals {
                viewport_size: [logical_width as f32, logical_height as f32],
                _pad: [0.0, 0.0],
            };
            self.queue
                .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
        }
    }

    /// Render a frame with the given root element
    pub fn render<E: Element>(&mut self, root: &mut E) -> Result<(), wgpu::SurfaceError> {
        // Clear scene
        self.scene.clear();
        self.layout_engine.clear();

        // Layout phase
        let mut layout_cx = LayoutContext {
            layout_engine: &mut self.layout_engine,
            text_system: &mut self.text_system,
        };
        let (layout_id, mut state) = root.request_layout(&mut layout_cx);

        // Compute layout
        self.layout_engine.compute_layout(
            layout_id,
            Size::new(self.logical_width, self.logical_height),
        );

        // Get root bounds
        let bounds = self.layout_engine.layout(layout_id);

        // Paint phase
        let mut paint_cx = PaintContext {
            scene: &mut self.scene,
            text_system: &mut self.text_system,
            layout_engine: &self.layout_engine,
        };
        root.paint(bounds, &mut state, &mut paint_cx);

        // Process text runs into text quads
        for text_run in &self.scene.text_runs {
            let quads = self.text_system.layout(
                &text_run.text,
                text_run.position,
                text_run.font_size,
                text_run.color,
            );
            self.scene.text_quads.extend(quads);
        }

        // Update text atlas
        self.text_system.update_atlas(&self.queue);

        // Render to surface
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("render") });

        // Create buffers
        let quad_buffer = if !self.scene.quads.is_empty() {
            Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("quads"),
                        contents: bytemuck::cast_slice(&self.scene.quads),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        } else {
            None
        };

        let text_buffer = if !self.scene.text_quads.is_empty() {
            Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("text_quads"),
                        contents: bytemuck::cast_slice(&self.scene.text_quads),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        } else {
            None
        };

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Render quads
            if let Some(ref buf) = quad_buffer {
                pass.set_pipeline(&self.quad_pipeline);
                pass.set_bind_group(0, &self.globals_bind_group, &[]);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..4, 0..self.scene.quads.len() as u32);
            }

            // Render text
            if let (Some(buf), Some(bind_group)) =
                (&text_buffer, &self.text_system.atlas_bind_group)
            {
                pass.set_pipeline(&self.text_pipeline);
                pass.set_bind_group(0, &self.globals_bind_group, &[]);
                pass.set_bind_group(1, bind_group, &[]);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..4, 0..self.scene.text_quads.len() as u32);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    /// Get mutable access to the text system (for measurement)
    pub fn text_system(&mut self) -> &mut TextSystem {
        &mut self.text_system
    }

    /// Get mutable access to the layout engine
    pub fn layout_engine(&mut self) -> &mut LayoutEngine {
        &mut self.layout_engine
    }
}

/// Create bind group layout for text rendering
fn create_text_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
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

/// Create the text rendering pipeline
fn create_text_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    globals_layout: &wgpu::BindGroupLayout,
    text_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("text_shader"),
        source: wgpu::ShaderSource::Wgsl(TEXT_SHADER.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("text_pipeline_layout"),
        bind_group_layouts: &[globals_layout, text_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("text_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<TextQuad>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x2,
                    },
                    wgpu::VertexAttribute {
                        offset: 8,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Float32x2,
                    },
                    wgpu::VertexAttribute {
                        offset: 16,
                        shader_location: 2,
                        format: wgpu::VertexFormat::Float32x2,
                    },
                    wgpu::VertexAttribute {
                        offset: 24,
                        shader_location: 3,
                        format: wgpu::VertexFormat::Float32x2,
                    },
                    wgpu::VertexAttribute {
                        offset: 32,
                        shader_location: 4,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                ],
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Run a wgpui application in the browser
pub async fn run_app<F, E>(canvas_id: &str, build_ui: F) -> Result<(), String>
where
    F: Fn() -> E + 'static,
    E: Element,
{
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).ok();

    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    // Get or create canvas
    let canvas: HtmlCanvasElement = if let Some(el) = document.get_element_by_id(canvas_id) {
        el.dyn_into().map_err(|_| "Not a canvas")?
    } else {
        let canvas: HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(|_| "Failed to create canvas")?
            .dyn_into()
            .map_err(|_| "Not a canvas")?;
        canvas.set_id(canvas_id);
        canvas.style().set_property("width", "100%").ok();
        canvas.style().set_property("height", "100%").ok();
        canvas.style().set_property("display", "block").ok();
        document
            .body()
            .ok_or("No body")?
            .append_child(&canvas)
            .map_err(|_| "Failed to append canvas")?;
        canvas
    };

    let platform = Rc::new(RefCell::new(WebPlatform::new(canvas.clone()).await?));

    // Hide loading indicator
    if let Some(loading) = document.get_element_by_id("loading") {
        loading.class_list().add_1("hidden").ok();
    }

    // Dispatch ready event
    let ready_event = web_sys::CustomEvent::new("wgpui-ready")
        .map_err(|_| "Failed to create event")?;
    window.dispatch_event(&ready_event).ok();

    log::info!("WGPUI initialized successfully");

    // Set up resize handler
    {
        let platform = platform.clone();
        let canvas = canvas.clone();
        let closure = Closure::<dyn Fn()>::new(move || {
            let dpr = web_sys::window().unwrap().device_pixel_ratio() as f32;
            let logical_width = canvas.client_width() as u32;
            let logical_height = canvas.client_height() as u32;
            let physical_width = (logical_width as f32 * dpr) as u32;
            let physical_height = (logical_height as f32 * dpr) as u32;
            canvas.set_width(physical_width);
            canvas.set_height(physical_height);
            platform
                .borrow_mut()
                .resize(logical_width, logical_height, dpr);
        });
        window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .ok();
        closure.forget();
    }

    // Animation loop
    fn request_animation_frame(f: &Closure<dyn FnMut()>) {
        web_sys::window()
            .unwrap()
            .request_animation_frame(f.as_ref().unchecked_ref())
            .unwrap();
    }

    let f = Rc::new(RefCell::new(None::<Closure<dyn FnMut()>>));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::new(move || {
        let mut root = build_ui();
        if let Err(e) = platform.borrow_mut().render(&mut root) {
            log::error!("Render error: {:?}", e);
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());

    log::info!("wgpui app running!");
    Ok(())
}
