use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Result;
use wgpui::components::hud::{DotsGrid, DotsOrigin, DotShape, DrawDirection, Frame, FrameAnimation};
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, Key, Modifiers, MouseButton,
    NamedKey, PaintContext, Point, Quad, Scene, Size, Text, TextSystem, theme,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::{Window, WindowId};

use crate::backend::{BackendConfig, BackendEvent, BackendHandle, start_backend};
use crate::state::AppState;
use crate::views::{ChatView, ContextView, DashboardView, ParallelView};

pub fn run() -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let backend = start_backend(BackendConfig::default());
    let mut app = GuiApp::new(backend);

    event_loop.run_app(&mut app)?;
    Ok(())
}

struct GuiApp {
    state: Option<RenderState>,
    backend: BackendHandle,
    modifiers: ModifiersState,
    cursor_position: Point,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    ui: AutopilotUi,
}

impl GuiApp {
    fn new(backend: BackendHandle) -> Self {
        Self {
            state: None,
            backend,
            modifiers: ModifiersState::default(),
            cursor_position: Point::ZERO,
        }
    }

    fn apply_backend_events(backend: &BackendHandle, ui: &mut AutopilotUi) -> bool {
        let mut updated = false;
        while let Ok(event) = backend.receiver.try_recv() {
            ui.apply_backend_event(event);
            updated = true;
        }
        updated
    }
}

