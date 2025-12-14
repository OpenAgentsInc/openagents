//! HUD Demo - showcases all HUD components.
//!
//! Run with: `cargo hud` or `cargo run --bin hud_demo`

use std::sync::Arc;

use hud::{
    animator::{AnimatorManager, ManagerMode},
    background::{DotGridBackground, GridLinesBackground, LineDirection, MovingLinesBackground},
    button::HudButton,
    effects::Illuminator,
    frame::{FrameCircle, FrameCorners, FrameHeader, FrameLines, FrameOctagon, FrameSides, FrameUnderline},
    text::{TextDecipher, TextSequence},
    theme::{hud as colors, timing},
};
use wgpui::platform::desktop::{create_window, DesktopPlatform};
use wgpui::platform::Platform;
use wgpui::{Bounds, InputEvent, Point, Quad, Scene};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

struct HudDemo {
    platform: Option<DesktopPlatform>,

    // Backgrounds
    dot_grid: DotGridBackground,
    grid_lines: GridLinesBackground,
    moving_lines: MovingLinesBackground,

    // Effects
    illuminator: Illuminator,

    // Main frame
    main_frame: FrameCorners,
    title_lines: FrameLines,

    // Text animations
    title_text: TextSequence,
    status_text: TextDecipher,

    // Buttons
    button_manager: AnimatorManager,
    buttons: Vec<HudButton>,

    // Left panels with staggered animation
    panels: Vec<FrameCorners>,
    panel_manager: AnimatorManager,

    // Frame showcases
    octagon_frame: FrameOctagon,
    circle_frame: FrameCircle,
    header_frame: FrameHeader,
    underline_frame: FrameUnderline,

