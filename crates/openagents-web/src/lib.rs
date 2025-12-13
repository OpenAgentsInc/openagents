//! OpenAgents Web Demo
//!
//! A standalone wgpu-based UI demo that renders GPUI-style components.
//! Features Berkeley Mono font and theme_oa colors.

mod components;
mod text;
mod theme;

use bytemuck::{Pod, Zeroable};
use components::{ButtonStyle, GpuQuad};
use std::cell::RefCell;
use std::rc::Rc;
use text::{TextQuad, TextSystem};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;

/// Global uniforms
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Globals {
    viewport_size: [f32; 2],
    _pad: [f32; 2],
}

/// WGSL shader for rendering quads
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

/// Clickable region for hit testing
#[derive(Clone)]
struct ClickRegion {
    bounds: [f32; 4], // x, y, width, height
    id: String,
}

/// Input field state
struct InputState {
    value: String,
    placeholder: String,
    selected: bool,      // Whether all text is selected
    cursor_pos: usize,   // Cursor position (character index)
}

/// Input geometry for click position calculation
struct InputGeometry {
    text_x: f32,  // X position where text starts
    y: f32,
    w: f32,
    h: f32,
}

/// Application state
struct AppState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    quad_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    globals_bind_group_layout: wgpu::BindGroupLayout,
    text_system: TextSystem,
    quads: Vec<GpuQuad>,
    text_quads: Vec<TextQuad>,
    scale_factor: f32,
    logical_width: f32,
    logical_height: f32,
    click_regions: Vec<ClickRegion>,
    hovered_id: Option<String>,
    focused_id: Option<String>,
    inputs: std::collections::HashMap<String, InputState>,
    input_geometry: std::collections::HashMap<String, InputGeometry>,
    cursor_visible: bool,
    frame_count: u32,
}

impl AppState {
    async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
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

        log::info!("Adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("openagents-web"),
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
        // Use LOGICAL dimensions for viewport so UI coordinates work correctly
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
                    array_stride: std::mem::size_of::<GpuQuad>() as u64,
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
        let text_bind_group_layout = text::create_text_bind_group_layout(&device);
        text_system.init_gpu(&device, &text_bind_group_layout);

        let text_pipeline = text::create_text_pipeline(
            &device,
            format,
            &globals_bind_group_layout,
            &text_bind_group_layout,
        );

        // Initialize input states
        let mut inputs = std::collections::HashMap::new();
        inputs.insert("input_enter_your_name".to_string(), InputState {
            value: String::new(),
            placeholder: "Enter your name...".to_string(),
            selected: false,
            cursor_pos: 0,
        });
        inputs.insert("input_email_address".to_string(), InputState {
            value: "user@example.com".to_string(),
            placeholder: "Email address".to_string(),
            selected: false,
            cursor_pos: 16, // end of "user@example.com"
        });

        Ok(Self {
            device,
            queue,
            surface,
            config,
            quad_pipeline,
            text_pipeline,
            globals_buffer,
            globals_bind_group,
            globals_bind_group_layout,
            text_system,
            quads: Vec::new(),
            text_quads: Vec::new(),
            scale_factor: dpr,
            logical_width: logical_width as f32,
            logical_height: logical_height as f32,
            click_regions: Vec::new(),
            hovered_id: None,
            focused_id: None,
            inputs,
            input_geometry: std::collections::HashMap::new(),
            cursor_visible: true,
            frame_count: 0,
        })
    }

