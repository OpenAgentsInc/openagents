use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpui::components::hud::{
    CornerConfig, DotShape, DotsGrid, DotsOrigin, DrawDirection, Frame, FrameAnimation, FrameStyle,
};
use wgpui::renderer::Renderer;
use wgpui::{
    Animation, Bounds, Component, Easing, Hsla, PaintContext, Point, Quad, Scene, Size,
    SpringAnimation, TextSystem, theme,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)] // All priority levels are valid, demo only uses some
enum Priority {
    Background,
    Normal,
    Elevated,
    Urgent,
    Critical,
}

impl Priority {
    fn glow_color(&self) -> Option<Hsla> {
        match self {
            Priority::Background | Priority::Normal => None,
            Priority::Elevated => Some(Hsla::new(0.5, 1.0, 0.6, 0.8)), // cyan (180°)
            Priority::Urgent => Some(Hsla::new(0.125, 1.0, 0.5, 0.9)), // orange (45°)
            Priority::Critical => Some(Hsla::new(0.0, 1.0, 0.5, 1.0)), // red (0°)
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum PaneState {
    Creating,
    Open,
    Minimized,
    Closing,
}

struct VisualPane {
    #[allow(dead_code)]
    id: String,
    title: String,
    target_x: f32,
    target_y: f32,
    target_w: f32,
    target_h: f32,
    x_anim: Animation<f32>,
    y_anim: Animation<f32>,
    w_anim: Animation<f32>,
    h_anim: Animation<f32>,
    alpha_anim: Animation<f32>,
    priority: Priority,
    custom_glow: Option<Hsla>,
    frame_style: FrameStyle,
    frame_animation: FrameAnimation,
    draw_direction: DrawDirection,
    state: PaneState,
    z_index: i32,
    shake: SpringAnimation<f32>,
    shake_target: f32,
    shake_phase: u8,
    content_type: String,
}

impl VisualPane {
    fn new(id: &str, title: &str, x: f32, y: f32, w: f32, h: f32) -> Self {
        let x_anim = Animation::new(x, x, Duration::from_millis(500)).easing(Easing::EaseOutCubic);
        let y_anim = Animation::new(y, y, Duration::from_millis(500)).easing(Easing::EaseOutCubic);
        let w_anim = Animation::new(w, w, Duration::from_millis(300)).easing(Easing::EaseOutCubic);
        let h_anim = Animation::new(h, h, Duration::from_millis(300)).easing(Easing::EaseOutCubic);
        let mut alpha_anim =
            Animation::new(0.0, 1.0, Duration::from_millis(400)).easing(Easing::EaseOut);

        alpha_anim.start();

        Self {
            id: id.to_string(),
            title: title.to_string(),
            target_x: x,
            target_y: y,
            target_w: w,
            target_h: h,
            x_anim,
            y_anim,
            w_anim,
            h_anim,
            alpha_anim,
            priority: Priority::Normal,
            custom_glow: None,
            frame_style: FrameStyle::Corners,
            frame_animation: FrameAnimation::Fade,
            draw_direction: DrawDirection::CenterOut,
            state: PaneState::Creating,
            z_index: 0,
            shake: SpringAnimation::new(0.0, 0.0)
                .stiffness(300.0)
                .damping(10.0),
            shake_target: 0.0,
            shake_phase: 0,
            content_type: "generic".to_string(),
        }
    }

    fn move_to(&mut self, x: f32, y: f32, animate: bool) {
        self.target_x = x;
        self.target_y = y;
        if animate {
            self.x_anim =
                Animation::new(self.x_anim.current_value(), x, Duration::from_millis(400))
                    .easing(Easing::EaseInOutCubic);
            self.y_anim =
                Animation::new(self.y_anim.current_value(), y, Duration::from_millis(400))
                    .easing(Easing::EaseInOutCubic);
            self.x_anim.start();
            self.y_anim.start();
        }
    }

    fn resize_to(&mut self, w: f32, h: f32, animate: bool) {
        self.target_w = w;
        self.target_h = h;
        if animate {
            self.w_anim =
                Animation::new(self.w_anim.current_value(), w, Duration::from_millis(300))
                    .easing(Easing::EaseInOutCubic);
            self.h_anim =
                Animation::new(self.h_anim.current_value(), h, Duration::from_millis(300))
                    .easing(Easing::EaseInOutCubic);
            self.w_anim.start();
            self.h_anim.start();
        }
    }

    fn set_priority(&mut self, priority: Priority) {
        self.priority = priority;
    }

    fn set_glow(&mut self, color: Option<Hsla>) {
        self.custom_glow = color;
    }

    fn request_attention(&mut self) {
        self.shake_phase = 1;
        self.shake_target = 15.0;
        self.shake.set_target(15.0);
    }

    fn minimize(&mut self) {
        self.state = PaneState::Minimized;
        self.h_anim = Animation::new(
            self.h_anim.current_value(),
            30.0,
            Duration::from_millis(300),
        )
        .easing(Easing::EaseInOutCubic);
        self.h_anim.start();
    }

    fn close(&mut self) {
        self.state = PaneState::Closing;
        self.alpha_anim =
            Animation::new(1.0, 0.0, Duration::from_millis(300)).easing(Easing::EaseIn);
        self.alpha_anim.start();
    }

    fn tick(&mut self, dt: Duration) {
        self.x_anim.tick(dt);
        self.y_anim.tick(dt);
        self.w_anim.tick(dt);
        self.h_anim.tick(dt);
        self.alpha_anim.tick(dt);
        self.shake.tick(dt);

        if self.shake_phase > 0 && self.shake.is_settled() {
            match self.shake_phase {
                1 => {
                    self.shake_target = -12.0;
                    self.shake.set_target(-12.0);
                    self.shake_phase = 2;
                }
                2 => {
                    self.shake_target = 0.0;
                    self.shake.set_target(0.0);
                    self.shake_phase = 3;
                }
                _ => {
                    self.shake_phase = 0;
                }
            }
        }

        if self.state == PaneState::Creating && self.alpha_anim.is_finished() {
            self.state = PaneState::Open;
        }
    }

    fn current_bounds(&self) -> Bounds {
        let shake_offset = if self.shake_phase > 0 {
            self.shake.current()
        } else {
            0.0
        };
        Bounds::new(
            self.x_anim.current_value() + shake_offset,
            self.y_anim.current_value(),
            self.w_anim.current_value(),
            self.h_anim.current_value(),
        )
    }

    fn glow_color(&self) -> Option<Hsla> {
        self.custom_glow.or_else(|| self.priority.glow_color())
    }

    fn is_visible(&self) -> bool {
        self.alpha_anim.current_value() > 0.01
    }
}

struct ToolCallLog {
    entries: Vec<(f32, String)>,
    max_entries: usize,
}

impl ToolCallLog {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 8,
        }
    }

    fn add(&mut self, time: f32, msg: String) {
        self.entries.push((time, msg));
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }
}

struct DemoState {
    panes: HashMap<String, VisualPane>,
    z_counter: i32,
    tool_log: ToolCallLog,
    start_time: Instant,
    last_action_time: f32,
    scenario_index: usize,
    paused: bool,
    dots_anim: Animation<f32>,
    frame_anim: Animation<f32>,
}

impl DemoState {
    fn new() -> Self {
        let mut dots_anim = Animation::new(0.0_f32, 1.0, Duration::from_millis(2000))
            .easing(Easing::Linear)
            .iterations(0)
            .alternate();
        dots_anim.start();

        let mut frame_anim = Animation::new(0.0_f32, 1.0, Duration::from_millis(2500))
            .easing(Easing::EaseInOutCubic)
            .iterations(0)
            .alternate();
        frame_anim.start();

        Self {
            panes: HashMap::new(),
            z_counter: 0,
            tool_log: ToolCallLog::new(),
            start_time: Instant::now(),
            last_action_time: 0.0,
            scenario_index: 0,
            paused: false,
            dots_anim,
            frame_anim,
        }
    }

