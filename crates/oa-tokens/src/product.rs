//! OpenAgents roles in Apps SDK UI's naming scheme, for surfaces Apps SDK UI
//! does not cover.

use crate::Section;

/// Appended to the component token layer.
pub const COMPONENTS: &[Section] = &[Section {
    // Terminal panels stay Coder Noir in both themes (adoption plan, phase 4).
    // The same values as `crate::noir`, Coder Noir.
    title: "Terminal (always Coder Noir)",
    tokens: &[
        ("--terminal-background-color", "#0e0e0e"),
        ("--terminal-text-color", "#ededed"),
        ("--terminal-cursor-color", "#ededed"),
        ("--terminal-selection-background-color", "#333333"),
        ("--terminal-selection-text-color", "#ffffff"),
        ("--terminal-ansi-0", "#1a1a1a"),
        ("--terminal-ansi-1", "#ff4d42"),
        ("--terminal-ansi-2", "#9fd08a"),
        ("--terminal-ansi-3", "#e6c15c"),
        ("--terminal-ansi-4", "#7fb2e8"),
        ("--terminal-ansi-5", "#d093d0"),
        ("--terminal-ansi-6", "#74cfd1"),
        ("--terminal-ansi-7", "#c9c9c9"),
        ("--terminal-ansi-8", "#666666"),
        ("--terminal-ansi-9", "#ff6e64"),
        ("--terminal-ansi-10", "#b7e2a3"),
        ("--terminal-ansi-11", "#f2d47c"),
        ("--terminal-ansi-12", "#9dc7f2"),
        ("--terminal-ansi-13", "#e0aede"),
        ("--terminal-ansi-14", "#93e1e2"),
        ("--terminal-ansi-15", "#ffffff"),
    ],
}];
