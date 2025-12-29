use std::sync::Arc;
use std::time::Instant;
use tracing::info;
use wgpui::{
    Bounds, Component, Easing, Hsla, PaintContext, Point, Quad, Scene, Size, TextSystem,
};
use wgpui::components::hud::{
    CornerConfig, DotsGrid, DotsOrigin, DotShape, DrawDirection, Frame, FrameAnimation,
};
use wgpui::renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use autopilot::{StartupState, LogStatus, ClaudeModel, wrap_text};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new(
                        "autopilot=debug,openagents=debug,wgpui=info,cosmic_text=warn,wgpu=warn,info"
                    )
                })
        )
        .with_target(true)
        .init();

    info!("Starting Autopilot");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}

#[derive(Default)]
struct App {
    state: Option<RenderState>,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    start_time: Instant,
    startup_state: StartupState,
    scroll_offset: f32,
    auto_scroll: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Autopilot")
            .with_inner_size(winit::dpi::LogicalSize::new(1600, 1000));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to create device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, surface_format);
            let scale_factor = window.scale_factor() as f32;
            let text_system = TextSystem::new(scale_factor);

            let model = match std::env::var("AUTOPILOT_MODEL").as_deref() {
                Ok("opus") | Ok("Opus") | Ok("OPUS") => ClaudeModel::Opus,
                _ => ClaudeModel::Sonnet,
            };
            
            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                start_time: Instant::now(),
                startup_state: StartupState::with_model(model),
                scroll_offset: 0.0,
                auto_scroll: true,
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
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed()
                    && let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                        event_loop.exit();
                    }
            }
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_amount = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };

                if scroll_amount.abs() > 0.1 {
                    state.auto_scroll = false;
                }

                state.scroll_offset = (state.scroll_offset - scroll_amount).max(0.0);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;

                let elapsed = state.start_time.elapsed().as_secs_f32();
                let dots_progress = ease_out_cubic((elapsed / 1.5).min(1.0));
                let frame_progress = ease_out_cubic(((elapsed - 0.8) / 1.0).clamp(0.0, 1.0));

                if frame_progress > 0.7 {
                    let startup_elapsed = elapsed - 1.8;
                    if startup_elapsed > 0.0 {
                        state.startup_state.tick(startup_elapsed);
                    }
                }

                let mut scene = Scene::new();
                let (max_scroll, _) = render(
                    &mut scene,
                    &mut state.text_system,
                    width,
                    height,
                    dots_progress,
                    frame_progress,
                    &state.startup_state,
                    state.scroll_offset,
                    state.auto_scroll,
                );

                state.scroll_offset = state.scroll_offset.min(max_scroll).max(0.0);

                if state.scroll_offset >= max_scroll - 1.0 {
                    state.auto_scroll = true;
                }

                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = state
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });

                state.renderer.resize(&state.queue, Size::new(width, height), 1.0);

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                let scale_factor = state.window.scale_factor() as f32;
                state.renderer.prepare(&state.device, &state.queue, &scene, scale_factor);
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

fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

struct WrappedLine {
    text: String,
    color: Hsla,
}

fn render(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    width: f32,
    height: f32,
    dots_progress: f32,
    frame_progress: f32,
    startup_state: &StartupState,
    scroll_offset: f32,
    auto_scroll: bool,
) -> (f32, f32) {
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, height))
            .with_background(Hsla::new(0.0, 0.0, 0.0, 1.0)),
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);

    let mut dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 0.25, 0.2))
        .shape(DotShape::Circle)
        .distance(48.0)
        .size(2.0)
        .origin(DotsOrigin::Center)
        .easing(Easing::EaseOut)
        .animation_progress(dots_progress);

    dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);

    if frame_progress > 0.0 {
        let frame_w = 1000.0;
        let frame_h = 600.0;
        let frame_x = (width - frame_w) / 2.0;
        let frame_y = (height - frame_h) / 2.0;

        let line_color = Hsla::new(0.0, 0.0, 0.7, frame_progress);
        let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.95 * frame_progress);
        let glow_color = Hsla::new(180.0, 0.6, 0.5, 0.3 * frame_progress);

        let mut frame = Frame::nefrex()
            .line_color(line_color)
            .bg_color(bg_color)
            .glow_color(glow_color)
            .stroke_width(1.5)
            .corner_config(CornerConfig::all())
            .square_size(10.0)
            .small_line_length(10.0)
            .large_line_length(40.0)
            .animation_mode(FrameAnimation::Assemble)
            .draw_direction(DrawDirection::CenterOut)
            .animation_progress(frame_progress);

        frame.paint(Bounds::new(frame_x, frame_y, frame_w, frame_h), &mut cx);

        if frame_progress > 0.5 {
            let text_alpha = ((frame_progress - 0.5) * 2.0).min(1.0);
            let line_height = 22.0;
            let font_size = 12.0;
            let padding = 16.0;
            let text_area_x = frame_x + padding;
            let text_area_y = frame_y + padding;
            let text_area_w = frame_w - padding * 2.0;
            let text_area_h = frame_h - padding * 2.0;

            let char_width = 7.2;
            let max_chars = (text_area_w / char_width) as usize;
            let max_visible_lines = (text_area_h / line_height) as usize;

            let mut all_wrapped: Vec<WrappedLine> = Vec::new();

            for log_line in &startup_state.lines {
                let color = match log_line.status {
                    LogStatus::Pending => Hsla::new(45.0, 0.9, 0.65, text_alpha),
                    LogStatus::Success => Hsla::new(120.0, 0.7, 0.6, text_alpha),
                    LogStatus::Error => Hsla::new(0.0, 0.8, 0.6, text_alpha),
                    LogStatus::Info => Hsla::new(0.0, 0.0, 0.7, text_alpha),
                    LogStatus::Thinking => Hsla::new(270.0, 0.5, 0.6, text_alpha * 0.7),
                };

                let prefix = match log_line.status {
                    LogStatus::Pending => "> ",
                    _ => "  ",
                };

                let full_text = format!("{}{}", prefix, log_line.text);
                let wrapped = wrap_text(&full_text, max_chars);

                for line in wrapped {
                    all_wrapped.push(WrappedLine { text: line, color });
                }
            }

            let total_visual_lines = all_wrapped.len();
            let content_height = total_visual_lines as f32 * line_height;
            let max_scroll = (content_height - text_area_h).max(0.0);

            let start_idx = if auto_scroll {
                total_visual_lines.saturating_sub(max_visible_lines)
            } else {
                let scroll_lines = (scroll_offset / line_height) as usize;
                scroll_lines.min(total_visual_lines.saturating_sub(max_visible_lines))
            };

            for (i, wrapped_line) in all_wrapped.iter().skip(start_idx).take(max_visible_lines + 1).enumerate() {
                let y = text_area_y + (i as f32 * line_height);

                if y > frame_y + frame_h - padding {
                    break;
                }

                let text_run = cx.text.layout(&wrapped_line.text, Point::new(text_area_x, y), font_size, wrapped_line.color);
                cx.scene.draw_text(text_run);
            }

            return (max_scroll, content_height);
        }
    }

    (0.0, 0.0)
}
