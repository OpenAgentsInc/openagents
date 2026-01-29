use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use anyhow::{Context, Result};
use autopilot_app::{
    App as AutopilotApp, AppConfig, AppEvent, DvmHistorySnapshot, DvmProviderStatus, EventRecorder,
    PylonStatus, SessionId, UserAction, WalletStatus,
};
use autopilot_core::guidance::{GuidanceMode, ensure_guidance_demo_lm, run_guidance_decision};
use autopilot_ui::MinimalRoot;
use codex_client::{
    AppServerClient, AppServerConfig, AskForApproval, ClientInfo, ReasoningEffort, SandboxMode,
    SandboxPolicy, ThreadListParams, ThreadResumeParams, ThreadStartParams, TurnInterruptParams,
    TurnStartParams, UserInput,
};
use dsrs::signatures::{
    GuidanceDecisionSignature, GuidanceRouterSignature, PlanningSignature,
    TaskUnderstandingSignature,
};
use dsrs::{Predict, Predictor, example};
use full_auto::{
    FullAutoAction, FullAutoDecisionRequest, FullAutoDecisionResult, FullAutoState, decision_model,
    ensure_codex_lm, run_full_auto_decision,
};
use futures::StreamExt;
use nostr::nip90::{JobInput, JobRequest, KIND_JOB_TEXT_GENERATION};
use nostr_client::dvm::DvmClient;
use openagents_runtime::UnifiedIdentity;
use openagents_spark::{Network as SparkNetwork, SparkSigner, SparkWallet, WalletConfig};
use pylon::PylonConfig;
use pylon::db::{PylonDb, jobs::JobStatus};
use pylon::provider::{ProviderError, PylonProvider};
use serde_json::{Value, json};
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

mod full_auto;

const WINDOW_TITLE: &str = "Autopilot";
const WINDOW_WIDTH: f64 = 1280.0;
const WINDOW_HEIGHT: f64 = 800.0;
const PADDING: f32 = 0.0;
const EVENT_BUFFER: usize = 256;
const DEFAULT_THREAD_MODEL: &str = "gpt-5.1-codex-mini";
const ENV_GUIDANCE_GOAL: &str = "OPENAGENTS_GUIDANCE_GOAL";
const DEFAULT_GUIDANCE_GOAL_INTENT: &str =
    "Keep making progress on the current task using the latest plan and diff.";
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 2.5;
const ZOOM_STEP_KEY: f32 = 0.1;
const ZOOM_STEP_WHEEL: f32 = 0.05;

fn parse_reasoning_effort(value: &str) -> Option<ReasoningEffort> {
    match value.trim().to_lowercase().as_str() {
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "xhigh" | "x-high" => Some(ReasoningEffort::XHigh),
        "minimal" => Some(ReasoningEffort::Minimal),
        "none" => Some(ReasoningEffort::None),
        _ => None,
    }
}

#[derive(Clone)]
struct SessionRuntime {
    thread_id: Arc<tokio::sync::Mutex<Option<String>>>,
    turn_id: Arc<tokio::sync::Mutex<Option<String>>>,
    pending_interrupt: Arc<AtomicBool>,
}

impl SessionRuntime {
    fn new() -> Self {
        Self {
            thread_id: Arc::new(tokio::sync::Mutex::new(None)),
            turn_id: Arc::new(tokio::sync::Mutex::new(None)),
            pending_interrupt: Arc::new(AtomicBool::new(false)),
        }
    }
}

struct InProcessPylon {
    provider: Option<PylonProvider>,
    started_at: Option<std::time::Instant>,
    last_error: Option<String>,
}

impl InProcessPylon {
    fn new() -> Self {
        Self {
            provider: None,
            started_at: None,
            last_error: None,
        }
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();
    let _ = rustls::crypto::ring::default_provider().install_default();

    let event_loop = EventLoop::<AppEvent>::with_user_event()
        .build()
        .context("failed to create event loop")?;
    let proxy = event_loop.create_proxy();
    let (action_tx, action_rx) = mpsc::channel();
    spawn_event_bridge(proxy, action_rx);
    let mut app = App::new(action_tx);
    event_loop.run_app(&mut app).context("event loop failed")?;
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
    zoom_factor: f32,
    root: MinimalRoot,
    cursor_icon: Cursor,
}

impl RenderState {
    fn effective_scale(&self) -> f32 {
        (self.scale_factor * self.zoom_factor).max(0.1)
    }

    fn bump_zoom(&mut self, delta: f32) {
        let next = (self.zoom_factor + delta).clamp(ZOOM_MIN, ZOOM_MAX);
        if (next - self.zoom_factor).abs() > f32::EPSILON {
            self.zoom_factor = next;
            self.text_system.set_scale_factor(self.effective_scale());
        }
    }

    fn set_zoom(&mut self, zoom: f32) {
        let next = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        if (next - self.zoom_factor).abs() > f32::EPSILON {
            self.zoom_factor = next;
            self.text_system.set_scale_factor(self.effective_scale());
        }
    }
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
                state.text_system.set_scale_factor(state.effective_scale());
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = state.effective_scale();
                self.cursor_position =
                    Point::new(position.x as f32 / scale, position.y as f32 / scale);
                let input_event = InputEvent::MouseMove {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y,
                };
                let bounds = content_bounds(logical_size(&state.config, state.effective_scale()));
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

                let bounds = content_bounds(logical_size(&state.config, state.effective_scale()));
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
                update_cursor(state);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy, zoom_dir) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (-x * 24.0, -y * 24.0, y),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        let scale = state.effective_scale();
                        (-pos.x as f32 / scale, -pos.y as f32 / scale, pos.y as f32)
                    }
                };

                let modifiers = to_modifiers(self.modifiers);
                let mut handled = false;

                if modifiers.meta {
                    let step = if zoom_dir > 0.0 {
                        ZOOM_STEP_WHEEL
                    } else if zoom_dir < 0.0 {
                        -ZOOM_STEP_WHEEL
                    } else {
                        0.0
                    };
                    if step != 0.0 {
                        state.bump_zoom(step);
                        state.window.request_redraw();
                        handled = true;
                    }
                }

                if !handled {
                    let input_event = InputEvent::Scroll { dx, dy };
                    let bounds =
                        content_bounds(logical_size(&state.config, state.effective_scale()));
                    if state.root.handle_input(&input_event, bounds) {
                        state.window.request_redraw();
                        handled = true;
                    }
                }

                if !handled {
                    let step = if zoom_dir > 0.0 {
                        ZOOM_STEP_WHEEL
                    } else if zoom_dir < 0.0 {
                        -ZOOM_STEP_WHEEL
                    } else {
                        0.0
                    };
                    if step != 0.0 {
                        state.bump_zoom(step);
                        state.window.request_redraw();
                    }
                }

                update_cursor(state);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && self.modifiers.super_key() {
                    if let WinitKey::Character(ch) = &event.logical_key {
                        match ch.as_str() {
                            "+" | "=" => {
                                state.bump_zoom(ZOOM_STEP_KEY);
                                state.window.request_redraw();
                                return;
                            }
                            "-" => {
                                state.bump_zoom(-ZOOM_STEP_KEY);
                                state.window.request_redraw();
                                return;
                            }
                            "0" => {
                                state.set_zoom(1.0);
                                state.window.request_redraw();
                                return;
                            }
                            _ => {}
                        }
                    }
                }
                let Some(key) = map_key(&event.logical_key) else {
                    return;
                };
                let modifiers = to_modifiers(self.modifiers);
                let input_event = match event.state {
                    ElementState::Pressed => InputEvent::KeyDown { key, modifiers },
                    ElementState::Released => InputEvent::KeyUp { key, modifiers },
                };
                let bounds = content_bounds(logical_size(&state.config, state.effective_scale()));
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
                update_cursor(state);
            }
            WindowEvent::RedrawRequested => {
                let continue_redraw = state.root.needs_redraw();
                if let Err(err) = render_frame(state) {
                    tracing::warn!(error = %err, "render frame failed");
                }
                if continue_redraw {
                    state.window.request_redraw();
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
            zoom_factor: 1.0,
            root,
            cursor_icon: Cursor::Default,
        })
    })
}

