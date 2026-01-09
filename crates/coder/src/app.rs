//! Main application state and event handling.

use std::sync::Arc;
use web_time::Instant;
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Hsla, Quad, Scene, Size, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

/// Render state holding all GPU and UI resources
struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    #[allow(dead_code)]
    last_tick: Instant,
}

/// Main application
#[derive(Default)]
pub struct CoderApp {
    state: Option<RenderState>,
}

impl ApplicationHandler for CoderApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Coder")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 600));

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
                last_tick: Instant::now(),
            }
        });

        let window_clone = state.window.clone();
        self.state = Some(state);
        tracing::info!("Window initialized");

        // Request initial redraw
        window_clone.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                state.config.width = size.width.max(1);
                state.config.height = size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            _ => {}
        }
    }
}

impl CoderApp {
    fn render(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        // Get surface texture
        let output = match state.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                state.surface.configure(&state.device, &state.config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                tracing::error!("Out of memory");
                return;
            }
            Err(_) => return,
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Build scene
        let mut scene = Scene::new();
        let bounds = Bounds::new(
            0.0,
            0.0,
            state.config.width as f32,
            state.config.height as f32,
        );

        // Dark terminal background (slightly visible gray for debugging)
        scene.draw_quad(Quad::new(bounds).with_background(Hsla::new(220.0, 0.15, 0.12, 1.0)));

        // Draw title - bright cyan
        let title_run = state.text_system.layout_styled_mono(
            "Coder",
            wgpui::Point::new(16.0, 16.0),
            20.0,
            Hsla::new(187.0, 0.9, 0.65, 1.0), // Bright cyan
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(title_run);

        // Draw subtitle - lighter gray
        let subtitle_run = state.text_system.layout_styled_mono(
            "Terminal-style Claude Code interface",
            wgpui::Point::new(16.0, 48.0),
            14.0,
            Hsla::new(0.0, 0.0, 0.6, 1.0), // Lighter gray
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(subtitle_run);

        // Draw hello world - bright green
        let hello_run = state.text_system.layout_styled_mono(
            "Hello, World! WGPUI is working.",
            wgpui::Point::new(16.0, 100.0),
            14.0,
            Hsla::new(120.0, 0.8, 0.6, 1.0), // Bright green
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(hello_run);

        // Render
        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Coder Render"),
            });

        let physical_width = state.config.width as f32;
        let physical_height = state.config.height as f32;
        let scale_factor = state.window.scale_factor() as f32;

        // Resize renderer to match window
        state.renderer.resize(
            &state.queue,
            Size::new(physical_width, physical_height),
            1.0,
        );

        // Update text atlas if needed
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
}
