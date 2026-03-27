use std::sync::Arc;
use std::time::Instant;
use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result};
use nostr::load_or_create_identity;
use wgpu::util::DeviceExt;
use wgpui::components::Text;
use wgpui::components::hud::{Command, CommandPalette};
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, Hsla, PaintContext, Point, Quad, Scene, Size, SvgQuad, TextSystem, theme,
};
use winit::event_loop::ActiveEventLoop;
#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::window::Window;

use crate::app_state::{
    PaneKind, ProviderMode, RenderState, SidebarState, WINDOW_HEIGHT, WINDOW_TITLE, WINDOW_WIDTH,
};
use crate::apple_fm_bridge::{AppleFmBridgeSnapshot, AppleFmBridgeWorker};
use crate::bitcoin_display::{format_btc_amount_from_sats, format_sats_amount};
use crate::codex_lane::{CodexLaneConfig, CodexLaneSnapshot, CodexLaneWorker};
use crate::hotbar::{configure_hotbar, hotbar_bounds, new_hotbar};
use crate::input::{bootstrap_startup_cad_mesh, ensure_mission_control_local_runtime_preflight};
use crate::local_inference_runtime::{
    LocalInferenceRuntimeCommand, default_local_inference_runtime,
    initial_local_inference_runtime_snapshot,
};
use crate::nip_sa_wallet_bridge::spark_total_balance_sats;
use crate::pane_registry::{
    PaneSearchFilter, enabled_pane_specs, pane_search_tier, startup_pane_kinds,
};
use crate::pane_renderer::PaneRenderer;
use crate::pane_system::{
    PANE_MIN_HEIGHT, PANE_MIN_WIDTH, PaneController, RIGHT_SIDEBAR_ENABLED,
    cad_palette_command_specs, clamp_all_panes_to_window, mission_control_docked_visible,
    sidebar_reserved_width,
};
use crate::provider_nip90_lane::{ProviderNip90LaneSnapshot, ProviderNip90LaneWorker};
use crate::runtime_lanes::{
    AcCreditCommand, AcLaneSnapshot, AcLaneWorker, SaLaneSnapshot, SaLaneWorker,
    SaLifecycleCommand, SklLaneSnapshot, SklLaneWorker,
};
use crate::spark_wallet::SparkWalletCommand;
use crate::voice_playground::VoicePlaygroundWorker;

const WALLET_BALANCE_CHIP_MARGIN: f32 = 12.0;
const WALLET_BALANCE_CHIP_HEIGHT: f32 = 48.0;
const WALLET_BALANCE_CHIP_MIN_WIDTH: f32 = 140.0;
const WALLET_BALANCE_CHIP_MAX_WIDTH: f32 = 220.0;
const OPENAGENTS_BRAND_ICON_SIZE: f32 = 32.0;
const SIDEBAR_HANDLE_ICON_SIZE: f32 = 16.0;
const SIDEBAR_HANDLE_ICON_TOP_PAD: f32 = 12.0;
const SIDEBAR_HANDLE_ICON_LEFT_INSET: f32 = 2.0;
const SIDEBAR_COLLAPSED_RAIL_WIDTH: f32 = 28.0;
const LOCAL_SIM_RUNTIME_BOOTSTRAP_ENV: &str = "OPENAGENTS_ENABLE_LOCAL_SIMULATION_LANES";

fn app_glass_overlay_color() -> Hsla {
    Hsla::from_hex(0x08111A)
}

fn app_glass_sidebar_color() -> Hsla {
    Hsla::from_hex(0x333333)
}

fn preferred_surface_alpha_mode(
    alpha_modes: &[wgpu::CompositeAlphaMode],
) -> wgpu::CompositeAlphaMode {
    #[cfg(target_os = "macos")]
    {
        for preferred in [
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
            wgpu::CompositeAlphaMode::Auto,
        ] {
            if alpha_modes.contains(&preferred) {
                return preferred;
            }
        }
    }

    alpha_modes
        .first()
        .copied()
        .unwrap_or(wgpu::CompositeAlphaMode::Auto)
}
pub(crate) const COMMAND_PALETTE_PANE_FILTER_CYCLE_ACTION: &str = "pane.search_filter.cycle";
const BACKDROP_BLUR_SHADER: &str = r#"
struct BlurUniforms {
    texel_size: vec2<f32>,
    direction: vec2<f32>,
};

@group(0) @binding(0)
var input_texture: texture_2d<f32>;
@group(0) @binding(1)
var input_sampler: sampler;
@group(0) @binding(2)
var<uniform> blur_uniforms: BlurUniforms;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );

    var out: VertexOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let delta = blur_uniforms.texel_size * blur_uniforms.direction * 3.5;
    var color = textureSample(input_texture, input_sampler, in.uv) * 0.14;
    color += textureSample(input_texture, input_sampler, in.uv + delta * 1.0) * 0.14;
    color += textureSample(input_texture, input_sampler, in.uv - delta * 1.0) * 0.14;
    color += textureSample(input_texture, input_sampler, in.uv + delta * 2.0) * 0.12;
    color += textureSample(input_texture, input_sampler, in.uv - delta * 2.0) * 0.12;
    color += textureSample(input_texture, input_sampler, in.uv + delta * 3.0) * 0.095;
    color += textureSample(input_texture, input_sampler, in.uv - delta * 3.0) * 0.095;
    color += textureSample(input_texture, input_sampler, in.uv + delta * 4.0) * 0.075;
    color += textureSample(input_texture, input_sampler, in.uv - delta * 4.0) * 0.075;
    return color;
}
"#;

pub struct BackdropBlurRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    scene_texture: Option<wgpu::Texture>,
    scene_view: Option<wgpu::TextureView>,
    blur_texture: Option<wgpu::Texture>,
    blur_view: Option<wgpu::TextureView>,
    target_size: (u32, u32),
    target_format: wgpu::TextureFormat,
}

