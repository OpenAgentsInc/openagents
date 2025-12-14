//! Desktop platform implementation using winit and wgpu.
//!
//! This module provides native window management and rendering for
//! macOS, Windows, and Linux.

use crate::geometry::Size;
use crate::input::{
    Cursor, InputEvent, Key, KeyCode, Modifiers, MouseButton, NamedKey,
};
use crate::platform::Platform;
use crate::renderer::Renderer;
use crate::scene::Scene;
use crate::text::TextSystem;

use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton as WinitMouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as WinitKey, NamedKey as WinitNamedKey};
use winit::window::{CursorIcon, Window, WindowAttributes};

/// Desktop platform for running wgpui natively.
pub struct DesktopPlatform {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    scale_factor: f32,
    logical_size: Size,
}

impl DesktopPlatform {
    /// Initialize the platform with the given window.
    pub fn new(window: Arc<Window>) -> Result<Self, String> {
        let scale_factor = window.scale_factor() as f32;
        let physical_size = window.inner_size();
        let logical_size = Size::new(
            physical_size.width as f32 / scale_factor,
            physical_size.height as f32 / scale_factor,
        );

        // Initialize wgpu
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("Failed to create surface: {:?}", e))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or("No adapter found")?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor::default(),
            None,
        ))
        .map_err(|e| format!("Failed to create device: {:?}", e))?;

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: physical_size.width,
            height: physical_size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create renderer
        let renderer = Renderer::new(&device, surface_format);
        renderer.resize(&queue, logical_size, scale_factor);

        // Create text system
        let text_system = TextSystem::new(scale_factor);

        Ok(Self {
            window,
            device,
            queue,
            surface,
            surface_config,
            renderer,
            text_system,
            scale_factor,
            logical_size,
        })
    }

    /// Get the window.
    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Get the device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Get the queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Handle a window event and return an optional InputEvent.
    pub fn handle_window_event(&self, event: &WindowEvent) -> Option<InputEvent> {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.scale_factor;
                Some(InputEvent::MouseMove {
                    position: crate::geometry::Point::new(
                        position.x as f32 / scale,
                        position.y as f32 / scale,
                    ),
                    modifiers: Modifiers::NONE, // TODO: track modifier state
                })
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let btn = convert_mouse_button(*button);
                let position = crate::geometry::Point::ZERO; // TODO: track cursor position
                match state {
                    ElementState::Pressed => Some(InputEvent::MouseDown {
                        position,
                        button: btn,
                        modifiers: Modifiers::NONE,
                    }),
                    ElementState::Released => Some(InputEvent::MouseUp {
                        position,
                        button: btn,
                        modifiers: Modifiers::NONE,
                    }),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        crate::geometry::Point::new(*x * 20.0, *y * 20.0)
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        let scale = self.scale_factor;
                        crate::geometry::Point::new(
                            pos.x as f32 / scale,
                            pos.y as f32 / scale,
                        )
                    }
                };
                Some(InputEvent::Wheel {
                    delta,
                    modifiers: Modifiers::NONE,
                })
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let key = convert_key(&event.logical_key);
                let code = convert_key_code(event.physical_key);
                match event.state {
                    ElementState::Pressed => Some(InputEvent::KeyDown {
                        key,
                        code,
                        modifiers: Modifiers::NONE, // TODO: track modifiers
                        repeat: event.repeat,
                    }),
                    ElementState::Released => Some(InputEvent::KeyUp {
                        key,
                        code,
                        modifiers: Modifiers::NONE,
                    }),
                }
            }
            WindowEvent::Focused(focused) => {
                if *focused {
                    Some(InputEvent::FocusIn)
                } else {
                    Some(InputEvent::FocusOut)
                }
            }
            _ => None,
        }
    }
}

