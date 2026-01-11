//! First Light - Full Component System Demo
//!
//! Demonstrates all wgpui components including:
//! - Animation system (easing, spring physics)
//! - Atoms (StatusDot, ModeBadge, ModelBadge, StreamingIndicator)
//! - Molecules (MessageHeader, ModeSelector, ModelSelector)
//! - HUD (StatusBar, Tooltip, ContextMenu)
//! - Basic components (Text, Button, Div, VirtualList)

use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpui::components::atoms::{
    Mode, ModeBadge, Model, ModelBadge, Status, StatusDot, StreamingIndicator,
};
use wgpui::components::hud::{
    CornerConfig, DotShape, DotsGrid, DotsOrigin, DrawDirection, Frame, FrameAnimation,
    Notifications, StatusBar, StatusItem,
};
use wgpui::components::molecules::{MessageHeader, ModeSelector, ModelSelector};
use wgpui::renderer::Renderer;
use wgpui::{
    Animation, Bounds, Button, ButtonVariant, Component, Div, Easing, EventContext, Hsla,
    InputEvent, PaintContext, Point, Quad, Scene, Size, SpringAnimation, Text, TextSystem,
    VirtualList, theme,
};
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
    demo: DemoState,
}

struct DemoState {
    #[allow(dead_code)]
    start_time: Instant,
    position_anim: Animation<f32>,
    color_anim: Animation<Hsla>,
    spring: SpringAnimation<f32>,
    spring_going_up: bool,
    streaming_indicator: StreamingIndicator,
    virtual_list: VirtualList<String>,
    virtual_list_bounds: Bounds,
    status_bar: StatusBar,
    #[allow(dead_code)]
    notifications: Notifications,
    dots_grid: DotsGrid,
    dots_anim: Animation<f32>,
    frame_anim: Animation<f32>,
    frame_count: u64,
}

