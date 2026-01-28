use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use anyhow::{Context, Result};
use autopilot_app::{AppEvent, App as AutopilotApp, AppConfig, EventRecorder, UserAction};
use codex_client::{
    AppServerClient, AppServerConfig, AskForApproval, ClientInfo, SandboxMode, SandboxPolicy,
    ThreadStartParams, TurnStartParams, UserInput,
};
use autopilot_ui::MinimalRoot;
use futures::StreamExt;
use serde_json::json;
use tracing_subscriber::EnvFilter;
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, Cursor, InputEvent, Key, Modifiers, MouseButton, NamedKey, PaintContext,
    Point, Scene, Size, TextSystem,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::{CursorIcon, Window, WindowId};

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
    root: MinimalRoot,
    cursor_icon: Cursor,
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
                update_cursor(&mut state);
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
                let scale = state.scale_factor.max(0.1);
                self.cursor_position =
                    Point::new(position.x as f32 / scale, position.y as f32 / scale);
                let input_event = InputEvent::MouseMove {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y,
                };
                let bounds = content_bounds(logical_size(&state.config, state.scale_factor));
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
                update_cursor(state);
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

                let bounds = content_bounds(logical_size(&state.config, state.scale_factor));
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
                update_cursor(state);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (-x * 24.0, -y * 24.0),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        let scale = state.scale_factor.max(0.1);
                        (-pos.x as f32 / scale, -pos.y as f32 / scale)
                    }
                };
                let input_event = InputEvent::Scroll { dx, dy };
                let bounds = content_bounds(logical_size(&state.config, state.scale_factor));
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
                update_cursor(state);
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
                let bounds = content_bounds(logical_size(&state.config, state.scale_factor));
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
                update_cursor(state);
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

    let mut root = MinimalRoot::new();
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
            cursor_icon: Cursor::Default,
        })
    })
}