impl Platform for DesktopPlatform {
    fn logical_size(&self) -> Size {
        self.logical_size
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn text_system(&mut self) -> &mut TextSystem {
        &mut self.text_system
    }

    fn render(&mut self, scene: &Scene) -> Result<(), String> {
        // Update atlas if needed
        if self.text_system.is_dirty() {
            self.renderer.update_atlas(
                &self.queue,
                self.text_system.atlas_data(),
                self.text_system.atlas_size(),
            );
            self.text_system.mark_clean();
        }

        // Prepare scene
        self.renderer.prepare(&self.device, scene);

        // Get frame
        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| format!("Failed to get surface texture: {:?}", e))?;

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Render
        self.renderer.render(&mut encoder, &view);

        // Submit
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }

    fn request_redraw(&self) {
        self.window.request_redraw();
    }

    fn set_cursor(&self, cursor: Cursor) {
        let icon = match cursor {
            Cursor::Default => CursorIcon::Default,
            Cursor::Pointer => CursorIcon::Pointer,
            Cursor::Text => CursorIcon::Text,
            Cursor::Crosshair => CursorIcon::Crosshair,
            Cursor::Move => CursorIcon::Move,
            Cursor::NotAllowed => CursorIcon::NotAllowed,
            Cursor::Grab => CursorIcon::Grab,
            Cursor::Grabbing => CursorIcon::Grabbing,
            Cursor::EwResize => CursorIcon::EwResize,
            Cursor::NsResize => CursorIcon::NsResize,
            Cursor::NeswResize => CursorIcon::NeswResize,
            Cursor::NwseResize => CursorIcon::NwseResize,
            Cursor::ColResize => CursorIcon::ColResize,
            Cursor::RowResize => CursorIcon::RowResize,
            Cursor::Wait => CursorIcon::Wait,
            Cursor::Progress => CursorIcon::Progress,
            Cursor::Help => CursorIcon::Help,
            Cursor::ZoomIn => CursorIcon::ZoomIn,
            Cursor::ZoomOut => CursorIcon::ZoomOut,
            Cursor::None => {
                self.window.set_cursor_visible(false);
                return;
            }
        };
        self.window.set_cursor_visible(true);
        self.window.set_cursor(icon);
    }

    fn handle_resize(&mut self) {
        self.scale_factor = self.window.scale_factor() as f32;
        let physical_size = self.window.inner_size();

        if physical_size.width > 0 && physical_size.height > 0 {
            self.logical_size = Size::new(
                physical_size.width as f32 / self.scale_factor,
                physical_size.height as f32 / self.scale_factor,
            );

            self.surface_config.width = physical_size.width;
            self.surface_config.height = physical_size.height;
            self.surface.configure(&self.device, &self.surface_config);

            self.renderer
                .resize(&self.queue, self.logical_size, self.scale_factor);

            self.text_system.set_scale_factor(self.scale_factor);
        }
    }
}

// Conversion helpers

fn convert_mouse_button(button: WinitMouseButton) -> MouseButton {
    match button {
        WinitMouseButton::Left => MouseButton::Left,
        WinitMouseButton::Right => MouseButton::Right,
        WinitMouseButton::Middle => MouseButton::Middle,
        WinitMouseButton::Back => MouseButton::Back,
        WinitMouseButton::Forward => MouseButton::Forward,
        WinitMouseButton::Other(n) => MouseButton::Other(n),
    }
}

fn convert_key(key: &WinitKey) -> Key {
    match key {
        WinitKey::Character(s) => Key::Character(s.to_string()),
        WinitKey::Named(named) => Key::Named(convert_named_key(*named)),
        _ => Key::Unidentified,
    }
}

