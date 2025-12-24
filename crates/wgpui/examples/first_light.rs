//! First Light - Component System Demo
//!
//! Demonstrates all wgpui components:
//! - Div: Container with background and border
//! - Text: Styled text rendering (normal, bold, italic)
//! - Button: All variants (Primary, Secondary, Ghost, Danger)
//! - VirtualList: Efficient large list rendering

use std::sync::Arc;
use wgpui::{
    Bounds, Button, ButtonVariant, Component, Div, PaintContext, Point, Quad, Scene, Size, Text,
    TextSystem, VirtualList, theme,
};
use wgpui::renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
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
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("wgpui - Component Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 700));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

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

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
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
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;

                let mut scene = Scene::new();
                build_component_demo(
                    &mut scene,
                    &mut state.text_system,
                    width,
                    height,
                );

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

                state.renderer.resize(
                    &state.queue,
                    Size::new(width, height),
                    1.0,
                );

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                state.renderer.prepare(&state.device, &scene);
                state.renderer.render(&mut encoder, &view);

                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
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

fn build_component_demo(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    width: f32,
    _height: f32,
) {
    let margin = 20.0;
    let spacing = 16.0;
    let section_spacing = 24.0;
    let content_width = width - margin * 2.0;

    let mut y = margin;

    demo_text_component(scene, text_system, margin, content_width, spacing, &mut y);
    y += section_spacing;

    demo_button_component(scene, text_system, margin, &mut y);
    y += section_spacing;

    demo_div_component(scene, text_system, margin, content_width, spacing, &mut y);
    y += section_spacing;

    demo_virtual_list(scene, text_system, margin, content_width, &mut y);
    y += section_spacing;

    demo_theme_colors(scene, text_system, margin, &mut y);
}

fn demo_text_component(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    margin: f32,
    content_width: f32,
    spacing: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, margin, y, "Text Component");

    let mut text_normal = Text::new("Normal text - The quick brown fox jumps over the lazy dog");
    let mut text_bold = Text::new("Bold text - WGPUI GPU-accelerated rendering").bold();
    let mut text_italic = Text::new("Italic text - Beautiful typography").italic();
    let mut text_large = Text::new("Large text (24px)")
        .font_size(24.0)
        .color(theme::accent::PRIMARY);
    let mut text_muted = Text::new("Muted secondary text").color(theme::text::MUTED);

    let text_height = 24.0;
    let mut cx = PaintContext::new(scene, text_system, 1.0);

    text_normal.paint(Bounds::new(margin, *y, content_width, text_height), &mut cx);
    *y += text_height + spacing / 2.0;

    text_bold.paint(Bounds::new(margin, *y, content_width, text_height), &mut cx);
    *y += text_height + spacing / 2.0;

    text_italic.paint(Bounds::new(margin, *y, content_width, text_height), &mut cx);
    *y += text_height + spacing / 2.0;

    text_large.paint(Bounds::new(margin, *y, content_width, 32.0), &mut cx);
    *y += 32.0 + spacing / 2.0;

    text_muted.paint(Bounds::new(margin, *y, content_width, text_height), &mut cx);
    *y += text_height;
}

fn demo_button_component(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    margin: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, margin, y, "Button Component");

    let button_height = 36.0;
    let button_width = 140.0;
    let button_spacing = 12.0;

    let mut btn_primary = Button::new("Primary");
    let mut btn_secondary = Button::new("Secondary").variant(ButtonVariant::Secondary);
    let mut btn_ghost = Button::new("Ghost").variant(ButtonVariant::Ghost);
    let mut btn_danger = Button::new("Danger").variant(ButtonVariant::Danger);
    let mut btn_disabled = Button::new("Disabled").disabled(true);

    let mut cx = PaintContext::new(scene, text_system, 1.0);

    let mut x = margin;
    btn_primary.paint(Bounds::new(x, *y, button_width, button_height), &mut cx);
    x += button_width + button_spacing;

    btn_secondary.paint(Bounds::new(x, *y, button_width, button_height), &mut cx);
    x += button_width + button_spacing;

    btn_ghost.paint(Bounds::new(x, *y, button_width, button_height), &mut cx);
    x += button_width + button_spacing;

    btn_danger.paint(Bounds::new(x, *y, button_width, button_height), &mut cx);
    x += button_width + button_spacing;

    btn_disabled.paint(Bounds::new(x, *y, button_width, button_height), &mut cx);

    *y += button_height;
}