fn render_frame(state: &mut RenderState) -> Result<()> {
    let scale_factor = state.effective_scale();
    let logical = logical_size(&state.config, scale_factor);
    let content_bounds = content_bounds(logical);

    let mut scene = Scene::new();
    let mut paint = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
    state.root.set_zoom_factor(state.zoom_factor);
    state.root.paint(content_bounds, &mut paint);

    state.renderer.resize(&state.queue, logical, scale_factor);

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
        Cursor::ResizeNs => CursorIcon::NsResize,
        Cursor::ResizeEw => CursorIcon::EwResize,
        Cursor::ResizeNesw => CursorIcon::NeswResize,
        Cursor::ResizeNwse => CursorIcon::NwseResize,
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
    Bounds::new(
        bounds.origin.x + padding,
        bounds.origin.y + padding,
        width,
        height,
    )
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
            let workspace_id = workspace.workspace_id().to_string();
            let mut stream = workspace.events();
            let session_states = Arc::new(tokio::sync::Mutex::new(HashMap::<
                SessionId,
                SessionRuntime,
            >::new()));
            let thread_to_session =
                Arc::new(tokio::sync::Mutex::new(HashMap::<String, SessionId>::new()));

            let bootstrap_session = workspace.start_session(Some("Bootstrap".to_string()));
            let bootstrap_id = bootstrap_session.session_id();
            let bootstrap_state = SessionRuntime::new();
            {
                let mut guard = session_states.lock().await;
                guard.insert(bootstrap_id, bootstrap_state.clone());
            }

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
            let full_auto_state = Arc::new(tokio::sync::Mutex::new(None::<FullAutoState>));

            let client_info = ClientInfo {
                name: "autopilot-desktop".to_string(),
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
                    model: Some(DEFAULT_THREAD_MODEL.to_string()),
                    model_provider: None,
                    cwd: Some(cwd_string.clone()),
                    approval_policy: Some(AskForApproval::Never),
                    sandbox: Some(SandboxMode::WorkspaceWrite),
                })
                .await
            {
                Ok(response) => {
                    let mut guard = bootstrap_state.thread_id.lock().await;
                    *guard = Some(response.thread.id.clone());
                    {
                        let mut map = thread_to_session.lock().await;
                        map.insert(response.thread.id.clone(), bootstrap_id);
                    }
                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                        message: json!({
                            "method": "thread/started",
                            "params": {
                                "threadId": response.thread.id,
                                "model": DEFAULT_THREAD_MODEL,
                                "sessionId": bootstrap_id.to_string()
                            }
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
            let client_interrupt = client.clone();
            let session_states_notifications = session_states.clone();
            let thread_to_session_notifications = thread_to_session.clone();
            let full_auto_state_notifications = full_auto_state.clone();
            let proxy_full_auto = proxy.clone();
            let client_full_auto = client.clone();
            let cwd_full_auto = cwd_string.clone();
            let workspace_id_full_auto = workspace_id.clone();
            tokio::spawn(async move {
                let mut notification_rx = channels.notifications;
                while let Some(notification) = notification_rx.recv().await {
                    let params = notification.params.as_ref();
                    let thread_id_value = extract_thread_id(params);
                    let turn_id_value = extract_turn_id(params);
                    if notification.method == "turn/started" {
                        if let Some(params) = notification.params.as_ref() {
                            let next_turn = params
                                .get("turnId")
                                .and_then(|id| id.as_str())
                                .or_else(|| {
                                    params
                                        .get("turn")
                                        .and_then(|turn| turn.get("id"))
                                        .and_then(|id| id.as_str())
                                })
                                .map(|id| id.to_string());
                            if let Some(next_turn) = next_turn {
                                let session_id = if let Some(thread_id) = thread_id_value.as_deref()
                                {
                                    thread_to_session_notifications
                                        .lock()
                                        .await
                                        .get(thread_id)
                                        .copied()
                                } else {
                                    None
                                };
                                if let Some(session_id) = session_id {
                                    let state = session_states_notifications
                                        .lock()
                                        .await
                                        .get(&session_id)
                                        .cloned();
                                    if let Some(state) = state {
                                        {
                                            let mut guard = state.turn_id.lock().await;
                                            *guard = Some(next_turn.clone());
                                        }
                                        if state.pending_interrupt.swap(false, Ordering::SeqCst) {
                                            if let Some(thread_id) =
                                                state.thread_id.lock().await.clone()
                                            {
                                                let _ = client_interrupt
                                                    .turn_interrupt(TurnInterruptParams {
                                                        thread_id,
                                                        turn_id: next_turn.clone(),
                                                    })
                                                    .await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if notification.method == "turn/completed" {
                        let completed_turn = notification
                            .params
                            .as_ref()
                            .and_then(|params| params.get("turnId"))
                            .and_then(|id| id.as_str())
                            .or_else(|| {
                                notification
                                    .params
                                    .as_ref()
                                    .and_then(|params| params.get("turn"))
                                    .and_then(|turn| turn.get("id"))
                                    .and_then(|id| id.as_str())
                            })
                            .map(|id| id.to_string());
                        let session_id = if let Some(thread_id) = thread_id_value.as_deref() {
                            thread_to_session_notifications
                                .lock()
                                .await
                                .get(thread_id)
                                .copied()
                        } else {
                            None
                        };
                        if let Some(session_id) = session_id {
                            let state = session_states_notifications
                                .lock()
                                .await
                                .get(&session_id)
                                .cloned();
                            if let Some(state) = state {
                                let mut guard = state.turn_id.lock().await;
                                if completed_turn
                                    .as_deref()
                                    .map(|id| guard.as_deref() == Some(id))
                                    .unwrap_or(true)
                                {
                                    *guard = None;
                                }
                            }
                        }
                    }

                    let decision_request: Option<FullAutoDecisionRequest> = {
                        let mut full_auto_guard = full_auto_state_notifications.lock().await;
                        if let Some(state) = full_auto_guard.as_mut() {
                            state.record_event(
                                &notification.method,
                                params,
                                thread_id_value.as_deref(),
                                turn_id_value.as_deref(),
                            );
                            if notification.method == "thread/started" {
                                if let Some(thread_id) = thread_id_value.as_deref() {
                                    state.adopt_thread(thread_id);
                                }
                            }
                            if notification.method == "turn/completed" {
                                state.prepare_decision(
                                    thread_id_value.as_deref(),
                                    turn_id_value.as_deref(),
                                )
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };

                    if let Some(request) = decision_request {
                        let full_auto_state = full_auto_state_notifications.clone();
                        let proxy = proxy_full_auto.clone();
                        let client = client_full_auto.clone();
                        let cwd = cwd_full_auto.clone();
                        let workspace_id = workspace_id_full_auto.clone();
                        tokio::spawn(async move {
                            let (guidance_mode, guidance_inputs, mut lm) = {
                                let guard = full_auto_state.lock().await;
                                if let Some(state) = guard.as_ref() {
                                    (
                                        state.guidance_mode(),
                                        Some(state.build_guidance_inputs(&request.summary)),
                                        state.decision_lm(),
                                    )
                                } else {
                                    (GuidanceMode::Legacy, None, None)
                                }
                            };

                            if lm.is_none() {
                                let built = match guidance_mode {
                                    GuidanceMode::Demo => ensure_guidance_demo_lm().await,
                                    GuidanceMode::Legacy => {
                                        let model = decision_model();
                                        ensure_codex_lm(&model).await
                                    }
                                };
                                match built {
                                    Ok(built) => {
                                        lm = Some(built.clone());
                                        let mut guard = full_auto_state.lock().await;
                                        if let Some(state) = guard.as_mut() {
                                            state.set_decision_lm(built);
                                        }
                                    }
                                    Err(error) => {
                                        let payload = json!({
                                            "method": "fullauto/decision",
                                            "params": {
                                                "threadId": request.thread_id,
                                                "turnId": request.turn_id,
                                                "action": "pause",
                                                "reason": error,
                                                "confidence": 0.0,
                                                "state": "paused"
                                            }
                                        });
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: payload.to_string(),
                                        });
                                        let mut guard = full_auto_state.lock().await;
                                        *guard = None;
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "fullauto/status",
                                                "params": {
                                                    "workspaceId": workspace_id,
                                                    "enabled": false,
                                                    "state": "paused"
                                                }
                                            })
                                            .to_string(),
                                        });
                                        return;
                                    }
                                }
                            }

                            let Some(lm) = lm else {
                                return;
                            };

                            let decision_result = match guidance_mode {
                                GuidanceMode::Demo => {
                                    let Some(inputs) = guidance_inputs.as_ref() else {
                                        let error = "Guidance inputs missing; pausing Full Auto."
                                            .to_string();
                                        let payload = json!({
                                            "method": "fullauto/decision",
                                            "params": {
                                                "threadId": request.thread_id,
                                                "turnId": request.turn_id,
                                                "action": "pause",
                                                "reason": error,
                                                "confidence": 0.0,
                                                "state": "paused"
                                            }
                                        });
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: payload.to_string(),
                                        });
                                        let mut guard = full_auto_state.lock().await;
                                        *guard = None;
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "fullauto/status",
                                                "params": {
                                                    "workspaceId": workspace_id,
                                                    "enabled": false,
                                                    "state": "paused"
                                                }
                                            })
                                            .to_string(),
                                        });
                                        return;
                                    };
                                    run_guidance_decision(inputs, &lm).await
                                }
                                GuidanceMode::Legacy => {
                                    run_full_auto_decision(&request.summary, &lm).await
                                }
                            };

                            let decision_result = match decision_result {
                                Ok(decision) => decision,
                                Err(error) => {
                                    let payload = json!({
                                        "method": "fullauto/decision",
                                        "params": {
                                            "threadId": request.thread_id,
                                            "turnId": request.turn_id,
                                            "action": "pause",
                                            "reason": error,
                                            "confidence": 0.0,
                                            "state": "paused"
                                        }
                                    });
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: payload.to_string(),
                                    });
                                    let mut guard = full_auto_state.lock().await;
                                    *guard = None;
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "fullauto/status",
                                            "params": {
                                                "workspaceId": workspace_id,
                                                "enabled": false,
                                                "state": "paused"
                                            }
                                        })
                                        .to_string(),
                                    });
                                    return;
                                }
                            };

                            let FullAutoDecisionResult {
                                decision: raw_decision,
                                diagnostics,
                            } = decision_result;

                            let (decision, run_id, sequence_id) = {
                                let mut guard = full_auto_state.lock().await;
                                if let Some(state) = guard.as_mut() {
                                    let decision = state.enforce_guardrails(
                                        &request.thread_id,
                                        &request.summary,
                                        raw_decision,
                                    );
                                    state.apply_decision(&request.thread_id, &decision);
                                    let sequence_id = state.next_decision_sequence();
                                    (decision, state.run_id.clone(), sequence_id)
                                } else {
                                    (raw_decision, "unknown".to_string(), 0)
                                }
                            };

                            let decision_state = if decision.action == FullAutoAction::Continue {
                                "running"
                            } else {
                                "paused"
                            };
                            let next_input_preview = decision
                                .next_input
                                .as_deref()
                                .unwrap_or_default()
                                .chars()
                                .take(140)
                                .collect::<String>();

                            let guardrail_value = decision
                                .guardrail
                                .as_ref()
                                .and_then(|g| serde_json::to_value(g).ok());
                            let summary_value =
                                serde_json::to_value(&request.summary).unwrap_or(Value::Null);
                            let diagnostics_value =
                                serde_json::to_value(&diagnostics).unwrap_or(Value::Null);

                            let payload = json!({
                                "method": "fullauto/decision",
                                "params": {
                                    "threadId": request.thread_id,
                                    "turnId": request.turn_id,
                                    "action": decision.action.as_str(),
                                    "reason": decision.reason,
                                    "confidence": decision.confidence,
                                    "state": decision_state,
                                    "nextInput": next_input_preview,
                                    "sequenceId": sequence_id,
                                    "runId": run_id,
                                    "guardrail": guardrail_value,
                                    "summary": summary_value
                                }
                            });
                            let _ = proxy.send_event(AppEvent::AppServerEvent {
                                message: payload.to_string(),
                            });

                            let raw_payload = json!({
                                "method": "fullauto/decision_raw",
                                "params": {
                                    "threadId": request.thread_id,
                                    "turnId": request.turn_id,
                                    "sequenceId": sequence_id,
                                    "runId": run_id,
                                    "rawPrediction": diagnostics.raw_prediction,
                                    "parseDiagnostics": diagnostics_value,
                                    "summary": summary_value
                                }
                            });
                            let _ = proxy.send_event(AppEvent::AppServerEvent {
                                message: raw_payload.to_string(),
                            });

                            match decision.action {
                                FullAutoAction::Continue => {
                                    let next_input = decision
                                        .next_input
                                        .unwrap_or_else(|| request.fallback_prompt.clone());
                                    let params = TurnStartParams {
                                        thread_id: request.thread_id.clone(),
                                        input: vec![UserInput::Text { text: next_input }],
                                        model: None,
                                        effort: None,
                                        summary: None,
                                        approval_policy: Some(AskForApproval::Never),
                                        sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
                                        cwd: Some(cwd),
                                    };
                                    let _ = client.turn_start(params).await;
                                }
                                FullAutoAction::Pause
                                | FullAutoAction::Stop
                                | FullAutoAction::Review => {
                                    let mut guard = full_auto_state.lock().await;
                                    *guard = None;
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "fullauto/status",
                                            "params": {
                                                "workspaceId": workspace_id,
                                                "enabled": false,
                                                "state": "paused"
                                            }
                                        })
                                        .to_string(),
                                    });
                                }
                            }
                        });
                    }

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
            let full_auto_state_requests = full_auto_state.clone();
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
                        _ => build_auto_response(request.method.as_str(), request.params.as_ref())
                            .unwrap_or_else(|| json!({})),
                    };
                    let _ = client_requests.respond(request.id, &response).await;

                    let params = request.params.as_ref();
                    let thread_id_value = extract_thread_id(params);
                    let turn_id_value = extract_turn_id(params);
                    let mut full_auto_guard = full_auto_state_requests.lock().await;
                    if let Some(state) = full_auto_guard.as_mut() {
                        state.record_event(
                            request.method.as_str(),
                            params,
                            thread_id_value.as_deref(),
                            turn_id_value.as_deref(),
                        );
                    }
                }
            });

            let handle = tokio::runtime::Handle::current();
            let workspace_for_actions = workspace.clone();
            let client_for_actions = client.clone();
            let session_states_for_actions = session_states.clone();
            let thread_to_session_for_actions = thread_to_session.clone();
            let proxy_actions = proxy.clone();
            let cwd_for_actions = cwd_string.clone();
            tokio::task::spawn_blocking(move || {
                let mut pylon_runtime = InProcessPylon::new();
                while let Ok(action) = action_rx.recv() {
                    workspace_for_actions.dispatch(action.clone());
                    match action {
                        UserAction::NewChat { model, .. } => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            let cwd = cwd_for_actions.clone();
                            let full_auto_state = full_auto_state.clone();
                            let workspace = workspace_for_actions.clone();
                            let workspace_id = workspace_id.clone();
                            let session_states = session_states_for_actions.clone();
                            let thread_to_session = thread_to_session_for_actions.clone();
                            let session_handle =
                                workspace.start_session(Some("New chat".to_string()));
                            let session_id = session_handle.session_id();
                            let session_state = SessionRuntime::new();

                            handle.spawn(async move {
                                {
                                    let mut guard = session_states.lock().await;
                                    guard.insert(session_id, session_state.clone());
                                }

                                {
                                    let mut guard = full_auto_state.lock().await;
                                    if guard.is_some() {
                                        *guard = None;
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "fullauto/status",
                                                "params": {
                                                    "workspaceId": workspace_id,
                                                    "enabled": false,
                                                    "state": "paused"
                                                }
                                            })
                                            .to_string(),
                                        });
                                    }
                                }

                                let selected_model =
                                    model.unwrap_or_else(|| DEFAULT_THREAD_MODEL.to_string());
                                match client
                                    .thread_start(ThreadStartParams {
                                        model: Some(selected_model.clone()),
                                        model_provider: None,
                                        cwd: Some(cwd.clone()),
                                        approval_policy: Some(AskForApproval::Never),
                                        sandbox: Some(SandboxMode::WorkspaceWrite),
                                    })
                                    .await
                                {
                                    Ok(response) => {
                                        {
                                            let mut guard = session_state.thread_id.lock().await;
                                            *guard = Some(response.thread.id.clone());
                                        }
                                        {
                                            let mut map = thread_to_session.lock().await;
                                            map.insert(response.thread.id.clone(), session_id);
                                        }
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "thread/started",
                                                "params": {
                                                    "threadId": response.thread.id,
                                                    "model": selected_model,
                                                    "sessionId": session_id.to_string()
                                                }
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
                            });
                        }
                        UserAction::Message {
                            session_id,
                            text,
                            model,
                            reasoning,
                        } => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            let cwd = cwd_for_actions.clone();
                            let session_states = session_states_for_actions.clone();
                            let full_auto_state = full_auto_state.clone();
                            handle.spawn(async move {
                                let session_state =
                                    session_states.lock().await.get(&session_id).cloned();
                                let Some(session_state) = session_state else {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "codex/error",
                                            "params": { "message": "Session not ready" }
                                        })
                                        .to_string(),
                                    });
                                    return;
                                };
                                let thread_id = session_state.thread_id.lock().await.clone();
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
                                let (should_intercept, cached_lm, goal_intent) = {
                                    let mut guard = full_auto_state.lock().await;
                                    if let Some(state) = guard.as_mut() {
                                        if state.matches_thread(Some(thread_id.as_str())) {
                                            state.adopt_thread(&thread_id);
                                            let should_intercept = state.activate_guidance_mode();
                                            (
                                                should_intercept,
                                                state.decision_lm(),
                                                state.guidance_goal_intent(),
                                            )
                                        } else {
                                            (false, None, guidance_goal_intent())
                                        }
                                    } else {
                                        (false, None, guidance_goal_intent())
                                    }
                                };

                                if should_intercept {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "guidance/user_message",
                                            "params": {
                                                "threadId": thread_id,
                                                "text": text
                                            }
                                        })
                                        .to_string(),
                                    });
                                    let (lm, should_cache) = match resolve_guidance_lm(cached_lm)
                                        .await
                                    {
                                        Ok(result) => result,
                                        Err(error) => {
                                            let payload = json!({
                                                "method": "guidance/response",
                                                "params": {
                                                    "threadId": thread_id,
                                                    "text": format!("Guidance error: {error}"),
                                                    "signatures": ["GuidanceRouterSignature"],
                                                    "model": "unknown"
                                                }
                                            });
                                            let _ = proxy.send_event(AppEvent::AppServerEvent {
                                                message: payload.to_string(),
                                            });
                                            return;
                                        }
                                    };
                                    if should_cache {
                                        let mut guard = full_auto_state.lock().await;
                                        if let Some(state) = guard.as_mut() {
                                            state.set_decision_lm(lm.clone());
                                        }
                                    }
                                    let (response, signatures) = if is_super_trigger(&text) {
                                        match run_guidance_super(
                                            &proxy,
                                            &thread_id,
                                            &text,
                                            &goal_intent,
                                            &lm,
                                        )
                                        .await
                                        {
                                            Ok(result) => result,
                                            Err(error) => (
                                                format!("Guidance error: {error}"),
                                                vec![
                                                    "TaskUnderstandingSignature".to_string(),
                                                    "PlanningSignature".to_string(),
                                                    "GuidanceDecisionSignature".to_string(),
                                                ],
                                            ),
                                        }
                                    } else {
                                        match run_guidance_router(
                                            &proxy,
                                            &thread_id,
                                            &text,
                                            &goal_intent,
                                            &lm,
                                        )
                                        .await
                                        {
                                            Ok(result) => result,
                                            Err(error) => (
                                                format!("Guidance error: {error}"),
                                                vec!["GuidanceRouterSignature".to_string()],
                                            ),
                                        }
                                    };
                                    let payload = json!({
                                        "method": "guidance/response",
                                        "params": {
                                            "threadId": thread_id,
                                            "text": response,
                                            "signatures": signatures,
                                            "model": lm.model
                                        }
                                    });
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: payload.to_string(),
                                    });
                                    return;
                                }

                                let params = TurnStartParams {
                                    thread_id,
                                    input: vec![UserInput::Text { text }],
                                    model,
                                    effort: reasoning.as_deref().and_then(parse_reasoning_effort),
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
                        UserAction::ThreadsRefresh => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            handle.spawn(async move {
                                let params = ThreadListParams {
                                    limit: Some(10),
                                    ..Default::default()
                                };
                                match client.thread_list(params).await {
                                    Ok(response) => {
                                        let threads = response
                                            .data
                                            .into_iter()
                                            .map(|thread| autopilot_app::ThreadSummary {
                                                id: thread.id,
                                                preview: thread.preview,
                                                model_provider: thread.model_provider,
                                                cwd: thread.cwd,
                                                created_at: thread.created_at,
                                            })
                                            .collect::<Vec<_>>();
                                        let _ =
                                            proxy.send_event(AppEvent::ThreadsUpdated { threads });
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
                            });
                        }
                        UserAction::ThreadOpen { thread_id } => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            let cwd = cwd_for_actions.clone();
                            let workspace = workspace_for_actions.clone();
                            let session_states = session_states_for_actions.clone();
                            let thread_to_session = thread_to_session_for_actions.clone();
                            let session_handle =
                                workspace.start_session(Some("Thread".to_string()));
                            let session_id = session_handle.session_id();
                            let session_state = SessionRuntime::new();

                            handle.spawn(async move {
                                {
                                    let mut guard = session_states.lock().await;
                                    guard.insert(session_id, session_state.clone());
                                }

                                match client
                                    .thread_resume(ThreadResumeParams {
                                        thread_id: thread_id.clone(),
                                        model: None,
                                        model_provider: None,
                                        cwd: Some(cwd),
                                        approval_policy: Some(AskForApproval::Never),
                                        sandbox: Some(SandboxMode::WorkspaceWrite),
                                    })
                                    .await
                                {
                                    Ok(response) => {
                                        {
                                            let mut guard = session_state.thread_id.lock().await;
                                            *guard = Some(response.thread.id.clone());
                                        }
                                        {
                                            let mut map = thread_to_session.lock().await;
                                            map.insert(response.thread.id.clone(), session_id);
                                        }
                                        let thread = autopilot_app::ThreadSnapshot {
                                            id: response.thread.id,
                                            preview: response.thread.preview,
                                            turns: response
                                                .thread
                                                .turns
                                                .into_iter()
                                                .map(|turn| autopilot_app::ThreadTurn {
                                                    id: turn.id,
                                                    items: turn.items,
                                                })
                                                .collect(),
                                        };
                                        let _ = proxy.send_event(AppEvent::ThreadLoaded {
                                            session_id,
                                            thread,
                                            model: response.model,
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
                            });
                        }
                        UserAction::Interrupt { session_id, .. } => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            let session_states = session_states_for_actions.clone();
                            handle.spawn(async move {
                                let session_state =
                                    session_states.lock().await.get(&session_id).cloned();
                                let Some(session_state) = session_state else {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "codex/error",
                                            "params": { "message": "Session not ready" }
                                        })
                                        .to_string(),
                                    });
                                    return;
                                };
                                let thread_id = session_state.thread_id.lock().await.clone();
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

                                let mut turn_guard = session_state.turn_id.lock().await;
                                let turn_value =
                                    turn_guard.clone().unwrap_or_else(|| "pending".into());
                                if turn_guard.is_none() {
                                    session_state
                                        .pending_interrupt
                                        .store(true, Ordering::SeqCst);
                                } else {
                                    *turn_guard = None;
                                }

                                if let Err(err) = client
                                    .turn_interrupt(TurnInterruptParams {
                                        thread_id,
                                        turn_id: turn_value,
                                    })
                                    .await
                                {
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
                        UserAction::PylonInit => {
                            let proxy = proxy_actions.clone();
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        init_pylon_identity(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => pylon_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy.send_event(AppEvent::PylonStatus { status });
                        }
                        UserAction::PylonStart => {
                            let proxy = proxy_actions.clone();
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        start_pylon_in_process(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => pylon_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy.send_event(AppEvent::PylonStatus { status });
                        }
                        UserAction::PylonStop => {
                            let proxy = proxy_actions.clone();
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        stop_pylon_in_process(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => pylon_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy.send_event(AppEvent::PylonStatus { status });
                        }
                        UserAction::PylonRefresh => {
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        refresh_pylon_status(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => pylon_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy_actions.send_event(AppEvent::PylonStatus { status });
                        }
                        UserAction::WalletRefresh => {
                            let proxy = proxy_actions.clone();
                            handle.block_on(async move {
                                let status = fetch_wallet_status().await;
                                let _ = proxy.send_event(AppEvent::WalletStatus { status });
                            });
                        }
                        UserAction::DvmProviderStart => {
                            let proxy = proxy_actions.clone();
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        let _ = start_pylon_in_process(&mut pylon_runtime, &config)
                                            .await;
                                        fetch_dvm_provider_status(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => dvm_provider_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy.send_event(AppEvent::DvmProviderStatus { status });
                        }
                        UserAction::DvmProviderStop => {
                            let proxy = proxy_actions.clone();
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        let _ = stop_pylon_in_process(&mut pylon_runtime, &config)
                                            .await;
                                        fetch_dvm_provider_status(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => dvm_provider_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy.send_event(AppEvent::DvmProviderStatus { status });
                        }
                        UserAction::DvmProviderRefresh => {
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        fetch_dvm_provider_status(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => dvm_provider_status_error(err.to_string()),
                                }
                            });
                            let _ =
                                proxy_actions.send_event(AppEvent::DvmProviderStatus { status });
                        }
                        UserAction::DvmHistoryRefresh => {
                            let snapshot = fetch_dvm_history();
                            let _ = proxy_actions.send_event(AppEvent::DvmHistory { snapshot });
                        }
                        UserAction::Nip90Submit {
                            kind,
                            prompt,
                            relays,
                            provider,
                        } => {
                            let proxy = proxy_actions.clone();
                            handle.spawn(async move {
                                let log = |message: String| {
                                    let _ = proxy.send_event(AppEvent::Nip90Log { message });
                                };

                                log("Submitting NIP-90 job...".to_string());

                                let config = match PylonConfig::load() {
                                    Ok(config) => config,
                                    Err(err) => {
                                        log(format!("Failed to load Pylon config: {err}"));
                                        return;
                                    }
                                };

                                let data_dir = match config.data_path() {
                                    Ok(path) => path,
                                    Err(err) => {
                                        log(format!("Failed to resolve Pylon data dir: {err}"));
                                        return;
                                    }
                                };
                                let identity_path = data_dir.join("identity.mnemonic");
                                if !identity_path.exists() {
                                    log(format!(
                                        "No identity found. Run 'pylon init' first. Expected: {}",
                                        identity_path.display()
                                    ));
                                    return;
                                }

                                let mnemonic = match std::fs::read_to_string(&identity_path) {
                                    Ok(value) => value.trim().to_string(),
                                    Err(err) => {
                                        log(format!("Failed to read identity: {err}"));
                                        return;
                                    }
                                };

                                let keypair = match nostr::derive_keypair(&mnemonic) {
                                    Ok(pair) => pair,
                                    Err(err) => {
                                        log(format!("Failed to derive Nostr keys: {err}"));
                                        return;
                                    }
                                };

                                let client = match DvmClient::new(keypair.private_key) {
                                    Ok(client) => client,
                                    Err(err) => {
                                        log(format!("Failed to init DVM client: {err}"));
                                        return;
                                    }
                                };

                                let relays = if relays.is_empty() {
                                    config.relays.clone()
                                } else {
                                    relays
                                };
                                if relays.is_empty() {
                                    log("No relays configured for NIP-90 submission.".to_string());
                                    return;
                                }

                                let kind = if kind == 0 {
                                    KIND_JOB_TEXT_GENERATION
                                } else {
                                    kind
                                };

                                let mut request = match JobRequest::new(kind) {
                                    Ok(request) => request.add_input(JobInput::text(prompt)),
                                    Err(err) => {
                                        log(format!("Invalid job kind {kind}: {err}"));
                                        return;
                                    }
                                };

                                for relay in &relays {
                                    request = request.add_relay(relay.clone());
                                }
                                if let Some(provider) = provider {
                                    request = request.add_service_provider(provider);
                                }

                                let relay_refs: Vec<&str> =
                                    relays.iter().map(|relay| relay.as_str()).collect();
                                let submission = match client.submit_job(request, &relay_refs).await
                                {
                                    Ok(submission) => submission,
                                    Err(err) => {
                                        log(format!("Job submission failed: {err}"));
                                        return;
                                    }
                                };

                                log(format!("Submitted job {}", submission.event_id));

                                match client
                                    .await_result(
                                        &submission.event_id,
                                        std::time::Duration::from_secs(60),
                                    )
                                    .await
                                {
                                    Ok(result) => {
                                        let preview = if result.content.len() > 400 {
                                            format!("{}…", &result.content[..400])
                                        } else {
                                            result.content
                                        };
                                        log(format!("Result: {}", preview));
                                    }
                                    Err(err) => {
                                        log(format!("Result timeout/error: {err}"));
                                    }
                                }
                            });
                        }
                        UserAction::FullAutoToggle {
                            session_id,
                            enabled,
                            continue_prompt,
                            ..
                        } => {
                            let proxy = proxy_actions.clone();
                            let full_auto_state = full_auto_state.clone();
                            let workspace_id = workspace_id.clone();
                            let session_states = session_states_for_actions.clone();
                            handle.spawn(async move {
                                let thread_id = {
                                    let state =
                                        session_states.lock().await.get(&session_id).cloned();
                                    if let Some(state) = state {
                                        state.thread_id.lock().await.clone()
                                    } else {
                                        None
                                    }
                                };
                                if enabled {
                                    let mut guard = full_auto_state.lock().await;
                                    let mut next_state = FullAutoState::new(
                                        &workspace_id,
                                        thread_id.clone(),
                                        continue_prompt.clone(),
                                    );
                                    next_state.enabled = true;
                                    if let Some(thread_id) = thread_id.clone() {
                                        next_state.thread_id = Some(thread_id);
                                    }
                                    next_state.set_continue_prompt(continue_prompt);
                                    let continue_prompt = next_state.continue_prompt.clone();
                                    *guard = Some(next_state);
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "fullauto/status",
                                            "params": {
                                                "workspaceId": workspace_id,
                                                "enabled": true,
                                                "state": "running",
                                                "continuePrompt": continue_prompt
                                            }
                                        })
                                        .to_string(),
                                    });
                                } else {
                                    let mut guard = full_auto_state.lock().await;
                                    *guard = None;
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "fullauto/status",
                                            "params": {
                                                "workspaceId": workspace_id,
                                                "enabled": false,
                                                "state": "paused"
                                            }
                                        })
                                        .to_string(),
                                    });
                                }
                            });
                        }
                        _ => {}
                    }
                }
            });

            futures::future::pending::<()>().await;
        });
    });
}

