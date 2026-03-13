#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::unwrap_used
)]

use anyhow::{Context, Result, anyhow};
use autopilot_desktop::rive_assets::{
    PackagedRiveAsset, default_packaged_rive_asset, packaged_rive_asset, packaged_rive_assets,
};
use clap::{Parser, ValueEnum};
use std::sync::Arc;
use wgpui::components::hud::{DotShape, DotsGrid, Scanlines};
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, EventContext, Hsla, InputEvent, Modifiers, MouseButton, PaintContext, Point,
    Quad, RiveFitMode, RiveHandle, RiveMetrics, RiveSurface, Scene, Size, TextSystem, theme,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton as WinitMouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum FitArg {
    Contain,
    Cover,
    Fill,
}

impl From<FitArg> for RiveFitMode {
    fn from(value: FitArg) -> Self {
        match value {
            FitArg::Contain => Self::Contain,
            FitArg::Cover => Self::Cover,
            FitArg::Fill => Self::Fill,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Standalone native WGPUI viewer for the packaged Rive asset registry"
)]
struct ViewerArgs {
    #[arg(long, default_value = "simple_fui_hud")]
    asset: String,
    #[arg(long, default_value = "default")]
    artboard: String,
    #[arg(long, default_value = "default")]
    scene: String,
    #[arg(long, value_enum, default_value_t = FitArg::Contain)]
    fit: FitArg,
    #[arg(long, default_value_t = false)]
    list_assets: bool,
}

fn main() -> Result<()> {
    let args = ViewerArgs::parse();
    if args.list_assets {
        print_packaged_assets();
        return Ok(());
    }
    let event_loop = EventLoop::new().context("failed to create viewer event loop")?;
    let mut app = App::new(args);
    event_loop
        .run_app(&mut app)
        .context("viewer event loop terminated with error")?;
    Ok(())
}

struct App {
    args: ViewerArgs,
    state: Option<RenderState>,
}

impl App {
    fn new(args: ViewerArgs) -> Self {
        Self { args, state: None }
    }
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    viewer: ViewerState,
}

struct ViewerState {
    asset: PackagedRiveAsset,
    surface: RiveSurface,
    canvas_bounds: Bounds,
    artboard_handle: RiveHandle,
    scene_handle: RiveHandle,
    mouse_position: Point,
    last_logged_metrics: Option<RiveMetrics>,
}

impl ViewerState {
    fn new(args: &ViewerArgs) -> Result<Self> {
        let asset = packaged_rive_asset(args.asset.as_str()).with_context(|| {
            format!(
                "unknown packaged Rive asset id '{}' (use --list-assets)",
                args.asset
            )
        })?;
        let artboard_handle = parse_handle(args.artboard.as_str())?;
        let scene_handle = parse_handle(args.scene.as_str())?;
        let mut surface = RiveSurface::from_bytes_with_handles(
            asset.bytes,
            artboard_handle.clone(),
            scene_handle.clone(),
            None,
        )
        .with_context(|| {
            format!(
                "failed to instantiate {} with artboard={} scene={}",
                asset.file_name,
                handle_label(&artboard_handle),
                handle_label(&scene_handle)
            )
        })?;
        surface.controller_mut().set_fit_mode(args.fit.into());
        println!(
            "Loaded {} from {} with artboard={} scene={} fit={}",
            asset.file_name,
            asset.runtime_path,
            handle_label(&artboard_handle),
            handle_label(&scene_handle),
            fit_label(surface.controller().fit_mode())
        );
        Ok(Self {
            asset,
            surface,
            canvas_bounds: Bounds::ZERO,
            artboard_handle,
            scene_handle,
            mouse_position: Point::ZERO,
            last_logged_metrics: None,
        })
    }

    fn toggle_playback(&mut self) {
        if self.surface.controller().is_paused() {
            self.surface.controller_mut().play();
            println!("Playback resumed");
        } else {
            self.surface.controller_mut().pause();
            println!("Playback paused");
        }
    }

    fn restart(&mut self) {
        match self.surface.controller_mut().restart() {
            Ok(()) => println!("Scene restarted"),
            Err(error) => eprintln!("Restart failed: {error}"),
        }
    }

    fn set_fit_mode(&mut self, fit_mode: RiveFitMode) {
        self.surface.controller_mut().set_fit_mode(fit_mode);
        println!("Fit mode -> {}", fit_label(fit_mode));
    }

