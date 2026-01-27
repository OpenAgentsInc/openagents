use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use anyhow::{Context, Result};
use autopilot_app::{AppEvent, App as AutopilotApp, AppConfig, UserAction};
use autopilot_ui::DesktopRoot;
use futures::StreamExt;
use tracing_subscriber::EnvFilter;
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, InputEvent, Key, Modifiers, MouseButton, NamedKey, PaintContext, Point,
    Scene, Size, TextSystem,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::{Window, WindowId};

const WINDOW_TITLE: &str = "Autopilot Desktop (WGPUI)";
const WINDOW_WIDTH: f64 = 1280.0;
const WINDOW_HEIGHT: f64 = 800.0;
const PADDING: f32 = 16.0;
const EVENT_BUFFER: usize = 256;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let event_loop = EventLoop::<AppEvent>::with_user_event()
        .build()
        .context("failed to create event loop")?;
    let proxy = event_loop.create_proxy();
    let (action_tx, action_rx) = mpsc::channel();
    spawn_event_bridge(proxy, action_rx);
    let mut app = App::new(action_tx);
    event_loop
        .run_app(&mut app)
        .context("event loop failed")?;
    Ok(())
}

struct App {
    state: Option<RenderState>,
    pending_events: Vec<AppEvent>,
    action_tx: mpsc::Sender<UserAction>,
    cursor_position: Point,
    modifiers: ModifiersState,
}

impl App {
    fn new(action_tx: mpsc::Sender<UserAction>) -> Self {
        Self {
            state: None,
            pending_events: Vec::new(),
            action_tx,
            cursor_position: Point::ZERO,
            modifiers: ModifiersState::default(),
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
    scale_factor: f32,
    root: DesktopRoot,
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let action_tx = self.action_tx.clone();
        match init_state(event_loop, action_tx) {
            Ok(mut state) => {
                for event in self.pending_events.drain(..) {
                    state.root.apply_event(event);
                }
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to initialize WGPUI window");
                event_loop.exit();
            }
        }
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
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Point::new(position.x as f32, position.y as f32);
                let input_event = InputEvent::MouseMove {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y,
                };
                let bounds = content_bounds(&state.config);
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button,
                ..
            } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };

                let modifiers = to_modifiers(self.modifiers);
                let input_event = match mouse_state {
                    ElementState::Pressed => InputEvent::MouseDown {
                        button,
                        x: self.cursor_position.x,
                        y: self.cursor_position.y,
                        modifiers,
                    },
                    ElementState::Released => InputEvent::MouseUp {
                        button,
                        x: self.cursor_position.x,
                        y: self.cursor_position.y,
                    },
                };

                let bounds = content_bounds(&state.config);
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (-x * 24.0, -y * 24.0),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        (-pos.x as f32, -pos.y as f32)
                    }
                };
                let input_event = InputEvent::Scroll { dx, dy };
                let bounds = content_bounds(&state.config);
                if state.root.handle_input(&input_event, bounds) {
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
                let bounds = content_bounds(&state.config);
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = render_frame(state) {
                    tracing::warn!(error = %err, "render frame failed");
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        if let Some(state) = &mut self.state {
            state.root.apply_event(event);
            state.window.request_redraw();
        } else {
            self.pending_events.push(event);
        }
    }
}

fn init_state(
    event_loop: &ActiveEventLoop,
    action_tx: mpsc::Sender<UserAction>,
) -> Result<RenderState> {
    let window_attrs = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));

    let window = Arc::new(
        event_loop
            .create_window(window_attrs)
            .context("failed to create window")?,
    );

    let mut root = DesktopRoot::new();
    root.set_send_handler(move |action| {
        let _ = action_tx.send(action);
    });

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
            .context("failed to find a compatible adapter")?;

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

        Ok(RenderState {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            text_system,
            scale_factor,
            root,
        })
    })
}

fn render_frame(state: &mut RenderState) -> Result<()> {
    let bounds = window_bounds(&state.config);
    let content_bounds = content_bounds(&state.config);

    let mut scene = Scene::new();
    let mut paint = PaintContext::new(&mut scene, &mut state.text_system, state.scale_factor);
    state.root.paint(content_bounds, &mut paint);

    state.renderer.resize(
        &state.queue,
        Size::new(bounds.size.width, bounds.size.height),
        state.scale_factor,
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
            return Ok(());
        }
        Err(err) => {
            return Err(anyhow::anyhow!("surface error: {err:?}"));
        }
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

    let scale_factor = state.window.scale_factor() as f32;
    state
        .renderer
        .prepare(&state.device, &state.queue, &scene, scale_factor);
    state.renderer.render(&mut encoder, &view);
    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
}

fn window_bounds(config: &wgpu::SurfaceConfiguration) -> Bounds {
    Bounds::new(0.0, 0.0, config.width as f32, config.height as f32)
}

fn content_bounds(config: &wgpu::SurfaceConfiguration) -> Bounds {
    inset_bounds(window_bounds(config), PADDING)
}

fn inset_bounds(bounds: Bounds, padding: f32) -> Bounds {
    let width = (bounds.size.width - padding * 2.0).max(0.0);
    let height = (bounds.size.height - padding * 2.0).max(0.0);
    Bounds::new(bounds.origin.x + padding, bounds.origin.y + padding, width, height)
}

fn spawn_event_bridge(proxy: EventLoopProxy<AppEvent>, action_rx: mpsc::Receiver<UserAction>) {
    std::thread::spawn(move || {
        let app = AutopilotApp::new(AppConfig {
            event_buffer: EVENT_BUFFER,
        });

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let workspace = Arc::new(app.open_workspace(cwd));
        let mut stream = workspace.events();

        let session = workspace.start_session(Some("Bootstrap".to_string()));
        workspace.dispatch(UserAction::Message {
            session_id: session.session_id(),
            text: "Autopilot desktop WGPUI bootstrapped.".to_string(),
        });

        let workspace_for_actions = workspace.clone();
        std::thread::spawn(move || {
            while let Ok(action) = action_rx.recv() {
                workspace_for_actions.dispatch(action);
            }
        });

        futures::executor::block_on(async move {
            while let Some(event) = stream.next().await {
                let _ = proxy.send_event(event);
            }
        });
    });
}

fn map_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Named(named) => match named {
            WinitNamedKey::Enter => Some(Key::Named(NamedKey::Enter)),
            WinitNamedKey::Escape => Some(Key::Named(NamedKey::Escape)),
            WinitNamedKey::Backspace => Some(Key::Named(NamedKey::Backspace)),
            WinitNamedKey::Delete => Some(Key::Named(NamedKey::Delete)),
            WinitNamedKey::Tab => Some(Key::Named(NamedKey::Tab)),
            WinitNamedKey::Space => Some(Key::Named(NamedKey::Space)),
            WinitNamedKey::Home => Some(Key::Named(NamedKey::Home)),
            WinitNamedKey::End => Some(Key::Named(NamedKey::End)),
            WinitNamedKey::PageUp => Some(Key::Named(NamedKey::PageUp)),
            WinitNamedKey::PageDown => Some(Key::Named(NamedKey::PageDown)),
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

// DesktopRoot moved to `crates/autopilot_ui`.
