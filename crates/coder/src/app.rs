//! Main application state and event handling.

use std::sync::Arc;
use web_time::Instant;
use wgpui::components::{Component, EventContext, PaintContext};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Hsla, InputEvent, Quad, Scene, Size, TextInput, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::Key;
use winit::window::{Window, WindowId};

const INPUT_HEIGHT: f32 = 40.0;
const INPUT_PADDING: f32 = 12.0;

/// Render state holding all GPU and UI resources
struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    event_context: EventContext,
    input: TextInput,
    mouse_pos: (f32, f32),
    #[allow(dead_code)]
    last_tick: Instant,
}

/// Main application
#[derive(Default)]
pub struct CoderApp {
    state: Option<RenderState>,
}

impl ApplicationHandler for CoderApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Coder")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 600));

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
            let event_context = EventContext::new();

            // Create input with terminal styling - extra left padding for ">" prompt
            let mut input = TextInput::new()
                .with_id(1)
                .font_size(14.0)
                .padding(28.0, 10.0) // Extra left padding for prompt character
                .background(Hsla::new(220.0, 0.15, 0.08, 1.0))
                .border_color(Hsla::new(220.0, 0.15, 0.25, 1.0)) // Unfocused: dark gray
                .border_color_focused(Hsla::new(0.0, 0.0, 1.0, 1.0)) // Focused: white
                .mono(true);
            input.focus();

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                event_context,
                input,
                mouse_pos: (0.0, 0.0),
                last_tick: Instant::now(),
            }
        });

        let window_clone = state.window.clone();
        self.state = Some(state);
        tracing::info!("Window initialized");

        // Request initial redraw
        window_clone.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };

        let scale_factor = state.window.scale_factor() as f32;
        let logical_width = state.config.width as f32 / scale_factor;
        let logical_height = state.config.height as f32 / scale_factor;

        // Input bounds at bottom of window
        let input_bounds = Bounds::new(
            INPUT_PADDING,
            logical_height - INPUT_HEIGHT - INPUT_PADDING,
            logical_width - INPUT_PADDING * 2.0,
            INPUT_HEIGHT,
        );

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                state.config.width = size.width.max(1);
                state.config.height = size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32 / scale_factor;
                let y = position.y as f32 / scale_factor;
                state.mouse_pos = (x, y);
                let input_event = InputEvent::MouseMove { x, y };
                state
                    .input
                    .event(&input_event, input_bounds, &mut state.event_context);
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                let (x, y) = state.mouse_pos;
                let modifiers = wgpui::Modifiers::default();
                let input_event = if button_state == ElementState::Pressed {
                    InputEvent::MouseDown {
                        button: convert_mouse_button(button),
                        x,
                        y,
                        modifiers,
                    }
                } else {
                    InputEvent::MouseUp {
                        button: convert_mouse_button(button),
                        x,
                        y,
                    }
                };
                state
                    .input
                    .event(&input_event, input_bounds, &mut state.event_context);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event: key_event, .. } => {
                if key_event.state == ElementState::Pressed {
                    let modifiers = wgpui::Modifiers::default();

                    let key = match &key_event.logical_key {
                        Key::Character(c) => wgpui::input::Key::Character(c.to_string()),
                        Key::Named(named) => {
                            wgpui::input::Key::Named(convert_named_key(*named))
                        }
                        _ => return,
                    };

                    let input_event = InputEvent::KeyDown { key, modifiers };
                    state
                        .input
                        .event(&input_event, input_bounds, &mut state.event_context);
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

impl CoderApp {
    fn render(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let scale_factor = state.window.scale_factor() as f32;
        let logical_width = state.config.width as f32 / scale_factor;
        let logical_height = state.config.height as f32 / scale_factor;

        // Get surface texture
        let output = match state.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                state.surface.configure(&state.device, &state.config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                tracing::error!("Out of memory");
                return;
            }
            Err(_) => return,
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Build scene
        let mut scene = Scene::new();
        let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);

        // Dark terminal background
        scene.draw_quad(Quad::new(bounds).with_background(Hsla::new(220.0, 0.15, 0.10, 1.0)));

        // Paint input at bottom
        let input_bounds = Bounds::new(
            INPUT_PADDING,
            logical_height - INPUT_HEIGHT - INPUT_PADDING,
            logical_width - INPUT_PADDING * 2.0,
            INPUT_HEIGHT,
        );

        let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
        state.input.paint(input_bounds, &mut paint_cx);

        // Draw ">" prompt inside input
        let prompt_run = state.text_system.layout_styled_mono(
            ">",
            wgpui::Point::new(
                input_bounds.origin.x + 12.0,
                input_bounds.origin.y + input_bounds.size.height * 0.5 - 7.0,
            ),
            14.0,
            Hsla::new(0.0, 0.0, 0.6, 1.0), // Gray prompt
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(prompt_run);

        // Render
        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Coder Render"),
            });

        let physical_width = state.config.width as f32;
        let physical_height = state.config.height as f32;

        // Resize renderer to match window
        state.renderer.resize(
            &state.queue,
            Size::new(physical_width, physical_height),
            1.0,
        );

        // Update text atlas if needed
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
}

fn convert_mouse_button(button: winit::event::MouseButton) -> wgpui::MouseButton {
    match button {
        winit::event::MouseButton::Left => wgpui::MouseButton::Left,
        winit::event::MouseButton::Right => wgpui::MouseButton::Right,
        winit::event::MouseButton::Middle => wgpui::MouseButton::Middle,
        _ => wgpui::MouseButton::Left,
    }
}

fn convert_named_key(key: winit::keyboard::NamedKey) -> wgpui::input::NamedKey {
    use winit::keyboard::NamedKey as WinitKey;
    use wgpui::input::NamedKey;

    match key {
        WinitKey::Enter => NamedKey::Enter,
        WinitKey::Tab => NamedKey::Tab,
        WinitKey::Space => NamedKey::Space,
        WinitKey::Backspace => NamedKey::Backspace,
        WinitKey::Delete => NamedKey::Delete,
        WinitKey::Escape => NamedKey::Escape,
        WinitKey::ArrowUp => NamedKey::ArrowUp,
        WinitKey::ArrowDown => NamedKey::ArrowDown,
        WinitKey::ArrowLeft => NamedKey::ArrowLeft,
        WinitKey::ArrowRight => NamedKey::ArrowRight,
        WinitKey::Home => NamedKey::Home,
        WinitKey::End => NamedKey::End,
        WinitKey::PageUp => NamedKey::PageUp,
        WinitKey::PageDown => NamedKey::PageDown,
        _ => NamedKey::Tab, // fallback
    }
}