fn demo_div_component(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    margin: f32,
    content_width: f32,
    spacing: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, margin, y, "Div Container Component");

    let div_height = 60.0;
    let div_width = (content_width - spacing * 2.0) / 3.0;

    let mut div_bg = Div::new().background(theme::bg::SURFACE);
    let mut div_border = Div::new()
        .background(theme::bg::MUTED)
        .border(theme::border::DEFAULT, 1.0);
    let mut div_accent = Div::new()
        .background(theme::accent::PRIMARY.with_alpha(0.2))
        .border(theme::accent::PRIMARY, 2.0);

    let mut cx = PaintContext::new(scene, text_system, 1.0);

    div_bg.paint(Bounds::new(margin, *y, div_width, div_height), &mut cx);
    div_border.paint(
        Bounds::new(margin + div_width + spacing, *y, div_width, div_height),
        &mut cx,
    );
    div_accent.paint(
        Bounds::new(margin + (div_width + spacing) * 2.0, *y, div_width, div_height),
        &mut cx,
    );

    let mut label1 = Text::new("Surface bg").color(theme::text::MUTED).font_size(12.0);
    let mut label2 = Text::new("With border").color(theme::text::MUTED).font_size(12.0);
    let mut label3 = Text::new("Accent style").color(theme::text::MUTED).font_size(12.0);

    label1.paint(Bounds::new(margin + 8.0, *y + div_height / 2.0, 100.0, 20.0), &mut cx);
    label2.paint(
        Bounds::new(margin + div_width + spacing + 8.0, *y + div_height / 2.0, 100.0, 20.0),
        &mut cx,
    );
    label3.paint(
        Bounds::new(
            margin + (div_width + spacing) * 2.0 + 8.0,
            *y + div_height / 2.0,
            100.0,
            20.0,
        ),
        &mut cx,
    );

    *y += div_height;
}

fn demo_virtual_list(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    margin: f32,
    content_width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, margin, y, "VirtualList Component (100 items)");

    let list_height = 160.0;
    let item_height = 32.0;

    let items: Vec<String> = (0..100)
        .map(|i| format!("List Item #{} - Virtual scrolling demo", i))
        .collect();

    scene.draw_quad(
        Quad::new(Bounds::new(margin, *y, content_width, list_height))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let mut virtual_list = VirtualList::new(
        items,
        item_height,
        move |item: &String, idx: usize, bounds: Bounds, cx: &mut PaintContext| {
            let bg_color = if idx % 2 == 0 {
                theme::bg::SURFACE
            } else {
                theme::bg::MUTED
            };

            cx.scene.draw_quad(Quad::new(bounds).with_background(bg_color));

            let font_size = theme::font_size::SM;
            let text_run = cx.text.layout(
                item,
                Point::new(bounds.origin.x + 12.0, bounds.origin.y + bounds.size.height * 0.5 - font_size * 0.55),
                font_size,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(text_run);
        },
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    virtual_list.paint(Bounds::new(margin, *y, content_width, list_height), &mut cx);

    *y += list_height;
}

fn demo_theme_colors(scene: &mut Scene, text_system: &mut TextSystem, margin: f32, y: &mut f32) {
    draw_section_header(scene, text_system, margin, y, "Theme Colors");

    let swatch_size = 40.0;
    let swatch_spacing = 8.0;

    let bg_colors = [
        theme::bg::APP,
        theme::bg::SURFACE,
        theme::bg::MUTED,
    ];

    let mut x = margin;
    for color in bg_colors {
        scene.draw_quad(
            Quad::new(Bounds::new(x, *y, swatch_size, swatch_size))
                .with_background(color)
                .with_border(theme::border::DEFAULT, 1.0),
        );
        x += swatch_size + swatch_spacing;
    }

    let accent_colors = [
        theme::accent::PRIMARY,
        theme::accent::BLUE,
        theme::accent::GREEN,
        theme::accent::RED,
        theme::accent::PURPLE,
    ];

    x += swatch_spacing * 2.0;
    for color in accent_colors {
        scene.draw_quad(
            Quad::new(Bounds::new(x, *y, swatch_size, swatch_size)).with_background(color),
        );
        x += swatch_size + swatch_spacing;
    }

    let status_colors = [
        theme::status::SUCCESS,
        theme::status::WARNING,
        theme::status::ERROR,
        theme::status::INFO,
    ];

    x += swatch_spacing * 2.0;
    for color in status_colors {
        scene.draw_quad(
            Quad::new(Bounds::new(x, *y, swatch_size, swatch_size)).with_background(color),
        );
        x += swatch_size + swatch_spacing;
    }

    *y += swatch_size;
}

fn draw_section_header(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    margin: f32,
    y: &mut f32,
    title: &str,
) {
    let header_height = 28.0;

    let mut header_text = Text::new(title)
        .font_size(theme::font_size::LG)
        .bold()
        .color(theme::text::PRIMARY);

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    header_text.paint(Bounds::new(margin, *y, 400.0, header_height), &mut cx);

    *y += header_height + 8.0;

    scene.draw_quad(
        Quad::new(Bounds::new(margin, *y, 200.0, 2.0)).with_background(theme::accent::PRIMARY),
    );

    *y += 12.0;
}
