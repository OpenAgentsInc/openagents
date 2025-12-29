//! WGPUI wallet GUI application.

use std::sync::Arc;

use anyhow::Result;
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, EventContext, EventResult, InputEvent, Key, Modifiers, NamedKey,
    PaintContext, Point, Scene, Size, TextSystem,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::{Window, WindowId};

use super::backend::{start_backend, WalletBackendHandle};
use super::types::{WalletCommand, WalletUpdate};
use super::view::WalletView;

pub fn run_gui() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let backend = start_backend(runtime.handle().clone());

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = WalletAppHandler::new(backend);
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct WalletAppHandler {
    backend: Option<WalletBackendHandle>,
    state: Option<RenderState>,
    modifiers: ModifiersState,
    cursor_position: Point,
}

impl WalletAppHandler {
    fn new(backend: WalletBackendHandle) -> Self {
        Self {
            backend: Some(backend),
            state: None,
            modifiers: ModifiersState::default(),
            cursor_position: Point::ZERO,
        }
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
    ui: WalletUi,
}

impl ApplicationHandler for WalletAppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("OpenAgents Wallet")
            .with_inner_size(winit::dpi::LogicalSize::new(1100, 740));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("failed to create window"),
        );

        let backend = self
            .backend
            .take()
            .expect("wallet backend already initialized");
        let state = pollster::block_on(init_render_state(window, backend));
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
                state.ui.set_scale_factor(scale_factor as f32);
                state.text_system.set_scale_factor(scale_factor as f32);
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::RedrawRequested => {
                let bounds = window_bounds(&state.config);
                let mut scene = Scene::new();
                state.ui.paint(bounds, &mut scene, &mut state.text_system);

                state.renderer.resize(
                    &state.queue,
                    Size::new(bounds.size.width, bounds.size.height),
                    state.ui.scale_factor(),
                );

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
                    Err(_) => return,
                };

                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = state
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Wallet Render Encoder"),
                    });

                state.renderer.prepare(&state.device, &state.queue, &scene, state.ui.scale_factor());
                state.renderer.render(&mut encoder, &view);
                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Point::new(position.x as f32, position.y as f32);
                let input_event = InputEvent::MouseMove {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y,
                };
                let bounds = window_bounds(&state.config);
                if state.ui.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state: mouse_state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => wgpui::MouseButton::Left,
                    winit::event::MouseButton::Right => wgpui::MouseButton::Right,
                    winit::event::MouseButton::Middle => wgpui::MouseButton::Middle,
                    _ => return,
                };

                let input_event = match mouse_state {
                    ElementState::Pressed => InputEvent::MouseDown {
                        button,
                        x: self.cursor_position.x,
                        y: self.cursor_position.y,
                    },
                    ElementState::Released => InputEvent::MouseUp {
                        button,
                        x: self.cursor_position.x,
                        y: self.cursor_position.y,
                    },
                };

                let bounds = window_bounds(&state.config);
                if state.ui.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (-x * 24.0, -y * 24.0),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => (-pos.x as f32, -pos.y as f32),
                };
                let input_event = InputEvent::Scroll { dx, dy };
                let bounds = window_bounds(&state.config);
                if state.ui.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let Some(key) = map_key(&event.logical_key) else {
                    return;
                };

                let modifiers = to_modifiers(self.modifiers);
                let input_event = match event.state {
                    ElementState::Pressed => InputEvent::KeyDown { key, modifiers },
                    ElementState::Released => InputEvent::KeyUp { key, modifiers },
                };

                let bounds = window_bounds(&state.config);
                if state.ui.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            let mut redraw = state.ui.apply_backend_events();
            redraw |= state.ui.flush_commands();
            if redraw {
                state.window.request_redraw();
            }
        }
    }
}

async fn init_render_state(window: Arc<Window>, backend: WalletBackendHandle) -> RenderState {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let surface = instance
        .create_surface(window.clone())
        .expect("failed to create surface");

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("no compatible adapter found");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .await
        .expect("failed to create device");

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
    let scale_factor = window.scale_factor() as f32;
    let text_system = TextSystem::new(scale_factor);
    let (command_tx, update_rx) = backend.split();

    RenderState {
        window,
        surface,
        device,
        queue,
        config,
        renderer,
        text_system,
        ui: WalletUi::new(scale_factor, command_tx, update_rx),
    }
}

fn map_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Named(named) => match named {
            WinitNamedKey::Enter => Some(Key::Named(NamedKey::Enter)),
            WinitNamedKey::Escape => Some(Key::Named(NamedKey::Escape)),
            WinitNamedKey::Backspace => Some(Key::Named(NamedKey::Backspace)),
            WinitNamedKey::Delete => Some(Key::Named(NamedKey::Delete)),
            WinitNamedKey::Tab => Some(Key::Named(NamedKey::Tab)),
            WinitNamedKey::Home => Some(Key::Named(NamedKey::Home)),
            WinitNamedKey::End => Some(Key::Named(NamedKey::End)),
            WinitNamedKey::ArrowUp => Some(Key::Named(NamedKey::ArrowUp)),
            WinitNamedKey::ArrowDown => Some(Key::Named(NamedKey::ArrowDown)),
            WinitNamedKey::ArrowLeft => Some(Key::Named(NamedKey::ArrowLeft)),
            WinitNamedKey::ArrowRight => Some(Key::Named(NamedKey::ArrowRight)),
            _ => None,
        },
        WinitKey::Character(ch) => Some(Key::Character(ch.to_string())),
        _ => None,
    }
}

fn to_modifiers(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        shift: modifiers.shift_key(),
        ctrl: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    }
}

fn window_bounds(config: &wgpu::SurfaceConfiguration) -> Bounds {
    Bounds::new(0.0, 0.0, config.width as f32, config.height as f32)
}

struct WalletUi {
    view: WalletView,
    event_context: EventContext,
    scale_factor: f32,
    command_tx: tokio::sync::mpsc::UnboundedSender<WalletCommand>,
    update_rx: tokio::sync::mpsc::UnboundedReceiver<WalletUpdate>,
}

impl WalletUi {
    fn new(
        scale_factor: f32,
        command_tx: tokio::sync::mpsc::UnboundedSender<WalletCommand>,
        update_rx: tokio::sync::mpsc::UnboundedReceiver<WalletUpdate>,
    ) -> Self {
        Self {
            view: WalletView::new(),
            event_context: EventContext::new(),
            scale_factor,
            command_tx,
            update_rx,
        }
    }

    fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn paint(&mut self, bounds: Bounds, scene: &mut Scene, text: &mut TextSystem) {
        let mut cx = PaintContext::new(scene, text, self.scale_factor);
        self.view.paint(bounds, &mut cx);
    }

    fn handle_input(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let result = self
            .view
            .event(event, bounds, &mut self.event_context);
        self.flush_commands();
        matches!(result, EventResult::Handled)
    }

    fn apply_backend_events(&mut self) -> bool {
        let mut updated = false;
        while let Ok(update) = self.update_rx.try_recv() {
            self.view.apply_update(update);
            updated = true;
        }
        updated
    }

    fn flush_commands(&mut self) -> bool {
        let commands = self.view.drain_commands();
        if commands.is_empty() {
            return false;
        }
        for command in commands {
            let _ = self.command_tx.send(command);
        }
        true
    }
}
