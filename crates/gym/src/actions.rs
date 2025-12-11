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
    // Tab navigation shortcuts
    cx.bind_keys([
        // Cmd+1/2/3/4/5 for tab switching
        KeyBinding::new("cmd-1", SwitchToTrajectories, None),
        KeyBinding::new("cmd-2", SwitchToTBCC, None),
        KeyBinding::new("cmd-3", SwitchToHillClimber, None),
        KeyBinding::new("cmd-4", SwitchToTestGen, None),
        KeyBinding::new("cmd-5", SwitchToRegexCrusade, None),
    ]);
}