impl BackdropBlurRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Backdrop Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(BACKDROP_BLUR_SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Backdrop Blur Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Backdrop Blur Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Backdrop Blur Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Backdrop Blur Uniform Buffer"),
            contents: &blur_uniform_bytes([1.0, 1.0], [1.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Backdrop Blur Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buffer,
            scene_texture: None,
            scene_view: None,
            blur_texture: None,
            blur_view: None,
            target_size: (0, 0),
            target_format,
        }
    }

    pub fn ensure_size(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        target_format: wgpu::TextureFormat,
    ) {
        let width = width.max(1);
        let height = height.max(1);
        if self.target_size == (width, height)
            && self.target_format == target_format
            && self.scene_view.is_some()
            && self.blur_view.is_some()
        {
            return;
        }
        self.target_size = (width, height);
        self.target_format = target_format;
        let (scene_texture, scene_view) = create_blur_target_texture(
            device,
            "Backdrop Blur Scene Texture",
            width,
            height,
            target_format,
        );
        let (blur_texture, blur_view) = create_blur_target_texture(
            device,
            "Backdrop Blur Intermediate Texture",
            width,
            height,
            target_format,
        );
        self.scene_texture = Some(scene_texture);
        self.scene_view = Some(scene_view);
        self.blur_texture = Some(blur_texture);
        self.blur_view = Some(blur_view);
    }

    pub fn scene_view(&self) -> Option<&wgpu::TextureView> {
        self.scene_view.as_ref()
    }

    pub fn render_blurred(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
    ) {
        let Some(scene_view) = self.scene_view.as_ref() else {
            return;
        };
        let Some(blur_view) = self.blur_view.as_ref() else {
            return;
        };
        let texel_size = [
            1.0 / self.target_size.0.max(1) as f32,
            1.0 / self.target_size.1.max(1) as f32,
        ];

        self.run_blur_pass(
            device,
            queue,
            encoder,
            scene_view,
            blur_view,
            texel_size,
            [1.0, 0.0],
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        self.run_blur_pass(
            device,
            queue,
            encoder,
            blur_view,
            scene_view,
            texel_size,
            [0.0, 1.0],
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        self.run_blur_pass(
            device,
            queue,
            encoder,
            scene_view,
            blur_view,
            texel_size,
            [1.0, 0.0],
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        self.run_blur_pass(
            device,
            queue,
            encoder,
            blur_view,
            output_view,
            texel_size,
            [0.0, 1.0],
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        );
    }

    fn run_blur_pass(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        target_view: &wgpu::TextureView,
        texel_size: [f32; 2],
        direction: [f32; 2],
        load_op: wgpu::LoadOp<wgpu::Color>,
    ) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            &blur_uniform_bytes(texel_size, direction),
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Backdrop Blur Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        });
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Backdrop Blur Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: load_op,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

fn create_blur_target_texture(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn blur_uniform_bytes(texel_size: [f32; 2], direction: [f32; 2]) -> [u8; 16] {
    let values = [texel_size[0], texel_size[1], direction[0], direction[1]];
    let mut bytes = [0_u8; 16];
    for (index, value) in values.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

const SETTINGS_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M249.9 176.3C243.4 179.5 237.1 183.1 231.1 187.2C222.9 192.7 212.6 194.1 203.1 191L131.4 167.2L93.5 232.8L150 283.1C157.4 289.7 161.3 299.3 160.7 309.2C160.2 316.4 160.2 323.8 160.7 331C161.4 340.9 157.4 350.5 150 357.1L93.5 407.3L131.4 473L203.1 449.2C212.5 446.1 222.8 447.5 231.1 453C237.1 457 243.4 460.7 249.9 463.9C258.8 468.3 265.1 476.5 267.1 486.2L282.3 560.2L358.1 560.2L373.3 486.2C375.3 476.5 381.7 468.3 390.5 463.9C397 460.7 403.3 457.1 409.3 453C417.5 447.5 427.8 446.1 437.3 449.2L509 473L546.9 407.3L490.4 357.1C483 350.5 479.1 340.9 479.7 331C479.9 327.4 480.1 323.8 480.1 320.1C480.1 316.4 480 312.8 479.7 309.2C479 299.3 483 289.7 490.4 283.1L546.9 232.9L509 167.2L437.3 191C427.9 194.1 417.6 192.7 409.3 187.2C403.3 183.2 397 179.5 390.5 176.3C381.6 171.9 375.3 163.7 373.3 154L358.1 80L282.3 80L267.1 154C265.1 163.7 258.7 171.9 249.9 176.3zM358.2 48C373.4 48 386.5 58.7 389.5 73.5L404.7 147.5C412.5 151.3 420.1 155.7 427.3 160.6L499 136.8C513.4 132 529.2 138 536.8 151.2L574.7 216.9C582.3 230.1 579.6 246.7 568.2 256.8L511.9 307C512.5 315.6 512.5 324.5 511.9 333L568.4 383.2C579.8 393.3 582.4 410 574.9 423.1L537 488.8C529.4 502 513.6 508 499.2 503.2L427.5 479.4C420.3 484.2 412.8 488.6 404.9 492.5L389.7 566.5C386.6 581.4 373.5 592 358.4 592L282.6 592C267.4 592 254.3 581.3 251.3 566.5L236.1 492.5C228.3 488.7 220.7 484.3 213.5 479.4L141.5 503.2C127.1 508 111.3 502 103.7 488.8L65.8 423.2C58.2 410.1 60.9 393.4 72.3 383.3L128.7 333C128.1 324.4 128.1 315.5 128.7 307L72.2 256.8C60.8 246.7 58.2 230 65.7 216.9L103.7 151.2C111.3 138 127.1 132 141.5 136.8L213.2 160.6C220.4 155.8 227.9 151.4 235.8 147.5L251 73.5C254.1 58.7 267.2 48 282.4 48L358.2 48zM264.3 320C264.3 350.8 289.2 375.7 320 375.7C350.8 375.7 375.7 350.8 375.7 320C375.7 289.2 350.8 264.3 320 264.3C289.2 264.3 264.3 289.2 264.3 320zM319.7 408C271.1 407.8 231.8 368.3 232 319.7C232.2 271.1 271.7 231.8 320.3 232C368.9 232.2 408.2 271.7 408 320.3C407.8 368.9 368.3 408.2 319.7 408z"/></svg>"##;
const SIDEBAR_HANDLE_SVG_RAW: &str = r##"<svg id="Layer_1" xmlns="http://www.w3.org/2000/svg" version="1.1" viewBox="0 0 640 640">
  <path fill="#3A4A5E" d="M509.2,22H119.2c-55.5,0-100.7,45.1-100.7,100.7v402.7c0,55.5,45.1,100.7,100.7,100.7h390.1c55.5,0,100.7-45.1,100.7-100.7V122.7c0-55.5-45.1-100.7-100.7-100.7ZM383.4,575.7H119.2c-27.8,0-50.3-22.5-50.3-50.3V122.7c0-27.8,22.5-50.3,50.3-50.3h264.3v503.3ZM559.6,525.3c0,27.8-22.5,50.3-50.3,50.3h-75.5V72.3h75.5c27.8,0,50.3,22.5,50.3,50.3v402.7Z"/>
</svg>"##;
const OPENAGENTS_LOGO_SVG_RAW: &str = r##"<svg version="1.0" id="Layer_1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" x="0px" y="0px"
	 viewBox="0 0 612 792" enable-background="new 0 0 612 792" xml:space="preserve">
<path fill="#3A4A5E" d="M229.4,469.1c-64.6,31.7-124,77.5-174.7,134.9c35.2-78.3,87.9-142.9,146.7-192.5c31-26.1,64-47.7,97.7-68.2l58.8-35.9
	c16.1-9.8,30.8-21.3,45.2-34.2c10.7-9.5,20.2-19.6,28.8-31.6c0.7-1,1.4-1.9,1.8-2.3c0.6-0.7,2.9,0.3,3.3,1.3L569,561.5
	c1,4.2-4.2,7.2-6.8,7.2l-107-0.3c-9.7,0-21.3-7.6-25.8-18.6L377.1,422c-3-2-6.2-2-10.1-1.2C320,430.9,274.1,447.1,229.4,469.1z"/>
<path fill="#3A4A5E" d="M246.4,319.7C182.2,365.3,129,414,81.6,485c-19.5,29.4-36.3,60.7-51.1,94c-6.4,9.4-15.7,14.6-25.9,13.4l48.9-105.3
	l38.2-82.9L148,279.6l83.6-193.7c5.1-12,16.1-21.8,27.3-21.8l88-0.1c12.7,0,22,10.8,28,23.5l42.4,91.3c3.3,7.1-1.7,15.6-4,21.6
	c-17.6,26.7-47.8,47.6-72.6,62.9l-26.5-81.9c-1.7-5-6.4-7.5-10-7.5c-4,0-8.2,2.5-10,7.5L246.4,319.7z"/>
</svg>"##;

fn read_system_clipboard() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        read_clipboard_with_command("pbpaste", &[])
            .or_else(|| read_clipboard_with_command("/usr/bin/pbpaste", &[]))
    }

    #[cfg(target_os = "linux")]
    {
        read_clipboard_with_command("wl-paste", &["-n"])
            .or_else(|| read_clipboard_with_command("xclip", &["-selection", "clipboard", "-o"]))
            .or_else(|| read_clipboard_with_command("xsel", &["--clipboard", "--output"]))
    }

    #[cfg(target_os = "windows")]
    {
        read_clipboard_with_command("powershell", &["-NoProfile", "-Command", "Get-Clipboard"])
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn read_clipboard_with_command(cmd: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(cmd).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    match String::from_utf8(output.stdout) {
        Ok(text) => Some(text),
        Err(error) => Some(String::from_utf8_lossy(error.as_bytes()).to_string()),
    }
}

fn configure_event_context_clipboard(event_context: &mut wgpui::EventContext) {
    event_context.set_clipboard(read_system_clipboard, |text| {
        let _ = wgpui::clipboard::copy_to_clipboard(text);
    });
}

pub fn init_state(
    event_loop: &ActiveEventLoop,
    window_visible: bool,
    disable_codex: bool,
) -> Result<RenderState> {
    let window_attrs = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
        .with_transparent(cfg!(target_os = "macos"))
        .with_blur(cfg!(target_os = "macos"))
        .with_visible(window_visible);
    #[cfg(target_os = "macos")]
    let window_attrs = window_attrs
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true);

    let window = Arc::new(
        event_loop
            .create_window(window_attrs)
            .context("failed to create window")?,
    );
    #[cfg(target_os = "macos")]
    window.set_blur(true);

    pollster::block_on(async move {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .context("failed to create surface")?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("failed to find compatible adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .context("failed to create device")?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|format| format.is_srgb())
            .copied()
            .or_else(|| surface_caps.formats.first().copied())
            .context("surface formats empty")?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: preferred_surface_alpha_mode(&surface_caps.alpha_modes),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let renderer = Renderer::new(&device, surface_format);
        let backdrop_blur = BackdropBlurRenderer::new(&device, surface_format);
        let scale_factor = window.scale_factor() as f32;
        let text_system = TextSystem::new(scale_factor);

        let hotbar = new_hotbar();
        let initial_hotbar_bounds = hotbar_bounds(logical_size(&config, scale_factor));

        let (nostr_identity, nostr_identity_error) = match load_or_create_identity() {
            Ok(identity) => (Some(identity), None),
            Err(err) => (None, Some(err.to_string())),
        };
        let spacetime_presence =
            crate::spacetime_presence::SpacetimePresenceRuntime::new(nostr_identity.as_ref());
        let spacetime_presence_snapshot = spacetime_presence.snapshot();

        let spark_wallet = crate::spark_wallet::SparkPaneState::default();
        let spark_worker = crate::spark_wallet::SparkWalletWorker::spawn(spark_wallet.network);
        let stable_sats_blink_worker =
            crate::stablesats_blink_worker::StableSatsBlinkWorker::spawn();
        let settings = crate::app_state::SettingsState::load_from_disk();
        let settings_inputs = crate::app_state::SettingsPaneInputs::from_state(&settings);
        let initial_relay_urls = settings.document.configured_relay_urls();
        let credentials = crate::app_state::CredentialsState::load_from_disk();
        let credentials_inputs = crate::app_state::CredentialsPaneInputs::from_state(&credentials);
        let autopilot_goals = crate::state::autopilot_goals::AutopilotGoalsState::load_from_disk();
        let mut codex_lane_config = CodexLaneConfig::default();
        codex_lane_config.connect_on_startup = false;
        codex_lane_config.bootstrap_thread = false;
        let codex_lane_worker = CodexLaneWorker::spawn(codex_lane_config.clone());
        let sa_lane_worker = SaLaneWorker::spawn();
        let skl_lane_worker = SklLaneWorker::spawn();
        let ac_lane_worker = AcLaneWorker::spawn();
        let provider_nip90_lane_worker = ProviderNip90LaneWorker::spawn(initial_relay_urls.clone());
        let nip28_chat_lane_worker = {
            let mut cfg = crate::app_state::DefaultNip28ChannelConfig::from_env_or_default();
            cfg.private_key_hex = nostr_identity.as_ref().map(|id| id.private_key_hex.clone());
            crate::nip28_chat_lane::Nip28ChatLaneWorker::spawn_with_config(cfg)
        };
        let apple_fm_execution_worker = AppleFmBridgeWorker::spawn();
        let voice_playground_worker = VoicePlaygroundWorker::new();
        let chat_terminal_worker = crate::chat_terminal::ChatTerminalWorker::spawn();
        let local_inference_runtime = default_local_inference_runtime().map_err(|error| {
            anyhow::anyhow!("failed to start default local inference runtime: {error}")
        })?;
        let (provider_admin_runtime, provider_admin_listen_addr, provider_admin_last_error) =
            match crate::provider_admin::spawn_runtime() {
                Ok(runtime) => {
                    let listen_addr = runtime.listen_addr().to_string();
                    (Some(runtime), Some(listen_addr), None)
                }
                Err(error) => (None, None, Some(error)),
            };
        let sync_apply_engine = match crate::sync_apply::SyncApplyEngine::load_or_new_default() {
            Ok(engine) => engine,
            Err(error) => {
                tracing::warn!("sync apply checkpoint load failed: {}", error);
                crate::sync_apply::SyncApplyEngine::load_or_new(
                    std::env::temp_dir().join("openagents-sync-checkpoints-fallback.json"),
                    crate::sync_apply::SyncApplyPolicy::default(),
                )
                .map_err(|fallback_error| {
                    anyhow::anyhow!(
                        "failed to initialize fallback sync apply checkpoint engine: {}",
                        fallback_error
                    )
                })?
            }
        };
        let mut sync_health = crate::app_state::SyncHealthState::default();
        sync_health.last_applied_event_seq = sync_apply_engine.max_checkpoint_seq();
        sync_health.cursor_position = sync_health.last_applied_event_seq;
        sync_health.cursor_target_position = sync_health.last_applied_event_seq;
        let command_palette_actions = Rc::new(RefCell::new(Vec::<String>::new()));
        let default_pane_search_filter = PaneSearchFilter::Release;
        let mut command_palette = CommandPalette::new()
            .mono(true)
            .commands(command_registry(default_pane_search_filter))
            .aux_button_label(default_pane_search_filter.button_label());
        {
            let action_queue = Rc::clone(&command_palette_actions);
            command_palette = command_palette.on_select(move |command| {
                action_queue.borrow_mut().push(command.id.clone());
            });
        }
        {
            let action_queue = Rc::clone(&command_palette_actions);
            command_palette = command_palette.on_aux_button(move || {
                action_queue
                    .borrow_mut()
                    .push(COMMAND_PALETTE_PANE_FILTER_CYCLE_ACTION.to_string());
            });
        }

        let mut event_context = wgpui::EventContext::new();
        configure_event_context_clipboard(&mut event_context);

        let mut relay_connections = crate::app_state::RelayConnectionsState::default();
        relay_connections.replace_configured_relays(initial_relay_urls.as_slice());

        let mut state = RenderState {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            backdrop_blur,
            text_system,
            scale_factor,
            desktop_shell_mode: crate::desktop_shell::DesktopShellMode::from_env(),
            buy_mode_enabled: crate::desktop_shell::buy_mode_enabled_from_env(),
            hotbar,
            hotbar_bounds: initial_hotbar_bounds,
            cursor_position: Point::ZERO,
            event_context,
            input_modifiers: wgpui::Modifiers::default(),
            panes: Vec::new(),
            pane_size_memory: crate::app_state::PaneSizeMemory::load_or_default(),
            nostr_identity,
            nostr_identity_error,
            nostr_secret_state: crate::app_state::NostrSecretState::default(),
            nostr_identity_pane: crate::app_state::NostrIdentityPaneState::default(),
            spark_wallet_pane: crate::app_state::SparkWalletPaneState::default(),
            spark_wallet,
            spark_wallet_scroll_offset: 0.0,
            spark_worker,
            stable_sats_blink_worker,
            spark_inputs: crate::app_state::SparkPaneInputs::default(),
            pay_invoice_inputs: crate::app_state::PayInvoicePaneInputs::default(),
            create_invoice_inputs: crate::app_state::CreateInvoicePaneInputs::default(),
            relay_connections_inputs: crate::app_state::RelayConnectionsPaneInputs::default(),
            network_requests_inputs: crate::app_state::NetworkRequestsPaneInputs::default(),
            voice_playground_inputs: crate::app_state::VoicePlaygroundPaneInputs::default(),
            local_inference_inputs: crate::app_state::LocalInferencePaneInputs::default(),
            apple_fm_workbench_inputs: crate::app_state::AppleFmWorkbenchPaneInputs::default(),
            apple_adapter_training_inputs:
                crate::app_state::AppleAdapterTrainingPaneInputs::default(),
            settings_inputs,
            credentials_inputs,
            job_history_inputs: crate::app_state::JobHistoryPaneInputs::default(),
            chat_inputs: crate::app_state::ChatPaneInputs::default(),
            data_seller_inputs: crate::app_state::DataSellerPaneInputs::default(),
            calculator_inputs: crate::app_state::CalculatorPaneInputs::default(),
            provider_control: crate::app_state::ProviderControlPaneState::default(),
            mission_control: crate::app_state::MissionControlPaneState::default(),
            provider_status_pane: crate::app_state::ProviderStatusPaneState::default(),
            tailnet_status_pane: crate::app_state::TailnetStatusPaneState::default(),
            sync_health_pane: crate::app_state::SyncHealthPaneState::default(),
            log_stream: crate::app_state::LogStreamPaneState::default(),
            buy_mode_payments: crate::app_state::BuyModePaymentsPaneState::default(),
            nip90_sent_payments: crate::app_state::Nip90SentPaymentsPaneState::default(),
            data_seller: crate::app_state::DataSellerPaneState::default(),
            data_buyer: crate::app_state::DataBuyerPaneState::default(),
            data_market: crate::app_state::DataMarketPaneState::default(),
            spark_replay: crate::app_state::SparkReplayPaneState::default(),
            autopilot_chat: crate::app_state::AutopilotChatState::default(),
            project_ops: crate::project_ops::ProjectOpsPaneState::default(),
            chat_transcript_selection_drag: None,
            codex_account: crate::app_state::CodexAccountPaneState::default(),
            codex_models: crate::app_state::CodexModelsPaneState::default(),
            codex_config: crate::app_state::CodexConfigPaneState::default(),
            codex_mcp: crate::app_state::CodexMcpPaneState::default(),
            codex_apps: crate::app_state::CodexAppsPaneState::default(),
            codex_labs: crate::app_state::CodexLabsPaneState::default(),
            desktop_control: crate::app_state::DesktopControlState::default(),
            codex_remote: crate::app_state::CodexRemoteState::default(),
            codex_diagnostics: crate::app_state::CodexDiagnosticsPaneState::default(),
            codex_disabled: disable_codex,
            codex_lane: CodexLaneSnapshot::idle(),
            codex_lane_config,
            codex_lane_worker,
            codex_command_responses: Vec::new(),
            codex_notifications: Vec::new(),
            next_codex_command_seq: 1,
            sa_lane: SaLaneSnapshot::default(),
            skl_lane: SklLaneSnapshot::default(),
            ac_lane: AcLaneSnapshot::default(),
            sa_lane_worker,
            skl_lane_worker,
            ac_lane_worker,
            provider_nip90_lane: ProviderNip90LaneSnapshot::with_relays(initial_relay_urls),
            provider_nip90_lane_worker,
            nip28_chat_lane_worker,
            apple_fm_execution: AppleFmBridgeSnapshot::default(),
            apple_fm_execution_worker,
            voice_playground_worker,
            gpt_oss_execution: initial_local_inference_runtime_snapshot(),
            local_inference_runtime,
            runtime_command_responses: Vec::new(),
            next_runtime_command_seq: 1,
            provider_runtime: crate::app_state::ProviderRuntimeState::default(),
            provider_heartbeat_cadence: crate::app_state::ProviderHeartbeatCadenceState::default(),
            background_cadence: crate::app_state::BackgroundCadenceState::default(),
            voice_playground: crate::app_state::VoicePlaygroundPaneState::default(),
            local_inference: crate::app_state::LocalInferencePaneState::default(),
            attnres_lab: crate::app_state::AttnResLabPaneState::default(),
            tassadar_lab: crate::app_state::TassadarLabPaneState::default(),
            rive_preview: crate::app_state::RivePreviewPaneState::default(),
            rive_preview_runtime: crate::app_state::RivePreviewRuntimeState::default(),
            presentation: crate::app_state::PresentationPaneState::default(),
            presentation_runtime: crate::app_state::PresentationRuntimeState::default(),
            provider_control_hud_runtime: crate::app_state::ProviderControlHudRuntimeState::default(
            ),
            frame_debugger: crate::app_state::FrameDebuggerPaneState::default(),
            apple_fm_workbench: crate::app_state::AppleFmWorkbenchPaneState::default(),
            apple_adapter_training: crate::app_state::AppleAdapterTrainingPaneState::default(),
            provider_admin_runtime,
            provider_admin_listen_addr,
            provider_admin_last_error,
            provider_admin_last_sync_signature: None,
            provider_admin_last_sync_at: None,
            desktop_control_runtime: None,
            desktop_control_last_sync_signature: None,
            desktop_control_last_sync_at: None,
            codex_remote_runtime: None,
            codex_remote_last_sync_signature: None,
            codex_remote_last_sync_at: None,
            earnings_scoreboard: crate::app_state::EarningsScoreboardState::default(),
            network_aggregate_counters: crate::app_state::NetworkAggregateCountersState::default(),
            relay_connections,
            sync_health,
            sync_bootstrap_note: None,
            sync_bootstrap_error: None,
            sync_bootstrap_stream_grants: Vec::new(),
            hosted_control_base_url: None,
            hosted_control_bearer_token: None,
            kernel_projection_worker: crate::kernel_control::KernelProjectionWorker::default(),
            sync_apply_engine,
            sync_lifecycle_worker_id: "desktopw:sync".to_string(),
            sync_lifecycle: crate::sync_lifecycle::RuntimeSyncLifecycleManager::default(),
            sync_lifecycle_snapshot: None,
            spacetime_presence,
            spacetime_presence_snapshot,
            network_requests: crate::app_state::NetworkRequestsState::default(),
            starter_jobs: crate::app_state::StarterJobsState::default(),
            reciprocal_loop: crate::app_state::ReciprocalLoopState::default(),
            activity_feed: crate::app_state::ActivityFeedState::default(),
            alerts_recovery: crate::app_state::AlertsRecoveryState::default(),
            settings,
            credentials,
            job_inbox: crate::app_state::JobInboxState::default(),
            active_job: crate::app_state::ActiveJobState::default(),
            job_history: crate::app_state::JobHistoryState::default(),
            earn_job_lifecycle_projection:
                crate::app_state::EarnJobLifecycleProjectionState::default(),
            nip90_buyer_payment_attempts:
                crate::state::nip90_buyer_payment_attempts::Nip90BuyerPaymentAttemptLedgerState::default(),
            nip90_payment_facts:
                crate::state::nip90_payment_facts::Nip90PaymentFactLedgerState::default(),
            earn_kernel_receipts:
                crate::state::earn_kernel_receipts::EarnKernelReceiptState::default(),
            economy_snapshot: crate::state::economy_snapshot::EconomySnapshotState::default(),
            agent_profile_state: crate::app_state::AgentProfileStatePaneState::default(),
            agent_schedule_tick: crate::app_state::AgentScheduleTickPaneState::default(),
            trajectory_audit: crate::app_state::TrajectoryAuditPaneState::default(),
            cast_control: crate::app_state::CastControlPaneState::default(),
            cast_control_process: None,
            skill_registry: crate::app_state::SkillRegistryPaneState::default(),
            skill_trust_revocation: crate::app_state::SkillTrustRevocationPaneState::default(),
            credit_desk: crate::app_state::CreditDeskPaneState::default(),
            credit_settlement_ledger: crate::app_state::CreditSettlementLedgerPaneState::default(),
            chat_terminal_worker,
            cad_demo: crate::app_state::CadDemoPaneState::default(),
            stable_sats_simulation: crate::app_state::StableSatsSimulationPaneState::default(),
            autopilot_goals,
            goal_loop_executor: crate::state::goal_loop_executor::GoalLoopExecutorState::default(),
            goal_restart_recovery_ran: false,
            sidebar: SidebarState::default(),
            next_pane_id: 1,
            next_z_index: 1,
            pane_drag_mode: None,
            cad_camera_drag_state: None,
            pane_resizer: wgpui::components::hud::ResizablePane::new()
                .min_size(PANE_MIN_WIDTH, PANE_MIN_HEIGHT),
            hotbar_flash_was_active: false,
            onboarding: crate::onboarding::OnboardingState::load_or_default(),
            command_palette,
            command_palette_actions,
            pane_search_filter: default_pane_search_filter,
        };
        rehydrate_startup_earnings_history(&mut state);
        apply_spacetime_sync_bootstrap(&mut state);
        bootstrap_runtime_lanes(&mut state);
        state.sync_chat_identities();
        let _ = state.sync_provider_nip90_lane_identity();
        let _ = state.sync_provider_nip90_lane_relays();
        let _ = state.queue_local_inference_runtime_command(LocalInferenceRuntimeCommand::Refresh);
        if let Ok(bind_addr) = std::env::var(crate::desktop_control::DESKTOP_CONTROL_BIND_ENV) {
            let trimmed = bind_addr.trim();
            if !trimmed.is_empty() {
                state.desktop_control.requested_bind_addr = trimmed.to_string();
            }
        }
        open_startup_panes(&mut state);
        let _ = crate::desktop_control::enable_runtime(&mut state, None);
        Ok(state)
    })
}

fn rehydrate_startup_earnings_history(state: &mut RenderState) {
    let reference_epoch_seconds = crate::app_state::current_reference_epoch_seconds();
    let source_error = state.earn_kernel_receipts.last_error.as_deref();
    let rows = if source_error.is_none() {
        state.earn_kernel_receipts.authoritative_job_history_rows()
    } else {
        Vec::new()
    };
    state.job_history.replace_rows_from_persisted_receipts(
        rows,
        reference_epoch_seconds,
        source_error,
    );
}

pub(crate) fn sync_project_ops_runtime_contract_state(state: &mut RenderState) {
    state.project_ops.sync_runtime_contract_state(
        state.sync_bootstrap_note.as_deref(),
        state.sync_bootstrap_error.as_deref(),
        state.sync_bootstrap_stream_grants.as_slice(),
        state.sync_lifecycle_snapshot.as_ref(),
    );
}

pub(crate) fn apply_spacetime_sync_bootstrap(state: &mut RenderState) {
    state.sync_bootstrap_note = None;
    state.sync_bootstrap_error = None;
    state.sync_bootstrap_stream_grants.clear();
    state.hosted_control_base_url = None;
    state.hosted_control_bearer_token = None;
    state.spacetime_presence.clear_live_client();
    let worker_id = state.sync_lifecycle_worker_id.clone();
    state.sync_lifecycle.mark_idle(worker_id.as_str());
    state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
    sync_project_ops_runtime_contract_state(state);

    let client = match reqwest::blocking::Client::builder().build() {
        Ok(value) => value,
        Err(error) => {
            let message = format!("Sync token client initialization failed: {error}");
            state.sync_bootstrap_error = Some(message.clone());
            let reason = crate::sync_lifecycle::classify_disconnect_reason(message.as_str());
            let _ = state.sync_lifecycle.mark_disconnect(
                worker_id.as_str(),
                reason,
                Some(message.clone()),
            );
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            state.sync_health.refresh_from_lifecycle(
                std::time::Instant::now(),
                state.sync_lifecycle_snapshot.as_ref(),
            );
            state.sync_health.last_action = Some("Spacetime bootstrap failed".to_string());
            state.sync_health.last_error = Some(message);
            sync_project_ops_runtime_contract_state(state);
            crate::kernel_control::sync_kernel_authority_mode(state);
            return;
        }
    };
    let bound_nostr_pubkey = state
        .nostr_identity
        .as_ref()
        .map(|identity| identity.npub.as_str());

    if let Ok(control_base_url) = crate::sync_bootstrap::resolve_control_base_url_from_env() {
        state.hosted_control_base_url = Some(control_base_url.clone());
        match crate::sync_bootstrap::resolve_control_bearer_auth_from_env(
            &client,
            control_base_url.as_str(),
            bound_nostr_pubkey,
        ) {
            Ok(Some(token)) => {
                state.hosted_control_bearer_token = Some(token);
            }
            Ok(None) => {}
            Err(error) => {
                state.sync_bootstrap_error = Some(error.clone());
            }
        }
    }

    match crate::sync_bootstrap::bootstrap_sync_session_from_env(&client, bound_nostr_pubkey) {
        Ok(Some(result)) => {
            state.sync_bootstrap_stream_grants = result.token_lease.stream_grants.clone();
            let mut note = format!(
                "Minted sync token via {} and prepared subscribe target {}",
                result.control_token_endpoint, result.target.subscribe_url
            );
            match autopilot_spacetime::live::LiveSpacetimeClient::new(
                result.target.base_url.as_str(),
                result.target.database.as_str(),
                None,
            ) {
                Ok(live_client) => {
                    state
                        .spacetime_presence
                        .configure_live_client(live_client.clone());
                    match hydrate_remote_sync_checkpoints(state, &live_client) {
                        Ok(adopted) if adopted > 0 => {
                            note.push_str(
                                format!(" and hydrated {adopted} remote checkpoints").as_str(),
                            );
                        }
                        Ok(_) => {}
                        Err(error) => {
                            state.sync_bootstrap_error = Some(error.clone());
                        }
                    }
                }
                Err(error) => {
                    state.sync_bootstrap_error = Some(error.clone());
                }
            }
            state.sync_bootstrap_note = Some(note.clone());
            state.sync_lifecycle.mark_connecting(worker_id.as_str());
            let replay_cursor = state.sync_apply_engine.max_checkpoint_seq();
            state.sync_lifecycle.mark_replay_bootstrap(
                worker_id.as_str(),
                replay_cursor,
                Some(replay_cursor),
            );
            state.sync_lifecycle.mark_live(
                worker_id.as_str(),
                result.token_lease.refresh_after_in_seconds,
            );
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            state.sync_health.refresh_from_lifecycle(
                std::time::Instant::now(),
                state.sync_lifecycle_snapshot.as_ref(),
            );
            state.spacetime_presence_snapshot = state.spacetime_presence.snapshot();
            state.sync_health.last_error = None;
            state.sync_health.last_action = Some(note);
            sync_project_ops_runtime_contract_state(state);
        }
        Ok(None) => {
            let note =
                "Spacetime bootstrap disabled (set OPENAGENTS_ENABLE_SPACETIME_SYNC=1)".to_string();
            state.sync_bootstrap_note = Some(note.clone());
            state.sync_lifecycle.mark_idle(worker_id.as_str());
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            state.sync_health.refresh_from_lifecycle(
                std::time::Instant::now(),
                state.sync_lifecycle_snapshot.as_ref(),
            );
            state.spacetime_presence_snapshot = state.spacetime_presence.snapshot();
            state.sync_health.last_error = None;
            state.sync_health.last_action = Some(note);
            sync_project_ops_runtime_contract_state(state);
        }
        Err(error) => {
            state.sync_bootstrap_error = Some(error.clone());
            let reason = crate::sync_lifecycle::classify_disconnect_reason(error.as_str());
            let _ = state.sync_lifecycle.mark_disconnect(
                worker_id.as_str(),
                reason,
                Some(error.clone()),
            );
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            state.sync_health.refresh_from_lifecycle(
                std::time::Instant::now(),
                state.sync_lifecycle_snapshot.as_ref(),
            );
            state.spacetime_presence_snapshot = state.spacetime_presence.snapshot();
            state.sync_health.last_action = Some("Spacetime bootstrap failed".to_string());
            state.sync_health.last_error = Some(error);
            sync_project_ops_runtime_contract_state(state);
        }
    }
    crate::kernel_control::sync_kernel_authority_mode(state);
}

fn hydrate_remote_sync_checkpoints(
    state: &mut RenderState,
    client: &autopilot_spacetime::live::LiveSpacetimeClient,
) -> Result<usize, String> {
    let checkpoints = client.list_checkpoints(state.sync_lifecycle_worker_id.as_str())?;
    let mut adopted = 0_usize;
    for checkpoint in checkpoints {
        if state
            .sync_apply_engine
            .adopt_checkpoint_if_newer(checkpoint.stream_id.as_str(), checkpoint.last_applied_seq)?
        {
            adopted = adopted.saturating_add(1);
        }
    }
    state.sync_health.last_applied_event_seq = state.sync_apply_engine.max_checkpoint_seq();
    state.sync_health.cursor_position = state.sync_health.last_applied_event_seq;
    state.sync_health.cursor_target_position = state.sync_health.last_applied_event_seq;
    sync_project_ops_runtime_contract_state(state);
    Ok(adopted)
}

fn bootstrap_runtime_lanes(state: &mut RenderState) {
    if !local_sim_runtime_bootstrap_enabled() {
        return;
    }

    let _ = state.queue_sa_command(SaLifecycleCommand::PublishAgentProfile {
        display_name: "Autopilot".to_string(),
        about: "Desktop sovereign agent runtime".to_string(),
        version: "mvp".to_string(),
    });
    let _ = state.queue_sa_command(SaLifecycleCommand::PublishAgentState {
        encrypted_state_ref: "nip44:ciphertext:bootstrap".to_string(),
    });
    let _ = state.queue_sa_command(SaLifecycleCommand::ConfigureAgentSchedule {
        heartbeat_seconds: 30,
    });

    let _ = state.queue_ac_command(AcCreditCommand::PublishCreditIntent {
        scope: "bootstrap:credit".to_string(),
        request_type: "bootstrap.credit".to_string(),
        payload: "{\"bootstrap\":true}".to_string(),
        skill_scope_id: Some("33400:npub1agent:summarize-text:0.1.0".to_string()),
        credit_envelope_ref: None,
        requested_sats: 1500,
        timeout_seconds: 60,
    });
}

fn local_sim_runtime_bootstrap_enabled() -> bool {
    std::env::var(LOCAL_SIM_RUNTIME_BOOTSTRAP_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn queue_startup_wallet_refresh(state: &mut RenderState) {
    state
        .spark_wallet
        .begin_startup_convergence(crate::app_state::current_reference_epoch_seconds());
    if let Err(error) = state.spark_worker.enqueue(SparkWalletCommand::Refresh) {
        state.spark_wallet.last_error = Some(error);
    }
}

fn open_startup_pane(state: &mut RenderState, pane_kind: PaneKind) {
    let _ = PaneController::create_for_kind(state, pane_kind);
    match pane_kind {
        PaneKind::GoOnline => {
            let _ = ensure_mission_control_local_runtime_preflight(state);
            queue_startup_wallet_refresh(state);
        }
        PaneKind::ProviderControl => {
            let _ = ensure_mission_control_local_runtime_preflight(state);
            queue_startup_wallet_refresh(state);
        }
        PaneKind::CadDemo => bootstrap_startup_cad_mesh(state),
        PaneKind::SparkWallet => {
            queue_startup_wallet_refresh(state);
        }
        _ => {}
    }
}

fn layout_split_shell_startup_panes(state: &mut RenderState) {
    if mission_control_docked_visible(state) {
        if let Some(chat_idx) = state
            .panes
            .iter()
            .position(|pane| pane.kind == PaneKind::AutopilotChat)
        {
            let logical = logical_size(&state.config, state.scale_factor);
            let usable_width = (logical.width - sidebar_reserved_width(state)).max(0.0);
            let chat_bounds = state.panes[chat_idx].bounds;
            state.panes[chat_idx].bounds = Bounds::new(
                12.0,
                56.0,
                chat_bounds
                    .size
                    .width
                    .min((usable_width - 24.0).max(PANE_MIN_WIDTH)),
                chat_bounds
                    .size
                    .height
                    .min((logical.height - 96.0).max(PANE_MIN_HEIGHT)),
            );
            clamp_all_panes_to_window(state);
        }
        return;
    }

    let chat_idx = state
        .panes
        .iter()
        .position(|pane| pane.kind == PaneKind::AutopilotChat);
    let mission_idx = state
        .panes
        .iter()
        .position(|pane| pane.kind == PaneKind::GoOnline);
    if let (Some(chat_idx), Some(mission_idx)) = (chat_idx, mission_idx) {
        let logical = logical_size(&state.config, state.scale_factor);
        let usable_width = (logical.width - sidebar_reserved_width(state)).max(0.0);
        let usable_height = logical.height.max(0.0);
        let margin = 12.0;
        let gap = 10.0;
        let top = 12.0;
        let bottom_margin = 12.0;
        let available_height = (usable_height - top - bottom_margin).max(300.0);
        let available_width = (usable_width - margin * 2.0 - gap).max(900.0);

        let chat_min_width = 620.0;
        let mission_min_width = 520.0;
        let mission_pref_width = state.panes[mission_idx].bounds.size.width;
        let mission_width = mission_pref_width
            .clamp(mission_min_width, 760.0)
            .min((available_width - chat_min_width).max(mission_min_width));
        let chat_width = (available_width - mission_width).max(chat_min_width);

        let chat_height = state.panes[chat_idx]
            .bounds
            .size
            .height
            .min(available_height);
        let mission_height = state.panes[mission_idx]
            .bounds
            .size
            .height
            .min(available_height);

        state.panes[chat_idx].bounds = Bounds::new(margin, top, chat_width, chat_height);
        state.panes[mission_idx].bounds = Bounds::new(
            margin + chat_width + gap,
            top,
            mission_width,
            mission_height,
        );
        clamp_all_panes_to_window(state);
        return;
    }

    let Some(provider_idx) = state
        .panes
        .iter()
        .position(|pane| pane.kind == PaneKind::ProviderControl)
    else {
        return;
    };
    let Some(earnings_idx) = state
        .panes
        .iter()
        .position(|pane| pane.kind == PaneKind::EarningsScoreboard)
    else {
        return;
    };

    let logical = logical_size(&state.config, state.scale_factor);
    let usable_width = (logical.width - sidebar_reserved_width(state)).max(0.0);
    let provider_size = state.panes[provider_idx].bounds.size;
    let earnings_size = state.panes[earnings_idx].bounds.size;

    state.panes[provider_idx].bounds =
        Bounds::new(12.0, 12.0, provider_size.width, provider_size.height);
    state.panes[earnings_idx].bounds = Bounds::new(
        (usable_width - earnings_size.width - 12.0).max(24.0),
        76.0,
        earnings_size.width,
        earnings_size.height,
    );
    clamp_all_panes_to_window(state);
}

fn open_startup_panes(state: &mut RenderState) {
    let startup_panes = startup_pane_kinds();
    for pane_kind in [
        PaneKind::AutopilotChat,
        PaneKind::GoOnline,
        PaneKind::EarningsScoreboard,
        PaneKind::ProviderControl,
    ] {
        if startup_panes.contains(&pane_kind) {
            open_startup_pane(state, pane_kind);
        }
    }
    for pane_kind in startup_panes {
        if matches!(
            pane_kind,
            PaneKind::AutopilotChat
                | PaneKind::GoOnline
                | PaneKind::EarningsScoreboard
                | PaneKind::ProviderControl
        ) {
            continue;
        }
        open_startup_pane(state, pane_kind);
    }

    layout_split_shell_startup_panes(state);
    if let Some(mission_id) = state
        .panes
        .iter()
        .find(|pane| pane.kind == PaneKind::GoOnline)
        .map(|pane| pane.id)
    {
        PaneController::bring_to_front(state, mission_id);
    } else if let Some(provider_id) = state
        .panes
        .iter()
        .find(|pane| pane.kind == PaneKind::ProviderControl)
        .map(|pane| pane.id)
    {
        PaneController::bring_to_front(state, provider_id);
    }
}

pub fn render_frame(state: &mut RenderState) -> Result<crate::app_state::FrameRenderReport> {
    let frame_start = Instant::now();
    crate::onboarding::sync_progress(state);
    let onboarding_view = crate::onboarding::build_view(state);
    let onboarding_backdrop_blur_active = matches!(
        onboarding_view.phase,
        crate::onboarding::OnboardingPhase::SetupModal
            | crate::onboarding::OnboardingPhase::TourHotkeys
            | crate::onboarding::OnboardingPhase::TourSellCompute
    );
    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let height = logical.height;
    let active_pane = PaneController::active(state);
    let fullscreen_pane_active = pane_fullscreen_active(state);

    let mut scene = Scene::new();
    #[cfg(target_os = "macos")]
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, height))
            .with_background(app_glass_overlay_color().with_alpha(0.70)),
    );
    #[cfg(not(target_os = "macos"))]
    scene
        .draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Sidebar UI is intentionally disabled for now; keep the underlying code path intact.
    let panel_width = sidebar_reserved_width(state);
    let sidebar_x = (width - panel_width).max(0.0);

    if panel_width > 0.0 {
        #[cfg(target_os = "macos")]
        let sidebar_color = app_glass_sidebar_color().with_alpha(0.92);
        #[cfg(not(target_os = "macos"))]
        let sidebar_color = app_glass_sidebar_color();

        scene.draw_quad(
            Quad::new(Bounds::new(sidebar_x, 0.0, panel_width, height))
                .with_background(sidebar_color),
        );
        scene.draw_quad(
            Quad::new(Bounds::new(sidebar_x, 0.0, 1.0, height))
                .with_background(theme::border::DEFAULT.with_alpha(0.28)),
        );
    }

    if RIGHT_SIDEBAR_ENABLED && panel_width > 0.0 {
        // Settings icon in the bottom-right corner of the sidebar.
        let icon_size = 16.0;
        let padding = 12.0;
        let icon_x = sidebar_x + panel_width - icon_size - padding;
        let icon_y = height - icon_size - padding;
        let icon_bounds = Bounds::new(icon_x, icon_y, icon_size, icon_size);
        let icon_tint = theme::text::MUTED;
        let svg = SvgQuad::new(
            icon_bounds,
            std::sync::Arc::<[u8]>::from(SETTINGS_SVG_RAW.as_bytes()),
        )
        .with_tint(icon_tint);
        scene.draw_svg(svg);
    }

    if RIGHT_SIDEBAR_ENABLED {
        // Top-right panel handle icon for resize/collapse/expand affordance.
        let panel_left_x = if state.sidebar.is_open {
            sidebar_x
        } else {
            (width - SIDEBAR_COLLAPSED_RAIL_WIDTH).max(0.0)
        };
        let handle_bounds = Bounds::new(
            (panel_left_x + SIDEBAR_HANDLE_ICON_LEFT_INSET).max(0.0),
            SIDEBAR_HANDLE_ICON_TOP_PAD,
            SIDEBAR_HANDLE_ICON_SIZE,
            SIDEBAR_HANDLE_ICON_SIZE,
        );
        scene.draw_svg(
            SvgQuad::new(
                handle_bounds,
                std::sync::Arc::<[u8]>::from(SIDEBAR_HANDLE_SVG_RAW.as_bytes()),
            )
            .with_tint(theme::accent::PRIMARY),
        );
    }

    // Animate tooltip: quick fade-in, immediate disappear on mouse-out.
    if RIGHT_SIDEBAR_ENABLED && state.sidebar.settings_hover {
        state.sidebar.settings_tooltip_t = (state.sidebar.settings_tooltip_t + 0.25).min(1.0);
    } else {
        state.sidebar.settings_hover = false;
        state.sidebar.settings_tooltip_t = 0.0;
    }

    let provider_blockers = state.provider_blockers();
    let provider_inventory = crate::provider_inventory::inventory_status_for_state(state);
    let training_status = crate::desktop_control::current_training_status(state);
    let remote_training_status = crate::desktop_control::current_remote_training_status(state);
    let pane_paint_report;
    {
        let buy_mode_enabled = state.mission_control_buy_mode_enabled();
        let mut paint = PaintContext::new(&mut scene, &mut state.text_system, state.scale_factor);

        if RIGHT_SIDEBAR_ENABLED && panel_width > 0.0 {
            let left = sidebar_x + 12.0;
            let right = sidebar_x + panel_width - 12.0;
            let mut y = 16.0;

            let providers_online = state.network_aggregate_counters.providers_online;
            let jobs_completed = state.network_aggregate_counters.jobs_completed;
            let sats_paid_network = state.network_aggregate_counters.sats_paid;
            let global_earnings_today_sats =
                state.network_aggregate_counters.global_earnings_today_sats;

            paint.scene.draw_text(paint.text.layout(
                "Autopilot - Mission Control",
                Point::new(left, y),
                13.0,
                theme::text::PRIMARY,
            ));
            y += 18.0;

            paint.scene.draw_text(paint.text.layout_mono(
                &format!(
                    "Global Network Earnings Today: {}",
                    format_btc_amount_from_sats(global_earnings_today_sats)
                ),
                Point::new(left, y),
                10.0,
                if state.network_aggregate_counters.load_state
                    == crate::app_state::PaneLoadState::Ready
                {
                    theme::status::SUCCESS
                } else {
                    theme::text::MUTED
                },
            ));
            y += 18.0;

            let status_label = match state.provider_runtime.mode {
                ProviderMode::Offline => "OFFLINE",
                ProviderMode::Connecting => "CONNECTING",
                ProviderMode::Online => "ONLINE",
                ProviderMode::Degraded => "DEGRADED",
            };
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Status: {status_label}"),
                Point::new(left, y),
                11.0,
                if state.provider_runtime.mode == ProviderMode::Online {
                    theme::status::SUCCESS
                } else {
                    theme::text::MUTED
                },
            ));

            let go_online_bounds = Bounds::new(left, 72.0, (panel_width - 24.0).max(120.0), 34.0);
            let is_online = state.provider_runtime.mode != ProviderMode::Offline;
            let action_label = if is_online { "GO OFFLINE" } else { "GO ONLINE" };
            paint.scene.draw_quad(
                Quad::new(go_online_bounds)
                    .with_background(if is_online {
                        theme::status::ERROR.with_alpha(0.25)
                    } else {
                        theme::status::SUCCESS.with_alpha(0.28)
                    })
                    .with_border(
                        if is_online {
                            theme::status::ERROR.with_alpha(0.75)
                        } else {
                            theme::status::SUCCESS.with_alpha(0.75)
                        },
                        1.0,
                    )
                    .with_corner_radius(8.0),
            );
            let action_width = paint.text.measure(action_label, 11.0);
            paint.scene.draw_text(paint.text.layout_mono(
                action_label,
                Point::new(
                    go_online_bounds.origin.x + (go_online_bounds.size.width - action_width) * 0.5,
                    go_online_bounds.origin.y + 22.0,
                ),
                11.0,
                wgpui::Hsla::white(),
            ));

            y = go_online_bounds.max_y() + 18.0;
            paint.scene.draw_text(paint.text.layout(
                "Network Stats",
                Point::new(left, y),
                11.0,
                theme::text::MUTED,
            ));
            y += 16.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Providers Online: {providers_online}"),
                Point::new(left, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!(
                    "Providers Source: {}",
                    state.network_aggregate_counters.providers_online_source_tag
                ),
                Point::new(left, y),
                9.0,
                theme::text::MUTED,
            ));
            y += 14.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Jobs Completed: {jobs_completed}"),
                Point::new(left, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Paid: {}", format_btc_amount_from_sats(sats_paid_network)),
                Point::new(left, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
            if let Some(snapshot) = state.economy_snapshot.latest_snapshot.as_ref() {
                paint.scene.draw_text(paint.text.layout_mono(
                    &format!("sv: {:.2}% | N: {}", snapshot.sv * 100.0, snapshot.n),
                    Point::new(left, y),
                    10.0,
                    theme::text::PRIMARY,
                ));
                y += 14.0;
                paint.scene.draw_text(paint.text.layout_mono(
                    &format!(
                        "Snapshot: {}",
                        snapshot
                            .snapshot_id
                            .strip_prefix("snapshot.economy:")
                            .unwrap_or(snapshot.snapshot_id.as_str())
                    ),
                    Point::new(left, y),
                    9.0,
                    theme::text::MUTED,
                ));
            } else {
                paint.scene.draw_text(paint.text.layout_mono(
                    "Snapshot: pending",
                    Point::new(left, y),
                    9.0,
                    theme::text::MUTED,
                ));
            }

            y += 20.0;
            paint.scene.draw_text(paint.text.layout(
                "Your Earnings",
                Point::new(left, y),
                11.0,
                theme::text::MUTED,
            ));
            y += 16.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!(
                    "Today: {}",
                    format_sats_amount(state.earnings_scoreboard.sats_today)
                ),
                Point::new(left, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
            paint.scene.draw_text(paint.text.layout_mono(
                &format!(
                    "Total: {}",
                    format_sats_amount(state.earnings_scoreboard.lifetime_sats)
                ),
                Point::new(left, y),
                10.0,
                theme::text::PRIMARY,
            ));

            y += 20.0;
            paint.scene.draw_text(paint.text.layout(
                "Recent Payouts",
                Point::new(left, y),
                11.0,
                theme::text::MUTED,
            ));
            y += 16.0;

            if let Some(active) = state.active_job.job.as_ref() {
                paint.scene.draw_text(paint.text.layout_mono(
                    &format!(
                        "{} | {} | {} | {}",
                        active.job_id,
                        active.capability,
                        format_sats_amount(active.quoted_price_sats),
                        active.stage.label()
                    ),
                    Point::new(left, y),
                    10.0,
                    theme::text::PRIMARY,
                ));
                y += 14.0;
            }

            let recent_rows = state
                .job_history
                .wallet_reconciled_payout_rows(&state.spark_wallet);

            if recent_rows.is_empty() && state.active_job.job.is_none() {
                let waiting = if state.provider_runtime.mode == ProviderMode::Online {
                    "Waiting for first job..."
                } else {
                    "(empty)"
                };
                paint.scene.draw_text(paint.text.layout(
                    waiting,
                    Point::new(left, y),
                    10.0,
                    theme::text::MUTED,
                ));
                y += 14.0;
            } else {
                for row in recent_rows.into_iter().take(2) {
                    paint.scene.draw_text(paint.text.layout_mono(
                        &format!(
                            "{} | settled | {}",
                            row.job_id,
                            format_sats_amount(row.payout_sats)
                        ),
                        Point::new(left, y),
                        10.0,
                        theme::text::PRIMARY,
                    ));
                    y += 14.0;
                }
            }

            y += 10.0;
            let wallet_state = if state.spark_wallet.last_error.is_some() {
                "degraded"
            } else {
                state.spark_wallet.network_status_label()
            };
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Wallet: {wallet_state}"),
                Point::new(left, y),
                10.0,
                if wallet_state == "connected" {
                    theme::status::SUCCESS
                } else {
                    theme::text::MUTED
                },
            ));
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("Lane: {}", state.sync_health.subscription_state),
                Point::new(right - 118.0, y),
                10.0,
                theme::text::MUTED,
            ));
        }

        let mission_control_last_action = state.mission_control.last_action.clone();
        let mission_control_last_error = state.mission_control.last_error.clone();
        let spark_wallet_scroll_offset = state.spark_wallet_pane.scroll_offset();
        pane_paint_report = PaneRenderer::paint(
            &mut state.panes,
            Bounds::new(0.0, 0.0, width, height),
            active_pane,
            state.cursor_position,
            state.desktop_shell_mode,
            buy_mode_enabled,
            state.kernel_projection_worker.uses_remote_authority(),
            state.nostr_identity.as_ref(),
            state.nostr_identity_error.as_deref(),
            &state.nostr_secret_state,
            &mut state.nostr_identity_pane,
            &mut state.spark_wallet_pane,
            &state.autopilot_chat,
            &state.project_ops,
            &state.spacetime_presence_snapshot,
            &state.codex_account,
            &state.codex_models,
            &state.codex_config,
            &state.codex_mcp,
            &state.codex_apps,
            &state.codex_labs,
            &state.codex_remote,
            &state.codex_diagnostics,
            &state.sa_lane,
            &state.skl_lane,
            &state.ac_lane,
            &state.provider_runtime,
            &state.gpt_oss_execution,
            &state.apple_fm_execution,
            &state.voice_playground,
            &state.local_inference,
            &state.attnres_lab,
            &state.tassadar_lab,
            &mut state.rive_preview,
            &mut state.rive_preview_runtime,
            &mut state.presentation,
            &mut state.presentation_runtime,
            &mut state.provider_control_hud_runtime,
            &state.frame_debugger,
            &mut state.apple_fm_workbench,
            &mut state.apple_adapter_training,
            &training_status,
            &remote_training_status,
            provider_blockers.as_slice(),
            &mut state.earnings_scoreboard,
            &state.relay_connections,
            &state.sync_health,
            &state.network_requests,
            &state.nip90_buyer_payment_attempts,
            &state.nip90_payment_facts,
            &state.starter_jobs,
            &state.reciprocal_loop,
            &state.activity_feed,
            &state.alerts_recovery,
            &state.settings,
            &state.credentials,
            &state.job_inbox,
            &state.active_job,
            &state.job_history,
            &state.earn_job_lifecycle_projection,
            &state.agent_profile_state,
            &state.agent_schedule_tick,
            &state.trajectory_audit,
            &state.cast_control,
            &state.skill_registry,
            &state.skill_trust_revocation,
            &state.credit_desk,
            &state.credit_settlement_ledger,
            &state.cad_demo,
            &state.spark_wallet,
            spark_wallet_scroll_offset,
            &provider_inventory,
            &mut state.spark_inputs,
            &mut state.pay_invoice_inputs,
            &mut state.create_invoice_inputs,
            &mut state.relay_connections_inputs,
            &mut state.network_requests_inputs,
            &mut state.voice_playground_inputs,
            &mut state.local_inference_inputs,
            &mut state.apple_fm_workbench_inputs,
            &mut state.apple_adapter_training_inputs,
            &mut state.settings_inputs,
            &mut state.credentials_inputs,
            &mut state.job_history_inputs,
            &mut state.chat_inputs,
            &mut state.data_seller_inputs,
            &mut state.calculator_inputs,
            &state.sidebar,
            &mut state.mission_control,
            &mut state.provider_control,
            &mut state.provider_status_pane,
            &mut state.tailnet_status_pane,
            &mut state.sync_health_pane,
            mission_control_last_action.as_deref(),
            mission_control_last_error.as_deref(),
            &mut state.log_stream,
            &mut state.buy_mode_payments,
            &mut state.nip90_sent_payments,
            &state.data_seller,
            &state.data_buyer,
            &state.data_market,
            &mut state.spark_replay,
            &mut paint,
        );
        let hotbar_layer = pane_paint_report.next_layer;
        paint.scene.set_layer(hotbar_layer);

        if fullscreen_pane_active {
            state.hotbar_bounds = Bounds::ZERO;
        } else {
            let wallet_chip_bounds = wallet_balance_chip_bounds_for_logical(logical);
            let wallet_chip_label = state
                .spark_wallet
                .balance
                .as_ref()
                .map(|balance| format_sats_amount(spark_total_balance_sats(balance)))
                .unwrap_or_else(|| "LOADING".to_string());
            let wallet_label_font_size = 11.0;
            let icon_text_gap = 8.0;
            let label_width = paint
                .text
                .measure(&wallet_chip_label, wallet_label_font_size);
            let _group_width = OPENAGENTS_BRAND_ICON_SIZE + icon_text_gap + label_width;
            let group_x = wallet_chip_bounds.origin.x + 6.0;
            let center_y = wallet_chip_bounds.origin.y + wallet_chip_bounds.size.height * 0.5;
            let wallet_icon_bounds = Bounds::new(
                group_x,
                center_y - OPENAGENTS_BRAND_ICON_SIZE * 0.5,
                OPENAGENTS_BRAND_ICON_SIZE,
                OPENAGENTS_BRAND_ICON_SIZE,
            );
            paint.scene.draw_svg(
                SvgQuad::new(
                    wallet_icon_bounds,
                    std::sync::Arc::<[u8]>::from(OPENAGENTS_LOGO_SVG_RAW.as_bytes()),
                )
                .with_tint(theme::accent::PRIMARY),
            );
            paint.scene.draw_text(paint.text.layout_mono(
                &wallet_chip_label,
                Point::new(wallet_icon_bounds.max_x() + icon_text_gap, center_y - 7.0),
                wallet_label_font_size,
                theme::text::PRIMARY,
            ));

            let bar_bounds = hotbar_bounds(logical);
            state.hotbar_bounds = bar_bounds;
            configure_hotbar(&mut state.hotbar);
            state.hotbar.paint(bar_bounds, &mut paint);
        }

        state
            .command_palette
            .paint(Bounds::new(0.0, 0.0, width, height), &mut paint);

        // Sidebar tooltip for the settings icon.
        if RIGHT_SIDEBAR_ENABLED && state.sidebar.settings_tooltip_t > 0.01 && panel_width > 0.0 {
            let _tooltip_alpha = state.sidebar.settings_tooltip_t;
            let icon_size = 16.0;
            let padding = 12.0;
            let icon_x = sidebar_x + panel_width - icon_size - padding;
            let icon_y = height - icon_size - padding;

            let tooltip_text = "Settings";
            let tooltip_font_size = theme::font_size::XS - 1.0;
            let measured_w = paint.text.measure(tooltip_text, tooltip_font_size);
            let tooltip_h_pad = 10.0;
            let tooltip_width = measured_w + tooltip_h_pad * 2.0 + 10.0;
            let tooltip_height = 24.0;
            let caret_size = 6.0;
            let tooltip_margin = 8.0 + caret_size;
            let tooltip_radius = 6.0;
            let icon_center_x = icon_x + icon_size * 0.5;
            let mut tooltip_x = icon_center_x - tooltip_width * 0.5;
            let tooltip_y = icon_y - tooltip_height - tooltip_margin;

            // Clamp so the tooltip stays inside the sidebar panel.
            let sidebar_left = sidebar_x;
            let sidebar_right = sidebar_x + panel_width;
            if tooltip_x < sidebar_left + 4.0 {
                tooltip_x = sidebar_left + 4.0;
            }
            if tooltip_x + tooltip_width > sidebar_right - 4.0 {
                tooltip_x = sidebar_right - tooltip_width - 4.0;
            }
            let tooltip_y = tooltip_y.max(4.0);
            let tooltip_bounds = Bounds::new(tooltip_x, tooltip_y, tooltip_width, tooltip_height);

            let tooltip_bg = theme::bg::MUTED.with_alpha(_tooltip_alpha);
            paint.scene.draw_quad(
                Quad::new(tooltip_bounds)
                    .with_background(tooltip_bg)
                    .with_corner_radius(tooltip_radius),
            );

            // Downward caret at bottom-right of tooltip
            let caret_svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{h}" viewBox="0 0 {s} {h}"><polygon points="0,0 {s},0 {mid},{h}" fill="#FFFFFF"/></svg>"##,
                s = (caret_size * 2.0) as i32,
                h = caret_size as i32,
                mid = caret_size as i32,
            );
            let caret_w = caret_size * 2.0;
            let caret_h = caret_size;
            let caret_x = icon_center_x - caret_w * 0.5;
            let caret_y = tooltip_bounds.origin.y + tooltip_bounds.size.height;
            let caret_bounds = Bounds::new(caret_x, caret_y, caret_w, caret_h);
            paint.scene.draw_svg(SvgQuad {
                bounds: caret_bounds,
                svg_data: std::sync::Arc::from(caret_svg.as_bytes()),
                tint: Some(theme::bg::MUTED.with_alpha(_tooltip_alpha)),
                opacity: 1.0,
            });

            // Horizontally centered text — use full tooltip width to avoid clipping
            let tooltip_text_color = theme::text::PRIMARY.with_alpha(_tooltip_alpha);
            let font_size = tooltip_font_size;
            let text_x = tooltip_bounds.origin.x + 5.0;
            let text_y =
                tooltip_bounds.origin.y + (tooltip_bounds.size.height - font_size) * 0.5 - 13.0;
            let text_bounds = Bounds::new(text_x, text_y, tooltip_bounds.size.width, font_size);
            let mut label = Text::new(tooltip_text)
                .font_size(font_size)
                .color(tooltip_text_color);
            label.paint(text_bounds, &mut paint);
        }
    }

    let overlay_scene = if state.onboarding.is_active() {
        let mut overlay_scene = Scene::new();
        let overlay_buy_mode_enabled = state.mission_control_buy_mode_enabled();
        let overlay_backend_kernel_authority =
            state.kernel_projection_worker.uses_remote_authority();
        let overlay_cursor_position = state.cursor_position;
        let overlay_desktop_shell_mode = state.desktop_shell_mode;
        let mut overlay_paint = PaintContext::new(
            &mut overlay_scene,
            &mut state.text_system,
            state.scale_factor,
        );
        if matches!(
            onboarding_view.phase,
            crate::onboarding::OnboardingPhase::TourSellCompute
        ) {
            crate::pane_renderer::paint_mission_control_sell_compute_focus(
                Bounds::new(0.0, 0.0, width, height),
                overlay_cursor_position,
                overlay_desktop_shell_mode,
                overlay_buy_mode_enabled,
                &state.autopilot_chat,
                state.nostr_identity.as_ref(),
                &mut state.mission_control,
                &state.provider_control,
                &state.provider_runtime,
                &state.gpt_oss_execution,
                &mut state.log_stream,
                &state.buy_mode_payments,
                &state.earn_job_lifecycle_projection,
                &state.sa_lane,
                &state.skl_lane,
                &state.ac_lane,
                overlay_backend_kernel_authority,
                provider_blockers.as_slice(),
                &state.earnings_scoreboard,
                &state.spark_wallet,
                &state.network_requests,
                &state.job_inbox,
                &state.active_job,
                &mut overlay_paint,
            );
        }
        crate::onboarding::paint_overlay(
            &mut state.onboarding,
            &onboarding_view,
            Bounds::new(0.0, 0.0, width, height),
            &mut overlay_paint,
        );
        Some(overlay_scene)
    } else {
        None
    };

    state
        .renderer
        .resize(&state.queue, logical, state.scale_factor.max(0.1));

    if state.text_system.is_dirty() {
        state.renderer.update_atlas(
            &state.queue,
            state.text_system.atlas_data(),
            state.text_system.atlas_size(),
        );
        state.text_system.mark_clean();
    }

    let surface_acquire_start = Instant::now();
    let output = match state.surface.get_current_texture() {
        Ok(frame) => frame,
        Err(wgpu::SurfaceError::Lost) => {
            state.surface.configure(&state.device, &state.config);
            return Ok(crate::app_state::FrameRenderReport::default());
        }
        Err(err) => return Err(anyhow::anyhow!("surface error: {err:?}")),
    };
    let surface_acquire_ms = surface_acquire_start.elapsed().as_secs_f32() * 1_000.0;

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Autopilot Render Encoder"),
        });

    let scene_build_ms = frame_start.elapsed().as_secs_f32() * 1_000.0 - surface_acquire_ms;

    let submit_present_start = Instant::now();
    state.renderer.prepare(
        &state.device,
        &state.queue,
        &scene,
        state.scale_factor.max(0.1),
    );
    if onboarding_backdrop_blur_active {
        state.backdrop_blur.ensure_size(
            &state.device,
            state.config.width,
            state.config.height,
            state.config.format,
        );
        if let Some(scene_view) = state.backdrop_blur.scene_view() {
            state.renderer.render(&mut encoder, scene_view);
            state
                .backdrop_blur
                .render_blurred(&state.device, &state.queue, &mut encoder, &view);
            if let Some(overlay_scene) = overlay_scene.as_ref() {
                state.renderer.prepare(
                    &state.device,
                    &state.queue,
                    overlay_scene,
                    state.scale_factor.max(0.1),
                );
                state.renderer.render_overlay(&mut encoder, &view);
            }
        } else {
            #[cfg(target_os = "macos")]
            state
                .renderer
                .render_with_clear(&mut encoder, &view, wgpu::Color::TRANSPARENT);
            #[cfg(not(target_os = "macos"))]
            state.renderer.render(&mut encoder, &view);
            if let Some(overlay_scene) = overlay_scene.as_ref() {
                state.renderer.prepare(
                    &state.device,
                    &state.queue,
                    overlay_scene,
                    state.scale_factor.max(0.1),
                );
                state.renderer.render_overlay(&mut encoder, &view);
            }
        }
    } else {
        #[cfg(target_os = "macos")]
        state
            .renderer
            .render_with_clear(&mut encoder, &view, wgpu::Color::TRANSPARENT);
        #[cfg(not(target_os = "macos"))]
        state.renderer.render(&mut encoder, &view);
        if let Some(overlay_scene) = overlay_scene.as_ref() {
            state.renderer.prepare(
                &state.device,
                &state.queue,
                overlay_scene,
                state.scale_factor.max(0.1),
            );
            state.renderer.render_overlay(&mut encoder, &view);
        }
    }
    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();
    let submit_present_ms = submit_present_start.elapsed().as_secs_f32() * 1_000.0;
    let metrics = state.renderer.render_metrics();

    Ok(crate::app_state::FrameRenderReport {
        scene_build_ms: scene_build_ms.max(0.0),
        surface_acquire_ms,
        prepare_cpu_ms: metrics.prepare_cpu_ms as f32,
        render_cpu_ms: metrics.render_cpu_ms as f32,
        submit_present_ms,
        total_cpu_ms: frame_start.elapsed().as_secs_f32() * 1_000.0,
        draw_calls: metrics.draw_calls,
        layer_count: metrics.layer_count,
        vector_batches: metrics.vector_batches,
        image_instances: metrics.image_instances,
        svg_instances: metrics.svg_instances,
        svg_cache_size: state.renderer.svg_cache_size(),
        pane_paint_samples: pane_paint_report.pane_paint_samples,
    })
}