    fn build_ui(&mut self) {
        self.quads.clear();
        self.text_quads.clear();
        self.click_regions.clear();
        self.input_geometry.clear();

        // Toggle cursor every 30 frames (~0.5 sec at 60fps)
        self.frame_count += 1;
        if self.frame_count % 30 == 0 {
            self.cursor_visible = !self.cursor_visible;
        }

        // Use logical dimensions for UI layout
        let w = self.logical_width;
        let h = self.logical_height;

        // Background - pure black
        self.quads.push(GpuQuad::new(0.0, 0.0, w, h).bg(theme::bg::APP));

        // Header bar
        self.quads
            .push(GpuQuad::new(0.0, 0.0, w, 48.0).bg(theme::bg::HEADER));

        // Header title
        let title_quads = self.text_system.layout(
            "OpenAgents Web Demo",
            [16.0, 16.0],
            theme::FONT_SIZE_LG,
            theme::text::PRIMARY,
        );
        self.text_quads.extend(title_quads);

        // Main card
        let card_w = 480.0;
        let card_h = 320.0;
        let card_x = (w - card_w) / 2.0;
        let card_y = 80.0;

        self.quads.push(
            GpuQuad::new(card_x, card_y, card_w, card_h)
                .bg(theme::bg::CARD)
                .border(theme::border::DEFAULT, 1.0)
                .radius(8.0),
        );

        // Card title
        let card_title = self.text_system.layout(
            "UI Components Demo",
            [card_x + 20.0, card_y + 28.0],
            theme::FONT_SIZE_LG,
            theme::text::PRIMARY,
        );
        self.text_quads.extend(card_title);

        // Subtitle
        let subtitle = self.text_system.layout(
            "Berkeley Mono + theme_oa colors",
            [card_x + 20.0, card_y + 48.0],
            theme::FONT_SIZE,
            theme::text::MUTED,
        );
        self.text_quads.extend(subtitle);

        // Button row - different styles
        let btn_y = card_y + 80.0;
        let mut btn_x = card_x + 20.0;

        let button_configs = [
            ("Primary", ButtonStyle::Default),
            ("Secondary", ButtonStyle::Secondary),
            ("Ghost", ButtonStyle::Ghost),
            ("Outline", ButtonStyle::Outline),
        ];

        for (label, style) in button_configs {
            let width = self.text_system.measure(label, 11.0) + 16.0;
            self.render_button(btn_x, btn_y, width, 22.0, label, style);
            btn_x += width + 12.0;
        }

        // Button sizes row
        let sizes_y = btn_y + 40.0;
        let mut sizes_x = card_x + 20.0;

        let size_configs = [
            ("Large", 32.0, 12.0),
            ("Medium", 28.0, 11.0),
            ("Default", 22.0, 11.0),
            ("Compact", 18.0, 10.0),
        ];

        for (label, height, font_size) in size_configs {
            let width = self.text_system.measure(label, font_size) + 16.0;
            self.render_button_with_size(sizes_x, sizes_y, width, height, label, font_size, ButtonStyle::Secondary);
            sizes_x += width + 12.0;
        }

        // Input fields
        let input_y = sizes_y + 50.0;
        self.render_input(card_x + 20.0, input_y, 200.0, "input_enter_your_name");
        self.render_input(card_x + 240.0, input_y, 200.0, "input_email_address");

        // Destructive button
        let del_width = self.text_system.measure("Delete", 11.0) + 16.0;
        self.render_button(card_x + 20.0, input_y + 50.0, del_width, 22.0, "Delete", ButtonStyle::Destructive);

        // Status colors demo
        let status_y = card_y + card_h + 20.0;
        let status_label = self.text_system.layout(
            "Status Colors:",
            [card_x, status_y],
            theme::FONT_SIZE,
            theme::text::SECONDARY,
        );
        self.text_quads.extend(status_label);

        let status_colors = [
            ("Success", theme::status::SUCCESS),
            ("Error", theme::status::ERROR),
            ("Warning", theme::status::WARNING),
            ("Info", theme::status::INFO),
        ];

        let mut status_x = card_x + 100.0;
        for (label, color) in status_colors {
            let label_quads =
                self.text_system.layout(label, [status_x, status_y], theme::FONT_SIZE, color);
            self.text_quads.extend(label_quads);
            status_x += 80.0;
        }

        // Accent colors demo
        let accent_y = status_y + 24.0;
        let accent_label = self.text_system.layout(
            "Accent Colors:",
            [card_x, accent_y],
            theme::FONT_SIZE,
            theme::text::SECONDARY,
        );
        self.text_quads.extend(accent_label);

        let accent_colors = [
            ("Primary", theme::accent::PRIMARY),
            ("Blue", theme::accent::BLUE),
            ("Green", theme::accent::GREEN),
            ("Orange", theme::accent::ORANGE),
        ];

        let mut accent_x = card_x + 100.0;
        for (label, color) in accent_colors {
            let label_quads =
                self.text_system.layout(label, [accent_x, accent_y], theme::FONT_SIZE, color);
            self.text_quads.extend(label_quads);
            accent_x += 80.0;
        }
    }