fn extract_thread_id(params: Option<&Value>) -> Option<String> {
    let params = params?;
    if let Some(thread_id) = params
        .get("threadId")
        .or_else(|| params.get("thread_id"))
        .and_then(|id| id.as_str())
    {
        return Some(thread_id.to_string());
    }
    params
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(|id| id.as_str())
        .map(|id| id.to_string())
}

fn extract_turn_id(params: Option<&Value>) -> Option<String> {
    let params = params?;
    if let Some(turn_id) = params
        .get("turnId")
        .or_else(|| params.get("turn_id"))
        .and_then(|id| id.as_str())
    {
        return Some(turn_id.to_string());
    }
    params
        .get("turn")
        .and_then(|turn| turn.get("id"))
        .and_then(|id| id.as_str())
        .map(|id| id.to_string())
}

fn guidance_goal_intent() -> String {
    env::var(ENV_GUIDANCE_GOAL)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_GUIDANCE_GOAL_INTENT.to_string())
}

fn is_super_trigger(message: &str) -> bool {
    let trimmed = message.trim().to_lowercase();
    let normalized = trimmed
        .trim_matches(|ch: char| !ch.is_alphanumeric() && !ch.is_whitespace())
        .to_string();
    matches!(
        normalized.as_str(),
        "go" | "go ahead"
            | "do it"
            | "just do it"
            | "do the thing"
            | "do this"
            | "execute"
            | "run it"
            | "continue"
            | "proceed"
            | "ship it"
            | "make it happen"
    )
}

