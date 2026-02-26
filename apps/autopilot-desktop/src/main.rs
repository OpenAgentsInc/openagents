use std::sync::Arc;

use anyhow::{Context, Result};
use wgpui::components::hud::{DotShape, DotsGrid, DotsOrigin};
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, Easing, Hsla, PaintContext, Point, Quad, Scene, Size, TextSystem, theme,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const WINDOW_TITLE: &str = "Autopilot";
const WINDOW_WIDTH: f64 = 1280.0;
const WINDOW_HEIGHT: f64 = 800.0;
const HOTBAR_SLOT_MAX: u8 = 9;
const HOTBAR_HEIGHT: f32 = 82.0;
const HOTBAR_PADDING_X: f32 = 18.0;
const HOTBAR_PADDING_Y: f32 = 14.0;
const HOTBAR_SLOT_GAP: f32 = 8.0;

fn main() -> Result<()> {
    let event_loop = EventLoop::new().context("failed to create event loop")?;
    let mut app = App::default();
    event_loop
        .run_app(&mut app)
        .context("event loop terminated with error")?;
    Ok(())
}

#[derive(Default)]
struct App {
    state: Option<RenderState>,
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
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                    key => {
                        if let Some(slot) = hotbar_slot_for_key(key) {
                            state.selected_hotbar_slot = slot;
                            state.window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if render_frame(state).is_err() {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
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

        Ok(RenderState {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            text_system,
            scale_factor,
            selected_hotbar_slot: 1,
        })
    })
}

fn render_frame(state: &mut RenderState) -> Result<()> {
    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let height = logical.height;

    let mut scene = Scene::new();
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let grid_bounds = Bounds::new(0.0, 0.0, width, (height - HOTBAR_HEIGHT).max(0.0));
    {
        let mut paint = PaintContext::new(&mut scene, &mut state.text_system, state.scale_factor);
        let mut dots_grid = DotsGrid::new()
            .color(Hsla::new(0.0, 0.0, 0.30, 0.26))
            .shape(DotShape::Cross)
            .distance(30.0)
            .size(5.0)
            .cross_thickness(1.0)
            .origin(DotsOrigin::Center)
            .easing(Easing::EaseOut);
        dots_grid.paint(grid_bounds, &mut paint);
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

    paint_hotbar(
        &mut scene,
        &mut state.text_system,
        width,
        height,
        state.selected_hotbar_slot,
    );

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

fn paint_hotbar(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    width: f32,
    height: f32,
    selected_slot: u8,
) {
    let bar_bounds = Bounds::new(0.0, height - HOTBAR_HEIGHT, width, HOTBAR_HEIGHT);
    scene.draw_quad(Quad::new(bar_bounds).with_background(Hsla::new(0.0, 0.0, 0.06, 0.96)));
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, bar_bounds.origin.y, width, 1.0))
            .with_background(theme::accent::PRIMARY.with_alpha(0.38)),
    );

    let slot_count = usize::from(HOTBAR_SLOT_MAX) + 1;
    let total_gap = HOTBAR_SLOT_GAP * (slot_count as f32 - 1.0);
    let available_width = (width - HOTBAR_PADDING_X * 2.0 - total_gap).max(10.0);
    let slot_width = available_width / slot_count as f32;
    let slot_height = HOTBAR_HEIGHT - HOTBAR_PADDING_Y * 2.0;
    let slot_y = bar_bounds.origin.y + HOTBAR_PADDING_Y;

    for slot in 0..=HOTBAR_SLOT_MAX {
        let idx = slot as f32;
        let x = HOTBAR_PADDING_X + idx * (slot_width + HOTBAR_SLOT_GAP);
        let is_selected = slot == selected_slot;
        let bg = if is_selected {
            theme::accent::PRIMARY.with_alpha(0.25)
        } else {
            Hsla::new(0.0, 0.0, 0.12, 0.90)
        };
        let border = if is_selected {
            theme::accent::PRIMARY
        } else {
            Hsla::new(0.0, 0.0, 0.35, 0.85)
        };

        scene.draw_quad(
            Quad::new(Bounds::new(x, slot_y, slot_width, slot_height))
                .with_background(bg)
                .with_border(border, 1.0)
                .with_corner_radius(8.0),
        );

        let label = if slot == 0 {
            "0".to_string()
        } else {
            slot.to_string()
        };
        let text_x = x + slot_width * 0.5 - 4.0;
        let text_y = slot_y + slot_height * 0.5 - 6.0;
        let run = text_system.layout(
            &label,
            Point::new(text_x, text_y),
            12.0,
            if is_selected {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            },
        );
        scene.draw_text(run);
    }

    let helper_text = text_system.layout(
        "Press 0-9 to select hotbar slot",
        Point::new(20.0, bar_bounds.origin.y - 8.0),
        11.0,
        theme::text::MUTED,
    );
    scene.draw_text(helper_text);
}

fn logical_size(config: &wgpu::SurfaceConfiguration, scale_factor: f32) -> Size {
    let scale = scale_factor.max(0.1);
    Size::new(config.width as f32 / scale, config.height as f32 / scale)
}

fn hotbar_slot_for_key(key: PhysicalKey) -> Option<u8> {
    match key {
        PhysicalKey::Code(KeyCode::Digit1) | PhysicalKey::Code(KeyCode::Numpad1) => Some(1),
        PhysicalKey::Code(KeyCode::Digit2) | PhysicalKey::Code(KeyCode::Numpad2) => Some(2),
        PhysicalKey::Code(KeyCode::Digit3) | PhysicalKey::Code(KeyCode::Numpad3) => Some(3),
        PhysicalKey::Code(KeyCode::Digit4) | PhysicalKey::Code(KeyCode::Numpad4) => Some(4),
        PhysicalKey::Code(KeyCode::Digit5) | PhysicalKey::Code(KeyCode::Numpad5) => Some(5),
        PhysicalKey::Code(KeyCode::Digit6) | PhysicalKey::Code(KeyCode::Numpad6) => Some(6),
        PhysicalKey::Code(KeyCode::Digit7) | PhysicalKey::Code(KeyCode::Numpad7) => Some(7),
        PhysicalKey::Code(KeyCode::Digit8) | PhysicalKey::Code(KeyCode::Numpad8) => Some(8),
        PhysicalKey::Code(KeyCode::Digit9) | PhysicalKey::Code(KeyCode::Numpad9) => Some(9),
        PhysicalKey::Code(KeyCode::Digit0) | PhysicalKey::Code(KeyCode::Numpad0) => Some(0),
        _ => None,
    }
}
