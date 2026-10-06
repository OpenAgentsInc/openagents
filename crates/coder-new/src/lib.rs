//! A Coder terminal with bundled plugins, live chat, and demo fixtures.

pub mod acp_discovery;
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
pub mod programmatic;
pub mod provider;
pub mod slash;
pub mod snapshot;
pub mod theme;
pub mod tools;
pub mod trajectory;
pub mod ui;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Demo fixtures are available only in local development builds.
pub const DEMO_AVAILABLE: bool = cfg!(debug_assertions);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Conversation,
    Plugins,
    PluginSettings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Demo,
    Live,
}

impl Default for Mode {
    fn default() -> Self {
        if DEMO_AVAILABLE {
            Self::Demo
        } else {
            Self::Live
        }
    }
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
    pub delegations: Vec<live::Delegation>,
    pub cwd: Option<std::path::PathBuf>,
    pub branch: Option<String>,
    pub request: Option<live::Request>,
    pub request_id: u64,
    pub checking_key: bool,
    pub checking_jev: bool,
    pub slash_selected: usize,
    pub slash_hidden: bool,
    pub notice: Option<String>,
    pub model_picker: Option<models::Picker>,
    pub(crate) active_options: models::GenerationOptions,
    pending_export_path: Option<std::path::PathBuf>,
    active_delegation: Option<String>,
    main_draft: Draft,
    main_scroll: u16,
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
    fn scroll_main_to_end(&mut self) {
        if self.selected_agent.is_none() {
            self.scroll = u16::MAX;
        } else {
            self.main_scroll = u16::MAX;
        }
    }
    fn apply_delegation(
        &mut self,
        id: String,
        name: String,
        task: String,
        event: bundled_runtime::RuntimeEvent,
    ) {
        let index = self
            .delegations
            .iter()
            .position(|child| child.id == id)
            .unwrap_or_else(|| {
                self.live.entries.push(live::Entry::Delegation {
                    id: id.clone(),
                    name: name.clone(),
                    task: task.clone(),
                    running: true,
                    output: serde_json::Value::Null,
                });
                self.delegations.push(live::Delegation {
                    id: id.clone(),
                    name: name.clone(),
                    task: task.clone(),
                    chat: live::Chat {
                        entries: vec![live::Entry::User(task)],
                        busy: true,
                        ..live::Chat::default()
                    },
                    started_at: self.elapsed_seconds,
                    elapsed_seconds: 0,
                    running: true,
                    draft: Draft::default(),
                    scroll: u16::MAX,
                });
                self.delegations.len() - 1
            });
        let child = &mut self.delegations[index];
        match event {
            bundled_runtime::RuntimeEvent::Text(text) => child.chat.partial.push_str(&text),
            bundled_runtime::RuntimeEvent::Model(model) => {
                child.chat.partial_model = live::model_slug(&model)
            }
            bundled_runtime::RuntimeEvent::Tool {
                name: tool,
                input,
                output,
                running,
            } if tool == "microcoder" || tool == "acp_subagent" => {
                child.running = running;
                child.chat.busy = running;
                if !running {
                    child.elapsed_seconds = self.elapsed_seconds.saturating_sub(child.started_at);
                    child.chat.tokens = output
                        .get("tokens")
                        .and_then(serde_json::Value::as_u64)
                        .or_else(|| {
                            output
                                .get("usage")
                                .and_then(|usage| usage.get("total_tokens"))
                                .and_then(serde_json::Value::as_u64)
                        })
                        .unwrap_or(0);
                    if let Some(model) = output.get("model").and_then(serde_json::Value::as_str) {
                        child.chat.partial_model = live::model_slug(model);
                    }
                    if child.chat.partial.is_empty() {
                        if let Some(reply) = output.get("reply").and_then(serde_json::Value::as_str)
                        {
                            child.chat.partial = reply.into();
                        }
                    }
                    child.chat.finish_partial();
                    child
                        .chat
                        .stop_tools("The delegation ended before this tool returned.");
                    child.chat.notice = output
                        .get("error")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned);
                }
                if let Some(live::Entry::Delegation { running: state, output: result, .. }) = self.live.entries.iter_mut().find(|entry| matches!(entry, live::Entry::Delegation {id: previous,..} if previous == &id)) {
                    *state = running;
                    *result = output;
                }
                let _ = input;
            }
            bundled_runtime::RuntimeEvent::Tool {
                name,
                input,
                output,
                running,
            } => child.chat.tool(name, input, output, running),
            bundled_runtime::RuntimeEvent::Delegation { .. } => {}
        }
        if self.selected_agent == Some(index) {
            self.scroll = u16::MAX;
        } else {
            child.scroll = u16::MAX;
        }
    }

    pub fn load_plugin_settings(&mut self, store: plugin_store::Store) -> Result<(), String> {
        let result = self.plugins.load_settings(store);
        if self.screen == Screen::PluginSettings {
            self.open_plugin_settings();
        }
        result
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if mode == Mode::Demo && !DEMO_AVAILABLE {
            self.notice = Some("Demo mode is available only in local development builds.".into());
            return;
        }
        if self.mode == mode {
            return;
        }
        self.cancel_request();
        self.select_agent(None);
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
        if self.slash_hidden || self.model_picker.is_some() || self.screen != Screen::Conversation {
            return Vec::new();
        }
        slash::matches(&self.draft.text)
            .into_iter()
            .filter(|command| *command != slash::Command::Models || self.plugins.enabled)
            .collect()
    }

    pub fn cancel_request(&mut self) {
        self.active_delegation = None;
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
            if let live::Entry::Delegation {
                running, output, ..
            } = entry
            {
                if *running {
                    *running = false;
                    *output = serde_json::json!({"error":"Stopped by the user."});
                }
            }
        }
        for delegation in &mut self.delegations {
            if delegation.running {
                delegation.running = false;
                delegation.elapsed_seconds =
                    self.elapsed_seconds.saturating_sub(delegation.started_at);
                delegation.chat.busy = false;
                delegation.chat.notice = Some("Stopped by the user.".into());
                delegation.chat.finish_partial();
                delegation.chat.stop_tools("Stopped by the user.");
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
            live::Update::Delegation {
                delegation,
                name,
                task,
                event,
                ..
            } if self.live.busy => {
                let delegation = if self.active_delegation.as_deref() == Some(&delegation) {
                    delegation
                } else {
                    format!("{}:{delegation}", self.request_id)
                };
                self.apply_delegation(delegation, name, task, event);
            }
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
                self.scroll_main_to_end();
            }
            live::Update::Delta { text, .. } if self.live.busy => {
                self.live.partial.push_str(&text);
                self.scroll_main_to_end();
            }
            live::Update::Model { model, .. } if self.live.busy => {
                if model == "openagents/fallback" {
                    self.active_options = models::GenerationOptions::default();
                    return;
                }
                self.live.partial_model =
                    live::model_slug(&model).map(|model| self.active_options.slug(&model));
            }
            live::Update::Finished { result, .. } if self.live.busy => {
                self.live.busy = false;
                if let Some(id) = self.active_delegation.take() {
                    if let Some(child) = self.delegations.iter_mut().find(|child| child.id == id) {
                        child.running = false;
                        child.chat.busy = false;
                        child.elapsed_seconds =
                            self.elapsed_seconds.saturating_sub(child.started_at);
                        if let Err(error) = result {
                            child.chat.notice = Some(error);
                        }
                        child.chat.finish_partial();
                        child
                            .chat
                            .stop_tools("The delegation ended before this tool returned.");
                    }
                    return;
                }
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
                            model: live::model_slug(&reply.model)
                                .map(|model| self.active_options.slug(&model)),
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
                self.scroll_main_to_end();
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
            slash::Command::Demo => self.set_mode(if self.mode == Mode::Demo {
                Mode::Live
            } else {
                Mode::Demo
            }),
            slash::Command::Plugins => self.open_plugins(),
            slash::Command::Models => self.open_models(),
            slash::Command::Export => self.export(None),
            slash::Command::Help => self.notice = Some(slash::help()),
        }
    }

    fn export(&mut self, path: Option<&std::path::Path>) {
        self.pending_export_path = None;
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
        let root = model_access::store::openagents_dir();
        self.notice = Some(
            match trajectory::export_app(self, path, &cwd, root.as_deref()) {
                Ok(path) => {
                    let path = path.canonicalize().unwrap_or(path);
                    let notice = format!("Exported ATIF to {}.", path.display());
                    self.pending_export_path = Some(path);
                    notice
                }
                Err(error) => error,
            },
        );
        self.draft = Draft::default();
    }

    /// Copy a saved export path once, through the terminal's clipboard adapter.
    pub fn copy_export_path(&mut self, copy: impl FnOnce(&str) -> std::io::Result<()>) {
        let Some(path) = self.pending_export_path.take() else {
            return;
        };
        let status = match copy(&path.to_string_lossy()) {
            Ok(()) => "Path copied to clipboard.".to_owned(),
            Err(error) => format!("Cannot copy path to clipboard: {error}."),
        };
        self.notice = Some(format!("Exported ATIF to {}. {status}", path.display()));
    }

    pub fn submit(&mut self, text: &str, cwd: &std::path::Path) {
        self.cwd = Some(cwd.to_owned());
        self.draft.text = text.into();
        self.draft.cursor = text.len();
        self.submit_live();
    }

    pub fn submit_live(&mut self) {
        if self.live.busy {
            self.live.notice = Some("Wait for the current reply or press Esc to stop it.".into());
            return;
        }
        if let Some(index) = self.selected_agent {
            self.submit_delegation(index);
            return;
        }
        let key = self
            .plugins
            .key_for_request()
            .filter(|_| self.plugins.enabled);
        self.cancel_request();
        self.live
            .entries
            .push(live::Entry::User(std::mem::take(&mut self.draft.text)));
        self.draft.cursor = 0;
        self.live.notice = None;
        self.live.partial.clear();
        self.live.partial_model = None;
        self.live.busy = true;
        self.active_options = if key.is_some() {
            self.plugins.options.clone()
        } else {
            models::GenerationOptions::default()
        };
        self.screen = Screen::Conversation;
        self.scroll = u16::MAX;
        let execution = self
            .plugins
            .execution_settings(self.cwd.clone().unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            }));
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

    fn submit_delegation(&mut self, index: usize) {
        let Some(child) = self.delegations.get(index) else {
            return;
        };
        let id = child.id.clone();
        let name = child.name.clone();
        let mut history = child.chat.messages();
        let text = std::mem::take(&mut self.draft.text);
        history.push(openrouter::Message::user(text.clone()));
        let task = history
            .into_iter()
            .map(|message| format!("{}: {}", message.role, message.content))
            .collect::<Vec<_>>()
            .join("\n\n");
        let execution = self
            .plugins
            .execution_settings(self.cwd.clone().unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            }));
        let (tool, arguments) = if name == "microcoder" {
            ("microcoder".to_owned(), serde_json::json!({"task":task}))
        } else if let Some(agent) = execution
            .agents
            .iter()
            .find(|agent| agent.name == name && agent.enabled)
        {
            (
                "acp_subagent".to_owned(),
                serde_json::json!({"agent":agent.id,"task":task}),
            )
        } else {
            self.draft.text = text;
            self.live.notice = Some(
                "This delegated agent is unavailable or turned off. Enable it in /plugins.".into(),
            );
            return;
        };
        self.cancel_request();
        self.active_delegation = Some(id.clone());
        self.active_options = self.plugins.options.clone();
        let child = &mut self.delegations[index];
        child.chat.entries.push(live::Entry::User(text));
        child.chat.notice = None;
        child.chat.busy = true;
        child.running = true;
        child.started_at = self.elapsed_seconds;
        self.live.busy = true;
        self.draft.cursor = 0;
        self.scroll = u16::MAX;
        let key = self
            .plugins
            .key_for_request()
            .filter(|_| self.plugins.enabled)
            .unwrap_or_else(|| model_access::ApiKey::new(""));
        self.request = Some(live::Request {
            id: self.request_id,
            key,
            kind: live::Work::Delegate {
                delegation: id,
                name,
                tool,
                arguments,
                execution,
                model: self.plugins.model.clone(),
                options: self.plugins.options.clone(),
            },
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
        if self.mode == Mode::Live {
            if selected.is_some_and(|index| index >= self.delegations.len()) {
                return;
            }
            if let Some(previous) = self.selected_agent {
                self.delegations[previous].draft = std::mem::take(&mut self.draft);
                self.delegations[previous].scroll = self.scroll;
            } else {
                self.main_draft = std::mem::take(&mut self.draft);
                self.main_scroll = self.scroll;
            }
            if let Some(next) = selected {
                self.draft = std::mem::take(&mut self.delegations[next].draft);
                self.scroll = self.delegations[next].scroll;
            } else {
                self.draft = std::mem::take(&mut self.main_draft);
                self.scroll = self.main_scroll;
            }
            self.selected_agent = selected;
            self.screen = Screen::Conversation;
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
                        "acp-subagents" => return true,
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
                if matches!(key.code, KeyCode::Char('p' | 'P'))
                    && (key.modifiers == KeyModifiers::SUPER
                        || key.modifiers == KeyModifiers::CONTROL)
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
                            match key.code {
                                KeyCode::Esc => self.screen = Screen::Plugins,
                                KeyCode::Up => self.plugins.bundled.select_acp(true),
                                KeyCode::Down => self.plugins.bundled.select_acp(false),
                                KeyCode::Home => self.plugins.bundled.acp_selected = 0,
                                KeyCode::End => {
                                    self.plugins.bundled.acp_selected =
                                        self.plugins.bundled.acp_choices().len().saturating_sub(1);
                                }
                                KeyCode::Char('r') => self.plugins.bundled.refresh_acp(),
                                KeyCode::Char(' ') | KeyCode::Enter => {
                                    if self.plugins.bundled.toggle_acp_agent()
                                        && self.mode == Mode::Live
                                    {
                                        self.cancel_request();
                                    }
                                }
                                _ => {}
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
                    KeyCode::Down if !self.delegations.is_empty() => self.select_agent(Some(
                        self.selected_agent
                            .map_or(0, |index| (index + 1).min(self.delegations.len() - 1)),
                    )),
                    KeyCode::Up if self.mode == Mode::Live => self
                        .select_agent(self.selected_agent.and_then(|index| index.checked_sub(1))),
                    KeyCode::Esc if self.mode == Mode::Live => {
                        self.cancel_request();
                        self.live
                            .notice
                            .get_or_insert_with(|| "Request stopped.".into());
                    }
                    KeyCode::Esc => self.select_agent(None),
                    KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(5),
                    KeyCode::PageDown => self.scroll = self.scroll.saturating_add(5),
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                        self.draft.insert("\n");
                    }
                    KeyCode::Enter if !ctrl => {
                        if let Some(path) = self
                            .draft
                            .text
                            .trim()
                            .strip_prefix("/export ")
                            .map(str::trim)
                            .filter(|path| !path.is_empty())
                            .map(std::path::PathBuf::from)
                        {
                            self.export(Some(&path));
                        } else if let Some(command) = slash::parse(self.draft.text.trim()) {
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
