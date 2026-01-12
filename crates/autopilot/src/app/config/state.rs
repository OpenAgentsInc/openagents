use tokio::sync::mpsc;

use crate::app::ModelOption;
use crate::app::codex_app_server as app_server;
use crate::keybindings::Keybinding;

use super::CoderSettings;

pub(crate) struct SettingsState {
    pub(crate) coder_settings: CoderSettings,
    pub(crate) keybindings: Vec<Keybinding>,
    pub(crate) command_history: Vec<String>,
    pub(crate) selected_model: ModelOption,
    pub(crate) app_server_models: Vec<app_server::ModelInfo>,
    pub(crate) app_server_model_error: Option<String>,
    pub(crate) settings_update_tx: Option<mpsc::UnboundedSender<SettingsUpdate>>,
    pub(crate) settings_update_rx: Option<mpsc::UnboundedReceiver<SettingsUpdate>>,
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
            app_server_models: Vec::new(),
            app_server_model_error: None,
            settings_update_tx: None,
            settings_update_rx: None,
        }
    }
}

pub(crate) enum SettingsUpdate {
    ModelsLoaded(Vec<app_server::ModelInfo>),
    ConfigLoaded {
        model: Option<String>,
        reasoning_effort: Option<app_server::ReasoningEffort>,
    },
    Error(String),
}