    fn dispatch_input(&mut self, input: InputEvent) {
        let mut event_context = EventContext::new();
        let _ = self
            .surface
            .event(&input, self.canvas_bounds, &mut event_context);
    }

    fn render(
        &mut self,
        scene: &mut Scene,
        text_system: &mut TextSystem,
        window_bounds: Bounds,
        scale_factor: f32,
    ) {
        scene.draw_quad(Quad::new(window_bounds).with_background(theme::bg::APP));

        let mut paint = PaintContext::new(scene, text_system, scale_factor);
        let mut dots = DotsGrid::new()
            .shape(DotShape::Cross)
            .distance(26.0)
            .size(1.1)
            .color(Hsla::from_hex(0x1dcad3).with_alpha(0.18))
            .animation_progress(1.0);
        dots.paint(window_bounds, &mut paint);

        self.paint_header(&mut paint, window_bounds);
        self.paint_canvas_shell(&mut paint);
        self.surface.paint(self.canvas_bounds, &mut paint);
        self.paint_overlay_guides(&mut paint);
        self.paint_info_panel(&mut paint, window_bounds);
        self.maybe_log_metrics();
    }

    fn paint_header(&self, paint: &mut PaintContext, bounds: Bounds) {
        paint.scene.draw_text(paint.text.layout_mono(
            "RIVE HUD VIEWER  //  NATIVE WGPUI BRING-UP",
            Point::new(24.0, 22.0),
            13.0,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout(
            "Packaged asset registry, no pane system, same RiveSurface path used by desktop integration.",
            Point::new(24.0, 40.0),
            11.0,
            theme::text::MUTED,
        ));
        paint.scene.draw_quad(
            Quad::new(Bounds::new(bounds.max_x() - 220.0, 24.0, 196.0, 2.0))
                .with_background(theme::accent::PRIMARY.with_alpha(0.65)),
        );
    }

    fn paint_canvas_shell(&self, paint: &mut PaintContext) {
        let shell_bounds = Bounds::new(
            self.canvas_bounds.origin.x - 12.0,
            self.canvas_bounds.origin.y - 12.0,
            self.canvas_bounds.size.width + 24.0,
            self.canvas_bounds.size.height + 24.0,
        );
        paint.scene.draw_quad(
            Quad::new(shell_bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::accent::PRIMARY.with_alpha(0.35), 1.0)
                .with_corner_radius(10.0),
        );
        paint.scene.draw_quad(
            Quad::new(self.canvas_bounds)
                .with_background(Hsla::from_hex(0x07131a).with_alpha(0.98))
                .with_corner_radius(8.0),
        );
        let mut scanlines = Scanlines::new()
            .spacing(14.0)
            .line_color(theme::accent::PRIMARY.with_alpha(0.06))
            .scan_color(theme::accent::PRIMARY.with_alpha(0.16))
            .scan_width(18.0)
            .scan_progress(0.32)
            .opacity(0.8);
        scanlines.paint(self.canvas_bounds, paint);
    }

    fn paint_overlay_guides(&self, paint: &mut PaintContext) {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                self.canvas_bounds.origin.x,
                self.canvas_bounds.origin.y,
                self.canvas_bounds.size.width,
                1.0,
            ))
            .with_background(theme::accent::PRIMARY.with_alpha(0.18)),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                self.canvas_bounds.origin.x,
                self.canvas_bounds.max_y() - 1.0,
                self.canvas_bounds.size.width,
                1.0,
            ))
            .with_background(theme::accent::PRIMARY.with_alpha(0.18)),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            "canvas",
            Point::new(
                self.canvas_bounds.origin.x + 8.0,
                self.canvas_bounds.origin.y + 10.0,
            ),
            9.0,
            theme::text::MUTED,
        ));
    }

    fn paint_info_panel(&self, paint: &mut PaintContext, full_bounds: Bounds) {
        let panel_bounds = Bounds::new(
            self.canvas_bounds.max_x() + 24.0,
            self.canvas_bounds.origin.y,
            full_bounds.max_x() - self.canvas_bounds.max_x() - 48.0,
            self.canvas_bounds.size.height,
        );
        paint.scene.draw_quad(
            Quad::new(panel_bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(10.0),
        );

        let metrics = self.surface.controller().metrics().clone();
        let lines = [
            format!("asset id     {}", self.asset.id),
            format!("asset        {}", self.asset.file_name),
            format!("runtime      {}", self.asset.runtime_path),
            format!("summary      {}", self.asset.description),
            format!("artboard     {}", handle_label(&self.artboard_handle)),
            format!("scene        {}", handle_label(&self.scene_handle)),
            format!(
                "fit          {}",
                fit_label(self.surface.controller().fit_mode())
            ),
            format!(
                "playback     {}",
                if self.surface.controller().is_paused() {
                    "paused"
                } else {
                    "playing"
                }
            ),
            format!("metrics.scene {}", metrics.scene_name),
            format!(
                "metrics.size  {:.0} x {:.0}",
                metrics.artboard_size.width, metrics.artboard_size.height
            ),
            format!("commands     {}", metrics.command_count),
            format!("images       {}", metrics.image_count),
            format!(
                "pointer      {:.0},{:.0}",
                self.mouse_position.x, self.mouse_position.y
            ),
        ];

        paint.scene.draw_text(paint.text.layout_mono(
            "VERIFICATION",
            Point::new(panel_bounds.origin.x + 14.0, panel_bounds.origin.y + 16.0),
            11.0,
            theme::accent::PRIMARY,
        ));
        for (index, line) in lines.iter().enumerate() {
            paint.scene.draw_text(paint.text.layout_mono(
                line,
                Point::new(
                    panel_bounds.origin.x + 14.0,
                    panel_bounds.origin.y + 42.0 + index as f32 * 18.0,
                ),
                10.0,
                theme::text::PRIMARY,
            ));
        }

        paint.scene.draw_text(paint.text.layout_mono(
            "CONTROLS",
            Point::new(panel_bounds.origin.x + 14.0, panel_bounds.origin.y + 278.0),
            11.0,
            theme::accent::PRIMARY,
        ));
        let control_lines = [
            "space  pause / resume",
            "r      restart scene",
            "1      contain fit",
            "2      cover fit",
            "3      fill fit",
            "--asset <id>  choose packaged asset",
            "esc    quit viewer",
            "mouse  forwards pointer events",
        ];
        for (index, line) in control_lines.iter().enumerate() {
            paint.scene.draw_text(paint.text.layout_mono(
                line,
                Point::new(
                    panel_bounds.origin.x + 14.0,
                    panel_bounds.origin.y + 304.0 + index as f32 * 18.0,
                ),
                10.0,
                theme::text::PRIMARY,
            ));
        }
    }

    fn maybe_log_metrics(&mut self) {
        let metrics = self.surface.controller().metrics().clone();
        if self.last_logged_metrics.as_ref() == Some(&metrics) {
            return;
        }
        println!(
            "Frame metrics: scene={} size={:.0}x{:.0} commands={} images={}",
            metrics.scene_name,
            metrics.artboard_size.width,
            metrics.artboard_size.height,
            metrics.command_count,
            metrics.image_count
        );
        self.last_logged_metrics = Some(metrics);
    }
}

