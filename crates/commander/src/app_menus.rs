//! Application menu definitions for Commander
//!
//! Defines the native menu bar structure for macOS and other platforms.

use gpui::{Menu, MenuItem, OsAction, SystemMenuType};

use crate::actions::*;
use ui::text_input;

/// Build the application menus
pub fn app_menus() -> Vec<Menu> {
    vec![
        // OpenAgents (app name menu on macOS)
        Menu {
            name: "OpenAgents".into(),
            items: vec![
                MenuItem::action("About OpenAgents", ShowAbout),
                MenuItem::separator(),
                MenuItem::action("Settings...", ShowSettings),
                MenuItem::separator(),
                #[cfg(target_os = "macos")]
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                #[cfg(target_os = "macos")]
                MenuItem::separator(),
                MenuItem::action("Quit OpenAgents", Quit),
            ],
        },
        // File menu
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New Trajectory", NewTrajectory),
                MenuItem::action("Open Trajectory...", OpenTrajectory),
                MenuItem::separator(),
                MenuItem::action("Save", SaveTrajectory),
                MenuItem::action("Export...", ExportTrajectory),
            ],
        },
        // Edit menu
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::os_action("Undo", Undo, OsAction::Undo),
                MenuItem::os_action("Redo", Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", text_input::Cut, OsAction::Cut),
                MenuItem::os_action("Copy", text_input::Copy, OsAction::Copy),
                MenuItem::os_action("Paste", text_input::Paste, OsAction::Paste),
                MenuItem::os_action("Select All", text_input::SelectAll, OsAction::SelectAll),
            ],
        },
        // View menu
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Toggle Sidebar", ToggleSidebar),
                MenuItem::separator(),
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::action("Reset Zoom", ZoomReset),
                MenuItem::separator(),
                MenuItem::action("Toggle Fullscreen", ToggleFullscreen),
            ],
        },
        // Navigate menu (Commander-specific)
        Menu {
            name: "Navigate".into(),
            items: vec![
                MenuItem::action("Commander", GoToCommander),
                MenuItem::action("Gym", GoToGym),
                MenuItem::action("Compute", GoToCompute),
                MenuItem::action("Wallet", GoToWallet),
                MenuItem::action("Marketplace", GoToMarketplace),
            ],
        },
        // Help menu
        Menu {
            name: "Help".into(),
            items: vec![
                MenuItem::action("Documentation", OpenDocs),
                MenuItem::action("Discord Community", OpenDiscord),
                MenuItem::separator(),
                MenuItem::action("Report Issue...", ReportIssue),
            ],
        },
    ]
}
