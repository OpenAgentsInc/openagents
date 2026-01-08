//! Autopilot Shell - unified HUD with dock-based layout

use std::process::Command;
use std::sync::Arc;
use tracing::info;
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, EventContext, InputEvent, Key, Modifiers, NamedKey, PaintContext, Scene,
    Size, TextSystem,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use autopilot_shell::AutopilotShell;

fn main() {
    // Check for --allow-multiple flag
    let args: Vec<String> = std::env::args().collect();
    let allow_multiple = args.iter().any(|a| a == "--allow-multiple");

    // Kill existing autopilot processes unless --allow-multiple is passed
    if !allow_multiple {
        kill_existing_autopilots();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new(
                        "autopilot=debug,autopilot_shell=debug,openagents=debug,wgpui=info,cosmic_text=warn,wgpu=warn,info"
                    )
                })
        )
        .with_target(true)
        .init();

    info!("Starting Autopilot Shell");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}

#[derive(Default)]
struct App {
    state: Option<RenderState>,
    current_modifiers: Modifiers,
    last_cursor_pos: (f32, f32),
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    shell: AutopilotShell,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Autopilot")
            .with_inner_size(winit::dpi::LogicalSize::new(1600, 1000))
            .with_maximized(true);

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

            let shell = AutopilotShell::new();

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                shell,
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
                    // Check for escape to exit
                    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                        event_loop.exit();
                        return;
                    }

                    // Convert to wgpui InputEvent with current modifiers
                    let key = physical_key_to_key(&event.physical_key);

                    let input_event = InputEvent::KeyDown {
                        key,
                        modifiers: self.current_modifiers,
                    };

                    let scale_factor = state.window.scale_factor() as f32;
                    let width = state.config.width as f32 / scale_factor;
                    let height = state.config.height as f32 / scale_factor;
                    let bounds = Bounds::new(0.0, 0.0, width, height);

                    let mut cx = EventContext::new();
                    let _ = state.shell.event(&input_event, bounds, &mut cx);

                    // Check for fullscreen toggle request
                    if state.shell.take_fullscreen_toggle() {
                        use winit::window::Fullscreen;
                        let current = state.window.fullscreen();
                        if current.is_some() {
                            state.window.set_fullscreen(None);
                        } else {
                            state
                                .window
                                .set_fullscreen(Some(Fullscreen::Borderless(None)));
                        }
                    }
                }
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                // Track current modifier state
                let mods = modifiers.state();
                self.current_modifiers = Modifiers {
                    shift: mods.shift_key(),
                    ctrl: mods.control_key(),
                    alt: mods.alt_key(),
                    meta: mods.super_key(),
                };
            }
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };

                let input_event = InputEvent::Scroll {
                    dx: 0.0,
                    dy: scroll_delta,
                };

                let scale_factor = state.window.scale_factor() as f32;
                let width = state.config.width as f32 / scale_factor;
                let height = state.config.height as f32 / scale_factor;
                let bounds = Bounds::new(0.0, 0.0, width, height);

                let mut cx = EventContext::new();
                let _ = state.shell.event(&input_event, bounds, &mut cx);

                state.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale_factor = state.window.scale_factor() as f32;
                let x = position.x as f32 / scale_factor;
                let y = position.y as f32 / scale_factor;

                // Track cursor position for use in MouseInput events
                self.last_cursor_pos = (x, y);

                let input_event = InputEvent::MouseMove { x, y };

                let width = state.config.width as f32 / scale_factor;
                let height = state.config.height as f32 / scale_factor;
                let bounds = Bounds::new(0.0, 0.0, width, height);

                let mut cx = EventContext::new();
                let _ = state.shell.event(&input_event, bounds, &mut cx);

                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                let scale_factor = state.window.scale_factor() as f32;
                let width = state.config.width as f32 / scale_factor;
                let height = state.config.height as f32 / scale_factor;
                let bounds = Bounds::new(0.0, 0.0, width, height);

                // Use tracked cursor position from CursorMoved events
                let (x, y) = self.last_cursor_pos;

                let mouse_button = match button {
                    winit::event::MouseButton::Left => wgpui::MouseButton::Left,
                    winit::event::MouseButton::Right => wgpui::MouseButton::Right,
                    winit::event::MouseButton::Middle => wgpui::MouseButton::Middle,
                    _ => wgpui::MouseButton::Left,
                };

                let input_event = if button_state.is_pressed() {
                    InputEvent::MouseDown {
                        button: mouse_button,
                        x,
                        y,
                    }
                } else {
                    InputEvent::MouseUp {
                        button: mouse_button,
                        x,
                        y,
                    }
                };

                let mut cx = EventContext::new();
                let _ = state.shell.event(&input_event, bounds, &mut cx);

                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let scale_factor = state.window.scale_factor() as f32;
                let width = state.config.width as f32 / scale_factor;
                let height = state.config.height as f32 / scale_factor;

                let mut scene = Scene::new();
                let mut cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);

                let bounds = Bounds::new(0.0, 0.0, width, height);
                state.shell.paint(bounds, &mut cx);

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
                    Size::new(width, height),  // Pass logical size, resize() multiplies by scale
                    scale_factor,
                );

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

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

