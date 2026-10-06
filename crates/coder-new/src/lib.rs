//! A Coder terminal with bundled plugins, live chat, and demo fixtures.

pub mod agents;
pub mod bundled_runtime;
pub mod bundled_settings;
pub mod credentials;
pub mod jev_plugin;
pub mod live;
pub mod model_catalog;
pub mod models;
pub mod plugin_definition;
pub mod plugin_store;
pub mod plugin_tools;
pub mod plugins;
pub mod provider;
pub mod slash;
pub mod snapshot;
pub mod theme;
pub mod tools;
pub mod ui;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    #[default]
    Conversation,
    Plugins,
    PluginSettings,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Demo,
    Live,
}

#[derive(Default)]
pub struct App {
    pub mode: Mode,
    pub screen: Screen,
    pub draft: Draft,
    pub messages: Vec<String>,
    pub scroll: u16,
    pub selected_agent: Option<usize>,
    pub animation_frame: u8,
    pub cursor_blink_frame: u8,
    pub elapsed_seconds: u64,
    pub plugins: plugins::Plugins,
    pub live: live::Chat,
    pub request: Option<live::Request>,
    pub request_id: u64,
    pub checking_key: bool,
    pub checking_jev: bool,
    pub slash_selected: usize,
    pub slash_hidden: bool,
    pub notice: Option<String>,
    pub model_picker: Option<models::Picker>,
    other_draft: Draft,
    return_screen: Screen,
    saved_chats: [Chat; 5],
}

#[derive(Default)]
struct Chat {
    draft: Draft,
    messages: Vec<String>,
    scroll: u16,
}

impl App {
    pub fn load_plugin_settings(&mut self, store: plugin_store::Store) -> Result<(), String> {
        let result = self.plugins.load_settings(store);
        if self.screen == Screen::PluginSettings {
            self.open_plugin_settings();
        }
        result
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        self.cancel_request();
        self.model_picker = None;
        std::mem::swap(&mut self.draft, &mut self.other_draft);
        self.plugins.set_live(mode == Mode::Live);
        self.mode = mode;
        self.screen = Screen::Conversation;
        self.scroll = 0;
        self.notice = None;
        self.slash_selected = 0;
        self.slash_hidden = false;
    }

    pub fn slash_hints(&self) -> Vec<slash::Command> {
        if self.slash_hidden
            || self.model_picker.is_some()
            || !matches!(self.screen, Screen::Conversation | Screen::Welcome)
        {
            return Vec::new();
        }
        slash::matches(&self.draft.text)
            .into_iter()
            .filter(|command| *command != slash::Command::Models || self.plugins.enabled)
            .collect()
    }

    pub fn cancel_request(&mut self) {
        self.request_id = self.request_id.wrapping_add(1);
        self.request = None;
        self.checking_key = false;
        self.checking_jev = false;
        if self.live.busy {
            if !self.live.partial.is_empty() {
                self.live.entries.push(live::Entry::Assistant {
                    text: std::mem::take(&mut self.live.partial),
                    model: self.live.partial_model.take(),
                });
            }
            self.live.partial_model = None;
            self.live.busy = false;
            self.live.notice = Some("Reply stopped.".into());
        }
        if matches!(self.plugins.connection, plugins::Connection::Checking) {
            self.plugins.connection = plugins::Connection::Unchecked;
        }
        if matches!(
            self.plugins.bundled.connection,
            plugins::Connection::Checking
        ) {
            self.plugins.bundled.connection = plugins::Connection::Unchecked;
        }
        for entry in &mut self.live.entries {
            if let live::Entry::Tool {
                running, output, ..
            } = entry
            {
                if *running {
                    *running = false;
                    *output = serde_json::json!({"error":"Stopped by the user."});
                }
            }
        }
    }

    pub fn check_key(&mut self) {
        if self.mode != Mode::Live {
            return;
        }
        if self.live.busy {
            self.plugins.error = Some("Wait for the current reply before testing a key.");
            return;
        }
        let Some(key) = self.plugins.key_for_check() else {
            self.plugins.connection =
                plugins::Connection::Failed("Add an OpenRouter API key first.".into());
            return;
        };
        self.cancel_request();
        self.plugins.connection = plugins::Connection::Checking;
        self.checking_key = true;
        self.request = Some(live::Request {
            id: self.request_id,
            key,
            kind: live::Work::Check,
        });
    }

