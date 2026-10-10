//! Page-local original plugin presentation and editors, without providers or storage.
use super::{
    Draft, Key, KeyCode,
    models::{DEFAULT_MODEL, GenerationOptions, Model, OPENROUTER_PLUGIN},
    plugin_definition::{
        DEFINITIONS, ModelProviderBinding, RailBinding, ResolvedRail, resolve_composer_rails,
    },
};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;
pub const ENDPOINT: &str = "https://openrouter.ai/api/v1";
#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingsFocus {
    #[default]
    ApiKey,
    Gateway,
    Endpoint,
    Model,
    TestKey,
    Save,
    RemoveKey,
    Cancel,
}

impl SettingsFocus {
    fn next(self, backwards: bool) -> Self {
        let fields = [
            Self::ApiKey,
            Self::Model,
            Self::TestKey,
            Self::Save,
            Self::RemoveKey,
            Self::Cancel,
        ];
        let index = fields.iter().position(|field| *field == self).unwrap_or(0);
        fields[(index + if backwards { fields.len() - 1 } else { 1 }) % fields.len()]
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub enum Connection {
    #[default]
    Unchecked,
    Checking,
    Verified,
    Failed(String),
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Plugins {
    pub selected: usize,
    pub bundled: super::bundled_settings::BundledSettings,
    pub enabled: bool,
    pub key_configured: bool,
    pub model: String,
    pub options: GenerationOptions,
    pub focus: SettingsFocus,
    #[serde(skip)]
    pub key_draft: Draft,
    pub model_draft: Draft,
    pub remove_key: bool,
    #[serde(skip)]
    pub error: Option<&'static str>,
    pub storage_error: Option<String>,
    pub connection: Connection,
    pub check_requested: bool,
    pub saved: bool,
    pub credential_changed: bool,
    pub live: bool,
    pub saved_connection: Option<Connection>,
}
impl Default for Plugins {
    fn default() -> Self {
        Self {
            selected: 0,
            bundled: Default::default(),
            enabled: false,
            key_configured: false,
            model: DEFAULT_MODEL.into(),
            options: Default::default(),
            focus: Default::default(),
            key_draft: Default::default(),
            model_draft: Default::default(),
            remove_key: false,
            error: None,
            storage_error: None,
            connection: Default::default(),
            check_requested: false,
            saved: false,
            credential_changed: false,
            live: false,
            saved_connection: None,
        }
    }
}
impl Plugins {
    pub fn toggle_enabled(&mut self) -> bool {
        self.enabled = !self.enabled;
        true
    }
    pub fn storage_label(&self) -> &str {
        "Demo settings last only until you quit"
    }
    pub fn set_model(&mut self, model: &Model, options: GenerationOptions) -> bool {
        if model.plugin != OPENROUTER_PLUGIN
            || !options.valid()
            || options
                .reasoning
                .as_ref()
                .is_some_and(|v| !model.efforts.contains(v))
            || options.max_tokens.is_some_and(|v| {
                !model.supports_output_limit || model.max_output_tokens.is_some_and(|m| v > m)
            })
        {
            self.storage_error = Some("This model does not support those settings.".into());
            return false;
        }
        self.model.clone_from(&model.id);
        self.options = options;
        true
    }
    fn save(&mut self) -> bool {
        if self.key_draft.text.chars().any(char::is_whitespace) {
            self.error = Some("The API key cannot contain spaces.");
            self.focus = SettingsFocus::ApiKey;
            return false;
        }
        self.key_configured =
            !self.key_draft.text.is_empty() || (self.key_configured && !self.remove_key);
        let model = self.edited_model();
        if model != self.model {
            self.options = Default::default();
        }
        self.model = model;
        self.discard_draft();
        self.saved = true;
        self.saved_connection = None;
        true
    }
    pub fn composer_rails(&self) -> Vec<ResolvedRail> {
        let model = self.options.slug(&self.model);
        resolve_composer_rails(DEFINITIONS, |definition, binding| {
            if !self.enabled || definition.id != OPENROUTER_PLUGIN {
                return None;
            }
            match (definition.model_provider, binding) {
                (Some(ModelProviderBinding::OpenRouter), RailBinding::SelectedModel) => {
                    Some(model.as_str())
                }
                _ => None,
            }
        })
    }
    pub fn status(&self) -> &'static str {
        match (self.enabled, self.key_configured) {
            (false, _) => "Disabled",
            (true, false) => "Setup required",
            (true, true) if self.live => match self.connection {
                Connection::Checking => "Checking",
                Connection::Verified => "Verified",
                Connection::Failed(_) => "Unavailable",
                Connection::Unchecked => "Configured",
            },
            (true, true) => "Configured",
        }
    }
    pub fn selected_definition(&self) -> &'static super::plugin_definition::PluginDefinition {
        &DEFINITIONS[self.selected.min(DEFINITIONS.len() - 1)]
    }
    pub fn enabled_for(&self, id: &str) -> bool {
        if id == OPENROUTER_PLUGIN {
            self.enabled
        } else {
            self.bundled.enabled(id)
        }
    }
    pub fn status_for(&self, id: &str) -> &str {
        if id == OPENROUTER_PLUGIN {
            self.status()
        } else {
            self.bundled.status(id)
        }
    }
    pub fn toggle_selected(&mut self) -> bool {
        let id = self.selected_definition().id;
        if id == OPENROUTER_PLUGIN {
            self.toggle_enabled()
        } else {
            self.bundled.toggle(id)
        }
    }
    pub fn select(&mut self, backwards: bool) {
        self.selected = if backwards {
            self.selected.saturating_sub(1)
        } else {
            (self.selected + 1).min(DEFINITIONS.len() - 1)
        };
    }
    pub fn connection_label(&self) -> &str {
        if !self.live {
            return "Demo · no requests sent";
        }
        match &self.connection {
            Connection::Unchecked => "Not checked",
            Connection::Checking => "Checking OpenRouter API key…",
            Connection::Verified => "OpenRouter API key verified",
            Connection::Failed(error) => error,
        }
    }
    pub fn begin_settings(&mut self) {
        self.discard_draft();
        self.saved_connection = Some(self.connection.clone());
        // The default shows as an empty field: "Leave empty for auto".
        if self.model == DEFAULT_MODEL {
            self.model_draft.text.clear();
        } else {
            self.model_draft.text.clone_from(&self.model);
        }
        self.model_draft.cursor = self.model_draft.text.len();
        self.focus = SettingsFocus::ApiKey;
    }
    pub fn discard_draft(&mut self) {
        self.key_draft.erase();
        self.model_draft = Draft::default();
        self.remove_key = false;
        self.error = None;
    }
    pub fn key_label(&self) -> &'static str {
        if self.remove_key && self.key_draft.text.is_empty() {
            "Key will be removed on save"
        } else if self.key_configured && self.key_draft.text.is_empty() {
            "Key added · paste to replace"
        } else if !self.key_draft.text.is_empty() {
            "Key hidden"
        } else {
            "Get a key at openrouter.ai/keys"
        }
    }
    pub fn field(&self, key: bool) -> (String, usize) {
        if key {
            (
                "•".repeat(self.key_draft.text.graphemes(true).count()),
                self.key_draft.text[..self.key_draft.cursor]
                    .graphemes(true)
                    .count()
                    * "•".len(),
            )
        } else {
            (self.model_draft.text.clone(), self.model_draft.cursor)
        }
    }
    pub fn paste(&mut self, text: &str) {
        let draft = match self.focus {
            SettingsFocus::ApiKey => &mut self.key_draft,
            SettingsFocus::Model => &mut self.model_draft,
            _ => return,
        };
        draft.insert(&text.trim().replace(['\r', '\n'], ""));
        self.error = None;
    }
    pub fn handle(&mut self, key: Key) -> bool {
        self.saved = false;
        self.credential_changed = false;
        self.check_requested = false;
        match key.code {
            KeyCode::Tab | KeyCode::Down => self.focus = self.focus.next(false),
            KeyCode::BackTab | KeyCode::Up => self.focus = self.focus.next(true),
            KeyCode::Esc => {
                self.discard_draft();
                self.restore_connection();
                return true;
            }
            KeyCode::Enter => match self.focus {
                SettingsFocus::Gateway | SettingsFocus::Endpoint => {
                    self.focus = SettingsFocus::ApiKey;
                }
                SettingsFocus::ApiKey => self.focus = SettingsFocus::Model,
                SettingsFocus::Model => self.focus = SettingsFocus::Save,
                SettingsFocus::TestKey => self.check_requested = true,
                SettingsFocus::Save => return self.save(),
                SettingsFocus::RemoveKey => {
                    self.key_draft.erase();
                    self.remove_key = true;
                    self.error = None;
                }
                SettingsFocus::Cancel => {
                    self.discard_draft();
                    self.restore_connection();
                    return true;
                }
            },
            _ => {
                let draft = match self.focus {
                    SettingsFocus::ApiKey => &mut self.key_draft,
                    SettingsFocus::Model => &mut self.model_draft,
                    _ => return false,
                };
                draft.edit(key);
                self.error = None;
            }
        }
        false
    }
    fn edited_model(&self) -> String {
        let model = self.model_draft.text.trim();
        if model.is_empty() {
            DEFAULT_MODEL.into()
        } else {
            model.into()
        }
    }
    fn restore_connection(&mut self) {
        if let Some(connection) = self.saved_connection.take() {
            self.connection = if matches!(connection, Connection::Checking) {
                Connection::Unchecked
            } else {
                connection
            };
        }
    }
}

impl Drop for Plugins {
    fn drop(&mut self) {
        self.key_draft.erase();
    }
}
