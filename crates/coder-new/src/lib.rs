//! A Coder terminal with bundled plugins, live chat, and demo fixtures.

pub mod account;
pub mod account_export;
pub mod account_sync;
pub mod acp_discovery;
pub mod agents;
pub mod appearance;
pub mod approval;
pub mod attachments;
pub mod brainstorm;
pub mod bundled_runtime;
pub mod bundled_settings;
mod checkout_guard;
/// The mode that runs turns on this computer's Claude account login.
pub mod claude;
pub mod claude_chat;
pub mod claude_plan;
pub mod claude_stream;
pub mod claude_tools;
pub mod cloud;
pub mod cloud_settings;
pub mod cloud_tools;
mod codex_usage;
pub mod comms;
mod composer_history;
pub mod composer_state;
pub mod computer_tool;
pub mod connection_tools;
#[cfg(test)]
mod copy_guard_tests;
pub mod credentials;
mod delegation_events;
mod demo;
pub mod file_tools;
pub mod fleet;
mod fleet_app;
mod git_guard;
pub mod github_tool;
pub mod harness;
pub mod harness_plugin;
pub mod home;
pub mod identity;
pub mod issue_run;
pub mod jev_plugin;
pub mod live;
pub mod login_choice;
mod long_session;
mod loops;
pub mod media;
pub mod memory;
mod memory_sync;
pub mod model_catalog;
pub mod models;
pub mod ops_tool;
mod own_runs;
mod plan;
#[cfg(unix)]
pub mod plugin_catalog;
pub mod plugin_definition;
pub mod plugin_store;
pub mod plugin_tools;
pub mod plugins;
pub mod preferences;
pub mod programmatic;
mod prompt_queue;
pub mod provider;
pub mod resume;
pub mod risk_policy;
mod route;
pub mod routing_preferences;
mod schedule;
mod schedule_sync;
pub mod sessions;
mod shells;
pub mod slash;
pub mod snapshot;
pub mod solver;
pub mod solver_plugin;
mod stuck;
mod supervise;
pub mod swarm;
pub mod theme;
pub mod tools;
pub mod trace_upload;
pub mod trajectory;
pub mod ui;
pub mod update;
mod workflow;
// Screenshots and files asked for on openagents.com (#11185).
mod web_asks;
pub mod web_tools;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// What the composer holds after the model the person chose failed or
/// refused a turn (#11132): Enter opens the model picker.
pub const TRY_ANOTHER_MODEL: &str = "/models";

/// How long a finished delegation stays on the rail under the input.
pub const RAIL_KEEP_SECONDS: u64 = 30;

/// Demo fixtures are available only in local development builds.
pub const DEMO_AVAILABLE: bool = cfg!(debug_assertions);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Conversation,
    Plugins,
    PluginSettings,
    Appearance,
    Preferences,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Demo,
    /// The chat. Each turn runs on the harness plugin its chat was routed
    /// to ([`harness_plugin`]): Claude Code or Coder's own loop. There is
    /// no mode for either (#11453).
    Live,
    /// Four constrained agents take an issue to a pull request
    /// ([`solver`]). `coder solver N` enters it to watch the run.
    Solver,
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

/// How much of each tool call the transcript shows. Ctrl+O switches between
/// them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    /// The default: each tool call on one row, parameters and all, with no
    /// blank row between calls.
    #[default]
    Compact,
    /// The tool call on one row, its parameters and result under it, and a
    /// blank row after it.
    Verbose,
}

#[derive(Default)]
pub struct App {
    pub mode: Mode,
    /// How much of each tool call the transcript shows (Ctrl+O).
    pub view: ViewMode,
    pub screen: Screen,
    pub draft: Draft,
    pub composer: composer_state::ComposerState,
    pub(crate) next_image_id: u64,
    /// Where sent attachments are saved (#11173); `None` is
    /// `~/.openagents/coder/attachments`.
    pub attachment_dir: Option<std::path::PathBuf>,
    pub(crate) composer_history: composer_history::History,
    pub(crate) composer_width: u16,
    footer_focused: bool,
    replaying_prompt: bool,
    queue_hint_count: u8,
    escape_at: Option<std::time::Instant>,
    pub messages: Vec<String>,
    pub scroll: u16,
    pub selected_agent: Option<usize>,
    pub animation_frame: u8,
    pub elapsed_seconds: u64,
    pub plugins: plugins::Plugins,
    pub appearance: appearance::Appearance,
    pub preferences: preferences::Preferences,
    pub live: live::Chat,
    pub delegations: Vec<live::Delegation>,
    pub cwd: Option<std::path::PathBuf>,
    /// The session's own worktree and the read-only checkout it was
    /// started from, when `coder chat` gave it one (#11372).
    pub boundary: Option<checkout_guard::Boundary>,
    pub branch: Option<String>,
    /// The signed-in openagents.com account's name (#11045).
    pub account: Option<String>,
    /// Where the account sign-in is kept (`~/.openagents/coder`).
    pub account_dir: Option<std::path::PathBuf>,
    pub(crate) login: Option<account::Login>,
    /// Saving chats to the account (#11046), once read.
    pub(crate) sync: Option<account_sync::SyncState>,
    pub request: Option<live::Request>,
    pub request_id: u64,
    /// Whether the turn under way is the claude chat's, so a message sent
    /// now goes to Claude Code at once (#11447). The worker sets it.
    pub(crate) claude_turn: bool,
    pub(crate) prompt_inbox: prompt_queue::Inbox,
    pub(crate) queued_prompts: Vec<prompt_queue::Prompt>,
    /// Background agents started from this terminal (#11163).
    pub fleet: fleet::Fleet,
    /// This process's agent list on disk, for the desktop app's Agents
    /// panel (#11180); set by [`App::publish_agents`].
    pub(crate) board: Option<agent_fleet::board::Publisher>,
    /// The `/agents` panel, while it is open.
    pub(crate) agents_panel: Option<fleet_app::Panel>,
    /// Compaction, usage-limit pauses and dollars (#11179).
    pub(crate) long_session: long_session::State,
    /// Prompts repeated with `/loop` (#11177).
    pub(crate) loops: loops::Loops,
    pub checking_key: bool,
    pub checking_jev: bool,
    pub(crate) brainstorm_job: Option<brainstorm::Job>,
    brainstorm_conversation: Option<brainstorm::Conversation>,
    pub interactive_disclosures: bool,
    pub(crate) model_owned_input: bool,
    disclosure_desk: Option<std::sync::Arc<approval::Desk>>,
    pub(crate) disclosure_event: Option<serde_json::Value>,
    pub(crate) disclosure_scroll: u16,
    pub(crate) disclosure_seen: bool,
    pub slash_selected: usize,
    pub slash_hidden: bool,
    pub notice: Option<String>,
    /// A newer Coder is ready or available (#11128); shown on the context row.
    pub update_line: Option<String>,
    pub model_picker: Option<models::Picker>,
    pub resume_picker: Option<resume::Picker>,
    history: resume::History,
    pub(crate) active_options: models::GenerationOptions,
    pending_export_path: Option<std::path::PathBuf>,
    /// Cmd+A (or Ctrl+Shift+A) selected the whole input: typing replaces
    /// it, Backspace or Delete clears it, Cmd+C copies it, Cmd+X cuts it.
    pub composer_selected: bool,
    /// Input text waiting for the terminal clipboard (Cmd+C, Cmd+X).
    pending_copy: Option<String>,
    /// Counts presses on the transcript for double- and triple-click.
    clicks: coder_terminal::selection::Clicks,
    /// A transcript selection went to the clipboard: the input's top-left
    /// rail says so until then.
    pub(crate) copied_until: Option<std::time::Instant>,
    export_notice_expiry: Option<(std::time::Instant, String)>,
    active_delegation: Option<String>,
    main_draft: Draft,
    main_composer: composer_state::ComposerState,
    main_scroll: u16,
    /// The furthest the shown transcript could scroll at the last frame, so
    /// scrolling back from the followed tail (`scroll == u16::MAX`) starts
    /// at the latest output.
    pub(crate) scroll_max: u16,
    other_draft: Draft,
    return_screen: Screen,
    appearance_return_screen: Screen,
    appearance_error: Option<String>,
    preferences_return_screen: Screen,
    pub(crate) preferences_error: Option<String>,
    saved_chats: [Chat; 6],
    /// A `coder issue-run` in this terminal (#11214).
    issue_run: Option<issue_run::Feed>,
    /// Where the watched run's rows start: after the conversation that led
    /// to it (the solver's Intaker chat), which the run must not overwrite.
    issue_run_base: usize,
    /// Whether the watched run's cost is already on the chat's total.
    issue_run_costed: bool,
}

#[derive(Default)]
struct Chat {
    draft: Draft,
    messages: Vec<String>,
    scroll: u16,
}

impl App {
    fn clear_disclosure(&mut self) {
        if let Some(desk) = self.disclosure_desk.take() {
            desk.close();
        }
        self.disclosure_event = None;
        self.disclosure_scroll = 0;
        self.disclosure_seen = false;
    }

    pub(crate) fn poll_disclosure(&mut self) {
        if !self.live.busy {
            self.clear_disclosure();
            return;
        }
        if let Some(desk) = &self.disclosure_desk {
            for event in desk.drain() {
                if event["event"] == "approval"
                    && (event["kind"] == "disclosure"
                        || event["kind"] == "computer"
                        || event["kind"] == "action")
                {
                    self.disclosure_event = Some(event);
                    self.disclosure_scroll = 0;
                    self.disclosure_seen = false;
                } else if event["event"] == "approval_answered"
                    && self
                        .disclosure_event
                        .as_ref()
                        .is_some_and(|pending| pending["id"] == event["id"])
                {
                    self.disclosure_event = None;
                }
            }
        }
    }

    fn answer_disclosure(&mut self, confirm: bool) {
        self.answer_disclosure_from(confirm, "terminal");
    }

    /// Answers the open question, saying where the answer came from
    /// (`terminal`, `phone`, or `web`) for the approval record (#11170).
    pub(crate) fn answer_disclosure_from(&mut self, confirm: bool, via: &str) {
        if let (Some(desk), Some(event)) = (&self.disclosure_desk, self.disclosure_event.take()) {
            let _ = desk.answer_from(
                &format!(
                    "{} {}",
                    if confirm { "confirm" } else { "reject" },
                    event["id"]
                ),
                via,
            );
        }
    }

