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
use coder_sdk::checkout_guard;
/// The mode that runs turns on this computer's Claude account login.
pub mod claude;
pub mod claude_ask;
pub mod claude_chat;
pub mod claude_plan;
pub mod claude_stream;
pub mod claude_tools;
pub mod cloud;
pub mod cloud_settings;
pub mod cloud_tools;
pub mod codex;
pub mod codex_chat;
mod codex_usage;
pub mod comms;
mod composer_history;
pub mod composer_state;
pub mod computer_tool;
pub mod connection_tools;
pub mod context;
pub mod context_hook;
#[cfg(test)]
mod copy_guard_tests;
pub mod credentials;
pub mod delegate_host;
mod delegation_events;
mod demo;
pub mod dispatch;
pub mod engine_update;
pub mod escalate;
pub mod file_tools;
pub mod fleet;
mod fleet_app;
pub mod floors;
pub mod gate;
pub mod gateway_models;
#[cfg(test)]
mod no_harness_tests;
use coder_sdk::git_guard;
pub mod fanout;
pub mod github_tool;
pub mod harness;
pub mod harness_pick;
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
pub mod message_route;
pub mod model_catalog;
pub mod models;
pub mod no_harness;
pub mod opencode_chat;
pub mod ops_tool;
mod own_runs;
mod plan;
#[cfg(unix)]
pub mod plugin_catalog;
pub mod plugin_definition;
pub mod plugin_hooks;
pub mod plugin_store;
pub mod plugin_tools;
pub mod plugins;
pub mod preferences;
pub mod programmatic;
mod prompt_queue;
pub mod provider;
pub mod resume;
pub mod rewind;
pub mod risk_policy;
mod route;
pub mod router;
pub mod routing_preferences;
pub mod routing_record;
mod schedule;
mod schedule_sync;
pub mod sdk_hooks;
pub mod second_look;
pub mod sessions;
pub mod shell_changes;
mod shells;
pub mod slash;
pub mod snapshot;
pub mod solver;
pub mod solver_plugin;
mod stuck;
mod supervise;
pub mod swarm;
pub mod theme;
pub mod token_savers;
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
    /// The OpenAgents gateway model picked in /models (#11526); `None` is
    /// `auto`.
    pub gateway_model: Option<String>,
    /// The model the chat's latest no-harness turn was sent to (#11548),
    /// for the rail before the reply names it.
    pub(crate) no_harness_model: Option<String>,
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
    /// Claude Code's question waiting on the person (#11449).
    pub(crate) question: Option<claude_ask::Screen>,
    pub(crate) disclosure_scroll: u16,
    pub(crate) disclosure_seen: bool,
    pub slash_selected: usize,
    pub slash_hidden: bool,
    pub notice: Option<String>,
    /// A newer Coder is ready or available (#11128); shown on the context row.
    pub update_line: Option<String>,
    pub model_picker: Option<models::Picker>,
    pub resume_picker: Option<resume::Picker>,
    /// The `/rewind` picker, while it is open (#11515).
    pub rewind_picker: Option<rewind::Picker>,
    /// Whether each turn keeps the files it changed for `/rewind`: set by
    /// the terminal, never by an App a test or a helper builds.
    pub rewind_records: bool,
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
    /// Counts presses on the input, for a double-click on a paste chip.
    composer_clicks: coder_terminal::selection::Clicks,
    /// Where the input's text was last drawn, and how far it was scrolled.
    pub(crate) composer_text: Option<(ratatui::layout::Rect, u16)>,
    /// A transcript selection went to the clipboard: the input's top-left
    /// rail says so until then.
    pub(crate) copied_until: Option<std::time::Instant>,
    /// A passing status ("Answering a reply from openagents.com") on the
    /// input's bottom-left rail until the instant, never a transcript row.
    pub(crate) rail_status: Option<(std::time::Instant, String)>,
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
    /// The turn whose routing record waits for its outcome (#11454).
    routing: Option<routing_record::Open>,
    /// What chose the next turn's harness, set where it is chosen; a turn
    /// with none set continues on the chat's harness.
    pub(crate) routing_rule: Option<routing_record::Rule>,
    /// The harness the person picked with Shift+Tab (#11524) since the
    /// chat's last turn; the chat's own pin otherwise.
    pub(crate) next_pick: Option<harness_pick::Pick>,
    /// When the highlighted pick's facts are refreshed.
    pub(crate) pick_refresher: harness_pick::Refresher,
    /// How many pick refreshes this App has seen finish.
    pub(crate) pick_refreshes_seen: u64,
    /// What router v0 read for the next turn's choice: System One's
    /// probabilities and its trace, for the routing record (#11460).
    pub(crate) routing_router: Option<(Vec<routing_record::Probability>, router::Trace)>,
    /// The chat's last recorded turn, to tell whether the next message
    /// re-asks it (#11461).
    pub(crate) last_routed: Option<routing_record::LastTurn>,
    /// Whether this chat's routing choices and outcomes are recorded:
    /// set by the terminal and `coder chat`, never by an App a test or a
    /// helper builds, so the records hold real turns only (#11461).
    pub routing_records: bool,
    /// A message held while System One judges where it runs (#11460).
    pub(crate) pending_route: Option<router::Pending>,
    /// The router's call for the held message, read by
    /// [`App::route_turn`] when the message is sent.
    pub(crate) routed: Option<router::Routed>,
    /// The held message is being sent now: its routing point is over.
    pub(crate) route_settled: bool,
    /// How the next turn's briefing was chosen, for the routing record
    /// (#11473).
    pub(crate) routing_context: Option<context::Trace>,
    /// A checkpoint waiting for System One's call (#11474).
    pub(crate) pending_escalation: Option<escalate::Pending>,
    /// The chat's dispatch plan for a multi-part ask, while it runs
    /// (#11474).
    pub(crate) plan: Option<dispatch::Plan>,
    /// The next message is a new piece of work whatever the chat holds: a
    /// plan's part.
    pub(crate) fresh_work: bool,
    /// A plan's part is being sent.
    pub(crate) sending_part: bool,
}

#[derive(Default)]
struct Chat {
    draft: Draft,
    messages: Vec<String>,
    scroll: u16,
}

impl App {
    /// The openagents.com sign-in this chat's state holds, if any: it
    /// reaches the account's Connections and opens `auto`'s gateway lane
    /// (#11238, #11517).
    pub(crate) fn connections(&self) -> Option<openagents_login::Saved> {
        self.account_dir
            .as_deref()
            .and_then(connection_tools::signed_in)
    }
}

impl App {
    fn clear_disclosure(&mut self) {
        if let Some(desk) = self.disclosure_desk.take() {
            desk.close();
            crate::floors::install_desk(None);
        }
        self.disclosure_event = None;
        self.disclosure_scroll = 0;
        self.disclosure_seen = false;
    }