/// Convert winit PhysicalKey to wgpui Key
fn physical_key_to_key(physical_key: &PhysicalKey) -> Key {
    match physical_key {
        PhysicalKey::Code(code) => match code {
            KeyCode::Escape => Key::Named(NamedKey::Escape),
            KeyCode::Enter => Key::Named(NamedKey::Enter),
            KeyCode::Tab => Key::Named(NamedKey::Tab),
            KeyCode::Backspace => Key::Named(NamedKey::Backspace),
            KeyCode::Delete => Key::Named(NamedKey::Delete),
            KeyCode::ArrowUp => Key::Named(NamedKey::ArrowUp),
            KeyCode::ArrowDown => Key::Named(NamedKey::ArrowDown),
            KeyCode::ArrowLeft => Key::Named(NamedKey::ArrowLeft),
            KeyCode::ArrowRight => Key::Named(NamedKey::ArrowRight),
            KeyCode::Home => Key::Named(NamedKey::Home),
            KeyCode::End => Key::Named(NamedKey::End),
            KeyCode::PageUp => Key::Named(NamedKey::PageUp),
            KeyCode::PageDown => Key::Named(NamedKey::PageDown),
            KeyCode::KeyA => Key::Character("a".to_string()),
            KeyCode::KeyB => Key::Character("b".to_string()),
            KeyCode::KeyC => Key::Character("c".to_string()),
            KeyCode::KeyD => Key::Character("d".to_string()),
            KeyCode::KeyE => Key::Character("e".to_string()),
            KeyCode::KeyF => Key::Character("f".to_string()),
            KeyCode::KeyG => Key::Character("g".to_string()),
            KeyCode::KeyH => Key::Character("h".to_string()),
            KeyCode::KeyI => Key::Character("i".to_string()),
            KeyCode::KeyJ => Key::Character("j".to_string()),
            KeyCode::KeyK => Key::Character("k".to_string()),
            KeyCode::KeyL => Key::Character("l".to_string()),
            KeyCode::KeyM => Key::Character("m".to_string()),
            KeyCode::KeyN => Key::Character("n".to_string()),
            KeyCode::KeyO => Key::Character("o".to_string()),
            KeyCode::KeyP => Key::Character("p".to_string()),
            KeyCode::KeyQ => Key::Character("q".to_string()),
            KeyCode::KeyR => Key::Character("r".to_string()),
            KeyCode::KeyS => Key::Character("s".to_string()),
            KeyCode::KeyT => Key::Character("t".to_string()),
            KeyCode::KeyU => Key::Character("u".to_string()),
            KeyCode::KeyV => Key::Character("v".to_string()),
            KeyCode::KeyW => Key::Character("w".to_string()),
            KeyCode::KeyX => Key::Character("x".to_string()),
            KeyCode::KeyY => Key::Character("y".to_string()),
            KeyCode::KeyZ => Key::Character("z".to_string()),
            KeyCode::Digit0 => Key::Character("0".to_string()),
            KeyCode::Digit1 => Key::Character("1".to_string()),
            KeyCode::Digit2 => Key::Character("2".to_string()),
            KeyCode::Digit3 => Key::Character("3".to_string()),
            KeyCode::Digit4 => Key::Character("4".to_string()),
            KeyCode::Digit5 => Key::Character("5".to_string()),
            KeyCode::Digit6 => Key::Character("6".to_string()),
            KeyCode::Digit7 => Key::Character("7".to_string()),
            KeyCode::Digit8 => Key::Character("8".to_string()),
            KeyCode::Digit9 => Key::Character("9".to_string()),
            KeyCode::Backslash => Key::Character("\\".to_string()),
            KeyCode::BracketLeft => Key::Character("[".to_string()),
            KeyCode::BracketRight => Key::Character("]".to_string()),
            _ => Key::Named(NamedKey::Unidentified),
        },
        PhysicalKey::Unidentified(_) => Key::Named(NamedKey::Unidentified),
    }
}

/// Kill any existing autopilot processes (except self)
fn kill_existing_autopilots() {
    let my_pid = std::process::id();

    // Use pgrep to find autopilot processes, then kill them
    if let Ok(output) = Command::new("pgrep")
        .arg("-f")
        .arg("target/debug/autopilot")
        .output()
    {
        let pids = String::from_utf8_lossy(&output.stdout);
        for line in pids.lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                if pid != my_pid {
                    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).output();
                }
            }
        }
    }

    // Also check for release builds
    if let Ok(output) = Command::new("pgrep")
        .arg("-f")
        .arg("target/release/autopilot")
        .output()
    {
        let pids = String::from_utf8_lossy(&output.stdout);
        for line in pids.lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                if pid != my_pid {
                    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).output();
                }
            }
        }
    }
}
