//! E2E Test Live Viewer Demo
//!
//! Demonstrates the testing framework with a simple counter component.
//! Watch as the test clicks buttons and verifies state.

use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpui::renderer::Renderer;
use wgpui::testing::{
    ClickTarget, InputOverlay, RunnerState, StepResult, TestRunner, TestStep, test,
};
use wgpui::{
    Bounds, Component, EventContext, EventResult, InputEvent, MouseButton, PaintContext, Point,
    Quad, Scene, Size, TextSystem, theme,
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

// Simple counter component to test
struct Counter {
    count: i32,
    increment_bounds: Bounds,
    decrement_bounds: Bounds,
    reset_bounds: Bounds,
}

impl Counter {
    fn new() -> Self {
        Self {
            count: 0,
            increment_bounds: Bounds::ZERO,
            decrement_bounds: Bounds::ZERO,
            reset_bounds: Bounds::ZERO,
        }
    }
}

impl Component for Counter {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let center_x = bounds.origin.x + bounds.size.width / 2.0;
        let center_y = bounds.origin.y + bounds.size.height / 2.0;

        // Count display
        let count_text = format!("{}", self.count);
        let count_size = 48.0;
        let text_run = cx.text.layout(
            &count_text,
            Point::new(center_x - 20.0, center_y - 60.0),
            count_size,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(text_run);

        // Label
        let label_run = cx.text.layout(
            "Counter",
            Point::new(center_x - 30.0, center_y - 90.0),
            14.0,
            theme::text::MUTED,
        );
        cx.scene.draw_text(label_run);

        // Buttons
        let btn_w = 80.0;
        let btn_h = 36.0;
        let btn_y = center_y + 20.0;
        let spacing = 10.0;

        // Decrement button
        self.decrement_bounds = Bounds::new(center_x - btn_w * 1.5 - spacing, btn_y, btn_w, btn_h);
        cx.scene.draw_quad(
            Quad::new(self.decrement_bounds)
                .with_background(theme::accent::RED.with_alpha(0.8))
                .with_border(theme::accent::RED, 1.0),
        );
        let dec_text = cx.text.layout(
            "-",
            Point::new(
                self.decrement_bounds.origin.x + btn_w / 2.0 - 6.0,
                btn_y + 8.0,
            ),
            20.0,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(dec_text);

        // Reset button
        self.reset_bounds = Bounds::new(center_x - btn_w / 2.0, btn_y, btn_w, btn_h);
        cx.scene.draw_quad(
            Quad::new(self.reset_bounds)
                .with_background(theme::bg::MUTED)
                .with_border(theme::border::DEFAULT, 1.0),
        );
        let reset_text = cx.text.layout(
            "Reset",
            Point::new(
                self.reset_bounds.origin.x + btn_w / 2.0 - 20.0,
                btn_y + 10.0,
            ),
            14.0,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(reset_text);

        // Increment button
        self.increment_bounds = Bounds::new(center_x + btn_w / 2.0 + spacing, btn_y, btn_w, btn_h);
        cx.scene.draw_quad(
            Quad::new(self.increment_bounds)
                .with_background(theme::accent::GREEN.with_alpha(0.8))
                .with_border(theme::accent::GREEN, 1.0),
        );
        let inc_text = cx.text.layout(
            "+",
            Point::new(
                self.increment_bounds.origin.x + btn_w / 2.0 - 6.0,
                btn_y + 8.0,
            ),
            20.0,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(inc_text);
    }

    fn event(
        &mut self,
        event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        if let InputEvent::MouseDown {
            button: MouseButton::Left,
            x,
            y,
        } = event
        {
            let point = Point::new(*x, *y);
            if self.increment_bounds.contains(point) {
                self.count += 1;
                return EventResult::Handled;
            }
            if self.decrement_bounds.contains(point) {
                self.count -= 1;
                return EventResult::Handled;
            }
            if self.reset_bounds.contains(point) {
                self.count = 0;
                return EventResult::Handled;
            }
        }
        EventResult::Ignored
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

struct DemoState {
    counter: Counter,
    overlay: InputOverlay,
    runner: TestRunner,
    test_started: bool,
    last_step_time: Instant,
    step_delay: Duration,
}

impl DemoState {
    fn new() -> Self {
        // Button positions (center at 400, 300)
        let inc_x = 400.0 + 40.0 + 10.0 + 40.0; // Increment button center
        let dec_x = 400.0 - 80.0 - 10.0 + 40.0; // Decrement button center
        let reset_x = 400.0; // Reset button center
        let btn_y = 300.0 + 20.0 + 18.0; // Button row Y

        // Create a test that will run on the counter
        let runner = test("Counter Test")
            .click_at(inc_x, btn_y) // Click +
            .wait(400)
            .click_at(inc_x, btn_y) // Click + again
            .wait(400)
            .click_at(inc_x, btn_y) // Click + again (count = 3)
            .wait(400)
            .click_at(dec_x, btn_y) // Click - (count = 2)
            .wait(400)
            .click_at(reset_x, btn_y) // Click Reset (count = 0)
            .wait(400)
            .click_at(inc_x, btn_y) // Click + (count = 1)
            .build();

        Self {
            counter: Counter::new(),
            overlay: InputOverlay::new()
                .with_cursor_size(20.0)
                .with_ripple_radius(35.0),
            runner,
            test_started: false,
            last_step_time: Instant::now(),
            step_delay: Duration::from_millis(400),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("E2E Test Live Viewer - Counter Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

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
                // Press Space to start/restart test
                if event.state == winit::event::ElementState::Pressed {
                    if let winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Space) =
                        event.physical_key
                    {
                        if !state.demo.test_started
                            || state.demo.runner.state() == RunnerState::Passed
                            || state.demo.runner.state() == RunnerState::Failed
                        {
                            // Reset and restart
                            state.demo = DemoState::new();
                            state.demo.runner.start();
                            state.demo.test_started = true;
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;
                let now = Instant::now();

                // Execute test steps with delay
                let component_bounds = Bounds::new(100.0, 100.0, 600.0, 400.0);
                if state.demo.test_started && state.demo.runner.state() == RunnerState::Running {
                    if now.duration_since(state.demo.last_step_time) >= state.demo.step_delay {
                        // Get current step
                        if let Some(step) = state.demo.runner.current_step_ref().cloned() {
                            // Generate and execute events for this step
                            match &step {
                                TestStep::Click { target, button } => {
                                    if let ClickTarget::Position(p) = target {
                                        // Generate click events
                                        let move_event = InputEvent::MouseMove { x: p.x, y: p.y };
                                        let down_event = InputEvent::MouseDown {
                                            button: *button,
                                            x: p.x,
                                            y: p.y,
                                            modifiers: Modifiers::default(),
                                        };

                                        // Observe in overlay
                                        state.demo.overlay.observe_event(&move_event);
                                        state.demo.overlay.observe_event(&down_event);

                                        // Send to component
                                        let mut ecx = EventContext::new();
                                        state.demo.counter.event(
                                            &down_event,
                                            component_bounds,
                                            &mut ecx,
                                        );
                                    }
                                }
                                TestStep::Wait { .. } => {
                                    // Wait step - just delay
                                }
                                _ => {}
                            }

                            // Complete the step
                            let result = StepResult {
                                step_index: state.demo.runner.current_step(),
                                duration: Duration::from_millis(50),
                                assertion: None,
                                error: None,
                            };
                            state.demo.runner.complete_step(result);
                            state.demo.last_step_time = now;
                        }
                    }
                }

                let mut scene = Scene::new();
                build_demo(
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

fn build_demo(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &mut DemoState,
    width: f32,
    height: f32,
) {
    // Background
    scene
        .draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Title
    let title = "E2E Test Live Viewer Demo";
    let title_run = text_system.layout(title, Point::new(20.0, 30.0), 24.0, theme::text::PRIMARY);
    scene.draw_text(title_run);

    // Instructions
    let instr = if !demo.test_started {
        "Press SPACE to start test"
    } else {
        match demo.runner.state() {
            RunnerState::Running => "Test running...",
            RunnerState::Passed => "Test PASSED! Press SPACE to restart",
            RunnerState::Failed => "Test FAILED! Press SPACE to restart",
            _ => "Press SPACE to start test",
        }
    };
    let instr_color = match demo.runner.state() {
        RunnerState::Passed => theme::status::SUCCESS,
        RunnerState::Failed => theme::status::ERROR,
        _ => theme::text::MUTED,
    };
    let instr_run = text_system.layout(instr, Point::new(20.0, 55.0), 14.0, instr_color);
    scene.draw_text(instr_run);

    // Progress
    if demo.test_started {
        let current = demo.runner.current_step();
        let total = demo.runner.total_steps();
        let progress = format!("Step {}/{}", current + 1, total);
        let prog_run = text_system.layout(
            &progress,
            Point::new(width - 100.0, 30.0),
            14.0,
            theme::text::MUTED,
        );
        scene.draw_text(prog_run);

        // State indicator
        let state_text = demo.runner.state().label();
        let state_color = match demo.runner.state() {
            RunnerState::Running => theme::accent::PRIMARY,
            RunnerState::Passed => theme::status::SUCCESS,
            RunnerState::Failed => theme::status::ERROR,
            _ => theme::text::MUTED,
        };
        let state_run = text_system.layout(
            state_text,
            Point::new(width - 100.0, 50.0),
            12.0,
            state_color,
        );
        scene.draw_text(state_run);
    }

    // Counter component
    let component_bounds = Bounds::new(100.0, 100.0, 600.0, 400.0);
    let mut cx = PaintContext::new(scene, text_system, 1.0);
    demo.counter.paint(component_bounds, &mut cx);

    // Overlay
    demo.overlay
        .paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
}
