//! HUD Demo - showcases all HUD components.
//!
//! Run with: `cargo hud` or `cargo run --bin hud_demo`

use std::sync::Arc;

use hud::{
    animator::{AnimatorManager, ManagerMode},
    background::DotGridBackground,
    button::HudButton,
    frame::{FrameCorners, FrameLines, FrameSides},
    theme::{hud as colors, timing},
};
use wgpui::platform::desktop::{create_window, DesktopPlatform};
use wgpui::platform::Platform;
use wgpui::{Bounds, Point, Quad, Scene};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

struct HudDemo {
    platform: Option<DesktopPlatform>,

    // Components
    background: DotGridBackground,
    main_frame: FrameCorners,
    title_lines: FrameLines,
    button_manager: AnimatorManager,
    buttons: Vec<HudButton>,

    // Panels with staggered animation
    panels: Vec<FrameCorners>,
    panel_manager: AnimatorManager,

    // State
    window_size: (f32, f32),
    started: bool,
}

impl HudDemo {
    fn new() -> Self {
        // Create buttons
        let buttons = vec![
            HudButton::new("CONNECT")
                .font_size(12.0)
                .corner_length(10.0),
            HudButton::new("SCAN")
                .font_size(12.0)
                .corner_length(10.0),
            HudButton::new("ABORT")
                .font_size(12.0)
                .corner_length(10.0),
        ];

        // Create button manager for staggered animation
        let mut button_manager = AnimatorManager::new(ManagerMode::Stagger)
            .stagger_offset(timing::STAGGER_OFFSET);
        for _ in &buttons {
            button_manager.add_child(
                hud::animator::HudAnimator::new().enter_duration(timing::ENTER_FRAMES),
            );
        }

        // Create panels
        let panels = vec![
            FrameCorners::new()
                .corner_length(15.0)
                .color(colors::FRAME_DIM),
            FrameCorners::new()
                .corner_length(15.0)
                .color(colors::FRAME_DIM),
            FrameCorners::new()
                .corner_length(15.0)
                .color(colors::FRAME_DIM),
        ];

        // Create panel manager for staggered animation
        let mut panel_manager = AnimatorManager::new(ManagerMode::Stagger)
            .stagger_offset(timing::STAGGER_OFFSET * 2);
        for _ in &panels {
            panel_manager
                .add_child(hud::animator::HudAnimator::new().enter_duration(timing::ENTER_FRAMES));
        }

        Self {
            platform: None,
            background: DotGridBackground::new()
                .spacing(25.0)
                .dot_radius(1.0)
                .color(colors::DOT_GRID),
            main_frame: FrameCorners::new()
                .corner_length(40.0)
                .line_width(2.0)
                .color(colors::FRAME_BRIGHT),
            title_lines: FrameLines::new()
                .sides(FrameSides::horizontal())
                .gap(100.0)
                .line_width(1.0)
                .color(colors::FRAME_DIM),
            button_manager,
            buttons,
            panels,
            panel_manager,
            window_size: (1280.0, 720.0),
            started: false,
        }
    }

    fn start_animations(&mut self) {
        log::info!("Starting HUD animations");

        self.background.animator_mut().enter();
        self.main_frame.animator_mut().enter();
        self.title_lines.animator_mut().enter();
        self.button_manager.enter();
        self.panel_manager.enter();

        // Start button animators
        for button in &mut self.buttons {
            button.animator_mut().enter();
        }

        // Start panel animators
        for panel in &mut self.panels {
            panel.animator_mut().enter();
        }

        self.started = true;
    }

    fn tick(&mut self) {
        self.background.tick();
        self.main_frame.tick();
        self.title_lines.tick();
        self.button_manager.tick();
        self.panel_manager.tick();

        // Sync button animators with manager
        for (i, button) in self.buttons.iter_mut().enumerate() {
            if let Some(managed) = self.button_manager.child(i) {
                // Copy progress from manager's child to button
                if managed.state().is_entered() {
                    button.animator_mut().set_entered();
                }
            }
            button.tick();
        }

        // Sync panel animators with manager
        for (i, panel) in self.panels.iter_mut().enumerate() {
            if let Some(managed) = self.panel_manager.child(i) {
                if managed.state().is_entered() {
                    panel.animator_mut().set_entered();
                }
            }
            panel.tick();
        }
    }

