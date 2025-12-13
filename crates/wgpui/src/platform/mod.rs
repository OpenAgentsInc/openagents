//! Platform abstraction layer.

use crate::geometry::{Point, Size};

/// Platform events.
#[derive(Clone, Debug)]
pub enum Event {
    /// Window resize event.
    Resize { size: Size, scale_factor: f32 },
    /// Mouse move event.
    MouseMove { position: Point },
    /// Mouse button pressed.
    MouseDown { position: Point, button: MouseButton },
    /// Mouse button released.
    MouseUp { position: Point, button: MouseButton },
    /// Mouse wheel scroll.
    Wheel { delta: Point },
    /// Key pressed.
    KeyDown { key: Key, modifiers: Modifiers },
    /// Key released.
    KeyUp { key: Key, modifiers: Modifiers },
}

/// Mouse buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Other(u16),
}

/// Keyboard keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Character(char),
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Space,
    Other(String),
}

/// Keyboard modifiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use web::WebPlatform;
