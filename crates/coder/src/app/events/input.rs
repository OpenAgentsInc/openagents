use wgpui::input::{Key as UiKey, Modifiers as UiModifiers, NamedKey as UiNamedKey};
use winit::event::MouseButton as WinitMouseButton;
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};

use crate::keybindings::{Action as KeyAction, Keybinding};

pub(crate) fn key_to_string(key: &UiKey) -> String {
    match key {
        UiKey::Named(named) => match named {
            UiNamedKey::Enter => "Enter".to_string(),
            UiNamedKey::Escape => "Escape".to_string(),
            UiNamedKey::Backspace => "Backspace".to_string(),
            UiNamedKey::Delete => "Delete".to_string(),
            UiNamedKey::Tab => "Tab".to_string(),
            UiNamedKey::Space => "Space".to_string(),
            UiNamedKey::Home => "Home".to_string(),
            UiNamedKey::End => "End".to_string(),
            UiNamedKey::PageUp => "PageUp".to_string(),
            UiNamedKey::PageDown => "PageDown".to_string(),
            UiNamedKey::ArrowUp => "ArrowUp".to_string(),
            UiNamedKey::ArrowDown => "ArrowDown".to_string(),
            UiNamedKey::ArrowLeft => "ArrowLeft".to_string(),
            UiNamedKey::ArrowRight => "ArrowRight".to_string(),
            UiNamedKey::Unidentified => "Unidentified".to_string(),
        },
        UiKey::Character(text) => text.to_string(),
    }
}

pub(crate) fn key_from_string(value: &str) -> Option<UiKey> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let named = match trimmed.to_ascii_lowercase().as_str() {
        "enter" => Some(UiNamedKey::Enter),
        "escape" => Some(UiNamedKey::Escape),
        "backspace" => Some(UiNamedKey::Backspace),
        "delete" => Some(UiNamedKey::Delete),
        "tab" => Some(UiNamedKey::Tab),
        "space" => Some(UiNamedKey::Space),
        "home" => Some(UiNamedKey::Home),
        "end" => Some(UiNamedKey::End),
        "pageup" => Some(UiNamedKey::PageUp),
        "pagedown" => Some(UiNamedKey::PageDown),
        "arrowup" => Some(UiNamedKey::ArrowUp),
        "arrowdown" => Some(UiNamedKey::ArrowDown),
        "arrowleft" => Some(UiNamedKey::ArrowLeft),
        "arrowright" => Some(UiNamedKey::ArrowRight),
        _ => None,
    };
    if let Some(named) = named {
        return Some(UiKey::Named(named));
    }
    Some(UiKey::Character(trimmed.to_ascii_lowercase()))
}

pub(crate) fn format_keybinding(binding: &Keybinding) -> String {
    let mut parts = Vec::new();
    if binding.modifiers.ctrl {
        parts.push("Ctrl");
    }
    if binding.modifiers.alt {
        parts.push("Alt");
    }
    if binding.modifiers.shift {
        parts.push("Shift");
    }
    if binding.modifiers.meta {
        parts.push("Meta");
    }
    let key_label = match &binding.key {
        UiKey::Named(named) => match named {
            UiNamedKey::Enter => "Enter".to_string(),
            UiNamedKey::Escape => "Escape".to_string(),
            UiNamedKey::Backspace => "Backspace".to_string(),
            UiNamedKey::Delete => "Delete".to_string(),
            UiNamedKey::Tab => "Tab".to_string(),
            UiNamedKey::Space => "Space".to_string(),
            UiNamedKey::Home => "Home".to_string(),
            UiNamedKey::End => "End".to_string(),
            UiNamedKey::PageUp => "PageUp".to_string(),
            UiNamedKey::PageDown => "PageDown".to_string(),
            UiNamedKey::ArrowUp => "ArrowUp".to_string(),
            UiNamedKey::ArrowDown => "ArrowDown".to_string(),
            UiNamedKey::ArrowLeft => "ArrowLeft".to_string(),
            UiNamedKey::ArrowRight => "ArrowRight".to_string(),
            UiNamedKey::Unidentified => "Key".to_string(),
        },
        UiKey::Character(text) => text.to_uppercase(),
    };
    parts.push(&key_label);
    parts.join("+")
}

pub(crate) fn keybinding_labels(
    bindings: &[Keybinding],
    action: KeyAction,
    fallback: &str,
) -> String {
    let mut labels: Vec<String> = bindings
        .iter()
        .filter(|binding| binding.action == action)
        .map(format_keybinding)
        .collect();
    labels.sort();
    labels.dedup();
    if labels.is_empty() {
        fallback.to_string()
    } else {
        labels.join(" / ")
    }
}

pub(crate) fn convert_mouse_button(button: WinitMouseButton) -> wgpui::MouseButton {
    match button {
        WinitMouseButton::Left => wgpui::MouseButton::Left,
        WinitMouseButton::Right => wgpui::MouseButton::Right,
        WinitMouseButton::Middle => wgpui::MouseButton::Middle,
        _ => wgpui::MouseButton::Left,
    }
}

pub(crate) fn convert_modifiers(mods: &ModifiersState) -> UiModifiers {
    UiModifiers {
        shift: mods.shift_key(),
        ctrl: mods.control_key(),
        alt: mods.alt_key(),
        meta: mods.super_key(),
    }
}

pub(crate) fn convert_key_for_input(key: &WinitKey) -> Option<UiKey> {
    match key {
        WinitKey::Named(named) => Some(UiKey::Named(convert_named_key(*named))),
        WinitKey::Character(c) => Some(UiKey::Character(c.to_string())),
        _ => None,
    }
}

pub(crate) fn convert_key_for_binding(key: &WinitKey) -> Option<UiKey> {
    match key {
        WinitKey::Named(named) => Some(UiKey::Named(convert_named_key(*named))),
        WinitKey::Character(c) => {
            let lowered = c.as_str().to_ascii_lowercase();
            Some(UiKey::Character(lowered))
        }
        _ => None,
    }
}

fn convert_named_key(key: WinitNamedKey) -> UiNamedKey {
    match key {
        WinitNamedKey::Enter => UiNamedKey::Enter,
        WinitNamedKey::Tab => UiNamedKey::Tab,
        WinitNamedKey::Space => UiNamedKey::Space,
        WinitNamedKey::Backspace => UiNamedKey::Backspace,
        WinitNamedKey::Delete => UiNamedKey::Delete,
        WinitNamedKey::Escape => UiNamedKey::Escape,
        WinitNamedKey::ArrowUp => UiNamedKey::ArrowUp,
        WinitNamedKey::ArrowDown => UiNamedKey::ArrowDown,
        WinitNamedKey::ArrowLeft => UiNamedKey::ArrowLeft,
        WinitNamedKey::ArrowRight => UiNamedKey::ArrowRight,
        WinitNamedKey::Home => UiNamedKey::Home,
        WinitNamedKey::End => UiNamedKey::End,
        WinitNamedKey::PageUp => UiNamedKey::PageUp,
        WinitNamedKey::PageDown => UiNamedKey::PageDown,
        _ => UiNamedKey::Tab,
    }
}