    fn render_button(&mut self, x: f32, y: f32, w: f32, h: f32, label: &str, style: ButtonStyle) {
        self.render_button_with_size(x, y, w, h, label, 11.0, style);
    }

    fn render_button_with_size(&mut self, x: f32, y: f32, w: f32, h: f32, label: &str, font_size: f32, style: ButtonStyle) {
        let button_id = format!("btn_{}", label.to_lowercase().replace(' ', "_"));
        let is_hovered = self.hovered_id.as_ref() == Some(&button_id);

        let (bg, hover_bg, text_color, border) = match style {
            ButtonStyle::Default => (
                theme::ui::button::DEFAULT_BG,
                theme::ui::button::DEFAULT_HOVER_BG,
                theme::ui::button::DEFAULT_TEXT,
                None,
            ),
            ButtonStyle::Secondary => (
                theme::ui::button::SECONDARY_BG,
                theme::ui::button::SECONDARY_HOVER_BG,
                theme::ui::button::SECONDARY_TEXT,
                None,
            ),
            ButtonStyle::Ghost => (
                theme::ui::button::GHOST_BG,
                theme::ui::button::GHOST_HOVER_BG,
                theme::ui::button::GHOST_TEXT,
                None,
            ),
            ButtonStyle::Outline => (
                theme::ui::button::OUTLINE_BG,
                theme::ui::button::OUTLINE_HOVER_BG,
                theme::ui::button::OUTLINE_TEXT,
                Some(theme::ui::button::OUTLINE_BORDER),
            ),
            ButtonStyle::Destructive => (
                theme::ui::button::DESTRUCTIVE_BG,
                theme::ui::button::DESTRUCTIVE_HOVER_BG,
                theme::ui::button::DESTRUCTIVE_TEXT,
                None,
            ),
        };

        let actual_bg = if is_hovered { hover_bg } else { bg };
        let mut quad = GpuQuad::new(x, y, w, h).bg(actual_bg).radius(4.0);
        if let Some(border_color) = border {
            quad = quad.border(border_color, 1.0);
        }
        self.quads.push(quad);

        // Register click region
        self.click_regions.push(ClickRegion {
            bounds: [x, y, w, h],
            id: button_id,
        });

        // Center text vertically - position is TOP of text area, not baseline
        let text_w = self.text_system.measure(label, font_size);
        let line_height = font_size * 1.2;
        let text_x = x + (w - text_w) / 2.0;
        let text_y = y + (h - line_height) / 2.0;
        let text_quads = self.text_system.layout(label, [text_x, text_y], font_size, text_color);
        self.text_quads.extend(text_quads);
    }

