use wgpui::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Interrupt,
    OpenCommandPalette,
    OpenSettings,
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
        ]
    }

    pub fn id(&self) -> &'static str {
        match self {
            Action::Interrupt => "interrupt",
            Action::OpenCommandPalette => "command_palette",
            Action::OpenSettings => "settings",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Action::Interrupt => "Interrupt request",
            Action::OpenCommandPalette => "Command palette",
            Action::OpenSettings => "Open settings",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        match id {
            "interrupt" => Some(Action::Interrupt),
            "command_palette" => Some(Action::OpenCommandPalette),
            "settings" => Some(Action::OpenSettings),
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
    ]
}

pub fn match_action(key: &Key, modifiers: Modifiers, bindings: &[Keybinding]) -> Option<Action> {
    bindings
        .iter()
        .find(|binding| binding.matches(key, modifiers))
        .map(|binding| binding.action)
}
