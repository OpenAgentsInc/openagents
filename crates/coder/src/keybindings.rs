use wgpui::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Interrupt,
    OpenCommandPalette,
    OpenSettings,
    OpenWallet,
    OpenDspy,
    ToggleLeftSidebar,
    ToggleRightSidebar,
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
            Action::OpenWallet,
            Action::OpenDspy,
            Action::ToggleLeftSidebar,
            Action::ToggleRightSidebar,
            Action::ToggleSidebars,
        ]
    }

    pub fn id(&self) -> &'static str {
        match self {
            Action::Interrupt => "interrupt",
            Action::OpenCommandPalette => "command_palette",
            Action::OpenSettings => "settings",
            Action::OpenWallet => "wallet",
            Action::OpenDspy => "dspy",
            Action::ToggleLeftSidebar => "sidebar_left",
            Action::ToggleRightSidebar => "sidebar_right",
            Action::ToggleSidebars => "sidebar_toggle",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Action::Interrupt => "Interrupt request",
            Action::OpenCommandPalette => "Command palette",
            Action::OpenSettings => "Open settings",
            Action::OpenWallet => "Open wallet",
            Action::OpenDspy => "Open DSPy",
            Action::ToggleLeftSidebar => "Toggle left sidebar",
            Action::ToggleRightSidebar => "Toggle right sidebar",
            Action::ToggleSidebars => "Toggle sidebars",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        match id {
            "interrupt" => Some(Action::Interrupt),
            "command_palette" => Some(Action::OpenCommandPalette),
            "settings" => Some(Action::OpenSettings),
            "wallet" => Some(Action::OpenWallet),
            "dspy" => Some(Action::OpenDspy),
            "sidebar_left" => Some(Action::ToggleLeftSidebar),
            "sidebar_right" => Some(Action::ToggleRightSidebar),
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
            key: Key::Character("w".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenWallet,
        },
        Keybinding {
            key: Key::Character("d".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenDspy,
        },
        Keybinding {
            key: Key::Character("[".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::ToggleLeftSidebar,
        },
        Keybinding {
            key: Key::Character("]".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::ToggleRightSidebar,
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
            key: Key::Character("w".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenWallet,
        },
        Keybinding {
            key: Key::Character("d".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenDspy,
        },
        Keybinding {
            key: Key::Character("[".to_string()),
            modifiers: Modifiers {
                meta: true,
                ..Default::default()
            },
            action: Action::ToggleLeftSidebar,
        },
        Keybinding {
            key: Key::Character("]".to_string()),
            modifiers: Modifiers {
                meta: true,
                ..Default::default()
            },
            action: Action::ToggleRightSidebar,
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
