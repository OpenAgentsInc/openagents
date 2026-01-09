use wgpui::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Interrupt,
    OpenCommandPalette,
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
    ]
}

pub fn match_action(key: &Key, modifiers: Modifiers, bindings: &[Keybinding]) -> Option<Action> {
    bindings
        .iter()
        .find(|binding| binding.matches(key, modifiers))
        .map(|binding| binding.action)
}