fn guidance_response_score(text: &str) -> f32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    if trimmed.contains('\n') || trimmed.len() > 140 {
        return 0.0;
    }
    if trimmed.contains('?') || trimmed.contains('`') || trimmed.contains('\"') {
        return 0.0;
    }
    let lower = trimmed.to_lowercase();
    let banned = [
        "i am an ai",
        "i'm an ai",
        "as an ai",
        "i do not have",
        "i don't have",
        "i cannot",
        "i can't",
        "i am a language model",
    ];
    if banned.iter().any(|phrase| lower.contains(phrase)) {
        return 0.0;
    }
    1.0
}

fn sanitize_guidance_response(text: &str) -> String {
    let mut line = text.lines().next().unwrap_or("").trim().to_string();
    line = line.trim_matches('"').trim_matches('\'').trim().to_string();
    if line.len() > 140 {
        line.truncate(140);
    }
    line
}

fn fallback_guidance_response(goal_intent: &str) -> String {
    if goal_intent.trim().is_empty() {
        "Summarize the request and propose the next concrete step.".to_string()
    } else {
        format!(
            "Summarize the request and propose the next step toward: {}.",
            goal_intent.trim()
        )
    }
}

async fn resolve_guidance_lm(cached_lm: Option<dsrs::LM>) -> Result<(dsrs::LM, bool), String> {
    if let Some(lm) = cached_lm.clone()
        && lm.model.starts_with("codex:")
    {
        return Ok((lm, false));
    }
    if let Ok(lm) = ensure_codex_lm(&decision_model()).await {
        return Ok((lm, true));
    }
    if let Some(lm) = cached_lm {
        return Ok((lm, false));
    }
    let lm = ensure_guidance_demo_lm().await?;
    Ok((lm, true))
}

