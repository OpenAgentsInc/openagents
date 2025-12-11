//! Gym Actions
//!
//! Keyboard shortcuts and actions for the Gym workbench.

use gpui::*;

actions!(
    gym,
    [
        // Tab navigation
        SwitchToTrajectories,
        SwitchToTBCC,
        SwitchToHillClimber,
        SwitchToTestGen,
        SwitchToRegexCrusade,

        // General
        FocusGym,
    ]
);

/// Register Gym actions and keybindings
pub fn register_actions(cx: &mut App) {
    // Tab navigation shortcuts (within Gym only)
    // Note: Cmd+1-6 conflict with global nav, so Crusade has no shortcut - click tab
    cx.bind_keys([
        KeyBinding::new("cmd-1", SwitchToTrajectories, None),
        KeyBinding::new("cmd-2", SwitchToTBCC, None),
        KeyBinding::new("cmd-3", SwitchToHillClimber, None),
        KeyBinding::new("cmd-4", SwitchToTestGen, None),
        // SwitchToRegexCrusade - no shortcut, conflicts with global cmd-5
    ]);
}
