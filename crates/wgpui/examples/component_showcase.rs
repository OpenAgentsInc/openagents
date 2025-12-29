use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpui::{
    Bounds, Button, ButtonVariant, Component, Div, InputEvent, MouseButton, Point, 
    PaintContext, Quad, Scene, Size, Text, TextInput, VirtualList, theme,
    Animation, Easing, SpringAnimation, Animatable,
    Tooltip, TooltipPosition, ContextMenu, MenuItem,
};
use wgpui::components::atoms::{Mode, Model, Status, StatusDot, ModeBadge, ModelBadge, StreamingIndicator, EntryType};
use wgpui::components::molecules::{MessageHeader, ModeSelector, ModelSelector};
use wgpui::components::hud::{CommandPalette, Command, StatusBar, StatusItem, Notifications};
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
    text_system: wgpui::TextSystem,
    demo_state: DemoState,
}

struct DemoState {
    start_time: Instant,
    position_anim: Animation<f32>,
    color_anim: Animation<wgpui::Hsla>,
    spring: SpringAnimation<f32>,
    tooltip: Tooltip,
    context_menu: ContextMenu,
    command_palette: CommandPalette,
    status_bar: StatusBar,
    notifications: Notifications,
    text_input: TextInput,
    selected_mode: Mode,
    selected_model: Model,
    hover_button: Option<usize>,
    message_count: usize,
}

impl Default for DemoState {
    fn default() -> Self {
        let mut position_anim = Animation::new(0.0_f32, 200.0, Duration::from_millis(2000))
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

        Self {
            start_time: Instant::now(),
            position_anim,
            color_anim,
            spring,
            tooltip: Tooltip::new("This button does something amazing!")
                .position(TooltipPosition::Top)
                .delay(20),
            context_menu: ContextMenu::new().items(vec![
                MenuItem::new("new", "New File").shortcut("Cmd+N"),
                MenuItem::new("open", "Open...").shortcut("Cmd+O"),
                MenuItem::separator(),
                MenuItem::new("save", "Save").shortcut("Cmd+S"),
                MenuItem::new("saveas", "Save As...").shortcut("Cmd+Shift+S"),
                MenuItem::separator(),
                MenuItem::new("close", "Close").shortcut("Cmd+W"),
            ]),
            command_palette: CommandPalette::new().commands(vec![
                Command::new("file.new", "New File").keybinding("Cmd+N"),
                Command::new("file.open", "Open File").keybinding("Cmd+O"),
                Command::new("file.save", "Save").keybinding("Cmd+S"),
                Command::new("edit.undo", "Undo").keybinding("Cmd+Z"),
                Command::new("edit.redo", "Redo").keybinding("Cmd+Shift+Z"),
                Command::new("view.zoom", "Zoom In").keybinding("Cmd++"),
            ]),
            status_bar: StatusBar::new().items(vec![
                StatusItem::mode("mode", Mode::Normal).left(),
                StatusItem::text("file", "component_showcase.rs").center(),
                StatusItem::model("model", Model::Claude).right(),
                StatusItem::status("status", Status::Online).right(),
            ]),
            notifications: Notifications::new(),
            text_input: TextInput::new().placeholder("Type something..."),
            selected_mode: Mode::Normal,
            selected_model: Model::Claude,
            hover_button: None,
            message_count: 10000,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("wgpui Component Showcase")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 900));

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
            let text_system = wgpui::TextSystem::new(scale_factor);

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                demo_state: DemoState::default(),
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
                let delta = Duration::from_millis(16);

                state.demo_state.position_anim.tick(delta);
                state.demo_state.color_anim.tick(delta);
                state.demo_state.spring.tick(delta);