fn prediction_to_string(prediction: &dsrs::Prediction, key: &str) -> String {
    match prediction.get(key, None) {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => "".to_string(),
        other => other.to_string(),
    }
}

fn guidance_repo_context() -> String {
    env::current_dir()
        .ok()
        .map(|path| format!("Repo path: {}", path.display()))
        .unwrap_or_else(|| "Repo path: unknown".to_string())
}

fn emit_guidance_step(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    signature: &str,
    text: &str,
    model: &str,
) {
    let payload = json!({
        "method": "guidance/step",
        "params": {
            "threadId": thread_id,
            "signature": signature,
            "text": text,
            "model": model
        }
    });
    let _ = proxy.send_event(AppEvent::AppServerEvent {
        message: payload.to_string(),
    });
}

fn emit_guidance_status(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    signature: Option<&str>,
    text: &str,
) {
    let payload = json!({
        "method": "guidance/status",
        "params": {
            "threadId": thread_id,
            "signature": signature,
            "text": text
        }
    });
    let _ = proxy.send_event(AppEvent::AppServerEvent {
        message: payload.to_string(),
    });
}

fn extract_first_json_string(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return None;
    }
    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed) {
        for item in items {
            if let Some(text) = item.as_str() {
                if !text.trim().is_empty() {
                    return Some(text.trim().to_string());
                }
            } else if !item.is_null() {
                let text = item.to_string();
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
        }
    }
    let first = trimmed
        .split('\n')
        .map(str::trim)
        .find(|line| !line.is_empty());
    first.map(|line| line.trim_matches('"').trim_matches('\'').to_string())
}

