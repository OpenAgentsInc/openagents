//! Private settings and editors for the bundled tools and ACP agents.

use crossterm::event::{KeyCode, KeyEvent};
use model_access::ApiKey;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
};
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
    #[serde(default)]
    brainstorm: crate::brainstorm::Preferences,
    #[serde(default)]
    boat: crate::cloud_settings::Configuration,
    #[serde(default = "crate::cloud_settings::Configuration::gce")]
    gce: crate::cloud_settings::Configuration,
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
            brainstorm: crate::brainstorm::Preferences::default(),
            boat: Default::default(),
            gce: crate::cloud_settings::Configuration::gce(),
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
        host.trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
    });
    (url.scheme() == "https" || url.scheme() == "http" && loopback)
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn same_origin(left: &str, right: &str) -> bool {
    match (reqwest::Url::parse(left), reqwest::Url::parse(right)) {
        (Ok(left), Ok(right)) => left.origin() == right.origin(),
        _ => false,
    }
}

fn gateway_label(endpoint: &str) -> &'static str {
    match endpoint.trim_end_matches('/') {
        crate::jev_plugin::DEFAULT_ENDPOINT => "TypeSafe direct",
        crate::jev_plugin::GATEWAY_ENDPOINT => "Vercel AI Gateway",
        _ => "Custom gateway",
    }
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
            && self.brainstorm.valid()
            && self.boat.valid(coder_cloud::Placement::Boat)
            && self.gce.valid(coder_cloud::Placement::Gce)
    }
}