    /// Opens the approval card a floor asks on for this turn, when a
    /// person is at the terminal to answer it (#11475).
    fn open_floor_desk(&mut self) {
        if !self.interactive_disclosures {
            return;
        }
        let desk = self
            .disclosure_desk
            .get_or_insert_with(approval::Desk::new)
            .clone();
        crate::floors::install_desk(Some(desk));
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
    /// The full id of a delegation an earlier turn opened for `delegation`
    /// (`{request}:{delegation}`): a background Claude Code subagent
    /// reports its progress and end in later turns (#11482). Claude Code's
    /// call ids are unique; other providers reuse theirs between turns, so
    /// only the claude chat looks up by them.
    pub(crate) fn known_delegation(&self, delegation: &str) -> Option<String> {
        self.delegations
            .iter()
            .find(|child| {
                child
                    .id
                    .split_once(':')
                    .is_some_and(|(_, rest)| rest == delegation)
            })
            .map(|child| child.id.clone())
    }

    /// An event of a delegation an earlier turn opened, applied without
    /// opening a turn. Returns whether there was one.
    pub(crate) fn apply_known_delegation(
        &mut self,
        delegation: &str,
        name: String,
        task: String,
        event: bundled_runtime::RuntimeEvent,
    ) -> Result<(), bundled_runtime::RuntimeEvent> {
        match self.known_delegation(delegation) {
            Some(known) => {
                self.apply_delegation(known, name, task, event);
                self.history.dirty = true;
                Ok(())
            }
            None => Err(event),
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
            bundled_runtime::RuntimeEvent::Thinking(on) => child.chat.thinking = on,
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
                || tool == coder_sdk::delegation::TOOL
                || tool == "acp_subagent"
                || tool == "remote_delegate"
                || tool == solver_plugin::TOOL =>
            {
                let was_running = child.running;
                child.running = running;
                child.chat.busy = running;
                if !running {
                    // A second report of the same end (Claude Code's own
                    // notice after the call's result, #11482) keeps the
                    // time the first one took.
                    if was_running {
                        child.elapsed_seconds =
                            self.elapsed_seconds.saturating_sub(child.started_at);
                    }
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
        codex::load_choice(store.root());
        self.gateway_model = gateway_models::load_choice(store.root());
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
            || self.rewind_picker.is_some()
            || self.screen != Screen::Conversation
        {
            return Vec::new();
        }
        slash::matches(&self.draft.text)
            .into_iter()
            // A turn's files are kept in live chats only, and a chat
            // without a message has nothing to rewind (#11515).
            .filter(|command| {
                *command != slash::Command::Rewind
                    || (self.mode == Mode::Live && self.live.user_count() > 0)
            })
            .filter(|command| {
                *command != slash::Command::Models
                    || self.plugins.enabled
                    || self.on_claude()
                    || self.on_codex()
                    || self.on_opencode()
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
                Some((id, _, _)) => match claude::choose(id, claude::choice().effort) {
                    Ok(()) => self.claude_model_switched(id),
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
            // A stopped turn's files can be undone too (#11515).
            self.finish_turn_record();
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
            self.live.thinking = false;
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
                delegation.chat.thinking = false;
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
                    self.live.thinking = false;
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
                    self.live.thinking = false;
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
            live::Update::Thinking { on, .. } if self.live.busy => {
                self.live.thinking = on;
            }
            live::Update::Delta { text, .. } if self.live.busy => {
                self.live.text_streamed = true;
                self.live.partial.push_str(&text);
                self.scroll_main_to_end();
            }
            live::Update::OpenCodeSession { session, model, .. } => {
                self.live.opencode_session = Some(live::OpenCodeSession {
                    id: session,
                    users: self.live.user_count(),
                    model,
                });
                self.history.dirty = true;
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
                self.live.thinking = false;
                let text_streamed = std::mem::take(&mut self.live.text_streamed);
                if let Some(id) = self.active_delegation.take() {
                    if let Some(child) = self.delegations.iter_mut().find(|child| child.id == id) {
                        child.running = false;
                        child.chat.busy = false;
                        child.chat.thinking = false;
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
                // The files the turn changed, for /rewind (#11515).
                self.finish_turn_record();
                // Coder's own loop waits for its agents within the turn: one
                // still shown running when the turn ends never reported back,
                // and would keep its chat working (#11462).
                if !self.on_claude() && !self.on_codex() && !self.on_opencode() {
                    for child in &mut self.delegations {
                        if child.running && !child.background {
                            child.running = false;
                            child.chat.busy = false;
                            child.chat.thinking = false;
                            child.elapsed_seconds =
                                self.elapsed_seconds.saturating_sub(child.started_at);
                            child.chat.finish_partial();
                            child
                                .chat
                                .stop_tools("The turn ended before this agent reported back.");
                        }
                    }
                    for entry in &mut self.live.entries {
                        if let live::Entry::Delegation {
                            running, output, ..
                        } = entry
                            && *running
                        {
                            *running = false;
                            *output = serde_json::json!({"error":"The turn ended before this agent reported back."});
                        }
                    }
                }
                let (model, error) = match &result {
                    Ok(reply) => (Some(reply.model.clone()), None),
                    Err(error) => (None, Some(error.clone())),
                };
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
                                ..Default::default()
                            };
                        }
                    }
                }
                let failed = error.clone();
                self.record_end(None, model, error);
                // A turn that ended with failed checks, an error, a limit or
                // a stall is a checkpoint (#11474).
                self.checkpoint_turn(failed.as_deref());
                // A plan's part ended: the next part, or the report.
                self.advance_plan(failed.as_deref());
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
        // The chosen message comes back into the input after it clears.
        if command == slash::Command::Rewind {
            if self.rewind() {
                self.draft = Draft::default();
                self.composer = Default::default();
                self.composer_history.reset();
                self.slash_selected = 0;
                self.slash_hidden = false;
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
            slash::Command::Resume | slash::Command::Rewind => {
                unreachable!("Resume and Rewind are handled before clearing the draft")
            }
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
                        2 => {
                            cache.model_tip = None;
                            cache.select_word(at);
                        }
                        3 => {
                            cache.model_tip = None;
                            cache.select_line(at);
                        }
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
                    // Dragging selects; the model label is for a plain click.
                    cache.model_tip = None;
                    cache.follow_drag();
                    return true;
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    let Some(drag) = cache.drag.take() else {
                        return false;
                    };
                    if cache.selection.is_none() {
                        // A click without a drag: the message's model shows
                        // at the row's right end for a moment.
                        cache.show_model(drag.anchor.row, std::time::Instant::now());
                        return true;
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

    /// A click in the input puts the cursor there; a double-click on a
    /// `[Pasted: N lines]` chip turns it back into plain text to edit, as
    /// Grok Build's composer does. Returns whether the click was used up.
    fn composer_click(&mut self, event: &Event) -> bool {
        use crossterm::event::MouseButton;
        let Event::Mouse(mouse) = event else {
            return false;
        };
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return false;
        }
        let Some((area, scroll)) = self.composer_text else {
            return false;
        };
        if mouse.column < area.x
            || mouse.column >= area.x + area.width
            || mouse.row < area.y
            || mouse.row >= area.y + area.height
        {
            return false;
        }
        let (column, row) = (mouse.column - area.x, mouse.row - area.y + scroll);
        let (offset, chip) = self.draft.hit(self.composer_width, column, row);
        let at = coder_terminal::selection::Point {
            row: usize::from(row),
            col: column,
        };
        let presses = self.composer_clicks.press(std::time::Instant::now(), at);
        if let Some(start) = chip
            && presses >= 2
        {
            self.draft.expand_chip(start);
        }
        self.composer_selected = false;
        self.footer_focused = false;
        self.draft.cursor = offset;
        self.draft.snap_cursor();
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
        self.live.thinking = false;
        if let brainstorm::Conversation::Delegation(id) = conversation {
            if let Some(child) = self.delegations.iter_mut().find(|child| &child.id == id) {
                child.running = false;
                child.chat.busy = false;
                child.chat.thinking = false;
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
        let (images, chars) = (self.composer.images.len(), text.chars().count());
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
            self.begin_turn_record();
            self.record_choice(harness_plugin::Harness::ClaudeCode, images, chars);
            // A floor asks the person on the approval card (#11475).
            self.open_floor_desk();
        }
        self.request = Some(live::Request {
            id: self.request_id,
            key: model_access::ApiKey::new(""),
            kind: live::Work::Claude {
                messages,
                cwd,
                tools: Box::new(crate::claude_tools::Tools::new(self.agent_execution())),
                session,
                briefing: self.live.briefing.clone().map(Box::new),
                model: self.live.pinned_model.clone(),
            },
        });
    }

    /// A Codex turn (#11510): the person's message to the chat's own Codex
    /// process ([`crate::codex_chat`]) on `codex app-server`, with Codex's
    /// own tools, Coder's tools in process, and each command and patch
    /// answered by Coder's hooks. A message sent while a turn runs is read
    /// in that turn. The same one conversation; only the harness differs.
    pub(crate) fn submit_codex(&mut self) {
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
        let (images, chars) = (self.composer.images.len(), text.chars().count());
        self.draft.cursor = 0;
        self.composer = Default::default();
        self.screen = Screen::Conversation;
        self.live.finish_partial();
        self.live.entries.push(live::Entry::User(text));
        let messages = self.live.messages();
        let session = self.live.codex_resume();
        self.history.dirty = true;
        self.scroll = u16::MAX;
        if !midturn {
            self.begin_claude_turn();
            // The files the turn changes, for /rewind (#11515).
            self.begin_turn_record();
            self.record_choice(harness_plugin::Harness::Codex, images, chars);
            // A floor asks the person on the approval card while Codex
            // waits (#11475).
            self.open_floor_desk();
        }
        self.request = Some(live::Request {
            id: self.request_id,
            key: model_access::ApiKey::new(""),
            kind: live::Work::Codex {
                messages,
                cwd,
                tools: Box::new(crate::claude_tools::Tools::new(self.agent_execution())),
                session,
                briefing: self.live.briefing.clone().map(Box::new),
                model: self.live.pinned_model.clone(),
            },
        });
    }

    /// The draft as the chat's next OpenCode turn (#11507): one `opencode
    /// run` in the chat's OpenCode session ([`crate::opencode_chat`]), on
    /// the pinned model or the person's OpenCode pick, with every tool
    /// checked by Coder before it runs. A message sent while a turn runs
    /// waits for it.
    fn submit_opencode(&mut self) {
        if self.draft.text.trim().is_empty() {
            return;
        }
        if self.live.busy || self.checking_key || self.checking_jev {
            self.queue_prompt();
            return;
        }
        if !self.composer.images.is_empty() {
            self.live.notice = Some(attachments::TEXT_ONLY_NOTICE.into());
        }
        self.record_prompt();
        self.cancel_request();
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
        let text = std::mem::take(&mut self.draft.text);
        let chars = text.chars().count();
        self.draft.cursor = 0;
        self.composer = Default::default();
        self.screen = Screen::Conversation;
        self.live.finish_partial();
        self.live.entries.push(live::Entry::User(text));
        let messages = self.live.messages();
        let session = self.live.opencode_resume();
        // The briefing goes with a new piece of work, and with a new
        // session's first message.
        let new_work = self.live.work_started + 1 >= self.live.user_count();
        let briefing = (session.is_none() || new_work)
            .then(|| self.live.briefing.clone().map(Box::new))
            .flatten();
        let model = opencode_chat::model(self.live.pinned_model.clone());
        self.history.dirty = true;
        self.scroll = u16::MAX;
        self.begin_claude_turn();
        // The files the turn changes, for /rewind (#11515).
        self.begin_turn_record();
        self.record_choice(harness_plugin::Harness::OpenCode, 0, chars);
        self.request = Some(live::Request {
            id: self.request_id,
            key: model_access::ApiKey::new(""),
            kind: live::Work::OpenCode {
                messages,
                cwd,
                session,
                briefing,
                model,
                tools: true,
            },
        });
    }

    /// After the person picks `model` for Codex in /models: the next
    /// message runs on it, in this same chat and thread.
    fn codex_model_switched(&mut self, model: &str) -> String {
        if self.live.runs_on == Some(harness_plugin::Harness::Codex) {
            self.live.pinned_model = None;
        }
        let name = codex::name(model);
        if self.live.codex_session.is_some() {
            format!("Your next message runs on {name}, in this same chat.")
        } else {
            format!("Codex now runs {name}.")
        }
    }

    /// After the person picks `model` for claude in /models (#11480): a
    /// pin's model no longer holds this chat's Claude Code turns, so the
    /// next message runs on the pick, and the words saying so.
    fn claude_model_switched(&mut self, model: &str) -> String {
        if self.live.runs_on == Some(harness_plugin::Harness::ClaudeCode) {
            self.live.pinned_model = None;
        }
        let name = claude::name(model);
        if self.live.claude_session.is_some() {
            format!("Your next message runs on {name}, in this same chat.")
        } else {
            format!("claude now runs {name}.")
        }
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
        // The chat folder's own repository (#11536); empty when it has no
        // GitHub remote, and the Intaker asks which one.
        let github = issue_run::github_repo(&cwd).unwrap_or_default();
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
                github,
                claude: self.plugins.enabled_for(crate::claude::PLUGIN),
                account: self.connections(),
            },
        });
    }

    /// Starts a solver run on `issue` in `repo`, or without one in the
    /// chat folder's own GitHub repository (#11536).
    fn start_solver(&mut self, repo: Option<String>, issue: u64) {
        if self.live.busy || self.issue_run.is_some() {
            self.notice = Some("A solver run is already working here.".into());
            return;
        }
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
        let mut args = vec![
            issue.to_string(),
            "--repo".into(),
            cwd.display().to_string(),
        ];
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
        // A `/brainstorm` lookup is Coder's own native read, whatever
        // harness the chat runs on: it never reaches a harness's model.
        let native_lookup = brainstorm::parse(self.draft.text.trim()).is_some();
        if self.mode == Mode::Live
            && self.selected_agent.is_none()
            && !native_lookup
            && (!self.draft.text.trim().is_empty() || !self.composer.images.is_empty())
        {
            // The person's own message ends a checkpoint still waiting
            // for its call: they went on (#11474).
            if !self.route_settled && !self.sending_part {
                self.pending_escalation = None;
                self.plan = None;
            }
            // A Shift+Tab pick holds from this message on (#11524).
            self.apply_pick();
            if let Some(agent) = self.picked_agent() {
                // A list-shaped message may still run as parts, each on
                // this agent (#11544).
                if self.ask_router() {
                    return;
                }
                self.submit_agent(agent);
                return;
            }
            // Never means never (#11475).
            if !self.route_settled && self.pending_route.is_none() && self.honor_never() {
                return;
            }
            // A message sent while another waits for its route queues
            // behind it; a routing point waits for System One (#11460).
            if self.pending_route.is_some() {
                self.queue_prompt();
                return;
            }
            if self.ask_router() {
                return;
            }
            match self.route_turn() {
                harness_plugin::Harness::ClaudeCode => {
                    self.submit_claude();
                    return;
                }
                harness_plugin::Harness::Codex => {
                    self.submit_codex();
                    return;
                }
                harness_plugin::Harness::OpenCode => {
                    self.submit_opencode();
                    return;
                }
                harness_plugin::Harness::CoderLoop => {}
            }
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
        let (images, chars) = (self.composer.images.len(), self.draft.text.chars().count());
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
        self.begin_turn_record();
        self.record_choice(harness_plugin::Harness::CoderLoop, images, chars);
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
        // One briefing for every harness (#11473).
        execution.briefing = self.live.briefing.clone();
        execution.prompt_inbox = Some(self.prompt_inbox.clone());
        execution.boundary = self.boundary.clone();
        // Google Drive through the signed-in account's Connections (#11238).
        execution.connections = self.connections();
        // A gateway model picked in /models, while the gateway answers
        // this sign-in (#11526).
        execution.gateway_model = crate::gateway_models::active(
            self.gateway_model.as_deref(),
            execution.connections.as_ref(),
        );
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
        // A floor asks the person on the approval card (#11475).
        self.open_floor_desk();
        // A model the person chose keeps its own path (#11132); `auto`
        // routes across every connected door.
        // A pin's model (#11475), else the person's choice in /models.
        let model = self
            .live
            .pinned_model
            .clone()
            .unwrap_or_else(|| self.plugins.model.clone());
        // A cheap answer runs on a cheap model unless the person chose
        // one (#11537).
        execution.cheap = self.cheap_turn();
        // No harness can run here (#11548): every call on the one model.
        execution.no_harness = self.no_harness_turn();
        self.no_harness_model = execution
            .no_harness
            .as_ref()
            .map(|no_harness| no_harness.model().to_owned());
        let kind = if key.is_some() && models::pinned(&model) {
            live::Work::Chat {
                model,
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

    /// The no-harness turn (#11548): Auto found no harness that can run
    /// here (Claude Code, Codex, OpenCode all off, missing, signed out, or
    /// at their limits), or `CODER_NO_HARNESS` forced it, and nobody chose
    /// a model, so the Coder loop runs every call on the Coder SDK's
    /// no-harness model, Gemini 4 Argon by default.
    pub(crate) fn no_harness_turn(&self) -> Option<crate::route::NoHarness> {
        if self.live.pinned_model.is_some() || self.live.agent.is_some() {
            return None;
        }
        let keyed = self.plugins.enabled && self.plugins.key_configured;
        let account = self.connections();
        let chosen = (keyed && models::pinned(&self.plugins.model))
            || gateway_models::active(self.gateway_model.as_deref(), account.as_ref()).is_some();
        if chosen {
            return None;
        }
        let policy = no_harness::policy()?;
        let usable = crate::harness_plugin::candidates(&crate::harness_plugin::Facts::read(self))
            .admitted
            .iter()
            .any(|harness| *harness != crate::harness_plugin::Harness::CoderLoop);
        match policy.engine(usable) {
            coder_sdk::auto_policy::Engine::Harnesses => None,
            coder_sdk::auto_policy::Engine::CoderLoop(_) => Some(crate::route::NoHarness {
                policy,
                gemini_key: no_harness::gemini_key(),
                gemini_base: no_harness::gemini_base(),
            }),
        }
    }

    /// The cheap model this Coder loop turn runs on when the router
    /// judged its work a cheap answer (#11537) and nobody chose a model:
    /// the gateway's quick router on the free allowance while signed in,
    /// else each login's cheapest.
    pub(crate) fn cheap_turn(&self) -> Option<crate::route::Cheap> {
        if !self.live.cheap || self.live.pinned || self.live.pinned_model.is_some() {
            return None;
        }
        let account = self.connections();
        let chosen = models::pinned(&self.plugins.model)
            || gateway_models::active(self.gateway_model.as_deref(), account.as_ref()).is_some();
        if chosen {
            return None;
        }
        // Signed in, the gateway's quick router answers on the account's
        // free daily allowance; its named models, even ones it marks free,
        // ask for credit from a signed-in account.
        let gateway_model = account
            .as_ref()
            .filter(|account| gateway_models::reachable(account))
            .map(|_| crate::models::GATEWAY_FAST.to_owned());
        Some(crate::route::Cheap { gateway_model })
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
        self.rewind_picker = None;
        self.model_picker = None;
        if self.screen != Screen::Appearance {
            self.appearance_return_screen = self.screen;
        }
        self.screen = Screen::Appearance;
    }

    pub fn open_preferences(&mut self) {
        self.resume_picker = None;
        self.rewind_picker = None;
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
        self.rewind_picker = None;
        if !matches!(self.screen, Screen::Plugins | Screen::PluginSettings) {
            self.return_screen = self.screen;
        }
        self.screen = Screen::Plugins;
        self.plugins.bundled.check_engines(false);
    }

    pub fn open_models(&mut self) {
        self.resume_picker = None;
        // A Claude Code chat picks among Claude models on this computer's
        // login, with no OpenRouter key.
        // An OpenCode chat picks among the models the person's OpenCode
        // reaches (#11507).
        if self.on_opencode() {
            let current = self.live.opencode_session.as_ref().map_or_else(
                || opencode_chat::model(self.live.pinned_model.clone()),
                |session| session.model.clone(),
            );
            self.model_picker = Some(models::Picker::new(
                opencode_chat::catalog(),
                opencode_chat::PLUGIN,
                &current,
                models::GenerationOptions::default(),
                false,
            ));
            return;
        }
        if self.on_codex() {
            let picked = codex::choice();
            self.model_picker = Some(models::Picker::new(
                codex::catalog(),
                codex::PICKER,
                &picked.model,
                models::GenerationOptions {
                    reasoning: picked.effort,
                    max_tokens: None,
                },
                false,
            ));
            return;
        }
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
        // The Coder loop: the OpenAgents gateway's models while this
        // sign-in reaches it (#11526), from the kept list, refreshed in
        // the background; OpenRouter's while BYOK is on.
        let account = self.connections();
        let gateway = self
            .plugins
            .store_root()
            .map(|root| gateway_models::listed(root, account.as_ref()))
            .unwrap_or_default();
        let reachable = account.as_ref().is_some_and(gateway_models::reachable);
        if !self.plugins.enabled && !reachable {
            self.notice = Some(
                "Sign in to OpenAgents or turn on OpenRouter BYOK in /plugins to choose a model."
                    .into(),
            );
            return;
        }
        let mut catalog = if self.plugins.enabled {
            models::openrouter_catalog()
        } else {
            Vec::new()
        };
        catalog.extend(gateway_models::catalog(&gateway, !self.plugins.enabled));
        let picked = gateway_models::active(self.gateway_model.as_deref(), account.as_ref());
        let (plugin, model) = match &picked {
            Some(model) if !models::pinned(&self.plugins.model) => {
                (gateway_models::PICKER, model.clone())
            }
            _ if self.plugins.enabled => (models::OPENROUTER_PLUGIN, self.plugins.model.clone()),
            _ => (gateway_models::PICKER, models::AUTO.to_owned()),
        };
        self.model_picker = Some(models::Picker::new(
            catalog,
            plugin,
            &model,
            self.plugins.options.clone(),
            self.plugins.enabled && matches!(self.mode, Mode::Live | Mode::Solver),
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
        self.plugins.bundled.engines.poll();
        self.poll_pick();
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
        if let Some(screen) = &self.question {
            return ("blocked", Some(screen.question().question.clone()));
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
            // A message waiting for its route stays with the chat it was
            // written in, back in that chat's input (#11460).
            self.cancel_route();
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
        if let Some(screen) = &mut self.question {
            if let Event::Key(key) = event {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    return true;
                }
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    screen.decline();
                    self.question = None;
                    self.cancel_request();
                    return false;
                }
                let outcome = match key.code {
                    KeyCode::Up => {
                        screen.up();
                        claude_ask::Outcome::Asking
                    }
                    KeyCode::Down | KeyCode::Tab => {
                        screen.down();
                        claude_ask::Outcome::Asking
                    }
                    KeyCode::Char(' ') => {
                        screen.toggle();
                        claude_ask::Outcome::Asking
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        screen.type_char(c);
                        claude_ask::Outcome::Asking
                    }
                    KeyCode::Backspace => {
                        screen.backspace();
                        claude_ask::Outcome::Asking
                    }
                    KeyCode::Enter => screen.enter(),
                    KeyCode::Esc => screen.decline(),
                    _ => claude_ask::Outcome::Asking,
                };
                if outcome == claude_ask::Outcome::Done {
                    self.question = None;
                }
            } else if let Event::Paste(text) = event {
                for c in text.chars().filter(|c| !c.is_control()) {
                    screen.type_char(c);
                }
            }
            return true;
        }
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
            && self.rewind_picker.is_none()
            && self.agents_panel.is_none()
            && self.composer_click(&event)
        {
            return true;
        }
        if matches!(self.mode, Mode::Live | Mode::Solver)
            && self.screen == Screen::Conversation
            && self.model_picker.is_none()
            && self.resume_picker.is_none()
            && self.rewind_picker.is_none()
            && self.agents_panel.is_none()
            && self.transcript_select(&event)
        {
            return true;
        }
        if self.mode == Mode::Live
            && self.screen == Screen::Conversation
            && self.model_picker.is_none()
            && self.resume_picker.is_none()
            && self.rewind_picker.is_none()
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
                    && self.resume_picker.is_none()
                    && self.rewind_picker.is_none() =>
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
                if self.resume_picker.is_some() || self.rewind_picker.is_some() {
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
                        self.draft.paste(&text);
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
                // Ctrl+N starts a new chat in place; the last one stays in
                // /resume (#11480).
                if key.modifiers == KeyModifiers::CONTROL
                    && matches!(key.code, KeyCode::Char('n' | 'N'))
                {
                    self.new_chat();
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
                if let Some(picker) = &mut self.rewind_picker {
                    match picker.handle(key) {
                        rewind::Action::Continue => {}
                        rewind::Action::Close => self.rewind_picker = None,
                        rewind::Action::Select(message) => {
                            self.rewind_to(message);
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
                                Ok(()) => {
                                    self.model_picker = None;
                                    self.notice = Some(self.claude_model_switched(&model.id));
                                }
                                Err(error) => {
                                    if let Some(picker) = &mut self.model_picker {
                                        picker.error = Some(error);
                                    }
                                }
                            }
                        }
                        models::Action::Save(model, _) if model.plugin == opencode_chat::PLUGIN => {
                            match opencode_chat::choose(&model.id) {
                                Ok(()) => {
                                    self.model_picker = None;
                                    if self.live.runs_on == Some(harness_plugin::Harness::OpenCode)
                                    {
                                        self.live.pinned_model = None;
                                    }
                                    self.notice = Some(if self.live.opencode_session.is_some() {
                                        format!(
                                            "Your next message runs on {}, in this same chat.",
                                            model.id
                                        )
                                    } else {
                                        format!("OpenCode now runs {}.", model.id)
                                    });
                                }
                                Err(error) => {
                                    if let Some(picker) = &mut self.model_picker {
                                        picker.error = Some(error);
                                    }
                                }
                            }
                        }
                        models::Action::Save(model, options) if model.plugin == codex::PICKER => {
                            match codex::choose(&model.id, options.reasoning) {
                                Ok(()) => {
                                    self.model_picker = None;
                                    self.notice = Some(self.codex_model_switched(&model.id));
                                }
                                Err(error) => {
                                    if let Some(picker) = &mut self.model_picker {
                                        picker.error = Some(error);
                                    }
                                }
                            }
                        }
                        models::Action::Save(model, _)
                            if model.plugin == gateway_models::PICKER =>
                        {
                            // A gateway pick (#11526) is the Coder loop's
                            // model; an OpenRouter pick it replaces goes
                            // back to auto.
                            let reset = !self.plugins.enabled
                                || !models::pinned(&self.plugins.model)
                                || self.plugins.set_model(
                                    &models::openrouter_catalog()[0],
                                    models::GenerationOptions::default(),
                                );
                            let saved = if reset {
                                gateway_models::choose(self.plugins.store_root(), Some(&model.id))
                            } else {
                                Err(self
                                    .plugins
                                    .storage_error
                                    .clone()
                                    .unwrap_or_else(|| "Cannot save the model.".into()))
                            };
                            match saved {
                                Ok(picked) => {
                                    self.gateway_model = picked;
                                    self.model_picker = None;
                                    self.notice = Some(if models::pinned(&model.id) {
                                        format!("Coder now runs {} on OpenAgents.", model.name)
                                    } else {
                                        "OpenAgents picks each turn's model.".into()
                                    });
                                }
                                Err(error) => {
                                    if let Some(picker) = &mut self.model_picker {
                                        picker.error = Some(error);
                                    }
                                }
                            }
                        }
                        models::Action::Save(model, options) => {
                            // An OpenRouter pick replaces a gateway pick.
                            if gateway_models::choose(self.plugins.store_root(), None).is_ok() {
                                self.gateway_model = None;
                            }
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
                                KeyCode::Char('r') => {
                                    self.plugins.bundled.refresh_acp();
                                    self.plugins.bundled.check_engines(true);
                                }
                                KeyCode::Char('u') => {
                                    self.plugins.bundled.update_selected_agent();
                                }
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
                            // So does turning the Codex plugin off.
                            if self.on_codex()
                                && !self.plugins.bundled.enabled(crate::codex::PLUGIN)
                            {
                                self.cancel_request();
                                self.live.runs_on = None;
                            }
                            // And the OpenCode plugin (#11507).
                            if self.on_opencode()
                                && !self.plugins.bundled.enabled(crate::opencode_chat::PLUGIN)
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
                // Shift+Tab steps the harness pick on the input's rail
                // (#11524). It never sends and leaves the draft as it is.
                if key.code == KeyCode::BackTab
                    && self.mode == Mode::Live
                    && self.selected_agent.is_none()
                {
                    self.cycle_pick();
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
                        if self.cancel_route() {
                            // The message waiting for its route comes back
                            // to the input, unsent.
                            self.escape_at = None;
                        } else if self.footer_focused {
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
    /// Large pastes, each drawn as one `[Pasted: 12 lines]` chip while the
    /// text keeps every pasted byte, as Grok Build's composer does.
    pub pastes: Vec<PasteChip>,
}

/// One pasted run of the draft shown as a chip. It holds the pasted text, so
/// a chip whose text no longer sits at `start` (the draft was replaced) is
/// simply not a chip any more.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasteChip {
    pub start: usize,
    pub text: String,
}

impl PasteChip {
    fn end(&self) -> usize {
        self.start + self.text.len()
    }

    /// `[Pasted: N lines]`, or `[Pasted: 12 KB]` for a paste over
    /// [`PASTE_CHIP_BYTES`].
    #[must_use]
    pub fn label(&self) -> String {
        let bytes = self.text.len();
        if bytes > PASTE_CHIP_BYTES {
            let size = if bytes >= 1_000_000 {
                format!("{:.1} MB", bytes as f64 / 1_000_000.0)
            } else {
                format!("{} KB", bytes / 1000)
            };
            format!("[Pasted: {size}]")
        } else {
            let lines = self.text.lines().count();
            format!(
                "[Pasted: {lines} line{}]",
                if lines == 1 { "" } else { "s" }
            )
        }
    }
}

/// One grapheme of the draft as the composer lays it out.
struct Placed {
    offset: Option<usize>,
    chip: Option<usize>,
    row: usize,
    column: usize,
    cells: usize,
    text: String,
}

/// A paste of this many lines or more shows as a chip.
pub const PASTE_CHIP_LINES: usize = 4;
/// A paste over this many bytes shows as a chip, whatever its lines.
pub const PASTE_CHIP_BYTES: usize = 10_000;

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
                if let Some((start, _)) = self.chip_around(self.cursor) {
                    self.cursor = start;
                }
            }
            KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
                self.cursor += self.text[self.cursor..]
                    .find('\n')
                    .unwrap_or(self.text.len() - self.cursor);
                self.snap_cursor();
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

    fn clean(text: &str) -> String {
        text.replace("\r\n", "\n")
            .replace(['\r', '\u{2028}', '\u{2029}'], "\n")
            .replace('\t', "    ")
            .chars()
            .filter(|ch| *ch == '\n' || !ch.is_control())
            .collect()
    }

    fn insert(&mut self, text: &str) {
        let clean = Self::clean(text);
        self.insert_clean(&clean);
    }

    fn insert_clean(&mut self, clean: &str) {
        self.pastes
            .retain(|chip| self.text.get(chip.start..chip.end()) == Some(&chip.text));
        for chip in &mut self.pastes {
            if chip.start >= self.cursor {
                chip.start += clean.len();
            }
        }
        self.text.insert_str(self.cursor, clean);
        self.cursor += clean.len();
        self.snap_cursor();
    }

    /// Inserts a paste: one of [`PASTE_CHIP_LINES`] lines or more, or over
    /// [`PASTE_CHIP_BYTES`], becomes a chip that the cursor steps over and
    /// Backspace removes whole; the text sent is still the whole paste.
    pub(crate) fn paste(&mut self, text: &str) {
        let clean = Self::clean(text);
        if clean.lines().count() < PASTE_CHIP_LINES && clean.len() <= PASTE_CHIP_BYTES {
            self.insert_clean(&clean);
            return;
        }
        let start = self.cursor;
        self.insert_clean(&clean);
        self.pastes.push(PasteChip { start, text: clean });
    }

    /// The chips still in the draft, in order.
    pub fn chips(&self) -> Vec<&PasteChip> {
        let mut chips: Vec<&PasteChip> = self
            .pastes
            .iter()
            .filter(|chip| self.text.get(chip.start..chip.end()) == Some(chip.text.as_str()))
            .collect();
        chips.sort_by_key(|chip| chip.start);
        chips
    }

    /// The chip the cursor would sit inside at `offset`, if any.
    fn chip_around(&self, offset: usize) -> Option<(usize, usize)> {
        self.chips()
            .into_iter()
            .map(|chip| (chip.start, chip.end()))
            .find(|(start, end)| *start < offset && offset < *end)
    }

    pub(crate) fn snap_cursor(&mut self) {
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
        // The cursor never rests inside a paste chip.
        if let Some((_, end)) = self.chip_around(self.cursor) {
            self.cursor = end;
        }
    }

    fn previous(&self) -> usize {
        if let Some(chip) = self
            .chips()
            .into_iter()
            .find(|chip| chip.end() == self.cursor)
        {
            return chip.start;
        }
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(offset, _)| offset)
    }

    fn next(&self) -> usize {
        if let Some(chip) = self
            .chips()
            .into_iter()
            .find(|chip| chip.start == self.cursor)
        {
            return chip.end();
        }
        self.cursor
            + self.text[self.cursor..]
                .graphemes(true)
                .next()
                .map_or(0, str::len)
    }

    /// Removes `range`, dropping a chip it touches and moving the later ones.
    fn remove(&mut self, range: std::ops::Range<usize>) {
        let len = range.len();
        self.pastes.retain(|chip| {
            self.text.get(chip.start..chip.end()) == Some(chip.text.as_str())
                && (chip.end() <= range.start || chip.start >= range.end)
        });
        for chip in &mut self.pastes {
            if chip.start >= range.end {
                chip.start -= len;
            }
        }
        self.text.drain(range);
    }

    fn backspace(&mut self) {
        let previous = self.previous();
        self.remove(previous..self.cursor);
        self.cursor = previous;
        self.snap_cursor();
    }

    fn delete(&mut self) {
        self.remove(self.cursor..self.next());
        self.snap_cursor();
    }

    /// Hard-wraps by terminal cells so the rendered cursor matches the draft.
    /// Each drawn grapheme in the wrapped layout: the byte offset it
    /// starts at (`None` for a chip label's later graphemes), the chip it
    /// belongs to, its row and column, and its text.
    fn layout(&self, width: u16) -> Vec<Placed> {
        let width = usize::from(width.max(1));
        let chips: Vec<(usize, usize, String)> = self
            .chips()
            .into_iter()
            .map(|chip| (chip.start, chip.end(), chip.label()))
            .collect();
        // A chip draws as its label: its first grapheme carries the label's
        // start, the rest of its text draws nothing.
        let mut pieces: Vec<(Option<usize>, Option<usize>, String)> = Vec::new();
        let mut skip_to = 0;
        for (offset, grapheme) in self.text.grapheme_indices(true) {
            if offset < skip_to {
                continue;
            }
            if let Some((_, end, label)) = chips.iter().find(|(start, ..)| *start == offset) {
                skip_to = *end;
                for (index, part) in label.graphemes(true).enumerate() {
                    pieces.push((
                        (index == 0).then_some(offset),
                        Some(offset),
                        part.to_owned(),
                    ));
                }
            } else {
                pieces.push((Some(offset), None, grapheme.to_owned()));
            }
        }
        let mut placed = Vec::with_capacity(pieces.len());
        let (mut row, mut column) = (0usize, 0usize);
        for (offset, chip, text) in pieces {
            let cells = text.width();
            if text != "\n" && column + cells > width && column > 0 {
                row += 1;
                column = 0;
            }
            placed.push(Placed {
                offset,
                chip,
                row,
                column,
                cells,
                text,
            });
            let last = placed.last().expect("just pushed");
            if last.text == "\n" {
                row += 1;
                column = 0;
            } else {
                column += cells;
            }
        }
        placed
    }

    /// Hard-wraps by terminal cells so the rendered cursor matches the draft.
    pub fn wrapped(&self, width: u16) -> (Vec<String>, (u16, u16)) {
        let wide = usize::from(width.max(1));
        let mut lines = vec![String::new()];
        let mut cursor = (0, 0);
        let mut column = 0;
        for piece in self.layout(width) {
            while lines.len() <= piece.row {
                lines.push(String::new());
            }
            if piece.offset == Some(self.cursor) {
                cursor = (piece.column as u16, piece.row as u16);
            }
            if piece.text == "\n" {
                lines.push(String::new());
                column = 0;
            } else {
                lines[piece.row].push_str(&piece.text);
                column = piece.column + piece.cells;
            }
        }
        if self.cursor == self.text.len() {
            if column >= wide {
                lines.push(String::new());
                column = 0;
            }
            cursor = (column as u16, (lines.len() - 1) as u16);
        }
        (lines, cursor)
    }

    /// What a click at `column`, `row` of the wrapped draft lands on: the
    /// byte offset the cursor goes to, and the paste chip under it, if any.
    pub(crate) fn hit(&self, width: u16, column: u16, row: u16) -> (usize, Option<usize>) {
        let (column, row) = (usize::from(column), usize::from(row));
        let layout = self.layout(width);
        if let Some(piece) = layout.iter().find(|piece| {
            piece.row == row
                && piece.text != "\n"
                && column >= piece.column
                && column < piece.column + piece.cells.max(1)
        }) {
            if let Some(start) = piece.chip {
                let end = self
                    .chips()
                    .into_iter()
                    .find(|chip| chip.start == start)
                    .map_or(start, PasteChip::end);
                return (end, Some(start));
            }
            return (piece.offset.unwrap_or(self.text.len()), None);
        }
        // Past the end of a row: its line break, or just after its last
        // grapheme (a chip's whole text); a row past the end is the end.
        let on_row: Vec<&Placed> = layout.iter().filter(|piece| piece.row == row).collect();
        let end = if let Some(newline) = on_row.iter().find(|piece| piece.text == "\n") {
            newline.offset.unwrap_or(self.text.len())
        } else {
            on_row
                .last()
                .map_or(self.text.len(), |piece| match piece.chip {
                    Some(start) => self
                        .chips()
                        .into_iter()
                        .find(|chip| chip.start == start)
                        .map_or(self.text.len(), PasteChip::end),
                    None => piece
                        .offset
                        .map_or(self.text.len(), |at| at + piece.text.len()),
                })
        };
        (end, None)
    }

    /// Turns the paste chip that starts at `start` back into plain text.
    pub(crate) fn expand_chip(&mut self, start: usize) -> bool {
        let before = self.pastes.len();
        self.pastes.retain(|chip| chip.start != start);
        self.pastes.len() != before
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

    /// A `/brainstorm` lookup is Coder's own native read on every
    /// harness: it never reaches Claude Code's, Codex's or OpenCode's
    /// model, and with Brainstorm off it is refused before any request.
    #[test]
    fn a_brainstorm_lookup_never_reaches_a_harness() {
        let key = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        for harness in [
            Harness::ClaudeCode,
            Harness::Codex,
            Harness::OpenCode,
            Harness::CoderLoop,
        ] {
            let mut app = typed(harness, &format!("/brainstorm rank {key}"));
            app.plugins.bundled.claude = true;
            app.plugins.bundled.codex = true;
            app.plugins.bundled.opencode = true;
            app.submit_live();
            assert!(app.request.is_none(), "{harness:?}");
            assert!(app.live.entries.is_empty(), "{harness:?}");
            assert!(!app.live.busy, "{harness:?}");
            assert!(
                app.live
                    .notice
                    .as_deref()
                    .is_some_and(|notice| notice.starts_with("Enable Brainstorm")),
                "{harness:?}: {:?}",
                app.live.notice
            );
        }
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

    /// #11480: a pick in /models takes this chat's next Claude Code
    /// message, a pin's model included, and says so in plain words.
    #[test]
    fn a_claude_model_pick_takes_the_next_message_over_a_pin() {
        let mut app = typed(Harness::ClaudeCode, "");
        app.live.pinned_model = Some(claude::MODEL.into());
        app.live.claude_session = Some(live::ClaudeSession {
            id: "5b0c1d2e".into(),
            users: 1,
        });
        let said = app.claude_model_switched("claude-opus-5-5");
        assert_eq!(app.live.pinned_model, None);
        assert_eq!(
            said,
            "Your next message runs on Opus 5.5, in this same chat."
        );

        // A chat on another harness keeps its pin.
        let mut app = typed(Harness::CoderLoop, "");
        app.live.pinned_model = Some("x-ai/grok-4.7".into());
        assert_eq!(
            app.claude_model_switched("claude-sonnet-5-5"),
            "claude now runs Sonnet 5.5."
        );
        assert_eq!(app.live.pinned_model.as_deref(), Some("x-ai/grok-4.7"));
    }

    /// #11482: a background subagent opened in one turn stays running,
    /// takes its tokens and end in a later turn without opening a new
    /// agent, and keeps the time its first end took.
    #[test]
    fn a_background_agent_settles_in_a_later_turn() {
        use bundled_runtime::RuntimeEvent;
        let tool = |running: bool, output: serde_json::Value| RuntimeEvent::Tool {
            name: "acp_subagent".into(),
            input: serde_json::Value::Null,
            output,
            running,
        };
        let mut app = typed(Harness::ClaudeCode, "");
        app.apply_delegation(
            "6:t1".into(),
            "Plan".into(),
            "sketch".into(),
            tool(true, serde_json::Value::Null),
        );
        // The launch result keeps it running.
        app.apply_delegation(
            "6:t1".into(),
            "Plan".into(),
            "sketch".into(),
            tool(true, serde_json::Value::Null),
        );
        assert!(app.delegations[0].running);
        assert!(
            app.live
                .entries
                .iter()
                .any(|entry| matches!(entry, live::Entry::Delegation { running: true, .. }))
        );

        app.elapsed_seconds = 40;
        assert!(
            app.apply_known_delegation(
                "t1",
                "Claude".into(),
                String::new(),
                RuntimeEvent::Tokens(14_180)
            )
            .is_ok()
        );
        assert_eq!(app.delegations.len(), 1, "no second agent");
        assert_eq!(app.delegations[0].chat.tokens, 14_180);
        assert!(
            app.apply_known_delegation(
                "t1",
                "Claude".into(),
                String::new(),
                tool(false, serde_json::json!({"tokens": 15_000}))
            )
            .is_ok()
        );
        assert!(!app.delegations[0].running);
        assert_eq!(app.delegations[0].elapsed_seconds, 40);
        assert_eq!(app.delegations[0].chat.tokens, 15_000);
        assert!(
            app.live
                .entries
                .iter()
                .any(|entry| matches!(entry, live::Entry::Delegation { running: false, .. }))
        );
        app.elapsed_seconds = 90;
        app.apply_delegation(
            "6:t1".into(),
            "Plan".into(),
            "sketch".into(),
            tool(false, serde_json::json!({})),
        );
        assert_eq!(
            app.delegations[0].elapsed_seconds, 40,
            "a repeated end keeps its time"
        );
        assert!(
            app.apply_known_delegation(
                "t9",
                "Claude".into(),
                String::new(),
                RuntimeEvent::Tokens(1)
            )
            .is_err()
        );
    }

    /// #11480: Ctrl+N empties the chat in place, same checkout; busy work
    /// must stop first.
    #[test]
    fn ctrl_n_starts_a_new_chat_in_place() {
        let ctrl_n = || {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char('n'),
                KeyModifiers::CONTROL,
            ))
        };
        let mut app = typed(Harness::ClaudeCode, "half a thought");
        app.cwd = Some(std::path::PathBuf::from("/tmp/repo"));
        app.live.entries.push(live::Entry::User("old".into()));
        app.live.busy = true;
        assert!(app.handle(ctrl_n()));
        assert_eq!(app.live.entries.len(), 1, "busy work is not dropped");

        app.live.busy = false;
        assert!(app.handle(ctrl_n()));
        assert!(app.live.entries.is_empty());
        assert!(app.draft.text.is_empty());
        assert_eq!(app.live.runs_on, None, "the new chat is routed anew");
        assert_eq!(app.cwd.as_deref(), Some(std::path::Path::new("/tmp/repo")));
        assert!(app.notice.as_deref().unwrap().contains("/resume"));
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

    /// An agent that never reported back stops working with the turn: the
    /// rail and the program status return to idle (#11462).
    #[test]
    fn an_agent_that_never_reported_back_ends_with_the_turn() {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.live.busy = true;
        app.apply_update(live::Update::Delegation {
            id: app.request_id,
            delegation: "d1".into(),
            name: "cursor".into(),
            task: "check".into(),
            event: bundled_runtime::RuntimeEvent::Text("Starting".into()),
        });
        assert!(app.delegations[0].chat.busy);
        app.apply_update(live::Update::Finished {
            id: app.request_id,
            result: Err("The agent failed.".into()),
        });
        assert_eq!(app.program_status().0, "idle");
        assert!(!app.delegations[0].running);
        assert!(!app.delegations[0].chat.busy);
        assert!(
            !app.live
                .entries
                .iter()
                .any(|entry| matches!(entry, live::Entry::Delegation { running: true, .. }))
        );
    }

    /// Claude Code's question opens a choice screen: the program status
    /// says the chat waits on the person, and the pick goes back as the
    /// tool's answer (#11449).
    #[test]
    fn a_claude_question_is_answered_on_the_choice_screen() {
        let asks = claude_ask::Asks::default();
        let waiting = {
            let asks = asks.clone();
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(async move {
                        asks.ask(&serde_json::json!({"questions":[{"question":"Which one?","header":"Pick","multiSelect":false,"options":[{"label":"A","description":""},{"label":"B","description":""}]}]}))
                            .await
                    })
            })
        };
        while asks.waiting().is_none() {
            std::thread::yield_now();
        }
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.question = Some(claude_ask::Screen::new(
            asks.clone(),
            asks.waiting().unwrap(),
        ));
        assert_eq!(
            app.program_status(),
            ("blocked", Some("Which one?".to_owned()))
        );
        let press = |app: &mut App, code| {
            app.handle(Event::Key(crossterm::event::KeyEvent::new(
                code,
                KeyModifiers::NONE,
            )))
        };
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert!(app.question.is_none());
        let answered = waiting.join().unwrap().unwrap();
        assert_eq!(answered["answers"]["Which one?"], "B");
        assert_eq!(app.program_status().0, "idle");
    }
}