fn extract_first_step_description(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return None;
    }
    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed) {
        for item in items {
            if let Some(desc) = item.get("description").and_then(|value| value.as_str()) {
                if !desc.trim().is_empty() {
                    return Some(desc.trim().to_string());
                }
            }
        }
    }
    None
}

fn strip_question_marks(text: &str) -> String {
    text.replace('?', "").trim().to_string()
}

fn is_question_like(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }
    if lower.contains('?') {
        return true;
    }
    let cues = [
        "clarify",
        "could you",
        "can you",
        "please provide",
        "need more",
        "what do you want",
        "which task",
        "specific task",
        "details",
    ];
    cues.iter().any(|cue| lower.contains(cue))
}

async fn run_task_understanding(
    message: &str,
    repo_context: &str,
    goal_intent: &str,
    lm: &dsrs::LM,
) -> Result<String, String> {
    let predictor = Predict::new(TaskUnderstandingSignature::new());
    let inputs = example! {
        "user_request": "input" => message.to_string(),
        "repo_context": "input" => repo_context.to_string(),
    };
    let lm = std::sync::Arc::new(lm.clone());
    let prediction = predictor
        .forward_with_config(inputs, lm)
        .await
        .map_err(|e| format!("Task understanding failed: {e}"))?;
    let task_type = prediction_to_string(&prediction, "task_type");
    let requirements_raw = prediction_to_string(&prediction, "requirements");
    let questions_raw = prediction_to_string(&prediction, "clarifying_questions");
    if let Some(question) = extract_first_json_string(&questions_raw) {
        let question = strip_question_marks(&question);
        return Ok(sanitize_guidance_response(&format!(
            "Need clarification: {}.",
            question
        )));
    }
    if let Some(requirement) = extract_first_json_string(&requirements_raw) {
        let task_type = if task_type.trim().is_empty() {
            "Task".to_string()
        } else {
            task_type
        };
        return Ok(sanitize_guidance_response(&format!(
            "{} focus: {}.",
            task_type, requirement
        )));
    }
    Ok(fallback_guidance_response(goal_intent))
}

async fn run_planning_summary(
    message: &str,
    repo_context: &str,
    goal_intent: &str,
    lm: &dsrs::LM,
) -> Result<String, String> {
    let predictor = Predict::new(PlanningSignature::new());
    let inputs = example! {
        "task_description": "input" => message.to_string(),
        "repo_context": "input" => repo_context.to_string(),
        "file_tree": "input" => "".to_string(),
        "context_summary": "input" => "".to_string(),
        "constraints": "input" => "full_auto_guidance".to_string(),
    };
    let lm = std::sync::Arc::new(lm.clone());
    let prediction = predictor
        .forward_with_config(inputs, lm)
        .await
        .map_err(|e| format!("Planning failed: {e}"))?;
    let steps_raw = prediction_to_string(&prediction, "steps");
    if let Some(step) = extract_first_step_description(&steps_raw) {
        return Ok(sanitize_guidance_response(&format!("Next step: {}.", step)));
    }
    Ok(fallback_guidance_response(goal_intent))
}

async fn handle_guidance_route(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    message: &str,
    goal_intent: &str,
    route: &str,
    response: &str,
    lm: &dsrs::LM,
) -> Result<(String, Vec<String>), String> {
    let mut signatures = vec!["GuidanceRouterSignature".to_string()];
    let repo_context = guidance_repo_context();
    match route.trim().to_lowercase().as_str() {
        "understand" => {
            emit_guidance_status(
                proxy,
                thread_id,
                Some("TaskUnderstandingSignature"),
                "Running…",
            );
            let text = run_task_understanding(message, &repo_context, goal_intent, lm).await?;
            signatures.push("TaskUnderstandingSignature".to_string());
            Ok((text, signatures))
        }
        "plan" => {
            emit_guidance_status(
                proxy,
                thread_id,
                Some("PlanningSignature"),
                "Running…",
            );
            let text = run_planning_summary(message, &repo_context, goal_intent, lm).await?;
            signatures.push("PlanningSignature".to_string());
            Ok((text, signatures))
        }
        _ => {
            let text = if response.trim().is_empty() {
                fallback_guidance_response(goal_intent)
            } else {
                sanitize_guidance_response(response)
            };
            Ok((text, signatures))
        }
    }
}

