use std::sync::Arc;

use anyhow::{Context, Result};
use wgpui::components::hud::{
    DotShape, DotsGrid, DotsOrigin, Hotbar, HotbarSlot, PaneFrame, ResizablePane, ResizeEdge,
};
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, Easing, EventContext, Hsla, InputEvent, Modifiers, MouseButton,
    PaintContext, Point, Quad, Scene, Size, TextSystem, theme,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorIcon, Window, WindowId};

const WINDOW_TITLE: &str = "Autopilot";
const WINDOW_WIDTH: f64 = 1280.0;
const WINDOW_HEIGHT: f64 = 800.0;

const HOTBAR_HEIGHT: f32 = 52.0;
const HOTBAR_FLOAT_GAP: f32 = 18.0;
const HOTBAR_ITEM_SIZE: f32 = 36.0;
const HOTBAR_ITEM_GAP: f32 = 6.0;
const HOTBAR_PADDING: f32 = 6.0;
const HOTBAR_SLOT_NEW_CHAT: u8 = 1;

const GRID_DOT_DISTANCE: f32 = 32.0;
const PANE_TITLE_HEIGHT: f32 = 28.0;
const PANE_DEFAULT_WIDTH: f32 = 420.0;
const PANE_DEFAULT_HEIGHT: f32 = 280.0;
const PANE_MIN_WIDTH: f32 = 220.0;
const PANE_MIN_HEIGHT: f32 = 140.0;
const PANE_MARGIN: f32 = 18.0;
const PANE_CASCADE_X: f32 = 26.0;
const PANE_CASCADE_Y: f32 = 22.0;
const PANE_BOTTOM_RESERVED: f32 = HOTBAR_HEIGHT + HOTBAR_FLOAT_GAP + PANE_MARGIN;

fn main() -> Result<()> {
    let event_loop = EventLoop::new().context("failed to create event loop")?;
    let mut app = App::default();
    event_loop
        .run_app(&mut app)
        .context("event loop terminated with error")?;
    Ok(())
}

struct App {
    state: Option<RenderState>,
    cursor_position: Point,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: None,
            cursor_position: Point::ZERO,
        }
    }
}

#[derive(Clone, Copy)]
enum PaneDragMode {
    Moving {
        pane_id: u64,
        start_mouse: Point,
        start_bounds: Bounds,
    },
    Resizing {
        pane_id: u64,
        edge: ResizeEdge,
        start_mouse: Point,
        start_bounds: Bounds,
    },
}