fn render_frame(state: &mut RenderState) -> Result<()> {
    let logical = logical_size(&state.config, state.scale_factor);
    let content_bounds = content_bounds(logical);

    let mut scene = Scene::new();
    let mut paint = PaintContext::new(&mut scene, &mut state.text_system, state.scale_factor);
    state.root.paint(content_bounds, &mut paint);

    state.renderer.resize(
        &state.queue,
        logical,
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

fn update_cursor(state: &mut RenderState) {
    let cursor = state.root.cursor();
    if cursor != state.cursor_icon {
        state.window.set_cursor(map_cursor_icon(cursor));
        state.cursor_icon = cursor;
    }
}

fn map_cursor_icon(cursor: Cursor) -> CursorIcon {
    match cursor {
        Cursor::Default => CursorIcon::Default,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
        Cursor::Grab => CursorIcon::Grab,
        Cursor::Grabbing => CursorIcon::Grabbing,
    }
}

fn window_bounds(size: Size) -> Bounds {
    Bounds::new(0.0, 0.0, size.width, size.height)
}

fn content_bounds(size: Size) -> Bounds {
    inset_bounds(window_bounds(size), PADDING)
}

fn logical_size(config: &wgpu::SurfaceConfiguration, scale_factor: f32) -> Size {
    let scale = scale_factor.max(0.1);
    Size::new(config.width as f32 / scale, config.height as f32 / scale)
}

fn inset_bounds(bounds: Bounds, padding: f32) -> Bounds {
    let width = (bounds.size.width - padding * 2.0).max(0.0);
    let height = (bounds.size.height - padding * 2.0).max(0.0);
    Bounds::new(bounds.origin.x + padding, bounds.origin.y + padding, width, height)
}

fn spawn_event_bridge(proxy: EventLoopProxy<AppEvent>, action_rx: mpsc::Receiver<UserAction>) {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime init failed");
        runtime.block_on(async move {
            let app = AutopilotApp::new(AppConfig {
                event_buffer: EVENT_BUFFER,
            });

            let mut recorder = std::env::var("AUTOPILOT_REPLAY_PATH")
                .ok()
                .and_then(|path| EventRecorder::create(path).ok());

            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let cwd_string = cwd.to_string_lossy().to_string();
            let workspace = Arc::new(app.open_workspace(cwd.clone()));
            let mut stream = workspace.events();

            let _session = workspace.start_session(Some("Bootstrap".to_string()));

            let proxy_events = proxy.clone();
            tokio::spawn(async move {
                while let Some(event) = stream.next().await {
                    let _ = proxy_events.send_event(event.clone());
                    if let Some(writer) = recorder.as_mut() {
                        if let Err(err) = writer.record_event(&event) {
                            tracing::warn!(error = %err, "failed to record replay event");
                        }
                    }
                }
            });

            let (client, channels) = match AppServerClient::spawn(AppServerConfig {
                cwd: Some(cwd.clone()),
                wire_log: None,
                env: Vec::new(),
            })
            .await
            {
                Ok(result) => result,
                Err(err) => {
                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                        message: json!({
                            "method": "codex/error",
                            "params": { "message": err.to_string() }
                        })
                        .to_string(),
                    });
                    futures::future::pending::<()>().await;
                    return;
                }
            };

            let client = Arc::new(client);
            let thread_id = Arc::new(tokio::sync::Mutex::new(None::<String>));

            let client_info = ClientInfo {
                name: "autopilot-desktop-wgpu".to_string(),
                title: Some("Autopilot Desktop".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            };
            if let Err(err) = client.initialize(client_info).await {
                let _ = proxy.send_event(AppEvent::AppServerEvent {
                    message: json!({
                        "method": "codex/error",
                        "params": { "message": err.to_string() }
                    })
                    .to_string(),
                });
            }

            match client
                .thread_start(ThreadStartParams {
                    model: None,
                    model_provider: None,
                    cwd: Some(cwd_string.clone()),
                    approval_policy: Some(AskForApproval::Never),
                    sandbox: Some(SandboxMode::WorkspaceWrite),
                })
                .await
            {
                Ok(response) => {
                    let mut guard = thread_id.lock().await;
                    *guard = Some(response.thread.id.clone());
                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                        message: json!({
                            "method": "thread/started",
                            "params": { "threadId": response.thread.id }
                        })
                        .to_string(),
                    });
                }
                Err(err) => {
                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                        message: json!({
                            "method": "codex/error",
                            "params": { "message": err.to_string() }
                        })
                        .to_string(),
                    });
                }
            }

            let proxy_notifications = proxy.clone();
            tokio::spawn(async move {
                let mut notification_rx = channels.notifications;
                while let Some(notification) = notification_rx.recv().await {
                    let payload = json!({
                        "method": notification.method,
                        "params": notification.params,
                    });
                    let _ = proxy_notifications.send_event(AppEvent::AppServerEvent {
                        message: payload.to_string(),
                    });
                }
            });

            let client_requests = client.clone();
            let proxy_requests = proxy.clone();
            tokio::spawn(async move {
                let mut request_rx = channels.requests;
                while let Some(request) = request_rx.recv().await {
                    let payload = json!({
                        "id": request.id,
                        "method": request.method,
                        "params": request.params,
                    });
                    let _ = proxy_requests.send_event(AppEvent::AppServerEvent {
                        message: payload.to_string(),
                    });

                    let response = match request.method.as_str() {
                        "execCommandApproval" | "applyPatchApproval" => {
                            json!({ "decision": "approved" })
                        }
                        _ => json!({}),
                    };
                    let _ = client_requests.respond(request.id, &response).await;
                }
            });

            let handle = tokio::runtime::Handle::current();
            let workspace_for_actions = workspace.clone();
            let client_for_actions = client.clone();
            let thread_id_for_actions = thread_id.clone();
            let proxy_actions = proxy.clone();
            let cwd_for_actions = cwd_string.clone();
            tokio::task::spawn_blocking(move || {
                while let Ok(action) = action_rx.recv() {
                    workspace_for_actions.dispatch(action.clone());
                    if let UserAction::Message { text, .. } = action {
                        let client = client_for_actions.clone();
                        let thread_id = thread_id_for_actions.clone();
                        let proxy = proxy_actions.clone();
                        let cwd = cwd_for_actions.clone();
                        handle.spawn(async move {
                            let thread_id = thread_id.lock().await.clone();
                            let Some(thread_id) = thread_id else {
                                let _ = proxy.send_event(AppEvent::AppServerEvent {
                                    message: json!({
                                        "method": "codex/error",
                                        "params": { "message": "Codex thread not ready" }
                                    })
                                    .to_string(),
                                });
                                return;
                            };

                            let params = TurnStartParams {
                                thread_id,
                                input: vec![UserInput::Text { text }],
                                model: None,
                                effort: None,
                                summary: None,
                                approval_policy: Some(AskForApproval::Never),
                                sandbox_policy: Some(SandboxPolicy::WorkspaceWrite {
                                    writable_roots: vec![cwd.clone()],
                                    network_access: true,
                                    exclude_tmpdir_env_var: false,
                                    exclude_slash_tmp: false,
                                }),
                                cwd: Some(cwd),
                            };

                            if let Err(err) = client.turn_start(params).await {
                                let _ = proxy.send_event(AppEvent::AppServerEvent {
                                    message: json!({
                                        "method": "codex/error",
                                        "params": { "message": err.to_string() }
                                    })
                                    .to_string(),
                                });
                            }
                        });
                    }
                }
            });

            futures::future::pending::<()>().await;
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