fn print_packaged_assets() {
    println!(
        "Default packaged asset: {}",
        default_packaged_rive_asset().id
    );
    for asset in packaged_rive_assets() {
        println!(
            "{}  {}  {}  artboard={} scene={}  {}",
            asset.id,
            asset.file_name,
            asset.runtime_path,
            asset.default_artboard,
            asset.default_scene,
            asset.description
        );
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Autopilot Rive HUD Viewer")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 820.0));
        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("failed to create viewer window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });
            let surface = instance
                .create_surface(window.clone())
                .expect("failed to create viewer surface");
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("failed to find compatible viewer adapter");
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("failed to create viewer device");
            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|format| format.is_srgb())
                .copied()
                .or_else(|| surface_caps.formats.first().copied())
                .expect("viewer surface formats empty");
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps
                    .alpha_modes
                    .first()
                    .copied()
                    .unwrap_or(wgpu::CompositeAlphaMode::Auto),
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);
            let renderer = Renderer::new(&device, surface_format);
            let scale_factor = window.scale_factor() as f32;
            let text_system = TextSystem::new(scale_factor);
            let viewer = ViewerState::new(&self.args).expect("viewer state should initialize");

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                viewer,
            }
        });

        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.viewer.mouse_position = Point::new(position.x as f32, position.y as f32);
                state.viewer.dispatch_input(InputEvent::MouseMove {
                    x: state.viewer.mouse_position.x,
                    y: state.viewer.mouse_position.y,
                });
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                if let Some(button) = map_mouse_button(button) {
                    let input = match button_state {
                        ElementState::Pressed => InputEvent::MouseDown {
                            button,
                            x: state.viewer.mouse_position.x,
                            y: state.viewer.mouse_position.y,
                            modifiers: Modifiers::default(),
                        },
                        ElementState::Released => InputEvent::MouseUp {
                            button,
                            x: state.viewer.mouse_position.x,
                            y: state.viewer.mouse_position.y,
                        },
                    };
                    state.viewer.dispatch_input(input);
                    state.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                    PhysicalKey::Code(KeyCode::Space) => {
                        state.viewer.toggle_playback();
                        state.window.request_redraw();
                    }
                    PhysicalKey::Code(KeyCode::KeyR) => {
                        state.viewer.restart();
                        state.window.request_redraw();
                    }
                    PhysicalKey::Code(KeyCode::Digit1 | KeyCode::Numpad1) => {
                        state.viewer.set_fit_mode(RiveFitMode::Contain);
                        state.window.request_redraw();
                    }
                    PhysicalKey::Code(KeyCode::Digit2 | KeyCode::Numpad2) => {
                        state.viewer.set_fit_mode(RiveFitMode::Cover);
                        state.window.request_redraw();
                    }
                    PhysicalKey::Code(KeyCode::Digit3 | KeyCode::Numpad3) => {
                        state.viewer.set_fit_mode(RiveFitMode::Fill);
                        state.window.request_redraw();
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;
                let max_canvas_width = (width - 320.0).max(240.0);
                state.viewer.canvas_bounds = Bounds::new(
                    24.0,
                    72.0,
                    (width * 0.68).max(360.0).min(max_canvas_width),
                    (height - 104.0).max(240.0),
                );

                let scale_factor = state.window.scale_factor() as f32;
                let window_bounds = Bounds::new(0.0, 0.0, width, height);
                let mut scene = Scene::new();
                state.viewer.render(
                    &mut scene,
                    &mut state.text_system,
                    window_bounds,
                    scale_factor,
                );

                let output = state
                    .surface
                    .get_current_texture()
                    .expect("failed to acquire viewer surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Rive Viewer Encoder"),
                        });

                state
                    .renderer
                    .resize(&state.queue, Size::new(width, height), scale_factor);
                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }
                state
                    .renderer
                    .prepare(&state.device, &state.queue, &scene, scale_factor);
                state.renderer.render(&mut encoder, &view);
                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