struct DesktopPane {
    id: u64,
    title: String,
    bounds: Bounds,
    z_index: i32,
    frame: PaneFrame,
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
    hotbar: Hotbar,
    hotbar_bounds: Bounds,
    event_context: EventContext,
    panes: Vec<DesktopPane>,
    next_pane_id: u64,
    next_z_index: i32,
    pane_drag_mode: Option<PaneDragMode>,
    pane_resizer: ResizablePane,
    hotbar_flash_was_active: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        match init_state(event_loop) {
            Ok(state) => {
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(_err) => {
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
            WindowEvent::CursorMoved { position, .. } => {
                let scale = state.scale_factor.max(0.1);
                self.cursor_position =
                    Point::new(position.x as f32 / scale, position.y as f32 / scale);

                let mut needs_redraw = false;
                if update_drag(state, self.cursor_position) {
                    needs_redraw = true;
                }

                let pane_move_event = InputEvent::MouseMove {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y,
                };
                if dispatch_pane_frame_event(state, &pane_move_event) {
                    needs_redraw = true;
                }

                if state
                    .hotbar
                    .event(
                        &pane_move_event,
                        state.hotbar_bounds,
                        &mut state.event_context,
                    )
                    .is_handled()
                {
                    needs_redraw = true;
                }

                state
                    .window
                    .set_cursor(cursor_icon_for_pointer(state, self.cursor_position));

                if needs_redraw {
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

                let input = match mouse_state {
                    ElementState::Pressed => InputEvent::MouseDown {
                        button,
                        x: self.cursor_position.x,
                        y: self.cursor_position.y,
                        modifiers: Modifiers::default(),
                    },
                    ElementState::Released => InputEvent::MouseUp {
                        button,
                        x: self.cursor_position.x,
                        y: self.cursor_position.y,
                    },
                };

                match mouse_state {
                    ElementState::Pressed => {
                        let mut handled = false;
                        if state.hotbar_bounds.contains(self.cursor_position) {
                            handled |= state
                                .hotbar
                                .event(&input, state.hotbar_bounds, &mut state.event_context)
                                .is_handled();
                            handled |= process_hotbar_clicks(state);
                            if !handled {
                                handled |=
                                    handle_pane_mouse_down(state, self.cursor_position, button);
                            }
                        } else {
                            handled |= handle_pane_mouse_down(state, self.cursor_position, button);
                            handled |= state
                                .hotbar
                                .event(&input, state.hotbar_bounds, &mut state.event_context)
                                .is_handled();
                            handled |= process_hotbar_clicks(state);
                        }

                        state
                            .window
                            .set_cursor(cursor_icon_for_pointer(state, self.cursor_position));
                        if handled {
                            state.window.request_redraw();
                        }
                    }
                    ElementState::Released => {
                        let mut handled = handle_pane_mouse_up(state, &input);
                        handled |= state
                            .hotbar
                            .event(&input, state.hotbar_bounds, &mut state.event_context)
                            .is_handled();
                        handled |= process_hotbar_clicks(state);

                        state
                            .window
                            .set_cursor(cursor_icon_for_pointer(state, self.cursor_position));
                        if handled {
                            state.window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                if event.repeat {
                    return;
                }

                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => {
                        if let Some(pane_id) = active_pane_id(state) {
                            close_pane(state, pane_id);
                            state.window.request_redraw();
                        }
                    }
                    key => {
                        if let Some(slot) = hotbar_slot_for_key(key)
                            && slot == HOTBAR_SLOT_NEW_CHAT
                        {
                            activate_hotbar_slot(state, slot);
                            state.window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if render_frame(state).is_err() {
                    event_loop.exit();
                    return;
                }
                let flashing_now = state.hotbar.is_flashing();
                if flashing_now || state.hotbar_flash_was_active {
                    state.window.request_redraw();
                }
                state.hotbar_flash_was_active = flashing_now;
            }
            _ => {}
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
            .context("failed to find compatible adapter")?;

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

        let mut hotbar = Hotbar::new()
            .item_size(HOTBAR_ITEM_SIZE)
            .padding(HOTBAR_PADDING)
            .gap(HOTBAR_ITEM_GAP)
            .corner_radius(8.0)
            .font_scale(1.0);
        hotbar.set_items(build_hotbar_items());

        let initial_hotbar_bounds = hotbar_bounds(logical_size(&config, scale_factor));
        let mut state = RenderState {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            text_system,
            scale_factor,
            hotbar,
            hotbar_bounds: initial_hotbar_bounds,
            event_context: EventContext::new(),
            panes: Vec::new(),
            next_pane_id: 1,
            next_z_index: 1,
            pane_drag_mode: None,
            pane_resizer: ResizablePane::new().min_size(PANE_MIN_WIDTH, PANE_MIN_HEIGHT),
            hotbar_flash_was_active: false,
        };
        create_empty_pane(&mut state);
        Ok(state)
    })
}

fn render_frame(state: &mut RenderState) -> Result<()> {
    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let height = logical.height;
    let active_pane = active_pane_id(state);

    let mut scene = Scene::new();
    scene
        .draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    {
        let mut paint = PaintContext::new(&mut scene, &mut state.text_system, state.scale_factor);

        let mut dots_grid = DotsGrid::new()
            .color(Hsla::new(0.0, 0.0, 0.30, 0.26))
            .shape(DotShape::Cross)
            .distance(GRID_DOT_DISTANCE)
            .size(5.0)
            .cross_thickness(1.0)
            .origin(DotsOrigin::Center)
            .easing(Easing::EaseOut);
        dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut paint);

        paint_panes(&mut state.panes, active_pane, &mut paint);

        let bar_bounds = hotbar_bounds(logical);
        state.hotbar_bounds = bar_bounds;
        state.hotbar.set_item_size(HOTBAR_ITEM_SIZE);
        state.hotbar.set_padding(HOTBAR_PADDING);
        state.hotbar.set_gap(HOTBAR_ITEM_GAP);
        state.hotbar.set_corner_radius(8.0);
        state.hotbar.set_font_scale(1.0);
        state.hotbar.paint(bar_bounds, &mut paint);
    }

    state
        .renderer
        .resize(&state.queue, logical, state.scale_factor.max(0.1));

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
        Err(err) => return Err(anyhow::anyhow!("surface error: {err:?}")),
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Autopilot Render Encoder"),
        });

    state.renderer.prepare(
        &state.device,
        &state.queue,
        &scene,
        state.scale_factor.max(0.1),
    );
    state.renderer.render(&mut encoder, &view);
    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
}

fn paint_panes(panes: &mut [DesktopPane], active_id: Option<u64>, paint: &mut PaintContext) {
    let mut indices: Vec<usize> = (0..panes.len()).collect();
    indices.sort_by_key(|idx| panes[*idx].z_index);

    for idx in indices {
        let pane = &mut panes[idx];
        pane.frame.set_title(&pane.title);
        pane.frame.set_active(active_id == Some(pane.id));
        pane.frame.set_title_height(PANE_TITLE_HEIGHT);
        pane.frame.paint(pane.bounds, paint);

        let content_bounds = pane.frame.content_bounds();
        paint.scene.draw_quad(
            Quad::new(content_bounds)
                .with_background(theme::bg::SURFACE.with_alpha(0.48))
                .with_corner_radius(4.0),
        );

        let empty = paint.text.layout(
            "Empty pane",
            Point::new(
                content_bounds.origin.x + 12.0,
                content_bounds.origin.y + 16.0,
            ),
            12.0,
            theme::text::MUTED,
        );
        paint.scene.draw_text(empty);
    }
}

fn hotbar_bounds(size: Size) -> Bounds {
    let slot_count = hotbar_display_order().len();
    let bar_width = HOTBAR_PADDING * 2.0
        + HOTBAR_ITEM_SIZE * slot_count as f32
        + HOTBAR_ITEM_GAP * (slot_count.saturating_sub(1) as f32);
    let bar_x = size.width * 0.5 - bar_width * 0.5;
    let bar_y = size.height - HOTBAR_FLOAT_GAP - HOTBAR_HEIGHT;
    Bounds::new(bar_x, bar_y, bar_width, HOTBAR_HEIGHT)
}

fn process_hotbar_clicks(state: &mut RenderState) -> bool {
    let mut changed = false;
    for slot in state.hotbar.take_clicked_slots() {
        if slot == HOTBAR_SLOT_NEW_CHAT {
            activate_hotbar_slot(state, slot);
            changed = true;
        }
    }
    changed
}

fn activate_hotbar_slot(state: &mut RenderState, slot: u8) {
    state.hotbar.flash_slot(slot);
    create_empty_pane(state);
    state.hotbar_flash_was_active = true;
}

fn hotbar_display_order() -> [u8; 1] {
    [HOTBAR_SLOT_NEW_CHAT]
}

fn build_hotbar_items() -> Vec<HotbarSlot> {
    hotbar_display_order()
        .into_iter()
        .map(|slot| HotbarSlot::new(slot, "+", "New pane"))
        .collect()
}

fn hotbar_slot_for_key(key: PhysicalKey) -> Option<u8> {
    match key {
        PhysicalKey::Code(KeyCode::Digit1) | PhysicalKey::Code(KeyCode::Numpad1) => {
            Some(HOTBAR_SLOT_NEW_CHAT)
        }
        _ => None,
    }
}

fn create_empty_pane(state: &mut RenderState) {
    let id = state.next_pane_id;
    state.next_pane_id = state.next_pane_id.saturating_add(1);

    let logical = logical_size(&state.config, state.scale_factor);
    let tier = (id as usize - 1) % 10;
    let x = PANE_MARGIN + tier as f32 * PANE_CASCADE_X;
    let y = PANE_MARGIN + tier as f32 * PANE_CASCADE_Y;

    let bounds = clamp_bounds_to_window(
        Bounds::new(x, y, PANE_DEFAULT_WIDTH, PANE_DEFAULT_HEIGHT),
        logical,
    );

    let pane = DesktopPane {
        id,
        title: format!("Pane {id}"),
        bounds,
        z_index: state.next_z_index,
        frame: PaneFrame::new()
            .title(format!("Pane {id}"))
            .active(true)
            .dismissable(true)
            .title_height(PANE_TITLE_HEIGHT),
    };

    state.next_z_index = state.next_z_index.saturating_add(1);
    state.panes.push(pane);
}

fn handle_pane_mouse_down(state: &mut RenderState, point: Point, button: MouseButton) -> bool {
    if button != MouseButton::Left {
        return false;
    }

    for pane_idx in pane_indices_by_z_desc(state) {
        let pane_id = state.panes[pane_idx].id;
        let bounds = state.panes[pane_idx].bounds;

        let down_event = InputEvent::MouseDown {
            button,
            x: point.x,
            y: point.y,
            modifiers: Modifiers::default(),
        };
        if state.panes[pane_idx]
            .frame
            .event(&down_event, bounds, &mut state.event_context)
            .is_handled()
        {
            bring_pane_to_front(state, pane_id);
            return true;
        }

        let resize_edge = state.pane_resizer.edge_at(bounds, point);
        let title_bounds = pane_title_bounds(bounds);

        if resize_edge != ResizeEdge::None || bounds.contains(point) {
            bring_pane_to_front(state, pane_id);

            if resize_edge != ResizeEdge::None {
                state.pane_drag_mode = Some(PaneDragMode::Resizing {
                    pane_id,
                    edge: resize_edge,
                    start_mouse: point,
                    start_bounds: bounds,
                });
                return true;
            }

            if title_bounds.contains(point) {
                state.pane_drag_mode = Some(PaneDragMode::Moving {
                    pane_id,
                    start_mouse: point,
                    start_bounds: bounds,
                });
            }

            return true;
        }
    }

    false
}

fn handle_pane_mouse_up(state: &mut RenderState, event: &InputEvent) -> bool {
    let mut handled = false;
    let mut close_target: Option<u64> = None;

    for pane_idx in pane_indices_by_z_desc(state) {
        let bounds = state.panes[pane_idx].bounds;
        if state.panes[pane_idx]
            .frame
            .event(event, bounds, &mut state.event_context)
            .is_handled()
        {
            handled = true;
        }

        if state.panes[pane_idx].frame.take_close_clicked() {
            close_target = Some(state.panes[pane_idx].id);
            break;
        }
    }

    if let Some(pane_id) = close_target {
        close_pane(state, pane_id);
        handled = true;
    }

    if state.pane_drag_mode.take().is_some() {
        handled = true;
    }

    handled
}

fn dispatch_pane_frame_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let mut handled = false;
    for pane_idx in pane_indices_by_z_desc(state) {
        let bounds = state.panes[pane_idx].bounds;
        if state.panes[pane_idx]
            .frame
            .event(event, bounds, &mut state.event_context)
            .is_handled()
        {
            handled = true;
        }
    }
    handled
}

fn update_drag(state: &mut RenderState, current_mouse: Point) -> bool {
    let Some(mode) = state.pane_drag_mode else {
        return false;
    };

    let logical = logical_size(&state.config, state.scale_factor);

    match mode {
        PaneDragMode::Moving {
            pane_id,
            start_mouse,
            start_bounds,
        } => {
            let dx = current_mouse.x - start_mouse.x;
            let dy = current_mouse.y - start_mouse.y;

            if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
                let next = Bounds::new(
                    start_bounds.origin.x + dx,
                    start_bounds.origin.y + dy,
                    start_bounds.size.width,
                    start_bounds.size.height,
                );
                pane.bounds = clamp_bounds_to_window(next, logical);
                return true;
            }
        }
        PaneDragMode::Resizing {
            pane_id,
            edge,
            start_mouse,
            start_bounds,
        } => {
            if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
                let next = state.pane_resizer.resize_bounds(
                    edge,
                    start_bounds,
                    start_mouse,
                    current_mouse,
                );
                pane.bounds = clamp_bounds_to_window(next, logical);
                return true;
            }
        }
    }

    false
}

fn pane_indices_by_z_desc(state: &RenderState) -> Vec<usize> {
    let mut ordered: Vec<usize> = (0..state.panes.len()).collect();
    ordered.sort_by(|lhs, rhs| state.panes[*rhs].z_index.cmp(&state.panes[*lhs].z_index));
    ordered
}

fn bring_pane_to_front(state: &mut RenderState, pane_id: u64) {
    if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
        pane.z_index = state.next_z_index;
        state.next_z_index = state.next_z_index.saturating_add(1);
    }
}

