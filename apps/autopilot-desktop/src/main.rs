use std::sync::Arc;

use anyhow::{Context, Result};
use wgpui::components::hud::{DotShape, DotsGrid, DotsOrigin, Hotbar, HotbarSlot};
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, Easing, EventContext, EventResult, Hsla, InputEvent, Modifiers,
    MouseButton, PaintContext, Point, Quad, Scene, Size, TextSystem, theme,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const WINDOW_TITLE: &str = "Autopilot";
const WINDOW_WIDTH: f64 = 1280.0;
const WINDOW_HEIGHT: f64 = 800.0;

const HOTBAR_HEIGHT: f32 = 52.0;
const HOTBAR_FLOAT_GAP: f32 = 18.0;
const HOTBAR_ITEM_SIZE: f32 = 36.0;
const HOTBAR_ITEM_GAP: f32 = 6.0;
const HOTBAR_PADDING: f32 = 6.0;

const HOTBAR_SLOT_EVENTS: u8 = 0;
const HOTBAR_SLOT_NEW_CHAT: u8 = 1;
const HOTBAR_SLOT_IDENTITY: u8 = 2;
const HOTBAR_SLOT_WALLET: u8 = 3;
const HOTBAR_SLOT_THREADS: u8 = 4;
const HOTBAR_SLOT_AUTH: u8 = 5;
const HOTBAR_SLOT_INBOX: u8 = 6;
const HOTBAR_SLOT_CHAT_A: u8 = 7;
const HOTBAR_SLOT_CHAT_B: u8 = 8;
const HOTBAR_SLOT_CHAT_C: u8 = 9;

