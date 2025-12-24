use std::sync::Arc;
use wgpui::{Bounds, Quad, Scene, Size, theme};
use wgpui::renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

fn main() {
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
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("wgpui - First Light")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

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
            renderer.resize(
                &queue,
                Size::new(size.width as f32, size.height as f32),
                1.0,
            );

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
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
                state.renderer.resize(
                    &state.queue,
                    Size::new(new_size.width as f32, new_size.height as f32),
                    1.0,
                );
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let scene = build_scene(state.config.width as f32, state.config.height as f32);

                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        });

                let mut renderer = Renderer::new(&state.device, state.config.format);
                renderer.resize(
                    &state.queue,
                    Size::new(state.config.width as f32, state.config.height as f32),
                    1.0,
                );
                renderer.prepare(&state.device, &scene);
                renderer.render(&mut encoder, &view);

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

fn build_scene(width: f32, _height: f32) -> Scene {
    let mut scene = Scene::new();

    let margin = 20.0;
    let spacing = 16.0;
    let quad_height = 80.0;
    let content_width = width - margin * 2.0;

    let mut y = margin;

    scene.draw_quad(
        Quad::new(Bounds::new(margin, y, content_width, quad_height))
            .with_background(theme::bg::APP)
            .with_border(theme::border::DEFAULT, 1.0),
    );
    y += quad_height + spacing;

    scene.draw_quad(
        Quad::new(Bounds::new(margin, y, content_width, quad_height))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );
    y += quad_height + spacing;

    scene.draw_quad(
        Quad::new(Bounds::new(margin, y, content_width, quad_height))
            .with_background(theme::bg::MUTED)
            .with_border(theme::border::DEFAULT, 1.0),
    );
    y += quad_height + spacing;

    let accent_width = (content_width - spacing * 4.0) / 5.0;
    scene.draw_quad(
        Quad::new(Bounds::new(margin, y, accent_width, quad_height))
            .with_background(theme::accent::PRIMARY),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(
            margin + accent_width + spacing,
            y,
            accent_width,
            quad_height,
        ))
        .with_background(theme::accent::BLUE),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(
            margin + (accent_width + spacing) * 2.0,
            y,
            accent_width,
            quad_height,
        ))
        .with_background(theme::accent::GREEN),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(
            margin + (accent_width + spacing) * 3.0,
            y,
            accent_width,
            quad_height,
        ))
        .with_background(theme::accent::RED),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(
            margin + (accent_width + spacing) * 4.0,
            y,
            accent_width,
            quad_height,
        ))
        .with_background(theme::accent::PURPLE),
    );
    y += quad_height + spacing;

    let status_width = (content_width - spacing * 3.0) / 4.0;
    scene.draw_quad(
        Quad::new(Bounds::new(margin, y, status_width, quad_height))
            .with_background(theme::status::SUCCESS),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(
            margin + status_width + spacing,
            y,
            status_width,
            quad_height,
        ))
        .with_background(theme::status::WARNING),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(
            margin + (status_width + spacing) * 2.0,
            y,
            status_width,
            quad_height,
        ))
        .with_background(theme::status::ERROR),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(
            margin + (status_width + spacing) * 3.0,
            y,
            status_width,
            quad_height,
        ))
        .with_background(theme::status::INFO),
    );

    scene
}