pub struct BundledSettings {
    pub brainstorm: crate::brainstorm::Settings,
    pub boat: crate::cloud_settings::Configuration,
    pub gce: crate::cloud_settings::Configuration,
    pub cloud_editor: Option<crate::cloud_settings::Editor>,
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
    pub acp_selected: usize,
    acp_available: BTreeSet<String>,
    acp_environment: BTreeMap<String, OsString>,
    jev_key: Option<ApiKey>,
    jev_model: String,
    jev_endpoint: String,
    key_draft: Draft,
    model_draft: Draft,
    endpoint_draft: Draft,
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
            brainstorm: crate::brainstorm::Settings::default(),
            boat: Default::default(),
            gce: crate::cloud_settings::Configuration::gce(),
            cloud_editor: None,
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
            acp_selected: 0,
            acp_available: BTreeSet::new(),
            acp_environment: BTreeMap::new(),
            jev_key: None,
            jev_model: defaults.jev_model.clone(),
            jev_endpoint: defaults.jev_endpoint.clone(),
            key_draft: Draft::default(),
            model_draft: Draft::default(),
            endpoint_draft: Draft::default(),
            remove_key: false,
            live: !crate::DEMO_AVAILABLE,
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
            brainstorm: self.brainstorm.preferences.clone(),
            boat: self.boat.clone(),
            gce: self.gce.clone(),
        }
    }

    fn apply(&mut self, value: Preferences) {
        self.boat = value.boat;
        self.gce = value.gce;
        self.brainstorm.configure(value.brainstorm, self.live);
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
        let current = self.preferences();
        let next = std::mem::replace(&mut self.other, current);
        self.live = live;
        self.apply(next);
        self.connection = Connection::Unchecked;
        self.refresh_acp();
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
        self.refresh_acp();
        Ok(())
    }

    pub fn set_jev_environment(&mut self, key: Option<ApiKey>, endpoint: Option<String>) {
        self.import_jev_environment(key, endpoint, None, None);
    }

    /// Pair imported credentials with their configured origin without changing saved routing.
    pub fn import_jev_environment(
        &mut self,
        key: Option<ApiKey>,
        endpoint: Option<String>,
        gateway_key: Option<ApiKey>,
        model: Option<String>,
    ) {
        let mut value = if self.live {
            self.preferences()
        } else {
            self.other.clone()
        };
        let key = key.filter(|key| valid_key(key.expose()));
        let gateway_key = gateway_key.filter(|key| valid_key(key.expose()));
        let endpoint = endpoint.filter(|endpoint| valid_endpoint(endpoint));
        let key_endpoint = endpoint
            .as_deref()
            .unwrap_or(crate::jev_plugin::DEFAULT_ENDPOINT);
        if !self.configured {
            let previous_endpoint = value.jev_endpoint.clone();
            if let Some(endpoint) = &endpoint {
                value.jev_endpoint.clone_from(endpoint);
            } else if key.is_none() && gateway_key.is_some() {
                value.jev_endpoint = crate::jev_plugin::GATEWAY_ENDPOINT.into();
            } else {
                value.jev_endpoint = crate::jev_plugin::DEFAULT_ENDPOINT.into();
            }
            if let Some(model) = model.filter(|model| valid_model(model)) {
                value.jev_model = model;
            } else if gateway_label(&value.jev_endpoint) == "Vercel AI Gateway" {
                value.jev_model = crate::jev_plugin::GATEWAY_MODEL.into();
            } else {
                value.jev_model = jev::defaults::MODEL.into();
            }
            if !same_origin(&previous_endpoint, &value.jev_endpoint) {
                value.jev_key = None;
            }
        }
        if value.jev_key.is_none() {
            value.jev_key = if same_origin(&value.jev_endpoint, crate::jev_plugin::GATEWAY_ENDPOINT)
            {
                gateway_key.or_else(|| {
                    same_origin(&value.jev_endpoint, key_endpoint)
                        .then_some(key)
                        .flatten()
                })
            } else if same_origin(&value.jev_endpoint, key_endpoint) {
                key
            } else {
                None
            };
        }
        if self.live {
            self.apply(value);
        } else {
            self.other = value;
        }
    }

    pub fn enabled(&self, id: &str) -> bool {
        match id {
            "boat-cloud" => self.boat.enabled,
            "gce-cloud" => self.gce.enabled,
            "microcoder" => self.microcoder,
            "openagents-cli" => self.cli,
            "acp-subagents" => self.acp,
            "jev" => self.jev_enabled,
            crate::brainstorm::PLUGIN => self.brainstorm.preferences.enabled,
            _ => false,
        }
    }

    pub fn toggle(&mut self, id: &str) -> bool {
        let mut value = self.preferences();
        let flag = match id {
            "boat-cloud" => &mut value.boat.enabled,
            "gce-cloud" => &mut value.gce.enabled,
            "microcoder" => &mut value.microcoder,
            "openagents-cli" => &mut value.cli,
            "acp-subagents" => &mut value.acp,
            "jev" => &mut value.jev_enabled,
            crate::brainstorm::PLUGIN => &mut value.brainstorm.enabled,
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
            crate::brainstorm::PLUGIN => self.brainstorm.status(),
            // No key saved: the built-in decision service answers, needing
            // none, unless it is turned off on this computer.
            "jev" if self.jev_key.is_none() && keyless_jev() => "Built in",
            "jev" if self.jev_key.is_none() => "Setup required",
            "jev" => match self.connection {
                Connection::Checking => "Checking",
                Connection::Verified => "Verified",
                Connection::Failed(_) => "Unavailable",
                Connection::Unchecked => "Configured",
            },
            "acp-subagents" if self.acp_available.is_empty() => "No agents detected",
            "acp-subagents" if self.acp_registered().is_empty() => "All agents off",
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
        self.configured = true;
        true
    }

    pub fn save_brainstorm(&mut self) -> bool {
        let preferences = match self.brainstorm.edited_preferences() {
            Ok(preferences) => preferences,
            Err(error) => {
                self.brainstorm.error = Some(error.into());
                return false;
            }
        };
        let mut value = self.preferences();
        value.brainstorm = preferences;
        if !self.persist(&value) {
            self.brainstorm.error = self.storage_error.clone();
            return false;
        }
        self.apply(value);
        self.brainstorm.begin();
        true
    }

    pub fn cloud_root(&self) -> std::path::PathBuf {
        self.store
            .as_ref()
            .map(|s| s.root().to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from(".coder-state"))
    }
    pub fn cloud(&self, p: coder_cloud::Placement) -> &crate::cloud_settings::Configuration {
        match p {
            coder_cloud::Placement::Boat => &self.boat,
            coder_cloud::Placement::Gce => &self.gce,
        }
    }
    pub fn configure_cloud(
        &mut self,
        p: coder_cloud::Placement,
        input: serde_json::Value,
    ) -> Result<(), String> {
        let config = self.cloud(p).update(input, p)?;
        let mut value = self.preferences();
        match p {
            coder_cloud::Placement::Boat => value.boat = config,
            coder_cloud::Placement::Gce => value.gce = config,
        };
        if !self.persist(&value) {
            return Err(self
                .storage_error
                .clone()
                .unwrap_or_else(|| "Cannot save cloud settings.".into()));
        }
        self.apply(value);
        Ok(())
    }
    pub fn begin_cloud(&mut self, p: coder_cloud::Placement) {
        self.cloud_editor = Some(crate::cloud_settings::Editor::new(p, self.cloud(p).clone()));
    }
    pub fn cloud_key(&mut self, key: KeyEvent) -> bool {
        let Some(editor) = &mut self.cloud_editor else {
            return true;
        };
        if key.code == KeyCode::Enter && editor.focus == 5 {
            let p = editor.placement;
            let result = editor.value();
            match result.and_then(|c| self.configure_cloud(p, serde_json::to_value(c).unwrap())) {
                Ok(()) => {
                    self.cloud_editor = None;
                    return true;
                }
                Err(e) => {
                    if let Some(editor) = &mut self.cloud_editor {
                        editor.error = Some(e);
                    }
                    return false;
                }
            }
        }
        let closed = editor.key(key);
        if closed {
            self.cloud_editor = None;
        }
        closed
    }
    pub fn begin_settings(&mut self) {
        self.discard();
        self.saved_connection = Some(self.connection.clone());
        self.model_draft.text.clone_from(&self.jev_model);
        self.model_draft.cursor = self.model_draft.text.len();
        self.endpoint_draft.text.clone_from(&self.jev_endpoint);
        self.endpoint_draft.cursor = self.endpoint_draft.text.len();
        self.focus = SettingsFocus::ApiKey;
    }

    pub fn discard(&mut self) {
        self.key_draft = Draft::default();
        self.model_draft = Draft::default();
        self.endpoint_draft = Draft::default();
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
        } else if self.origin_changed() && self.jev_key.is_some() {
            "Add a key for the new gateway"
        } else if self.jev_key.is_some() {
            "Key added · paste to replace"
        } else {
            "Add the API key for this gateway"
        }
    }

    pub fn connection_label(&self) -> &str {
        if !self.live {
            return "Demo · no requests sent";
        }
        match &self.connection {
            Connection::Unchecked if self.jev_key.is_none() && keyless_jev() => {
                "Built in · no key needed"
            }
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
        } else if self.remove_key || self.origin_changed() {
            None
        } else {
            self.jev_key.clone()
        }
    }

    pub fn endpoint_for_check(&self) -> Result<String, String> {
        let endpoint = if self.saved_connection.is_some() {
            self.endpoint_draft.text.trim()
        } else {
            &self.jev_endpoint
        };
        if valid_endpoint(endpoint) {
            Ok(endpoint.trim_end_matches('/').into())
        } else {
            Err("Enter an HTTPS API base URL without credentials, query, or fragment.".into())
        }
    }

    pub fn model_for_check(&self) -> Result<String, String> {
        let model = if self.saved_connection.is_some() {
            self.model_draft.text.trim()
        } else {
            &self.jev_model
        };
        let model = if model.is_empty() {
            self.default_model()
        } else {
            model
        };
        if valid_model(model) {
            Ok(model.into())
        } else {
            Err("Enter a valid Jev model ID using at most 128 bytes.".into())
        }
    }

    pub fn gateway_label(&self) -> &'static str {
        gateway_label(if self.saved_connection.is_some() {
            self.endpoint_draft.text.trim()
        } else {
            &self.jev_endpoint
        })
    }

    pub fn default_model(&self) -> &'static str {
        if self.gateway_label() == "Vercel AI Gateway" {
            crate::jev_plugin::GATEWAY_MODEL
        } else {
            jev::defaults::MODEL
        }
    }

    fn origin_changed(&self) -> bool {
        self.saved_connection.is_some()
            && !same_origin(self.endpoint_draft.text.trim(), &self.jev_endpoint)
    }

    fn edited(&mut self) {
        self.connection = Connection::Unchecked;
        self.check_requested = false;
        self.error = None;
    }

    fn endpoint_edited(&mut self, previous: &str) {
        if !same_origin(previous, self.endpoint_draft.text.trim()) {
            self.key_draft = Draft::default();
        }
        self.edited();
    }

    fn cycle_gateway(&mut self) {
        let previous = self.endpoint_draft.text.clone();
        let (endpoint, model) = if self.gateway_label() == "Vercel AI Gateway" {
            (crate::jev_plugin::DEFAULT_ENDPOINT, jev::defaults::MODEL)
        } else {
            (
                crate::jev_plugin::GATEWAY_ENDPOINT,
                crate::jev_plugin::GATEWAY_MODEL,
            )
        };
        self.endpoint_draft.text = endpoint.into();
        self.endpoint_draft.cursor = endpoint.len();
        self.model_draft.text = model.into();
        self.model_draft.cursor = model.len();
        self.endpoint_edited(&previous);
    }

    pub fn endpoint_field(&self) -> (String, usize) {
        (self.endpoint_draft.text.clone(), self.endpoint_draft.cursor)
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
        let previous_endpoint = self.endpoint_draft.text.clone();
        let draft = match self.focus {
            SettingsFocus::ApiKey => &mut self.key_draft,
            SettingsFocus::Model => &mut self.model_draft,
            SettingsFocus::Endpoint => &mut self.endpoint_draft,
            _ => return,
        };
        let previous = draft.text.clone();
        if draft.text.len().saturating_add(text.len()) <= 16 * 1024 {
            draft.insert(&text.trim().replace(['\r', '\n'], ""));
            if draft.text == previous {
                return;
            }
            if self.focus == SettingsFocus::Endpoint {
                self.endpoint_edited(&previous_endpoint);
            } else {
                self.edited();
            }
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> bool {
        self.saved = false;
        self.credential_changed = false;
        self.check_requested = false;
        match key.code {
            KeyCode::Tab | KeyCode::Down | KeyCode::BackTab | KeyCode::Up => {
                let fields = [
                    SettingsFocus::Gateway,
                    SettingsFocus::Endpoint,
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
                SettingsFocus::Gateway => self.cycle_gateway(),
                SettingsFocus::Endpoint => self.focus = SettingsFocus::ApiKey,
                SettingsFocus::ApiKey => self.focus = SettingsFocus::Model,
                SettingsFocus::Model => self.focus = SettingsFocus::Save,
                SettingsFocus::TestKey => self.check_requested = true,
                SettingsFocus::Save => return self.save(),
                SettingsFocus::RemoveKey => {
                    self.key_draft = Draft::default();
                    self.remove_key = true;
                    self.edited();
                }
                SettingsFocus::Cancel => {
                    self.discard();
                    return true;
                }
            },
            KeyCode::Left | KeyCode::Right if self.focus == SettingsFocus::Gateway => {
                self.cycle_gateway();
            }
            _ => {
                let previous_endpoint = self.endpoint_draft.text.clone();
                let draft = match self.focus {
                    SettingsFocus::ApiKey => &mut self.key_draft,
                    SettingsFocus::Model => &mut self.model_draft,
                    SettingsFocus::Endpoint => &mut self.endpoint_draft,
                    _ => return false,
                };
                let previous = draft.text.clone();
                if draft.text.len() < 16 * 1024 || !matches!(key.code, KeyCode::Char(_)) {
                    draft.edit(key);
                }
                if draft.text == previous {
                    return false;
                }
                if self.focus == SettingsFocus::Endpoint {
                    self.endpoint_edited(&previous_endpoint);
                } else {
                    self.edited();
                }
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
        let Ok(endpoint) = self.endpoint_for_check() else {
            self.error =
                Some("Enter an HTTPS API base URL without credentials, query, or fragment.");
            self.focus = SettingsFocus::Endpoint;
            return false;
        };
        let Ok(model) = self.model_for_check() else {
            self.error = Some("Enter a valid Jev model ID using at most 128 bytes.");
            self.focus = SettingsFocus::Model;
            return false;
        };
        let mut value = self.preferences();
        value.jev_model = model;
        value.jev_endpoint = endpoint;
        if self.origin_changed()
            && self.key_draft.text.is_empty()
            && !self.remove_key
            && self.jev_key.is_some()
        {
            self.error = Some("Add an API key for the new gateway or remove the saved key.");
            self.focus = SettingsFocus::ApiKey;
            return false;
        }
        if !self.key_draft.text.is_empty() {
            value.jev_key = Some(ApiKey::new(&self.key_draft.text));
        } else if self.remove_key {
            value.jev_key = None;
        }
        let changed = value.jev_key != self.jev_key
            || value.jev_endpoint != self.jev_endpoint
            || value.jev_model != self.jev_model;
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

    /// Snapshot only executable-discovery inputs, never credentials.
    pub fn discover_acp(&mut self, variable: &dyn Fn(&str) -> Option<OsString>) {
        self.acp_environment = crate::acp_discovery::ENVIRONMENT
            .iter()
            .filter_map(|name| variable(name).map(|value| ((*name).to_owned(), value)))
            .collect();
        self.refresh_acp();
    }

    pub fn begin_acp(&mut self) {
        self.refresh_acp();
    }

    /// Rescan executable metadata while preserving each saved enabled choice.
    pub fn refresh_acp(&mut self) {
        let selected = self
            .acp_choices()
            .get(self.acp_selected)
            .map(|a| a.id.clone());
        let variable = |name: &str| self.acp_environment.get(name).cloned();
        let mut available = BTreeSet::new();
        for mut found in crate::acp_discovery::discover(&variable) {
            if let Some(saved) = self.acp_agents.iter_mut().find(|a| a.id == found.id) {
                found.enabled = saved.enabled;
                *saved = found.clone();
            } else if self.acp_agents.len() < 32 {
                self.acp_agents.push(found.clone());
            } else {
                continue;
            }
            available.insert(found.id);
        }
        for agent in &mut self.acp_agents {
            if !crate::acp_discovery::managed(&agent.id)
                && !available.contains(&agent.id)
                && let Some(program) = crate::acp_discovery::resolve(&agent.program, &variable)
            {
                agent.program = program;
                available.insert(agent.id.clone());
            }
        }
        self.acp_available = available;
        let choices = self.acp_choices();
        self.acp_selected = selected
            .and_then(|selected| choices.iter().position(|a| a.id == selected))
            .unwrap_or_else(|| self.acp_selected.min(choices.len().saturating_sub(1)));
    }

    /// Installed choices shown by the picker, including agents turned off.
    pub fn acp_choices(&self) -> Vec<&AcpAgent> {
        self.acp_agents
            .iter()
            .filter(|agent| self.acp_available.contains(&agent.id))
            .collect()
    }

    /// Only enabled, currently installed agents can be offered to chat.
    pub fn acp_registered(&self) -> Vec<AcpAgent> {
        self.acp_choices()
            .into_iter()
            .filter(|agent| agent.enabled)
            .cloned()
            .collect()
    }

    pub fn select_acp(&mut self, backwards: bool) {
        self.acp_selected = if backwards {
            self.acp_selected.saturating_sub(1)
        } else {
            self.acp_selected
                .saturating_add(1)
                .min(self.acp_choices().len().saturating_sub(1))
        };
    }

    pub fn toggle_acp_agent(&mut self) -> bool {
        let Some(agent) = self.acp_choices().get(self.acp_selected).cloned() else {
            return false;
        };
        let id = agent.id.clone();
        let mut value = self.preferences();
        let Some(agent) = value.acp_agents.iter_mut().find(|agent| agent.id == id) else {
            return false;
        };
        agent.enabled = !agent.enabled;
        if !self.persist(&value) {
            return false;
        }
        self.apply(value);
        true
    }
}

/// Whether Jev answers here with no saved key (the hosted decision
/// service, unless `OPENAGENTS_JEV_HOSTED=off`).
fn keyless_jev() -> bool {
    crate::jev_plugin::keyless_available(&|name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn save(settings: &mut BundledSettings) -> bool {
        settings.focus = SettingsFocus::Save;
        settings.handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
    }

    fn replace(settings: &mut BundledSettings, focus: SettingsFocus, text: &str) {
        settings.focus = focus;
        let draft = match focus {
            SettingsFocus::Endpoint => &mut settings.endpoint_draft,
            SettingsFocus::Model => &mut settings.model_draft,
            _ => unreachable!(),
        };
        *draft = Draft::default();
        settings.paste(text);
    }

    #[test]
    fn jev_without_a_key_is_built_in_and_a_saved_key_shows_its_own_state() {
        if !keyless_jev() {
            // OPENAGENTS_JEV_HOSTED=off on this computer: nothing to show.
            return;
        }
        let mut settings = BundledSettings::default();
        settings.set_live(true);
        assert_eq!(settings.status("jev"), "Built in");
        assert_eq!(settings.connection_label(), "Built in · no key needed");
        settings.set_jev_environment(Some(ApiKey::new("direct-fixture")), None);
        assert_eq!(settings.status("jev"), "Configured");
        assert_eq!(settings.connection_label(), "Not checked");
    }

    #[test]
    fn gateway_changes_require_a_new_key_and_cancel_restores_saved_verification() {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::under(temporary.path());
        let mut settings = BundledSettings::default();
        settings.load(store.clone()).unwrap();
        settings.set_live(true);
        settings.set_jev_environment(Some(ApiKey::new("direct-fixture")), None);
        settings.connection = Connection::Verified;
        settings.begin_settings();
        settings.paste("replacement-direct-fixture");
        settings.focus = SettingsFocus::Gateway;
        settings.handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(settings.gateway_label(), "Vercel AI Gateway");
        assert_eq!(
            settings.endpoint_for_check().unwrap(),
            crate::jev_plugin::GATEWAY_ENDPOINT
        );
        assert_eq!(
            settings.model_for_check().unwrap(),
            crate::jev_plugin::GATEWAY_MODEL
        );
        assert!(settings.field(true).0.is_empty());
        assert!(settings.key_for_check().is_none());
        assert!(!save(&mut settings));
        assert!(matches!(settings.connection, Connection::Unchecked));
        assert!(!temporary.path().join(FILE).exists());
        settings.discard();
        assert!(matches!(settings.connection, Connection::Verified));
        assert_eq!(settings.jev_endpoint(), crate::jev_plugin::DEFAULT_ENDPOINT);
        assert_eq!(settings.key_for_check().unwrap().expose(), "direct-fixture");

        settings.begin_settings();
        settings.focus = SettingsFocus::Gateway;
        settings.handle(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        settings.focus = SettingsFocus::ApiKey;
        settings.paste("gateway-fixture");
        assert_eq!(
            settings.key_for_check().unwrap().expose(),
            "gateway-fixture"
        );
        assert!(save(&mut settings));
        assert!(settings.credential_changed);
        assert_eq!(
            settings.endpoint_for_check().unwrap(),
            crate::jev_plugin::GATEWAY_ENDPOINT
        );
        assert_eq!(
            settings.model_for_check().unwrap(),
            crate::jev_plugin::GATEWAY_MODEL
        );
        let mut restored = BundledSettings::default();
        restored.set_live(true);
        restored.load(store).unwrap();
        assert_eq!(restored.jev_key().unwrap().expose(), "gateway-fixture");
        assert_eq!(restored.gateway_label(), "Vercel AI Gateway");
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temporary.path().join(FILE)).unwrap()).unwrap();
        assert_eq!(document["version"], 1);
    }

    #[test]
    fn endpoint_and_model_edits_invalidate_checks_without_changing_saved_settings() {
        let mut settings = BundledSettings::default();
        settings.set_live(true);
        settings.set_jev_environment(Some(ApiKey::new("direct-fixture")), None);
        settings.connection = Connection::Verified;
        settings.begin_settings();
        replace(
            &mut settings,
            SettingsFocus::Endpoint,
            "https://api.typesafe.ai/custom",
        );
        assert_eq!(settings.gateway_label(), "Custom gateway");
        assert!(matches!(settings.connection, Connection::Unchecked));
        assert_eq!(settings.key_for_check().unwrap().expose(), "direct-fixture");
        assert_eq!(settings.jev_endpoint(), crate::jev_plugin::DEFAULT_ENDPOINT);
        settings.connection = Connection::Verified;
        replace(&mut settings, SettingsFocus::Model, "jev-fixture");
        assert!(matches!(settings.connection, Connection::Unchecked));
        assert_eq!(settings.model_for_check().unwrap(), "jev-fixture");
        assert_eq!(settings.jev_model(), jev::defaults::MODEL);
        settings.connection = Connection::Verified;
        settings.handle(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert!(matches!(settings.connection, Connection::Verified));
        assert!(save(&mut settings));
        assert!(settings.credential_changed);
        assert_eq!(settings.jev_endpoint(), "https://api.typesafe.ai/custom");
        assert_eq!(settings.jev_model(), "jev-fixture");
    }

    #[test]
    fn invalid_endpoint_drafts_do_not_save_or_disclose_saved_credentials() {
        let mut settings = BundledSettings::default();
        settings.set_live(true);
        settings.set_jev_environment(Some(ApiKey::new("direct-fixture")), None);
        settings.begin_settings();
        for endpoint in [
            "http://example.invalid",
            "https://key@example.invalid",
            "https://example.invalid?key=fixture",
            "https://example.invalid#fragment",
            "",
            "not-a-url",
        ] {
            replace(&mut settings, SettingsFocus::Endpoint, endpoint);
            assert!(settings.endpoint_for_check().is_err());
            assert!(settings.key_for_check().is_none());
            assert!(!save(&mut settings));
            assert_eq!(settings.jev_endpoint(), crate::jev_plugin::DEFAULT_ENDPOINT);
        }
        assert!(valid_endpoint("http://127.0.0.1:9090/typesafe"));
        assert!(valid_endpoint("http://[::1]:9090/typesafe"));
    }

    #[test]
    fn startup_credentials_are_paired_with_their_origin_and_saved_routing_wins() {
        let direct = || Some(ApiKey::new("direct-fixture"));
        let gateway = || Some(ApiKey::new("gateway-fixture"));
        let mut auto = BundledSettings::default();
        auto.import_jev_environment(None, None, gateway(), None);
        auto.set_live(true);
        assert_eq!(auto.gateway_label(), "Vercel AI Gateway");
        assert_eq!(auto.jev_model(), crate::jev_plugin::GATEWAY_MODEL);
        assert_eq!(auto.jev_key().unwrap().expose(), "gateway-fixture");
        auto.import_jev_environment(direct(), None, None, None);
        assert_eq!(auto.gateway_label(), "TypeSafe direct");
        assert_eq!(auto.jev_key().unwrap().expose(), "direct-fixture");
        auto.import_jev_environment(None, None, gateway(), None);
        assert_eq!(auto.gateway_label(), "Vercel AI Gateway");
        assert_eq!(auto.jev_key().unwrap().expose(), "gateway-fixture");

        for endpoint in [None, Some("https://api.typesafe.ai".into())] {
            let mut settings = BundledSettings::default();
            settings.import_jev_environment(
                direct(),
                endpoint,
                gateway(),
                Some("jev-fixture".into()),
            );
            settings.set_live(true);
            assert_eq!(settings.jev_key().unwrap().expose(), "direct-fixture");
            assert_eq!(settings.jev_model(), "jev-fixture");
        }

        let temporary = tempfile::tempdir().unwrap();
        let store = Store::under(temporary.path());
        let custom = Preferences {
            jev_endpoint: "https://custom.invalid/typesafe".into(),
            jev_model: "custom-jev".into(),
            ..Preferences::default()
        };
        store.save_extra(FILE, &custom).unwrap();
        let mut saved = BundledSettings::default();
        saved.load(store.clone()).unwrap();
        saved.import_jev_environment(direct(), None, gateway(), Some("env-model".into()));
        saved.set_live(true);
        assert!(saved.jev_key().is_none());
        assert_eq!(saved.jev_model(), "custom-jev");
        assert_eq!(saved.jev_endpoint(), "https://custom.invalid/typesafe");
        saved.import_jev_environment(
            direct(),
            Some("https://custom.invalid".into()),
            gateway(),
            None,
        );
        assert_eq!(saved.jev_key().unwrap().expose(), "direct-fixture");
        assert_eq!(
            store
                .read_extra::<Preferences>(FILE)
                .unwrap()
                .unwrap()
                .jev_key,
            None
        );
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
        settings.focus = SettingsFocus::Gateway;
        settings.handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        settings.focus = SettingsFocus::ApiKey;
        settings.paste("fixture-key");
        assert!(save(&mut settings));
        assert_eq!(settings.jev_endpoint(), crate::jev_plugin::GATEWAY_ENDPOINT);
        assert!(settings.jev_key().is_none());
        assert!(!temporary.path().join(FILE).exists());
        settings.set_live(true);
        assert!(settings.microcoder && settings.cli && settings.acp && settings.jev_enabled);
        assert!(settings.jev_key().is_none());
        assert_eq!(settings.jev_endpoint(), crate::jev_plugin::DEFAULT_ENDPOINT);
        settings.set_live(false);
        assert_eq!(settings.jev_endpoint(), crate::jev_plugin::GATEWAY_ENDPOINT);
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
    fn saved_acp_definitions_accept_explicit_programs_and_require_unique_ids() {
        let agent = serde_json::json!({"id":"reviewer","name":"Reviewer","program":"fixture-agent","arguments":["--acp"],"enabled":true});
        let agent: AcpAgent = serde_json::from_value(agent).unwrap();
        assert!(
            valid_agents(&[agent.clone(), agent.clone()])
                .unwrap_err()
                .contains("unique")
        );
        assert!(valid_agents(&[agent]).is_ok());
    }
}

impl BundledSettings {
    pub(crate) fn demo_view(&self) -> coder_ui::demo::bundled_settings::BundledSettings {
        let mut value = coder_ui::demo::bundled_settings::BundledSettings::default();
        value.brainstorm = self.brainstorm.demo_view();
        value.boat = crate::demo::cloud(&self.boat);
        value.gce = crate::demo::cloud(&self.gce);
        value.cloud_editor = self.cloud_editor.as_ref().map(crate::demo::editor);
        value.microcoder = self.microcoder;
        value.cli = self.cli;
        value.acp = self.acp;
        value.jev_enabled = self.jev_enabled;
        value.acp_agents = self
            .acp_agents
            .iter()
            .map(|a| coder_ui::demo::bundled_settings::AcpAgent {
                id: a.id.clone(),
                name: a.name.clone(),
                program: a.program.clone(),
                enabled: a.enabled,
            })
            .collect();
        value.acp_selected = self.acp_selected;
        value.acp_available = self.acp_available.clone();
        value.focus = crate::demo::focus(self.focus);
        value.error = self.error;
        value.storage_error = self.storage_error.clone();
        value.connection = crate::demo::connection(&self.connection);
        value.check_requested = self.check_requested;
        value.saved = self.saved;
        value.credential_changed = self.credential_changed;
        value.jev_key = self.jev_key.as_ref().map(|_| ());
        value.jev_model = self.jev_model.clone();
        value.jev_endpoint = self.jev_endpoint.clone();
        value.key_draft = crate::demo::draft(&self.key_draft);
        value.model_draft = crate::demo::draft(&self.model_draft);
        value.endpoint_draft = crate::demo::draft(&self.endpoint_draft);
        value.remove_key = self.remove_key;
        value.live = self.live;
        value.saved_connection = self.saved_connection.as_ref().map(crate::demo::connection);
        value
    }
}
