//! Commander - OpenAgents startup with auth checking

use std::sync::Arc;
use std::time::Instant;
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

// Import auth from openagents crate
use openagents::auth;

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}

/// Log line for terminal display
#[derive(Clone)]
struct LogLine {
    text: String,
    timestamp: f32,
    status: LogStatus,
}

#[derive(Clone, Copy, PartialEq)]
enum LogStatus {
    Pending,
    Success,
    Error,
    Info,
}

/// Auth checking state machine
#[derive(Clone)]
struct AuthState {
    lines: Vec<LogLine>,
    phase: AuthPhase,
    phase_started: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum AuthPhase {
    CheckingOpenCode,
    CheckingOpenAgents,
    CopyingAuth,
    Complete,
}

impl AuthState {
    fn new() -> Self {
        Self {
            lines: vec![],
            phase: AuthPhase::CheckingOpenCode,
            phase_started: 0.0,
        }
    }

    fn add_line(&mut self, text: &str, status: LogStatus, elapsed: f32) {
        self.lines.push(LogLine {
            text: text.to_string(),
            timestamp: elapsed,
            status,
        });
    }

    fn tick(&mut self, elapsed: f32) {
        let phase_time = elapsed - self.phase_started;

        match self.phase {
            AuthPhase::CheckingOpenCode => {
                if self.lines.is_empty() {
                    self.add_line("Checking OpenCode auth...", LogStatus::Pending, elapsed);
                }

                if phase_time > 0.5 {
                    // Actually check OpenCode auth
                    let opencode_path = auth::opencode_auth_path();
                    let status = auth::check_opencode_auth();

                    // Update the pending line
                    if let Some(line) = self.lines.last_mut() {
                        line.status = LogStatus::Info;
                    }

                    match status {
                        auth::AuthStatus::Found { ref providers } => {
                            self.add_line(
                                &format!("  Found at {}", opencode_path.display()),
                                LogStatus::Success,
                                elapsed,
                            );
                            self.add_line(
                                &format!("  Providers: {}", providers.join(", ")),
                                LogStatus::Success,
                                elapsed,
                            );
                        }
                        auth::AuthStatus::NotFound => {
                            self.add_line(
                                &format!("  Not found at {}", opencode_path.display()),
                                LogStatus::Error,
                                elapsed,
                            );
                        }
                        auth::AuthStatus::Error(e) => {
                            self.add_line(&format!("  Error: {}", e), LogStatus::Error, elapsed);
                        }
                        _ => {}
                    }

                    self.phase = AuthPhase::CheckingOpenAgents;
                    self.phase_started = elapsed;
                }
            }

            AuthPhase::CheckingOpenAgents => {
                if phase_time < 0.3 {
                    return;
                }

                if !self.lines.iter().any(|l| l.text.contains("OpenAgents auth")) {
                    self.add_line("Checking OpenAgents auth...", LogStatus::Pending, elapsed);
                }

                if phase_time > 0.8 {
                    let openagents_path = auth::openagents_auth_path();
                    let status = auth::check_openagents_auth();

                    // Update pending line
                    if let Some(line) = self.lines.last_mut() {
                        if line.status == LogStatus::Pending {
                            line.status = LogStatus::Info;
                        }
                    }

                    match status {
                        auth::AuthStatus::Found { ref providers } => {
                            self.add_line(
                                &format!("  Found at {}", openagents_path.display()),
                                LogStatus::Success,
                                elapsed,
                            );
                            self.add_line(
                                &format!("  Providers: {}", providers.join(", ")),
                                LogStatus::Success,
                                elapsed,
                            );
                            self.phase = AuthPhase::Complete;
                            self.phase_started = elapsed;
                        }
                        auth::AuthStatus::NotFound => {
                            self.add_line("  Not configured yet", LogStatus::Info, elapsed);
                            // Try to copy from OpenCode
                            self.phase = AuthPhase::CopyingAuth;
                            self.phase_started = elapsed;
                        }
                        auth::AuthStatus::Error(e) => {
                            self.add_line(&format!("  Error: {}", e), LogStatus::Error, elapsed);
                            self.phase = AuthPhase::Complete;
                            self.phase_started = elapsed;
                        }
                        _ => {}
                    }
                }
            }

            AuthPhase::CopyingAuth => {
                if phase_time < 0.3 {
                    return;
                }

                if !self.lines.iter().any(|l| l.text.contains("Copying")) {
                    self.add_line("Copying credentials from OpenCode...", LogStatus::Pending, elapsed);
                }

                if phase_time > 0.8 {
                    // Update pending line
                    if let Some(line) = self.lines.last_mut() {
                        if line.status == LogStatus::Pending {
                            line.status = LogStatus::Info;
                        }
                    }

                    match auth::copy_opencode_auth() {
                        Ok(auth::AuthStatus::Copied { providers }) => {
                            self.add_line(
                                &format!("  Imported {} provider(s)", providers.len()),
                                LogStatus::Success,
                                elapsed,
                            );
                            self.add_line(
                                &format!("  Saved to {}", auth::openagents_auth_path().display()),
                                LogStatus::Success,
                                elapsed,
                            );
                        }
                        Ok(auth::AuthStatus::NotFound) => {
                            self.add_line("  No credentials to copy", LogStatus::Error, elapsed);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            self.add_line(&format!("  Error: {}", e), LogStatus::Error, elapsed);
                        }
                    }

                    self.phase = AuthPhase::Complete;
                    self.phase_started = elapsed;
                }
            }

            AuthPhase::Complete => {
                if phase_time > 0.3 && !self.lines.iter().any(|l| l.text.contains("Ready")) {
                    if auth::has_anthropic_auth() {
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Ready.", LogStatus::Success, elapsed);
                    } else {
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Anthropic auth not configured.", LogStatus::Error, elapsed);
                        self.add_line("Run: opencode auth login", LogStatus::Info, elapsed);
                    }
                }
            }
        }
    }
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
    auth_state: AuthState,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Commander")
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

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                start_time: Instant::now(),
                auth_state: AuthState::new(),
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
                if event.state.is_pressed() {
                    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;

                let elapsed = state.start_time.elapsed().as_secs_f32();
                let dots_progress = ease_out_cubic((elapsed / 1.5).min(1.0));
                let frame_progress = ease_out_cubic(((elapsed - 0.8) / 1.0).clamp(0.0, 1.0));

                // Update auth state after frame is visible
                if frame_progress > 0.7 {
                    let auth_elapsed = elapsed - 1.8; // Start auth checks after frame is mostly in
                    if auth_elapsed > 0.0 {
                        state.auth_state.tick(auth_elapsed);
                    }
                }

                let mut scene = Scene::new();
                render(
                    &mut scene,
                    &mut state.text_system,
                    width,
                    height,
                    dots_progress,
                    frame_progress,
                    &state.auth_state,
                );

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

                state.renderer.prepare(&state.device, &scene);
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

fn render(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    width: f32,
    height: f32,
    dots_progress: f32,
    frame_progress: f32,
    auth_state: &AuthState,
) {
    // Black background
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, height))
            .with_background(Hsla::new(0.0, 0.0, 0.0, 1.0)),
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);