fn convert_named_key(key: WinitNamedKey) -> NamedKey {
    match key {
        WinitNamedKey::Shift => NamedKey::Shift,
        WinitNamedKey::Control => NamedKey::Control,
        WinitNamedKey::Alt => NamedKey::Alt,
        WinitNamedKey::Super => NamedKey::Meta,
        WinitNamedKey::Enter => NamedKey::Enter,
        WinitNamedKey::Tab => NamedKey::Tab,
        WinitNamedKey::Space => NamedKey::Space,
        WinitNamedKey::ArrowUp => NamedKey::ArrowUp,
        WinitNamedKey::ArrowDown => NamedKey::ArrowDown,
        WinitNamedKey::ArrowLeft => NamedKey::ArrowLeft,
        WinitNamedKey::ArrowRight => NamedKey::ArrowRight,
        WinitNamedKey::Home => NamedKey::Home,
        WinitNamedKey::End => NamedKey::End,
        WinitNamedKey::PageUp => NamedKey::PageUp,
        WinitNamedKey::PageDown => NamedKey::PageDown,
        WinitNamedKey::Backspace => NamedKey::Backspace,
        WinitNamedKey::Delete => NamedKey::Delete,
        WinitNamedKey::Insert => NamedKey::Insert,
        WinitNamedKey::Escape => NamedKey::Escape,
        WinitNamedKey::F1 => NamedKey::F1,
        WinitNamedKey::F2 => NamedKey::F2,
        WinitNamedKey::F3 => NamedKey::F3,
        WinitNamedKey::F4 => NamedKey::F4,
        WinitNamedKey::F5 => NamedKey::F5,
        WinitNamedKey::F6 => NamedKey::F6,
        WinitNamedKey::F7 => NamedKey::F7,
        WinitNamedKey::F8 => NamedKey::F8,
        WinitNamedKey::F9 => NamedKey::F9,
        WinitNamedKey::F10 => NamedKey::F10,
        WinitNamedKey::F11 => NamedKey::F11,
        WinitNamedKey::F12 => NamedKey::F12,
        WinitNamedKey::CapsLock => NamedKey::CapsLock,
        WinitNamedKey::NumLock => NamedKey::NumLock,
        WinitNamedKey::ScrollLock => NamedKey::ScrollLock,
        WinitNamedKey::PrintScreen => NamedKey::PrintScreen,
        WinitNamedKey::Pause => NamedKey::Pause,
        WinitNamedKey::ContextMenu => NamedKey::ContextMenu,
        _ => NamedKey::Escape, // Fallback
    }
}

