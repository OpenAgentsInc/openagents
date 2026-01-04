//! Convert winit input events to wgpui InputEvent

use wgpui::input::{InputEvent, Key, Modifiers, NamedKey};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};

/// Convert winit NamedKey to wgpui NamedKey
pub fn convert_named_key(key: &WinitNamedKey) -> Option<NamedKey> {
    match key {
        WinitNamedKey::Enter => Some(NamedKey::Enter),
        WinitNamedKey::Escape => Some(NamedKey::Escape),
        WinitNamedKey::Backspace => Some(NamedKey::Backspace),
        WinitNamedKey::Delete => Some(NamedKey::Delete),
        WinitNamedKey::Tab => Some(NamedKey::Tab),
        WinitNamedKey::Home => Some(NamedKey::Home),
        WinitNamedKey::End => Some(NamedKey::End),
        WinitNamedKey::PageUp => Some(NamedKey::PageUp),
        WinitNamedKey::PageDown => Some(NamedKey::PageDown),
        WinitNamedKey::ArrowUp => Some(NamedKey::ArrowUp),
        WinitNamedKey::ArrowDown => Some(NamedKey::ArrowDown),
        WinitNamedKey::ArrowLeft => Some(NamedKey::ArrowLeft),
        WinitNamedKey::ArrowRight => Some(NamedKey::ArrowRight),
        _ => None,
    }
}

/// Convert winit Key to wgpui Key
pub fn convert_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Named(named) => convert_named_key(named).map(Key::Named),
        WinitKey::Character(c) => Some(Key::Character(c.to_string())),
        _ => None,
    }
}

/// Convert winit ModifiersState to wgpui Modifiers
pub fn convert_modifiers(mods: &ModifiersState) -> Modifiers {
    Modifiers {
        shift: mods.shift_key(),
        ctrl: mods.control_key(),
        alt: mods.alt_key(),
        meta: mods.super_key(),
    }
}

/// Create wgpui KeyDown event from winit keyboard event
pub fn create_key_down(key: &WinitKey, mods: &ModifiersState) -> Option<InputEvent> {
    convert_key(key).map(|key| InputEvent::KeyDown {
        key,
        modifiers: convert_modifiers(mods),
    })
}