    fn elapsed(&self) -> f32 {
        self.start_time.elapsed().as_secs_f32()
    }

    fn create_pane(
        &mut self,
        id: &str,
        title: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        content_type: &str,
    ) {
        let mut pane = VisualPane::new(id, title, x, y, w, h);
        pane.content_type = content_type.to_string();

        match content_type {
            "code" => {
                pane.frame_style = FrameStyle::Corners;
                pane.frame_animation = FrameAnimation::Draw;
                pane.draw_direction = DrawDirection::CenterOut;
            }
            "terminal" => {
                pane.frame_style = FrameStyle::Lines;
                pane.frame_animation = FrameAnimation::Draw;
                pane.draw_direction = DrawDirection::LeftToRight;
            }
            "chat" => {
                pane.frame_style = FrameStyle::Nefrex;
                pane.frame_animation = FrameAnimation::Assemble;
                pane.draw_direction = DrawDirection::CenterOut;
            }
            "diagnostics" => {
                pane.frame_style = FrameStyle::Octagon;
                pane.frame_animation = FrameAnimation::Flicker;
                pane.draw_direction = DrawDirection::EdgesIn;
            }
            _ => {}
        }

        self.z_counter += 1;
        pane.z_index = self.z_counter;
        self.panes.insert(id.to_string(), pane);
        self.tool_log.add(
            self.elapsed(),
            format!("CreatePane {{ id: \"{}\", title: \"{}\" }}", id, title),
        );
    }