fn convert_key_code(code: winit::keyboard::PhysicalKey) -> KeyCode {
    use winit::keyboard::KeyCode as WK;
    match code {
        winit::keyboard::PhysicalKey::Code(c) => match c {
            WK::KeyA => KeyCode::KeyA,
            WK::KeyB => KeyCode::KeyB,
            WK::KeyC => KeyCode::KeyC,
            WK::KeyD => KeyCode::KeyD,
            WK::KeyE => KeyCode::KeyE,
            WK::KeyF => KeyCode::KeyF,
            WK::KeyG => KeyCode::KeyG,
            WK::KeyH => KeyCode::KeyH,
            WK::KeyI => KeyCode::KeyI,
            WK::KeyJ => KeyCode::KeyJ,
            WK::KeyK => KeyCode::KeyK,
            WK::KeyL => KeyCode::KeyL,
            WK::KeyM => KeyCode::KeyM,
            WK::KeyN => KeyCode::KeyN,
            WK::KeyO => KeyCode::KeyO,
            WK::KeyP => KeyCode::KeyP,
            WK::KeyQ => KeyCode::KeyQ,
            WK::KeyR => KeyCode::KeyR,
            WK::KeyS => KeyCode::KeyS,
            WK::KeyT => KeyCode::KeyT,
            WK::KeyU => KeyCode::KeyU,
            WK::KeyV => KeyCode::KeyV,
            WK::KeyW => KeyCode::KeyW,
            WK::KeyX => KeyCode::KeyX,
            WK::KeyY => KeyCode::KeyY,
            WK::KeyZ => KeyCode::KeyZ,
            WK::Digit0 => KeyCode::Digit0,
            WK::Digit1 => KeyCode::Digit1,
            WK::Digit2 => KeyCode::Digit2,
            WK::Digit3 => KeyCode::Digit3,
            WK::Digit4 => KeyCode::Digit4,
            WK::Digit5 => KeyCode::Digit5,
            WK::Digit6 => KeyCode::Digit6,
            WK::Digit7 => KeyCode::Digit7,
            WK::Digit8 => KeyCode::Digit8,
            WK::Digit9 => KeyCode::Digit9,
            WK::F1 => KeyCode::F1,
            WK::F2 => KeyCode::F2,
            WK::F3 => KeyCode::F3,
            WK::F4 => KeyCode::F4,
            WK::F5 => KeyCode::F5,
            WK::F6 => KeyCode::F6,
            WK::F7 => KeyCode::F7,
            WK::F8 => KeyCode::F8,
            WK::F9 => KeyCode::F9,
            WK::F10 => KeyCode::F10,
            WK::F11 => KeyCode::F11,
            WK::F12 => KeyCode::F12,
            WK::ShiftLeft => KeyCode::ShiftLeft,
            WK::ShiftRight => KeyCode::ShiftRight,
            WK::ControlLeft => KeyCode::ControlLeft,
            WK::ControlRight => KeyCode::ControlRight,
            WK::AltLeft => KeyCode::AltLeft,
            WK::AltRight => KeyCode::AltRight,
            WK::SuperLeft => KeyCode::MetaLeft,
            WK::SuperRight => KeyCode::MetaRight,
            WK::ArrowUp => KeyCode::ArrowUp,
            WK::ArrowDown => KeyCode::ArrowDown,
            WK::ArrowLeft => KeyCode::ArrowLeft,
            WK::ArrowRight => KeyCode::ArrowRight,
            WK::Home => KeyCode::Home,
            WK::End => KeyCode::End,
            WK::PageUp => KeyCode::PageUp,
            WK::PageDown => KeyCode::PageDown,
            WK::Backspace => KeyCode::Backspace,
            WK::Delete => KeyCode::Delete,
            WK::Insert => KeyCode::Insert,
            WK::Enter => KeyCode::Enter,
            WK::Tab => KeyCode::Tab,
            WK::Escape => KeyCode::Escape,
            WK::Space => KeyCode::Space,
            WK::Minus => KeyCode::Minus,
            WK::Equal => KeyCode::Equal,
            WK::BracketLeft => KeyCode::BracketLeft,
            WK::BracketRight => KeyCode::BracketRight,
            WK::Backslash => KeyCode::Backslash,
            WK::Semicolon => KeyCode::Semicolon,
            WK::Quote => KeyCode::Quote,
            WK::Backquote => KeyCode::Backquote,
            WK::Comma => KeyCode::Comma,
            WK::Period => KeyCode::Period,
            WK::Slash => KeyCode::Slash,
            WK::Numpad0 => KeyCode::Numpad0,
            WK::Numpad1 => KeyCode::Numpad1,
            WK::Numpad2 => KeyCode::Numpad2,
            WK::Numpad3 => KeyCode::Numpad3,
            WK::Numpad4 => KeyCode::Numpad4,
            WK::Numpad5 => KeyCode::Numpad5,
            WK::Numpad6 => KeyCode::Numpad6,
            WK::Numpad7 => KeyCode::Numpad7,
            WK::Numpad8 => KeyCode::Numpad8,
            WK::Numpad9 => KeyCode::Numpad9,
            WK::NumpadAdd => KeyCode::NumpadAdd,
            WK::NumpadSubtract => KeyCode::NumpadSubtract,
            WK::NumpadMultiply => KeyCode::NumpadMultiply,
            WK::NumpadDivide => KeyCode::NumpadDivide,
            WK::NumpadDecimal => KeyCode::NumpadDecimal,
            WK::NumpadEnter => KeyCode::NumpadEnter,
            WK::CapsLock => KeyCode::CapsLock,
            WK::NumLock => KeyCode::NumLock,
            WK::ScrollLock => KeyCode::ScrollLock,
            WK::PrintScreen => KeyCode::PrintScreen,
            WK::Pause => KeyCode::Pause,
            WK::ContextMenu => KeyCode::ContextMenu,
            _ => KeyCode::Unknown,
        },
        winit::keyboard::PhysicalKey::Unidentified(_) => KeyCode::Unknown,
    }
}

/// Helper to create a window with wgpui defaults.
pub fn create_window(
    event_loop: &ActiveEventLoop,
    title: &str,
    width: u32,
    height: u32,
) -> Result<Window, String> {
    let attrs = WindowAttributes::default()
        .with_title(title)
        .with_inner_size(LogicalSize::new(width, height));

    event_loop
        .create_window(attrs)
        .map_err(|e| format!("Failed to create window: {:?}", e))
}