pub fn logical_size(config: &wgpu::SurfaceConfiguration, scale_factor: f32) -> Size {
    let scale = scale_factor.max(0.1);
    Size::new(config.width as f32 / scale, config.height as f32 / scale)
}

fn pane_fullscreen_active_for_panes(panes: &[crate::app_state::DesktopPane]) -> bool {
    panes
        .iter()
        .any(|pane| pane.presentation == crate::app_state::PanePresentation::Fullscreen)
}

pub fn pane_fullscreen_active(state: &RenderState) -> bool {
    pane_fullscreen_active_for_panes(&state.panes)
}

pub fn wallet_balance_chip_bounds_for_logical(logical: Size) -> Bounds {
    let available_width = (logical.width - WALLET_BALANCE_CHIP_MARGIN * 2.0).max(0.0);
    let width = available_width
        .min(WALLET_BALANCE_CHIP_MAX_WIDTH)
        .max(WALLET_BALANCE_CHIP_MIN_WIDTH.min(available_width));
    let y = (logical.height - WALLET_BALANCE_CHIP_MARGIN - WALLET_BALANCE_CHIP_HEIGHT).max(0.0);
    Bounds::new(
        WALLET_BALANCE_CHIP_MARGIN,
        y,
        width,
        WALLET_BALANCE_CHIP_HEIGHT,
    )
}

