//! Private settings and editors for the bundled tools and ACP agents.

use crossterm::event::{KeyCode, KeyEvent};
use model_access::ApiKey;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Draft,
    bundled_runtime::AcpAgent,
    plugin_store::Store,
    plugins::{Connection, SettingsFocus},
};

const FILE: &str = "bundled-plugins.json";
const INVALID: &str = "The saved bundled plugin settings are invalid. The file was not changed.";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    version: u32,
    microcoder: bool,
    cli: bool,
    acp: bool,
    jev_enabled: bool,
    jev_model: String,
    jev_endpoint: String,
    #[serde(serialize_with = "write_key", deserialize_with = "read_key")]
    jev_key: Option<ApiKey>,
    acp_agents: Vec<AcpAgent>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            microcoder: true,
            cli: true,
            acp: true,
            jev_enabled: true,
            jev_model: jev::defaults::MODEL.into(),
            jev_endpoint: crate::jev_plugin::DEFAULT_ENDPOINT.into(),
            jev_key: None,
            acp_agents: Vec::new(),
        }
    }
}

fn write_key<S: Serializer>(key: &Option<ApiKey>, serializer: S) -> Result<S::Ok, S::Error> {
    key.as_ref().map(ApiKey::expose).serialize(serializer)
}

fn read_key<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<ApiKey>, D::Error> {
    let key = Option::<String>::deserialize(deserializer)?;
    if key.as_ref().is_some_and(|key| !valid_key(key)) {
        return Err(serde::de::Error::custom("Invalid API key."));
    }
    Ok(key.map(ApiKey::new))
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 16 * 1024
        && !key
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 128
        && model.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '/' | ':')
        })
}