impl Default for DemoState {
    fn default() -> Self {
        let mut position_anim = Animation::new(0.0_f32, 300.0, Duration::from_millis(2000))
            .easing(Easing::EaseInOutCubic)
            .iterations(0)
            .alternate();
        position_anim.start();

        let mut color_anim = Animation::new(
            theme::accent::PRIMARY,
            theme::accent::GREEN,
            Duration::from_millis(3000),
        )
        .easing(Easing::EaseInOut)
        .iterations(0)
        .alternate();
        color_anim.start();

        let spring = SpringAnimation::new(0.0, 100.0)
            .stiffness(80.0)
            .damping(8.0);

        let items: Vec<String> = (0..10000)
            .map(|i| format!("Item #{} - Virtual scrolling", i))
            .collect();
        let item_height = 26.0;
        let font_size = 12.0;
        let virtual_list = VirtualList::new(
            items,
            item_height,
            move |item: &String, idx: usize, bounds: Bounds, cx: &mut PaintContext| {
                let bg = if idx % 2 == 0 {
                    theme::bg::SURFACE
                } else {
                    theme::bg::MUTED
                };
                cx.scene.draw_quad(Quad::new(bounds).with_background(bg));
                let text_y = bounds.origin.y + (bounds.size.height - font_size) / 2.0;
                let run = cx.text.layout(
                    item,
                    Point::new(bounds.origin.x + 10.0, text_y),
                    font_size,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(run);
            },
        );

        Self {
            start_time: Instant::now(),
            position_anim,
            color_anim,
            spring,
            spring_going_up: true,
            streaming_indicator: StreamingIndicator::new(),
            virtual_list,
            virtual_list_bounds: Bounds::ZERO,
            status_bar: StatusBar::new().items(vec![
                StatusItem::mode("mode", Mode::Normal).left(),
                StatusItem::text("file", "first_light.rs").center(),
                StatusItem::model("model", Model::Codex).right(),
                StatusItem::status("status", Status::Online).right(),
            ]),
            notifications: Notifications::new(),
            dots_grid: DotsGrid::new()
                .color(Hsla::new(180.0, 0.5, 0.3, 0.4))
                .shape(DotShape::Cross)
                .distance(24.0)
                .size(6.0)
                .cross_thickness(1.0)
                .origin(DotsOrigin::Center)
                .easing(Easing::EaseOut),
            dots_anim: {
                let mut anim = Animation::new(0.0_f32, 1.0, Duration::from_millis(1500))
                    .easing(Easing::Linear)
                    .iterations(0)
                    .alternate();
                anim.start();
                anim
            },
            frame_anim: {
                let mut anim = Animation::new(0.0_f32, 1.0, Duration::from_millis(2000))
                    .easing(Easing::EaseInOutCubic)
                    .iterations(0)
                    .alternate();
                anim.start();
                anim
            },
            frame_count: 0,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("wgpui - Full Component Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(1100, 850));

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
                demo: DemoState::default(),
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
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (-x * 20.0, -y * 20.0),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        (-pos.x as f32, -pos.y as f32)
                    }
                };
                let scroll_event = InputEvent::Scroll { dx, dy };
                let mut ecx = EventContext::new();
                state.demo.virtual_list.event(
                    &scroll_event,
                    state.demo.virtual_list_bounds,
                    &mut ecx,
                );
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;
                let delta = Duration::from_millis(16);

                state.demo.position_anim.tick(delta);
                state.demo.color_anim.tick(delta);
                state.demo.spring.tick(delta);
                state.demo.dots_anim.tick(delta);
                state.demo.frame_anim.tick(delta);
                state.demo.streaming_indicator.tick();
                state.demo.frame_count += 1;

                if state.demo.spring.is_settled() {
                    if state.demo.spring_going_up {
                        state.demo.spring.set_target(0.0);
                    } else {
                        state.demo.spring.set_target(100.0);
                    }
                    state.demo.spring_going_up = !state.demo.spring_going_up;
                }

                let mut scene = Scene::new();
                build_full_demo(
                    &mut scene,
                    &mut state.text_system,
                    &mut state.demo,
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

                state
                    .renderer
                    .resize(&state.queue, Size::new(width, height), 1.0);

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                let scale_factor = state.window.scale_factor() as f32;
                state
                    .renderer
                    .prepare(&state.device, &state.queue, &scene, scale_factor);
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

fn build_full_demo(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &mut DemoState,
    width: f32,
    height: f32,
) {
    let margin = 20.0;
    let section_spacing = 28.0;
    let col_width = (width - margin * 3.0) / 2.0;

    scene
        .draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let dots_progress = demo.dots_anim.current_value();
    demo.dots_grid = DotsGrid::new()
        .color(Hsla::new(180.0, 0.5, 0.3, 0.4))
        .shape(DotShape::Cross)
        .distance(24.0)
        .size(6.0)
        .cross_thickness(1.0)
        .origin(DotsOrigin::Center)
        .easing(Easing::EaseOut)
        .animation_progress(dots_progress);

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    demo.dots_grid
        .paint(Bounds::new(0.0, 0.0, width, height), &mut cx);

    let mut y = margin;
    draw_header(scene, text_system, margin, &mut y, width);

    let left_x = margin;
    let right_x = margin * 2.0 + col_width;
    let mut left_y = y;
    let mut right_y = y;

    demo_animation_system(scene, text_system, demo, left_x, col_width, &mut left_y);
    left_y += section_spacing;

    demo_atoms(scene, text_system, demo, right_x, col_width, &mut right_y);
    right_y += section_spacing;

    demo_text_component(scene, text_system, left_x, col_width, &mut left_y);
    left_y += section_spacing;

    demo_molecules(scene, text_system, right_x, col_width, &mut right_y);
    right_y += section_spacing;

    demo_button_component(scene, text_system, left_x, &mut left_y);
    left_y += section_spacing;

    demo_div_component(scene, text_system, right_x, col_width, &mut right_y);
    right_y += section_spacing;

    demo_virtual_list(scene, text_system, demo, left_x, col_width, &mut left_y);
    let _unused = left_y;

    demo_theme_colors(scene, text_system, right_x, &mut right_y);
    right_y += section_spacing;

    demo_animated_frames(scene, text_system, demo, right_x, col_width, &mut right_y);

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    demo.status_bar
        .paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
}

fn draw_header(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    margin: f32,
    y: &mut f32,
    width: f32,
) {
    let title = "wgpui Component Showcase";
    let subtitle = "GPU-Accelerated UI • Animation • Atoms • Molecules • HUD • 377 Tests";

    let title_run = text_system.layout(
        title,
        Point::new(margin, *y + 24.0),
        26.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    let subtitle_run = text_system.layout(
        subtitle,
        Point::new(margin, *y + 50.0),
        13.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);

    scene.draw_quad(
        Quad::new(Bounds::new(margin, *y + 66.0, width - margin * 2.0, 2.0))
            .with_background(theme::accent::PRIMARY),
    );

    *y += 82.0;
}

fn draw_section_header(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    y: &mut f32,
    title: &str,
) {
    let run = text_system.layout(title, Point::new(x, *y + 14.0), 15.0, theme::text::PRIMARY);
    scene.draw_text(run);
    scene.draw_quad(
        Quad::new(Bounds::new(x, *y + 28.0, 150.0, 1.0))
            .with_background(theme::accent::PRIMARY.with_alpha(0.5)),
    );
    *y += 38.0;
}

fn demo_animation_system(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &DemoState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Animation System");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 100.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let anim_x = x + demo.position_anim.current_value();
    let anim_color = demo.color_anim.current_value();
    scene.draw_quad(
        Quad::new(Bounds::new(anim_x + 10.0, *y + 15.0, 35.0, 35.0)).with_background(anim_color),
    );

    let label = text_system.layout(
        "Easing + Color Animation",
        Point::new(x + 10.0, *y + 70.0),
        11.0,
        theme::text::MUTED,
    );
    scene.draw_text(label);

    let spring_val = demo.spring.current();
    scene.draw_quad(
        Quad::new(Bounds::new(
            x + width - 55.0,
            *y + 15.0 + (100.0 - spring_val) * 0.4,
            35.0,
            35.0,
        ))
        .with_background(theme::accent::PURPLE),
    );

    let spring_label = text_system.layout(
        "Spring Physics",
        Point::new(x + width - 100.0, *y + 85.0),
        11.0,
        theme::text::MUTED,
    );
    scene.draw_text(spring_label);

    *y += 110.0;
}

fn demo_atoms(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &mut DemoState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Atoms (13 Components)");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 100.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    let mut ax = x + 12.0;
    let ay = *y + 15.0;

    StatusDot::new(Status::Online).paint(Bounds::new(ax, ay, 10.0, 10.0), &mut cx);
    ax += 20.0;
    StatusDot::new(Status::Busy).paint(Bounds::new(ax, ay, 10.0, 10.0), &mut cx);
    ax += 20.0;
    StatusDot::new(Status::Away).paint(Bounds::new(ax, ay, 10.0, 10.0), &mut cx);
    ax += 20.0;
    StatusDot::new(Status::Offline).paint(Bounds::new(ax, ay, 10.0, 10.0), &mut cx);

    let label = cx.text.layout(
        "StatusDot",
        Point::new(x + 12.0, ay + 18.0),
        10.0,
        theme::text::MUTED,
    );
    cx.scene.draw_text(label);

    ax = x + 12.0;
    let ay2 = *y + 50.0;
    ModeBadge::new(Mode::Normal).paint(Bounds::new(ax, ay2, 55.0, 18.0), &mut cx);
    ax += 60.0;
    ModeBadge::new(Mode::Plan).paint(Bounds::new(ax, ay2, 55.0, 18.0), &mut cx);
    ax += 60.0;
    ModeBadge::new(Mode::Act).paint(Bounds::new(ax, ay2, 55.0, 18.0), &mut cx);

    let label2 = cx.text.layout(
        "ModeBadge",
        Point::new(x + 12.0, ay2 + 24.0),
        10.0,
        theme::text::MUTED,
    );
    cx.scene.draw_text(label2);

    ax = x + width - 160.0;
    ModelBadge::new(Model::Codex).paint(Bounds::new(ax, ay, 70.0, 20.0), &mut cx);
    ax += 75.0;
    ModelBadge::new(Model::Gpt4).paint(Bounds::new(ax, ay, 70.0, 20.0), &mut cx);

    let label3 = cx.text.layout(
        "ModelBadge",
        Point::new(x + width - 160.0, ay + 26.0),
        10.0,
        theme::text::MUTED,
    );
    cx.scene.draw_text(label3);

    demo.streaming_indicator
        .paint(Bounds::new(x + width - 80.0, ay2, 60.0, 18.0), &mut cx);
    let label4 = cx.text.layout(
        "Streaming",
        Point::new(x + width - 80.0, ay2 + 24.0),
        10.0,
        theme::text::MUTED,
    );
    cx.scene.draw_text(label4);

    *y += 110.0;
}

fn demo_molecules(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Molecules (10 Components)");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 90.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);

    MessageHeader::assistant(Model::Codex)
        .author("Codex")
        .timestamp("Just now")
        .paint(Bounds::new(x + 8.0, *y + 8.0, width - 16.0, 32.0), &mut cx);

    ModeSelector::new(Mode::Normal).paint(Bounds::new(x + 12.0, *y + 50.0, 120.0, 28.0), &mut cx);
    ModelSelector::new(Model::Codex)
        .paint(Bounds::new(x + 145.0, *y + 50.0, 120.0, 28.0), &mut cx);

    let label = text_system.layout(
        "MessageHeader + Selectors",
        Point::new(x + 280.0, *y + 58.0),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(label);

    *y += 100.0;
}

fn demo_text_component(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Text Component");

    let mut text_normal = Text::new("Normal text - The quick brown fox");
    let mut text_bold = Text::new("Bold text - WGPUI rendering").bold();
    let mut text_italic = Text::new("Italic text - Beautiful typography").italic();
    let mut text_accent = Text::new("Accent (20px)")
        .font_size(20.0)
        .color(theme::accent::PRIMARY);

    let text_height = 22.0;
    let mut cx = PaintContext::new(scene, text_system, 1.0);

    text_normal.paint(Bounds::new(x, *y, width, text_height), &mut cx);
    *y += text_height;
    text_bold.paint(Bounds::new(x, *y, width, text_height), &mut cx);
    *y += text_height;
    text_italic.paint(Bounds::new(x, *y, width, text_height), &mut cx);
    *y += text_height;
    text_accent.paint(Bounds::new(x, *y, width, 26.0), &mut cx);
    *y += 26.0;
}

fn demo_button_component(scene: &mut Scene, text_system: &mut TextSystem, x: f32, y: &mut f32) {
    draw_section_header(scene, text_system, x, y, "Button Variants");

    let btn_h = 32.0;
    let btn_w = 95.0;
    let spacing = 8.0;
    let mut cx = PaintContext::new(scene, text_system, 1.0);
    let mut bx = x;

    Button::new("Primary").paint(Bounds::new(bx, *y, btn_w, btn_h), &mut cx);
    bx += btn_w + spacing;
    Button::new("Secondary")
        .variant(ButtonVariant::Secondary)
        .paint(Bounds::new(bx, *y, btn_w, btn_h), &mut cx);
    bx += btn_w + spacing;
    Button::new("Ghost")
        .variant(ButtonVariant::Ghost)
        .paint(Bounds::new(bx, *y, btn_w, btn_h), &mut cx);
    bx += btn_w + spacing;
    Button::new("Danger")
        .variant(ButtonVariant::Danger)
        .paint(Bounds::new(bx, *y, btn_w, btn_h), &mut cx);
    bx += btn_w + spacing;
    Button::new("Disabled")
        .disabled(true)
        .paint(Bounds::new(bx, *y, btn_w, btn_h), &mut cx);

    *y += btn_h + 8.0;
}

fn demo_div_component(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Div Container");

    let div_h = 50.0;
    let div_w = (width - 16.0) / 3.0;
    let mut cx = PaintContext::new(scene, text_system, 1.0);

    Div::new()
        .background(theme::bg::SURFACE)
        .paint(Bounds::new(x, *y, div_w, div_h), &mut cx);
    Div::new()
        .background(theme::bg::MUTED)
        .border(theme::border::DEFAULT, 1.0)
        .paint(Bounds::new(x + div_w + 8.0, *y, div_w, div_h), &mut cx);
    Div::new()
        .background(theme::accent::PRIMARY.with_alpha(0.2))
        .border(theme::accent::PRIMARY, 2.0)
        .paint(
            Bounds::new(x + (div_w + 8.0) * 2.0, *y, div_w, div_h),
            &mut cx,
        );

    let labels = ["Surface", "Border", "Accent"];
    for (i, label) in labels.iter().enumerate() {
        let lbl = text_system.layout(
            label,
            Point::new(x + (div_w + 8.0) * i as f32 + 8.0, *y + div_h / 2.0 + 4.0),
            11.0,
            theme::text::MUTED,
        );
        scene.draw_text(lbl);
    }

    *y += div_h + 8.0;
}

fn demo_virtual_list(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &mut DemoState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "VirtualList (10k items)");

    let list_h = 130.0;
    let list_bounds = Bounds::new(x, *y, width, list_h);
    demo.virtual_list_bounds = list_bounds;

    scene.draw_quad(
        Quad::new(list_bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    demo.virtual_list.paint(list_bounds, &mut cx);

    *y += list_h + 8.0;
}

fn demo_theme_colors(scene: &mut Scene, text_system: &mut TextSystem, x: f32, y: &mut f32) {
    draw_section_header(scene, text_system, x, y, "Theme Colors");

    let size = 32.0;
    let gap = 6.0;
    let mut cx = x;

    for color in [theme::bg::APP, theme::bg::SURFACE, theme::bg::MUTED] {
        scene.draw_quad(
            Quad::new(Bounds::new(cx, *y, size, size))
                .with_background(color)
                .with_border(theme::border::DEFAULT, 1.0),
        );
        cx += size + gap;
    }
    cx += gap;
    for color in [
        theme::accent::PRIMARY,
        theme::accent::BLUE,
        theme::accent::GREEN,
        theme::accent::RED,
        theme::accent::PURPLE,
    ] {
        scene.draw_quad(Quad::new(Bounds::new(cx, *y, size, size)).with_background(color));
        cx += size + gap;
    }
    cx += gap;
    for color in [
        theme::status::SUCCESS,
        theme::status::WARNING,
        theme::status::ERROR,
        theme::status::INFO,
    ] {
        scene.draw_quad(Quad::new(Bounds::new(cx, *y, size, size)).with_background(color));
        cx += size + gap;
    }

    *y += size + 8.0;
}

fn draw_bitcoin_symbol(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    y: f32,
    font_size: f32,
    color: Hsla,
) {
    let bar_h = font_size * 0.18;
    let bar_w = 2.0;
    let bar_x1 = x + font_size * 0.15;
    let bar_x2 = x + font_size * 0.38;

    scene.draw_quad(
        Quad::new(Bounds::new(bar_x1, y - bar_h + 2.0, bar_w, bar_h)).with_background(color),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(bar_x2, y - bar_h + 2.0, bar_w, bar_h)).with_background(color),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(bar_x1, y + font_size - 4.0, bar_w, bar_h)).with_background(color),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(bar_x2, y + font_size - 4.0, bar_w, bar_h)).with_background(color),
    );

    let b = text_system.layout("B", Point::new(x, y), font_size, color);
    scene.draw_text(b);
}

fn demo_animated_frames(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &DemoState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Animated Frames (4 Modes)");

    let white = Hsla::new(0.0, 0.0, 1.0, 1.0);
    let white_glow = Hsla::new(0.0, 0.0, 1.0, 0.6);
    let dark_bg = Hsla::new(0.0, 0.0, 0.08, 0.8);
    let cyan_glow = Hsla::new(180.0, 1.0, 0.7, 0.5);
    let purple_glow = Hsla::new(280.0, 1.0, 0.7, 0.5);
    let green_glow = Hsla::new(120.0, 1.0, 0.6, 0.5);
    let muted = Hsla::new(0.0, 0.0, 0.7, 1.0);

    let frame_w = (width - 16.0) / 2.0;
    let frame_h = 60.0;
    let progress = demo.frame_anim.current_value();
    let mut cx = PaintContext::new(scene, text_system, 1.0);

    Frame::corners()
        .line_color(white)
        .bg_color(dark_bg)
        .glow_color(white_glow)
        .stroke_width(2.0)
        .corner_length(18.0)
        .animation_mode(FrameAnimation::Fade)
        .animation_progress(progress)
        .paint(Bounds::new(x, *y, frame_w, frame_h), &mut cx);
    let lbl = cx.text.layout(
        "Fade",
        Point::new(x + 10.0, *y + frame_h / 2.0),
        11.0,
        white.with_alpha(progress),
    );
    cx.scene.draw_text(lbl);

    Frame::lines()
        .line_color(white)
        .bg_color(dark_bg)
        .glow_color(cyan_glow)
        .stroke_width(2.0)
        .animation_mode(FrameAnimation::Draw)
        .draw_direction(DrawDirection::CenterOut)
        .animation_progress(progress)
        .paint(
            Bounds::new(x + frame_w + 8.0, *y, frame_w, frame_h),
            &mut cx,
        );
    let lbl = cx.text.layout(
        "Draw (CenterOut)",
        Point::new(x + frame_w + 18.0, *y + frame_h / 2.0),
        11.0,
        white,
    );
    cx.scene.draw_text(lbl);

    *y += frame_h + 10.0;

    Frame::octagon()
        .line_color(white)
        .bg_color(dark_bg)
        .glow_color(purple_glow)
        .stroke_width(2.0)
        .corner_length(14.0)
        .animation_mode(FrameAnimation::Flicker)
        .animation_progress(progress)
        .paint(Bounds::new(x, *y, frame_w, frame_h), &mut cx);
    let lbl = cx.text.layout(
        "Flicker",
        Point::new(x + 10.0, *y + frame_h / 2.0),
        11.0,
        white,
    );
    cx.scene.draw_text(lbl);

    Frame::nefrex()
        .line_color(white)
        .bg_color(dark_bg)
        .glow_color(green_glow)
        .stroke_width(1.5)
        .square_size(12.0)
        .small_line_length(12.0)
        .large_line_length(40.0)
        .corner_config(CornerConfig::all())
        .animation_mode(FrameAnimation::Assemble)
        .animation_progress(progress)
        .paint(
            Bounds::new(x + frame_w + 8.0, *y, frame_w, frame_h),
            &mut cx,
        );
    let lbl = cx.text.layout(
        "Assemble",
        Point::new(x + frame_w + 18.0, *y + frame_h / 2.0),
        11.0,
        white,
    );
    cx.scene.draw_text(lbl);

    *y += frame_h + 10.0;

    Frame::underline()
        .line_color(white)
        .bg_color(dark_bg)
        .glow_color(white_glow)
        .stroke_width(2.0)
        .square_size(12.0)
        .animation_mode(FrameAnimation::Draw)
        .draw_direction(DrawDirection::LeftToRight)
        .animation_progress(progress)
        .paint(Bounds::new(x, *y, frame_w, frame_h), &mut cx);
    let lbl = cx.text.layout(
        "Underline (Draw)",
        Point::new(x + 10.0, *y + frame_h / 2.0),
        11.0,
        white,
    );
    cx.scene.draw_text(lbl);

    Frame::kranox()
        .line_color(white)
        .bg_color(dark_bg)
        .glow_color(cyan_glow)
        .stroke_width(2.0)
        .square_size(10.0)
        .small_line_length(10.0)
        .large_line_length(35.0)
        .animation_mode(FrameAnimation::Draw)
        .draw_direction(DrawDirection::EdgesIn)
        .animation_progress(progress)
        .paint(
            Bounds::new(x + frame_w + 8.0, *y, frame_w, frame_h),
            &mut cx,
        );
    let lbl = cx.text.layout(
        "Kranox (EdgesIn)",
        Point::new(x + frame_w + 18.0, *y + frame_h / 2.0),
        11.0,
        white,
    );
    cx.scene.draw_text(lbl);

    *y += frame_h + 14.0;

    let wallet_bounds = Bounds::new(x, *y, width, 80.0);
    Frame::corners()
        .line_color(white)
        .bg_color(dark_bg)
        .glow_color(cyan_glow)
        .stroke_width(2.0)
        .corner_length(24.0)
        .animation_mode(FrameAnimation::Draw)
        .draw_direction(DrawDirection::CenterOut)
        .animation_progress(progress)
        .paint(wallet_bounds, &mut cx);

    let balance_label = cx
        .text
        .layout("Balance", Point::new(x + 20.0, *y + 14.0), 11.0, muted);
    cx.scene.draw_text(balance_label);

    let font_size = 28.0;
    let symbol_x = x + 20.0;
    let symbol_y = *y + 28.0;
    draw_bitcoin_symbol(cx.scene, cx.text, symbol_x, symbol_y, font_size, white);

    let sats_amount = cx.text.layout(
        "42069",
        Point::new(symbol_x + font_size * 0.55, symbol_y),
        font_size,
        white,
    );
    cx.scene.draw_text(sats_amount);

    let usd_value = cx
        .text
        .layout("~ $42.07", Point::new(x + 20.0, *y + 60.0), 13.0, muted);
    cx.scene.draw_text(usd_value);

    *y += 88.0;
}
