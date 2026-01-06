//! Onyx application handler

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use arboard::Clipboard;
use web_time::Instant;
use wgpui::components::{Component, EventContext, LiveEditor, PaintContext};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Scene, Size, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

/// Main application state
pub struct OnyxApp {
    state: Option<RenderState>,
}

impl Default for OnyxApp {
    fn default() -> Self {
        Self { state: None }
    }
}

/// Render state holding all GPU and UI resources
pub struct RenderState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    pub text_system: TextSystem,
    pub last_tick: Instant,
    pub modifiers: ModifiersState,
    #[allow(dead_code)] // Used by EventContext closures
    pub clipboard: Rc<RefCell<Option<Clipboard>>>,
    pub event_context: EventContext,

    // Editor
    pub editor: LiveEditor,
}

impl ApplicationHandler for OnyxApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Onyx")
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

            // Initialize clipboard
            let clipboard = Rc::new(RefCell::new(Clipboard::new().ok()));

            // Initialize EventContext with clipboard
            let mut event_context = EventContext::new();
            let read_clip = clipboard.clone();
            let write_clip = clipboard.clone();
            event_context.set_clipboard(
                move || read_clip.borrow_mut().as_mut()?.get_text().ok(),
                move |text| {
                    if let Some(clip) = write_clip.borrow_mut().as_mut() {
                        let _ = clip.set_text(text);
                    }
                },
            );

            // Create editor with sample content
            let sample_content = r#"# Welcome to Onyx

This is a **Markdown** editor built with WGPUI.

## Features

- Live inline formatting (coming soon!)
- Fast GPU-accelerated rendering
- Local-first design

## Getting Started

Start typing to edit this document. Use **Ctrl+S** to save.

### Keyboard Shortcuts

- **Arrow keys**: Navigate
- **Shift+arrows**: Select text
- **Ctrl+C/X/V**: Copy/Cut/Paste
- **Ctrl+A**: Select all
- **Tab**: Insert 4 spaces

```rust
fn main() {
    println!("Hello from Onyx!");
}
```

Happy writing!
"#;

            let mut editor = LiveEditor::new(sample_content)
                .with_id(1)
                .on_save(|| {
                    println!("Save requested!");
                });
            editor.focus();

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                last_tick: Instant::now(),
                modifiers: ModifiersState::empty(),
                clipboard,
                event_context,
                editor,
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

            WindowEvent::ModifiersChanged(mods) => {
                state.modifiers = mods.state();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    // Convert winit key to wgpui InputEvent
                    if let Some(wgpui_event) = convert_key_event(&event.logical_key, &state.modifiers) {
                        let scale_factor = state.window.scale_factor() as f32;
                        let logical_width = state.config.width as f32 / scale_factor;
                        let logical_height = state.config.height as f32 / scale_factor;
                        let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);

                        state.editor.event(&wgpui_event, bounds, &mut state.event_context);
                    }
                }
            }

            WindowEvent::MouseInput { state: button_state, button: _, .. } => {
                if button_state == ElementState::Pressed {
                    // For simplicity, we'll handle mouse events in a future iteration
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                let scale_factor = state.window.scale_factor();
                let _logical_x = position.x as f32 / scale_factor as f32;
                let _logical_y = position.y as f32 / scale_factor as f32;
                // Mouse move handling will be added later
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };

                let scale_factor = state.window.scale_factor() as f32;
                let logical_width = state.config.width as f32 / scale_factor;
                let logical_height = state.config.height as f32 / scale_factor;
                let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);

                let scroll_event = wgpui::InputEvent::Scroll { dx: 0.0, dy };
                state.editor.event(&scroll_event, bounds, &mut state.event_context);
            }

            WindowEvent::RedrawRequested => {
                let physical_width = state.config.width as f32;
                let physical_height = state.config.height as f32;
                let scale_factor = state.window.scale_factor() as f32;
                let logical_width = physical_width / scale_factor;
                let logical_height = physical_height / scale_factor;

                state.last_tick = Instant::now();

                // Build scene
                let mut scene = Scene::new();
                let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);
                let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
                state.editor.paint(bounds, &mut paint_cx);

                // Render
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

                state.renderer.resize(
                    &state.queue,
                    Size::new(physical_width, physical_height),
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

/// Convert winit key event to wgpui InputEvent
fn convert_key_event(key: &Key, modifiers: &ModifiersState) -> Option<wgpui::InputEvent> {
    let wgpui_modifiers = wgpui::input::Modifiers {
        shift: modifiers.shift_key(),
        ctrl: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    };

    let wgpui_key = match key {
        Key::Character(c) => wgpui::input::Key::Character(c.to_string()),
        Key::Named(named) => {
            let wgpui_named = match named {
                NamedKey::Enter => wgpui::input::NamedKey::Enter,
                NamedKey::Tab => wgpui::input::NamedKey::Tab,
                NamedKey::Space => wgpui::input::NamedKey::Space,
                NamedKey::Backspace => wgpui::input::NamedKey::Backspace,
                NamedKey::Delete => wgpui::input::NamedKey::Delete,
                NamedKey::Escape => wgpui::input::NamedKey::Escape,
                NamedKey::ArrowUp => wgpui::input::NamedKey::ArrowUp,
                NamedKey::ArrowDown => wgpui::input::NamedKey::ArrowDown,
                NamedKey::ArrowLeft => wgpui::input::NamedKey::ArrowLeft,
                NamedKey::ArrowRight => wgpui::input::NamedKey::ArrowRight,
                NamedKey::Home => wgpui::input::NamedKey::Home,
                NamedKey::End => wgpui::input::NamedKey::End,
                NamedKey::PageUp => wgpui::input::NamedKey::PageUp,
                NamedKey::PageDown => wgpui::input::NamedKey::PageDown,
                _ => return None,
            };
            wgpui::input::Key::Named(wgpui_named)
        }
        _ => return None,
    };

    Some(wgpui::InputEvent::KeyDown {
        key: wgpui_key,
        modifiers: wgpui_modifiers,
    })
}