    fn render_input(&mut self, x: f32, y: f32, w: f32, input_id: &str) {
        let h = 32.0;
        let is_hovered = self.hovered_id.as_ref().map(|s| s.as_str()) == Some(input_id);
        let is_focused = self.focused_id.as_ref().map(|s| s.as_str()) == Some(input_id);

        // Use focus border color when focused or hovered
        let border_color = if is_focused {
            theme::border::FOCUS
        } else if is_hovered {
            theme::accent::BLUE
        } else {
            theme::input::BORDER
        };

        self.quads.push(
            GpuQuad::new(x, y, w, h)
                .bg(theme::input::BG)
                .border(border_color, if is_focused { 2.0 } else { 1.0 })
                .radius(6.0),
        );

        // Register click region for input
        self.click_regions.push(ClickRegion {
            bounds: [x, y, w, h],
            id: input_id.to_string(),
        });

        // Center text vertically
        let line_height = theme::FONT_SIZE * 1.2;
        let text_y = y + (h - line_height) / 2.0;
        let text_x = x + 8.0;

        // Store geometry for click position calculation
        self.input_geometry.insert(input_id.to_string(), InputGeometry {
            text_x,
            y,
            w,
            h,
        });

        // Get text from input state
        let (text, color, is_selected, cursor_pos) = if let Some(state) = self.inputs.get(input_id) {
            if state.value.is_empty() {
                (state.placeholder.as_str(), theme::input::PLACEHOLDER, false, 0)
            } else {
                (state.value.as_str(), theme::input::TEXT, state.selected, state.cursor_pos)
            }
        } else {
            ("", theme::input::PLACEHOLDER, false, 0)
        };

        // Draw selection highlight if text is selected
        if is_selected && is_focused {
            let text_width = self.text_system.measure(text, theme::FONT_SIZE);
            let sel_y = y + 6.0;
            let sel_h = h - 12.0;
            self.quads.push(
                GpuQuad::new(text_x - 2.0, sel_y, text_width + 4.0, sel_h)
                    .bg(theme::input::SELECTION)
            );
        }

        let text_quads = self.text_system.layout(text, [text_x, text_y], theme::FONT_SIZE, color);
        self.text_quads.extend(text_quads);

        // Draw blinking cursor if focused (and not selected)
        if is_focused && self.cursor_visible && !is_selected {
            // Calculate cursor X based on cursor_pos
            let cursor_x = if let Some(state) = self.inputs.get(input_id) {
                if state.value.is_empty() {
                    text_x
                } else {
                    // Measure text up to cursor position
                    let text_before_cursor: String = state.value.chars().take(cursor_pos).collect();
                    text_x + self.text_system.measure(&text_before_cursor, theme::FONT_SIZE)
                }
            } else {
                text_x
            };
            let cursor_y = y + 6.0;
            let cursor_h = h - 12.0;

            // Draw cursor as a thin quad
            self.quads.push(
                GpuQuad::new(cursor_x, cursor_y, 1.5, cursor_h)
                    .bg(theme::input::CURSOR)
            );
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.build_ui();

        // Update text atlas
        self.text_system.update_atlas(&self.queue);

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render"),
            });

        // Create buffers
        let quad_buffer = if !self.quads.is_empty() {
            Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("quads"),
                        contents: bytemuck::cast_slice(&self.quads),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        } else {
            None
        };

        let text_buffer = if !self.text_quads.is_empty() {
            Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("text_quads"),
                        contents: bytemuck::cast_slice(&self.text_quads),
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
                pass.draw(0..4, 0..self.quads.len() as u32);
            }

