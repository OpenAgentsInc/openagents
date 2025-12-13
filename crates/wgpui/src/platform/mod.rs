//! Platform abstraction layer
//!
//! Provides a unified interface for running the UI framework on different platforms.

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use web::WebPlatform;

// Future: Native platform support
// #[cfg(feature = "native")]
// pub mod native;
// #[cfg(feature = "native")]
// pub use native::NativePlatform;

/// Events from the platform
#[derive(Clone, Debug)]
pub enum Event {
    /// Window/canvas was resized
    Resize { width: f32, height: f32 },
    /// Mouse moved
    MouseMove { x: f32, y: f32 },
    /// Mouse button pressed
    MouseDown { x: f32, y: f32, button: MouseButton },
    /// Mouse button released
    MouseUp { x: f32, y: f32, button: MouseButton },
    /// Mouse wheel scrolled
    Scroll { x: f32, y: f32, delta_x: f32, delta_y: f32 },
    /// Key pressed
    KeyDown { key: String, modifiers: Modifiers },
    /// Key released
    KeyUp { key: String, modifiers: Modifiers },
    /// Text input
    TextInput { text: String },
    /// Frame tick (request animation frame)
    Frame,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Other(u16),
}

impl From<i16> for MouseButton {
    fn from(button: i16) -> Self {
        match button {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            n => MouseButton::Other(n as u16),
        }
    }
}