                let mut scene = Scene::new();
                build_showcase(
                    &mut scene,
                    &mut state.text_system,
                    &mut state.demo_state,
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

                let scale_factor = state.window.scale_factor() as f32;
                state.renderer.prepare(&state.device, &state.queue, &scene, scale_factor);
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

fn build_showcase(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    demo: &mut DemoState,
    width: f32,
    height: f32,
) {
    let margin = 24.0;
    let col_width = (width - margin * 3.0) / 2.0;
    
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let mut y = margin;
    draw_header(scene, text_system, margin, &mut y, width);

    let left_x = margin;
    let right_x = margin * 2.0 + col_width;

    let mut left_y = y;
    let mut right_y = y;

    draw_animation_section(scene, text_system, demo, left_x, col_width, &mut left_y);
    draw_atoms_section(scene, text_system, right_x, col_width, &mut right_y);

    left_y += 32.0;
    right_y += 32.0;

    draw_buttons_section(scene, text_system, left_x, col_width, &mut left_y);
    draw_inputs_section(scene, text_system, demo, right_x, col_width, &mut right_y);

    left_y += 32.0;
    right_y += 32.0;

    draw_virtual_list_section(scene, text_system, demo, left_x, col_width, &mut left_y);
    draw_selectors_section(scene, text_system, demo, right_x, col_width, &mut right_y);

    demo.status_bar.paint(Bounds::new(0.0, 0.0, width, height), &mut PaintContext::new(scene, text_system, 1.0));
    demo.notifications.paint(Bounds::new(0.0, 0.0, width, height), &mut PaintContext::new(scene, text_system, 1.0));
}

fn draw_header(scene: &mut Scene, text_system: &mut wgpui::TextSystem, margin: f32, y: &mut f32, width: f32) {
    let title = "wgpui Component Showcase";
    let subtitle = "GPU-Accelerated UI • 40+ Components • Animation • Accessibility";

    let title_run = text_system.layout(
        title,
        Point::new(margin, *y + 24.0),
        28.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    let subtitle_run = text_system.layout(
        subtitle,
        Point::new(margin, *y + 52.0),
        14.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);

    scene.draw_quad(
        Quad::new(Bounds::new(margin, *y + 70.0, width - margin * 2.0, 2.0))
            .with_background(theme::accent::PRIMARY),
    );

    *y += 90.0;
}

fn draw_section_title(scene: &mut Scene, text_system: &mut wgpui::TextSystem, x: f32, y: &mut f32, title: &str) {
    let run = text_system.layout(title, Point::new(x, *y + 16.0), 18.0, theme::text::PRIMARY);
    scene.draw_text(run);
    *y += 32.0;
}

fn draw_animation_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    demo: &DemoState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, "Animation System");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 120.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let anim_x = x + demo.position_anim.current_value();
    let anim_color = demo.color_anim.current_value();
    scene.draw_quad(
        Quad::new(Bounds::new(anim_x + 10.0, *y + 20.0, 40.0, 40.0))
            .with_background(anim_color),
    );

    let label = text_system.layout(
        "Position + Color Animation",
        Point::new(x + 10.0, *y + 80.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(label);

    let spring_val = demo.spring.current();
    scene.draw_quad(
        Quad::new(Bounds::new(x + width - 60.0, *y + 20.0 + (100.0 - spring_val) * 0.5, 40.0, 40.0))
            .with_background(theme::accent::PURPLE),
    );

    let spring_label = text_system.layout(
        "Spring Physics",
        Point::new(x + width - 100.0, *y + 100.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(spring_label);

    *y += 130.0;
}

fn draw_atoms_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, "Atoms");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 120.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    let mut atom_x = x + 16.0;
    let atom_y = *y + 20.0;

    StatusDot::new(Status::Online).paint(Bounds::new(atom_x, atom_y, 12.0, 12.0), &mut cx);
    atom_x += 30.0;

    StatusDot::new(Status::Busy).paint(Bounds::new(atom_x, atom_y, 12.0, 12.0), &mut cx);
    atom_x += 30.0;

    StatusDot::new(Status::Away).paint(Bounds::new(atom_x, atom_y, 12.0, 12.0), &mut cx);
    atom_x += 30.0;

    StatusDot::new(Status::Offline).paint(Bounds::new(atom_x, atom_y, 12.0, 12.0), &mut cx);
    atom_x += 50.0;

    ModeBadge::new(Mode::Normal).paint(Bounds::new(atom_x, atom_y - 4.0, 60.0, 20.0), &mut cx);
    atom_x += 70.0;

    ModeBadge::new(Mode::Plan).paint(Bounds::new(atom_x, atom_y - 4.0, 60.0, 20.0), &mut cx);
    atom_x += 70.0;

    ModeBadge::new(Mode::Act).paint(Bounds::new(atom_x, atom_y - 4.0, 60.0, 20.0), &mut cx);

    let badge_y = atom_y + 40.0;
    atom_x = x + 16.0;

    ModelBadge::new(Model::Claude).paint(Bounds::new(atom_x, badge_y, 80.0, 24.0), &mut cx);
    atom_x += 90.0;

    ModelBadge::new(Model::Gpt4).paint(Bounds::new(atom_x, badge_y, 80.0, 24.0), &mut cx);
    atom_x += 90.0;

    ModelBadge::new(Model::Gemini).paint(Bounds::new(atom_x, badge_y, 80.0, 24.0), &mut cx);
    atom_x += 100.0;

    StreamingIndicator::new().paint(Bounds::new(atom_x, badge_y + 4.0, 40.0, 16.0), &mut cx);

    *y += 130.0;
}

fn draw_buttons_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    x: f32,
    _width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, "Button Variants");

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    let btn_height = 36.0;
    let btn_width = 100.0;
    let spacing = 12.0;
    let mut btn_x = x;

    Button::new("Primary").paint(Bounds::new(btn_x, *y, btn_width, btn_height), &mut cx);
    btn_x += btn_width + spacing;

    Button::new("Secondary")
        .variant(ButtonVariant::Secondary)
        .paint(Bounds::new(btn_x, *y, btn_width, btn_height), &mut cx);
    btn_x += btn_width + spacing;

    Button::new("Ghost")
        .variant(ButtonVariant::Ghost)
        .paint(Bounds::new(btn_x, *y, btn_width, btn_height), &mut cx);
    btn_x += btn_width + spacing;

    Button::new("Danger")
        .variant(ButtonVariant::Danger)
        .paint(Bounds::new(btn_x, *y, btn_width, btn_height), &mut cx);
    btn_x += btn_width + spacing;

    Button::new("Disabled")
        .disabled(true)
        .paint(Bounds::new(btn_x, *y, btn_width, btn_height), &mut cx);

    *y += btn_height + 16.0;
}

fn draw_inputs_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    demo: &mut DemoState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, "Text Input");

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    demo.text_input.paint(Bounds::new(x, *y, width - 20.0, 36.0), &mut cx);

    *y += 52.0;
}