pub fn wallet_balance_chip_bounds(state: &RenderState) -> Bounds {
    if pane_fullscreen_active(state) {
        return Bounds::ZERO;
    }
    wallet_balance_chip_bounds_for_logical(logical_size(&state.config, state.scale_factor))
}

pub fn wallet_balance_sats_label_bounds(state: &RenderState) -> Bounds {
    if pane_fullscreen_active(state) {
        return Bounds::ZERO;
    }
    let logical = logical_size(&state.config, state.scale_factor);
    let wallet_chip_bounds = wallet_balance_chip_bounds_for_logical(logical);
    let wallet_chip_label = state
        .spark_wallet
        .balance
        .as_ref()
        .map(|balance| format_sats_amount(spark_total_balance_sats(balance)))
        .unwrap_or_else(|| "LOADING".to_string());
    let wallet_label_font_size = 11.0;
    let icon_text_gap = 8.0;
    let group_x = wallet_chip_bounds.origin.x + 6.0;
    let center_y = wallet_chip_bounds.origin.y + wallet_chip_bounds.size.height * 0.5;
    let label_x = group_x + OPENAGENTS_BRAND_ICON_SIZE + icon_text_gap;
    let label_y = center_y - wallet_label_font_size * 0.5;
    let label_width = wallet_chip_label.len() as f32 * wallet_label_font_size * 0.6;
    Bounds::new(
        label_x,
        label_y,
        label_width.max(1.0) + 4.0,
        wallet_label_font_size + 8.0,
    )
}

