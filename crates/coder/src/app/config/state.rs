use crate::app::ModelOption;
use crate::keybindings::Keybinding;

use super::CoderSettings;

pub(crate) struct SettingsState {
    pub(crate) coder_settings: CoderSettings,
    pub(crate) keybindings: Vec<Keybinding>,
    pub(crate) command_history: Vec<String>,
    pub(crate) selected_model: ModelOption,
}

impl SettingsState {
    pub(crate) fn new(
        settings: CoderSettings,
        keybindings: Vec<Keybinding>,
        selected_model: ModelOption,
    ) -> Self {
        Self {
            coder_settings: settings,
            keybindings,
            command_history: Vec::new(),
            selected_model,
        }
    }
}
