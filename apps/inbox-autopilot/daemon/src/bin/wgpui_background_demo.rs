use std::sync::Arc;
use std::time::Instant;

use wgpui::components::hud::{DotShape, DotsGrid};
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, Hsla, PaintContext, Point, Quad, Scene, Size, Text, TextSystem, theme,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

const WINDOW_TITLE: &str = "Inbox Autopilot • WGPUI Canvas Demo";
const WINDOW_WIDTH: f64 = 1320.0;
const WINDOW_HEIGHT: f64 = 840.0;
const GRID_DOT_DISTANCE: f32 = 32.0;

fn main() {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = DemoApp::default();
    event_loop.run_app(&mut app).expect("event loop failed");
}

#[derive(Default)]
struct DemoApp {
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
    background: DemoBackground,
    last_frame_at: Instant,
}

#[derive(Default)]
struct DemoBackground {
    offset: Point,
}

impl DemoBackground {
    fn tick(&mut self, dt_seconds: f32) {
        // Slow drift gives the background a subtle living-canvas feel.
        self.offset = Point::new(
            (self.offset.x + dt_seconds * 7.5).rem_euclid(GRID_DOT_DISTANCE),
            (self.offset.y + dt_seconds * 3.0).rem_euclid(GRID_DOT_DISTANCE),
        );
    }
}

impl Component for DemoBackground {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene
            .draw_quad(Quad::new(bounds).with_background(Hsla::black()));

        let mut dots = DotsGrid::new()
            .shape(DotShape::Circle)
            .color(theme::text::MUTED)
            .opacity(0.12)
            .distance(GRID_DOT_DISTANCE)
            .size(1.5);
        let grid_bounds = Bounds::new(
            bounds.origin.x + self.offset.x,
            bounds.origin.y + self.offset.y,
            bounds.size.width,
            bounds.size.height,
        );
        dots.paint(grid_bounds, cx);

        let panel_bounds = Bounds::new(bounds.origin.x + 28.0, bounds.origin.y + 24.0, 560.0, 92.0);
        cx.scene.draw_quad(
            Quad::new(panel_bounds)
                .with_background(theme::bg::ELEVATED.with_alpha(0.9))
                .with_border(theme::border::SUBTLE, 1.0),
        );

        Text::new("Inbox Autopilot WGPUI Canvas Demo")
            .bold()
            .font_size(18.0)
            .color(theme::text::PRIMARY)
            .paint(
                Bounds::new(
                    panel_bounds.origin.x + 16.0,
                    panel_bounds.origin.y + 16.0,
                    panel_bounds.size.width - 32.0,
                    28.0,
                ),
                cx,
            );

        Text::new("Using the same dark+dots background style as Autopilot desktop.")
            .font_size(13.0)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(
                    panel_bounds.origin.x + 16.0,
                    panel_bounds.origin.y + 50.0,
                    panel_bounds.size.width - 32.0,
                    24.0,
                ),
                cx,
            );
    }
}

impl ApplicationHandler for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title(WINDOW_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance
                .create_surface(window.clone())
                .expect("failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("failed to request adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("failed to request device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|format| format.is_srgb())
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

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system: TextSystem::new(1.0),
                background: DemoBackground::default(),
                last_frame_at: Instant::now(),
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
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.text_system = TextSystem::new(scale_factor as f32);
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt_seconds = now.duration_since(state.last_frame_at).as_secs_f32();
                state.last_frame_at = now;
                state.background.tick(dt_seconds);

                let logical_size = Size::new(state.config.width as f32, state.config.height as f32);

                let mut scene = Scene::new();
                let mut paint = PaintContext::new(&mut scene, &mut state.text_system, 1.0);
                state
                    .background
                    .paint(Bounds::new(0.0, 0.0, logical_size.width, logical_size.height), &mut paint);

                state.renderer.resize(&state.queue, logical_size, 1.0);

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                let output = match state.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Lost) => {
                        state.surface.configure(&state.device, &state.config);
                        return;
                    }
                    Err(wgpu::SurfaceError::Outdated) => {
                        return;
                    }
                    Err(wgpu::SurfaceError::Timeout) => {
                        return;
                    }
                    Err(err) => {
                        eprintln!("surface error: {err:?}");
                        event_loop.exit();
                        return;
                    }
                };

                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("InboxAutopilotWgpuiDemoRenderEncoder"),
                        });

                state
                    .renderer
                    .prepare(&state.device, &state.queue, &scene, 1.0);
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
