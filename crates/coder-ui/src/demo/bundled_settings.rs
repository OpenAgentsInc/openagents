//! Original local bundled-plugin editors, without discovery or credentials.
use super::{
    Draft, Key, KeyCode,
    cloud_settings::{Configuration, Placement},
    plugins::{Connection, SettingsFocus},
};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;
pub const DEFAULT_ENDPOINT: &str = "https://api.typesafe.ai";
pub const GATEWAY_ENDPOINT: &str = "https://ai-gateway.vercel.sh/typesafe";
pub const GATEWAY_MODEL: &str = "typesafe-ai/jev";
pub const DEFAULT_MODEL: &str = "jev-latest";
#[derive(Clone, Serialize, Deserialize)]
pub struct AcpAgent {
    pub id: String,
    pub name: String,
    pub program: std::path::PathBuf,
    pub enabled: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct BundledSettings {
    pub brainstorm: super::brainstorm::Settings,
    pub boat: Configuration,
    pub gce: Configuration,
    pub cloud_editor: Option<super::cloud_settings::Editor>,
    pub microcoder: bool,
    pub cli: bool,
    pub acp: bool,
    pub jev_enabled: bool,
    pub acp_agents: Vec<AcpAgent>,
    pub acp_selected: usize,
    pub acp_available: std::collections::BTreeSet<String>,
    pub focus: SettingsFocus,
    #[serde(skip)]
    pub error: Option<&'static str>,
    pub storage_error: Option<String>,
    pub connection: Connection,
    pub check_requested: bool,
    pub saved: bool,
    pub credential_changed: bool,
    pub jev_key: Option<()>,
    pub jev_model: String,
    pub jev_endpoint: String,
    #[serde(skip)]
    pub key_draft: Draft,
    pub model_draft: Draft,
    pub endpoint_draft: Draft,
    pub remove_key: bool,
    pub live: bool,
    pub saved_connection: Option<Connection>,
}
impl Default for BundledSettings {
    fn default() -> Self {
        Self {
            brainstorm: Default::default(),
            boat: Default::default(),
            gce: Configuration::gce(),
            cloud_editor: None,
            microcoder: true,
            cli: true,
            acp: true,
            jev_enabled: true,
            acp_agents: vec![],
            acp_selected: 0,
            acp_available: Default::default(),
            focus: SettingsFocus::ApiKey,
            error: None,
            storage_error: None,
            connection: Default::default(),
            check_requested: false,
            saved: false,
            credential_changed: false,
            jev_key: None,
            jev_model: DEFAULT_MODEL.into(),
            jev_endpoint: DEFAULT_ENDPOINT.into(),
            key_draft: Default::default(),
            model_draft: Default::default(),
            endpoint_draft: Default::default(),
            remove_key: false,
            live: false,
            saved_connection: None,
        }
    }
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
    let Ok(url) = url::Url::parse(endpoint) else {
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
    match (url::Url::parse(left), url::Url::parse(right)) {
        (Ok(left), Ok(right)) => left.origin() == right.origin(),
        _ => false,
    }
}

fn gateway_label(endpoint: &str) -> &'static str {
    match endpoint.trim_end_matches('/') {
        DEFAULT_ENDPOINT => "TypeSafe direct",
        GATEWAY_ENDPOINT => "Vercel AI Gateway",
        _ => "Custom gateway",
    }
}

impl BundledSettings {
    pub fn toggle(&mut self, id: &str) -> bool {
        let flag = match id {
            "boat-cloud" => &mut self.boat.enabled,
            "gce-cloud" => &mut self.gce.enabled,
            "microcoder" => &mut self.microcoder,
            "openagents-cli" => &mut self.cli,
            "acp-subagents" => &mut self.acp,
            "jev" => &mut self.jev_enabled,
            "brainstorm" => &mut self.brainstorm.preferences.enabled,
            _ => return false,
        };
        *flag = !*flag;
        true
    }
    pub fn acp_choices(&self) -> Vec<&AcpAgent> {
        self.acp_agents
            .iter()
            .filter(|a| self.acp_available.contains(&a.id))
            .collect()
    }
    pub fn acp_registered(&self) -> Vec<&AcpAgent> {
        self.acp_choices()
            .into_iter()
            .filter(|a| a.enabled)
            .collect()
    }
    pub fn begin_acp(&mut self) {}
    pub fn refresh_acp(&mut self) {}
    pub fn select_acp(&mut self, backwards: bool) {
        self.acp_selected = if backwards {
            self.acp_selected.saturating_sub(1)
        } else {
            (self.acp_selected + 1).min(self.acp_choices().len().saturating_sub(1))
        };
    }
    pub fn toggle_acp_agent(&mut self) -> bool {
        if let Some(id) = self
            .acp_choices()
            .get(self.acp_selected)
            .map(|a| a.id.clone())
            && let Some(a) = self.acp_agents.iter_mut().find(|a| a.id == id)
        {
            a.enabled = !a.enabled;
            true
        } else {
            false
        }
    }
    pub fn configure_cloud(
        &mut self,
        p: Placement,
        input: serde_json::Value,
    ) -> Result<(), String> {
        let c = self.cloud(p).update(input, p)?;
        match p {
            Placement::Boat => self.boat = c,
            Placement::Gce => self.gce = c,
        };
        Ok(())
    }
    pub fn save_brainstorm(&mut self) -> bool {
        match self.brainstorm.edited_preferences() {
            Ok(p) => {
                self.brainstorm.preferences = p;
                self.brainstorm.connection = Connection::Unchecked;
                self.brainstorm.discovery = None;
                self.brainstorm.fixture = false;
                self.brainstorm.begin();
                true
            }
            Err(e) => {
                self.brainstorm.error = Some(e.into());
                false
            }
        }
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
        if self.origin_changed()
            && self.key_draft.text.is_empty()
            && !self.remove_key
            && self.jev_key.is_some()
        {
            self.error = Some("Add a key for the new gateway or remove the saved key.");
            self.focus = SettingsFocus::ApiKey;
            return false;
        }
        let key = if !self.key_draft.text.is_empty() {
            Some(())
        } else if self.remove_key {
            None
        } else {
            self.jev_key
        };
        let changed =
            key != self.jev_key || endpoint != self.jev_endpoint || model != self.jev_model;
        self.jev_key = key;
        self.jev_endpoint = endpoint;
        self.jev_model = model;
        self.discard();
        self.credential_changed = changed;
        self.saved = true;
        if changed {
            self.connection = Connection::Unchecked;
        }
        true
    }
    pub fn enabled(&self, id: &str) -> bool {
        match id {
            "boat-cloud" => self.boat.enabled,
            "gce-cloud" => self.gce.enabled,
            "microcoder" => self.microcoder,
            "openagents-cli" => self.cli,
            "acp-subagents" => self.acp,
            "jev" => self.jev_enabled,
            super::brainstorm::PLUGIN => self.brainstorm.preferences.enabled,
            _ => false,
        }
    }
    pub fn status(&self, id: &str) -> &str {
        if !self.enabled(id) {
            return "Disabled";
        }
        match id {
            super::brainstorm::PLUGIN => self.brainstorm.status(),
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
    pub fn jev_endpoint(&self) -> &str {
        &self.jev_endpoint
    }
    pub fn jev_model(&self) -> &str {
        &self.jev_model
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
        self.key_draft.erase();
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
            Connection::Unchecked => "Not checked",
            Connection::Checking => "Checking Jev API key…",
            Connection::Verified => "Jev API key verified",
            Connection::Failed(error) => error,
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
            GATEWAY_MODEL
        } else {
            DEFAULT_MODEL
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
            self.key_draft.erase();
        }
        self.edited();
    }
    fn cycle_gateway(&mut self) {
        let previous = self.endpoint_draft.text.clone();
        let (endpoint, model) = if self.gateway_label() == "Vercel AI Gateway" {
            (DEFAULT_ENDPOINT, DEFAULT_MODEL)
        } else {
            (GATEWAY_ENDPOINT, GATEWAY_MODEL)
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
    pub fn handle(&mut self, key: Key) -> bool {
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
                    self.key_draft.erase();
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
    pub fn begin_cloud(&mut self, p: super::cloud_settings::Placement) {
        self.cloud_editor = Some(super::cloud_settings::Editor::new(p, self.cloud(p).clone()));
    }
    pub fn cloud_key(&mut self, key: Key) -> bool {
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
    pub fn cloud(
        &self,
        p: super::cloud_settings::Placement,
    ) -> &super::cloud_settings::Configuration {
        match p {
            super::cloud_settings::Placement::Boat => &self.boat,
            super::cloud_settings::Placement::Gce => &self.gce,
        }
    }
}

impl Drop for BundledSettings {
    fn drop(&mut self) {
        self.key_draft.erase();
    }
}
