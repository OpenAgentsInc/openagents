use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct StoredModifiers {
    pub(crate) shift: bool,
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) meta: bool,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct StoredKeybinding {
    pub(crate) action: String,
    pub(crate) key: String,
    pub(crate) modifiers: StoredModifiers,
}
