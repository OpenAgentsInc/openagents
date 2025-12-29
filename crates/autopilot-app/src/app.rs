use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use autopilot_service::{AutopilotRuntime, RuntimeSnapshot, UnixDaemonClient};
use autopilot_ui::AutopilotIde;
use wgpui::{
    Bounds, EventContext, InputEvent, PaintContext, Point, Scene, Size, TextSystem,
};
use wgpui::renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton as WinitMouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

pub fn run() -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
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
    scale_factor: f32,
    root: AutopilotIde,
    event_cx: EventContext,
    runtime: AutopilotRuntime,
    cursor: Point,
    daemon_client: UnixDaemonClient,
    last_daemon_poll: Instant,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Autopilot IDE")
            .with_inner_size(winit::dpi::LogicalSize::new(1600, 1000));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
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
                scale_factor,
                root: AutopilotIde::new(),
                event_cx: EventContext::new(),
                runtime: AutopilotRuntime::default(),
                cursor: Point::ZERO,
                daemon_client: UnixDaemonClient::default(),
                last_daemon_poll: Instant::now() - Duration::from_secs(5),
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
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.scale_factor = scale_factor as f32;
                state.text_system.set_scale_factor(state.scale_factor);
                state.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor = Point::new(position.x as f32, position.y as f32);
                let input = InputEvent::MouseMove {
                    x: state.cursor.x,
                    y: state.cursor.y,
                };
                let bounds = Bounds::new(
                    0.0,
                    0.0,
                    state.config.width as f32,
                    state.config.height as f32,
                );
                let _ = state.root.event(&input, bounds, &mut state.event_cx);
                state.window.request_redraw();
            }
            WindowEvent::MouseInput { state: button_state, button, .. } => {
                let button = match button {
                    WinitMouseButton::Left => wgpui::MouseButton::Left,
                    WinitMouseButton::Right => wgpui::MouseButton::Right,
                    WinitMouseButton::Middle => wgpui::MouseButton::Middle,
                    _ => wgpui::MouseButton::Left,
                };

                let input = match button_state {
                    ElementState::Pressed => InputEvent::MouseDown {
                        button,
                        x: state.cursor.x,
                        y: state.cursor.y,
                    },
                    ElementState::Released => InputEvent::MouseUp {
                        button,
                        x: state.cursor.x,
                        y: state.cursor.y,
                    },
                };

                let bounds = Bounds::new(
                    0.0,
                    0.0,
                    state.config.width as f32,
                    state.config.height as f32,
                );
                let _ = state.root.event(&input, bounds, &mut state.event_cx);
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };

                let input = InputEvent::Scroll { dx: 0.0, dy };
                let bounds = Bounds::new(
                    0.0,
                    0.0,
                    state.config.width as f32,
                    state.config.height as f32,
                );
                let _ = state.root.event(&input, bounds, &mut state.event_cx);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed()
                    && matches!(event.physical_key, PhysicalKey::Code(KeyCode::Escape))
                {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                state.runtime.tick();
                let snapshot: RuntimeSnapshot = state.runtime.snapshot();
                state.root.apply_snapshot(&snapshot);

                if state.last_daemon_poll.elapsed() >= Duration::from_secs(2) {
                    let status = state.daemon_client.status();
                    state.root.set_daemon_status(status);
                    state.last_daemon_poll = Instant::now();
                }

                let width = state.config.width as f32;
                let height = state.config.height as f32;

                let mut scene = Scene::new();
                let mut cx = PaintContext::new(&mut scene, &mut state.text_system, state.scale_factor);
                let bounds = Bounds::new(0.0, 0.0, width, height);
                state.root.paint(bounds, &mut cx);

                state.renderer.resize(&state.queue, Size::new(width, height), state.scale_factor);

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

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
