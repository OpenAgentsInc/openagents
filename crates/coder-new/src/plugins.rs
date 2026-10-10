//! Plugin preferences, private persistence, and masked credential editing.

use crossterm::event::{KeyCode, KeyEvent};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Draft,
    models::{DEFAULT_MODEL, GenerationOptions, Model, OPENROUTER_PLUGIN},
    plugin_definition::{
        DEFINITIONS, ModelProviderBinding, RailBinding, ResolvedRail, resolve_composer_rails,
    },
    plugin_store::{SavedPlugin, Store},
};

pub const ENDPOINT: &str = "https://openrouter.ai/api/v1";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
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

pub struct Plugins {
    pub selected: usize,
    pub(crate) catalog_revision: u64,
    #[cfg(unix)]
    installed: Vec<background::plugins::Installed>,
    pub bundled: crate::bundled_settings::BundledSettings,
    pub enabled: bool,
    pub key_configured: bool,
    pub model: String,
    pub options: GenerationOptions,
    pub focus: SettingsFocus,
    key_draft: Draft,
    model_draft: Draft,
    remove_key: bool,
    pub error: Option<&'static str>,
    pub storage_error: Option<String>,
    pub connection: Connection,
    pub check_requested: bool,
    pub saved: bool,
    pub credential_changed: bool,
    live: bool,
    live_key: Option<model_access::ApiKey>,
    other_preferences: Preferences,
    saved_connection: Option<Connection>,
    store: Option<Store>,
    explicit_preferences: bool,
}

struct Preferences {
    enabled: bool,
    key_configured: bool,
    model: String,
    options: GenerationOptions,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: false,
            key_configured: false,
            model: DEFAULT_MODEL.into(),
            options: GenerationOptions::default(),
        }
    }
}

impl Default for Plugins {
    fn default() -> Self {
        Self {
            selected: 0,
            catalog_revision: 0,
            #[cfg(unix)]
            installed: Vec::new(),
            bundled: crate::bundled_settings::BundledSettings::default(),
            enabled: false,
            key_configured: false,
            model: DEFAULT_MODEL.into(),
            options: GenerationOptions::default(),
            focus: SettingsFocus::default(),
            key_draft: Draft::default(),
            model_draft: Draft::default(),
            remove_key: false,
            error: None,
            storage_error: None,
            connection: Connection::default(),
            check_requested: false,
            saved: false,
            credential_changed: false,
            live: !crate::DEMO_AVAILABLE,
            live_key: None,
            other_preferences: Preferences::default(),
            saved_connection: None,
            store: None,
            explicit_preferences: false,
        }
    }
}

#[derive(Clone, Default)]
pub enum Connection {
    #[default]
    Unchecked,
    Checking,
    Verified,
    Failed(String),
}

/// A picker row describes either a host binding or an installed package.
#[derive(Clone, Copy)]
pub struct PickerDefinition<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub description: &'a str,
}

impl<'a> From<&'a crate::plugin_definition::PluginDefinition> for PickerDefinition<'a> {
    fn from(definition: &'a crate::plugin_definition::PluginDefinition) -> Self {
        Self {
            id: definition.id,
            name: definition.name,
            description: definition.description,
        }
    }
}

impl Plugins {
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

    pub fn load_settings(&mut self, store: Store) -> Result<(), String> {
        let bundled_result = self.bundled.load(store.clone());
        self.explicit_preferences = store.contains_settings();
        let loaded = store.load();
        self.store = Some(store);
        match loaded {
            Ok(saved) => {
                let preferences = Preferences {
                    enabled: saved.enabled,
                    key_configured: saved.key.is_some(),
                    model: saved.model,
                    options: saved.options,
                };
                self.live_key = saved.key;
                if self.live {
                    self.enabled = preferences.enabled;
                    self.key_configured = preferences.key_configured;
                    self.model = preferences.model;
                    self.options = preferences.options;
                } else {
                    self.other_preferences = preferences;
                }
                self.connection = Connection::Unchecked;
                self.storage_error = None;
                bundled_result
            }
            Err(error) => {
                self.explicit_preferences = true;
                self.storage_error = Some(error.clone());
                Err(error)
            }
        }
    }

    /// Import startup credentials without writing files or overriding saved preferences.
    pub fn bootstrap_credentials(&mut self, imported: crate::credentials::Imported) {
        if self.live_key.is_none() {
            self.live_key = imported.openrouter_key;
        }
        if self.live {
            self.key_configured = self.live_key.is_some();
            if self.key_configured && !self.explicit_preferences {
                self.enabled = true;
            }
        } else {
            self.other_preferences.key_configured = self.live_key.is_some();
            if self.other_preferences.key_configured && !self.explicit_preferences {
                self.other_preferences.enabled = true;
            }
        }
        self.bundled.import_jev_environment(
            imported.jev_key,
            imported.jev_endpoint,
            imported.gateway_key,
            imported.jev_model,
        );
    }