    // DotsGrid background
    let mut dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 0.25, 0.2))
        .shape(DotShape::Circle)
        .distance(48.0)
        .size(2.0)
        .origin(DotsOrigin::Center)
        .easing(Easing::EaseOut)
        .animation_progress(dots_progress);

    dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);

    // Center frame (800x600)
    if frame_progress > 0.0 {
        let frame_w = 800.0;
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

        // Terminal log lines inside frame
        if frame_progress > 0.5 {
            let text_alpha = ((frame_progress - 0.5) * 2.0).min(1.0);
            let line_height = 18.0;
            let start_x = frame_x + 20.0;
            let start_y = frame_y + 24.0;

            for (i, log_line) in auth_state.lines.iter().enumerate() {
                let y = start_y + (i as f32 * line_height);

                // Skip if outside frame
                if y > frame_y + frame_h - 30.0 {
                    break;
                }

                let color = match log_line.status {
                    LogStatus::Pending => Hsla::new(45.0, 0.8, 0.6, text_alpha * 0.8), // Yellow
                    LogStatus::Success => Hsla::new(120.0, 0.5, 0.5, text_alpha), // Green
                    LogStatus::Error => Hsla::new(0.0, 0.6, 0.5, text_alpha),     // Red
                    LogStatus::Info => Hsla::new(0.0, 0.0, 0.5, text_alpha),      // Gray
                };

                let prefix = match log_line.status {
                    LogStatus::Pending => "> ",
                    LogStatus::Success => "  ",
                    LogStatus::Error => "  ",
                    LogStatus::Info => "  ",
                };

                let text = format!("{}{}", prefix, log_line.text);
                let text_run = cx.text.layout(&text, Point::new(start_x, y), 13.0, color);
                cx.scene.draw_text(text_run);
            }
        }
    }
}
