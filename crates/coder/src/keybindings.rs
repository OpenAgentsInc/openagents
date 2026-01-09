use wgpui::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Interrupt,
    OpenCommandPalette,
    OpenSettings,
    OpenLeftSidebar,
    OpenRightSidebar,
    ToggleSidebars,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keybinding {
    pub key: Key,
    pub modifiers: Modifiers,
    pub action: Action,
}

impl Keybinding {
    pub fn matches(&self, key: &Key, modifiers: Modifiers) -> bool {
        self.key == *key && self.modifiers == modifiers
    }
}

impl Action {
    pub fn all() -> &'static [Action] {
        &[
            Action::Interrupt,
            Action::OpenCommandPalette,
            Action::OpenSettings,
            Action::OpenLeftSidebar,
            Action::OpenRightSidebar,
            Action::ToggleSidebars,
        ]
    }

    pub fn id(&self) -> &'static str {
        match self {
            Action::Interrupt => "interrupt",
            Action::OpenCommandPalette => "command_palette",
            Action::OpenSettings => "settings",
            Action::OpenLeftSidebar => "sidebar_left",
            Action::OpenRightSidebar => "sidebar_right",
            Action::ToggleSidebars => "sidebar_toggle",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Action::Interrupt => "Interrupt request",
            Action::OpenCommandPalette => "Command palette",
            Action::OpenSettings => "Open settings",
            Action::OpenLeftSidebar => "Open left sidebar",
            Action::OpenRightSidebar => "Open right sidebar",
            Action::ToggleSidebars => "Toggle sidebars",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        match id {
            "interrupt" => Some(Action::Interrupt),
            "command_palette" => Some(Action::OpenCommandPalette),
            "settings" => Some(Action::OpenSettings),
            "sidebar_left" => Some(Action::OpenLeftSidebar),
            "sidebar_right" => Some(Action::OpenRightSidebar),
            "sidebar_toggle" => Some(Action::ToggleSidebars),
            _ => None,
        }
    }
}

pub fn default_keybindings() -> Vec<Keybinding> {
    vec![
        Keybinding {
            key: Key::Character("c".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::Interrupt,
        },
        Keybinding {
            key: Key::Character("k".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::OpenCommandPalette,
        },
        Keybinding {
            key: Key::Character(",".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::OpenSettings,
        },
        Keybinding {
            key: Key::Character("[".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::OpenLeftSidebar,
        },
        Keybinding {
            key: Key::Character("]".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::OpenRightSidebar,
        },
        Keybinding {
            key: Key::Character("\\".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::ToggleSidebars,
        },
        Keybinding {
            key: Key::Character("[".to_string()),
            modifiers: Modifiers {
                meta: true,
                ..Default::default()
            },
            action: Action::OpenLeftSidebar,
        },
        Keybinding {
            key: Key::Character("]".to_string()),
            modifiers: Modifiers {
                meta: true,
                ..Default::default()
            },
            action: Action::OpenRightSidebar,
        },
        Keybinding {
            key: Key::Character("\\".to_string()),
            modifiers: Modifiers {
                meta: true,
                ..Default::default()
            },
            action: Action::ToggleSidebars,
        },
    ]
}

pub fn match_action(key: &Key, modifiers: Modifiers, bindings: &[Keybinding]) -> Option<Action> {
    bindings
        .iter()
        .find(|binding| binding.matches(key, modifiers))
        .map(|binding| binding.action)
}