fn valid_endpoint(endpoint: &str) -> bool {
    if endpoint.len() > 2048 {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    let loopback = url.host_str().is_some_and(|host| {
        host.parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
    });
    (url.scheme() == "https" || url.scheme() == "http" && loopback)
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn valid_agents(agents: &[AcpAgent]) -> Result<(), String> {
    if agents.len() > 32 {
        return Err("Configure at most 32 ACP subagents.".into());
    }
    let mut ids = std::collections::HashSet::new();
    for agent in agents {
        agent.validate()?;
        if !ids.insert(&agent.id) {
            return Err("Each ACP subagent must have a unique ID.".into());
        }
    }
    Ok(())
}

impl Preferences {
    fn valid(&self) -> bool {
        self.version == 1
            && valid_model(&self.jev_model)
            && valid_endpoint(&self.jev_endpoint)
            && self
                .jev_key
                .as_ref()
                .is_none_or(|key| valid_key(key.expose()))
            && valid_agents(&self.acp_agents).is_ok()
    }
}

pub struct BundledSettings {
    pub microcoder: bool,
    pub cli: bool,
    pub acp: bool,
    pub jev_enabled: bool,
    pub acp_agents: Vec<AcpAgent>,
    pub focus: SettingsFocus,
    pub error: Option<&'static str>,
    pub storage_error: Option<String>,
    pub connection: Connection,
    pub check_requested: bool,
    pub saved: bool,
    pub credential_changed: bool,
    pub acp_draft: Draft,
    pub acp_error: Option<String>,
    jev_key: Option<ApiKey>,
    jev_model: String,
    jev_endpoint: String,
    key_draft: Draft,
    model_draft: Draft,
    remove_key: bool,
    live: bool,
    other: Preferences,
    store: Option<Store>,
    configured: bool,
    saved_connection: Option<Connection>,
}

impl Default for BundledSettings {
    fn default() -> Self {
        let defaults = Preferences::default();
        Self {
            microcoder: true,
            cli: true,
            acp: true,
            jev_enabled: true,
            acp_agents: Vec::new(),
            focus: SettingsFocus::ApiKey,
            error: None,
            storage_error: None,
            connection: Connection::Unchecked,
            check_requested: false,
            saved: false,
            credential_changed: false,
            acp_draft: Draft::default(),
            acp_error: None,
            jev_key: None,
            jev_model: defaults.jev_model.clone(),
            jev_endpoint: defaults.jev_endpoint.clone(),
            key_draft: Draft::default(),
            model_draft: Draft::default(),
            remove_key: false,
            live: false,
            other: defaults,
            store: None,
            configured: false,
            saved_connection: None,
        }
    }
}

impl BundledSettings {
    fn preferences(&self) -> Preferences {
        Preferences {
            version: 1,
            microcoder: self.microcoder,
            cli: self.cli,
            acp: self.acp,
            jev_enabled: self.jev_enabled,
            jev_model: self.jev_model.clone(),
            jev_endpoint: self.jev_endpoint.clone(),
            jev_key: self.jev_key.clone(),
            acp_agents: self.acp_agents.clone(),
        }
    }

    fn apply(&mut self, value: Preferences) {
        self.microcoder = value.microcoder;
        self.cli = value.cli;
        self.acp = value.acp;
        self.jev_enabled = value.jev_enabled;
        self.jev_model = value.jev_model;
        self.jev_endpoint = value.jev_endpoint;
        self.jev_key = value.jev_key;
        self.acp_agents = value.acp_agents;
    }

    pub fn set_live(&mut self, live: bool) {
        if live == self.live {
            return;
        }
        self.discard();
        self.cancel_acp();
        let current = self.preferences();
        let next = std::mem::replace(&mut self.other, current);
        self.apply(next);
        self.live = live;
        self.connection = Connection::Unchecked;
    }

    pub fn load(&mut self, store: Store) -> Result<(), String> {
        let loaded = store.read_extra::<Preferences>(FILE);
        self.store = Some(store);
        let loaded = match loaded {
            Ok(value) if value.as_ref().is_none_or(Preferences::valid) => value,
            _ => {
                self.storage_error = Some(INVALID.into());
                return Err(INVALID.into());
            }
        };
        self.configured = loaded.is_some();
        if let Some(value) = loaded {
            if self.live {
                self.apply(value);
            } else {
                self.other = value;
            }
        }
        self.storage_error = None;
        Ok(())
    }

    pub fn set_jev_environment(&mut self, key: Option<ApiKey>, endpoint: Option<String>) {
        let mut value = if self.live {
            self.preferences()
        } else {
            self.other.clone()
        };
        if value.jev_key.is_none() {
            value.jev_key = key.filter(|key| valid_key(key.expose()));
        }
        if !self.configured {
            if let Some(endpoint) = endpoint.filter(|endpoint| valid_endpoint(endpoint)) {
                value.jev_endpoint = endpoint;
            }
        }
        if self.live {
            self.apply(value);
        } else {
            self.other = value;
        }
    }

    pub fn enabled(&self, id: &str) -> bool {
        match id {
            "microcoder" => self.microcoder,
            "openagents-cli" => self.cli,
            "acp-subagents" => self.acp,
            "jev" => self.jev_enabled,
            _ => false,
        }
    }

    pub fn toggle(&mut self, id: &str) -> bool {
        let mut value = self.preferences();
        let flag = match id {
            "microcoder" => &mut value.microcoder,
            "openagents-cli" => &mut value.cli,
            "acp-subagents" => &mut value.acp,
            "jev" => &mut value.jev_enabled,
            _ => return false,
        };
        *flag = !*flag;
        if !self.persist(&value) {
            return false;
        }
        self.apply(value);
        true
    }

    pub fn status(&self, id: &str) -> &str {
        if !self.enabled(id) {
            return "Disabled";
        }
        match id {
            "jev" if self.jev_key.is_none() => "Setup required",
            "jev" => match self.connection {
                Connection::Checking => "Checking",
                Connection::Verified => "Verified",
                Connection::Failed(_) => "Unavailable",
                Connection::Unchecked => "Configured",
            },
            "acp-subagents" if self.acp_agents.iter().all(|agent| !agent.enabled) => {
                "Setup required"
            }
            _ => "Enabled",
        }
    }

    pub fn jev_key(&self) -> Option<ApiKey> {
        self.live.then(|| self.jev_key.clone()).flatten()
    }
    pub fn jev_endpoint(&self) -> &str {
        &self.jev_endpoint
    }
    pub fn jev_model(&self) -> &str {
        &self.jev_model
    }

    fn persist(&mut self, value: &Preferences) -> bool {
        if !self.live {
            return true;
        }
        if !value.valid() {
            self.storage_error = Some(INVALID.into());
            return false;
        }
        if let Some(store) = &self.store {
            match store.read_extra::<Preferences>(FILE) {
                Ok(existing) if existing.as_ref().is_none_or(Preferences::valid) => {}
                _ => {
                    self.storage_error = Some(INVALID.into());
                    return false;
                }
            }
            if let Err(error) = store.save_extra(FILE, value) {
                self.storage_error = Some(error);
                return false;
            }
        }
        self.storage_error = None;
        true
    }

    pub fn begin_settings(&mut self) {
        self.discard();
        self.saved_connection = Some(self.connection.clone());
        self.model_draft.text.clone_from(&self.jev_model);
        self.model_draft.cursor = self.model_draft.text.len();
        self.focus = SettingsFocus::ApiKey;
    }

    pub fn discard(&mut self) {
        self.key_draft = Draft::default();
        self.model_draft = Draft::default();
        self.remove_key = false;
        self.error = None;
        self.check_requested = false;
        if let Some(connection) = self.saved_connection.take() {
            self.connection = if matches!(connection, Connection::Checking) {
                Connection::Unchecked
            } else {
                connection
            };
        }
    }

    pub fn key_label(&self) -> &'static str {
        if self.remove_key && self.key_draft.text.is_empty() {
            "Key will be removed on save"
        } else if !self.key_draft.text.is_empty() {
            "Key hidden"
        } else if self.jev_key.is_some() {
            "Key added · paste to replace"
        } else {
            "Add your TypeSafe API key"
        }
    }

    pub fn connection_label(&self) -> &str {
        if !self.live {
            return "Demo · no requests sent";
        }
        match &self.connection {
            Connection::Unchecked => "Not checked",
            Connection::Checking => "Checking Jev API key…",
            Connection::Verified => "Jev API key verified",
            Connection::Failed(error) => error,
        }
    }

    pub fn key_for_check(&self) -> Option<ApiKey> {
        if !self.live {
            None
        } else if !self.key_draft.text.is_empty() {
            Some(ApiKey::new(&self.key_draft.text))
        } else if self.remove_key {
            None
        } else {
            self.jev_key.clone()
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
        if draft.text.len().saturating_add(text.len()) <= 16 * 1024 {
            draft.insert(&text.trim().replace(['\r', '\n'], ""));
            self.error = None;
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> bool {
        self.saved = false;
        self.credential_changed = false;
        self.check_requested = false;
        match key.code {
            KeyCode::Tab | KeyCode::Down | KeyCode::BackTab | KeyCode::Up => {
                let fields = [
                    SettingsFocus::ApiKey,
                    SettingsFocus::Model,
                    SettingsFocus::TestKey,
                    SettingsFocus::Save,
                    SettingsFocus::RemoveKey,
                    SettingsFocus::Cancel,
                ];
                let index = fields
                    .iter()
                    .position(|focus| *focus == self.focus)
                    .unwrap_or(0);
                self.focus = fields[(index
                    + if matches!(key.code, KeyCode::BackTab | KeyCode::Up) {
                        fields.len() - 1
                    } else {
                        1
                    })
                    % fields.len()];
            }
            KeyCode::Esc => {
                self.discard();
                return true;
            }
            KeyCode::Enter => match self.focus {
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
                    self.discard();
                    return true;
                }
            },
            _ => {
                let draft = match self.focus {
                    SettingsFocus::ApiKey => &mut self.key_draft,
                    SettingsFocus::Model => &mut self.model_draft,
                    _ => return false,
                };
                if draft.text.len() < 16 * 1024 || !matches!(key.code, KeyCode::Char(_)) {
                    draft.edit(key);
                }
                self.error = None;
            }
        }
        false
    }

    fn save(&mut self) -> bool {
        if !self.key_draft.text.is_empty() && !valid_key(&self.key_draft.text) {
            self.error = Some("The API key cannot contain spaces or control characters.");
            self.focus = SettingsFocus::ApiKey;
            return false;
        }
        let model = self.model_draft.text.trim();
        let model = if model.is_empty() {
            jev::defaults::MODEL
        } else {
            model
        };
        if !valid_model(model) {
            self.error = Some("Enter a valid Jev model ID using at most 128 bytes.");
            self.focus = SettingsFocus::Model;
            return false;
        }
        let mut value = self.preferences();
        value.jev_model = model.into();
        if !self.key_draft.text.is_empty() {
            value.jev_key = Some(ApiKey::new(&self.key_draft.text));
        } else if self.remove_key {
            value.jev_key = None;
        }
        let changed = value.jev_key != self.jev_key;
        if !self.persist(&value) {
            return false;
        }
        self.apply(value);
        self.discard();
        self.credential_changed = changed;
        self.saved = true;
        if changed {
            self.connection = Connection::Unchecked;
        }
        true
    }

    pub fn begin_acp(&mut self) {
        self.acp_draft.text =
            serde_json::to_string_pretty(&self.acp_agents).unwrap_or_else(|_| "[]".into());
        self.acp_draft.cursor = self.acp_draft.text.len();
        self.acp_error = None;
    }

    pub fn save_acp(&mut self) -> bool {
        if self.acp_draft.text.len() > 48 * 1024 {
            self.acp_error = Some("ACP settings exceed 48 KiB.".into());
            return false;
        }
        let agents: Vec<AcpAgent> = match serde_json::from_str(&self.acp_draft.text) {
            Ok(agents) => agents,
            Err(_) => {
                self.acp_error = Some("Enter a JSON array of ACP subagent definitions.".into());
                return false;
            }
        };
        if let Err(error) = valid_agents(&agents) {
            self.acp_error = Some(error);
            return false;
        }
        let mut value = self.preferences();
        value.acp_agents = agents;
        if !self.persist(&value) {
            return false;
        }
        self.apply(value);
        self.cancel_acp();
        true
    }

    pub fn cancel_acp(&mut self) {
        self.acp_draft = Draft::default();
        self.acp_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn save(settings: &mut BundledSettings) -> bool {
        settings.focus = SettingsFocus::Save;
        settings.handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
    }

    #[test]
    fn defaults_are_enabled_and_demo_edits_never_write_live_settings() {
        let temporary = tempfile::tempdir().unwrap();
        let mut settings = BundledSettings::default();
        settings.load(Store::under(temporary.path())).unwrap();
        for id in ["microcoder", "openagents-cli", "acp-subagents", "jev"] {
            assert!(settings.enabled(id));
            assert!(settings.toggle(id));
        }
        settings.begin_settings();
        settings.paste("fixture-key");
        assert!(save(&mut settings));
        assert!(settings.jev_key().is_none());
        assert!(!temporary.path().join(FILE).exists());
        settings.set_live(true);
        assert!(settings.microcoder && settings.cli && settings.acp && settings.jev_enabled);
        assert!(settings.jev_key().is_none());
        settings.set_live(false);
        assert!(!settings.microcoder && !settings.cli && !settings.acp && !settings.jev_enabled);
        assert_eq!(settings.key_label(), "Key added · paste to replace");
    }

    #[test]
    fn live_settings_round_trip_with_masked_editing_and_environment_fallback() {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::under(temporary.path());
        let mut settings = BundledSettings::default();
        settings.load(store.clone()).unwrap();
        settings.set_jev_environment(
            Some(ApiKey::new("fixture-key")),
            Some("https://example.invalid".into()),
        );
        settings.set_live(true);
        assert_eq!(settings.jev_endpoint(), "https://example.invalid");
        settings.begin_settings();
        settings.paste("replacement-key");
        assert!(!settings.field(true).0.contains("replacement"));
        settings.focus = SettingsFocus::Model;
        settings.model_draft = Draft::default();
        settings.paste("jev-fixture");
        assert!(save(&mut settings));
        let mut loaded = BundledSettings::default();
        loaded.load(store).unwrap();
        loaded.set_jev_environment(
            Some(ApiKey::new("environment-key")),
            Some("https://other.invalid".into()),
        );
        loaded.set_live(true);
        assert_eq!(loaded.jev_key().unwrap().expose(), "replacement-key");
        assert_eq!(loaded.jev_model(), "jev-fixture");
        assert_eq!(loaded.jev_endpoint(), "https://example.invalid");
        assert!(!format!("{:?}", loaded.preferences()).contains("replacement-key"));
    }

    #[test]
    fn newer_or_malformed_documents_block_writes_and_preserve_edits() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join(FILE);
        let store = Store::under(temporary.path());
        let mut newer = serde_json::to_value(Preferences::default()).unwrap();
        newer["version"] = serde_json::json!(2);
        for document in [newer.to_string(), "{broken}".into()] {
            std::fs::write(&path, &document).unwrap();
            let mut settings = BundledSettings::default();
            settings.set_live(true);
            assert!(settings.load(store.clone()).is_err());
            assert!(!settings.toggle("microcoder"));
            assert!(settings.microcoder);
            settings.begin_settings();
            settings.paste("fixture-key");
            assert!(!save(&mut settings));
            assert!(settings.jev_key().is_none());
            assert!(!settings.field(true).0.is_empty());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), document);
        }
    }

    #[test]
    fn acp_editor_accepts_explicit_programs_and_rejects_duplicate_ids() {
        let mut settings = BundledSettings::default();
        settings.begin_acp();
        let agent = serde_json::json!({"id":"reviewer","name":"Reviewer","program":"fixture-agent","arguments":["--acp"],"enabled":true});
        settings.acp_draft.text = serde_json::json!([agent.clone(), agent.clone()]).to_string();
        assert!(!settings.save_acp());
        assert!(settings.acp_agents.is_empty());
        assert!(settings.acp_error.as_deref().unwrap().contains("unique"));
        settings.acp_draft.text = serde_json::json!([agent]).to_string();
        assert!(settings.save_acp());
        assert_eq!(settings.acp_agents[0].id, "reviewer");
        assert!(settings.acp_draft.text.is_empty());
        settings.set_live(true);
        assert!(settings.acp_agents.is_empty());
    }
}