    pub fn storage_label(&self) -> &'static str {
        if self.store.is_some() {
            "Settings file: ~/.openagents/coder-new/plugins.json."
        } else {
            "The key stays in memory until you quit."
        }
    }

    pub fn toggle_enabled(&mut self) -> bool {
        let enabled = !self.enabled;
        if self.live
            && !self.persist(
                enabled,
                &self.model.clone(),
                &self.options.clone(),
                self.live_key.clone(),
            )
        {
            return false;
        }
        self.enabled = enabled;
        true
    }

    fn persist(
        &mut self,
        enabled: bool,
        model: &str,
        options: &GenerationOptions,
        key: Option<model_access::ApiKey>,
    ) -> bool {
        if let Some(store) = &self.store {
            if let Err(error) = store.save(&SavedPlugin {
                enabled,
                model: model.into(),
                options: options.clone(),
                key,
            }) {
                self.storage_error = Some(error);
                return false;
            }
        }
        self.storage_error = None;
        self.explicit_preferences = true;
        true
    }

    pub fn set_model(&mut self, model: &Model, options: GenerationOptions) -> bool {
        if model.plugin != OPENROUTER_PLUGIN
            || !options.valid()
            || options
                .reasoning
                .as_ref()
                .is_some_and(|effort| !model.efforts.contains(effort))
            || options.max_tokens.is_some_and(|limit| {
                !model.supports_output_limit
                    || model
                        .max_output_tokens
                        .is_some_and(|maximum| limit > maximum)
            })
        {
            self.storage_error = Some("This model does not support those settings.".into());
            return false;
        }
        if self.live && !self.persist(self.enabled, &model.id, &options, self.live_key.clone()) {
            return false;
        }
        self.model.clone_from(&model.id);
        self.options = options;
        true
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

    pub fn set_live(&mut self, live: bool) {
        if self.live == live {
            return;
        }
        self.discard_draft();
        std::mem::swap(&mut self.enabled, &mut self.other_preferences.enabled);
        std::mem::swap(
            &mut self.key_configured,
            &mut self.other_preferences.key_configured,
        );
        std::mem::swap(&mut self.model, &mut self.other_preferences.model);
        std::mem::swap(&mut self.options, &mut self.other_preferences.options);
        self.live = live;
        self.bundled.set_live(live);
        self.selected = self.selected.min(self.definitions().count() - 1);
    }

    pub fn definitions(&self) -> impl Iterator<Item = PickerDefinition<'_>> {
        let bundled = DEFINITIONS.iter().map(PickerDefinition::from);
        #[cfg(unix)]
        let definitions =
            bundled.chain(self.installed.iter().filter(|_| self.live).map(|plugin| {
                PickerDefinition {
                    id: &plugin.id,
                    name: &plugin.name,
                    description: &plugin.summary,
                }
            }));
        #[cfg(not(unix))]
        let definitions = bundled;
        definitions
    }

    pub fn selected_definition(&self) -> PickerDefinition<'_> {
        self.definitions()
            .nth(self.selected)
            .unwrap_or_else(|| PickerDefinition::from(&DEFINITIONS[0]))
    }

    #[cfg(unix)]
    pub fn selected_installed(&self) -> Option<&background::plugins::Installed> {
        self.installed_for(self.selected_definition().id)
    }

    #[cfg(unix)]
    fn installed_for(&self, id: &str) -> Option<&background::plugins::Installed> {
        self.installed
            .iter()
            .find(|plugin| self.live && plugin.id == id)
    }

    /// Replace the catalog without changing selection when its identity still exists.
    /// Return whether the selected package was removed.
    #[cfg(unix)]
    pub(crate) fn replace_installed(
        &mut self,
        installed: Vec<background::plugins::Installed>,
    ) -> bool {
        let selected = self.selected_definition().id.to_owned();
        self.installed = installed;
        let position = self.definitions().position(|plugin| plugin.id == selected);
        self.selected =
            position.unwrap_or_else(|| self.selected.min(self.definitions().count() - 1));
        position.is_none()
    }

    pub fn enabled_for(&self, id: &str) -> bool {
        #[cfg(unix)]
        if let Some(plugin) = self.installed_for(id) {
            return plugin.enabled;
        }
        if id == OPENROUTER_PLUGIN {
            self.enabled
        } else {
            self.bundled.enabled(id)
        }
    }

    pub fn status_for(&self, id: &str) -> &str {
        #[cfg(unix)]
        if let Some(plugin) = self.installed_for(id) {
            return if plugin.enabled {
                "Enabled"
            } else {
                "Disabled"
            };
        }
        if id == OPENROUTER_PLUGIN {
            self.status()
        } else {
            self.bundled.status(id)
        }
    }

    pub fn toggle_selected(&mut self) -> bool {
        #[cfg(unix)]
        if self.selected_installed().is_some() {
            return false;
        }
        let id = DEFINITIONS[self.selected.min(DEFINITIONS.len() - 1)].id;
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
            (self.selected + 1).min(self.definitions().count() - 1)
        };
    }

    pub fn execution_settings(
        &self,
        cwd: std::path::PathBuf,
    ) -> crate::plugin_tools::ExecutionSettings {
        let memory = crate::memory::Memory::discover(&cwd);
        crate::plugin_tools::ExecutionSettings {
            prompt_inbox: None,
            fleet: None,
            connections: None,
            boat: crate::cloud_settings::Configuration {
                enabled: self.live && self.bundled.boat.enabled,
                ..self.bundled.boat.clone()
            },
            gce: crate::cloud_settings::Configuration {
                enabled: self.live && self.bundled.gce.enabled,
                ..self.bundled.gce.clone()
            },
            cloud_root: self.bundled.cloud_root(),
            remote_targets: Default::default(),
            microcoder: self.bundled.microcoder,
            cli: self.bundled.cli,
            acp: self.bundled.acp,
            jev_enabled: self.bundled.jev_enabled,
            jev_key: self.bundled.jev_key(),
            jev_model: self.bundled.jev_model().into(),
            jev_endpoint: self.bundled.jev_endpoint().into(),
            redaction_keys: self
                .key_for_request()
                .into_iter()
                .chain(self.bundled.jev_key())
                .collect(),
            agents: self.bundled.acp_registered(),
            cwd,
            instructions: None,
            shell: true,
            brainstorm: self.bundled.brainstorm.native(),
            disclosure_desk: crate::approval::desk(),
            memory,
        }
    }

    pub fn key_for_request(&self) -> Option<model_access::ApiKey> {
        self.live.then(|| self.live_key.clone()).flatten()
    }

    pub fn key_for_check(&self) -> Option<model_access::ApiKey> {
        if !self.live {
            return None;
        }
        if self.key_draft.text.is_empty() {
            self.live_key.clone()
        } else {
            Some(model_access::ApiKey::new(self.key_draft.text.clone()))
        }
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
        self.model_draft.text.clone_from(&self.model);
        self.model_draft.cursor = self.model.len();
        self.focus = SettingsFocus::ApiKey;
    }

    pub fn discard_draft(&mut self) {
        self.key_draft = Draft::default();
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

    /// Returns display text and cursor position without exposing key bytes.
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

    /// Returns true when settings were saved or canceled.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
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
                    self.key_draft = Draft::default();
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

    fn save(&mut self) -> bool {
        if self.key_draft.text.chars().any(char::is_whitespace) {
            self.error = Some("The API key cannot contain spaces.");
            self.focus = SettingsFocus::ApiKey;
            return false;
        }
        if self.live {
            let key = if !self.key_draft.text.is_empty() {
                Some(model_access::ApiKey::new(self.key_draft.text.clone()))
            } else if self.remove_key {
                None
            } else {
                self.live_key.clone()
            };
            let model = self.edited_model();
            let options = if model == self.model {
                self.options.clone()
            } else {
                GenerationOptions::default()
            };
            let enabled = self.enabled || (!self.explicit_preferences && key.is_some());
            if !self.persist(enabled, &model, &options, key) {
                return false;
            }
            self.enabled = enabled;
            if !self.key_draft.text.is_empty() {
                self.live_key = Some(model_access::ApiKey::new(self.key_draft.text.clone()));
                self.connection = Connection::Unchecked;
                self.credential_changed = true;
            } else if self.remove_key {
                self.live_key = None;
                self.connection = Connection::Unchecked;
                self.credential_changed = true;
            } else {
                self.restore_connection();
            }
            self.key_configured = self.live_key.is_some();
        } else {
            self.key_configured =
                !self.key_draft.text.is_empty() || (self.key_configured && !self.remove_key);
        }
        let model = self.edited_model();
        if model != self.model {
            self.options = GenerationOptions::default();
        }
        self.model = model;
        self.discard_draft();
        self.saved = true;
        self.saved_connection = None;
        true
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

#[cfg(test)]
mod rail_tests {
    use super::*;
    use crate::{models::openrouter_catalog, plugin_definition::RailSlot};

    #[test]
    fn enabled_definition_registers_selected_model_without_requiring_a_key() {
        let mut plugins = Plugins::default();
        assert!(plugins.composer_rails().is_empty());
        assert!(plugins.toggle_enabled());
        assert!(!plugins.key_configured);
        assert_eq!(
            plugins.composer_rails(),
            vec![ResolvedRail {
                slot: RailSlot::ComposerTopRight,
                text: DEFAULT_MODEL.to_owned(),
            }]
        );
        assert!(plugins.set_model(&openrouter_catalog()[1], GenerationOptions::default()));
        assert_eq!(plugins.composer_rails()[0].text, "openai/gpt-6-luna");
        assert!(plugins.toggle_enabled());
        assert!(plugins.composer_rails().is_empty());
    }

    #[test]
    fn rail_follows_the_current_modes_plugin_preferences() {
        let mut plugins = Plugins::default();
        plugins.toggle_enabled();
        plugins.set_model(&openrouter_catalog()[3], GenerationOptions::default());
        plugins.set_live(true);
        assert!(plugins.composer_rails().is_empty());
        plugins.toggle_enabled();
        assert_eq!(plugins.composer_rails()[0].text, DEFAULT_MODEL.to_owned());
        plugins.set_live(false);
        assert_eq!(
            plugins.composer_rails()[0].text,
            "anthropic/claude-fable-5.1"
        );
        plugins.set_live(true);
        assert_eq!(plugins.composer_rails()[0].text, DEFAULT_MODEL.to_owned());
    }

    #[test]
    fn startup_key_enables_openrouter_without_writing_settings() {
        let temporary = tempfile::tempdir().unwrap();
        let mut plugins = Plugins::default();
        plugins
            .load_settings(Store::under(temporary.path()))
            .unwrap();
        plugins.bootstrap_credentials(crate::credentials::Imported {
            openrouter_key: Some(model_access::ApiKey::new("fixture-openrouter")),
            jev_key: Some(model_access::ApiKey::new("fixture-jev")),
            ..Default::default()
        });
        assert!(!plugins.enabled);
        assert!(plugins.key_for_request().is_none());
        assert!(plugins.bundled.jev_key().is_none());
        plugins.set_live(true);
        assert!(plugins.enabled && plugins.key_configured);
        assert_eq!(
            plugins.key_for_request().unwrap().expose(),
            "fixture-openrouter"
        );
        assert_eq!(plugins.bundled.jev_key().unwrap().expose(), "fixture-jev");
        assert!(plugins.bundled.jev_enabled);
        assert!(!temporary.path().join("plugins.json").exists());
        assert!(!temporary.path().join("bundled-plugins.json").exists());
    }

    #[test]
    fn saved_disabled_preferences_survive_startup_imports_and_keep_saved_key() {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::under(temporary.path());
        store
            .save(&SavedPlugin {
                key: Some(model_access::ApiKey::new("configured-key")),
                ..Default::default()
            })
            .unwrap();
        let original = std::fs::read(temporary.path().join("plugins.json")).unwrap();
        let mut plugins = Plugins::default();
        plugins.set_live(true);
        plugins.load_settings(store).unwrap();
        plugins.bootstrap_credentials(crate::credentials::Imported {
            openrouter_key: Some(model_access::ApiKey::new("environment-key")),
            ..Default::default()
        });
        assert!(!plugins.enabled);
        assert!(plugins.key_configured);
        assert_eq!(
            plugins.key_for_request().unwrap().expose(),
            "configured-key"
        );
        assert_eq!(
            std::fs::read(temporary.path().join("plugins.json")).unwrap(),
            original
        );
    }

    #[test]
    fn saved_off_without_a_key_stays_off_when_a_key_is_imported() {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::under(temporary.path());
        store.save(&SavedPlugin::default()).unwrap();
        let mut plugins = Plugins::default();
        plugins.set_live(true);
        plugins.load_settings(store).unwrap();
        plugins.bootstrap_credentials(crate::credentials::Imported {
            openrouter_key: Some(model_access::ApiKey::new("environment-key")),
            ..Default::default()
        });
        assert!(!plugins.enabled);
        assert!(plugins.key_configured);
        assert_eq!(
            plugins.key_for_request().unwrap().expose(),
            "environment-key"
        );
    }
}

impl Plugins {
    pub(crate) fn demo_view(&self) -> coder_ui::demo::plugins::Plugins {
        let mut value = coder_ui::demo::plugins::Plugins::default();
        value.selected = self.selected;
        value.bundled = self.bundled.demo_view();
        value.enabled = self.enabled;
        value.key_configured = self.key_configured;
        value.model = self.model.clone();
        value.options = crate::demo::options(&self.options);
        value.focus = crate::demo::focus(self.focus);
        value.key_draft = crate::demo::draft(&self.key_draft);
        value.model_draft = crate::demo::draft(&self.model_draft);
        value.remove_key = self.remove_key;
        value.error = self.error;
        value.storage_error = self.storage_error.clone();
        value.connection = crate::demo::connection(&self.connection);
        value.check_requested = self.check_requested;
        value.saved = self.saved;
        value.credential_changed = self.credential_changed;
        value.live = self.live;
        value.saved_connection = self.saved_connection.as_ref().map(crate::demo::connection);
        value
    }
}