fn map_mouse_button(button: WinitMouseButton) -> Option<MouseButton> {
    match button {
        WinitMouseButton::Left => Some(MouseButton::Left),
        WinitMouseButton::Right => Some(MouseButton::Right),
        WinitMouseButton::Middle => Some(MouseButton::Middle),
        _ => None,
    }
}

fn parse_handle(raw: &str) -> Result<RiveHandle> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("default") {
        return Ok(RiveHandle::Default);
    }
    if let Some(rest) = trimmed.strip_prefix("index:") {
        let index = rest
            .parse::<usize>()
            .with_context(|| format!("invalid artboard/scene handle index: {trimmed}"))?;
        return Ok(RiveHandle::Index(index));
    }
    if let Some(rest) = trimmed.strip_prefix("name:") {
        if rest.is_empty() {
            return Err(anyhow!("empty name handle is not supported"));
        }
        return Ok(RiveHandle::Name(rest.to_string()));
    }
    Ok(RiveHandle::Name(trimmed.to_string()))
}

fn handle_label(handle: &RiveHandle) -> String {
    match handle {
        RiveHandle::Default => "default".to_string(),
        RiveHandle::Index(index) => format!("index:{index}"),
        RiveHandle::Name(name) => name.clone(),
    }
}

fn fit_label(fit_mode: RiveFitMode) -> &'static str {
    match fit_mode {
        RiveFitMode::Contain => "contain",
        RiveFitMode::Cover => "cover",
        RiveFitMode::Fill => "fill",
    }
}