            // Render text
            if let (Some(buf), Some(bind_group)) =
                (&text_buffer, &self.text_system.atlas_bind_group)
            {
                pass.set_pipeline(&self.text_pipeline);
                pass.set_bind_group(0, &self.globals_bind_group, &[]);
                pass.set_bind_group(1, bind_group, &[]);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..4, 0..self.text_quads.len() as u32);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    fn resize(&mut self, logical_width: u32, logical_height: u32, scale_factor: f32) {
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

            // Use LOGICAL dimensions for viewport so UI coordinates work correctly
            let globals = Globals {
                viewport_size: [logical_width as f32, logical_height as f32],
                _pad: [0.0, 0.0],
            };
            self.queue
                .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
        }
    }

    /// Hit test to find element at position
    fn hit_test(&self, x: f32, y: f32) -> Option<String> {
        // Test click regions in reverse order (top-most first)
        for region in self.click_regions.iter().rev() {
            let [rx, ry, rw, rh] = region.bounds;
            if x >= rx && x <= rx + rw && y >= ry && y <= ry + rh {
                return Some(region.id.clone());
            }
        }
        None
    }

    /// Handle mouse move for hover effects. Returns true if hovering over clickable element.
    fn on_mouse_move(&mut self, x: f32, y: f32) -> bool {
        self.hovered_id = self.hit_test(x, y);
        self.hovered_id.is_some()
    }

    /// Handle mouse click
    fn on_click(&mut self, click_x: f32, _click_y: f32) {
        if let Some(id) = self.hit_test(click_x, _click_y) {
            log::info!("Clicked: {}", id);
            // Set focus if clicking an input
            if id.starts_with("input_") {
                self.focused_id = Some(id.clone());
                self.cursor_visible = true;
                self.frame_count = 0;

                // Calculate cursor position based on click X
                if let Some(geom) = self.input_geometry.get(&id) {
                    if let Some(state) = self.inputs.get_mut(&id) {
                        state.selected = false;  // Clear selection on click

                        if state.value.is_empty() {
                            state.cursor_pos = 0;
                        } else {
                            // Find character position closest to click
                            let relative_x = click_x - geom.text_x;
                            let mut best_pos = 0;
                            let mut best_dist = relative_x.abs();

                            for i in 1..=state.value.chars().count() {
                                let text_before: String = state.value.chars().take(i).collect();
                                let char_x = self.text_system.measure(&text_before, theme::FONT_SIZE);
                                let dist = (relative_x - char_x).abs();
                                if dist < best_dist {
                                    best_dist = dist;
                                    best_pos = i;
                                }
                            }
                            state.cursor_pos = best_pos;
                        }
                    }
                }
            } else {
                // Clicking anything else clears focus
                self.focused_id = None;
            }
        } else {
            // Clicking empty space clears focus
            self.focused_id = None;
        }
    }

    /// Handle keyboard input
    fn on_key(&mut self, key: &str, is_printable: bool, is_cmd: bool) {
        if let Some(ref focused_id) = self.focused_id.clone() {
            if let Some(state) = self.inputs.get_mut(focused_id) {
                // Handle Cmd/Ctrl shortcuts
                if is_cmd {
                    match key.to_lowercase().as_str() {
                        "a" => {
                            // Select all
                            state.selected = !state.value.is_empty();
                            state.cursor_pos = state.value.chars().count();
                        }
                        _ => {}
                    }
                    return;
                }

                // If text is selected, typing replaces it
                if state.selected && is_printable {
                    state.value.clear();
                    state.cursor_pos = 0;
                    state.selected = false;
                }

                if is_printable {
                    // Insert at cursor position
                    let char_count = state.value.chars().count();
                    if state.cursor_pos >= char_count {
                        state.value.push_str(key);
                    } else {
                        let before: String = state.value.chars().take(state.cursor_pos).collect();
                        let after: String = state.value.chars().skip(state.cursor_pos).collect();
                        state.value = format!("{}{}{}", before, key, after);
                    }
                    state.cursor_pos += key.chars().count();
                    state.selected = false;
                    self.cursor_visible = true;
                    self.frame_count = 0;
                } else if key == "Backspace" {
                    if state.selected {
                        state.value.clear();
                        state.cursor_pos = 0;
                        state.selected = false;
                    } else if state.cursor_pos > 0 {
                        // Delete character before cursor
                        let before: String = state.value.chars().take(state.cursor_pos - 1).collect();
                        let after: String = state.value.chars().skip(state.cursor_pos).collect();
                        state.value = format!("{}{}", before, after);
                        state.cursor_pos -= 1;
                    }
                    self.cursor_visible = true;
                    self.frame_count = 0;
                } else if key == "Delete" {
                    if state.selected {
                        state.value.clear();
                        state.cursor_pos = 0;
                        state.selected = false;
                    } else if state.cursor_pos < state.value.chars().count() {
                        // Delete character after cursor
                        let before: String = state.value.chars().take(state.cursor_pos).collect();
                        let after: String = state.value.chars().skip(state.cursor_pos + 1).collect();
                        state.value = format!("{}{}", before, after);
                    }
                    self.cursor_visible = true;
                    self.frame_count = 0;
                } else if key == "ArrowLeft" && state.cursor_pos > 0 {
                    state.cursor_pos -= 1;
                    state.selected = false;
                    self.cursor_visible = true;
                    self.frame_count = 0;
                } else if key == "ArrowRight" && state.cursor_pos < state.value.chars().count() {
                    state.cursor_pos += 1;
                    state.selected = false;
                    self.cursor_visible = true;
                    self.frame_count = 0;
                } else if key == "Home" {
                    state.cursor_pos = 0;
                    state.selected = false;
                    self.cursor_visible = true;
                    self.frame_count = 0;
                } else if key == "End" {
                    state.cursor_pos = state.value.chars().count();
                    state.selected = false;
                    self.cursor_visible = true;
                    self.frame_count = 0;
                }
            }
        }
    }

    /// Handle select all (Cmd+A)
    fn select_all(&mut self) {
        if let Some(ref focused_id) = self.focused_id.clone() {
            if let Some(state) = self.inputs.get_mut(focused_id) {
                state.selected = !state.value.is_empty();
            }
        }
    }

    /// Get selected text for copy
    fn get_selected_text(&self) -> Option<String> {
        if let Some(ref focused_id) = self.focused_id {
            if let Some(state) = self.inputs.get(focused_id) {
                if state.selected {
                    return Some(state.value.clone());
                }
            }
        }
        None
    }

    /// Cut selected text
    fn cut_selected(&mut self) -> Option<String> {
        if let Some(ref focused_id) = self.focused_id.clone() {
            if let Some(state) = self.inputs.get_mut(focused_id) {
                if state.selected {
                    let text = state.value.clone();
                    state.value.clear();
                    state.selected = false;
                    return Some(text);
                }
            }
        }
        None
    }

    /// Paste text
    fn paste(&mut self, text: &str) {
        if let Some(ref focused_id) = self.focused_id.clone() {
            if let Some(state) = self.inputs.get_mut(focused_id) {
                if state.selected {
                    state.value.clear();
                    state.selected = false;
                }
                state.value.push_str(text);
                self.cursor_visible = true;
                self.frame_count = 0;
            }
        }
    }
}

