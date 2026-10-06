//! Local plugin preferences and a settings draft. No credentials are persisted.

use crossterm::event::{KeyCode, KeyEvent};
use unicode_segmentation::UnicodeSegmentation;

use crate::Draft;

pub const ENDPOINT: &str = "https://openrouter.ai/api/v1";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsFocus {
    #[default]
    ApiKey,
    Model,
    Save,
    RemoveKey,
    Cancel,
}

impl SettingsFocus {
    fn next(self, backwards: bool) -> Self {
        let fields = [
            Self::ApiKey,
            Self::Model,
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
}

impl Plugins {
    pub fn status(&self) -> &'static str {
        match (self.enabled, self.key_configured) {
            (false, _) => "Disabled",
            (true, false) => "Setup required",
            (true, true) => "Configured",
        }
    }

    pub fn begin_settings(&mut self) {
        self.discard_draft();
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
        match key.code {
            KeyCode::Tab | KeyCode::Down => self.focus = self.focus.next(false),
            KeyCode::BackTab | KeyCode::Up => self.focus = self.focus.next(true),
            KeyCode::Esc => {
                self.discard_draft();
                return true;
            }
            KeyCode::Enter => match self.focus {
                SettingsFocus::ApiKey | SettingsFocus::Model => self.focus = self.focus.next(false),
                SettingsFocus::Save => return self.save(),
                SettingsFocus::RemoveKey => {
                    self.key_draft = Draft::default();
                    self.remove_key = true;
                    self.error = None;
                }
                SettingsFocus::Cancel => {
                    self.discard_draft();
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
        // The mock retains only configuration state, never the entered credential.
        self.key_configured =
            !self.key_draft.text.is_empty() || (self.key_configured && !self.remove_key);
        self.model = self.model_draft.text.trim().into();
        self.discard_draft();
        true
    }
}
