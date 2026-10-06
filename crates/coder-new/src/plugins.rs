//! Plugin preferences and masked credential editing. Keys stay in memory.

use crossterm::event::{KeyCode, KeyEvent};
use unicode_segmentation::UnicodeSegmentation;

use crate::Draft;

pub const ENDPOINT: &str = "https://openrouter.ai/api/v1";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsFocus {
    #[default]
    ApiKey,
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
        let index = fields.iter().position(|field| *field == self).unwrap();
        fields[(index + if backwards { fields.len() - 1 } else { 1 }) % fields.len()]
    }
}

#[derive(Default)]
pub struct Plugins {
    pub enabled: bool,
    pub key_configured: bool,
    pub model: String,
    pub focus: SettingsFocus,
    key_draft: Draft,
    model_draft: Draft,
    remove_key: bool,
    pub error: Option<&'static str>,
    pub connection: Connection,
    pub check_requested: bool,
    pub saved: bool,
    pub credential_changed: bool,
    live: bool,
    live_key: Option<model_access::ApiKey>,
    other_preferences: Preferences,
    saved_connection: Option<Connection>,
}

#[derive(Default)]
struct Preferences {
    enabled: bool,
    key_configured: bool,
    model: String,
}

#[derive(Clone, Default)]
pub enum Connection {
    #[default]
    Unchecked,
    Checking,
    Verified,
    Failed(String),
}

impl Plugins {
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
        self.live = live;
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
        self.model = self.model_draft.text.trim().into();
        self.discard_draft();
        self.saved = true;
        self.saved_connection = None;
        true
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
