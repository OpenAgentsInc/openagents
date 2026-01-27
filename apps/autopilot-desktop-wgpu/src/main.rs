use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use autopilot_app::{AppEvent, App as AutopilotApp, AppConfig, SessionId, UserAction, WorkspaceId};
use futures::StreamExt;
use tracing_subscriber::EnvFilter;
use wgpui::components::Text;
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Component, PaintContext, Quad, Scene, Size, TextSystem, theme};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

const WINDOW_TITLE: &str = "Autopilot Desktop (WGPUI)";
const WINDOW_WIDTH: f64 = 1280.0;
const WINDOW_HEIGHT: f64 = 800.0;
const PADDING: f32 = 48.0;
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
    spawn_event_bridge(proxy);
    let mut app = App::default();
    event_loop
        .run_app(&mut app)
        .context("event loop failed")?;
    Ok(())
}

struct App {
    state: Option<RenderState>,
    pending_events: Vec<AppEvent>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: None,
            pending_events: Vec::new(),
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

        match init_state(event_loop) {
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

fn init_state(event_loop: &ActiveEventLoop) -> Result<RenderState> {
    let window_attrs = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));

    let window = Arc::new(
        event_loop
            .create_window(window_attrs)
            .context("failed to create window")?,
    );

    let root = DesktopRoot::new();

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
    let content_bounds = inset_bounds(bounds, PADDING);

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

fn inset_bounds(bounds: Bounds, padding: f32) -> Bounds {
    let width = (bounds.size.width - padding * 2.0).max(0.0);
    let height = (bounds.size.height - padding * 2.0).max(0.0);
    Bounds::new(bounds.origin.x + padding, bounds.origin.y + padding, width, height)
}

fn spawn_event_bridge(proxy: EventLoopProxy<AppEvent>) {
    std::thread::spawn(move || {
        let app = AutopilotApp::new(AppConfig {
            event_buffer: EVENT_BUFFER,
        });

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let workspace = app.open_workspace(cwd);
        let mut stream = workspace.events();

        let session = workspace.start_session(Some("Bootstrap".to_string()));
        workspace.dispatch(UserAction::Message {
            session_id: session.session_id(),
            text: "Autopilot desktop WGPUI bootstrapped.".to_string(),
        });

        futures::executor::block_on(async move {
            while let Some(event) = stream.next().await {
                let _ = proxy.send_event(event);
            }
        });
    });
}

#[derive(Default, Clone)]
struct AppViewModel {
    workspace_id: Option<WorkspaceId>,
    workspace_path: Option<PathBuf>,
    session_id: Option<SessionId>,
    session_label: Option<String>,
    last_event: Option<String>,
    event_count: usize,
}

impl AppViewModel {
    fn apply_event(&mut self, event: &AppEvent) {
        self.event_count += 1;
        self.last_event = Some(format_event(event));

        match event {
            AppEvent::WorkspaceOpened { workspace_id, path } => {
                self.workspace_id = Some(*workspace_id);
                self.workspace_path = Some(path.clone());
            }
            AppEvent::SessionStarted { session_id, label, .. } => {
                self.session_id = Some(*session_id);
                self.session_label = label.clone();
            }
            AppEvent::UserActionDispatched { .. } => {}
        }
    }
}

struct DesktopRoot {
    view_model: AppViewModel,
    header: Text,
    body: Text,
}

impl DesktopRoot {
    fn new() -> Self {
        let mut root = Self {
            view_model: AppViewModel::default(),
            header: Text::new("Autopilot Desktop (WGPUI)")
                .font_size(30.0)
                .bold()
                .color(theme::text::PRIMARY),
            body: Text::new("Waiting for events...")
                .font_size(16.0)
                .color(theme::text::MUTED),
        };
        root.refresh_text();
        root
    }

    fn apply_event(&mut self, event: AppEvent) {
        self.view_model.apply_event(&event);
        self.refresh_text();
    }

    fn refresh_text(&mut self) {
        let workspace_line = self
            .view_model
            .workspace_path
            .as_ref()
            .map(|path| format!("Workspace: {}", path.display()))
            .unwrap_or_else(|| "Workspace: --".to_string());

        let session_line = match (self.view_model.session_id, &self.view_model.session_label) {
            (Some(id), Some(label)) => format!("Session: {:?} ({})", id, label),
            (Some(id), None) => format!("Session: {:?}", id),
            _ => "Session: --".to_string(),
        };

        let last_event_line = self
            .view_model
            .last_event
            .clone()
            .unwrap_or_else(|| "Last event: --".to_string());

        let count_line = format!("Event count: {}", self.view_model.event_count);

        let body = format!(
            "Immediate-mode view model (Zed/GPUI-style)\n{workspace}\n{session}\n{last_event}\n{count}",
            workspace = workspace_line,
            session = session_line,
            last_event = last_event_line,
            count = count_line
        );

        self.body.set_content(body);
    }
}

impl Component for DesktopRoot {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            48.0,
        );
        self.header.paint(header_bounds, cx);

        let body_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 64.0,
            bounds.size.width,
            bounds.size.height - 64.0,
        );
        let card = Quad::new(body_bounds)
            .with_background(theme::bg::SURFACE)
            .with_corner_radius(14.0);
        cx.scene.draw_quad(card);

        let inset = 24.0;
        let text_bounds = Bounds::new(
            body_bounds.origin.x + inset,
            body_bounds.origin.y + inset,
            (body_bounds.size.width - inset * 2.0).max(0.0),
            (body_bounds.size.height - inset * 2.0).max(0.0),
        );
        self.body.paint(text_bounds, cx);
    }
}

fn format_event(event: &AppEvent) -> String {
    match event {
        AppEvent::WorkspaceOpened { path, .. } => {
            format!("Last event: WorkspaceOpened ({})", path.display())
        }
        AppEvent::SessionStarted { session_id, .. } => {
            format!("Last event: SessionStarted ({:?})", session_id)
        }
        AppEvent::UserActionDispatched { action, .. } => match action {
            UserAction::Message { text, .. } => format!("Last event: Message ({})", text),
            UserAction::Command { name, .. } => format!("Last event: Command ({})", name),
        },
    }
}