    fn paint(&self, scene: &mut Scene, text_system: &mut wgpui::TextSystem) {
        let (width, height) = self.window_size;
        let screen = Bounds::new(0.0, 0.0, width, height);

        // Black background
        scene.draw_quad(Quad::new(screen).with_background(colors::BG));

        // Dot grid
        self.background.paint(screen, scene);

        // Main frame with padding
        let main_bounds = Bounds::new(40.0, 40.0, width - 80.0, height - 80.0);
        self.main_frame.paint(main_bounds, scene);

        // Title area
        let title_bounds = Bounds::new(60.0, 50.0, width - 120.0, 40.0);
        self.title_lines.paint(title_bounds, scene);

        // Title text
        let title_progress = self.main_frame.animator().progress();
        let title_color = wgpui::Hsla::new(
            colors::TEXT.h,
            colors::TEXT.s,
            colors::TEXT.l,
            colors::TEXT.a * title_progress,
        );
        let title = text_system.layout(
            "HUD SYSTEM v1.0",
            Point::new(main_bounds.origin.x + 60.0, main_bounds.origin.y + 15.0),
            16.0,
            title_color,
        );
        scene.draw_text(title);

        // Status text
        let status_color = wgpui::Hsla::new(
            colors::TEXT_MUTED.h,
            colors::TEXT_MUTED.s,
            colors::TEXT_MUTED.l,
            colors::TEXT_MUTED.a * title_progress,
        );
        let status = text_system.layout(
            "STATUS: ONLINE",
            Point::new(main_bounds.origin.x + main_bounds.size.width - 200.0, main_bounds.origin.y + 18.0),
            12.0,
            status_color,
        );
        scene.draw_text(status);

        // Left panels (staggered)
        let panel_width = 200.0;
        let panel_height = 120.0;
        let panel_x = main_bounds.origin.x + 20.0;
        let panel_start_y = main_bounds.origin.y + 80.0;
        let panel_spacing = 15.0;

        for (i, panel) in self.panels.iter().enumerate() {
            let panel_y = panel_start_y + (i as f32) * (panel_height + panel_spacing);
            let panel_bounds = Bounds::new(panel_x, panel_y, panel_width, panel_height);
            panel.paint(panel_bounds, scene);

            // Panel label
            let panel_progress = panel.animator().progress();
            let label_color = wgpui::Hsla::new(
                colors::TEXT_MUTED.h,
                colors::TEXT_MUTED.s,
                colors::TEXT_MUTED.l,
                colors::TEXT_MUTED.a * panel_progress,
            );
            let labels = ["SENSORS", "COMMS", "POWER"];
            if i < labels.len() {
                let label = text_system.layout(
                    labels[i],
                    Point::new(panel_x + 15.0, panel_y + 15.0),
                    10.0,
                    label_color,
                );
                scene.draw_text(label);
            }
        }

        // Buttons (right side, staggered)
        let button_width = 120.0;
        let button_height = 36.0;
        let button_x = main_bounds.origin.x + main_bounds.size.width - button_width - 40.0;
        let button_start_y = main_bounds.origin.y + 100.0;
        let button_spacing = 12.0;

        for (i, button) in self.buttons.iter().enumerate() {
            let button_y = button_start_y + (i as f32) * (button_height + button_spacing);
            let button_bounds = Bounds::new(button_x, button_y, button_width, button_height);
            button.paint(button_bounds, scene, text_system);
        }

        // Center content area with FrameLines
        let content_x = panel_x + panel_width + 30.0;
        let content_y = panel_start_y;
        let content_width = button_x - content_x - 40.0;
        let content_height = 3.0 * panel_height + 2.0 * panel_spacing;
        let content_bounds = Bounds::new(content_x, content_y, content_width, content_height);

        // Draw content frame
        let mut content_frame = FrameLines::new()
            .sides(FrameSides::all())
            .gap(0.0)
            .line_width(1.0)
            .color(colors::FRAME_DIM);
        content_frame.animator_mut().set_entered();
        let content_progress = self.main_frame.animator().progress();
        if content_progress > 0.0 {
            // Manually set progress via alpha
            let color = wgpui::Hsla::new(
                colors::FRAME_DIM.h,
                colors::FRAME_DIM.s,
                colors::FRAME_DIM.l,
                colors::FRAME_DIM.a * content_progress,
            );
            content_frame = content_frame.color(color);
            content_frame.paint(content_bounds, scene);
        }

        // Content text
        let content_text_color = wgpui::Hsla::new(
            colors::TEXT_MUTED.h,
            colors::TEXT_MUTED.s,
            colors::TEXT_MUTED.l,
            colors::TEXT_MUTED.a * content_progress,
        );
        let content_text = text_system.layout(
            "MAIN DISPLAY AREA",
            Point::new(
                content_bounds.origin.x + content_bounds.size.width / 2.0 - 60.0,
                content_bounds.origin.y + content_bounds.size.height / 2.0 - 6.0,
            ),
            12.0,
            content_text_color,
        );
        scene.draw_text(content_text);
    }