fn close_pane(state: &mut RenderState, pane_id: u64) {
    state.panes.retain(|pane| pane.id != pane_id);
}

fn active_pane_id(state: &RenderState) -> Option<u64> {
    state
        .panes
        .iter()
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.id)
}

fn pane_title_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        PANE_TITLE_HEIGHT,
    )
}

fn cursor_icon_for_pointer(state: &RenderState, point: Point) -> CursorIcon {
    if let Some(mode) = state.pane_drag_mode {
        return match mode {
            PaneDragMode::Moving { .. } => CursorIcon::Move,
            PaneDragMode::Resizing { edge, .. } => cursor_icon_for_resize_edge(edge),
        };
    }

    if state.hotbar_bounds.contains(point) {
        return CursorIcon::Pointer;
    }

    for pane_idx in pane_indices_by_z_desc(state) {
        let bounds = state.panes[pane_idx].bounds;
        if !bounds.contains(point) {
            continue;
        }

        let edge = state.pane_resizer.edge_at(bounds, point);
        if edge != ResizeEdge::None {
            return cursor_icon_for_resize_edge(edge);
        }

        if pane_title_bounds(bounds).contains(point) {
            return CursorIcon::Move;
        }

        return CursorIcon::Default;
    }

    CursorIcon::Default
}

