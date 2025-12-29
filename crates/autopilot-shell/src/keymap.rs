//! Shell-specific keybindings

use wgpui::action::KeyBinding;
use wgpui::keymap::Keymap;

use crate::actions::{
    FocusCenter, ToggleAllSidebars, ToggleBottomPanel, ToggleFullAuto, ToggleLeftSidebar,
    ToggleRightSidebar,
};

/// Create a keymap with shell-specific keybindings.
///
/// Keybindings:
/// - cmd-b: Toggle left sidebar
/// - cmd-shift-b: Toggle right sidebar
/// - cmd-j: Toggle bottom panel
/// - cmd-\: Toggle all sidebars
pub fn shell_keymap() -> Keymap {
    let mut keymap = Keymap::new();

    // Full Auto toggle
    add_binding(&mut keymap, "cmd-f", ToggleFullAuto);

    // Sidebar toggles
    add_binding(&mut keymap, "cmd-b", ToggleLeftSidebar);
    add_binding(&mut keymap, "cmd-shift-b", ToggleRightSidebar);
    add_binding(&mut keymap, "cmd-j", ToggleBottomPanel);
    add_binding(&mut keymap, "cmd-\\", ToggleAllSidebars);

    // Focus
    add_binding(&mut keymap, "cmd-1", FocusCenter);

    keymap
}

/// Helper to add a binding, ignoring parse errors.
fn add_binding<A: wgpui::action::Action>(keymap: &mut Keymap, keystroke: &str, action: A) {
    if let Ok(binding) = KeyBinding::new(keystroke, action) {
        keymap.add(binding);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpui::keymap::KeyContext;
    use wgpui::{Key, Modifiers};

    #[test]
    fn test_shell_keymap_not_empty() {
        let keymap = shell_keymap();
        assert!(!keymap.is_empty());
    }

    #[test]
    fn test_cmd_b_toggles_left_sidebar() {
        let keymap = shell_keymap();
        let context = KeyContext::new();

        let action = keymap.match_keystroke(
            &Key::Character("b".to_string()),
            &Modifiers {
                meta: true,
                ..Default::default()
            },
            &context,
        );

        assert!(action.is_some());
        assert_eq!(action.unwrap().name(), "shell::ToggleLeftSidebar");
    }

    #[test]
    fn test_cmd_shift_b_toggles_right_sidebar() {
        let keymap = shell_keymap();
        let context = KeyContext::new();

        let action = keymap.match_keystroke(
            &Key::Character("b".to_string()),
            &Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            &context,
        );

        assert!(action.is_some());
        assert_eq!(action.unwrap().name(), "shell::ToggleRightSidebar");
    }

    #[test]
    fn test_cmd_j_toggles_bottom_panel() {
        let keymap = shell_keymap();
        let context = KeyContext::new();

        let action = keymap.match_keystroke(
            &Key::Character("j".to_string()),
            &Modifiers {
                meta: true,
                ..Default::default()
            },
            &context,
        );

        assert!(action.is_some());
        assert_eq!(action.unwrap().name(), "shell::ToggleBottomPanel");
    }
}