    fn handle_input(&mut self, event: &wgpui::InputEvent) {
        let (width, height) = self.window_size;
        let main_bounds = Bounds::new(40.0, 40.0, width - 80.0, height - 80.0);

        let button_width = 120.0;
        let button_height = 36.0;
        let button_x = main_bounds.origin.x + main_bounds.size.width - button_width - 40.0;
        let button_start_y = main_bounds.origin.y + 100.0;
        let button_spacing = 12.0;

        for (i, button) in self.buttons.iter_mut().enumerate() {
            let button_y = button_start_y + (i as f32) * (button_height + button_spacing);
            let button_bounds = Bounds::new(button_x, button_y, button_width, button_height);
            button.event(event, button_bounds);
        }
    }
}

impl ApplicationHandler for HudDemo {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.platform.is_none() {
            let window = create_window(event_loop, "HUD Demo", 1280, 720)
                .expect("Failed to create window");
            let window = Arc::new(window);
            let platform = DesktopPlatform::new(window).expect("Failed to initialize platform");

            let size = platform.logical_size();
            self.window_size = (size.width, size.height);

            self.platform = Some(platform);
            log::info!("HUD Demo initialized");
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.platform.is_none() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                if let Some(platform) = &mut self.platform {
                    platform.handle_resize();
                    let size = platform.logical_size();
                    self.window_size = (size.width, size.height);
                    platform.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                // Start animations on first frame
                if !self.started {
                    self.start_animations();
                }

                // Update
                self.tick();

                // Take platform temporarily for paint
                let mut platform = self.platform.take();
                if let Some(ref mut p) = platform {
                    let mut scene = Scene::new();
                    self.paint(&mut scene, p.text_system());

                    // Render
                    if let Err(e) = p.render(&scene) {
                        log::error!("Render error: {}", e);
                    }

                    // Continuous redraw for animations
                    p.request_redraw();
                }
                // Put platform back
                self.platform = platform;
            }
            ref e => {
                // Handle input events
                let input_event = if let Some(platform) = &mut self.platform {
                    platform.handle_window_event(e)
                } else {
                    None
                };

                if let Some(input_event) = input_event {
                    self.handle_input(&input_event);
                    if let Some(platform) = &mut self.platform {
                        platform.request_redraw();
                    }
                }
            }
        }
    }
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("Starting HUD Demo...");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll); // Continuous for animations

    let mut demo = HudDemo::new();
    event_loop.run_app(&mut demo).unwrap();
}
