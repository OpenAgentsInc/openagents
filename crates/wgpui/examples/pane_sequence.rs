use std::sync::Arc;
use std::time::Instant;
use wgpui::{
    Bounds, Component, Easing, Hsla, PaintContext, Point, Quad, Scene, Size,
    TextSystem, theme,
};
use wgpui::components::hud::{CornerConfig, DotsGrid, DotsOrigin, DotShape, DrawDirection, Frame, FrameAnimation};
use wgpui::renderer::Renderer;
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

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    DotsIn,
    FrameIn,
    TextIn,
    Hold,
    FadeOut,
}

struct SequenceState {
    start_time: Instant,
    phase: Phase,
    phase_start: f32,
    dots_progress: f32,
    frame_progress: f32,
    text_alpha: f32,
    fade_out: f32,
}

impl SequenceState {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
            phase: Phase::DotsIn,
            phase_start: 0.0,
            dots_progress: 0.0,
            frame_progress: 0.0,
            text_alpha: 0.0,
            fade_out: 1.0,
        }
    }

    fn elapsed(&self) -> f32 {
        self.start_time.elapsed().as_secs_f32()
    }

    fn phase_elapsed(&self) -> f32 {
        self.elapsed() - self.phase_start
    }

    fn reset(&mut self) {
        self.start_time = Instant::now();
        self.phase = Phase::DotsIn;
        self.phase_start = 0.0;
        self.dots_progress = 0.0;
        self.frame_progress = 0.0;
        self.text_alpha = 0.0;
        self.fade_out = 1.0;
    }

    fn tick(&mut self) {
        let t = self.phase_elapsed();

        match self.phase {
            Phase::DotsIn => {
                self.dots_progress = ease_out_cubic(t / 1.5);
                if t >= 1.5 {
                    self.phase = Phase::FrameIn;
                    self.phase_start = self.elapsed();
                    self.dots_progress = 1.0;
                }
            }
            Phase::FrameIn => {
                self.frame_progress = ease_out_cubic(t / 1.2);
                if t >= 1.2 {
                    self.phase = Phase::TextIn;
                    self.phase_start = self.elapsed();
                    self.frame_progress = 1.0;
                }
            }
            Phase::TextIn => {
                self.text_alpha = ease_out_cubic(t / 0.8);
                if t >= 0.8 {
                    self.phase = Phase::Hold;
                    self.phase_start = self.elapsed();
                    self.text_alpha = 1.0;
                }
            }
            Phase::Hold => {
                if t >= 2.0 {
                    self.phase = Phase::FadeOut;
                    self.phase_start = self.elapsed();
                }
            }
            Phase::FadeOut => {
                self.fade_out = 1.0 - ease_in_cubic(t / 1.0);
                if t >= 1.0 {
                    self.reset();
                }
            }
        }
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn ease_in_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t.powi(3)
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
    sequence: SequenceState,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Pane Sequence")
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
                sequence: SequenceState::new(),
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
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                        PhysicalKey::Code(KeyCode::KeyR) => state.sequence.reset(),
                        _ => {}
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;

                state.sequence.tick();

                let mut scene = Scene::new();
                render_sequence(&mut scene, &mut state.text_system, &state.sequence, width, height);

                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = state
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });

                state.renderer.resize(&state.queue, Size::new(width, height), 1.0);

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

fn render_sequence(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    seq: &SequenceState,
    width: f32,
    height: f32,
) {
    let fade = seq.fade_out;
    
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, height))
            .with_background(theme::bg::APP),
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);

    let mut dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 0.3, 0.3 * fade))
        .shape(DotShape::Cross)
        .distance(32.0)
        .size(6.0)
        .cross_thickness(1.0)
        .origin(DotsOrigin::Center)
        .easing(Easing::EaseOut)
        .animation_progress(seq.dots_progress);
    dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);

    if seq.frame_progress > 0.0 || seq.phase == Phase::Hold || seq.phase == Phase::FadeOut {
        let pane_w = 500.0;
        let pane_h = 300.0;
        let pane_x = (width - pane_w) / 2.0;
        let pane_y = (height - pane_h) / 2.0;

        let white = Hsla::new(0.0, 0.0, 1.0, fade);
        let dark_bg = Hsla::new(0.0, 0.0, 0.06, 0.9 * fade);
        let cyan_glow = Hsla::new(0.5, 1.0, 0.6, 0.7 * fade);

        let mut frame = Frame::nefrex()
            .line_color(white)
            .bg_color(dark_bg)
            .glow_color(cyan_glow)
            .stroke_width(2.0)
            .corner_config(CornerConfig::all())
            .square_size(14.0)
            .small_line_length(14.0)
            .large_line_length(50.0)
            .animation_mode(FrameAnimation::Assemble)
            .draw_direction(DrawDirection::CenterOut)
            .animation_progress(seq.frame_progress);

        frame.paint(Bounds::new(pane_x, pane_y, pane_w, pane_h), &mut cx);

        if seq.text_alpha > 0.0 {
            let text_fade = seq.text_alpha * fade;
            
            let title = "OpenAgents";
            let title_run = cx.text.layout(
                title,
                Point::new(pane_x + 30.0, pane_y + 40.0),
                32.0,
                Hsla::new(0.0, 0.0, 1.0, text_fade),
            );
            cx.scene.draw_text(title_run);

            let subtitle = "Decentralized AI Infrastructure";
            let sub_run = cx.text.layout(
                subtitle,
                Point::new(pane_x + 30.0, pane_y + 80.0),
                16.0,
                Hsla::new(0.0, 0.0, 0.7, text_fade),
            );
            cx.scene.draw_text(sub_run);

            let body_lines = [
                "Build autonomous agents",
                "Deploy on decentralized compute", 
                "Earn Bitcoin for contributions",
            ];

            for (i, line) in body_lines.iter().enumerate() {
                let y = pane_y + 130.0 + (i as f32 * 28.0);
                let bullet = cx.text.layout(
                    "›",
                    Point::new(pane_x + 30.0, y),
                    14.0,
                    Hsla::new(0.5, 1.0, 0.6, text_fade),
                );
                cx.scene.draw_text(bullet);

                let text_run = cx.text.layout(
                    line,
                    Point::new(pane_x + 50.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.85, text_fade),
                );
                cx.scene.draw_text(text_run);
            }

            let footer = "[R] Restart  [Esc] Exit";
            let footer_run = cx.text.layout(
                footer,
                Point::new(pane_x + pane_w - 180.0, pane_y + pane_h - 30.0),
                11.0,
                Hsla::new(0.0, 0.0, 0.5, text_fade * 0.7),
            );
            cx.scene.draw_text(footer_run);
        }
    }
}
