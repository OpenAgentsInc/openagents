//! Default keybindings.

use super::Keymap;
use crate::action::KeyBinding;
use crate::action::standard::*;

/// Create a keymap with default keybindings.
///
/// These bindings follow common conventions:
/// - Cmd/Ctrl for primary shortcuts
/// - Arrow keys for navigation
/// - Tab for focus navigation
/// - Escape for cancel
/// - Enter for confirm
///
/// # Example
///
/// ```ignore
/// let keymap = default_keymap();
/// // Add user overrides...
/// keymap.add(KeyBinding::new("cmd-q", Quit).unwrap());
/// ```
pub fn default_keymap() -> Keymap {
    let mut keymap = Keymap::new();

    // Navigation
    add_binding(&mut keymap, "up", MoveUp);
    add_binding(&mut keymap, "down", MoveDown);
    add_binding(&mut keymap, "left", MoveLeft);
    add_binding(&mut keymap, "right", MoveRight);
    add_binding(&mut keymap, "home", MoveToStart);
    add_binding(&mut keymap, "end", MoveToEnd);

    // Focus navigation
    add_binding(&mut keymap, "tab", FocusNext);
    add_binding(&mut keymap, "shift-tab", FocusPrevious);

    // Cancel/confirm
    add_binding(&mut keymap, "escape", Cancel);
    add_binding(&mut keymap, "enter", Confirm);

    // Editing
    add_binding(&mut keymap, "backspace", Backspace);
    add_binding(&mut keymap, "delete", Delete);

    // Clipboard (macOS style - cmd)
    add_binding(&mut keymap, "cmd-c", Copy);
    add_binding(&mut keymap, "cmd-x", Cut);
    add_binding(&mut keymap, "cmd-v", Paste);
    add_binding(&mut keymap, "cmd-a", SelectAll);

    // Undo/redo (macOS style)
    add_binding(&mut keymap, "cmd-z", Undo);
    add_binding(&mut keymap, "cmd-shift-z", Redo);

    // File operations
    add_binding(&mut keymap, "cmd-s", Save);
    add_binding(&mut keymap, "cmd-o", Open);
    add_binding(&mut keymap, "cmd-n", New);

    // UI
    add_binding(&mut keymap, "cmd-shift-p", ToggleCommandPalette);
    add_binding(&mut keymap, "cmd-w", Close);
    add_binding(&mut keymap, "cmd-r", Refresh);

    keymap
}

/// Helper to add a binding, ignoring parse errors (which shouldn't happen for hardcoded keys).
fn add_binding<A: crate::action::Action>(keymap: &mut Keymap, keystroke: &str, action: A) {
    if let Ok(binding) = KeyBinding::new(keystroke, action) {
        keymap.add(binding);
    }
}

/// Create an empty keymap (for testing or custom setups).
#[allow(dead_code)]
pub fn empty_keymap() -> Keymap {
    Keymap::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::KeyContext;
    use crate::{Key, Modifiers, NamedKey};

    #[test]
    fn test_default_keymap_not_empty() {
        let keymap = default_keymap();
        assert!(!keymap.is_empty());
    }

    #[test]
    fn test_escape_maps_to_cancel() {
        let keymap = default_keymap();
        let context = KeyContext::new();

        let action = keymap.match_keystroke(
            &Key::Named(NamedKey::Escape),
            &Modifiers::default(),
            &context,
        );

        assert!(action.is_some());
        assert_eq!(action.unwrap().name(), "Cancel");
    }

    #[test]
    fn test_cmd_s_maps_to_save() {
        let keymap = default_keymap();
        let context = KeyContext::new();

        let action = keymap.match_keystroke(
            &Key::Character("s".to_string()),
            &Modifiers {
                meta: true,
                ..Default::default()
            },
            &context,
        );

        assert!(action.is_some());
        assert_eq!(action.unwrap().name(), "Save");
    }

    #[test]
    fn test_arrow_keys() {
        let keymap = default_keymap();
        let context = KeyContext::new();
        let mods = Modifiers::default();

        let action = keymap.match_keystroke(&Key::Named(NamedKey::ArrowUp), &mods, &context);
        assert_eq!(action.unwrap().name(), "MoveUp");

        let action = keymap.match_keystroke(&Key::Named(NamedKey::ArrowDown), &mods, &context);
        assert_eq!(action.unwrap().name(), "MoveDown");

        let action = keymap.match_keystroke(&Key::Named(NamedKey::ArrowLeft), &mods, &context);
        assert_eq!(action.unwrap().name(), "MoveLeft");

        let action = keymap.match_keystroke(&Key::Named(NamedKey::ArrowRight), &mods, &context);
        assert_eq!(action.unwrap().name(), "MoveRight");
    }

    #[test]
    fn test_tab_focus() {
        let keymap = default_keymap();
        let context = KeyContext::new();

        let action =
            keymap.match_keystroke(&Key::Named(NamedKey::Tab), &Modifiers::default(), &context);
        assert_eq!(action.unwrap().name(), "FocusNext");

        let action = keymap.match_keystroke(
            &Key::Named(NamedKey::Tab),
            &Modifiers {
                shift: true,
                ..Default::default()
            },
            &context,
        );
        assert_eq!(action.unwrap().name(), "FocusPrevious");
    }

    #[test]
    fn test_empty_keymap() {
        let keymap = empty_keymap();
        assert!(keymap.is_empty());
    }
}
