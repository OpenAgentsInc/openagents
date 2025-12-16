//! Input event types for wgpui.
//!
//! This module defines platform-agnostic input events that are used
//! across web and desktop platforms.

use crate::geometry::Point;

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

/// Keyboard modifier state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    };

    pub fn new(shift: bool, ctrl: bool, alt: bool, meta: bool) -> Self {
        Self {
            shift,
            ctrl,
            alt,
            meta,
        }
    }

    /// Returns true if any modifier is pressed.
    pub fn any(&self) -> bool {
        self.shift || self.ctrl || self.alt || self.meta
    }

    /// Returns true if the command modifier is pressed (Ctrl on Windows/Linux, Meta on macOS).
    #[cfg(target_os = "macos")]
    pub fn command(&self) -> bool {
        self.meta
    }

    /// Returns true if the command modifier is pressed (Ctrl on Windows/Linux, Meta on macOS).
    #[cfg(not(target_os = "macos"))]
    pub fn command(&self) -> bool {
        self.ctrl
    }
}

/// Physical key code representing the physical position on the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // Letters
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,

    // Numbers
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Modifiers
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
    MetaLeft,
    MetaRight,

    // Navigation
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,

    // Editing
    Backspace,
    Delete,
    Insert,
    Enter,
    Tab,
    Escape,
    Space,

    // Punctuation
    Minus,
    Equal,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Backquote,
    Comma,
    Period,
    Slash,

    // Numpad
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadSubtract,
    NumpadMultiply,
    NumpadDivide,
    NumpadDecimal,
    NumpadEnter,

    // Other
    CapsLock,
    NumLock,
    ScrollLock,
    PrintScreen,
    Pause,
    ContextMenu,

    Unknown,
}

/// Logical key representing the character or action produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// A character key.
    Character(String),
    /// A named key (Enter, Escape, etc.).
    Named(NamedKey),
    /// An unidentified key.
    Unidentified,
}

/// Named keys that don't produce characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamedKey {
    // Modifiers
    Shift,
    Control,
    Alt,
    Meta,

    // Whitespace
    Enter,
    Tab,
    Space,

    // Navigation
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,

    // Editing
    Backspace,
    Delete,
    Insert,
    Escape,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Lock keys
    CapsLock,
    NumLock,
    ScrollLock,

    // Other
    PrintScreen,
    Pause,
    ContextMenu,
}

/// Input events from the platform.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Mouse button pressed.
    MouseDown {
        position: Point,
        button: MouseButton,
        modifiers: Modifiers,
    },
    /// Mouse button released.
    MouseUp {
        position: Point,
        button: MouseButton,
        modifiers: Modifiers,
    },
    /// Mouse moved.
    MouseMove {
        position: Point,
        modifiers: Modifiers,
    },
    /// Mouse wheel/scroll.
    Wheel {
        /// Scroll delta in logical pixels.
        delta: Point,
        modifiers: Modifiers,
    },
    /// Key pressed.
    KeyDown {
        key: Key,
        code: KeyCode,
        modifiers: Modifiers,
        /// True if this is a key repeat event.
        repeat: bool,
    },
    /// Key released.
    KeyUp {
        key: Key,
        code: KeyCode,
        modifiers: Modifiers,
    },
    /// Text input from keyboard (handles IME composition).
    TextInput { text: String },
    /// IME composition started.
    ImeStart,
    /// IME composition updated.
    ImeUpdate { text: String, cursor: Option<usize> },
    /// IME composition ended.
    ImeEnd,
    /// Focus gained.
    FocusIn,
    /// Focus lost.
    FocusOut,
}

/// Cursor style for the platform to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Default,
    Pointer,
    Text,
    Crosshair,
    Move,
    NotAllowed,
    Grab,
    Grabbing,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
    Wait,
    Progress,
    Help,
    ZoomIn,
    ZoomOut,
    None,
}

impl Cursor {
    /// Get the CSS cursor value for web platform.
    pub fn as_css(&self) -> &'static str {
        match self {
            Cursor::Default => "default",
            Cursor::Pointer => "pointer",
            Cursor::Text => "text",
            Cursor::Crosshair => "crosshair",
            Cursor::Move => "move",
            Cursor::NotAllowed => "not-allowed",
            Cursor::Grab => "grab",
            Cursor::Grabbing => "grabbing",
            Cursor::EwResize => "ew-resize",
            Cursor::NsResize => "ns-resize",
            Cursor::NeswResize => "nesw-resize",
            Cursor::NwseResize => "nwse-resize",
            Cursor::ColResize => "col-resize",
            Cursor::RowResize => "row-resize",
            Cursor::Wait => "wait",
            Cursor::Progress => "progress",
            Cursor::Help => "help",
            Cursor::ZoomIn => "zoom-in",
            Cursor::ZoomOut => "zoom-out",
            Cursor::None => "none",
        }
    }
}