#[wasm_bindgen(start)]
pub async fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).ok();
    log::info!("OpenAgents Web Demo starting...");

    if let Err(e) = run().await {
        log::error!("Error: {}", e);
    }
}

async fn run() -> Result<(), String> {
    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    // Hide loading indicator
    if let Some(loading) = document.get_element_by_id("loading") {
        loading.class_list().add_1("hidden").ok();
    }

    // Create canvas
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|_| "Failed to create canvas")?
        .dyn_into()
        .map_err(|_| "Not a canvas")?;

    canvas.set_id("gpui-canvas");
    canvas.style().set_property("width", "100%").ok();
    canvas.style().set_property("height", "100%").ok();
    canvas.style().set_property("display", "block").ok();

    document
        .body()
        .ok_or("No body")?
        .append_child(&canvas)
        .map_err(|_| "Failed to append canvas")?;

    let state = Rc::new(RefCell::new(AppState::new(canvas.clone()).await?));

    // Set up resize handler
    {
        let state = state.clone();
        let canvas = canvas.clone();
        let closure = Closure::<dyn Fn()>::new(move || {
            let dpr = web_sys::window().unwrap().device_pixel_ratio() as f32;
            let logical_width = canvas.client_width() as u32;
            let logical_height = canvas.client_height() as u32;
            // Set canvas to physical size for proper rendering
            let physical_width = (logical_width as f32 * dpr) as u32;
            let physical_height = (logical_height as f32 * dpr) as u32;
            canvas.set_width(physical_width);
            canvas.set_height(physical_height);
            // Pass logical dimensions to resize
            state.borrow_mut().resize(logical_width, logical_height, dpr);
        });
        window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .ok();
        closure.forget();
    }

    // Set up mouse move handler for hover effects
    {
        let state = state.clone();
        let canvas_for_cursor = canvas.clone();
        let canvas_for_listener = canvas.clone();
        let closure = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
            // Use offset coordinates which are relative to the element
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;
            let has_hover = state.borrow_mut().on_mouse_move(x, y);
            // Update cursor style based on hover state
            let cursor = if has_hover { "pointer" } else { "default" };
            canvas_for_cursor.style().set_property("cursor", cursor).ok();
        });
        canvas_for_listener
            .add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())
            .ok();
        closure.forget();
    }

    // Set up click handler
    {
        let state = state.clone();
        let canvas_for_click = canvas.clone();
        let closure = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
            // Use offset coordinates which are relative to the element
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;
            state.borrow_mut().on_click(x, y);
        });
        canvas_for_click
            .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
            .ok();
        closure.forget();
    }

    // Set up keyboard handler for text input
    {
        let state = state.clone();
        let closure = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
            let key = event.key();
            let is_cmd = event.meta_key() || event.ctrl_key();

            // Check if it's a printable character (single char and not a control key)
            let is_printable = key.len() == 1 && !event.ctrl_key() && !event.meta_key() && !event.alt_key();

            // Handle Cmd/Ctrl shortcuts
            if is_cmd && state.borrow().focused_id.is_some() {
                match key.to_lowercase().as_str() {
                    "a" => {
                        event.prevent_default();
                        state.borrow_mut().select_all();
                        return;
                    }
                    "c" => {
                        event.prevent_default();
                        if let Some(text) = state.borrow().get_selected_text() {
                            // Copy to clipboard
                            if let Some(window) = web_sys::window() {
                                let clipboard = window.navigator().clipboard();
                                let _ = clipboard.write_text(&text);
                                log::info!("Copied: {}", text);
                            }
                        }
                        return;
                    }
                    "x" => {
                        event.prevent_default();
                        if let Some(text) = state.borrow_mut().cut_selected() {
                            // Copy to clipboard
                            if let Some(window) = web_sys::window() {
                                let clipboard = window.navigator().clipboard();
                                let _ = clipboard.write_text(&text);
                                log::info!("Cut: {}", text);
                            }
                        }
                        return;
                    }
                    "v" => {
                        event.prevent_default();
                        // Paste from clipboard
                        if let Some(window) = web_sys::window() {
                            let clipboard = window.navigator().clipboard();
                            let state_clone = state.clone();
                            let promise = clipboard.read_text();
                            let future = wasm_bindgen_futures::JsFuture::from(promise);
                            wasm_bindgen_futures::spawn_local(async move {
                                if let Ok(text) = future.await {
                                    if let Some(s) = text.as_string() {
                                        state_clone.borrow_mut().paste(&s);
                                        log::info!("Pasted: {}", s);
                                    }
                                }
                            });
                        }
                        return;
                    }
                    _ => {}
                }
            }

            // Prevent default for handled keys when input is focused
            if state.borrow().focused_id.is_some() {
                if is_printable || key == "Backspace" || key == "Delete"
                    || key == "ArrowLeft" || key == "ArrowRight"
                    || key == "Home" || key == "End" {
                    event.prevent_default();
                }
            }

            state.borrow_mut().on_key(&key, is_printable, is_cmd);
        });
        window
            .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
            .ok();
        closure.forget();
    }

    // Set cursor style on canvas for buttons
    canvas.style().set_property("cursor", "default").ok();

    // Make canvas focusable for keyboard events
    canvas.set_tab_index(0);

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
        if let Err(e) = state.borrow_mut().render() {
            log::error!("Render error: {:?}", e);
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());

    log::info!("OpenAgents Web Demo running!");
    Ok(())
}