fn draw_virtual_list_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    demo: &DemoState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, &format!("Virtual List ({} items)", demo.message_count));

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 200.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let items: Vec<String> = (0..20)
        .map(|i| format!("Message #{} - Virtualized for performance", i))
        .collect();

    let item_height = 32.0;
    let mut virtual_list = VirtualList::new(
        items,
        item_height,
        move |item: &String, idx: usize, bounds: Bounds, cx: &mut PaintContext| {
            let bg = if idx % 2 == 0 { theme::bg::SURFACE } else { theme::bg::MUTED };
            cx.scene.draw_quad(Quad::new(bounds).with_background(bg));
            
            let run = cx.text.layout(
                item,
                Point::new(bounds.origin.x + 12.0, bounds.origin.y + bounds.size.height * 0.6),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(run);
        },
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    virtual_list.paint(Bounds::new(x + 1.0, *y + 1.0, width - 2.0, 198.0), &mut cx);

    *y += 210.0;
}

fn draw_selectors_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    demo: &DemoState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, "Mode & Model Selectors");

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    
    ModeSelector::new(demo.selected_mode).paint(Bounds::new(x, *y, 150.0, 32.0), &mut cx);
    ModelSelector::new(demo.selected_model).paint(Bounds::new(x + 170.0, *y, 150.0, 32.0), &mut cx);

    *y += 48.0;

    draw_section_title(scene, text_system, x, y, "Message Header");

    MessageHeader::new(EntryType::Assistant)
        .model(Model::Claude)
        .paint(Bounds::new(x, *y, width, 40.0), &mut cx);

    *y += 56.0;
}
