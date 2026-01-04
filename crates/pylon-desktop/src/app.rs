//! Application handler with winit event loop

use std::sync::Arc;
use web_time::Instant;
use wgpui::renderer::Renderer;
use wgpui::{Scene, Size, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::bridge_manager::{BridgeManager, BridgeStatus};
use crate::fm_runtime::{FmEvent, FmRuntime};
use crate::state::{FmConnectionStatus, FmVizState};
use crate::ui;

#[derive(Default)]
pub struct PylonApp {
    state: Option<RenderState>,
}

pub struct RenderState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    pub text_system: TextSystem,
    pub fm_state: FmVizState,
    pub fm_runtime: FmRuntime,
    pub bridge: BridgeManager,
    pub last_tick: Instant,
}

impl ApplicationHandler for PylonApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Pylon - FM Bridge")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 700));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        // Start FM Bridge process first
        let mut bridge = BridgeManager::new();
        let mut fm_state = FmVizState::new();

        // Try to start the bridge
        match bridge.start() {
            Ok(()) => {
                fm_state.bridge_status_message = Some("Starting FM Bridge...".to_string());

                // Wait for it to be ready
                match bridge.wait_ready() {
                    Ok(()) => {
                        // Set the URL for FMClient
                        // SAFETY: We're in single-threaded init before any other threads start
                        unsafe { std::env::set_var("FM_BRIDGE_URL", bridge.url()) };
                        fm_state.bridge_url = bridge.url().replace("http://", "");
                        fm_state.bridge_status_message = Some("FM Bridge running".to_string());
                    }
                    Err(e) => {
                        fm_state.connection_status = FmConnectionStatus::Error;
                        fm_state.bridge_status_message = Some(format!("Bridge startup failed: {}", e));
                        fm_state.error_message = Some(e.to_string());
                    }
                }
            }
            Err(e) => {
                fm_state.connection_status = FmConnectionStatus::Error;
                fm_state.bridge_status_message = Some(format!("Bridge not found: {}", e));
                fm_state.error_message = Some(e.to_string());
            }
        }

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

            // Create FM runtime and request initial connection (only if bridge is running)
            let fm_runtime = FmRuntime::new();
            if bridge.status == BridgeStatus::Running {
                fm_runtime.connect();
                fm_state.connection_status = FmConnectionStatus::Connecting;
            }

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                fm_state,
                fm_runtime,
                bridge,
                last_tick: Instant::now(),
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
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    match &event.logical_key {
                        Key::Named(NamedKey::Enter) => {
                            // Send prompt if we can
                            if state.fm_state.can_send() {
                                let prompt = state.fm_state.prompt_input.clone();
                                state.fm_state.on_stream_start(&prompt);
                                state.fm_runtime.stream(prompt);
                            }
                        }
                        Key::Named(NamedKey::Backspace) => {
                            state.fm_state.prompt_input.pop();
                        }
                        Key::Character(c) => {
                            // Only accept input when not streaming
                            if !state.fm_state.is_streaming() {
                                state.fm_state.prompt_input.push_str(c);
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;

                // Update timing
                state.last_tick = Instant::now();

                // Build scene
                let mut scene = Scene::new();
                ui::build_pylon_ui(
                    &mut scene,
                    &mut state.text_system,
                    &mut state.fm_state,
                    width,
                    height,
                );

                // Render
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

                state
                    .renderer
                    .resize(&state.queue, Size::new(width, height), 1.0);

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                let scale_factor = state.window.scale_factor() as f32;
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
        if let Some(state) = &mut self.state {
            // Poll FM events (non-blocking)
            while let Ok(event) = state.fm_runtime.event_rx.try_recv() {
                match event {
                    FmEvent::Connected { model_available, latency_ms } => {
                        state.fm_state.on_connected(model_available, latency_ms);
                    }
                    FmEvent::ConnectionFailed(error) => {
                        state.fm_state.on_connection_failed(error);
                    }
                    FmEvent::FirstToken { text, ttft_ms } => {
                        state.fm_state.on_first_token(&text, ttft_ms);
                    }
                    FmEvent::Token { text } => {
                        state.fm_state.on_token(&text);
                    }
                    FmEvent::StreamComplete => {
                        state.fm_state.on_stream_complete();
                    }
                    FmEvent::StreamError(error) => {
                        state.fm_state.on_stream_error(error);
                    }
                }
            }

            state.window.request_redraw();
        }
    }
}