const GRID_DOT_DISTANCE: f32 = 32.0;

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

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    scale_factor: f32,
    selected_hotbar_slot: u8,
    hotbar: Hotbar,
    hotbar_bounds: Bounds,
    event_context: EventContext,
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
                self.cursor_position = Point::new(position.x as f32 / scale, position.y as f32 / scale);
                let input = InputEvent::MouseMove {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y,
                };
                let result = state
                    .hotbar
                    .event(&input, state.hotbar_bounds, &mut state.event_context);
                if result.is_handled() {
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

                let result = state
                    .hotbar
                    .event(&input, state.hotbar_bounds, &mut state.event_context);
                let changed = process_hotbar_clicks(state);
                if result == EventResult::Handled || changed {
                    state.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                    key => {
                        if let Some(slot) = hotbar_slot_for_key(key) {
                            state.hotbar.flash_slot(slot);
                            let _ = activate_hotbar_slot(state, slot);
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
                if state.hotbar.is_flashing() {
                    state.window.request_redraw();
                }
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

        let selected_hotbar_slot = HOTBAR_SLOT_NEW_CHAT;
        let mut hotbar = Hotbar::new()
            .item_size(HOTBAR_ITEM_SIZE)
            .padding(HOTBAR_PADDING)
            .gap(HOTBAR_ITEM_GAP)
            .corner_radius(8.0)
            .font_scale(1.0);
        hotbar.set_items(build_hotbar_items(selected_hotbar_slot));

        Ok(RenderState {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            text_system,
            scale_factor,
            selected_hotbar_slot,
            hotbar,
            hotbar_bounds: Bounds::ZERO,
            event_context: EventContext::new(),
        })
    })
}

fn render_frame(state: &mut RenderState) -> Result<()> {
    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let height = logical.height;

    let mut scene = Scene::new();
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

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

        let bar_bounds = hotbar_bounds(logical);
        state.hotbar_bounds = bar_bounds;
        state.hotbar.set_item_size(HOTBAR_ITEM_SIZE);
        state.hotbar.set_padding(HOTBAR_PADDING);
        state.hotbar.set_gap(HOTBAR_ITEM_GAP);
        state.hotbar.set_corner_radius(8.0);
        state.hotbar.set_font_scale(1.0);
        state.hotbar.paint(bar_bounds, &mut paint);
    }

    let title = state.text_system.layout(
        "Autopilot Desktop",
        Point::new(20.0, 18.0),
        18.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(title);

    let subtitle = state.text_system.layout(
        "MVP shell: grid + hotbar (window panes removed)",
        Point::new(20.0, 38.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle);

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

    state
        .renderer
        .prepare(&state.device, &state.queue, &scene, state.scale_factor.max(0.1));
    state.renderer.render(&mut encoder, &view);
    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
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
        state.hotbar.flash_slot(slot);
        changed |= activate_hotbar_slot(state, slot);
    }
    changed
}

fn activate_hotbar_slot(state: &mut RenderState, slot: u8) -> bool {
    if !is_selectable_slot(slot) {
        return false;
    }
    if state.selected_hotbar_slot == slot {
        return false;
    }

    state.selected_hotbar_slot = slot;
    state.hotbar.set_items(build_hotbar_items(slot));
    true
}

fn is_selectable_slot(slot: u8) -> bool {
    matches!(
        slot,
        HOTBAR_SLOT_NEW_CHAT
            | HOTBAR_SLOT_IDENTITY
            | HOTBAR_SLOT_WALLET
            | HOTBAR_SLOT_THREADS
            | HOTBAR_SLOT_AUTH
            | HOTBAR_SLOT_INBOX
            | HOTBAR_SLOT_EVENTS
    )
}

fn hotbar_display_order() -> [u8; 10] {
    [
        HOTBAR_SLOT_NEW_CHAT,
        HOTBAR_SLOT_IDENTITY,
        HOTBAR_SLOT_WALLET,
        HOTBAR_SLOT_THREADS,
        HOTBAR_SLOT_AUTH,
        HOTBAR_SLOT_INBOX,
        HOTBAR_SLOT_CHAT_A,
        HOTBAR_SLOT_CHAT_B,
        HOTBAR_SLOT_CHAT_C,
        HOTBAR_SLOT_EVENTS,
    ]
}

fn build_hotbar_items(selected_slot: u8) -> Vec<HotbarSlot> {
    hotbar_display_order()
        .into_iter()
        .map(|slot| {
            let mut item = match slot {
                HOTBAR_SLOT_NEW_CHAT => HotbarSlot::new(slot, "+", "New chat"),
                HOTBAR_SLOT_IDENTITY => HotbarSlot::new(slot, "ID", "Identity"),
                HOTBAR_SLOT_WALLET => HotbarSlot::new(slot, "WL", "Wallet"),
                HOTBAR_SLOT_THREADS => HotbarSlot::new(slot, "TH", "Threads"),
                HOTBAR_SLOT_AUTH => HotbarSlot::new(slot, "AU", "Auth"),
                HOTBAR_SLOT_INBOX => HotbarSlot::new(slot, "IN", "Inbox"),
                HOTBAR_SLOT_CHAT_A => HotbarSlot::new(slot, "", "Slot 7").ghost(true),
                HOTBAR_SLOT_CHAT_B => HotbarSlot::new(slot, "", "Slot 8").ghost(true),
                HOTBAR_SLOT_CHAT_C => HotbarSlot::new(slot, "", "Slot 9").ghost(true),
                HOTBAR_SLOT_EVENTS => HotbarSlot::new(slot, "EV", "Events"),
                _ => HotbarSlot::new(slot, "", format!("Slot {slot}")),
            };
            if is_selectable_slot(slot) && slot == selected_slot {
                item = item.active(true);
            }
            item
        })
        .collect()
}

fn logical_size(config: &wgpu::SurfaceConfiguration, scale_factor: f32) -> Size {
    let scale = scale_factor.max(0.1);
    Size::new(config.width as f32 / scale, config.height as f32 / scale)
}

fn hotbar_slot_for_key(key: PhysicalKey) -> Option<u8> {
    match key {
        PhysicalKey::Code(KeyCode::Digit1) | PhysicalKey::Code(KeyCode::Numpad1) => {
            Some(HOTBAR_SLOT_NEW_CHAT)
        }
        PhysicalKey::Code(KeyCode::Digit2) | PhysicalKey::Code(KeyCode::Numpad2) => {
            Some(HOTBAR_SLOT_IDENTITY)
        }
        PhysicalKey::Code(KeyCode::Digit3) | PhysicalKey::Code(KeyCode::Numpad3) => {
            Some(HOTBAR_SLOT_WALLET)
        }
        PhysicalKey::Code(KeyCode::Digit4) | PhysicalKey::Code(KeyCode::Numpad4) => {
            Some(HOTBAR_SLOT_THREADS)
        }
        PhysicalKey::Code(KeyCode::Digit5) | PhysicalKey::Code(KeyCode::Numpad5) => {
            Some(HOTBAR_SLOT_AUTH)
        }
        PhysicalKey::Code(KeyCode::Digit6) | PhysicalKey::Code(KeyCode::Numpad6) => {
            Some(HOTBAR_SLOT_INBOX)
        }
        PhysicalKey::Code(KeyCode::Digit7) | PhysicalKey::Code(KeyCode::Numpad7) => {
            Some(HOTBAR_SLOT_CHAT_A)
        }
        PhysicalKey::Code(KeyCode::Digit8) | PhysicalKey::Code(KeyCode::Numpad8) => {
            Some(HOTBAR_SLOT_CHAT_B)
        }
        PhysicalKey::Code(KeyCode::Digit9) | PhysicalKey::Code(KeyCode::Numpad9) => {
            Some(HOTBAR_SLOT_CHAT_C)
        }
        PhysicalKey::Code(KeyCode::Digit0) | PhysicalKey::Code(KeyCode::Numpad0) => {
            Some(HOTBAR_SLOT_EVENTS)
        }
        _ => None,
    }
}
