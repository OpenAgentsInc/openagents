//! Application menu definitions for MechaCoder.

use gpui_oa::{Menu, MenuItem};

use crate::actions::*;

/// Create the application menu structure.
pub fn app_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "MechaCoder".into(),
            items: vec![
                MenuItem::action("About MechaCoder", ShowAbout),
                MenuItem::separator(),
                MenuItem::action("Settings...", ShowSettings),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ],
        },
        Menu {
            name: "Thread".into(),
            items: vec![
                MenuItem::action("Send Message", SendMessage),
                MenuItem::action("Cancel Generation", CancelGeneration),
                MenuItem::separator(),
                MenuItem::action("Clear Thread", ClearThread),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Accept All Diffs", AcceptAllDiffs),
                MenuItem::action("Reject All Diffs", RejectAllDiffs),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Toggle Thread History", ToggleThreadHistory),
                MenuItem::action("Toggle Terminal Panel", ToggleTerminalPanel),
            ],
        },
        Menu {
            name: "Help".into(),
            items: vec![
                MenuItem::action("About MechaCoder", ShowAbout),
            ],
        },
    ]
}