    pub fn apply_update(&mut self, update: live::Update) {
        if update.id() != self.request_id || self.mode != Mode::Live {
            return;
        }
        match update {
            live::Update::Checked { result, .. } => {
                self.checking_key = false;
                self.plugins.connection = match result {
                    Ok(_) => plugins::Connection::Verified,
                    Err(error) => plugins::Connection::Failed(error),
                };
            }
            live::Update::CheckedJev { result, .. } => {
                self.checking_key = false;
                self.checking_jev = false;
                self.plugins.bundled.connection = match result {
                    Ok(_) => plugins::Connection::Verified,
                    Err(error) => plugins::Connection::Failed(error),
                };
            }
            live::Update::Tool {
                name,
                input,
                output,
                running,
                ..
            } if self.live.busy => {
                if let Some(live::Entry::Tool { input: previous_input, output: previous_output, running: previous_running, .. }) = self.live.entries.iter_mut().rev().find(|entry| {
                    matches!(entry, live::Entry::Tool { name: previous, input: previous_input, running: true, .. } if previous == &name && (input.is_null() || previous_input == &input))
                }) {
                    if !input.is_null() { *previous_input = input; }
                    *previous_output = output;
                    *previous_running = running;
                } else {
                    self.live.entries.push(live::Entry::Tool { name, input, output, running });
                }
                self.scroll = u16::MAX;
            }
            live::Update::Delta { text, .. } if self.live.busy => {
                self.live.partial.push_str(&text);
                self.scroll = u16::MAX;
            }
            live::Update::Model { model, .. } if self.live.busy => {
                self.live.partial_model = live::model_slug(&model);
            }
            live::Update::Finished { result, .. } if self.live.busy => {
                self.live.busy = false;
                for entry in &mut self.live.entries {
                    if let live::Entry::Tool {
                        running, output, ..
                    } = entry
                    {
                        if *running {
                            *running = false;
                            *output = serde_json::json!({"error":"The turn ended before this tool reported a result."});
                        }
                    }
                }
                match result {
                    Ok(reply) => {
                        self.live.tokens =
                            self.live.tokens.saturating_add(reply.usage.total_tokens);
                        self.live.entries.push(live::Entry::Assistant {
                            text: reply.text,
                            model: live::model_slug(&reply.model),
                        });
                        self.live.partial.clear();
                        self.live.partial_model = None;
                        self.live.notice = None;
                        if self.plugins.enabled && self.plugins.key_configured {
                            self.plugins.connection = plugins::Connection::Verified;
                        }
                    }
                    Err(error) => {
                        if error.contains("HTTP 401") {
                            self.plugins.connection = plugins::Connection::Failed(error.clone());
                        }
                        if !self.live.partial.is_empty() {
                            self.live.entries.push(live::Entry::Assistant {
                                text: std::mem::take(&mut self.live.partial),
                                model: self.live.partial_model.take(),
                            });
                        }
                        self.live.partial_model = None;
                        self.live.notice = Some(error);
                    }
                }
                self.scroll = u16::MAX;
            }
            _ => {}
        }
    }

    fn command(&mut self, command: slash::Command) {
        self.draft = Draft::default();
        self.slash_selected = 0;
        self.slash_hidden = false;
        self.notice = None;
        match command {
            slash::Command::Demo => self.set_mode(if self.mode == Mode::Demo { Mode::Live } else { Mode::Demo }),
            slash::Command::Plugins => self.open_plugins(),
            slash::Command::Models => self.open_models(),
            slash::Command::Help => self.notice = Some("/demo  Toggle demo/live\n/plugins  Manage plugins\n/models  Choose a model for an enabled provider\n/help  Show commands\nTab  Complete a command\nEsc  Close suggestions or stop a reply\nCtrl+C  Quit".into()),
        }
    }