async fn run_guidance_super(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    message: &str,
    goal_intent: &str,
    lm: &dsrs::LM,
) -> Result<(String, Vec<String>), String> {
    let mut signatures = Vec::new();
    let repo_context = guidance_repo_context();
    let lm_arc = std::sync::Arc::new(lm.clone());

    emit_guidance_status(
        proxy,
        thread_id,
        Some("TaskUnderstandingSignature"),
        "Running…",
    );
    let understanding_predictor = Predict::new(TaskUnderstandingSignature::new());
    let understanding_inputs = example! {
        "user_request": "input" => message.to_string(),
        "repo_context": "input" => repo_context.clone(),
    };
    let understanding = understanding_predictor
        .forward_with_config(understanding_inputs, lm_arc.clone())
        .await
        .map_err(|e| format!("Task understanding failed: {e}"))?;
    let task_type = prediction_to_string(&understanding, "task_type");
    let requirements_raw = prediction_to_string(&understanding, "requirements");
    let questions_raw = prediction_to_string(&understanding, "clarifying_questions");
    signatures.push("TaskUnderstandingSignature".to_string());
    let requirement = extract_first_json_string(&requirements_raw);
    let question = extract_first_json_string(&questions_raw);
    let step_text = if let Some(requirement) = requirement.as_ref() {
        let task_label = if task_type.trim().is_empty() {
            "Task".to_string()
        } else {
            task_type.clone()
        };
        sanitize_guidance_response(&format!("{} focus: {}.", task_label, requirement))
    } else if question.is_some() {
        let intent = if goal_intent.trim().is_empty() {
            "continue with the next logical task".to_string()
        } else {
            goal_intent.trim().to_string()
        };
        sanitize_guidance_response(&format!("Assumed intent: {}.", intent))
    } else {
        sanitize_guidance_response("Assumed intent: continue with the next logical task.")
    };
    emit_guidance_step(
        proxy,
        thread_id,
        "TaskUnderstandingSignature",
        &step_text,
        &lm.model,
    );

    emit_guidance_status(
        proxy,
        thread_id,
        Some("PlanningSignature"),
        "Running…",
    );
    let planning_predictor = Predict::new(PlanningSignature::new());
    let planning_message = if message.trim().len() <= 4 {
        if goal_intent.trim().is_empty() {
            "Continue with the next logical task.".to_string()
        } else {
            goal_intent.to_string()
        }
    } else {
        message.to_string()
    };
    let planning_inputs = example! {
        "task_description": "input" => planning_message,
        "repo_context": "input" => repo_context,
        "file_tree": "input" => "".to_string(),
        "context_summary": "input" => "".to_string(),
        "constraints": "input" => "full_auto_guidance".to_string(),
    };
    let planning = planning_predictor
        .forward_with_config(planning_inputs, lm_arc.clone())
        .await
        .map_err(|e| format!("Planning failed: {e}"))?;
    let steps_raw = prediction_to_string(&planning, "steps");
    let first_step = extract_first_step_description(&steps_raw);
    signatures.push("PlanningSignature".to_string());
    if let Some(step) = first_step.as_ref() {
        let step_text = sanitize_guidance_response(&format!("Plan step: {}.", step));
        emit_guidance_step(proxy, thread_id, "PlanningSignature", &step_text, &lm.model);
    } else {
        emit_guidance_step(
            proxy,
            thread_id,
            "PlanningSignature",
            "Plan ready.",
            &lm.model,
        );
    }

    emit_guidance_status(
        proxy,
        thread_id,
        Some("GuidanceDecisionSignature"),
        "Running…",
    );
    let decision_predictor = Predict::new(GuidanceDecisionSignature::new());
    let summary_payload = json!({
        "user_message": message,
        "task_type": task_type,
        "requirements": requirements_raw,
        "plan_steps": steps_raw,
        "assumptions": if question.is_some() { "Proceed without clarification" } else { "" },
    });
    let summary_json = serde_json::to_string_pretty(&summary_payload)
        .unwrap_or_else(|_| summary_payload.to_string());
    let state_payload = json!({
        "mode": "super",
        "turn_count": 0,
        "no_progress_count": 0,
        "permissions": {
            "can_exec": true,
            "can_write": true,
            "network": "full"
        }
    });
    let state_json =
        serde_json::to_string_pretty(&state_payload).unwrap_or_else(|_| state_payload.to_string());
    let decision_inputs = example! {
        "goal_intent": "input" => goal_intent.to_string(),
        "goal_success_criteria": "input" => "[]".to_string(),
        "summary": "input" => summary_json,
        "state": "input" => state_json,
    };
    let decision = decision_predictor
        .forward_with_config(decision_inputs, lm_arc)
        .await
        .map_err(|e| format!("Guidance decision failed: {e}"))?;
    let action = prediction_to_string(&decision, "action");
    let next_input = prediction_to_string(&decision, "next_input");
    let _reason = prediction_to_string(&decision, "reason");
    signatures.push("GuidanceDecisionSignature".to_string());
    let normalized_action = action.trim().to_lowercase();
    let action_valid = matches!(
        normalized_action.as_str(),
        "continue" | "pause" | "stop" | "review"
    );
    let mut selected_next = if !next_input.trim().is_empty() && !is_question_like(&next_input) {
        next_input.trim().to_string()
    } else if let Some(step) = first_step.as_ref() {
        format!("Next step: {}.", step)
    } else {
        fallback_guidance_response(goal_intent)
    };
    if is_question_like(&selected_next) {
        selected_next = fallback_guidance_response(goal_intent);
    }
    let selected_action = if action_valid && normalized_action == "continue" {
        "continue".to_string()
    } else {
        "continue".to_string()
    };
    let decision_text = format!("Decision: {} — {}.", selected_action, selected_next);
    emit_guidance_step(
        proxy,
        thread_id,
        "GuidanceDecisionSignature",
        &sanitize_guidance_response(&decision_text),
        &lm.model,
    );

    let final_response = sanitize_guidance_response(&selected_next);

    Ok((final_response, signatures))
}

async fn run_guidance_router(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    message: &str,
    goal_intent: &str,
    lm: &dsrs::LM,
) -> Result<(String, Vec<String>), String> {
    emit_guidance_status(
        proxy,
        thread_id,
        Some("GuidanceRouterSignature"),
        "Running…",
    );
    let predictor = Predict::new(GuidanceRouterSignature::new());
    let inputs = example! {
        "user_message": "input" => message.to_string(),
        "goal_intent": "input" => goal_intent.to_string(),
        "context": "input" => "full_auto".to_string(),
    };
    let lm = std::sync::Arc::new(lm.clone());
    let mut best: Option<(String, String, f32)> = None;
    for _ in 0..3 {
        let prediction = predictor
            .forward_with_config(inputs.clone(), lm.clone())
            .await
            .map_err(|e| format!("Guidance router failed: {e}"))?;
        let response = prediction_to_string(&prediction, "response");
        let route = prediction_to_string(&prediction, "route");
        let mut score = guidance_response_score(&response);
        let route_norm = route.trim().to_lowercase();
        if matches!(route_norm.as_str(), "plan" | "understand") {
            score = score.max(0.9);
        }
        if score >= 0.9 {
            return handle_guidance_route(
                proxy,
                thread_id,
                message,
                goal_intent,
                &route,
                &response,
                lm.as_ref(),
            )
            .await;
        }
        if best.as_ref().map(|(_, _, s)| score > *s).unwrap_or(true) {
            best = Some((response, route, score));
        }
    }
    if let Some((response, route, score)) = best {
        if score > 0.0 {
            return handle_guidance_route(
                proxy,
                thread_id,
                message,
                goal_intent,
                &route,
                &response,
                lm.as_ref(),
            )
            .await;
        }
    }
    Ok((
        fallback_guidance_response(goal_intent),
        vec!["GuidanceRouterSignature".to_string()],
    ))
}

