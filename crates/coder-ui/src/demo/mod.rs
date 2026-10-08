//! The original Coder demo as page-local presentation state.
//!
//! This controller has no filesystem, provider, host, or execution adapter.
//! Demo inputs affect only the six synthetic conversations and settings.

pub mod agents;
pub mod brainstorm;
pub mod bundled_settings;
pub mod cloud_settings;
mod draft;
pub mod models;
pub mod onboarding;
pub mod plugin_definition;
pub mod plugins;
pub mod slash;
pub mod tools;
mod trajectory;

pub use draft::Draft;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use unicode_segmentation::UnicodeSegmentation;

pub const MAX_DRAFT_BYTES: usize = 64 * 1024;
pub const MAX_CONVERSATION_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Up,
    Down,
    PageUp,
    PageDown,
    F(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub code: KeyCode,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub super_key: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub release: bool,
}
impl Key {
    pub fn new(code: KeyCode) -> Self {
        Self {
            code,
            ctrl: false,
            alt: false,
            super_key: false,
            shift: false,
            release: false,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Screen {
    #[default]
    Conversation,
    Plugins,
    PluginSettings,
}
#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Demo,
    Live,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Chat {
    pub draft: Draft,
    pub messages: Vec<String>,
    pub scroll: u16,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DemoState {
    pub mode: Mode,
    pub screen: Screen,
    pub draft: Draft,
    pub messages: Vec<String>,
    pub scroll: u16,
    pub selected_agent: Option<usize>,
    pub onboarding: bool,
    pub animation_frame: u8,
    pub cursor_blink_frame: u8,
    pub elapsed_seconds: u64,
    pub plugins: plugins::Plugins,
    pub cwd: Option<PathBuf>,
    pub branch: Option<String>,
    pub model_picker: Option<models::Picker>,
    pub slash_selected: usize,
    pub slash_hidden: bool,
    pub notice: Option<String>,
    pub saved_chats: [Chat; 6],
    pub other_draft: Draft,
    pub return_screen: Screen,
    #[serde(skip)]
    download: Option<Download>,
    #[serde(skip)]
    other_plugins: Option<plugins::Plugins>,
}
impl Default for DemoState {
    fn default() -> Self {
        Self {
            mode: Mode::Demo,
            screen: Screen::Conversation,
            draft: Default::default(),
            messages: vec![],
            scroll: 0,
            selected_agent: None,
            onboarding: false,
            animation_frame: 0,
            cursor_blink_frame: 0,
            elapsed_seconds: 0,
            plugins: Default::default(),
            cwd: None,
            branch: None,
            model_picker: None,
            slash_selected: 0,
            slash_hidden: false,
            notice: None,
            saved_chats: std::array::from_fn(|_| Chat::default()),
            other_draft: Default::default(),
            return_screen: Screen::Conversation,
            download: None,
            other_plugins: None,
        }
    }
}

/// An explicit browser download, containing only synthetic displayed records.
pub struct Download {
    pub filename: String,
    pub bytes: Vec<u8>,
}
impl Clone for Download {
    fn clone(&self) -> Self {
        Self {
            filename: self.filename.clone(),
            bytes: self.bytes.clone(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Input {
    pub label: &'static str,
    pub secret: bool,
}

impl DemoState {
    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1) % 8;
        self.cursor_blink_frame = self.cursor_blink_frame.wrapping_add(1) % 8;
    }
    pub fn slash_hints(&self) -> Vec<slash::Command> {
        if self.slash_hidden || self.model_picker.is_some() || self.screen != Screen::Conversation {
            return vec![];
        }
        slash::matches(&self.draft.text)
            .into_iter()
            .filter(|c| *c != slash::Command::Models || self.plugins.enabled)
            .filter(|c| {
                *c != slash::Command::Brainstorm
                    || self.plugins.bundled.brainstorm.preferences.enabled
            })
            .collect()
    }
    pub fn select_agent(&mut self, selected: Option<usize>) {
        self.select_conversation(selected, false);
    }
    pub fn select_onboarding(&mut self) {
        self.select_conversation(None, true);
    }
    fn select_conversation(&mut self, selected: Option<usize>, onboarding: bool) {
        if selected.is_some_and(|i| i >= agents::DEMOS.len()) {
            return;
        }
        self.screen = Screen::Conversation;
        self.model_picker = None;
        if self.selected_agent == selected && self.onboarding == onboarding {
            return;
        }
        let previous = if self.onboarding {
            5
        } else {
            self.selected_agent.map_or(0, |i| i + 1)
        };
        self.saved_chats[previous] = Chat {
            draft: std::mem::take(&mut self.draft),
            messages: std::mem::take(&mut self.messages),
            scroll: self.scroll,
        };
        let next = if onboarding {
            5
        } else {
            selected.map_or(0, |i| i + 1)
        };
        let chat = std::mem::take(&mut self.saved_chats[next]);
        self.draft = chat.draft;
        self.messages = chat.messages;
        self.scroll = chat.scroll;
        self.selected_agent = selected;
        self.onboarding = onboarding;
        self.screen = Screen::Conversation;
    }
    pub fn open_plugins(&mut self) {
        if !matches!(self.screen, Screen::Plugins | Screen::PluginSettings) {
            self.return_screen = self.screen;
        }
        self.screen = Screen::Plugins;
    }
    pub fn open_models(&mut self) {
        if !self.plugins.enabled {
            self.notice = Some("Turn on OpenRouter BYOK in /plugins to choose a model.".into());
            return;
        }
        self.model_picker = Some(models::Picker::new(
            models::openrouter_catalog(),
            models::OPENROUTER_PLUGIN,
            &self.plugins.model,
            self.plugins.options.clone(),
            false,
        ));
    }
    pub fn open_plugin_settings(&mut self) {
        self.open_plugins();
        match self.plugins.selected_definition().id {
            "openrouter-byok" => self.plugins.begin_settings(),
            "jev" => self.plugins.bundled.begin_settings(),
            "boat-cloud" => self
                .plugins
                .bundled
                .begin_cloud(cloud_settings::Placement::Boat),
            "gce-cloud" => self
                .plugins
                .bundled
                .begin_cloud(cloud_settings::Placement::Gce),
            "acp-subagents" => self.plugins.bundled.begin_acp(),
            "brainstorm" => self.plugins.bundled.brainstorm.begin(),
            _ => {}
        }
        self.screen = Screen::PluginSettings;
    }
    pub fn wheel(&mut self, delta: i16) {
        if self.screen == Screen::Conversation && self.model_picker.is_none() {
            self.scroll = self.scroll.saturating_add_signed(delta);
        }
    }
    pub fn paste(&mut self, text: &str) {
        self.cursor_blink_frame = 0;
        if let Some(picker) = &mut self.model_picker {
            if picker.query.text.len().saturating_add(text.len()) <= MAX_DRAFT_BYTES {
                picker.paste(text);
            }
            return;
        }
        if self.screen == Screen::PluginSettings {
            match self.plugins.selected_definition().id {
                "boat-cloud" | "gce-cloud" => {
                    if let Some(e) = &mut self.plugins.bundled.cloud_editor {
                        if e.template
                            .text
                            .len()
                            .saturating_add(e.credentials.text.len())
                            .saturating_add(e.paths.text.len())
                            .saturating_add(text.len())
                            <= MAX_DRAFT_BYTES
                        {
                            e.paste(text)
                        }
                    }
                }
                "brainstorm" => self.plugins.bundled.brainstorm.paste(text),
                "jev" => self.plugins.bundled.paste(text),
                "openrouter-byok" => {
                    if self
                        .plugins
                        .key_draft
                        .text
                        .len()
                        .saturating_add(self.plugins.model_draft.text.len())
                        .saturating_add(text.len())
                        <= MAX_DRAFT_BYTES
                    {
                        self.plugins.paste(text)
                    }
                }
                _ => {}
            }
            return;
        }
        if self.screen != Screen::Plugins {
            if self.draft.text.len().saturating_add(text.len()) > MAX_DRAFT_BYTES {
                self.notice = Some("The demo draft exceeds 64 KiB.".into());
                return;
            }
            self.draft.insert(text);
            self.slash_selected = 0;
            self.slash_hidden = false;
        }
    }
    pub fn key(&mut self, key: Key) -> bool {
        if key.release {
            return true;
        }
        self.cursor_blink_frame = 0;
        if key.ctrl && key.code == KeyCode::Char('c') {
            return false;
        }
        if let KeyCode::Char(ch) = key.code
            && !key.ctrl
            && !key.alt
            && self.active_draft_mut().is_some_and(|draft| {
                draft.text.len().saturating_add(ch.len_utf8()) > MAX_DRAFT_BYTES
            })
        {
            self.notice = Some("The demo editor exceeds 64 KiB.".into());
            return true;
        }
        if matches!(key.code, KeyCode::Char('p' | 'P'))
            && (key.super_key != key.ctrl)
            && !key.alt
            && !key.shift
        {
            self.model_picker = None;
            self.open_plugins();
            return true;
        }
        if let Some(picker) = &mut self.model_picker {
            match picker.handle(key) {
                models::Action::Continue => {}
                models::Action::Close => self.model_picker = None,
                models::Action::Save(model, options) => {
                    if self.plugins.set_model(&model, options) {
                        self.model_picker = None
                    }
                }
            }
            return true;
        }
        if self.screen == Screen::PluginSettings {
            let closed = match self.plugins.selected_definition().id {
                "boat-cloud" | "gce-cloud" => self.plugins.bundled.cloud_key(key),
                "brainstorm" => {
                    let closed = self.plugins.bundled.brainstorm.handle(key);
                    if self.plugins.bundled.brainstorm.check_requested
                        && self.plugins.bundled.brainstorm.preferences.enabled
                    {
                        self.plugins.bundled.brainstorm.fixture = true;
                    }
                    if self.plugins.bundled.brainstorm.save_requested {
                        self.plugins.bundled.save_brainstorm()
                    } else {
                        closed
                    }
                }
                "jev" => self.plugins.bundled.handle(key),
                "acp-subagents" => {
                    match key.code {
                        KeyCode::Esc => {
                            self.screen = Screen::Plugins;
                        }
                        KeyCode::Up => self.plugins.bundled.select_acp(true),
                        KeyCode::Down => self.plugins.bundled.select_acp(false),
                        KeyCode::Home => self.plugins.bundled.acp_selected = 0,
                        KeyCode::End => {
                            self.plugins.bundled.acp_selected =
                                self.plugins.bundled.acp_choices().len().saturating_sub(1)
                        }
                        KeyCode::Char('r') => self.plugins.bundled.refresh_acp(),
                        KeyCode::Char(' ') | KeyCode::Enter => {
                            self.plugins.bundled.toggle_acp_agent();
                        }
                        _ => {}
                    }
                    false
                }
                "openrouter-byok" => self.plugins.handle(key),
                _ => matches!(key.code, KeyCode::Esc | KeyCode::Enter),
            };
            if closed {
                self.screen = Screen::Plugins;
            }
            return true;
        }
        if self.screen == Screen::Plugins {
            match key.code {
                KeyCode::Up => self.plugins.select(true),
                KeyCode::Down => self.plugins.select(false),
                KeyCode::Char(' ') => {
                    self.plugins.toggle_selected();
                }
                KeyCode::Enter => self.open_plugin_settings(),
                KeyCode::Esc | KeyCode::F(2) => self.screen = self.return_screen,
                _ => {}
            }
            return true;
        }
        let hints = self.slash_hints();
        if !hints.is_empty() {
            match key.code {
                KeyCode::Down => {
                    self.slash_selected = (self.slash_selected + 1).min(hints.len() - 1);
                    return true;
                }
                KeyCode::Up => {
                    self.slash_selected = self.slash_selected.saturating_sub(1);
                    return true;
                }
                KeyCode::Tab => {
                    self.draft.text = format!(
                        "/{}",
                        hints[self.slash_selected.min(hints.len() - 1)].word()
                    );
                    self.draft.cursor = self.draft.text.len();
                    self.slash_selected = 0;
                    return true;
                }
                KeyCode::Esc => {
                    self.slash_hidden = true;
                    return true;
                }
                KeyCode::Enter if !key.ctrl && !key.alt => {
                    self.command(hints[self.slash_selected.min(hints.len() - 1)]);
                    return true;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::F(2) => self.open_plugins(),
            KeyCode::Down if self.mode == Mode::Demo => self.select_agent(Some(
                self.selected_agent
                    .map_or(0, |i| (i + 1).min(agents::DEMOS.len() - 1)),
            )),
            KeyCode::Up if self.mode == Mode::Demo => {
                self.select_agent(self.selected_agent.and_then(|i| i.checked_sub(1)))
            }
            KeyCode::Esc => self.select_agent(None),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(5),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(5),
            KeyCode::Enter if key.alt => self.draft.insert("\n"),
            KeyCode::Enter if !key.ctrl => self.submit(),
            _ => {
                if self.draft.text.len() < MAX_DRAFT_BYTES || !matches!(key.code, KeyCode::Char(_))
                {
                    self.draft.edit(key);
                }
                self.slash_selected = 0;
                self.slash_hidden = false;
            }
        }
        true
    }
    fn submit(&mut self) {
        let text = self.draft.text.trim();
        if text.starts_with("/resume ") || text == "/resume" {
            self.notice = Some("Conversation storage is unavailable.".into());
            return;
        }
        if text.starts_with("/export ") {
            self.export();
            self.draft = Default::default();
            return;
        }
        if let Some(command) = brainstorm::parse(text) {
            match command {
                Err(error) => self.notice = Some(error.into()),
                Ok(_) if !self.plugins.bundled.brainstorm.preferences.enabled => {
                    self.notice =
                        Some("Enable Brainstorm in /plugins before a public lookup.".into())
                }
                Ok(_) => {
                    self.messages.push(std::mem::take(&mut self.draft.text));
                    self.messages.push(brainstorm::FIXTURE.into());
                    self.draft.cursor = 0;
                }
            }
            return;
        }
        if let Some(command) = slash::parse(text) {
            self.command(command);
            return;
        }
        if slash::is_command_word(text) {
            self.notice = Some("Unknown command. Type / to see available commands.".into());
            self.draft = Default::default();
            return;
        }
        if text.is_empty() {
            return;
        }
        if self.mode == Mode::Live {
            self.notice = Some("Live execution is unavailable in this browser demo.".into());
            return;
        }
        if self
            .messages
            .iter()
            .map(String::len)
            .sum::<usize>()
            .saturating_add(self.draft.text.len())
            > MAX_CONVERSATION_BYTES
        {
            self.notice = Some(
                "This demo conversation exceeds 512 KiB. Start a fresh demo to continue.".into(),
            );
            return;
        }
        self.messages.push(std::mem::take(&mut self.draft.text));
        self.draft.cursor = 0;
        self.screen = Screen::Conversation;
        self.scroll = u16::MAX;
    }
    fn command(&mut self, command: slash::Command) {
        if command == slash::Command::Resume {
            self.notice = Some("Conversation storage is unavailable.".into());
            return;
        }
        self.draft = Default::default();
        self.slash_selected = 0;
        self.slash_hidden = false;
        self.notice = None;
        match command {
            slash::Command::Demo => self.set_mode(if self.mode == Mode::Demo {
                Mode::Live
            } else {
                Mode::Demo
            }),
            slash::Command::Plugins => self.open_plugins(),
            slash::Command::Models => self.open_models(),
            slash::Command::Export => self.export(),
            slash::Command::Resume => {}
            slash::Command::Brainstorm => self.notice = Some(brainstorm::USAGE.into()),
            slash::Command::Help => self.notice = Some(slash::help()),
        }
    }
    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        self.select_agent(None);
        self.model_picker = None;
        std::mem::swap(&mut self.draft, &mut self.other_draft);
        let mut next = self.other_plugins.take().unwrap_or_default();
        next.live = mode == Mode::Live;
        next.bundled.live = mode == Mode::Live;
        self.other_plugins = Some(std::mem::replace(&mut self.plugins, next));
        self.mode = mode;
        self.screen = Screen::Conversation;
        self.scroll = 0;
        self.notice = None;
        self.slash_selected = 0;
        self.slash_hidden = false;
    }
    pub fn take_download(&mut self) -> Option<Download> {
        self.download.take()
    }
    fn export(&mut self) {
        let value = trajectory::document(self);
        self.download = serde_json::to_vec_pretty(&value)
            .ok()
            .map(|bytes| Download {
                filename: "coder-demo.atif.json".into(),
                bytes,
            });
        self.notice = Some("Exported this synthetic demo as coder-demo.atif.json.".into());
    }
    pub fn retire_secrets(&mut self) {
        self.plugins.key_draft.erase();
        self.plugins.bundled.key_draft.erase();
        if let Some(other) = &mut self.other_plugins {
            other.key_draft.erase();
            other.bundled.key_draft.erase();
        }
    }
    pub fn input(&self) -> Option<Input> {
        if let Some(p) = &self.model_picker {
            return (p.stage == models::Stage::Models).then_some(Input {
                label: "Search models",
                secret: false,
            });
        }
        if self.screen == Screen::Plugins {
            return None;
        }
        if self.screen == Screen::Conversation {
            return Some(Input {
                label: "Message",
                secret: false,
            });
        }
        use plugins::SettingsFocus as F;
        match self.plugins.selected_definition().id {
            "openrouter-byok" => match self.plugins.focus {
                F::ApiKey => Some(Input {
                    label: "OpenRouter API key",
                    secret: true,
                }),
                F::Model => Some(Input {
                    label: "Model ID",
                    secret: false,
                }),
                _ => None,
            },
            "jev" => match self.plugins.bundled.focus {
                F::ApiKey => Some(Input {
                    label: "Gateway API key",
                    secret: true,
                }),
                F::Model => Some(Input {
                    label: "Jev model ID",
                    secret: false,
                }),
                F::Endpoint => Some(Input {
                    label: "API base URL",
                    secret: false,
                }),
                _ => None,
            },
            "brainstorm" => (self.plugins.bundled.brainstorm.focus == brainstorm::Focus::Origin)
                .then_some(Input {
                    label: "HTTPS origin",
                    secret: false,
                }),
            "boat-cloud" | "gce-cloud" => {
                self.plugins
                    .bundled
                    .cloud_editor
                    .as_ref()
                    .and_then(|e| match e.focus {
                        2 if e.placement == cloud_settings::Placement::Boat => Some(Input {
                            label: "Template",
                            secret: false,
                        }),
                        3 => Some(Input {
                            label: "Credential variables",
                            secret: false,
                        }),
                        4 => Some(Input {
                            label: "Workspace paths",
                            secret: false,
                        }),
                        _ => None,
                    })
            }
            _ => None,
        }
    }
    pub fn set_cursor(&mut self, offset: usize) -> bool {
        let Some(draft) = self.active_draft_mut() else {
            return false;
        };
        if offset <= draft.text.len()
            && (offset == draft.text.len()
                || draft.text.grapheme_indices(true).any(|(i, _)| i == offset))
        {
            draft.cursor = offset;
            true
        } else {
            false
        }
    }
    fn active_draft_mut(&mut self) -> Option<&mut Draft> {
        if let Some(p) = &mut self.model_picker {
            return (p.stage == models::Stage::Models).then_some(&mut p.query);
        }
        if self.screen == Screen::Conversation {
            return Some(&mut self.draft);
        }
        if self.screen != Screen::PluginSettings {
            return None;
        }
        use plugins::SettingsFocus as F;
        match self.plugins.selected_definition().id {
            "openrouter-byok" => match self.plugins.focus {
                F::ApiKey => Some(&mut self.plugins.key_draft),
                F::Model => Some(&mut self.plugins.model_draft),
                _ => None,
            },
            "jev" => match self.plugins.bundled.focus {
                F::ApiKey => Some(&mut self.plugins.bundled.key_draft),
                F::Model => Some(&mut self.plugins.bundled.model_draft),
                F::Endpoint => Some(&mut self.plugins.bundled.endpoint_draft),
                _ => None,
            },
            "brainstorm" => (self.plugins.bundled.brainstorm.focus == brainstorm::Focus::Origin)
                .then_some(&mut self.plugins.bundled.brainstorm.draft),
            "boat-cloud" | "gce-cloud" => {
                self.plugins
                    .bundled
                    .cloud_editor
                    .as_mut()
                    .and_then(|e| match e.focus {
                        2 if e.placement == cloud_settings::Placement::Boat => {
                            Some(&mut e.template)
                        }
                        3 => Some(&mut e.credentials),
                        4 => Some(&mut e.paths),
                        _ => None,
                    })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