fn cursor_icon_for_resize_edge(edge: ResizeEdge) -> CursorIcon {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorIcon::NsResize,
        ResizeEdge::Left | ResizeEdge::Right => CursorIcon::EwResize,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorIcon::NwseResize,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorIcon::NeswResize,
        ResizeEdge::None => CursorIcon::Default,
    }
}

fn logical_size(config: &wgpu::SurfaceConfiguration, scale_factor: f32) -> Size {
    let scale = scale_factor.max(0.1);
    Size::new(config.width as f32 / scale, config.height as f32 / scale)
}

fn clamp_bounds_to_window(bounds: Bounds, window_size: Size) -> Bounds {
    let max_width = (window_size.width - PANE_MARGIN * 2.0).max(PANE_MIN_WIDTH);
    let width = bounds.size.width.clamp(PANE_MIN_WIDTH, max_width);

    let max_height = (window_size.height - PANE_MARGIN - PANE_BOTTOM_RESERVED).max(PANE_MIN_HEIGHT);
    let height = bounds.size.height.clamp(PANE_MIN_HEIGHT, max_height);

    let max_x = (window_size.width - width - PANE_MARGIN).max(PANE_MARGIN);
    let max_y = (window_size.height - height - PANE_BOTTOM_RESERVED).max(PANE_MARGIN);

    let x = bounds.origin.x.clamp(PANE_MARGIN, max_x);
    let y = bounds.origin.y.clamp(PANE_MARGIN, max_y);

    Bounds::new(x, y, width, height)
}