fn build_tool_input_response(params: &Value) -> Value {
    let mut answers = serde_json::Map::new();
    let questions = params
        .get("questions")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    for question in questions {
        let id = question
            .get("id")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());
        let answer = question
            .get("options")
            .and_then(|value| value.as_array())
            .and_then(|options| options.first())
            .and_then(|option| option.get("id"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .unwrap_or_else(|| "yes".to_string());

        if let Some(id) = id {
            answers.insert(
                id,
                json!({
                    "answers": [answer],
                }),
            );
        }
    }

    json!({ "answers": answers })
}

fn build_auto_response(method: &str, params: Option<&Value>) -> Option<Value> {
    match method {
        "execCommandApproval" | "applyPatchApproval" => Some(json!({ "decision": "approved" })),
        "item/tool/requestUserInput" => params.map(build_tool_input_response),
        _ => None,
    }
}

fn load_pylon_config_ollama() -> Result<PylonConfig> {
    let mut config = PylonConfig::load()?;
    config.backend_preference = vec!["ollama".to_string()];
    if config.default_model.trim().is_empty() {
        config.default_model = "llama3.2".to_string();
    }
    Ok(config)
}

fn identity_path_for_config(config: &PylonConfig) -> Result<PathBuf> {
    Ok(config.data_path()?.join("identity.mnemonic"))
}

fn pylon_identity_exists(config: &PylonConfig) -> bool {
    identity_path_for_config(config)
        .map(|path| path.exists())
        .unwrap_or(false)
}

fn load_or_init_identity(config: &PylonConfig) -> Result<UnifiedIdentity> {
    let identity_path = identity_path_for_config(config)?;
    if identity_path.exists() {
        let mnemonic = std::fs::read_to_string(&identity_path)?.trim().to_string();
        return UnifiedIdentity::from_mnemonic(&mnemonic, "")
            .map_err(|err| anyhow::anyhow!("Failed to load identity: {err}"));
    }

    if let Some(parent) = identity_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let identity = UnifiedIdentity::generate()
        .map_err(|err| anyhow::anyhow!("Failed to generate identity: {err}"))?;
    std::fs::write(&identity_path, identity.mnemonic())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&identity_path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(identity)
}

fn pylon_status_error(err: impl Into<String>) -> PylonStatus {
    PylonStatus {
        last_error: Some(err.into()),
        ..PylonStatus::default()
    }
}

fn dvm_provider_status_error(err: impl Into<String>) -> DvmProviderStatus {
    DvmProviderStatus {
        last_error: Some(err.into()),
        ..DvmProviderStatus::default()
    }
}

async fn init_pylon_identity(state: &mut InProcessPylon, config: &PylonConfig) -> PylonStatus {
    match load_or_init_identity(config) {
        Ok(_) => state.last_error = None,
        Err(err) => state.last_error = Some(err.to_string()),
    }
    refresh_pylon_status(state, config).await
}

async fn start_pylon_in_process(state: &mut InProcessPylon, config: &PylonConfig) -> PylonStatus {
    if let Some(provider) = state.provider.as_ref() {
        let provider_status = provider.status().await;
        if provider_status.running {
            state.last_error = None;
            return refresh_pylon_status(state, config).await;
        }
    }

    let identity = match load_or_init_identity(config) {
        Ok(identity) => identity,
        Err(err) => {
            state.last_error = Some(err.to_string());
            return refresh_pylon_status(state, config).await;
        }
    };

    let mut provider = match state.provider.take() {
        Some(provider) => provider,
        None => match PylonProvider::new(config.clone()).await {
            Ok(provider) => provider,
            Err(err) => {
                state.last_error = Some(err.to_string());
                return refresh_pylon_status(state, config).await;
            }
        },
    };

    if let Err(err) = provider.init_with_identity(identity).await {
        state.last_error = Some(err.to_string());
        state.provider = None;
        state.started_at = None;
        return refresh_pylon_status(state, config).await;
    }

    let provider_status = provider.status().await;
    if !provider_status
        .backends
        .iter()
        .any(|backend| backend == "ollama")
    {
        state.last_error = Some("Ollama backend not detected on localhost:11434.".to_string());
        state.provider = None;
        state.started_at = None;
        return refresh_pylon_status(state, config).await;
    }

    match provider.start().await {
        Ok(()) | Err(ProviderError::AlreadyRunning) => {
            if state.started_at.is_none() {
                state.started_at = Some(std::time::Instant::now());
            }
            state.last_error = None;
            state.provider = Some(provider);
        }
        Err(err) => {
            state.last_error = Some(err.to_string());
            state.provider = None;
            state.started_at = None;
        }
    }

    refresh_pylon_status(state, config).await
}

async fn stop_pylon_in_process(state: &mut InProcessPylon, config: &PylonConfig) -> PylonStatus {
    if let Some(provider) = state.provider.as_mut() {
        match provider.stop().await {
            Ok(()) | Err(ProviderError::NotRunning) => {
                state.started_at = None;
                state.last_error = None;
            }
            Err(err) => {
                state.last_error = Some(err.to_string());
            }
        }
    } else {
        state.started_at = None;
    }

    refresh_pylon_status(state, config).await
}

async fn refresh_pylon_status(state: &mut InProcessPylon, config: &PylonConfig) -> PylonStatus {
    let identity_exists = pylon_identity_exists(config);
    let (running, jobs_completed, earnings_msats) = if let Some(provider) = state.provider.as_ref()
    {
        let provider_status = provider.status().await;
        (
            provider_status.running,
            provider_status.jobs_processed,
            provider_status.total_earnings_msats,
        )
    } else {
        (false, 0, 0)
    };

    if running && state.started_at.is_none() {
        state.started_at = Some(std::time::Instant::now());
    }
    if !running {
        state.started_at = None;
    }

    PylonStatus {
        running,
        pid: None,
        uptime_secs: state.started_at.as_ref().map(|t| t.elapsed().as_secs()),
        provider_active: Some(running),
        host_active: Some(false),
        jobs_completed,
        earnings_msats,
        identity_exists,
        last_error: state.last_error.clone(),
    }
}

async fn fetch_dvm_provider_status(
    state: &mut InProcessPylon,
    config: &PylonConfig,
) -> DvmProviderStatus {
    let running = if let Some(provider) = state.provider.as_ref() {
        provider.status().await.running
    } else {
        false
    };

    DvmProviderStatus {
        running,
        provider_active: Some(running),
        host_active: Some(false),
        min_price_msats: config.min_price_msats,
        require_payment: config.require_payment,
        default_model: config.default_model.clone(),
        backend_preference: config.backend_preference.clone(),
        network: config.network.clone(),
        enable_payments: config.enable_payments,
        last_error: state.last_error.clone(),
    }
}

fn fetch_dvm_history() -> DvmHistorySnapshot {
    let mut snapshot = DvmHistorySnapshot::default();

    let config = match PylonConfig::load() {
        Ok(config) => config,
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to load Pylon config: {err}"));
            return snapshot;
        }
    };

    let data_dir = match config.data_path() {
        Ok(path) => path,
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to resolve Pylon data dir: {err}"));
            return snapshot;
        }
    };

    let path = data_dir.join("pylon.db");

    let db = match PylonDb::open(path) {
        Ok(db) => db,
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to open Pylon DB: {err}"));
            return snapshot;
        }
    };

    match db.get_earnings_summary() {
        Ok(summary) => {
            snapshot.summary.total_msats = summary.total_msats;
            snapshot.summary.total_sats = summary.total_sats;
            snapshot.summary.job_count = summary.job_count;
            let mut sources = summary
                .by_source
                .into_iter()
                .collect::<Vec<(String, u64)>>();
            sources.sort_by(|a, b| a.0.cmp(&b.0));
            snapshot.summary.by_source = sources;
        }
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to load earnings summary: {err}"));
        }
    }

    match db.count_jobs_by_status() {
        Ok(counts) => {
            let mut status_counts = counts
                .into_iter()
                .map(|(status, count)| (status.as_str().to_string(), count))
                .collect::<Vec<_>>();
            status_counts.sort_by(|a, b| a.0.cmp(&b.0));
            snapshot.status_counts = status_counts;
        }
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to load job counts: {err}"));
        }
    }

    let mut jobs = Vec::new();
    for status in [
        JobStatus::Completed,
        JobStatus::Failed,
        JobStatus::Processing,
        JobStatus::Pending,
    ] {
        if let Ok(list) = db.list_jobs_by_status(status, 25) {
            jobs.extend(list);
        }
    }
    jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    jobs.truncate(25);
    snapshot.jobs = jobs
        .into_iter()
        .map(|job| autopilot_app::DvmJobSummary {
            id: job.id,
            status: job.status.as_str().to_string(),
            kind: job.kind,
            price_msats: job.price_msats,
            created_at: job.created_at,
        })
        .collect();

    snapshot
}

fn spark_network_for_pylon(network: &str) -> SparkNetwork {
    match network.to_lowercase().as_str() {
        "mainnet" => SparkNetwork::Mainnet,
        "testnet" => SparkNetwork::Testnet,
        "signet" => SparkNetwork::Signet,
        _ => SparkNetwork::Regtest,
    }
}

async fn fetch_wallet_status() -> WalletStatus {
    let mut status = WalletStatus {
        network: None,
        spark_sats: 0,
        lightning_sats: 0,
        onchain_sats: 0,
        total_sats: 0,
        spark_address: None,
        bitcoin_address: None,
        identity_exists: false,
        last_error: None,
    };

    let config = match PylonConfig::load() {
        Ok(config) => config,
        Err(err) => {
            status.last_error = Some(format!("Failed to load Pylon config: {err}"));
            return status;
        }
    };

    status.network = Some(config.network.clone());

    let data_dir = match config.data_path() {
        Ok(path) => path,
        Err(err) => {
            status.last_error = Some(format!("Failed to resolve Pylon data dir: {err}"));
            return status;
        }
    };
    let identity_path = data_dir.join("identity.mnemonic");
    if !identity_path.exists() {
        status.identity_exists = false;
        status.last_error = Some(format!(
            "No identity found. Run 'pylon init' first. Expected: {}",
            identity_path.display()
        ));
        return status;
    }
    status.identity_exists = true;

    let mnemonic = match std::fs::read_to_string(&identity_path) {
        Ok(value) => value.trim().to_string(),
        Err(err) => {
            status.last_error = Some(format!("Failed to read identity: {err}"));
            return status;
        }
    };

    let signer = match SparkSigner::from_mnemonic(&mnemonic, "") {
        Ok(signer) => signer,
        Err(err) => {
            status.last_error = Some(format!("Failed to derive Spark signer: {err}"));
            return status;
        }
    };

    let wallet_config = WalletConfig {
        network: spark_network_for_pylon(&config.network),
        api_key: None,
        storage_dir: data_dir.join("spark"),
    };

    let wallet = match SparkWallet::new(signer, wallet_config).await {
        Ok(wallet) => wallet,
        Err(err) => {
            status.last_error = Some(format!("Failed to init Spark wallet: {err}"));
            return status;
        }
    };

    match wallet.get_balance().await {
        Ok(balance) => {
            status.spark_sats = balance.spark_sats;
            status.lightning_sats = balance.lightning_sats;
            status.onchain_sats = balance.onchain_sats;
            status.total_sats = balance.total_sats();
        }
        Err(err) => {
            status.last_error = Some(format!("Failed to fetch balance: {err}"));
            return status;
        }
    }

    match wallet.get_spark_address().await {
        Ok(address) => status.spark_address = Some(address),
        Err(err) => {
            status.last_error = Some(format!("Failed to fetch Spark address: {err}"));
        }
    }

    match wallet.get_bitcoin_address().await {
        Ok(address) => status.bitcoin_address = Some(address),
        Err(err) => {
            status.last_error = Some(format!("Failed to fetch Bitcoin address: {err}"));
        }
    }

    status
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