/// Bounds of the sidebar resize handle in logical coordinates. Used for hit-testing and cursor.
pub fn sidebar_handle_bounds(state: &RenderState) -> Bounds {
    if !RIGHT_SIDEBAR_ENABLED {
        return Bounds::new(-1000.0, -1000.0, 0.0, 0.0);
    }

    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let min_sidebar_width = 220.0;
    let max_sidebar_width = (width * 0.5).max(min_sidebar_width);
    let configured_width = state
        .sidebar
        .width
        .max(min_sidebar_width)
        .min(max_sidebar_width);
    let panel_width = if state.sidebar.is_open {
        configured_width
    } else {
        0.0
    };
    let sidebar_x = (width - panel_width).max(0.0);
    let panel_left_x = if state.sidebar.is_open {
        sidebar_x
    } else {
        (width - SIDEBAR_COLLAPSED_RAIL_WIDTH).max(0.0)
    };
    Bounds::new(
        (panel_left_x + SIDEBAR_HANDLE_ICON_LEFT_INSET).max(0.0),
        SIDEBAR_HANDLE_ICON_TOP_PAD,
        SIDEBAR_HANDLE_ICON_SIZE,
        SIDEBAR_HANDLE_ICON_SIZE,
    )
}

/// Bounds of the "Go Online" mission-control button in the sidebar (when panel is open).
pub fn sidebar_go_online_button_bounds(state: &RenderState) -> Bounds {
    if !RIGHT_SIDEBAR_ENABLED {
        return Bounds::new(-1000.0, -1000.0, 0.0, 0.0);
    }

    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let min_sidebar_width = 220.0;
    let max_sidebar_width = (width * 0.5).max(min_sidebar_width);
    let configured_width = state
        .sidebar
        .width
        .max(min_sidebar_width)
        .min(max_sidebar_width);
    let panel_width = if state.sidebar.is_open {
        configured_width
    } else {
        0.0
    };
    let sidebar_x = (width - panel_width).max(0.0);
    if panel_width < 1.0 {
        return Bounds::new(-1000.0, -1000.0, 0.0, 0.0);
    }
    let width = (panel_width - 24.0).max(120.0);
    Bounds::new(sidebar_x + 12.0, 72.0, width, 34.0)
}