    /// Scroll the transcript back by `rows`, from the latest output when it
    /// is following the tail.
    fn scroll_back(&mut self, rows: u16) {
        if self.scroll == u16::MAX {
            self.scroll = self.scroll_max;
        }
        self.scroll = self.scroll.saturating_sub(rows);
    }
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
                self.live.finish_partial();
                self.live.entries.push(live::Entry::Delegation {
                    id: id.clone(),
                    name: name.clone(),
                    task: task.clone(),
                    running: true,
                    output: serde_json::Value::Null,
                    progress: None,
                });
                self.delegations.push(live::Delegation {
                    id: id.clone(),
                    name: name.clone(),
                    task: task.clone(),
                    chat: live::Chat {
                        entries: vec![live::Entry::User(task)],
                        busy: true,
                        reply_started_at: Some(std::time::Instant::now()),
                        ..live::Chat::default()
                    },
                    started_at: self.elapsed_seconds,
                    elapsed_seconds: 0,
                    running: true,
                    draft: Draft::default(),
                    composer: Default::default(),
                    scroll: u16::MAX,
                    background: false,
                });
                self.delegations.len() - 1
            });
        if let bundled_runtime::RuntimeEvent::Delegation {
            id: nested,
            name,
            task,
            event,
        } = event
        {
            self.apply_delegation(format!("{id}/{nested}"), name, task, *event);
            return;
        }
        let child = &mut self.delegations[index];
        match event {
            bundled_runtime::RuntimeEvent::Tokens(tokens) => child.chat.tokens = tokens,
            bundled_runtime::RuntimeEvent::Usage(usage) => child.chat.round(*usage),
            bundled_runtime::RuntimeEvent::Harness(run) => child.chat.harness = Some(*run),
            bundled_runtime::RuntimeEvent::Text(text) => child.chat.partial.push_str(&text),
            bundled_runtime::RuntimeEvent::Model(model) => {
                child.chat.partial_model = live::model_slug(&model)
            }
            bundled_runtime::RuntimeEvent::Tool {
                name: tool,
                input,
                output,
                running,
            } if tool == "microcoder"
                || tool == "acp_subagent"
                || tool == "remote_delegate"
                || tool == solver_plugin::TOOL =>
            {
                child.running = running;
                child.chat.busy = running;
                if !running {
                    child.elapsed_seconds = self.elapsed_seconds.saturating_sub(child.started_at);
                    if let Some(cost) = long_session::output_cost(&output) {
                        child.chat.cost_usd = cost;
                    }
                    child.chat.tokens = output
                        .get("tokens")
                        .and_then(serde_json::Value::as_u64)
                        .or_else(|| {
                            output
                                .get("usage")
                                .and_then(|usage| usage.get("total_tokens"))
                                .and_then(serde_json::Value::as_u64)
                        })
                        .or_else(|| {
                            let usage = output.get("usage")?;
                            Some(
                                usage
                                    .get("input_tokens")?
                                    .as_u64()?
                                    .saturating_add(usage.get("output_tokens")?.as_u64()?),
                            )
                        })
                        .unwrap_or(child.chat.tokens);
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
            bundled_runtime::RuntimeEvent::Progress { step, complete } => {
                if let Some(live::Entry::Delegation { progress, .. }) =
                    self.live.entries.iter_mut().find(|entry| {
                        matches!(entry, live::Entry::Delegation { id: previous, .. } if previous == &id)
                    })
                {
                    *progress = Some((step, complete));
                }
            }
            bundled_runtime::RuntimeEvent::Delegation { .. }
            | bundled_runtime::RuntimeEvent::Plan(_) => {}
        }
        if self.selected_agent == Some(index) {
            self.scroll = u16::MAX;
        } else {
            child.scroll = u16::MAX;
        }
    }

    /// Shows a `coder issue-run` in this conversation as it happens.
    pub fn watch_issue_run(&mut self, feed: issue_run::Feed) {
        self.issue_run = Some(feed);
        self.issue_run_base = self.live.entries.len();
        self.issue_run_costed = false;
        self.scroll = u16::MAX;
    }

    /// Applies the issue run's latest changes. Returns whether any arrived.
    pub fn poll_issue_run(&mut self) -> bool {
        let Some(feed) = &self.issue_run else {
            return false;
        };
        let events = feed.drain();
        let changed = !events.is_empty();
        let mut ended = false;
        for event in events {
            ended |= matches!(event, issue_run::Event::Busy(false));
            self.apply_issue_run(event);
        }
        // A finished run lets the next one start in this conversation.
        if ended {
            self.issue_run = None;
        }
        changed
    }

    /// Applies one issue-run change to the conversation.
    pub fn apply_issue_run(&mut self, event: issue_run::Event) {
        match event {
            issue_run::Event::Set { index, entry } => {
                // The run's summary carries its whole cost: count it once.
                if !self.issue_run_costed
                    && let live::Entry::Tool {
                        name,
                        input,
                        running: false,
                        ..
                    } = &entry
                    && name == issue_run::SUMMARY
                    && let Some(usd) = input
                        .pointer("/cost/total_usd")
                        .and_then(serde_json::Value::as_f64)
                {
                    self.live.cost_usd += usd;
                    self.issue_run_costed = true;
                }
                let index = self.issue_run_base + index;
                if index < self.live.entries.len() {
                    self.live.entries[index] = entry;
                } else {
                    self.live.entries.push(entry);
                }
            }
            issue_run::Event::Busy(busy) => {
                self.live.busy = busy;
                self.live.reply_started_at = busy.then(std::time::Instant::now);
            }
            issue_run::Event::Notice(text) => self.notice = Some(text),
        }
    }

    pub fn load_plugin_settings(&mut self, store: plugin_store::Store) -> Result<(), String> {
        claude::load_choice(store.root());
        let appearance = self.appearance.load(store.clone());
        self.appearance_error = appearance.as_ref().err().cloned();
        let preferences = self.preferences.load(store.clone());
        self.preferences_error = preferences.as_ref().err().cloned();
        let result = self.plugins.load_settings(store);
        if self.screen == Screen::PluginSettings {
            self.open_plugin_settings();
        }
        result.and(appearance).and(preferences)
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
        if !self.persist_session(true) {
            return;
        }
        self.select_agent(None);
        self.model_picker = None;
        self.resume_picker = None;
        std::mem::swap(&mut self.draft, &mut self.other_draft);
        self.plugins.set_live(mode == Mode::Live);
        self.mode = mode;
        self.screen = Screen::Conversation;
        // Switching mode (/demo) leaves the transcript where the reader
        // left it: a chat scrolled back stays scrolled back, and one
        // following the newest output keeps following it.
        self.notice = None;
        self.slash_selected = 0;
        self.slash_hidden = false;
    }

    pub fn slash_hints(&self) -> Vec<slash::Command> {
        if self.composer.mode == composer_state::InputMode::Bash
            || self.slash_hidden
            || self.model_picker.is_some()
            || self.resume_picker.is_some()
            || self.screen != Screen::Conversation
        {
            return Vec::new();
        }
        slash::matches(&self.draft.text)
            .into_iter()
            .filter(|command| {
                *command != slash::Command::Models || self.plugins.enabled || self.on_claude()
            })
            // Saving chats to the account needs a sign-in (#11046).
            .filter(|command| *command != slash::Command::Sync || self.account.is_some())
            // Background agents run in live mode only (#11163).
            .filter(|command| *command != slash::Command::Agents || self.on_coder_loop())
            // Long sessions and background work are live only (#11179, #11177).
            .filter(|command| {
                !matches!(
                    command,
                    slash::Command::Compact
                        | slash::Command::Loop
                        | slash::Command::Schedule
                        | slash::Command::Shells
                ) || self.on_coder_loop()
            })
            // Offer only the sign-in step that applies (#11045).
            .filter(|command| *command != slash::Command::Login || self.account.is_none())
            .filter(|command| *command != slash::Command::Logout || self.account.is_some())
            .filter(|command| {
                *command != slash::Command::Brainstorm
                    || self.plugins.bundled.brainstorm.preferences.enabled
            })
            .collect()
    }

    /// In a Claude Code chat, a slash command Coder does not run there goes to
    /// Claude Code as the message, and Claude Code expands it (#11439): a
    /// skill such as `/review`, `/compact`, `/config KEY=VALUE`, `/loop`,
    /// and the rest. `/model NAME` picks the mode's model as /models does.
    /// Coder's own commands keep their meaning. Returns whether it took the
    /// draft.
    fn claude_slash(&mut self) -> bool {
        // A new chat's first message can be a Claude Code command too, when
        // the selector would route the chat there.
        let claude = self.on_claude()
            || (self.mode == Mode::Live
                && self.live.runs_on.is_none()
                && self.selected_agent.is_none()
                && self.draft.text.trim_start().starts_with('/')
                && harness_plugin::fixed_rule(&harness_plugin::candidates(
                    &harness_plugin::Facts::read(self),
                )) == harness_plugin::Harness::ClaudeCode);
        if !claude {
            return false;
        }
        let text = self.draft.text.trim().to_owned();
        let Some(rest) = text.strip_prefix('/') else {
            return false;
        };
        let (word, argument) = rest
            .split_once(char::is_whitespace)
            .map_or((rest, ""), |(word, argument)| (word, argument.trim()));
        // A path such as `/usr/bin/env` is a message, not a command.
        if word.is_empty()
            || !word
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':'))
        {
            return false;
        }
        if word == "model" && !argument.is_empty() {
            self.record_prompt();
            self.draft = Draft::default();
            self.composer = Default::default();
            self.composer_history.reset();
            let wanted = argument.to_lowercase();
            let picked = claude::MODELS.iter().find(|(id, name, _)| {
                *id == wanted
                    || name.to_lowercase() == wanted
                    || name
                        .split(' ')
                        .next()
                        .is_some_and(|short| short.to_lowercase() == wanted)
            });
            self.notice = Some(match picked {
                Some((id, name, _)) => match claude::choose(id, claude::choice().effort) {
                    Ok(()) => format!("claude now runs {name}."),
                    Err(error) => error,
                },
                None => {
                    let names: Vec<&str> =
                        claude::MODELS.iter().map(|(_, name, _)| *name).collect();
                    format!(
                        "claude offers {}. Type /models to pick one.",
                        names.join(", ")
                    )
                }
            });
            return true;
        }
        let coders_own = slash::parse(&format!("/{word}")).is_some_and(|command| {
            !matches!(
                command,
                slash::Command::Compact
                    | slash::Command::Loop
                    | slash::Command::Schedule
                    | slash::Command::Shells
                    | slash::Command::Agents
            )
        });
        if coders_own {
            return false;
        }
        self.submit_live();
        true
    }

    pub fn cancel_request(&mut self) {
        self.prompt_inbox.lock().unwrap().clear();
        self.acknowledge_prompts();
        self.prompt_inbox = Default::default();
        self.clear_disclosure();
        let brainstorm = self.brainstorm_job.take();
        self.brainstorm_conversation = None;
        if let Some(job) = &brainstorm {
            job.cancellation.cancel();
            if matches!(
                self.plugins.bundled.brainstorm.connection,
                plugins::Connection::Checking
            ) {
                self.plugins.bundled.brainstorm.connection = plugins::Connection::Unchecked;
            }
        }
        self.history.dirty |= self.live.busy;
        self.active_delegation = None;
        self.request_id = self.request_id.wrapping_add(1);
        self.request = None;
        self.checking_key = false;
        self.checking_jev = false;
        if self.live.busy {
            if !self.live.partial.is_empty() {
                self.live.entries.push(live::Entry::Assistant {
                    elapsed_ms: self.live.reply_elapsed_ms(),
                    text: std::mem::take(&mut self.live.partial),
                    model: self.live.partial_model.take(),
                });
            }
            self.live.partial_model = None;
            self.live.text_streamed = false;
            self.live.busy = false;
            self.live.notice = Some(
                if brainstorm.is_some() {
                    "Brainstorm lookup cancelled."
                } else {
                    "Reply stopped."
                }
                .into(),
            );
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
        if !self.on_coder_loop() {
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
        // Only a mode that runs turns accepts their updates.
        if update.id() != self.request_id || !matches!(self.mode, Mode::Live | Mode::Solver) {
            return;
        }
        self.live.stamp();
        let finished = matches!(update, live::Update::Finished { result: Ok(_), .. });
        self.apply_live_update(update);
        self.live.stamp();
        // The Intaker named the issue: the solver run starts on it.
        if finished && self.mode == Mode::Solver && self.issue_run.is_none() {
            let chosen = self
                .live
                .entries
                .iter()
                .rev()
                .find_map(|entry| match entry {
                    live::Entry::Assistant { text, .. } => Some(solver::chosen(text)),
                    live::Entry::User(_) => Some(None),
                    _ => None,
                });
            if let Some(Some((repo, issue))) = chosen {
                self.start_solver(repo, issue);
            }
        }
    }

    fn apply_live_update(&mut self, update: live::Update) {
        // A summary request's result (#11179) never reaches the transcript.
        let Some(update) = self.finish_compaction(update) else {
            return;
        };
        self.acknowledge_prompts();
        self.history.dirty |= self.live.busy;
        match update {
            live::Update::BrainstormFinished {
                generation, result, ..
            } => {
                let Some(job) = self.brainstorm_job.take() else {
                    return;
                };
                let conversation = self.brainstorm_conversation.take();
                if generation != job.generation
                    || generation != self.plugins.bundled.brainstorm.generation
                {
                    self.live.busy = false;
                    if let Some(conversation) = &conversation {
                        self.finish_brainstorm_conversation(conversation);
                        if let Some(chat) = conversation.chat_mut(self) {
                            let reason = "Brainstorm settings changed before this result returned.";
                            chat.stop_tools(reason);
                            chat.notice = Some(reason.into());
                        }
                    }
                    return;
                }
                if matches!(job.command, brainstorm::Command::Test) {
                    self.plugins.bundled.brainstorm.connection = match result {
                        Ok(brainstorm::Outcome::Discovery(discovery))
                            if discovery.search_supported && discovery.rank_supported =>
                        {
                            self.plugins.bundled.brainstorm.discovery = Some(discovery);
                            plugins::Connection::Verified
                        }
                        Ok(_) => plugins::Connection::Failed(
                            "The origin does not advertise both required public lookup algorithms."
                                .into(),
                        ),
                        Err(error) => plugins::Connection::Failed(error.to_string()),
                    };
                    return;
                }
                let Some(conversation) = conversation else {
                    self.live.busy = false;
                    self.live.notice =
                        Some("The Brainstorm lookup conversation is unavailable.".into());
                    return;
                };
                self.finish_brainstorm_conversation(&conversation);
                let mut output = match result {
                    Ok(outcome) => brainstorm::output(outcome),
                    Err(error) => serde_json::json!({ "error": error.to_string(), "state": error,
                        "recipient": job.origin, "operation": job.command.name(), "completed_at_ms": atif::now_ms() }),
                };
                if let Some(reference) = job.input_reference() {
                    output["input_ref"] = serde_json::json!(reference);
                }
                let Some(chat) = conversation.chat_mut(self) else {
                    self.live.notice =
                        Some("The Brainstorm lookup conversation is unavailable.".into());
                    return;
                };
                chat.tool(
                    job.command.name().into(),
                    job.command.input(&job.origin),
                    output,
                    false,
                );
                chat.notice = None;
                self.live.notice = None;
                self.history.dirty = true;
                self.scroll_brainstorm_conversation(&conversation);
            }
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
                // What the model wrote before this call shows above it, not
                // at the end of the turn.
                self.live.finish_partial();
                if let Some(live::Entry::Tool { input: previous_input, output: previous_output, running: previous_running, .. }) = self.live.entries.iter_mut().rev().find(|entry| {
                    matches!(entry, live::Entry::Tool { name: previous, input: previous_input, running: true, .. } if previous == &name && (input.is_null() || previous_input == &input))
                }) {
                    if !input.is_null() { *previous_input = input; }
                    *previous_output = output;
                    *previous_running = running;
                } else {
                    self.live.entries.push(live::Entry::Tool { name, input, output, running });
                }
            }
            live::Update::Usage { usage, .. } if self.live.busy => {
                // The totals move as each call ends, not only when the turn
                // finishes.
                let tokens = usage.prompt_tokens.saturating_add(usage.completion_tokens);
                let cost = usage.cost.unwrap_or_default();
                self.live.tokens = self.live.tokens.saturating_add(tokens);
                self.live.cost_usd += cost;
                self.live.turn_counted.0 = self.live.turn_counted.0.saturating_add(tokens);
                self.live.turn_counted.1 += cost;
                self.live.round(usage);
            }
            live::Update::Harness { run, .. } if self.live.busy => {
                self.live.harness = Some(run);
            }
            live::Update::Delta { text, .. } if self.live.busy => {
                self.live.text_streamed = true;
                self.live.partial.push_str(&text);
                self.scroll_main_to_end();
            }
            live::Update::Model { model, .. } if self.live.busy => {
                if model == provider::PINNED_MISSED {
                    self.live.pinned_missed = true;
                    return;
                }
                if model == "openagents/fallback" {
                    self.active_options = models::GenerationOptions::default();
                    return;
                }
                self.live.partial_model =
                    live::model_slug(&model).map(|model| self.active_options.slug(&model));
            }
            live::Update::Finished { result, .. } if self.live.busy => {
                self.live.busy = false;
                let text_streamed = std::mem::take(&mut self.live.text_streamed);
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
                        // Only what the per-call reports didn't already add.
                        let (counted_tokens, counted_cost) =
                            std::mem::take(&mut self.live.turn_counted);
                        self.live.tokens = self.live.tokens.saturating_add(
                            reply.usage.total_tokens.saturating_sub(counted_tokens),
                        );
                        if let Some(cost) = reply.usage.cost.filter(|cost| cost.is_finite()) {
                            self.live.cost_usd += (cost - counted_cost).max(0.0);
                        }
                        let model = live::model_slug(&reply.model)
                            .map(|model| self.active_options.slug(&model));
                        if text_streamed {
                            // The turn's earlier text is already above its
                            // tool calls; only what came after the last one
                            // is left.
                            self.live.partial_model = model;
                            self.live.finish_partial();
                        } else {
                            self.live.entries.push(live::Entry::Assistant {
                                elapsed_ms: self.live.reply_elapsed_ms(),
                                text: reply.text,
                                model,
                            });
                        }
                        self.live.partial.clear();
                        self.live.partial_model = None;
                        self.live.notice = None;
                        self.live.pinned_missed = false;
                        if self.plugins.enabled && self.plugins.key_configured {
                            self.plugins.connection = plugins::Connection::Verified;
                        }
                    }
                    Err(error) => {
                        self.live.turn_counted = (0, 0.0);
                        if error.contains("HTTP 401") {
                            self.plugins.connection = plugins::Connection::Failed(error.clone());
                        }
                        if !self.live.partial.is_empty() {
                            self.live.entries.push(live::Entry::Assistant {
                                elapsed_ms: self.live.reply_elapsed_ms(),
                                text: std::mem::take(&mut self.live.partial),
                                model: self.live.partial_model.take(),
                            });
                        }
                        self.live.partial_model = None;
                        self.live.notice = Some(error);
                        // A usage limit pauses the chat until it resets.
                        self.note_turn_error();
                        // The model the person chose failed or refused the
                        // turn (#11132): Try another model is one Enter away.
                        if std::mem::take(&mut self.live.pinned_missed)
                            && self.draft.text.trim().is_empty()
                        {
                            self.draft = Draft {
                                text: TRY_ANOTHER_MODEL.into(),
                                cursor: TRY_ANOTHER_MODEL.len(),
                            };
                        }
                    }
                }
                self.scroll_main_to_end();
            }
            _ => {}
        }
    }

    /// Ctrl+O: switch between the compact and verbose transcripts. The
    /// transcript cache rebuilds on the next frame, because the view is part
    /// of what each entry's rows depend on.
    pub fn toggle_view(&mut self) {
        self.view = match self.view {
            ViewMode::Compact => ViewMode::Verbose,
            ViewMode::Verbose => ViewMode::Compact,
        };
        self.live.cache = Default::default();
        for delegation in &mut self.delegations {
            delegation.chat.cache = Default::default();
        }
    }

    fn command(&mut self, command: slash::Command) {
        self.record_prompt();
        if command == slash::Command::Resume {
            if self.resume(None) {
                self.draft = Draft::default();
                self.composer = Default::default();
                self.composer_history.reset();
            }
            return;
        }
        self.draft = Draft::default();
        self.composer = Default::default();
        self.composer_history.reset();
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
            slash::Command::Appearance => self.open_appearance(),
            slash::Command::Preferences => self.open_preferences(),
            slash::Command::Models => self.open_models(),
            slash::Command::Export => self.export(None),
            slash::Command::Resume => unreachable!("Resume is handled before clearing the draft"),
            slash::Command::Brainstorm => self.notice = Some(brainstorm::USAGE.into()),
            slash::Command::Help => self.notice = Some(slash::help()),
            slash::Command::Login => self.login(),
            slash::Command::Logout => self.logout(),
            slash::Command::Sync => self.sync_command(""),
            slash::Command::Memory => self.memory_command(""),
            slash::Command::Agents => self.open_agents_panel(),
            slash::Command::Compact => self.compact_command(),
            slash::Command::Loop => self.loop_command(""),
            slash::Command::Schedule => self.schedule_command(""),
            slash::Command::Shells => self.shells_command(),
        }
    }

    /// `/memory` lists saved notes; `/memory forget NAME` deletes one (#11176).
    fn memory_command(&mut self, argument: &str) {
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
        let Some(memory) = memory::Memory::discover(&cwd) else {
            self.notice = Some("Memory is off on this computer (OPENAGENTS_MEMORY=off).".into());
            return;
        };
        let argument = argument.trim();
        self.notice = Some(if argument.is_empty() {
            memory.listing()
        } else if let Some(name) = argument
            .strip_prefix("forget ")
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            match memory.forget(name, None) {
                Ok(entry) => format!("Forgot {}.", entry.name),
                Err(error) => error,
            }
        } else {
            "Use /memory to list what Coder remembers, or /memory forget NAME.".into()
        });
    }

    fn export(&mut self, path: Option<&std::path::Path>) {
        self.pending_export_path = None;
        self.export_notice_expiry = None;
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
        if self.pending_export_path.is_some() {
            self.time_export_notice();
        }
        self.draft = Draft::default();
        self.composer = Default::default();
        self.composer_history.reset();
    }

    /// Copy the input text Cmd+C or Cmd+X took, through the terminal's
    /// clipboard adapter (OSC 52).
    pub fn copy_pending(&mut self, copy: impl FnOnce(&str) -> std::io::Result<()>) {
        let Some(text) = self.pending_copy.take() else {
            return;
        };
        if let Err(error) = copy(&text) {
            self.notice = Some(format!("Cannot copy to clipboard: {error}."));
        }
    }

    /// Selecting transcript text with the mouse, as grok-build does it:
    /// drag to select (the view scrolls while the pointer is held past
    /// the top or bottom), double-click a word, triple-click a line. The
    /// selection is copied when the button comes up, and again on Cmd+C
    /// or Ctrl+Shift+C; Esc puts it down and still does what Esc does.
    /// The wheel and Run boxes keep scrolling as before. Returns whether
    /// the event was used up here.
    fn transcript_select(&mut self, event: &Event) -> bool {
        use crossterm::event::MouseButton;
        let composer_selected = self.composer_selected;
        let chat = self
            .selected_agent
            .and_then(|index| self.delegations.get_mut(index))
            .map_or(&mut self.live, |agent| &mut agent.chat);
        let cache = &mut chat.cache;
        let copied = match event {
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    cache.drag = None;
                    let Some(at) = cache.point_at(mouse.column, mouse.row, false) else {
                        // A click outside the transcript puts the selection down.
                        cache.selection = None;
                        return false;
                    };
                    match self.clicks.press(std::time::Instant::now(), at) {
                        2 => cache.select_word(at),
                        3 => cache.select_line(at),
                        _ => {
                            cache.selection = None;
                            cache.drag = Some(ui::Drag {
                                anchor: at,
                                mouse: (mouse.column, mouse.row),
                            });
                            return true;
                        }
                    }
                    cache.selected_text()
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    let Some(drag) = &mut cache.drag else {
                        return false;
                    };
                    drag.mouse = (mouse.column, mouse.row);
                    cache.follow_drag();
                    return true;
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    if cache.drag.take().is_none() {
                        return false;
                    }
                    cache.selected_text()
                }
                _ => return false,
            },
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if cache.selection.is_none() {
                    return false;
                }
                let letter_c =
                    matches!(key.code, KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'c'));
                let chord = key.modifiers == KeyModifiers::SUPER
                    || key.modifiers == KeyModifiers::CONTROL | KeyModifiers::SHIFT;
                if letter_c && chord && !composer_selected {
                    cache.selected_text()
                } else if key.code == KeyCode::Esc {
                    // Esc puts the selection down and still stops a reply.
                    cache.selection = None;
                    return false;
                } else {
                    return false;
                }
            }
            _ => return false,
        };
        if let Some(text) = copied {
            self.pending_copy = Some(text);
            self.copied_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
        }
        true
    }

    /// Keep scrolling while a selecting drag is held past the transcript's
    /// top or bottom edge; called on each tick.
    fn drag_scroll(&mut self) {
        let chat = self
            .selected_agent
            .and_then(|index| self.delegations.get(index))
            .map_or(&self.live, |agent| &agent.chat);
        let rows = chat.cache.drag_scroll();
        if rows < 0 {
            self.scroll_back(rows.unsigned_abs());
        } else if rows > 0 {
            self.scroll = self.scroll.saturating_add(rows as u16);
        }
    }

    /// Select-all in the input and what follows it. Cmd+A needs a terminal
    /// that reports Cmd keys (the kitty keyboard protocol); Ctrl+Shift+A is
    /// the same where the terminal tells it from Ctrl+A. Ctrl+A stays line
    /// start. Returns whether the key was used up here.
    fn composer_selection_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        let command = key.modifiers == KeyModifiers::SUPER;
        let letter =
            |want: char| matches!(key.code, KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&want));
        if letter('a') && (command || key.modifiers == KeyModifiers::CONTROL | KeyModifiers::SHIFT)
        {
            self.composer_selected = !self.draft.text.is_empty();
            self.draft.cursor = self.draft.text.len();
            return true;
        }
        if !self.composer_selected {
            return false;
        }
        self.composer_selected = false;
        if self.draft.text.is_empty() {
            return false;
        }
        if command && letter('c') {
            self.pending_copy = Some(self.draft.text.clone());
            self.composer_selected = true;
            return true;
        }
        let clear = |app: &mut Self| {
            app.draft = Draft::default();
            app.composer_history.reset();
            app.slash_selected = 0;
            app.slash_hidden = false;
        };
        if command && letter('x') {
            self.pending_copy = Some(self.draft.text.clone());
            clear(self);
            return true;
        }
        match key.code {
            KeyCode::Backspace | KeyCode::Delete => {
                clear(self);
                true
            }
            KeyCode::Char(_)
                if !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
            {
                // The typed character replaces the selection below.
                clear(self);
                false
            }
            _ => false,
        }
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
        self.time_export_notice();
    }

    fn time_export_notice(&mut self) {
        self.export_notice_expiry = self.notice.clone().map(|notice| {
            (
                std::time::Instant::now() + std::time::Duration::from_secs(3),
                notice,
            )
        });
    }

    fn expire_export_notice(&mut self, now: std::time::Instant) {
        if self
            .export_notice_expiry
            .as_ref()
            .is_some_and(|(deadline, _)| now >= *deadline)
        {
            let (_, notice) = self.export_notice_expiry.take().unwrap();
            // A subsequent command may have replaced the export notification.
            if self.notice.as_ref() == Some(&notice) {
                self.notice = None;
            }
        }
    }

    pub fn submit(&mut self, text: &str, cwd: &std::path::Path) {
        self.cwd = Some(cwd.to_owned());
        self.draft.text = text.into();
        self.draft.cursor = text.len();
        self.submit_live();
    }

    fn submit_brainstorm_command(&mut self) -> bool {
        let Some(command) = brainstorm::parse(self.draft.text.trim()) else {
            return false;
        };
        match command {
            Ok(command) => self.start_brainstorm(command),
            Err(error) => self.live.notice = Some(error.into()),
        }
        true
    }

    fn start_brainstorm(&mut self, command: brainstorm::Command) {
        if self.model_owned_input && !matches!(command, brainstorm::Command::Test) {
            self.live.notice = Some("A model-owned CLI prompt cannot admit a Brainstorm disclosure. Use the native Brainstorm tool and its exact input and recipient confirmation.".into());
            return;
        }
        if self.live.busy || self.checking_key || self.checking_jev || self.brainstorm_job.is_some()
        {
            self.live.notice = Some("Wait for the current work or press Esc to stop it.".into());
            return;
        }
        let settings = &self.plugins.bundled.brainstorm;
        if !settings.preferences.enabled {
            self.live.notice = Some("Enable Brainstorm in /plugins before a public lookup.".into());
            return;
        }
        if self.mode == Mode::Demo {
            if matches!(command, brainstorm::Command::Test) {
                self.plugins.bundled.brainstorm.fixture = true;
            } else {
                self.messages.push(std::mem::take(&mut self.draft.text));
                self.messages
                    .push(brainstorm::summary(&serde_json::json!({ "fixture": true })));
                self.draft.cursor = 0;
            }
            return;
        }
        let job = match settings.job(command) {
            Ok(job) => job,
            Err(error) => {
                self.live.notice = Some(error);
                return;
            }
        };
        let conversation = if matches!(job.command, brainstorm::Command::Test) {
            let edited = self.plugins.bundled.brainstorm.edited_preferences();
            if !edited
                .as_ref()
                .is_ok_and(|edited| edited == &self.plugins.bundled.brainstorm.preferences)
            {
                self.plugins.bundled.brainstorm.error =
                    Some("Save a valid origin before testing its connection.".into());
                return;
            }
            None
        } else {
            if !self.ensure_session() {
                return;
            }
            let Some(conversation) = brainstorm::Conversation::selected(self) else {
                self.live.notice = Some("The selected lookup conversation is unavailable.".into());
                return;
            };
            Some(conversation)
        };
        self.cancel_request();
        if matches!(job.command, brainstorm::Command::Test) {
            self.plugins.bundled.brainstorm.connection = plugins::Connection::Checking;
            self.plugins.bundled.brainstorm.discovery = None;
        } else if let Some(conversation) = &conversation {
            let text = std::mem::take(&mut self.draft.text);
            self.draft.cursor = 0;
            let Some(chat) = conversation.chat_mut(self) else {
                self.draft.text = text;
                self.draft.cursor = self.draft.text.len();
                self.live.notice = Some("The selected lookup conversation is unavailable.".into());
                return;
            };
            chat.entries.push(live::Entry::User(text));
            chat.tool(
                job.command.name().into(),
                job.command.input(&job.origin),
                serde_json::Value::Null,
                true,
            );
            chat.busy = true;
            chat.reply_started_at = Some(std::time::Instant::now());
            chat.notice = None;
            if let brainstorm::Conversation::Delegation(id) = conversation {
                if let Some(child) = self.delegations.iter_mut().find(|child| &child.id == id) {
                    child.running = true;
                    child.started_at = self.elapsed_seconds;
                }
            }
            self.live.busy = true;
            self.live.notice = None;
            self.history.dirty = true;
            self.scroll_brainstorm_conversation(conversation);
        }
        self.brainstorm_conversation = conversation;
        self.brainstorm_job = Some(job.clone());
        self.request = Some(live::Request {
            id: self.request_id,
            key: model_access::ApiKey::new(""),
            kind: live::Work::Brainstorm { job },
        });
    }

    fn finish_brainstorm_conversation(&mut self, conversation: &brainstorm::Conversation) {
        self.live.busy = false;
        if let brainstorm::Conversation::Delegation(id) = conversation {
            if let Some(child) = self.delegations.iter_mut().find(|child| &child.id == id) {
                child.running = false;
                child.chat.busy = false;
                child.elapsed_seconds = self.elapsed_seconds.saturating_sub(child.started_at);
            }
        }
    }

    fn scroll_brainstorm_conversation(&mut self, conversation: &brainstorm::Conversation) {
        match conversation {
            brainstorm::Conversation::Main => self.scroll_main_to_end(),
            brainstorm::Conversation::Delegation(id) => {
                if let Some(index) = self.delegations.iter().position(|child| &child.id == id) {
                    self.delegations[index].scroll = u16::MAX;
                    if self.selected_agent == Some(index) {
                        self.scroll = u16::MAX;
                    }
                }
            }
        }
    }

    pub fn check_brainstorm_connection(&mut self) {
        self.start_brainstorm(brainstorm::Command::Test);
        if self.brainstorm_job.is_none() && self.on_coder_loop() {
            self.plugins.bundled.brainstorm.error = self.live.notice.clone();
        }
    }

    /// A Claude Code turn: the person's message to the chat's own Claude Code
    /// process ([`crate::claude_chat`]), with Claude Code's whole toolset and
    /// every call approved, on the model picked in /models
    /// ([`crate::claude`]). A message sent while a turn runs goes to Claude
    /// Code at once rather than waiting. Pictures go as pictures. It is the
    /// same one conversation with the person; only the harness behind its
    /// turns differs from Coder's own loop.
    fn submit_claude(&mut self) {
        if self.draft.text.trim().is_empty() && self.composer.images.is_empty() {
            return;
        }
        if self.checking_key || self.checking_jev {
            self.queue_prompt();
            return;
        }
        if self.live.busy && !self.claude_turn {
            self.queue_prompt();
            return;
        }
        let midturn = self.live.busy;
        if let Err(error) = self.attach_to_prompt() {
            self.live.notice = Some(error);
            return;
        }
        self.record_prompt();
        if !midturn {
            self.cancel_request();
        }
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
        let text = std::mem::take(&mut self.draft.text);
        self.draft.cursor = 0;
        self.composer = Default::default();
        self.screen = Screen::Conversation;
        self.live.finish_partial();
        self.live.entries.push(live::Entry::User(text));
        let messages = self.live.messages();
        let session = self.live.claude_resume();
        self.history.dirty = true;
        self.scroll = u16::MAX;
        if !midturn {
            self.begin_claude_turn();
        }
        self.request = Some(live::Request {
            id: self.request_id,
            key: model_access::ApiKey::new(""),
            kind: live::Work::Claude {
                messages,
                cwd,
                tools: Box::new(crate::claude_tools::Tools::new(self.agent_execution())),
                session,
            },
        });
    }

    /// The chat shows a claude turn under way: one the person started, or
    /// one Claude Code started itself when a background job it left running
    /// came back. Returns false while the chat is busy with something else.
    pub(crate) fn begin_claude_turn(&mut self) -> bool {
        if self.live.busy || self.request.is_some() {
            return false;
        }
        self.live.finish_partial();
        self.live.notice = None;
        self.live.partial.clear();
        self.live.partial_model = None;
        self.live.text_streamed = false;
        self.live.busy = true;
        self.live.pinned_missed = false;
        self.live.reply_started_at = Some(std::time::Instant::now());
        self.history.dirty = true;
        self.scroll = u16::MAX;
        true
    }

    /// The solver conversation: an issue reference starts a run in this
    /// conversation; anything else gets the Intaker's question back.
    fn submit_solver(&mut self) {
        if self.draft.text.trim().is_empty() {
            return;
        }
        self.record_prompt();
        let text = self.draft.text.clone();
        self.draft = Draft::default();
        self.composer = Default::default();
        self.screen = Screen::Conversation;
        let Some((repo, issue)) = solver::issue_reference(&text) else {
            // Not an issue number: the Intaker talks it through, with gh.
            self.intake_solver(text);
            return;
        };
        self.start_solver(repo, issue);
    }

    /// One turn of solver's conversational Intaker on `text`.
    fn intake_solver(&mut self, text: String) {
        if self.live.busy || self.issue_run.is_some() {
            self.notice = Some("A solver run is already working here.".into());
            return;
        }
        self.cancel_request();
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
        self.live.finish_partial();
        self.live.entries.push(live::Entry::User(text));
        let messages = self.live.messages();
        self.live.notice = None;
        self.live.partial.clear();
        self.live.partial_model = None;
        self.live.text_streamed = false;
        self.live.busy = true;
        self.live.reply_started_at = Some(std::time::Instant::now());
        self.history.dirty = true;
        self.scroll = u16::MAX;
        self.request = Some(live::Request {
            id: self.request_id,
            key: model_access::ApiKey::new(""),
            kind: live::Work::SolverIntake {
                messages,
                cwd,
                github: solver::DEFAULT_GITHUB.into(),
            },
        });
    }

    /// Starts a solver run on `issue` (in `repo`, or the default one).
    fn start_solver(&mut self, repo: Option<String>, issue: u64) {
        if self.live.busy || self.issue_run.is_some() {
            self.notice = Some("A solver run is already working here.".into());
            return;
        }
        let mut args = vec![issue.to_string()];
        if let Some(repo) = repo {
            args.push("--github".into());
            args.push(repo);
        }
        match solver::Options::parse(&args) {
            Ok(options) => self.watch_issue_run(solver::start(options)),
            Err(why) => self.notice = Some(why),
        }
    }

    pub fn submit_live(&mut self) {
        if self.mode == Mode::Solver {
            self.submit_solver();
            return;
        }
        // The turn runs on the harness the chat was routed to; its first
        // turn routes it ([`harness_plugin`]). Claude Code runs turns of its
        // own, on the Claude Agent SDK.
        if self.mode == Mode::Live
            && self.selected_agent.is_none()
            && (!self.draft.text.trim().is_empty() || !self.composer.images.is_empty())
            && self.route_turn() == harness_plugin::Harness::ClaudeCode
        {
            self.submit_claude();
            return;
        }
        if self.live.busy
            || self.checking_key
            || self.checking_jev
            || self.brainstorm_job.is_some()
            || (self.selected_agent.is_none() && self.long_session.paused())
        {
            self.queue_prompt();
            return;
        }
        if self.submit_brainstorm_command() {
            return;
        }
        if !self.ensure_session() {
            return;
        }
        let attached = match self.attach_to_prompt() {
            Ok(count) => count,
            Err(error) => {
                self.live.notice = Some(error);
                return;
            }
        };
        if let Some(index) = self.selected_agent {
            if self
                .delegations
                .get(index)
                .is_some_and(|child| child.background)
            {
                self.message_agent(index);
            } else {
                self.submit_delegation(index);
            }
            return;
        }
        let key = self
            .plugins
            .key_for_request()
            .filter(|_| self.plugins.enabled);
        self.record_prompt();
        self.cancel_request();
        self.live
            .entries
            .push(live::Entry::User(std::mem::take(&mut self.draft.text)));
        self.draft.cursor = 0;
        self.composer = Default::default();
        self.live.notice = None;
        if attached > 0 && !(key.is_some() && models::accepts_images(&self.plugins.model)) {
            self.live.notice = Some(attachments::TEXT_ONLY_NOTICE.into());
        }
        self.live.partial.clear();
        self.live.partial_model = None;
        self.live.text_streamed = false;
        self.live.busy = true;
        self.live.pinned_missed = false;
        self.live.reply_started_at = Some(std::time::Instant::now());
        self.history.dirty = true;
        self.active_options = if key.is_some() {
            self.plugins.options.clone()
        } else {
            models::GenerationOptions::default()
        };
        self.screen = Screen::Conversation;
        self.scroll = u16::MAX;
        let mut execution = self
            .plugins
            .execution_settings(self.cwd.clone().unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            }));
        execution.instructions = self.live.instructions.clone();
        execution.prompt_inbox = Some(self.prompt_inbox.clone());
        execution.boundary = self.boundary.clone();
        // Google Drive through the signed-in account's Connections (#11238).
        execution.connections = self
            .account_dir
            .as_deref()
            .and_then(connection_tools::signed_in);
        execution.fleet = Some(self.fleet_host());
        // The chat keeps the harness its first tool turn chose (#11289).
        execution.harness = self.live.harness.clone();
        // An `auto` turn orders its doors by the person's /preferences
        // (#11374).
        execution.favor = crate::route::Favor {
            cost: self.preferences.cost,
            speed: self.preferences.speed,
            intelligence: self.preferences.intelligence,
        };
        if key.is_some()
            && (execution.brainstorm.is_some() || execution.cli)
            && execution.disclosure_desk.is_none()
            && self.interactive_disclosures
        {
            let desk = approval::Desk::new();
            execution.disclosure_desk = Some(desk.clone());
            self.disclosure_desk = Some(desk);
        }
        // A model the person chose keeps its own path (#11132); `auto`
        // routes across every connected door.
        let kind = if key.is_some() && models::pinned(&self.plugins.model) {
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
                options: self.plugins.options.clone(),
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
        let remote = if name.ends_with("@boat") || name.ends_with("@gce") {
            self.live.entries.iter().find_map(|entry| match entry {
                live::Entry::Delegation {
                    id: entry_id,
                    output,
                    ..
                } if entry_id == &id => output["job"].as_str().map(str::to_owned),
                _ => None,
            })
        } else {
            None
        };
        let (tool, arguments) = if let Some(job) = remote {
            let boat = name.ends_with("@boat");
            if !(if boat {
                execution.boat.enabled
            } else {
                execution.gce.enabled
            }) {
                self.draft.text = text;
                self.live.notice =
                    Some("Enable this cloud plugin in /plugins before continuing its job.".into());
                return;
            }
            (
                if boat {
                    "boat_job".to_owned()
                } else {
                    "gce_job".to_owned()
                },
                serde_json::json!({"operation":"continue","job":job,"message":task}),
            )
        } else if name == "microcoder" {
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
        self.draft.text = text.clone();
        self.record_prompt();
        self.draft.text.clear();
        self.composer = Default::default();
        self.cancel_request();
        self.active_delegation = Some(id.clone());
        self.active_options = self.plugins.options.clone();
        let child = &mut self.delegations[index];
        child.chat.entries.push(live::Entry::User(text));
        child.chat.notice = None;
        child.chat.busy = true;
        child.chat.reply_started_at = Some(std::time::Instant::now());
        child.running = true;
        child.started_at = self.elapsed_seconds;
        self.live.busy = true;
        self.history.dirty = true;
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

    pub fn open_appearance(&mut self) {
        self.resume_picker = None;
        self.model_picker = None;
        if self.screen != Screen::Appearance {
            self.appearance_return_screen = self.screen;
        }
        self.screen = Screen::Appearance;
    }

    pub fn open_preferences(&mut self) {
        self.resume_picker = None;
        self.model_picker = None;
        if self.screen != Screen::Preferences {
            self.preferences_return_screen = self.screen;
        }
        self.preferences.refresh_routing();
        self.screen = Screen::Preferences;
    }

    pub fn open_plugins(&mut self) {
        self.plugins.catalog_revision = self.plugins.catalog_revision.wrapping_add(1);
        self.resume_picker = None;
        if !matches!(self.screen, Screen::Plugins | Screen::PluginSettings) {
            self.return_screen = self.screen;
        }
        self.screen = Screen::Plugins;
    }

    pub fn open_models(&mut self) {
        self.resume_picker = None;
        // A Claude Code chat picks among Claude models on this computer's
        // login, with no OpenRouter key.
        if self.on_claude() {
            let picked = claude::choice();
            self.model_picker = Some(models::Picker::new(
                claude::catalog(),
                claude::PICKER,
                &picked.model,
                models::GenerationOptions {
                    reasoning: picked.effort,
                    max_tokens: None,
                },
                false,
            ));
            return;
        }
        if !self.plugins.enabled {
            self.notice = Some("Turn on OpenRouter BYOK in /plugins to choose a model.".into());
            return;
        }
        self.model_picker = Some(models::Picker::new(
            models::openrouter_catalog(),
            models::OPENROUTER_PLUGIN,
            &self.plugins.model,
            self.plugins.options.clone(),
            matches!(self.mode, Mode::Live | Mode::Solver),
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
                .begin_cloud(coder_cloud::Placement::Boat),
            "gce-cloud" => self
                .plugins
                .bundled
                .begin_cloud(coder_cloud::Placement::Gce),
            "acp-subagents" => self.plugins.bundled.begin_acp(),
            brainstorm::PLUGIN => self.plugins.bundled.brainstorm.begin(),
            _ => {}
        }
        self.screen = Screen::PluginSettings;
    }

    pub fn check_jev_key(&mut self) {
        if !self.on_coder_loop() {
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
        self.drag_scroll();
        self.poll_prompt_history();
        self.expire_export_notice(std::time::Instant::now());
        self.poll_login();
        self.poll_sync();
        self.animation_frame = self.animation_frame.wrapping_add(1) % 8;
    }

    /// The state this run reports to a program-status (OSC 7501) terminal:
    /// `blocked` while a question waits on the person, with the question's
    /// first line as the message; `working` while a turn runs; `idle`
    /// otherwise.
    pub fn program_status(&self) -> (&'static str, Option<String>) {
        if let Some(event) = &self.disclosure_event {
            let msg = supervise::question_text(event)
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned();
            return ("blocked", Some(msg));
        }
        (if self.live.busy { "working" } else { "idle" }, None)
    }

    /// The delegations the rail under the input lists, by index: every one
    /// still running, the one selected, and one that finished in the last
    /// [`RAIL_KEEP_SECONDS`]. The agents panel still lists them all.
    #[must_use]
    pub fn rail_agents(&self) -> Vec<usize> {
        self.delegations
            .iter()
            .enumerate()
            .filter(|(index, agent)| {
                agent.running
                    || self.selected_agent == Some(*index)
                    || self.elapsed_seconds
                        < agent
                            .started_at
                            .saturating_add(agent.elapsed_seconds)
                            .saturating_add(RAIL_KEEP_SECONDS)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn select_agent(&mut self, selected: Option<usize>) {
        if self.selected_agent != selected {
            self.composer_history.reset();
        }
        if self.selected_agent == selected {
            return;
        }
        if self.mode == Mode::Live {
            if selected.is_some_and(|index| index >= self.delegations.len()) {
                return;
            }
            if let Some(previous) = self.selected_agent {
                self.delegations[previous].draft = std::mem::take(&mut self.draft);
                self.delegations[previous].composer = std::mem::take(&mut self.composer);
                self.delegations[previous].scroll = self.scroll;
            } else {
                self.main_draft = std::mem::take(&mut self.draft);
                self.main_composer = std::mem::take(&mut self.composer);
                self.main_scroll = self.scroll;
            }
            if let Some(next) = selected {
                self.draft = std::mem::take(&mut self.delegations[next].draft);
                self.composer = std::mem::take(&mut self.delegations[next].composer);
                self.scroll = self.delegations[next].scroll;
            } else {
                self.draft = std::mem::take(&mut self.main_draft);
                self.composer = std::mem::take(&mut self.main_composer);
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
        if self.disclosure_event.is_some() {
            if let Event::Key(key) = event {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    return true;
                }
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.cancel_request();
                    return false;
                }
                match key.code {
                    KeyCode::Char('y' | 'Y') if self.disclosure_seen => {
                        self.answer_disclosure(true)
                    }
                    KeyCode::Char('n' | 'N') => self.answer_disclosure(false),
                    KeyCode::Esc => {
                        self.answer_disclosure(false);
                        self.cancel_request();
                    }
                    KeyCode::Down | KeyCode::PageDown => {
                        self.disclosure_scroll = self.disclosure_scroll.saturating_add(5)
                    }
                    KeyCode::Up | KeyCode::PageUp => {
                        self.disclosure_scroll = self.disclosure_scroll.saturating_sub(5)
                    }
                    KeyCode::End => self.disclosure_scroll = u16::MAX,
                    KeyCode::Home => self.disclosure_scroll = 0,
                    _ => {}
                }
            }
            return true;
        }
        if matches!(self.mode, Mode::Live | Mode::Solver)
            && self.screen == Screen::Conversation
            && self.model_picker.is_none()
            && self.resume_picker.is_none()
            && self.agents_panel.is_none()
            && self.transcript_select(&event)
        {
            return true;
        }
        if self.mode == Mode::Live
            && self.screen == Screen::Conversation
            && self.model_picker.is_none()
            && self.resume_picker.is_none()
            && self.agents_panel.is_none()
        {
            if let Event::Mouse(mouse) = &event {
                if matches!(
                    mouse.kind,
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                ) {
                    let chat = self
                        .selected_agent
                        .and_then(|i| self.delegations.get_mut(i))
                        .map_or(&mut self.live, |agent| &mut agent.chat);
                    if chat.cache.scroll_run(
                        mouse.column,
                        mouse.row,
                        mouse.kind == MouseEventKind::ScrollUp,
                    ) {
                        return true;
                    }
                }
            }
        }
        if self.following() {
            return match event {
                Event::Key(key) => self.follow_key(key),
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            self.scroll = self.scroll.saturating_sub(3);
                            self.follow_stick(false);
                        }
                        MouseEventKind::ScrollDown => self.scroll = self.scroll.saturating_add(3),
                        _ => {}
                    }
                    true
                }
                _ => true,
            };
        }
        match event {
            Event::Mouse(mouse)
                if self.screen == Screen::Conversation
                    && self.model_picker.is_none()
                    && self.resume_picker.is_none() =>
            {
                match mouse.kind {
                    MouseEventKind::ScrollUp => self.scroll_back(3),
                    MouseEventKind::ScrollDown => self.scroll = self.scroll.saturating_add(3),
                    _ => {}
                }
            }
            Event::Paste(text) => {
                self.footer_focused = false;
                self.escape_at = None;
                if self.resume_picker.is_some() {
                    return true;
                } else if let Some(picker) = &mut self.model_picker {
                    picker.paste(&text);
                } else if self.screen == Screen::PluginSettings {
                    match self.plugins.selected_definition().id {
                        "boat-cloud" | "gce-cloud" => {
                            if let Some(editor) = &mut self.plugins.bundled.cloud_editor {
                                editor.paste(&text);
                            }
                            return true;
                        }
                        brainstorm::PLUGIN => {
                            if self.brainstorm_job.is_some() {
                                self.cancel_request();
                            }
                            self.plugins.bundled.brainstorm.paste(&text);
                            return true;
                        }
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
                    if self.on_coder_loop() && self.plugins.focus == plugins::SettingsFocus::ApiKey
                    {
                        if self.checking_key {
                            self.cancel_request();
                        }
                        self.plugins.connection = plugins::Connection::Unchecked;
                    }
                    self.plugins.paste(&text);
                } else if !matches!(
                    self.screen,
                    Screen::Plugins | Screen::Appearance | Screen::Preferences
                ) {
                    self.composer_history.reset();
                    let cwd = self.cwd.clone().unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                    });
                    if text.starts_with("data:image/") || text.starts_with("data:application/pdf") {
                        match attachments::from_data_url(&text) {
                            Ok(_) => self.attach_image(text, None),
                            Err(error) => self.live.notice = Some(error),
                        }
                    } else if let Some(paths) = attachments::dropped_paths(&text, &cwd) {
                        self.attach_paths(paths);
                    } else {
                        if std::mem::take(&mut self.composer_selected) {
                            self.draft = Draft::default();
                        }
                        self.composer.pasted.push(text.clone());
                        self.draft.insert(&text);
                    }
                    self.slash_selected = 0;
                    self.slash_hidden = false;
                }
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
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
                // Ctrl+O switches the transcript between compact and verbose
                // wherever the person is, including over an open panel. The
                // change is plain on screen, so it needs no notice.
                if key.modifiers == KeyModifiers::CONTROL
                    && matches!(key.code, KeyCode::Char('o' | 'O'))
                {
                    self.toggle_view();
                    return true;
                }
                if self.agents_panel.is_some() {
                    self.agents_panel_key(key);
                    return true;
                }
                if let Some(picker) = &mut self.resume_picker {
                    match picker.handle(key) {
                        resume::Action::Continue => {}
                        resume::Action::Close => self.resume_picker = None,
                        resume::Action::Select(id) => {
                            self.resume(Some(&id));
                        }
                    }
                    return true;
                }
                if let Some(picker) = &mut self.model_picker {
                    match picker.handle(key) {
                        models::Action::Continue => {}
                        models::Action::Close => self.model_picker = None,
                        models::Action::Save(model, options) if model.plugin == claude::PICKER => {
                            match claude::choose(&model.id, options.reasoning) {
                                Ok(()) => self.model_picker = None,
                                Err(error) => {
                                    if let Some(picker) = &mut self.model_picker {
                                        picker.error = Some(error);
                                    }
                                }
                            }
                        }
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
                if self.screen == Screen::Appearance {
                    match key.code {
                        KeyCode::Char(' ') | KeyCode::Enter => {
                            self.appearance_error =
                                self.appearance.toggle_system_terminal_background().err();
                        }
                        KeyCode::Esc => self.screen = self.appearance_return_screen,
                        _ => {}
                    }
                    return true;
                }
                if self.screen == Screen::Preferences {
                    match key.code {
                        KeyCode::Up => self.preferences.up(),
                        KeyCode::Down => self.preferences.down(),
                        KeyCode::Right
                        | KeyCode::Char('+' | '=')
                        | KeyCode::Enter
                        | KeyCode::Char(' ') => {
                            self.preferences_error = self.preferences.higher().err();
                        }
                        KeyCode::Left | KeyCode::Char('-' | '_') => {
                            self.preferences_error = self.preferences.lower().err();
                        }
                        KeyCode::Esc => self.screen = self.preferences_return_screen,
                        _ => {}
                    }
                    return true;
                }
                if self.screen == Screen::PluginSettings {
                    match self.plugins.selected_definition().id {
                        "boat-cloud" | "gce-cloud" => {
                            if self.plugins.bundled.cloud_key(key) {
                                self.screen = Screen::Plugins;
                            }
                            return true;
                        }
                        brainstorm::PLUGIN => {
                            if self.brainstorm_job.is_some()
                                && (key.code == KeyCode::Esc
                                    || self.plugins.bundled.brainstorm.focus
                                        == brainstorm::Focus::Origin
                                        && matches!(
                                            key.code,
                                            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
                                        ))
                            {
                                self.cancel_request();
                            }
                            let closed = self.plugins.bundled.brainstorm.handle(key);
                            if self.plugins.bundled.brainstorm.check_requested {
                                self.check_brainstorm_connection();
                            }
                            if self.plugins.bundled.brainstorm.save_requested
                                && self.plugins.bundled.save_brainstorm()
                            {
                                if self.brainstorm_job.is_some() {
                                    self.cancel_request();
                                }
                                self.screen = Screen::Plugins;
                            } else if closed {
                                if self.brainstorm_job.is_some() {
                                    self.cancel_request();
                                }
                                self.screen = Screen::Plugins;
                            }
                            return true;
                        }
                        "jev" => {
                            if self.on_coder_loop()
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
                                if self.on_coder_loop() {
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
                                        && self.on_coder_loop()
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
                    if self.on_coder_loop()
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
                        if self.on_coder_loop() {
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
                            if self.plugins.toggle_selected() && self.on_coder_loop() {
                                self.cancel_request();
                            }
                            // Turning the Claude Code plugin off routes the
                            // chat's next turn again.
                            if self.on_claude()
                                && !self.plugins.bundled.enabled(crate::claude::PLUGIN)
                            {
                                self.cancel_request();
                                self.live.runs_on = None;
                            }
                        }
                        KeyCode::Enter => self.open_plugin_settings(),
                        KeyCode::Esc | KeyCode::F(2) => self.screen = self.return_screen,
                        _ => {}
                    }
                    return true;
                }
                if self.footer_focused && matches!(key.code, KeyCode::Up | KeyCode::Down) {
                    self.composer_arrow(key.code == KeyCode::Up);
                    return true;
                }
                if self.composer_selection_key(key) {
                    return true;
                }
                let hints = self.slash_hints();
                if !hints.is_empty() {
                    match key.code {
                        KeyCode::Down if hints.len() > 1 => {
                            self.slash_selected = (self.slash_selected + 1).min(hints.len() - 1);
                            return true;
                        }
                        KeyCode::Up if hints.len() > 1 => {
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
                    KeyCode::Up | KeyCode::Down => self.composer_arrow(key.code == KeyCode::Up),
                    KeyCode::Esc => {
                        if self.footer_focused {
                            self.footer_focused = false;
                            self.select_agent(None);
                        } else if self.notice.as_deref() == Some(slash::help().as_str()) {
                            self.notice = None;
                        } else if self.restore_queued_prompts() {
                            self.escape_at = None;
                        } else if self.live.busy {
                            self.restore_unanswered_prompt();
                            self.cancel_request();
                            self.live
                                .notice
                                .get_or_insert_with(|| "Request stopped.".into());
                        } else if !self.draft.text.is_empty() || !self.composer.images.is_empty() {
                            let now = std::time::Instant::now();
                            if self
                                .escape_at
                                .is_some_and(|at| now.duration_since(at).as_millis() < 1000)
                            {
                                self.record_prompt();
                                self.draft = Draft::default();
                                self.composer = Default::default();
                                self.composer_history.reset();
                                self.escape_at = None;
                            } else {
                                self.escape_at = Some(now);
                                self.notice = Some("Esc again to clear".into());
                            }
                        } else {
                            self.select_agent(None);
                        }
                    }
                    KeyCode::PageUp => self.scroll_back(5),
                    KeyCode::PageDown => self.scroll = self.scroll.saturating_add(5),
                    KeyCode::End if ctrl => self.scroll = u16::MAX,
                    // Shift+Enter or Alt+Enter: a new line in the input.
                    KeyCode::Enter
                        if key
                            .modifiers
                            .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) =>
                    {
                        self.composer_history.reset();
                        self.draft.insert("\n");
                    }
                    KeyCode::Enter if !ctrl => {
                        if self.on_coder_loop()
                            && self.draft.text.trim().is_empty()
                            && self.composer.images.is_empty()
                            && self.send_next_queued_prompt()
                        {
                            return true;
                        }
                        if self.composer.mode == composer_state::InputMode::Bash {
                            self.submit_bash();
                            return true;
                        }
                        if let Some(index) = self.selected_agent.filter(|index| {
                            self.on_coder_loop()
                                && self.delegations.get(*index).is_some_and(|c| c.background)
                                && !self.draft.text.trim().starts_with('/')
                        }) {
                            self.message_agent(index);
                            return true;
                        }
                        if self.on_coder_loop()
                            && self.live.busy
                            && (!self.draft.text.trim().is_empty()
                                || !self.composer.images.is_empty())
                        {
                            self.queue_prompt();
                            return true;
                        }
                        if self.submit_brainstorm_command() {
                            return true;
                        } else if let Some(selection) = self
                            .draft
                            .text
                            .trim()
                            .strip_prefix("/resume ")
                            .map(str::trim)
                            .map(str::to_owned)
                        {
                            if self.resume(if selection.is_empty() {
                                None
                            } else {
                                Some(&selection)
                            }) {
                                self.draft = Draft::default();
                                self.composer = Default::default();
                                self.composer_history.reset();
                            }
                        } else if let Some(argument) = self
                            .draft
                            .text
                            .trim()
                            .strip_prefix("/agent ")
                            .map(str::to_owned)
                        {
                            self.agent_command(&argument);
                        } else if self.draft.text.trim() == "/agent" {
                            self.agent_command("");
                        } else if let Some(argument) = self
                            .draft
                            .text
                            .trim()
                            .strip_prefix("/sync ")
                            .map(str::to_owned)
                        {
                            self.sync_command(&argument);
                            self.draft = Draft::default();
                            self.composer = Default::default();
                            self.composer_history.reset();
                        } else if let Some(argument) = self
                            .draft
                            .text
                            .trim()
                            .strip_prefix("/memory ")
                            .map(str::to_owned)
                        {
                            self.record_prompt();
                            self.memory_command(&argument);
                            self.draft = Draft::default();
                            self.composer = Default::default();
                            self.composer_history.reset();
                        } else if let Some(path) = self
                            .draft
                            .text
                            .trim()
                            .strip_prefix("/export ")
                            .map(str::trim)
                            .filter(|path| !path.is_empty())
                            .map(std::path::PathBuf::from)
                        {
                            self.export(Some(&path));
                        } else if self.claude_slash() {
                            // Claude Code's own commands and skills (#11439).
                        } else if self.long_work_command() {
                            // `/loop`, `/schedule` and `/shells` with
                            // their arguments (#11177).
                        } else if let Some(command) = slash::parse(self.draft.text.trim()) {
                            self.command(command);
                        } else if slash::is_command_word(self.draft.text.trim()) {
                            self.notice =
                                Some("Unknown command. Type / to see available commands.".into());
                            self.draft = Draft::default();
                            self.composer = Default::default();
                            self.composer_history.reset();
                        } else if matches!(self.mode, Mode::Live | Mode::Solver)
                            && (!self.draft.text.trim().is_empty()
                                || !self.composer.images.is_empty())
                        {
                            self.submit_live();
                        } else if !self.draft.text.trim().is_empty() {
                            self.record_prompt();
                            self.composer = Default::default();
                            self.messages.push(std::mem::take(&mut self.draft.text));
                            self.draft.cursor = 0;
                            self.screen = Screen::Conversation;
                            self.scroll = u16::MAX;
                        }
                    }
                    _ => {
                        self.footer_focused = false;
                        self.escape_at = None;
                        if key.code == KeyCode::Char('!')
                            && self.draft.text.is_empty()
                            && !ctrl
                            && self.composer.mode == composer_state::InputMode::Prompt
                        {
                            self.composer.mode = composer_state::InputMode::Bash;
                        } else if key.code == KeyCode::Backspace && self.draft.text.is_empty() {
                            self.composer.mode = composer_state::InputMode::Prompt;
                        } else {
                            if matches!(
                                key.code,
                                KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
                            ) {
                                self.composer_history.reset();
                            }
                            self.draft.edit(key);
                        }
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

#[derive(Default, Clone)]
pub struct Draft {
    pub text: String,
    pub cursor: usize,
}

impl Draft {
    fn edit(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char(ch)
                if !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
            {
                self.insert(&ch.to_string());
            }
            // Readline: Ctrl+A to the start of the line, Ctrl+E to its end.
            KeyCode::Char('a') if key.modifiers == KeyModifiers::CONTROL => {
                self.cursor = self.text[..self.cursor].rfind('\n').map_or(0, |at| at + 1);
            }
            KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
                self.cursor += self.text[self.cursor..]
                    .find('\n')
                    .unwrap_or(self.text.len() - self.cursor);
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

#[cfg(test)]
mod export_notice_tests {
    use super::*;

    #[test]
    fn export_notice_expires_after_three_seconds_without_clearing_replacements() {
        let root = tempfile::tempdir().unwrap();
        let mut app = App::default();
        app.cwd = Some(root.path().to_owned());
        app.export(Some(std::path::Path::new("chat.json")));
        let deadline = app.export_notice_expiry.as_ref().unwrap().0;
        app.expire_export_notice(deadline - std::time::Duration::from_millis(1));
        assert!(app.notice.as_deref().unwrap().contains("Exported ATIF"));
        app.copy_export_path(|_| Ok(()));
        let deadline = app.export_notice_expiry.as_ref().unwrap().0;
        assert!(
            app.notice
                .as_deref()
                .unwrap()
                .contains("Path copied to clipboard")
        );
        app.expire_export_notice(deadline);
        assert!(app.notice.is_none());
        assert!(app.export_notice_expiry.is_none());

        app.notice = Some("export notification".into());
        app.time_export_notice();
        let deadline = app.export_notice_expiry.as_ref().unwrap().0;
        app.notice = Some("another command".into());
        app.expire_export_notice(deadline);
        assert_eq!(app.notice.as_deref(), Some("another command"));
    }
}

#[cfg(test)]
mod select_all_tests {
    #[test]
    fn a_solver_run_follows_the_intake_chat_and_adds_its_cost_once() {
        let mut app = App::default();
        app.live
            .entries
            .push(live::Entry::User("pick something".into()));
        app.live.entries.push(live::Entry::Assistant {
            elapsed_ms: None,
            text: "ISSUE: #7".into(),
            model: None,
        });
        app.issue_run_base = app.live.entries.len();
        app.apply_issue_run(issue_run::Event::Set {
            index: 0,
            entry: live::Entry::User("Solve issue #7".into()),
        });
        let summary = live::Entry::Tool {
            name: issue_run::SUMMARY.into(),
            input: serde_json::json!({"cost": {"total_usd": 0.49}}),
            output: serde_json::json!({"diff": ""}),
            running: false,
        };
        app.apply_issue_run(issue_run::Event::Set {
            index: 1,
            entry: summary.clone(),
        });
        app.apply_issue_run(issue_run::Event::Set {
            index: 1,
            entry: summary,
        });
        assert!(matches!(&app.live.entries[0], live::Entry::User(t) if t == "pick something"));
        assert!(matches!(&app.live.entries[2], live::Entry::User(t) if t == "Solve issue #7"));
        assert_eq!(app.live.entries.len(), 4);
        assert!((app.live.cost_usd - 0.49).abs() < 1e-9);
    }

    use super::*;
    use crossterm::event::KeyEvent;

    fn press(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
        app.handle(Event::Key(KeyEvent::new(code, modifiers)));
    }

    fn typed(text: &str) -> App {
        let mut app = App::default();
        for ch in text.chars() {
            if ch == '\n' {
                press(&mut app, KeyCode::Enter, KeyModifiers::ALT);
            } else {
                press(&mut app, KeyCode::Char(ch), KeyModifiers::NONE);
            }
        }
        assert_eq!(app.draft.text, text);
        app
    }

    fn copied(app: &mut App) -> Option<String> {
        let mut out = None;
        app.copy_pending(|text| {
            out = Some(text.to_owned());
            Ok(())
        });
        out
    }

    #[test]
    fn cmd_a_selects_the_input_and_typing_replaces_it() {
        let mut app = typed("hello\nworld");
        press(&mut app, KeyCode::Char('a'), KeyModifiers::SUPER);
        assert!(app.composer_selected);
        assert_eq!(app.draft.text, "hello\nworld");
        press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(!app.composer_selected);
        assert_eq!(app.draft.text, "x");
        assert_eq!(app.draft.cursor, 1);
    }

    #[test]
    fn backspace_and_delete_clear_the_selection() {
        for code in [KeyCode::Backspace, KeyCode::Delete] {
            let mut app = typed("some words");
            press(&mut app, KeyCode::Char('a'), KeyModifiers::SUPER);
            press(&mut app, code, KeyModifiers::NONE);
            assert!(app.draft.text.is_empty());
            assert!(!app.composer_selected);
        }
    }

    #[test]
    fn cmd_c_copies_and_keeps_the_selection_and_cmd_x_cuts() {
        let mut app = typed("copy me");
        press(&mut app, KeyCode::Char('a'), KeyModifiers::SUPER);
        press(&mut app, KeyCode::Char('c'), KeyModifiers::SUPER);
        assert_eq!(copied(&mut app).as_deref(), Some("copy me"));
        assert!(app.composer_selected);
        assert_eq!(app.draft.text, "copy me");
        press(&mut app, KeyCode::Char('x'), KeyModifiers::SUPER);
        assert_eq!(copied(&mut app).as_deref(), Some("copy me"));
        assert!(app.draft.text.is_empty());
        assert!(!app.composer_selected);
        // Nothing selected: Cmd+C neither copies nor types a letter.
        let mut app = typed("ab");
        press(&mut app, KeyCode::Char('c'), KeyModifiers::SUPER);
        assert_eq!(copied(&mut app), None);
        assert_eq!(app.draft.text, "ab");
    }

    #[test]
    fn ctrl_shift_a_is_the_fallback_and_ctrl_a_stays_line_start() {
        let mut app = typed("one\ntwo");
        press(
            &mut app,
            KeyCode::Char('A'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert!(app.composer_selected);
        let mut app = typed("one\ntwo");
        press(&mut app, KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert!(!app.composer_selected);
        assert_eq!(app.draft.cursor, 4);
        press(&mut app, KeyCode::Char('e'), KeyModifiers::CONTROL);
        assert_eq!(app.draft.cursor, 7);
        press(&mut app, KeyCode::Home, KeyModifiers::NONE);
        press(&mut app, KeyCode::Char('e'), KeyModifiers::CONTROL);
        assert_eq!(app.draft.cursor, 3);
    }

    #[test]
    fn moving_or_pasting_ends_or_replaces_the_selection() {
        let mut app = typed("old");
        press(&mut app, KeyCode::Char('a'), KeyModifiers::SUPER);
        press(&mut app, KeyCode::Left, KeyModifiers::NONE);
        assert!(!app.composer_selected);
        assert_eq!(app.draft.text, "old");
        press(&mut app, KeyCode::Char('a'), KeyModifiers::SUPER);
        app.handle(Event::Paste("new".into()));
        assert_eq!(app.draft.text, "new");
        assert!(!app.composer_selected);
        // An empty input has nothing to select.
        let mut app = App::default();
        press(&mut app, KeyCode::Char('a'), KeyModifiers::SUPER);
        assert!(!app.composer_selected);
    }
}

#[cfg(test)]
mod claude_tests {
    use super::*;
    use harness_plugin::Harness;

    /// A chat on `harness` with `text` typed in its input.
    fn typed(harness: Harness, text: &str) -> App {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.live.runs_on = Some(harness);
        app.draft.text = text.into();
        app.draft.cursor = text.len();
        app
    }

    #[test]
    fn shift_enter_is_a_new_line_in_the_input() {
        let mut app = typed(Harness::CoderLoop, "first");
        app.handle(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::SHIFT,
        )));
        assert_eq!(app.draft.text, "first\n");
        assert!(app.live.entries.is_empty(), "nothing was sent");
    }

    #[test]
    fn a_claude_code_chat_starts_a_haiku_5_5_session_on_the_file_tools() {
        let mut app = typed(Harness::ClaudeCode, "hello");
        app.submit_live();
        assert_eq!(app.live.entries, vec![live::Entry::User("hello".into())]);
        // The turn is running a request on the mode's model, and nothing
        // was answered from the host.
        assert!(app.live.busy);
        assert!(app.draft.text.is_empty());
        assert!(app.notice.is_none());
        let request = app.request.take().expect("a claude turn was queued");
        assert!(request.key.expose().is_empty());
        match request.kind {
            live::Work::Claude { messages, .. } => {
                assert_eq!(messages.len(), 1);
                assert_eq!(messages[0].content, "hello");
            }
            _ => panic!("a claude turn"),
        }
    }

    /// A skill, `/compact`, or any command Coder does not run in this chat
    /// goes to Claude Code as the message (#11439).
    #[test]
    fn claude_codes_own_commands_go_to_claude_code() {
        for text in [
            "/review",
            "/compact",
            "/config thinking=false",
            "/my-skill do it",
        ] {
            let mut app = typed(Harness::ClaudeCode, text);
            assert!(app.claude_slash(), "{text}");
            match app.request.take().map(|request| request.kind) {
                Some(live::Work::Claude { messages, .. }) => {
                    assert_eq!(messages.last().unwrap().content, text);
                }
                _ => panic!("{text} should start a claude turn"),
            }
        }
    }

    /// A new chat the selector routes to Claude Code takes Claude Code's
    /// commands from its first message.
    #[test]
    fn a_new_chats_first_command_goes_to_claude_code_when_routed_there() {
        harness_plugin::sign_in_claude(true);
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.plugins.bundled.claude = true;
        app.draft.text = "/review".into();
        app.draft.cursor = 7;
        assert!(app.claude_slash());
        assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode));
        assert!(matches!(
            app.request.take().map(|request| request.kind),
            Some(live::Work::Claude { .. })
        ));
        harness_plugin::sign_in_claude(false);
    }

    /// Coder's own commands keep their meaning, and a path is a message.
    #[test]
    fn coders_commands_and_paths_stay_coders() {
        for text in [
            "/plugins",
            "/models",
            "/help",
            "/usr/bin/env is missing",
            "hello",
        ] {
            let mut app = typed(Harness::ClaudeCode, text);
            assert!(!app.claude_slash(), "{text}");
            assert!(app.request.is_none(), "{text}");
        }
        let mut app = typed(Harness::CoderLoop, "/review");
        assert!(!app.claude_slash());
    }

    #[test]
    fn model_with_an_unknown_name_lists_the_choices() {
        let mut app = typed(Harness::ClaudeCode, "/model gpt-6");
        assert!(app.claude_slash());
        assert!(app.request.is_none());
        assert!(app.notice.as_deref().unwrap().contains("Haiku 5.5"));
    }

    /// The path a person takes: type, press Enter. With Claude Code on and
    /// signed in, the selector routes the new chat to it, the chat is busy
    /// on Claude Code, and its reply lands as the stream reports it.
    #[test]
    fn enter_routes_a_new_chat_to_claude_code_and_its_reply_lands() {
        harness_plugin::sign_in_claude(true);
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.plugins.bundled.claude = true;
        for ch in "hello".chars() {
            app.handle(Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(ch),
                KeyModifiers::NONE,
            )));
        }
        app.handle(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert!(matches!(
            app.live.entries.as_slice(),
            [live::Entry::User(text)] if text == "hello"
        ));
        assert!(app.messages.is_empty(), "no demo message is appended");
        assert!(app.live.busy);
        assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode));
        assert!(matches!(
            app.request.as_ref().map(|request| &request.kind),
            Some(live::Work::Claude { .. })
        ));
        app.apply_update(live::Update::Finished {
            id: app.request_id,
            result: Ok(openrouter::Streamed {
                text: "hi".into(),
                model: claude::MODEL.into(),
                ..openrouter::Streamed::default()
            }),
        });
        assert!(!app.live.busy);
        assert!(matches!(
            app.live.entries.as_slice(),
            [live::Entry::User(_), live::Entry::Assistant { text, .. }] if text == "hi"
        ));
        assert!(app.live.notice.is_none(), "{:?}", app.live.notice);
        harness_plugin::sign_in_claude(false);
    }

    /// Without a Claude Code login, the selector routes a new chat to
    /// Coder's own loop, and Shift+Tab never moves it.
    #[test]
    fn a_new_chat_without_claude_code_runs_on_the_coder_loop() {
        harness_plugin::sign_in_claude(false);
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.handle(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::BackTab,
            KeyModifiers::NONE,
        )));
        app.submit("hello", std::path::Path::new("."));
        assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
        assert!(!matches!(
            app.request.as_ref().map(|request| &request.kind),
            Some(live::Work::Claude { .. })
        ));
    }

    /// Follow-ups stay on the chat's harness even after Claude Code is
    /// signed out (owner decision 1).
    #[test]
    fn follow_ups_stay_on_the_chats_harness() {
        harness_plugin::sign_in_claude(false);
        let mut app = typed(Harness::ClaudeCode, "next");
        app.submit_live();
        assert!(matches!(
            app.request.take().map(|request| request.kind),
            Some(live::Work::Claude { .. })
        ));
        assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode));
    }

    /// A picture reaches Claude Code as a picture (#11447): the message
    /// keeps its attachment line, and the turn's content carries the image.
    #[test]
    fn a_picture_goes_to_claude_code_as_an_image() {
        let dir = tempfile::tempdir().unwrap();
        let shot = dir.path().join("shot.png");
        std::fs::write(&shot, b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR").unwrap();
        let mut app = typed(Harness::ClaudeCode, "look");
        app.attachment_dir = Some(dir.path().join("attachments"));
        app.attach_image(shot.display().to_string(), None);
        app.submit_live();
        let request = app.request.take().expect("a claude turn was queued");
        let live::Work::Claude { messages, .. } = request.kind else {
            panic!("a claude turn");
        };
        let content = &messages.last().unwrap().content;
        assert!(content.starts_with("look"), "{content}");
        assert!(content.contains("[Image #1: "), "{content}");
        let blocks = claude::content(content);
        assert_eq!(blocks.len(), 2, "{blocks:?}");
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "image");
        assert_eq!(blocks[1]["source"]["media_type"], "image/png");
    }

    /// A message sent while the claude turn runs goes to Claude Code at
    /// once, in the same turn (#11447); while something else keeps the chat
    /// busy it waits in the queue.
    #[test]
    fn a_message_during_a_claude_turn_is_sent_at_once() {
        let mut app = typed(Harness::ClaudeCode, "first");
        app.submit_live();
        let first = app.request.take().unwrap().id;
        app.claude_turn = true;
        app.draft.text = "second".into();
        app.draft.cursor = 6;
        app.submit_live();
        assert!(app.queued_prompts.is_empty());
        let request = app.request.take().expect("the message was sent");
        assert_eq!(request.id, first, "it joins the running turn");
        assert!(app.live.busy);
        match request.kind {
            live::Work::Claude { messages, .. } => {
                assert_eq!(messages.last().unwrap().content, "second");
            }
            _ => panic!("a claude message"),
        }

        app.claude_turn = false;
        app.draft.text = "third".into();
        app.draft.cursor = 5;
        app.submit_live();
        assert_eq!(app.queued_prompts.len(), 1);
        assert_eq!(app.queued_prompts[0].text, "third");
        assert!(app.request.is_none());
        assert_eq!(app.request_id, first);
    }
}