    fn submit_live(&mut self) {
        if self.live.busy {
            self.live.notice = Some("Wait for the current reply or press Esc to stop it.".into());
            return;
        }
        let key = self
            .plugins
            .key_for_request()
            .filter(|_| self.plugins.enabled);
        if key.is_none() && !self.plugins.bundled.microcoder {
            self.live.notice = Some(
                if self.plugins.enabled {
                    "Add your OpenRouter API key or enable Microcoder in /plugins."
                } else {
                    "Turn on Microcoder or connect OpenRouter BYOK in /plugins."
                }
                .into(),
            );
            return;
        }
        self.cancel_request();
        self.live
            .entries
            .push(live::Entry::User(std::mem::take(&mut self.draft.text)));
        self.draft.cursor = 0;
        self.live.notice = None;
        self.live.partial.clear();
        self.live.partial_model = None;
        self.live.busy = true;
        self.screen = Screen::Conversation;
        self.scroll = u16::MAX;
        let execution = self.plugins.execution_settings(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
        let kind = if key.is_some() {
            live::Work::Chat {
                model: self.plugins.model.clone(),
                options: self.plugins.options.clone(),
                messages: self.live.messages(),
                execution,
            }
        } else {
            live::Work::Microcoder {
                messages: self.live.messages(),
                execution,
            }
        };
        self.request = Some(live::Request {
            id: self.request_id,
            key: key.unwrap_or_else(|| model_access::ApiKey::new("")),
            kind,
        });
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
            self.mode == Mode::Live,
        ));
    }

    pub fn open_plugin_settings(&mut self) {
        self.open_plugins();
        match self.plugins.selected_definition().id {
            "openrouter-byok" => self.plugins.begin_settings(),
            "jev" => self.plugins.bundled.begin_settings(),
            "acp-subagents" => self.plugins.bundled.begin_acp(),
            _ => {}
        }
        self.screen = Screen::PluginSettings;
    }

    pub fn check_jev_key(&mut self) {
        if self.mode != Mode::Live {
            return;
        }
        if self.live.busy {
            self.plugins.bundled.error = Some("Wait for the current reply before testing a key.");
            return;
        }
        let endpoint = match self.plugins.bundled.endpoint_for_check() {
            Ok(endpoint) => endpoint,
            Err(error) => {
                self.plugins.bundled.connection = plugins::Connection::Failed(error);
                return;
            }
        };
        let model = match self.plugins.bundled.model_for_check() {
            Ok(model) => model,
            Err(error) => {
                self.plugins.bundled.connection = plugins::Connection::Failed(error);
                return;
            }
        };
        let Some(key) = self.plugins.bundled.key_for_check() else {
            self.plugins.bundled.connection =
                plugins::Connection::Failed("Add an API key for this Jev connection first.".into());
            return;
        };
        self.cancel_request();
        self.plugins.bundled.connection = plugins::Connection::Checking;
        self.checking_key = true;
        self.checking_jev = true;
        self.request = Some(live::Request {
            id: self.request_id,
            key,
            kind: live::Work::CheckJev { endpoint, model },
        });
    }

    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1) % 8;
        self.cursor_blink_frame = self.cursor_blink_frame.wrapping_add(1) % 8;
    }

    fn select_agent(&mut self, selected: Option<usize>) {
        if self.selected_agent == selected {
            return;
        }
        let previous = self.selected_agent.map_or(0, |index| index + 1);
        self.saved_chats[previous] = Chat {
            draft: std::mem::take(&mut self.draft),
            messages: std::mem::take(&mut self.messages),
            scroll: self.scroll,
        };
        let next = selected.map_or(0, |index| index + 1);
        let chat = std::mem::take(&mut self.saved_chats[next]);
        self.draft = chat.draft;
        self.messages = chat.messages;
        self.scroll = chat.scroll;
        self.selected_agent = selected;
        self.screen = Screen::Conversation;
    }

    /// Returns false when the preview should close.
    pub fn handle(&mut self, event: Event) -> bool {
        match event {
            Event::Mouse(mouse)
                if self.screen == Screen::Conversation && self.model_picker.is_none() =>
            {
                match mouse.kind {
                    MouseEventKind::ScrollUp => self.scroll = self.scroll.saturating_sub(3),
                    MouseEventKind::ScrollDown => self.scroll = self.scroll.saturating_add(3),
                    _ => {}
                }
            }
            Event::Paste(text) => {
                self.cursor_blink_frame = 0;
                if let Some(picker) = &mut self.model_picker {
                    picker.paste(&text);
                } else if self.screen == Screen::PluginSettings {
                    match self.plugins.selected_definition().id {
                        "jev" => {
                            if matches!(
                                self.plugins.bundled.focus,
                                plugins::SettingsFocus::ApiKey
                                    | plugins::SettingsFocus::Endpoint
                                    | plugins::SettingsFocus::Model
                            ) {
                                if self.checking_key {
                                    self.cancel_request();
                                }
                                self.plugins.bundled.connection = plugins::Connection::Unchecked;
                            }
                            self.plugins.bundled.paste(&text);
                            return true;
                        }
                        "acp-subagents" => {
                            if self
                                .plugins
                                .bundled
                                .acp_draft
                                .text
                                .len()
                                .saturating_add(text.len())
                                <= 48 * 1024
                            {
                                self.plugins.bundled.acp_draft.insert(&text);
                                self.plugins.bundled.acp_error = None;
                            }
                            return true;
                        }
                        "openrouter-byok" => {}
                        _ => return true,
                    }
                    if self.mode == Mode::Live
                        && self.plugins.focus == plugins::SettingsFocus::ApiKey
                    {
                        if self.checking_key {
                            self.cancel_request();
                        }
                        self.plugins.connection = plugins::Connection::Unchecked;
                    }
                    self.plugins.paste(&text);
                } else if self.screen != Screen::Plugins {
                    self.draft.insert(&text);
                    self.slash_selected = 0;
                    self.slash_hidden = false;
                }
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                self.cursor_blink_frame = 0;
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                if ctrl && key.code == KeyCode::Char('c') {
                    return false;
                }
                if let Some(picker) = &mut self.model_picker {
                    match picker.handle(key) {
                        models::Action::Continue => {}
                        models::Action::Close => self.model_picker = None,
                        models::Action::Save(model, options) => {
                            if self.plugins.set_model(&model, options) {
                                self.model_picker = None;
                            } else if let Some(picker) = &mut self.model_picker {
                                picker.error = self.plugins.storage_error.clone();
                            }
                        }
                    }
                    return true;
                }
                if self.screen == Screen::PluginSettings {
                    match self.plugins.selected_definition().id {
                        "jev" => {
                            if self.mode == Mode::Live
                                && ((matches!(
                                    self.plugins.bundled.focus,
                                    plugins::SettingsFocus::ApiKey
                                        | plugins::SettingsFocus::Endpoint
                                        | plugins::SettingsFocus::Model
                                ) && matches!(
                                    key.code,
                                    KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
                                )) || (self.plugins.bundled.focus
                                    == plugins::SettingsFocus::Gateway
                                    && matches!(
                                        key.code,
                                        KeyCode::Enter | KeyCode::Left | KeyCode::Right
                                    ))
                                    || (self.plugins.bundled.focus
                                        == plugins::SettingsFocus::RemoveKey
                                        && key.code == KeyCode::Enter))
                            {
                                if self.checking_key {
                                    self.cancel_request();
                                }
                                self.plugins.bundled.connection = plugins::Connection::Unchecked;
                            }
                            let closed = self.plugins.bundled.handle(key);
                            if self.plugins.bundled.check_requested {
                                self.check_jev_key();
                            }
                            if closed {
                                self.screen = Screen::Plugins;
                                if self.mode == Mode::Live {
                                    if self.plugins.bundled.credential_changed || self.checking_key
                                    {
                                        let connection = self.plugins.bundled.connection.clone();
                                        self.cancel_request();
                                        self.plugins.bundled.connection = connection;
                                    }
                                    if self.plugins.bundled.saved
                                        && self.plugins.bundled.jev_key().is_some()
                                        && !self.live.busy
                                        && !matches!(
                                            self.plugins.bundled.connection,
                                            plugins::Connection::Verified
                                        )
                                    {
                                        self.check_jev_key();
                                    }
                                }
                            }
                            return true;
                        }
                        "acp-subagents" => {
                            if ctrl && key.code == KeyCode::Char('s') {
                                if self.plugins.bundled.save_acp() {
                                    if self.mode == Mode::Live {
                                        self.cancel_request();
                                    }
                                    self.screen = Screen::Plugins;
                                }
                            } else if key.code == KeyCode::Esc {
                                self.plugins.bundled.cancel_acp();
                                self.screen = Screen::Plugins;
                            } else if key.code == KeyCode::Enter {
                                self.plugins.bundled.acp_draft.insert("\n");
                            } else {
                                self.plugins.bundled.acp_draft.edit(key);
                            }
                            return true;
                        }
                        "openrouter-byok" => {}
                        _ => {
                            if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                                self.screen = Screen::Plugins;
                            }
                            return true;
                        }
                    }
                    if self.mode == Mode::Live
                        && self.plugins.focus == plugins::SettingsFocus::ApiKey
                        && matches!(
                            key.code,
                            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
                        )
                    {
                        if self.checking_key {
                            self.cancel_request();
                        }
                        self.plugins.connection = plugins::Connection::Unchecked;
                    }
                    let closed = self.plugins.handle(key);
                    if self.plugins.check_requested {
                        self.check_key();
                    }
                    if closed {
                        self.screen = Screen::Plugins;
                        if self.mode == Mode::Live {
                            if self.plugins.credential_changed || self.checking_key {
                                let connection = self.plugins.connection.clone();
                                self.cancel_request();
                                self.plugins.connection = connection;
                            }
                            if self.plugins.saved
                                && self.plugins.key_configured
                                && !self.live.busy
                                && !matches!(self.plugins.connection, plugins::Connection::Verified)
                            {
                                self.check_key();
                            }
                        }
                    }
                    return true;
                }
                if self.screen == Screen::Plugins {
                    match key.code {
                        KeyCode::Up => self.plugins.select(true),
                        KeyCode::Down => self.plugins.select(false),
                        KeyCode::Char(' ') => {
                            if self.plugins.toggle_selected() && self.mode == Mode::Live {
                                self.cancel_request();
                            }
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
                        KeyCode::Enter if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
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
                            .map_or(0, |index| (index + 1).min(agents::DEMOS.len() - 1)),
                    )),
                    KeyCode::Up if self.mode == Mode::Demo => self
                        .select_agent(self.selected_agent.and_then(|index| index.checked_sub(1))),
                    KeyCode::Esc if self.mode == Mode::Live => {
                        self.cancel_request();
                        self.live
                            .notice
                            .get_or_insert_with(|| "Request stopped.".into());
                    }
                    KeyCode::Esc => self.select_agent(None),
                    KeyCode::Tab | KeyCode::BackTab => {
                        self.screen = match self.screen {
                            Screen::Welcome => Screen::Conversation,
                            Screen::Conversation => Screen::Welcome,
                            Screen::Plugins | Screen::PluginSettings => unreachable!(),
                        };
                        self.scroll = 0;
                    }
                    KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(5),
                    KeyCode::PageDown => self.scroll = self.scroll.saturating_add(5),
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                        self.draft.insert("\n");
                    }
                    KeyCode::Enter if !ctrl => {
                        if let Some(command) = slash::parse(self.draft.text.trim()) {
                            self.command(command);
                        } else if slash::is_command_word(self.draft.text.trim()) {
                            self.notice =
                                Some("Unknown command. Type / to see available commands.".into());
                            self.draft = Draft::default();
                        } else if self.mode == Mode::Live && !self.draft.text.trim().is_empty() {
                            self.submit_live();
                        } else if !self.draft.text.trim().is_empty() {
                            self.messages.push(std::mem::take(&mut self.draft.text));
                            self.draft.cursor = 0;
                            self.screen = Screen::Conversation;
                            self.scroll = u16::MAX;
                        }
                    }
                    _ => {
                        self.draft.edit(key);
                        self.slash_selected = 0;
                        self.slash_hidden = false;
                    }
                }
            }
            _ => {}
        }
        true
    }
}