pub(crate) fn command_registry(pane_filter: PaneSearchFilter) -> Vec<Command> {
    let mut commands: Vec<Command> = enabled_pane_specs()
        .filter(|spec| pane_filter.includes(pane_search_tier(spec.kind)))
        .filter_map(|spec| {
            let command = spec.command?;
            let mut entry = Command::new(command.id, command.label)
                .description(command.description)
                .category("Panes");
            if let Some(keybinding) = command.keybinding {
                entry = entry.keybinding(keybinding);
            }
            Some(entry)
        })
        .collect();

    if pane_filter.includes(crate::pane_registry::PaneSearchTier::Experimental) {
        commands.extend(cad_palette_command_specs().iter().map(|spec| {
            let mut command = Command::new(spec.id, spec.label)
                .description(spec.description)
                .category("CAD");
            if let Some(keys) = spec.keybinding {
                command = command.keybinding(keys);
            }
            command
        }));
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::{
        command_registry, pane_fullscreen_active_for_panes, wallet_balance_chip_bounds_for_logical,
    };
    use crate::app_state::{DesktopPane, PaneKind, PanePresentation};
    use crate::pane_registry::{
        PaneSearchFilter, enabled_pane_specs, pane_search_tier, pane_spec_by_command_id,
        startup_pane_kinds,
    };
    use crate::pane_system::cad_palette_command_specs;
    use std::collections::BTreeSet;
    use wgpui::{Bounds, Size};

    #[test]
    fn command_registry_matches_pane_specs() {
        let commands = command_registry(PaneSearchFilter::All);
        let command_ids: BTreeSet<&str> =
            commands.iter().map(|command| command.id.as_str()).collect();

        let pane_command_ids: BTreeSet<&str> = enabled_pane_specs()
            .filter(|spec| PaneSearchFilter::All.includes(pane_search_tier(spec.kind)))
            .filter_map(|spec| spec.command.map(|command| command.id))
            .collect();
        let cad_command_ids: BTreeSet<&str> = cad_palette_command_specs()
            .iter()
            .map(|spec| spec.id)
            .collect();
        let expected_ids: BTreeSet<&str> =
            pane_command_ids.union(&cad_command_ids).copied().collect();
        assert_eq!(command_ids, expected_ids);

        for command in &commands {
            if let Some(spec) = pane_spec_by_command_id(&command.id) {
                let pane_command = spec.command.expect("resolved pane must define a command");
                assert_eq!(command.label, pane_command.label);
                continue;
            }
            let cad_spec = cad_palette_command_specs()
                .iter()
                .find(|spec| spec.id == command.id)
                .expect("command id from registry should resolve to pane or cad command");
            assert_eq!(command.label, cad_spec.label);
        }
    }

    #[test]
    fn command_registry_includes_job_inbox_command() {
        let commands = command_registry(PaneSearchFilter::All);
        assert!(
            commands
                .iter()
                .any(|command| { command.id == "pane.job_inbox" && command.label == "Job Inbox" })
        );
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.active_job" && command.label == "Active Job"
            })
        );
        assert!(commands.iter().any(|command| {
            command.id == "pane.earnings_scoreboard" && command.label == "Earnings & Jobs"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.relay_connections" && command.label == "Relay Connections"
        }));
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.sync_health" && command.label == "Sync Health"
            })
        );
        assert!(commands.iter().any(|command| {
            command.id == "pane.network_requests" && command.label == "Network Requests"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.starter_jobs" && command.label == "Starter Jobs"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.reciprocal_loop" && command.label == "Reciprocal Loop"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.activity_feed" && command.label == "Activity Feed"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.alerts_recovery" && command.label == "Alerts and Recovery"
        }));
        assert!(
            commands
                .iter()
                .any(|command| { command.id == "pane.settings" && command.label == "Settings" })
        );
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.credentials" && command.label == "Credentials"
            })
        );
        assert!(
            commands
                .iter()
                .any(|command| { command.id == "pane.wallet" && command.label == "Spark Wallet" })
        );
        assert!(commands.iter().any(|command| {
            command.id == "pane.pay_invoice" && command.label == "Pay Lightning Invoice"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.create_invoice" && command.label == "Create Lightning Invoice"
        }));
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.job_history" && command.label == "Job History"
            })
        );
        assert!(commands.iter().any(|command| {
            command.id == "pane.agent_profile_state" && command.label == "Agent Profile and State"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.agent_schedule_tick" && command.label == "Agent Schedule and Tick"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.trajectory_audit" && command.label == "Trajectory Audit"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.cast_control" && command.label == "CAST Control"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.skill_registry" && command.label == "Agent Skill Registry"
        }));
        assert!(commands.iter().any(|command| {
            command.id == "pane.skill_trust_revocation"
                && command.label == "Skill Trust and Revocation"
        }));
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.credit_desk" && command.label == "Credit Desk"
            })
        );
        assert!(commands.iter().any(|command| {
            command.id == "pane.credit_settlement_ledger"
                && command.label == "Credit Settlement Ledger"
        }));
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.data_seller" && command.label == "Data Seller"
            })
        );
        assert!(
            commands.iter().any(|command| {
                command.id == "pane.data_market" && command.label == "Data Market"
            })
        );
    }

    #[test]
    fn startup_pane_set_restores_mission_control() {
        let startup = startup_pane_kinds();
        assert_eq!(startup, vec![PaneKind::AutopilotChat, PaneKind::GoOnline]);
        assert!(!startup.contains(&PaneKind::ProjectOps));
        assert!(!startup.contains(&PaneKind::CadDemo));
        assert!(!startup.contains(&PaneKind::SparkWallet));
        assert!(!startup.contains(&PaneKind::Empty));
        assert!(!startup.contains(&PaneKind::ProviderControl));
        assert!(!startup.contains(&PaneKind::EarningsScoreboard));
    }

    #[test]
    fn wallet_balance_chip_is_anchored_bottom_left() {
        let bounds = wallet_balance_chip_bounds_for_logical(Size::new(1280.0, 800.0));
        assert!(bounds.origin.x <= 16.0);
        assert!(bounds.max_y() >= 784.0);
        assert!(bounds.size.width >= 140.0);
        assert!(bounds.size.height >= 24.0);
    }

    #[test]
    fn pane_fullscreen_active_detects_fullscreen_panes() {
        let windowed_pane = DesktopPane {
            id: 1,
            title: "Windowed".to_string(),
            kind: PaneKind::AutopilotChat,
            bounds: Bounds::ZERO,
            windowed_bounds: Bounds::ZERO,
            z_index: 1,
            frame: wgpui::components::hud::PaneFrame::default(),
            presentation: PanePresentation::Windowed,
        };
        let fullscreen_pane = DesktopPane {
            presentation: PanePresentation::Fullscreen,
            ..DesktopPane {
                id: 2,
                title: "Fullscreen".to_string(),
                kind: PaneKind::Presentation,
                bounds: Bounds::ZERO,
                windowed_bounds: Bounds::ZERO,
                z_index: 2,
                frame: wgpui::components::hud::PaneFrame::default(),
                presentation: PanePresentation::Windowed,
            }
        };

        let mut panes = vec![windowed_pane];
        assert!(!pane_fullscreen_active_for_panes(&panes));

        panes.push(fullscreen_pane);
        assert!(pane_fullscreen_active_for_panes(&panes));
    }

    #[test]
    fn pane_fullscreen_active_ignores_docked_panes() {
        let docked_pane = DesktopPane {
            id: 3,
            title: "Docked".to_string(),
            kind: PaneKind::GoOnline,
            bounds: Bounds::ZERO,
            windowed_bounds: Bounds::ZERO,
            z_index: 1,
            frame: wgpui::components::hud::PaneFrame::default(),
            presentation: PanePresentation::DockedRight,
        };

        assert!(!pane_fullscreen_active_for_panes(&[docked_pane]));
    }
}