impl ApplicationHandler for GuiApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("OpenAgents Autopilot")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 820));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("failed to create window"),
        );

        let state = pollster::block_on(init_render_state(window));
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
                        label: Some("Render Encoder"),
                    });

                state.renderer.prepare(&state.device, &scene);
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
                let handled = state.ui.handle_input(&input_event, bounds);
                if handled {
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state: mouse_state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
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
                let handled = state.ui.handle_input(&input_event, bounds);
                if handled {
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
                let handled = state.ui.handle_input(&input_event, bounds);
                if handled {
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
                let handled = state.ui.handle_input(&input_event, bounds);
                if handled {
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            if Self::apply_backend_events(&self.backend, &mut state.ui) {
                state.window.request_redraw();
            }
        }
    }
}

async fn init_render_state(window: Arc<Window>) -> RenderState {
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

    RenderState {
        window,
        surface,
        device,
        queue,
        config,
        renderer,
        text_system,
        ui: AutopilotUi::new(scale_factor),
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
    Bounds::new(
        0.0,
        0.0,
        config.width as f32,
        config.height as f32,
    )
}

struct AutopilotUi {
    state: Rc<RefCell<AppState>>,
    shell: Shell,
    event_context: EventContext,
    scale_factor: f32,
}

impl AutopilotUi {
    fn new(scale_factor: f32) -> Self {
        let state = Rc::new(RefCell::new(AppState::new()));
        let shell = Shell::new(state.clone());
        Self {
            state,
            shell,
            event_context: EventContext::new(),
            scale_factor,
        }
    }

    fn paint(&mut self, bounds: Bounds, scene: &mut Scene, text: &mut TextSystem) {
        let mut cx = PaintContext::new(scene, text, self.scale_factor);
        self.shell.paint(bounds, &mut cx);
    }

    fn handle_input(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let result = self
            .shell
            .event(event, bounds, &mut self.event_context);
        matches!(result, EventResult::Handled)
    }

    fn apply_backend_event(&mut self, event: BackendEvent) {
        let mut state = self.state.borrow_mut();
        match event {
            BackendEvent::Metrics { sessions, summary } => {
                state.sessions = sessions;
                state.summary = summary;
            }
            BackendEvent::Logs {
                path,
                session_id,
                lines,
            } => {
                state.log_path = path;
                state.log_session_id = session_id;
                state.log_lines = lines;
            }
            BackendEvent::Agents { agents } => {
                state.agents = agents;
            }
            BackendEvent::Status { message } => {
                state.set_status(Some(message));
            }
        }
    }

    fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }
}

const HEADER_HEIGHT: f32 = 56.0;
const MARGIN: f32 = 24.0;
const GAP: f32 = 18.0;
const PANE_PADDING: f32 = 16.0;
const PANE_TITLE_HEIGHT: f32 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaneKind {
    Dashboard,
    Parallel,
    Chat,
    Context,
}

impl PaneKind {
    fn label(&self) -> &'static str {
        match self {
            PaneKind::Dashboard => "Dashboard",
            PaneKind::Parallel => "Parallel Agents",
            PaneKind::Chat => "Live Stream",
            PaneKind::Context => "Context",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PaneLayout {
    kind: PaneKind,
    frame: Bounds,
    content: Bounds,
}

struct Shell {
    state: Rc<RefCell<AppState>>,
    dashboard: DashboardView,
    chat: ChatView,
    context: ContextView,
    parallel: ParallelView,
    hovered_pane: Option<PaneKind>,
    dots: DotsGrid,
}

impl Shell {
    fn new(state: Rc<RefCell<AppState>>) -> Self {
        let dots = DotsGrid::new()
            .color(Hsla::new(0.0, 0.0, 0.3, 0.25))
            .shape(DotShape::Cross)
            .distance(28.0)
            .size(5.0)
            .cross_thickness(1.0)
            .origin(DotsOrigin::Center)
            .animation_progress(1.0);

        Self {
            dashboard: DashboardView::new(state.clone()),
            chat: ChatView::new(state.clone()),
            context: ContextView::new(state.clone()),
            parallel: ParallelView::new(state.clone()),
            state,
            hovered_pane: None,
            dots,
        }
    }

    fn pane_view_mut(&mut self, kind: PaneKind) -> &mut dyn Component {
        match kind {
            PaneKind::Dashboard => &mut self.dashboard,
            PaneKind::Parallel => &mut self.parallel,
            PaneKind::Chat => &mut self.chat,
            PaneKind::Context => &mut self.context,
        }
    }

    fn pane_content(frame: Bounds) -> Bounds {
        let width = (frame.size.width - PANE_PADDING * 2.0).max(0.0);
        let height = (frame.size.height - PANE_PADDING * 2.0 - PANE_TITLE_HEIGHT).max(0.0);
        Bounds::new(
            frame.origin.x + PANE_PADDING,
            frame.origin.y + PANE_PADDING + PANE_TITLE_HEIGHT,
            width,
            height,
        )
    }

    fn pane_layouts(&self, bounds: Bounds) -> [PaneLayout; 4] {
        let header_height = HEADER_HEIGHT.min(bounds.size.height);
        let available_width = (bounds.size.width - MARGIN * 2.0).max(0.0);
        let available_height =
            (bounds.size.height - header_height - MARGIN * 2.0).max(0.0);

        let left_width = (available_width * 0.62).max(0.0);
        let right_width = (available_width - left_width - GAP).max(0.0);
        let top_height = (available_height * 0.38).max(0.0);
        let bottom_height = (available_height - top_height - GAP).max(0.0);

        let origin_x = bounds.origin.x + MARGIN;
        let origin_y = bounds.origin.y + header_height + MARGIN;

        let dashboard_frame = Bounds::new(origin_x, origin_y, left_width, top_height);
        let parallel_frame = Bounds::new(
            origin_x + left_width + GAP,
            origin_y,
            right_width,
            top_height,
        );
        let chat_frame = Bounds::new(
            origin_x,
            origin_y + top_height + GAP,
            left_width,
            bottom_height,
        );
        let context_frame = Bounds::new(
            origin_x + left_width + GAP,
            origin_y + top_height + GAP,
            right_width,
            bottom_height,
        );

        [
            PaneLayout {
                kind: PaneKind::Dashboard,
                frame: dashboard_frame,
                content: Self::pane_content(dashboard_frame),
            },
            PaneLayout {
                kind: PaneKind::Parallel,
                frame: parallel_frame,
                content: Self::pane_content(parallel_frame),
            },
            PaneLayout {
                kind: PaneKind::Chat,
                frame: chat_frame,
                content: Self::pane_content(chat_frame),
            },
            PaneLayout {
                kind: PaneKind::Context,
                frame: context_frame,
                content: Self::pane_content(context_frame),
            },
        ]
    }

    fn paint_pane(&mut self, layout: PaneLayout, cx: &mut PaintContext) {
        if layout.frame.size.width <= 1.0 || layout.frame.size.height <= 1.0 {
            return;
        }

        let hovered = self.hovered_pane == Some(layout.kind);
        let base_line = Hsla::new(0.0, 0.0, 1.0, 0.75);
        let accent = theme::accent::PRIMARY;
        let line_color = if hovered { accent } else { base_line };
        let glow = if hovered { Some(accent.with_alpha(0.35)) } else { None };
        let bg_color = Hsla::new(0.0, 0.0, 0.07, 0.85);

        let mut frame = Frame::kranox()
            .line_color(line_color)
            .bg_color(bg_color)
            .stroke_width(2.0)
            .small_line_length(10.0)
            .large_line_length(36.0)
            .square_size(10.0)
            .animation_mode(FrameAnimation::Fade)
            .draw_direction(DrawDirection::CenterOut)
            .animation_progress(1.0);

        if let Some(glow) = glow {
            frame = frame.glow_color(glow);
        }

        frame.paint(layout.frame, cx);

        let mut title = Text::new(layout.kind.label())
            .font_size(theme::font_size::SM)
            .color(line_color);
        title.paint(
            Bounds::new(
                layout.frame.origin.x + PANE_PADDING,
                layout.frame.origin.y + PANE_PADDING * 0.6,
                layout.frame.size.width - PANE_PADDING * 2.0,
                PANE_TITLE_HEIGHT,
            ),
            cx,
        );

        self.pane_view_mut(layout.kind)
            .paint(layout.content, cx);
    }
}

impl Component for Shell {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(Quad::new(bounds).with_background(theme::bg::APP));

        let header_height = HEADER_HEIGHT.min(bounds.size.height);
        let grid_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + header_height,
            bounds.size.width,
            (bounds.size.height - header_height).max(0.0),
        );

        self.dots.paint(grid_bounds, cx);

        let mut title = Text::new("Autopilot Control Room")
            .font_size(theme::font_size::LG)
            .color(theme::text::PRIMARY);
        title.paint(
            Bounds::new(
                bounds.origin.x + MARGIN,
                bounds.origin.y + 16.0,
                bounds.size.width - MARGIN * 2.0,
                header_height,
            ),
            cx,
        );

        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y + header_height,
                bounds.size.width,
                1.0,
            ))
            .with_background(theme::accent::PRIMARY.with_alpha(0.45)),
        );

        for layout in self.pane_layouts(bounds) {
            self.paint_pane(layout, cx);
        }

        let footer = "Permissions: auto-approved";
        let mut footer_text = Text::new(footer)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        footer_text.paint(
            Bounds::new(
                bounds.origin.x + MARGIN,
                bounds.origin.y + bounds.size.height - MARGIN * 0.6,
                bounds.size.width - MARGIN * 2.0,
                theme::font_size::XS * 1.2,
            ),
            cx,
        );

        if let Some(status) = self.state.borrow().status_message.as_ref() {
            let mut status_text = Text::new(status)
                .font_size(theme::font_size::XS)
                .color(theme::status::WARNING);
            status_text.paint(
                Bounds::new(
                    bounds.origin.x + MARGIN,
                    bounds.origin.y + bounds.size.height - MARGIN * 1.4,
                    bounds.size.width - MARGIN * 2.0,
                    theme::font_size::XS * 1.2,
                ),
                cx,
            );
        }
    }

    fn event(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        let layouts = self.pane_layouts(bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let hovered = layouts
                    .iter()
                    .find(|pane| pane.frame.contains(point))
                    .map(|pane| pane.kind);

                if hovered != self.hovered_pane {
                    self.hovered_pane = hovered;
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { x, y, .. } | InputEvent::MouseUp { x, y, .. } => {
                let point = Point::new(*x, *y);
                if let Some(pane) = layouts.iter().find(|pane| pane.frame.contains(point)) {
                    return self
                        .pane_view_mut(pane.kind)
                        .event(event, pane.content, cx);
                }
            }
            InputEvent::Scroll { .. } => {
                let target = self.hovered_pane.unwrap_or(PaneKind::Chat);
                if let Some(pane) = layouts.iter().find(|pane| pane.kind == target) {
                    return self
                        .pane_view_mut(pane.kind)
                        .event(event, pane.content, cx);
                }
            }
            InputEvent::KeyDown { .. } | InputEvent::KeyUp { .. } => {
                if let Some(pane) = layouts.iter().find(|pane| pane.kind == PaneKind::Chat) {
                    return self
                        .pane_view_mut(pane.kind)
                        .event(event, pane.content, cx);
                }
            }
        }

        EventResult::Ignored
    }
}