#[derive(Default)]
pub struct Draft {
    pub text: String,
    pub cursor: usize,
}

impl Draft {
    fn edit(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.insert(&ch.to_string());
            }
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.cursor = self.previous(),
            KeyCode::Right => self.cursor = self.next(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            _ => {}
        }
    }

    fn insert(&mut self, text: &str) {
        let clean: String = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\t', "    ")
            .chars()
            .filter(|ch| *ch == '\n' || !ch.is_control())
            .collect();
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
        self.snap_cursor();
    }

    fn snap_cursor(&mut self) {
        // Edits can join surrounding graphemes; keep the cursor at a boundary.
        while self.cursor < self.text.len()
            && !self
                .text
                .grapheme_indices(true)
                .any(|(offset, _)| offset == self.cursor)
        {
            self.cursor += self.text[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
        }
    }

    fn previous(&self) -> usize {
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(offset, _)| offset)
    }

    fn next(&self) -> usize {
        self.cursor
            + self.text[self.cursor..]
                .graphemes(true)
                .next()
                .map_or(0, str::len)
    }

    fn backspace(&mut self) {
        let previous = self.previous();
        self.text.drain(previous..self.cursor);
        self.cursor = previous;
        self.snap_cursor();
    }

    fn delete(&mut self) {
        self.text.drain(self.cursor..self.next());
        self.snap_cursor();
    }

    /// Hard-wraps by terminal cells so the rendered cursor matches the draft.
    pub fn wrapped(&self, width: u16) -> (Vec<String>, (u16, u16)) {
        let width = usize::from(width.max(1));
        let mut lines = vec![String::new()];
        let mut column = 0;
        let mut cursor = (0, 0);
        for (offset, grapheme) in self.text.grapheme_indices(true) {
            let cells = grapheme.width();
            if grapheme != "\n" && column + cells > width && column > 0 {
                lines.push(String::new());
                column = 0;
            }
            if offset == self.cursor {
                cursor = (column as u16, (lines.len() - 1) as u16);
            }
            if grapheme == "\n" {
                lines.push(String::new());
                column = 0;
            } else {
                lines
                    .last_mut()
                    .expect("the draft always has a line")
                    .push_str(grapheme);
                column += cells;
            }
        }
        if self.cursor == self.text.len() {
            if column >= width {
                lines.push(String::new());
                column = 0;
            }
            cursor = (column as u16, (lines.len() - 1) as u16);
        }
        (lines, cursor)
    }
}