    fn focus_pane(&mut self, id: &str) {
        self.z_counter += 1;
        if let Some(pane) = self.panes.get_mut(id) {
            pane.z_index = self.z_counter;
        }
        self.tool_log
            .add(self.elapsed(), format!("Focus {{ id: \"{}\" }}", id));
    }

    fn set_priority(&mut self, id: &str, priority: Priority) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.set_priority(priority);
        }
        let p_str = match priority {
            Priority::Background => "Background",
            Priority::Normal => "Normal",
            Priority::Elevated => "Elevated",
            Priority::Urgent => "Urgent",
            Priority::Critical => "Critical",
        };
        self.tool_log.add(
            self.elapsed(),
            format!("SetPriority {{ id: \"{}\", priority: \"{}\" }}", id, p_str),
        );
    }

    fn set_glow(&mut self, id: &str, color: Option<Hsla>) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.set_glow(color);
        }
        let color_str = color
            .map(|c| {
                format!(
                    "#{:02x}{:02x}{:02x}",
                    (c.l * 255.0) as u8,
                    (c.s * 255.0) as u8,
                    (c.h as u8)
                )
            })
            .unwrap_or_else(|| "none".to_string());
        self.tool_log.add(
            self.elapsed(),
            format!("SetGlow {{ id: \"{}\", color: \"{}\" }}", id, color_str),
        );
    }

    fn move_pane(&mut self, id: &str, x: f32, y: f32) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.move_to(x, y, true);
        }
        self.tool_log.add(
            self.elapsed(),
            format!("MovePane {{ id: \"{}\", x: {}, y: {} }}", id, x, y),
        );
    }

    fn resize_pane(&mut self, id: &str, w: f32, h: f32) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.resize_to(w, h, true);
        }
        self.tool_log.add(
            self.elapsed(),
            format!("ResizePane {{ id: \"{}\", w: {}, h: {} }}", id, w, h),
        );
    }

    fn request_attention(&mut self, id: &str, msg: &str) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.request_attention();
            pane.set_priority(Priority::Urgent);
        }
        self.focus_pane(id);
        self.tool_log.add(
            self.elapsed(),
            format!("RequestAttention {{ id: \"{}\", msg: \"{}\" }}", id, msg),
        );
    }

    fn minimize_pane(&mut self, id: &str) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.minimize();
        }
        self.tool_log.add(
            self.elapsed(),
            format!("SetState {{ id: \"{}\", state: \"Minimized\" }}", id),
        );
    }

    fn close_pane(&mut self, id: &str) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.close();
        }
        self.tool_log
            .add(self.elapsed(), format!("ClosePane {{ id: \"{}\" }}", id));
    }

    fn tick(&mut self, dt: Duration) {
        self.dots_anim.tick(dt);
        self.frame_anim.tick(dt);
        for pane in self.panes.values_mut() {
            pane.tick(dt);
        }
        self.panes
            .retain(|_, p| p.is_visible() || p.state != PaneState::Closing);
    }

    fn reset(&mut self) {
        self.panes.clear();
        self.z_counter = 0;
        self.tool_log = ToolCallLog::new();
        self.start_time = Instant::now();
        self.last_action_time = 0.0;
        self.scenario_index = 0;
    }
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

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("UI Pane Demo - Agent Tool Calls")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

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
                demo: DemoState::new(),
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
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                        PhysicalKey::Code(KeyCode::Space) => {
                            state.demo.paused = !state.demo.paused;
                        }
                        PhysicalKey::Code(KeyCode::KeyR) => {
                            state.demo.reset();
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;
                let delta = Duration::from_millis(16);

                if !state.demo.paused {
                    run_demo_script(&mut state.demo, width, height);
                    state.demo.tick(delta);
                }

                let mut scene = Scene::new();
                render_demo(
                    &mut scene,
                    &mut state.text_system,
                    &state.demo,
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

fn run_demo_script(demo: &mut DemoState, _width: f32, _height: f32) {
    let t = demo.elapsed();

    if demo.scenario_index == 0 && t >= 0.5 {
        demo.create_pane("editor", "Code Editor", 50.0, 60.0, 450.0, 250.0, "code");
        demo.scenario_index = 1;
    }
    if demo.scenario_index == 1 && t >= 1.0 {
        demo.create_pane(
            "terminal", "Terminal", 50.0, 340.0, 450.0, 180.0, "terminal",
        );
        demo.scenario_index = 2;
    }
    if demo.scenario_index == 2 && t >= 1.5 {
        demo.create_pane("chat", "AI Assistant", 540.0, 60.0, 340.0, 230.0, "chat");
        demo.scenario_index = 3;
    }
    if demo.scenario_index == 3 && t >= 2.0 {
        demo.create_pane(
            "diagnostics",
            "Diagnostics",
            540.0,
            320.0,
            340.0,
            200.0,
            "diagnostics",
        );
        demo.scenario_index = 4;
    }

    if demo.scenario_index == 4 && t >= 3.0 {
        demo.set_priority("diagnostics", Priority::Urgent);
        demo.scenario_index = 5;
    }
    if demo.scenario_index == 5 && t >= 3.3 {
        demo.focus_pane("diagnostics");
        demo.scenario_index = 6;
    }
    if demo.scenario_index == 6 && t >= 3.6 {
        if let Some(pane) = demo.panes.get_mut("diagnostics") {
            pane.request_attention();
        }
        demo.tool_log.add(
            t,
            "Animate { id: \"diagnostics\", animation: \"Pulse\" }".to_string(),
        );
        demo.scenario_index = 7;
    }

    if demo.scenario_index == 7 && t >= 5.0 {
        demo.set_priority("diagnostics", Priority::Normal);
        demo.set_glow("diagnostics", None);
        demo.scenario_index = 8;
    }
    if demo.scenario_index == 8 && t >= 5.3 {
        demo.focus_pane("editor");
        demo.set_glow("editor", Some(Hsla::new(0.389, 1.0, 0.5, 0.8))); // green (140°)
        demo.scenario_index = 9;
    }

    if demo.scenario_index == 9 && t >= 6.5 {
        demo.move_pane("terminal", 50.0, 330.0);
        demo.scenario_index = 10;
    }
    if demo.scenario_index == 10 && t >= 6.8 {
        demo.resize_pane("terminal", 500.0, 270.0);
        demo.scenario_index = 11;
    }
    if demo.scenario_index == 11 && t >= 7.1 {
        demo.set_priority("terminal", Priority::Elevated);
        demo.focus_pane("terminal");
        demo.scenario_index = 12;
    }

    if demo.scenario_index == 12 && t >= 8.5 {
        demo.minimize_pane("terminal");
        demo.scenario_index = 13;
    }
    if demo.scenario_index == 13 && t >= 9.0 {
        demo.request_attention("chat", "All tests passed!");
        demo.scenario_index = 14;
    }

    if demo.scenario_index == 14 && t >= 11.0 {
        demo.close_pane("diagnostics");
        demo.scenario_index = 15;
    }
    if demo.scenario_index == 15 && t >= 13.0 {
        demo.reset();
    }
}

fn render_demo(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &DemoState,
    width: f32,
    height: f32,
) {
    scene
        .draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let dots_progress = demo.dots_anim.current_value();
    let mut dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 0.3, 0.25))
        .shape(DotShape::Cross)
        .distance(28.0)
        .size(5.0)
        .cross_thickness(1.0)
        .origin(DotsOrigin::Center)
        .easing(Easing::EaseOut)
        .animation_progress(dots_progress);
    let mut cx = PaintContext::new(scene, text_system, 1.0);
    dots_grid.paint(Bounds::new(0.0, 40.0, width, height - 180.0), &mut cx);

    let title = "UI Pane Demo - Agent Tool Calls";
    let run = text_system.layout(title, Point::new(20.0, 18.0), 18.0, theme::text::PRIMARY);
    scene.draw_text(run);

    let subtitle = "[Space] Pause  [R] Restart  [Esc] Exit";
    let run = text_system.layout(
        subtitle,
        Point::new(width - 300.0, 22.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(run);

    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 40.0, width, 2.0))
            .with_background(theme::accent::PRIMARY.with_alpha(0.5)),
    );

    let mut panes: Vec<_> = demo.panes.values().collect();
    panes.sort_by_key(|p| p.z_index);

    let mut cx = PaintContext::new(scene, text_system, 1.0);

    for pane in panes {
        if !pane.is_visible() {
            continue;
        }

        let bounds = pane.current_bounds();
        let alpha = pane.alpha_anim.current_value();

        let white = Hsla::new(0.0, 0.0, 1.0, alpha);
        let dark_bg = Hsla::new(0.0, 0.0, 0.08, 0.85 * alpha);
        let muted = Hsla::new(0.0, 0.0, 0.6, alpha);

        let glow = pane.glow_color().map(|c| c.with_alpha(c.a * alpha));

        let frame = match pane.frame_style {
            FrameStyle::Corners => Frame::corners().corner_length(18.0),
            FrameStyle::Lines => Frame::lines(),
            FrameStyle::Octagon => Frame::octagon().corner_length(14.0),
            FrameStyle::Underline => Frame::underline().square_size(12.0),
            FrameStyle::Nefrex => Frame::nefrex()
                .corner_config(CornerConfig::diagonal())
                .square_size(10.0)
                .small_line_length(10.0)
                .large_line_length(35.0),
            FrameStyle::Kranox => Frame::kranox()
                .square_size(10.0)
                .small_line_length(10.0)
                .large_line_length(35.0),
            FrameStyle::Nero => Frame::nero().corner_length(20.0),
            FrameStyle::Header => Frame::header().corner_length(12.0).header_bottom(true),
            FrameStyle::Circle => Frame::circle().circle_segments(48),
        };

        let frame_progress = demo.frame_anim.current_value();
        let mut frame = frame
            .line_color(white)
            .bg_color(dark_bg)
            .stroke_width(2.0)
            .animation_mode(pane.frame_animation)
            .draw_direction(pane.draw_direction)
            .animation_progress(frame_progress);

        if let Some(g) = glow {
            frame = frame.glow_color(g);
        }

        frame.paint(bounds, &mut cx);

        let title_run = cx.text.layout(
            &pane.title,
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 14.0),
            13.0,
            white,
        );
        cx.scene.draw_text(title_run);

        let type_run = cx.text.layout(
            &pane.content_type,
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 32.0),
            10.0,
            muted,
        );
        cx.scene.draw_text(type_run);

        if pane.state == PaneState::Minimized {
            continue;
        }

        let content_y = bounds.origin.y + 50.0;
        let content_h = bounds.size.height - 60.0;
        if content_h > 20.0 {
            let content_color = Hsla::new(0.0, 0.0, 0.15, 0.5 * alpha);
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x + 8.0,
                    content_y,
                    bounds.size.width - 16.0,
                    content_h,
                ))
                .with_background(content_color),
            );

            let placeholder = match pane.content_type.as_str() {
                "code" => "fn main() {\n    println!(\"Hello\");\n}",
                "terminal" => "$ cargo test\n   Compiling...\n   Finished",
                "chat" => "AI: How can I help?\nUser: Fix the bug",
                "diagnostics" => "error[E0308]: mismatched types\n  --> src/main.rs:42",
                _ => "Content placeholder",
            };
            let text_run = cx.text.layout(
                placeholder,
                Point::new(bounds.origin.x + 14.0, content_y + 8.0),
                11.0,
                Hsla::new(0.0, 0.0, 0.7, alpha),
            );
            cx.scene.draw_text(text_run);
        }
    }

    let log_y = height - 140.0;
    let log_h = 130.0;

    cx.scene.draw_quad(
        Quad::new(Bounds::new(0.0, log_y, width, log_h))
            .with_background(Hsla::new(0.0, 0.0, 0.05, 0.95)),
    );

    cx.scene.draw_quad(
        Quad::new(Bounds::new(0.0, log_y, width, 1.0))
            .with_background(theme::accent::PRIMARY.with_alpha(0.3)),
    );

    let log_title = cx.text.layout(
        "Tool Call Log",
        Point::new(15.0, log_y + 10.0),
        12.0,
        theme::text::PRIMARY,
    );
    cx.scene.draw_text(log_title);

    let mut entry_y = log_y + 30.0;
    for (time, msg) in &demo.tool_log.entries {
        let time_str = format!("[{:.1}s]", time);
        let time_run = cx.text.layout(
            &time_str,
            Point::new(15.0, entry_y),
            11.0,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(time_run);

        let msg_run = cx.text.layout(
            &format!("ui_pane.{}", msg),
            Point::new(70.0, entry_y),
            11.0,
            theme::text::MUTED,
        );
        cx.scene.draw_text(msg_run);

        entry_y += 14.0;
    }

    if demo.paused {
        let paused_text = cx.text.layout(
            "PAUSED",
            Point::new(width / 2.0 - 40.0, height / 2.0),
            24.0,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(paused_text);
    }
}
