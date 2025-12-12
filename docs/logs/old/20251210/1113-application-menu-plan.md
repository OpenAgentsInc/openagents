# Plan: Add Application Menu to Commander

## Overview

Add a native application menu to the Commander desktop app for system navigation. The OpenAgents codebase already uses GPUI (same framework as Zed), so the menu infrastructure is already in place - we just need to wire it up.

## Current State

- **Commander entry point**: `crates/commander/src/main.rs`
- **GPUI menu API**: Already exists at `crates/gpui/src/platform/app_menu.rs`
- **Action patterns**: Already established in `crates/hud/src/actions.rs`
- **Current navigation**: Sidebar-based (trajectory list, collapsible panels)
- **Missing**: No `cx.set_menus()` call, no centralized actions module for Commander

## Implementation Steps

### Step 1: Create Commander Actions Module

**File**: `crates/commander/src/actions.rs`

Define navigation and app-level actions:

```rust
use gpui::actions;

actions!(
    commander,
    [
        // App
        Quit,
        ShowSettings,
        ShowAbout,

        // File
        NewTrajectory,
        OpenTrajectory,
        SaveTrajectory,
        ExportTrajectory,

        // Edit
        Undo,
        Redo,

        // View
        ToggleSidebar,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        ToggleFullscreen,

        // Navigate
        GoToCommander,
        GoToGym,
        GoToCompute,
        GoToWallet,
        GoToStore,

        // Help
        OpenDocs,
        OpenDiscord,
        ReportIssue,
    ]
);
```

### Step 2: Create App Menus Module

**File**: `crates/commander/src/app_menus.rs`

```rust
use gpui::{Menu, MenuItem, OsAction, SystemMenuType};
use crate::actions::*;

pub fn app_menus() -> Vec<Menu> {
    vec![
        // Commander (app name menu on macOS)
        Menu {
            name: "Commander".into(),
            items: vec![
                MenuItem::action("About Commander", ShowAbout),
                MenuItem::separator(),
                MenuItem::action("Settings...", ShowSettings),
                MenuItem::separator(),
                #[cfg(target_os = "macos")]
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                #[cfg(target_os = "macos")]
                MenuItem::separator(),
                MenuItem::action("Quit Commander", Quit),
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
                MenuItem::action("Store", GoToStore),
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
```

### Step 3: Register Action Handlers in main.rs

Add action handler registrations before `cx.open_window()`:

```rust
mod actions;
mod app_menus;

use actions::*;
use app_menus::app_menus;

// In main(), before cx.open_window():

// Register app-level action handlers
cx.on_action(|_: &Quit, cx| cx.quit());
cx.on_action(|_: &ShowSettings, cx| { /* open settings panel */ });
cx.on_action(|_: &ShowAbout, cx| { /* show about dialog */ });

// Navigation handlers (update app state to switch views)
cx.on_action(|_: &GoToCommander, cx| { /* navigate to Commander home */ });
cx.on_action(|_: &GoToGym, cx| { /* navigate to Gym */ });
cx.on_action(|_: &GoToCompute, cx| { /* navigate to Compute */ });
cx.on_action(|_: &GoToWallet, cx| { /* navigate to Wallet */ });
cx.on_action(|_: &GoToStore, cx| { /* navigate to Store */ });

// Set the menus
cx.set_menus(app_menus());
```

### Step 4: Add Keyboard Bindings

Extend existing `cx.bind_keys()` call:

```rust
cx.bind_keys([
    // Existing bindings...

    // App
    KeyBinding::new("cmd-q", Quit, None),
    KeyBinding::new("cmd-,", ShowSettings, None),

    // View
    KeyBinding::new("cmd-b", ToggleSidebar, None),
    KeyBinding::new("cmd-+", ZoomIn, None),
    KeyBinding::new("cmd--", ZoomOut, None),
    KeyBinding::new("cmd-0", ZoomReset, None),
    KeyBinding::new("cmd-ctrl-f", ToggleFullscreen, None),

    // Navigate
    KeyBinding::new("cmd-1", GoToCommander, None),
    KeyBinding::new("cmd-2", GoToGym, None),
    KeyBinding::new("cmd-3", GoToCompute, None),
    KeyBinding::new("cmd-4", GoToWallet, None),
    KeyBinding::new("cmd-5", GoToStore, None),
]);
```

### Step 5: Update lib.rs Exports

**File**: `crates/commander/src/lib.rs`

```rust
pub mod actions;
pub mod app_menus;
```

## Files to Create/Modify

| File | Action |
|------|--------|
| `crates/commander/src/actions.rs` | Create - define all Commander actions |
| `crates/commander/src/app_menus.rs` | Create - menu structure |
| `crates/commander/src/main.rs` | Modify - register handlers, set menus |
| `crates/commander/src/lib.rs` | Modify - export new modules |

## Platform Considerations

- **macOS**: Native menu bar via `NSMenu` (handled by GPUI automatically)
- **Windows/Linux**: Can render custom UI menu (see Zed's `title_bar/application_menu.rs` if needed)
- Current focus: macOS native menu first (Commander's primary target)

## Testing

1. Run Commander: `cargo run -p commander`
2. Verify menus appear in macOS menu bar
3. Test each menu item triggers correct action
4. Verify keyboard shortcuts work
5. Test menu item enable/disable based on context (future)

## Future Enhancements

- Dynamic menu items (recent trajectories, running agents)
- Context-sensitive menu enable/disable via `on_validate_app_menu_command`
- Cross-platform custom menu rendering for Windows/Linux
- Dock menu with quick actions
