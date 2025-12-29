//! Shell-specific actions for dock toggling and navigation

use wgpui::action::{Action, AnyAction};

/// Toggle left sidebar visibility (cmd-b)
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleLeftSidebar;

impl Action for ToggleLeftSidebar {
    fn name() -> &'static str {
        "shell::ToggleLeftSidebar"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Toggle right sidebar visibility (cmd-shift-b)
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleRightSidebar;

impl Action for ToggleRightSidebar {
    fn name() -> &'static str {
        "shell::ToggleRightSidebar"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Toggle bottom panel visibility (cmd-j)
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleBottomPanel;

impl Action for ToggleBottomPanel {
    fn name() -> &'static str {
        "shell::ToggleBottomPanel"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Toggle all sidebars at once (cmd-\)
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleAllSidebars;

impl Action for ToggleAllSidebars {
    fn name() -> &'static str {
        "shell::ToggleAllSidebars"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Focus the center panel
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusCenter;

impl Action for FocusCenter {
    fn name() -> &'static str {
        "shell::FocusCenter"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Toggle Full Auto mode (cmd-f)
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleFullAuto;

impl Action for ToggleFullAuto {
    fn name() -> &'static str {
        "shell::ToggleFullAuto"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}