    // State
    window_size: (f32, f32),
    mouse_pos: Point,
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
            // Backgrounds (layered)
            dot_grid: DotGridBackground::new()
                .spacing(25.0)
                .dot_radius(1.0)
                .color(colors::DOT_GRID),
            grid_lines: GridLinesBackground::new()
                .spacing(100.0)
                .line_width(1.0)
                .color(wgpui::Hsla::new(0.0, 0.0, 1.0, 0.03)),
            moving_lines: MovingLinesBackground::new()
                .spacing(60.0)
                .line_width(1.0)
                .speed(0.5)
                .direction(LineDirection::Down)
                .color(wgpui::Hsla::new(0.0, 0.0, 1.0, 0.02)),
            // Effects
            illuminator: Illuminator::new()
                .size(200.0)
                .color(wgpui::Hsla::new(0.0, 0.0, 1.0, 0.08))
                .smoothing(0.1),
            // Main frame
            main_frame: FrameCorners::new()
                .corner_length(40.0)
                .line_width(2.0)
                .color(colors::FRAME_BRIGHT),
            title_lines: FrameLines::new()
                .sides(FrameSides::horizontal())
                .gap(100.0)
                .line_width(1.0)
                .color(colors::FRAME_DIM),
            // Text animations
            title_text: TextSequence::new("HUD SYSTEM v2.0")
                .font_size(16.0)
                .color(colors::TEXT)
                .show_cursor(false),
            status_text: TextDecipher::new("STATUS: ONLINE")
                .font_size(12.0)
                .color(colors::TEXT_MUTED)
                .scramble_speed(2),
            button_manager,
            buttons,
            panels,
            panel_manager,
            // Frame showcases
            octagon_frame: FrameOctagon::new()
                .corner_size(15.0)
                .line_width(1.5)
                .color(colors::FRAME_NORMAL),
            circle_frame: FrameCircle::new()
                .line_width(1.5)
                .segments(48)
                .color(colors::FRAME_NORMAL),
            header_frame: FrameHeader::new()
                .line_width(1.5)
                .accent_size(12.0)
                .color(colors::FRAME_NORMAL)
                .show_bottom(true),
            underline_frame: FrameUnderline::new()
                .line_width(2.0)
                .color(colors::FRAME_NORMAL),
            window_size: (1280.0, 720.0),
            mouse_pos: Point::new(0.0, 0.0),
            started: false,
        }
    }

    fn start_animations(&mut self) {
        log::info!("Starting HUD animations");

        // Backgrounds
        self.dot_grid.animator_mut().enter();
        self.grid_lines.animator_mut().enter();
        self.moving_lines.animator_mut().enter();

        // Effects
        self.illuminator.animator_mut().enter();

        // Frames
        self.main_frame.animator_mut().enter();
        self.title_lines.animator_mut().enter();
        self.octagon_frame.animator_mut().enter();
        self.circle_frame.animator_mut().enter();
        self.header_frame.animator_mut().enter();
        self.underline_frame.animator_mut().enter();

        // Text animations
        self.title_text.animator_mut().enter();
        self.status_text.animator_mut().enter();

        // Managers
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
        // Backgrounds
        self.dot_grid.tick();
        self.grid_lines.tick();
        self.moving_lines.tick();

        // Effects
        self.illuminator.tick();

        // Frames
        self.main_frame.tick();
        self.title_lines.tick();
        self.octagon_frame.tick();
        self.circle_frame.tick();
        self.header_frame.tick();
        self.underline_frame.tick();

        // Text animations
        self.title_text.tick();
        self.status_text.tick();

        // Managers
        self.button_manager.tick();
        self.panel_manager.tick();

        // Sync button animators with manager
        for (i, button) in self.buttons.iter_mut().enumerate() {
            if let Some(managed) = self.button_manager.child(i) {
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

        // Layered backgrounds (back to front)
        self.grid_lines.paint(screen, scene);
        self.moving_lines.paint(screen, scene);
        self.dot_grid.paint(screen, scene);

        // Illuminator effect (follows mouse)
        self.illuminator.paint(screen, scene);

        // Main frame with padding
        let main_bounds = Bounds::new(40.0, 40.0, width - 80.0, height - 80.0);
        self.main_frame.paint(main_bounds, scene);

        // Title area
        let title_bounds = Bounds::new(60.0, 50.0, width - 120.0, 40.0);
        self.title_lines.paint(title_bounds, scene);

        // Title text (animated sequence)
        self.title_text.paint(
            Point::new(main_bounds.origin.x + 60.0, main_bounds.origin.y + 15.0),
            scene,
            text_system,
        );

        // Status text (animated decipher)
        self.status_text.paint(
            Point::new(main_bounds.origin.x + main_bounds.size.width - 200.0, main_bounds.origin.y + 18.0),
            scene,
            text_system,
        );

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

        // Center content area
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
            let color = wgpui::Hsla::new(
                colors::FRAME_DIM.h,
                colors::FRAME_DIM.s,
                colors::FRAME_DIM.l,
                colors::FRAME_DIM.a * content_progress,
            );
            content_frame = content_frame.color(color);
            content_frame.paint(content_bounds, scene);
        }

        // Frame showcase - 2x2 grid
        let showcase_size = 80.0;
        let showcase_spacing = 20.0;
        let grid_width = showcase_size * 2.0 + showcase_spacing;
        let grid_start_x = content_bounds.origin.x + (content_bounds.size.width - grid_width) / 2.0;
        let grid_start_y = content_bounds.origin.y + 50.0;

        let content_text_color = wgpui::Hsla::new(
            colors::TEXT_MUTED.h,
            colors::TEXT_MUTED.s,
            colors::TEXT_MUTED.l,
            colors::TEXT_MUTED.a * content_progress,
        );

        // Row 1: Octagon and Circle
        let octagon_bounds = Bounds::new(grid_start_x, grid_start_y, showcase_size, showcase_size);
        self.octagon_frame.paint(octagon_bounds, scene);
        let octagon_label = text_system.layout(
            "OCTAGON",
            Point::new(grid_start_x + 15.0, grid_start_y + showcase_size + 5.0),
            9.0,
            content_text_color,
        );
        scene.draw_text(octagon_label);

        let circle_bounds = Bounds::new(grid_start_x + showcase_size + showcase_spacing, grid_start_y, showcase_size, showcase_size);
        self.circle_frame.paint(circle_bounds, scene);
        let circle_label = text_system.layout(
            "CIRCLE",
            Point::new(grid_start_x + showcase_size + showcase_spacing + 20.0, grid_start_y + showcase_size + 5.0),
            9.0,
            content_text_color,
        );
        scene.draw_text(circle_label);

        // Row 2: Header and Underline
        let row2_y = grid_start_y + showcase_size + 40.0;
        let header_bounds = Bounds::new(grid_start_x, row2_y, showcase_size, showcase_size);
        self.header_frame.paint(header_bounds, scene);
        let header_label = text_system.layout(
            "HEADER",
            Point::new(grid_start_x + 20.0, row2_y + showcase_size + 5.0),
            9.0,
            content_text_color,
        );
        scene.draw_text(header_label);

        let underline_bounds = Bounds::new(grid_start_x + showcase_size + showcase_spacing, row2_y, showcase_size, showcase_size);
        self.underline_frame.paint(underline_bounds, scene);
        let underline_label = text_system.layout(
            "UNDERLINE",
            Point::new(grid_start_x + showcase_size + showcase_spacing + 10.0, row2_y + showcase_size + 5.0),
            9.0,
            content_text_color,
        );
        scene.draw_text(underline_label);

        // Content title
        let content_title = text_system.layout(
            "FRAME SHOWCASE",
            Point::new(content_bounds.origin.x + content_bounds.size.width / 2.0 - 50.0, content_bounds.origin.y + 15.0),
            12.0,
            content_text_color,
        );
        scene.draw_text(content_title);

        // Info text at bottom
        let info_text = text_system.layout(
            "MOVE MOUSE FOR ILLUMINATOR EFFECT",
            Point::new(content_bounds.origin.x + content_bounds.size.width / 2.0 - 100.0, content_bounds.origin.y + content_bounds.size.height - 25.0),
            9.0,
            content_text_color,
        );
        scene.draw_text(info_text);
    }

    fn handle_input(&mut self, event: &InputEvent) {
        // Update illuminator position on mouse move
        if let InputEvent::MouseMove { position, .. } = event {
            self.mouse_pos = *position;
            self.illuminator.set_position(position.x, position.y);
        }

        // Handle button events
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
            let window = create_window(event_loop, "HUD Demo - Arwes Port", 1280, 720)
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
