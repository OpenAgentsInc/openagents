//! Background provider and plugin work, separate from demo fixtures.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use model_access::ApiKey;
use openrouter::{Message, Streamed};
use tokio::sync::oneshot;

use crate::provider::{KeyInfo, Provider};
use crate::{bundled_runtime::RuntimeEvent, plugin_tools::ExecutionSettings};

#[derive(Default)]
pub struct Chat {
    pub entries: Vec<Entry>,
    pub partial: String,
    /// Whether this turn's reply already streamed into `partial`. A
    /// finished reply repeats every streamed word, so only the uncommitted
    /// tail is still owed; a mid-turn user entry (a background command's
    /// result, a queued notice) must not look like a new turn here.
    pub text_streamed: bool,
    pub partial_model: Option<String>,
    pub reply_started_at: Option<std::time::Instant>,
    pub busy: bool,
    pub notice: Option<String>,
    pub tokens: u64,
    /// The provider-reported dollars this chat's replies cost (#11179).
    pub cost_usd: f64,
    /// The model the person chose failed or refused this turn and Coder
    /// kept to it (#11132, [`crate::provider::PINNED_MISSED`]): the turn's
    /// failure offers another model.
    pub pinned_missed: bool,
    /// Standing instructions the model reads as system instructions every
    /// turn (`coder chat --instructions`). They are never an entry, so the
    /// transcript, a follower, and an export never show them.
    pub instructions: Option<String>,
    pub(crate) cache: crate::ui::TranscriptCache,
    /// Each model call's usage, with the index of the first entry it
    /// produced, for the export.
    pub rounds: Vec<(usize, crate::bundled_runtime::RoundUsage)>,
    /// Tokens and dollars this turn's calls already added to the totals,
    /// so the finished turn adds only the rest.
    pub turn_counted: (u64, f64),
    /// When each entry first appeared, in milliseconds since the epoch.
    pub stamps: Vec<u64>,
    /// How long each finished tool call ran, by entry index.
    pub tool_ms: std::collections::BTreeMap<usize, u64>,
    pub open_tools: std::collections::BTreeSet<usize>,
    /// The model's thinking is streaming in now, so the input's rail says
    /// `Thinking` beside `Working`.
    pub thinking: bool,
    /// Once the chat passes [`crate::long_session::ELIDE_PERCENT`] of the
    /// context window (#11286), bulky tool outputs older than the newest
    /// [`crate::long_session::VERBATIM_TURNS`] requests reach the model as
    /// short stubs. The transcript and the export keep them whole.
    pub elide_old_outputs: bool,
    /// The largest estimated context the model read, for the export.
    pub peak_context_tokens: u64,
    /// The harness this chat's tool turns run under (#11289), once decided.
    pub harness: Option<crate::harness::Run>,
    /// The Claude Code session this chat's claude turns continue (#11435).
    pub claude_session: Option<ClaudeSession>,
    /// The Codex thread this chat's Codex turns continue (#11510).
    pub codex_session: Option<ClaudeSession>,
    /// The OpenCode session this chat's OpenCode turns continue, and the
    /// model it last ran (#11507).
    pub opencode_session: Option<OpenCodeSession>,
    /// The harness plugin this chat's turns run on, once its first turn
    /// was routed ([`App::route_turn`](crate::App::route_turn)); follow-ups
    /// stay on it (#11453).
    pub runs_on: Option<crate::harness_plugin::Harness>,
    /// The person pinned [`Chat::runs_on`] (`coder chat --mode`, a pick
    /// in the terminal): follow-ups are not routed, in this process or a
    /// later one that reopens the chat (#11525).
    pub pinned: bool,
    /// The briefing every harness reads for this chat's current piece of
    /// work, chosen at its routing point (#11473); `None` loads what each
    /// harness loaded before.
    pub briefing: Option<crate::context::Briefing>,
    /// Which of the person's messages started the current piece of work,
    /// counting from zero (#11474).
    pub work_started: usize,
    /// The current piece of work was tried again at a checkpoint.
    pub retried: bool,
    /// The current piece of work moved harness at a checkpoint.
    pub moved: bool,
    /// The model a pin names for this chat's turns on its harness (#11475).
    pub pinned_model: Option<String>,
    /// The router judged the current piece of work a cheap answer
    /// (#11537): its Coder loop turns run on a cheap model.
    pub cheap: bool,
    /// The door the chat's first Coder-loop answer came through (#11563):
    /// later turns go only through it, so the chat keeps its model's
    /// payer. When it can't answer, the turn says why instead of moving
    /// to another payer.
    pub(crate) door: Option<crate::route::Lane>,
    /// The ACP agent the person pinned this chat to with Shift+Tab
    /// (#11524), by its id: its turns run there instead of on a harness.
    pub agent: Option<String>,
    /// The harness Auto had the chat on before a Shift+Tab pick, so going
    /// back to Auto carries on there.
    pub(crate) before_pin: Option<crate::harness_plugin::Harness>,
    /// The files each recent turn changed, newest last, so `/rewind` can
    /// undo them (#11515).
    pub(crate) turns: Vec<crate::shell_changes::TurnRecord>,
    /// `turns` changed since the chat was last saved.
    pub(crate) turns_changed: bool,
    /// The checkout's changed files as the running turn began, and where
    /// its message sits in `entries`.
    pub(crate) turn_start: Option<crate::rewind::Start>,
    /// The model and payer this chat runs on, from its first answer on
    /// (#11563): saved with the chat, so a reopened chat continues on them.
    pub ran: Option<Ran>,
}

/// The model a chat's first answer ran on and who paid for it (#11563).
/// With [`Chat::runs_on`] or [`Chat::agent`] it is the chat's lock: a
/// reopened chat goes on there, never on the default model.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ran {
    /// The harness's or agent's own model id ("claude-opus-5-5",
    /// "gpt-6.1-sol", "opencode/big-pickle"), or `auto` for the Coder
    /// loop's gateway pick.
    pub model: String,
    /// The effort it ran at, when the harness takes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    pub payer: crate::harness_plugin::Payer,
}

/// A Claude Code session a chat holds, and how far into the chat it reaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaudeSession {
    pub id: String,
    /// How many of the person's messages the session has read. A message
    /// another engine answered since then is missing from it, so the next
    /// claude turn starts a new session from the whole conversation instead.
    pub users: usize,
}

/// An OpenCode session a chat holds (#11507), as [`ClaudeSession`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenCodeSession {
    pub id: String,
    /// How many of the person's messages the session has read.
    pub users: usize,
    /// The model it last ran.
    pub model: String,
}

impl Chat {
    /// The OpenCode session the next OpenCode turn continues, and the
    /// model it last ran: the one the chat holds, when it has read every
    /// message the person sent before the newest.
    #[must_use]
    pub fn opencode_resume(&self) -> Option<(String, String)> {
        self.opencode_session
            .as_ref()
            .filter(|session| session.users + 1 == self.user_count())
            .map(|session| (session.id.clone(), session.model.clone()))
    }

    /// Whether a turn of this chat has answered: from then on it stays on
    /// its harness and model (#11556).
    #[must_use]
    pub fn answered(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| matches!(entry, Entry::Assistant { .. }))
    }

    /// How many of the person's messages the chat holds.
    #[must_use]
    pub fn user_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| matches!(entry, Entry::User(_)))
            .count()
    }

    /// The Claude Code session the next claude turn continues: the one the
    /// chat holds, when it has read every message the person sent before
    /// the newest.
    #[must_use]
    pub fn claude_resume(&self) -> Option<String> {
        self.claude_session
            .as_ref()
            .filter(|session| session.users + 1 == self.user_count())
            .map(|session| session.id.clone())
    }

    /// The Codex thread the next Codex turn continues, on the same rule.
    #[must_use]
    pub fn codex_resume(&self) -> Option<String> {
        self.codex_session
            .as_ref()
            .filter(|session| session.users + 1 == self.user_count())
            .map(|session| session.id.clone())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    User(String),
    Assistant {
        text: String,
        model: Option<String>,
        elapsed_ms: Option<u64>,
    },
    Tool {
        name: String,
        input: serde_json::Value,
        output: serde_json::Value,
        running: bool,
    },
    Delegation {
        id: String,
        name: String,
        task: String,
        running: bool,
        output: serde_json::Value,
        /// The run's latest step and Jev's estimate of how much is done,
        /// while a Microcoder run reports them. Never a budget.
        progress: Option<(usize, Option<f64>)>,
    },
}

pub struct Delegation {
    pub id: String,
    pub name: String,
    pub task: String,
    pub chat: Chat,
    pub started_at: u64,
    pub elapsed_seconds: u64,
    pub running: bool,
    pub(crate) draft: crate::Draft,
    pub(crate) composer: crate::composer_state::ComposerState,
    pub(crate) scroll: u16,
    /// A background agent (#11163): messages typed here go to it through
    /// the agent list instead of starting a foreground run.
    pub background: bool,
}

impl Chat {
    pub fn reply_elapsed_ms(&self) -> Option<u64> {
        self.reply_started_at
            .map(|started| started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64)
    }

    pub fn layout_builds(&self) -> usize {
        self.cache.builds
    }
    pub fn finish_partial(&mut self) {
        let text = std::mem::take(&mut self.partial);
        let text = text.trim();
        if !text.is_empty() {
            self.entries.push(Entry::Assistant {
                elapsed_ms: self.reply_elapsed_ms(),
                text: text.to_owned(),
                model: self.partial_model.take(),
            });
        }
    }

    /// Record one model call's usage against the entries it produces.
    pub fn round(&mut self, usage: crate::bundled_runtime::RoundUsage) {
        // A call's usage that arrives after its reply was already written
        // (a reply after thinking is written when the thinking ends)
        // belongs to that reply, not to the next message.
        let last = self.entries.len().checked_sub(1);
        let index = match (last, self.entries.last()) {
            (Some(last), Some(Entry::Assistant { .. }))
                if self.partial.is_empty() && !self.rounds.iter().any(|(at, _)| *at == last) =>
            {
                last
            }
            _ => self.entries.len(),
        };
        self.rounds.push((index, usage));
    }

    /// Stamp entries that appeared since the last stamp, and time tool
    /// calls from when they appeared to when they finished.
    pub fn stamp(&mut self) {
        let now = atif::now_ms();
        self.stamps.truncate(self.entries.len());
        self.rounds
            .retain(|(index, _)| *index <= self.entries.len());
        while self.stamps.len() < self.entries.len() {
            self.stamps.push(now);
        }
        // A delegation is timed like a tool call: a foreground run to
        // Solver lasts as long as Solver worked on it (#11506).
        for (index, entry) in self.entries.iter().enumerate() {
            if let Entry::Tool { running, .. } | Entry::Delegation { running, .. } = entry {
                if *running {
                    self.open_tools.insert(index);
                } else if self.open_tools.remove(&index) {
                    self.tool_ms
                        .insert(index, now.saturating_sub(self.stamps[index]));
                }
            }
        }
        self.tool_ms.retain(|index, _| *index < self.entries.len());
        self.open_tools.retain(|index| *index < self.entries.len());
    }

    /// A piece of the model's thinking as it streams: it joins the running
    /// thinking entry, or opens one. The finished block's entry closes it.
    pub fn think(&mut self, piece: &str) {
        if let Some(Entry::Tool {
            name,
            output,
            running: true,
            ..
        }) = self.entries.last_mut()
            && name == crate::claude_stream::THINKING
        {
            let mut text = crate::claude_stream::entry_text(output).to_owned();
            text.push_str(piece);
            *output = serde_json::json!({ "text": text });
            return;
        }
        self.finish_partial();
        self.entries.push(Entry::Tool {
            name: crate::claude_stream::THINKING.into(),
            input: serde_json::Value::Null,
            output: serde_json::json!({ "text": piece }),
            running: true,
        });
    }

    /// Thinking still streaming when a turn ends keeps what it said.
    pub fn close_thinking(&mut self) {
        for entry in &mut self.entries {
            if let Entry::Tool { name, running, .. } = entry
                && *running
                && name == crate::claude_stream::THINKING
            {
                *running = false;
            }
        }
    }

    pub fn stop_tools(&mut self, reason: &str) {
        self.close_thinking();
        for entry in &mut self.entries {
            if let Entry::Tool {
                running, output, ..
            } = entry
            {
                if *running {
                    *running = false;
                    *output = serde_json::json!({"error":reason});
                }
            }
        }
    }

    pub fn tool(
        &mut self,
        name: String,
        input: serde_json::Value,
        output: serde_json::Value,
        running: bool,
    ) {
        self.finish_partial();
        if let Some(Entry::Tool { input: previous_input, output: previous_output, running: previous_running, .. }) = self.entries.iter_mut().rev().find(|entry| {
            matches!(entry, Entry::Tool { name: previous, input: previous_input, running: true, .. } if previous == &name && (input.is_null() || previous_input == &input))
        }) {
            if !input.is_null() { *previous_input = input; }
            *previous_output = output;
            *previous_running = running;
        } else {
            self.entries.push(Entry::Tool { name, input, output, running });
        }
    }

    pub fn messages(&self) -> Vec<Message> {
        let newest_brainstorm = self.entries.iter().rposition(|entry| matches!(entry,
            Entry::Tool { name, output, running: false, .. } if crate::brainstorm::is_tool(name) && (output.get("observation").is_some() || output.get("error").is_some())));
        // A compacted chat (#11179): the model reads from the newest
        // summary on; earlier entries stay on screen only.
        let start = crate::long_session::context_start(&self.entries);
        let elide_before = if self.elide_old_outputs {
            crate::long_session::verbatim_start(&self.entries)
        } else {
            0
        };
        let current_plan = self.entries.iter().rev().find_map(|entry| match entry {
            Entry::Tool {
                name,
                output,
                running: false,
                ..
            } if name == crate::plan::TOOL => crate::plan::message(output),
            _ => None,
        });
        current_plan
            .into_iter()
            .chain(
                self.entries
                    .iter()
                    .enumerate()
                    .skip(start)
                    .filter_map(|(index, entry)| {
                        if let Entry::Tool { name, .. } = entry
                            && (crate::long_session::hidden_from_model(name)
                                || name == crate::plan::TOOL)
                        {
                            return None;
                        }
                        if let Entry::Tool {
                            name,
                            output,
                            running: false,
                            ..
                        } = entry
                        {
                            if name == crate::long_session::COMPACT_TOOL {
                                return crate::long_session::summary_message(output);
                            }
                            if crate::brainstorm::is_tool(name) {
                                if newest_brainstorm != Some(index) {
                                    return None;
                                }
                                return crate::brainstorm::context(output).map(|content| Message {
                                    role: "assistant".into(),
                                    content,
                                });
                            }
                        }
                        Some(match entry {
                            Entry::User(text) => Message::user(text.clone()),
                            Entry::Assistant { text, .. } => Message {
                                role: "assistant".into(),
                                content: text.clone(),
                            },
                            Entry::Tool {
                                name,
                                input,
                                output,
                                running,
                            } => Message {
                                role: "assistant".into(),
                                content: match (index < elide_before && !*running)
                                    .then(|| crate::long_session::stub(name, input, output))
                                    .flatten()
                                {
                                    Some(stub) => stub,
                                    None => format!(
                                        "Tool observation from {name} ({}): {output}",
                                        if *running { "running" } else { "finished" }
                                    ),
                                },
                            },
                            Entry::Delegation {
                                name,
                                task,
                                running,
                                output,
                                ..
                            } => Message {
                                role: "assistant".into(),
                                content: format!(
                                    "Delegation to {name}: {task} ({}): {output}",
                                    if *running { "running" } else { "finished" }
                                ),
                            },
                        })
                    }),
            )
            .collect()
    }
}

pub(crate) fn model_slug(model: &str) -> Option<String> {
    (!model.is_empty()
        && model.len() <= 1024
        && model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-/:".contains(&byte)))
    .then(|| model.to_owned())
}

pub struct Request {
    pub id: u64,
    pub key: ApiKey,
    pub kind: Work,
}

pub enum Work {
    Bash {
        command: String,
        cwd: std::path::PathBuf,
    },
    Brainstorm {
        job: crate::brainstorm::Job,
    },
    Check,
    CheckJev {
        endpoint: String,
        model: String,
    },
    Chat {
        model: String,
        options: crate::models::GenerationOptions,
        messages: Vec<Message>,
        execution: ExecutionSettings,
    },
    Microcoder {
        messages: Vec<Message>,
        execution: ExecutionSettings,
        /// The person's generation options, for the route's OpenRouter
        /// door when it takes the turn; the house doors ignore them.
        options: crate::models::GenerationOptions,
    },
    /// A claude turn: the person's message to the chat's long-lived Claude
    /// Code process ([`crate::claude_chat`]), started for it when the chat
    /// has none.
    Claude {
        messages: Vec<Message>,
        cwd: std::path::PathBuf,
        /// Coder's own tools the chat may hand work to: background agents,
        /// Solver, cloud jobs and the rest a live chat offers
        /// ([`crate::claude_tools`]).
        tools: Box<crate::claude_tools::Tools>,
        /// The Claude Code session the chat holds, which this turn
        /// continues; `None` starts one (#11435).
        session: Option<String>,
        /// The chat's briefing for this piece of work (#11473).
        briefing: Option<Box<crate::context::Briefing>>,
        /// The model a pin names (#11475); `None` runs the person's pick
        /// in /models, read for every turn.
        model: Option<String>,
    },
    /// An OpenCode turn (#11507): one `opencode run` in the chat's OpenCode
    /// session ([`crate::opencode_chat`]), started for it when the chat has
    /// none.
    OpenCode {
        messages: Vec<Message>,
        cwd: std::path::PathBuf,
        /// The session this turn continues and the model it last ran.
        session: Option<(String, String)>,
        briefing: Option<Box<crate::context::Briefing>>,
        model: String,
        /// Offer Coder's tools over MCP.
        tools: bool,
    },
    /// A Codex turn: the person's message to the chat's long-lived Codex
    /// process ([`crate::codex_chat`]), started for it when the chat has
    /// none (#11510).
    Codex {
        messages: Vec<Message>,
        cwd: std::path::PathBuf,
        /// Coder's own tools, offered to Codex in process.
        tools: Box<crate::claude_tools::Tools>,
        /// The Codex thread the chat holds, which this turn continues.
        session: Option<String>,
        briefing: Option<Box<crate::context::Briefing>>,
        /// The model a pin names; `None` runs the pick in /models.
        model: Option<String>,
    },
    /// A turn on an ACP agent the person picked with Shift+Tab (#11524):
    /// one session for the turn, reading the conversation so far, every
    /// step through Coder's checks ([`crate::bundled_runtime::acp_briefed`]).
    Acp {
        agent: crate::bundled_runtime::AcpAgent,
        messages: Vec<Message>,
        cwd: std::path::PathBuf,
        briefing: Option<String>,
    },
    /// A turn of solver's conversational Intaker: Haiku with a read-only
    /// `gh`, helping the person find or pick the issue to solve.
    SolverIntake {
        messages: Vec<Message>,
        cwd: std::path::PathBuf,
        github: String,
        /// Claude Code is on and signed in; otherwise the Intaker runs on
        /// the Coder loop through the gateway, on `account` (#11518).
        claude: bool,
        account: Option<openagents_login::Saved>,
    },
    /// A summary of the older part of the chat (#11179), written by the
    /// chat's model without tools; nothing streams into the transcript.
    Summarize {
        model: String,
        messages: Vec<Message>,
    },
    /// The same summary on a no-harness chat's one model (#11548),
    /// through the gateway while signed in, else the person's Gemini key.
    SummarizeNoHarness {
        messages: Vec<Message>,
        no_harness: crate::route::NoHarness,
        account: Option<openagents_login::Saved>,
    },
    /// A multi-part ask's independent parts at the same time (#11539):
    /// each a delegated child in its own worktree, then one converge step
    /// and the combined report as the reply.
    Fanout {
        run: Box<crate::fanout::Run>,
    },
    Delegate {
        delegation: String,
        name: String,
        tool: String,
        arguments: serde_json::Value,
        execution: ExecutionSettings,
        model: String,
        options: crate::models::GenerationOptions,
    },
}

pub enum Update {
    BrainstormFinished {
        id: u64,
        generation: u64,
        result: Result<crate::brainstorm::Outcome, brainstorm_client::Error>,
    },
    Checked {
        id: u64,
        result: Result<KeyInfo, String>,
    },
    CheckedJev {
        id: u64,
        result: Result<Vec<String>, String>,
    },
    Tool {
        id: u64,
        name: String,
        input: serde_json::Value,
        output: serde_json::Value,
        running: bool,
    },
    Delegation {
        id: u64,
        delegation: String,
        name: String,
        task: String,
        event: RuntimeEvent,
    },
    Delta {
        id: u64,
        text: String,
    },
    /// One model call's usage, as soon as its stream ends.
    Usage {
        id: u64,
        usage: crate::bundled_runtime::RoundUsage,
    },
    Harness {
        id: u64,
        run: crate::harness::Run,
    },
    /// The model's thinking started or stopped streaming in.
    Thinking {
        id: u64,
        on: bool,
    },
    /// A piece of the model's thinking, as it streams.
    ThinkingText {
        id: u64,
        text: String,
    },
    Model {
        id: u64,
        model: String,
    },
    /// The OpenCode session an OpenCode turn runs in (#11507).
    OpenCodeSession {
        id: u64,
        session: String,
        model: String,
    },
    Finished {
        id: u64,
        result: Result<Streamed, String>,
    },
}

impl Update {
    pub fn id(&self) -> u64 {
        match self {
            Self::Checked { id, .. }
            | Self::BrainstormFinished { id, .. }
            | Self::CheckedJev { id, .. }
            | Self::Tool { id, .. }
            | Self::Delegation { id, .. }
            | Self::Delta { id, .. }
            | Self::Usage { id, .. }
            | Self::Harness { id, .. }
            | Self::Thinking { id, .. }
            | Self::ThinkingText { id, .. }
            | Self::Model { id, .. }
            | Self::OpenCodeSession { id, .. }
            | Self::Finished { id, .. } => *id,
        }
    }
}

/// The message-route hook's note for `newest` (#11491).
fn route_note(
    app: &crate::App,
    newest: &str,
    model: &str,
    midturn: bool,
) -> crate::message_route::Note {
    let mut earlier = crate::router::earlier(&app.live.entries);
    if earlier
        .last()
        .is_some_and(|said| said.role == "user" && said.text == newest)
    {
        earlier.pop();
    }
    crate::message_route::Note {
        earlier,
        latest: newest.to_owned(),
        midturn,
        facts: crate::message_route::Facts {
            model: Some(crate::claude::name(model)),
            spend: (app.live.tokens > 0).then_some((app.live.tokens, app.live.cost_usd)),
        },
    }
}

/// One active request. Cancellation also waits for child process cleanup.
#[derive(Default)]
pub struct Background {
    active: Option<(u64, oneshot::Sender<()>, mpsc::Receiver<Update>)>,
    /// The claude chat's long-lived Claude Code process (#11447).
    claude: Option<crate::claude_chat::Chat>,
    /// The claude turn on screen: its request id and the process's number
    /// for it.
    claude_turn: Option<(u64, u64)>,
    /// The newest process turn that was stopped; what it still reports is
    /// dropped.
    claude_stopped: u64,
    /// The newest process turn that ended; what it still reports opens no
    /// new turn.
    claude_ended: u64,
    /// Processes of chats that were closed, finishing their work.
    claude_closing: Vec<crate::claude_chat::Chat>,
    /// The Codex chat's long-lived process (#11510). It shares the turn
    /// bookkeeping above: a chat runs on one harness at a time.
    codex: Option<crate::codex_chat::Chat>,
    codex_closing: Vec<crate::codex_chat::Chat>,
    /// The newest message handed to the chat's process, kept so a re-brief
    /// (#11491) can start it again in a fresh session.
    claude_last: Option<ClaudeAsk>,
}

/// A message handed to the chat's Claude Code process.
struct ClaudeAsk {
    message: Message,
    tools: crate::claude_tools::Tools,
    briefing: Option<crate::context::Briefing>,
    model: String,
}

impl Background {
    pub fn sync(&mut self, app: &mut crate::App) {
        if self
            .active
            .as_ref()
            .is_some_and(|(id, _, _)| *id != app.request_id)
        {
            self.cancel();
        }
        // Stopping a claude turn interrupts it; the process stays.
        if let Some((request_id, turn)) = self.claude_turn
            && request_id != app.request_id
        {
            self.claude_turn = None;
            if let Some(chat) = &self.claude {
                self.claude_stopped = self.claude_stopped.max(chat.interrupt(turn));
            }
            if let Some(chat) = &self.codex {
                self.claude_stopped = self.claude_stopped.max(chat.interrupt(turn));
            }
        }
        // A process belongs to the chat that holds its session; another chat
        // on screen closes it.
        if self.claude.as_ref().is_some_and(|chat| {
            chat.session.is_some()
                && chat.session.as_deref()
                    != app
                        .live
                        .claude_session
                        .as_ref()
                        .map(|session| session.id.as_str())
        }) {
            self.close_claude();
        }
        if self.codex.as_ref().is_some_and(|chat| {
            chat.session.is_some()
                && chat.session.as_deref()
                    != app
                        .live
                        .codex_session
                        .as_ref()
                        .map(|session| session.id.as_str())
        }) {
            self.close_codex();
        }
        if let Some(request) = app
            .request
            .take_if(|request| matches!(request.kind, Work::Claude { .. }))
        {
            self.cancel();
            self.close_codex();
            self.claude_request(request, app);
        }
        if let Some(request) = app
            .request
            .take_if(|request| matches!(request.kind, Work::Codex { .. }))
        {
            self.cancel();
            if self.claude.is_some() {
                self.close_claude();
            }
            self.codex_request(request, app);
        }
        if let Some(request) = app.request.take() {
            self.cancel();
            let id = request.id;
            let (sender, receiver) = mpsc::channel();
            let (cancel, canceled) = oneshot::channel();
            std::thread::spawn(move || run(request, sender, canceled));
            self.active = Some((id, cancel, receiver));
        }
        let mut finished = false;
        if let Some((id, _, receiver)) = &self.active {
            loop {
                match receiver.try_recv() {
                    Ok(update) => {
                        finished |= matches!(
                            update,
                            Update::Checked { .. }
                                | Update::BrainstormFinished { .. }
                                | Update::CheckedJev { .. }
                                | Update::Finished { .. }
                        );
                        app.apply_update(update);
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if !finished {
                            let error =
                                "The chat worker stopped before completing the request.".into();
                            app.apply_update(if let Some(job) = &app.brainstorm_job {
                                Update::BrainstormFinished {
                                    id: *id,
                                    generation: job.generation,
                                    result: Err(brainstorm_client::Error::Transport),
                                }
                            } else if app.checking_jev {
                                Update::CheckedJev {
                                    id: *id,
                                    result: Err(error),
                                }
                            } else if app.checking_key {
                                Update::Checked {
                                    id: *id,
                                    result: Err(error),
                                }
                            } else {
                                Update::Finished {
                                    id: *id,
                                    result: Err(error),
                                }
                            });
                        }
                        finished = true;
                        break;
                    }
                }
            }
        }
        if finished {
            self.active = None;
        }
        self.poll_claude(app);
        self.poll_codex(app);
        self.show_question(app);
        app.claude_turn = self.claude_turn.is_some();
        app.poll_disclosure();
        app.poll_fleet();
        app.poll_shells();
        app.poll_long_session();
        app.poll_route();
        app.poll_escalation();
        app.process_prompt_queue();
    }

    /// Claude Code's question waiting on the person opens the choice
    /// screen; one no longer waiting (its turn stopped) closes it (#11449).
    fn show_question(&self, app: &mut crate::App) {
        if app
            .question
            .as_ref()
            .is_some_and(|screen| !screen.current())
        {
            app.question = None;
        }
        if app.question.is_none()
            && let Some(chat) = &self.claude
            && let Some(questions) = chat.asks.waiting()
        {
            app.question = Some(crate::claude_ask::Screen::new(chat.asks.clone(), questions));
        }
    }

    fn cancel(&mut self) {
        if let Some((_, sender, _)) = self.active.take() {
            let _ = sender.send(());
        }
    }

    /// Hands a claude message to the chat's process: sent at once to the
    /// one already running this chat, mid-turn included, or to a new one
    /// started on the chat's session.
    fn claude_request(&mut self, request: Request, app: &mut crate::App) {
        let Work::Claude {
            messages,
            cwd,
            tools,
            session,
            briefing,
            model,
        } = request.kind
        else {
            return;
        };
        let newest = messages
            .last()
            .map(|message| message.content.clone())
            .unwrap_or_default();
        // The model this turn runs on, read now rather than when the
        // process started, so a switch in /models takes the next turn
        // (#11480).
        let model = crate::claude::turn_model(model);
        let running = self.claude.as_ref().is_some_and(|chat| {
            !chat.exited() && chat.cwd == cwd && chat.tools == *tools && chat.session == session
        });
        // What the message-route hook reads (#11491): the chat before this
        // message, and what Coder knows without asking the agent. A new
        // process without a session reads the whole conversation as its
        // first message, which is not routed.
        let midturn = self.claude_turn.is_some();
        if running || session.is_some() {
            crate::message_route::note(&cwd, route_note(app, &newest, &model, midturn));
        }
        if let Some(last) = messages.last() {
            self.claude_last = Some(ClaudeAsk {
                message: last.clone(),
                tools: (*tools).clone(),
                briefing: briefing.as_deref().cloned(),
                model: model.clone(),
            });
        }
        if !running {
            self.close_claude();
            let seed = crate::claude::task_with(&messages, &tools, &model);
            self.claude = Some(crate::claude_chat::Chat::start(
                cwd,
                *tools,
                session,
                newest,
                seed,
                briefing.map(|briefing| *briefing),
                Some(model),
            ));
        } else if let Some(chat) = &mut self.claude {
            // A new piece of work in a running process: its briefing goes
            // with the message rather than restarting the process, so the
            // session's prompt cache stays (#11473).
            let key = briefing.as_ref().map(|briefing| briefing.key());
            let newest = match briefing.filter(|_| key != chat.briefing) {
                Some(briefing) => {
                    chat.briefing = key;
                    // A compaction would summarize this message away; the
                    // context hook adds it back after one (#11490).
                    crate::context_hook::note_message_briefing(&cwd, Some(briefing.render(false)));
                    crate::claude::with_briefing(&briefing, &newest)
                }
                None => newest,
            };
            // A model switched since the last message: the process switches
            // before reading this one, which carries the news (#11480).
            let newest = if chat.model == model {
                newest
            } else {
                chat.set_model(&model);
                crate::claude::with_switch(&model, &newest)
            };
            chat.say(&newest, self.claude_turn.is_some());
            // The session has read this message too.
            let users = app.live.user_count();
            if let Some(session) = &mut app.live.claude_session {
                session.users = users;
            }
        }
        let turn = self
            .claude
            .as_ref()
            .map_or(1, crate::claude_chat::Chat::turn);
        self.claude_turn = Some((request.id, turn));
    }

    /// Starts the chat's newest message again in a fresh Claude Code
    /// session (#11491): the current briefing in its system prompt and
    /// only this message, so the old history is not re-read on every later
    /// call. Returns whether it started.
    fn rebrief(&mut self, app: &mut crate::App, id: u64) -> bool {
        let (Some(ask), Some(cwd)) = (
            self.claude_last.take(),
            self.claude.as_ref().map(|chat| chat.cwd.clone()),
        ) else {
            return false;
        };
        self.close_claude();
        app.live.claude_session = None;
        let seed =
            crate::claude::task_with(std::slice::from_ref(&ask.message), &ask.tools, &ask.model);
        self.claude = Some(crate::claude_chat::Chat::start(
            cwd,
            ask.tools,
            None,
            ask.message.content,
            seed,
            ask.briefing,
            Some(ask.model),
        ));
        let turn = self
            .claude
            .as_ref()
            .map_or(1, crate::claude_chat::Chat::turn);
        self.claude_turn = Some((id, turn));
        true
    }

    /// Ends the input of the chat's process, which finishes its work and
    /// exits; its handle is kept so quitting can still end it.
    fn close_claude(&mut self) {
        if let Some(mut chat) = self.claude.take() {
            if !chat.exited() {
                chat.close();
                self.claude_closing.push(chat);
            }
        }
        self.claude_turn = None;
        self.claude_stopped = 0;
        self.claude_ended = 0;
    }

    /// What the claude process reported, onto the chat: the turn on screen
    /// takes its events; a turn the process starts on its own (a background
    /// job's follow-up) opens one; a stopped turn's tail is dropped.
    fn poll_claude(&mut self, app: &mut crate::App) {
        for chat in &mut self.claude_closing {
            chat.poll();
        }
        self.claude_closing.retain(|chat| !chat.exited());
        let Some(chat) = self.claude.as_mut() else {
            return;
        };
        for event in chat.poll() {
            self.claude_event(app, event, false);
        }
        if self
            .claude
            .as_ref()
            .is_some_and(crate::claude_chat::Chat::exited)
        {
            self.claude = None;
            self.claude_stopped = 0;
            self.claude_ended = 0;
        }
    }

    /// One event of the claude process onto the chat.
    fn claude_event(
        &mut self,
        app: &mut crate::App,
        event: crate::claude_chat::Event,
        codex: bool,
    ) {
        match event {
            crate::claude_chat::Event::Session(session) => {
                let session = Some(ClaudeSession {
                    id: session,
                    users: app.live.user_count(),
                });
                if codex {
                    app.live.codex_session = session;
                } else {
                    app.live.claude_session = session;
                }
                app.history.dirty = true;
            }
            crate::claude_chat::Event::Runtime { turn, event } => {
                if turn <= self.claude_stopped {
                    return;
                }
                // A background subagent's progress or end, between turns
                // or during another, goes to the agent it belongs to and
                // opens no turn (#11482).
                let event = match event {
                    RuntimeEvent::Delegation {
                        id: delegation,
                        name,
                        task,
                        event,
                    } => match app.apply_known_delegation(
                        &delegation,
                        name.clone(),
                        task.clone(),
                        *event,
                    ) {
                        Ok(()) => return,
                        Err(event) => RuntimeEvent::Delegation {
                            id: delegation,
                            name,
                            task,
                            event: Box::new(event),
                        },
                    },
                    event => event,
                };
                let id = match &mut self.claude_turn {
                    Some((id, on_screen)) => {
                        *on_screen = (*on_screen).max(turn);
                        *id
                    }
                    // What a turn that already ended still reports (the
                    // plan usage read after it) opens no new turn: no
                    // end would follow it and the chat would stay
                    // working (#11462).
                    None if turn <= self.claude_ended || matches!(event, RuntimeEvent::Plan(_)) => {
                        return;
                    }
                    None if app.begin_claude_turn() => {
                        self.claude_turn = Some((app.request_id, turn));
                        app.request_id
                    }
                    None => return,
                };
                let update = match event {
                    RuntimeEvent::Text(text) => Update::Delta { id, text },
                    RuntimeEvent::Model(model) => Update::Model { id, model },
                    RuntimeEvent::Tool {
                        name,
                        input,
                        output,
                        running,
                    } => Update::Tool {
                        id,
                        name,
                        input,
                        output,
                        running,
                    },
                    RuntimeEvent::Delegation {
                        id: delegation,
                        name,
                        task,
                        event,
                    } => Update::Delegation {
                        id,
                        delegation,
                        name,
                        task,
                        event: *event,
                    },
                    RuntimeEvent::Usage(usage) => Update::Usage { id, usage: *usage },
                    RuntimeEvent::Harness(run) => Update::Harness { id, run: *run },
                    RuntimeEvent::Thinking(on) => Update::Thinking { id, on },
                    RuntimeEvent::ThinkingText(text) => Update::ThinkingText { id, text },
                    _ => return,
                };
                app.apply_update(update);
            }
            crate::claude_chat::Event::TurnEnd { turn, result } => {
                self.claude_ended = self.claude_ended.max(turn);
                if turn <= self.claude_stopped {
                    return;
                }
                if let Some((id, on_screen)) = self.claude_turn
                    && turn >= on_screen
                {
                    // The message-route hook (#11491) blocked this message:
                    // Coder answers it, or starts it in a fresh session.
                    let routed = self
                        .claude
                        .as_ref()
                        .filter(|_| !codex)
                        .and_then(|chat| crate::message_route::take_routed(&chat.cwd));
                    match routed {
                        Some(crate::message_route::Routed::Host(text)) => {
                            self.claude_turn = None;
                            app.apply_update(Update::Finished {
                                id,
                                result: Ok(Streamed {
                                    text,
                                    model: "coder".into(),
                                    finish_reason: Some("stop".into()),
                                    ..Streamed::default()
                                }),
                            });
                        }
                        Some(crate::message_route::Routed::Rebrief) if self.rebrief(app, id) => {}
                        _ => {
                            self.claude_turn = None;
                            app.apply_update(Update::Finished { id, result });
                        }
                    }
                }
            }
            crate::claude_chat::Event::Exited => {
                if let Some((id, _)) = self.claude_turn.take() {
                    app.apply_update(Update::Finished {
                        id,
                        result: Err(if codex {
                            "Codex stopped before the turn ended.".into()
                        } else {
                            "Claude Code stopped before the turn ended.".into()
                        }),
                    });
                }
            }
        }
    }

    /// Hands a Codex message to the chat's process: steered into the turn
    /// running now, sent to the one already running this chat, or to a new
    /// one started on the chat's thread (#11510).
    fn codex_request(&mut self, request: Request, app: &mut crate::App) {
        let Work::Codex {
            messages,
            cwd,
            tools,
            session,
            briefing,
            model,
        } = request.kind
        else {
            return;
        };
        let newest = messages
            .last()
            .map(|message| message.content.clone())
            .unwrap_or_default();
        let model = crate::codex::turn_model(model);
        let running = self.codex.as_ref().is_some_and(|chat| {
            !chat.exited() && chat.cwd == cwd && chat.tools == *tools && chat.session == session
        });
        if !running {
            self.close_codex();
            let seed = crate::codex::seed(&messages);
            self.codex = Some(crate::codex_chat::Chat::start(
                cwd,
                *tools,
                session,
                newest,
                seed,
                briefing.map(|briefing| *briefing),
                model,
            ));
        } else if let Some(chat) = &mut self.codex {
            // A new piece of work: its briefing goes with the message, so
            // the thread and its cache stay (#11473).
            let key = briefing.as_ref().map(|briefing| briefing.key());
            let newest = match briefing.filter(|_| key != chat.briefing) {
                Some(briefing) => {
                    chat.briefing = key;
                    crate::context_hook::note_message_briefing(&cwd, Some(briefing.render(false)));
                    crate::claude::with_briefing(&briefing, &newest)
                }
                None => newest,
            };
            let newest = if chat.model == model {
                newest
            } else {
                chat.set_model(&model);
                crate::codex::with_switch(&model, &newest)
            };
            chat.say(&newest, self.claude_turn.is_some());
            let users = app.live.user_count();
            if let Some(session) = &mut app.live.codex_session {
                session.users = users;
            }
        }
        let turn = self.codex.as_ref().map_or(1, crate::codex_chat::Chat::turn);
        self.claude_turn = Some((request.id, turn));
    }

    /// Ends the chat's Codex process once its turn is done.
    fn close_codex(&mut self) {
        if let Some(mut chat) = self.codex.take() {
            if !chat.exited() {
                chat.close();
                self.codex_closing.push(chat);
            }
            self.claude_turn = None;
            self.claude_stopped = 0;
            self.claude_ended = 0;
        }
    }

    /// What the Codex process reported, onto the chat.
    fn poll_codex(&mut self, app: &mut crate::App) {
        for chat in &mut self.codex_closing {
            chat.poll();
        }
        self.codex_closing.retain(|chat| !chat.exited());
        let Some(chat) = self.codex.as_mut() else {
            return;
        };
        for event in chat.poll() {
            self.claude_event(app, event, true);
        }
        if self
            .codex
            .as_ref()
            .is_some_and(crate::codex_chat::Chat::exited)
        {
            self.codex = None;
            self.claude_stopped = 0;
            self.claude_ended = 0;
        }
    }
}

impl Drop for Background {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn run(request: Request, sender: mpsc::Sender<Update>, canceled: oneshot::Receiver<()>) {
    run_with_provider(request, sender, canceled, Provider::new);
}

fn run_with_provider(
    request: Request,
    sender: mpsc::Sender<Update>,
    canceled: oneshot::Receiver<()>,
    create: impl Fn(openrouter::ApiKey) -> Result<Provider, String>,
) {
    let request = match request {
        Request {
            id,
            kind: Work::Brainstorm { job },
            ..
        } => {
            run_brainstorm(id, job, sender, canceled);
            return;
        }
        request => request,
    };
    let id = request.id;
    let checking = match request.kind {
        Work::Check => 1,
        Work::CheckJev { .. } => 2,
        _ => 0,
    };
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(runtime) = result else {
        failure(
            id,
            checking,
            &sender,
            "The chat worker could not start.".into(),
        );
        return;
    };
    runtime.block_on(async move {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut text_callback = |text: &str| {
            let _ = sender.send(Update::Delta { id, text: text.into() });
        };
        let mut model_callback = |model: &str| {
            let _ = sender.send(Update::Model { id, model: model.into() });
        };
        let mut event_callback = |event| match event {
            RuntimeEvent::Tool { name, input, output, running } => {
                let _ = sender.send(Update::Tool { id, name, input, output, running });
            },
            RuntimeEvent::Delegation { id: delegation, name, task, event } => {
                let _ = sender.send(Update::Delegation { id, delegation, name, task, event: *event });
            },
            RuntimeEvent::Usage(usage) => {
                let _ = sender.send(Update::Usage { id, usage: *usage });
            },
            RuntimeEvent::Thinking(on) => {
                let _ = sender.send(Update::Thinking { id, on });
            },
            RuntimeEvent::ThinkingText(text) => {
                let _ = sender.send(Update::ThinkingText { id, text });
            },
            RuntimeEvent::Harness(run) => {
                let _ = sender.send(Update::Harness { id, run: *run });
            },
            _ => {},
        };
        let work = async {
            let update = match request.kind {
                Work::Bash { command, cwd } => {
                    event_callback(RuntimeEvent::Tool { name: "Run".into(), input: serde_json::json!({"command":command}), output: serde_json::Value::Null, running: true });
                    let result = crate::bundled_runtime::run_command(&command, &cwd, &[], &cancel, &mut event_callback).await;
                    let output = result.as_ref().cloned().unwrap_or_else(|error| serde_json::json!({"error":error}));
                    event_callback(RuntimeEvent::Tool { name: "Run".into(), input: serde_json::json!({"command":command}), output, running: false });
                    Update::Finished { id, result: result.map(|_| Streamed::default()) }
                },
                Work::Delegate { delegation, name, tool, arguments, mut execution, model, options } => {
                    if !request.key.expose().is_empty() {
                        execution.redaction_keys.push(model_access::ApiKey::new(request.key.expose()));
                    }
                    let task = arguments["task"].as_str().or(arguments["message"].as_str()).unwrap_or_default().to_owned();
                    let cloud_job=if matches!(tool.as_str(),"boat_job"|"gce_job"){arguments["job"].as_str().map(str::to_owned)}else{None};
                    let mut events = |event| {
                        let event=match event{RuntimeEvent::Delegation{id,event,..} if cloud_job.as_deref()==Some(id.as_str())=>*event,event=>event};
                        event_callback(RuntimeEvent::Delegation {id:delegation.clone(),name:name.clone(),task:task.clone(),event:Box::new(event)});
                    };
                    events(RuntimeEvent::Tool { name: tool.clone(), input: arguments.clone(), output: serde_json::Value::Null, running: true });
                    let provider = if request.key.expose().is_empty() { None } else {
                        openrouter::Client::new(openrouter::Config::new(openrouter::ApiKey::new(request.key.expose()))).ok().map(|client| crate::plugin_tools::GenerationProvider { client, model, effort: options.reasoning, search_key: Some((openrouter::BASE_URL.to_owned(), request.key.expose().to_owned())) })
                    };
                    let result = execution.execute(&tool, arguments, provider, &cancel, &mut events).await;
                    let output = match &result { Ok(value) => value.clone(), Err(error) => serde_json::json!({"error":error}) };
                    events(RuntimeEvent::Tool { name: tool, input: serde_json::Value::Null, output, running: false });
                    Update::Finished { id, result: result.map(|value| Streamed { text: value["reply"].as_str().unwrap_or_default().into(), model: value["model"].as_str().unwrap_or_default().into(), ..Streamed::default() }) }
                },
                Work::Fanout { run } => {
                    let mut events = |event: RuntimeEvent| event_callback(event);
                    let result = crate::fanout::work(*run, &cancel, &mut events).await;
                    Update::Finished { id, result }
                }
                Work::CheckJev { endpoint, model } => Update::CheckedJev {
                    id,
                    result: crate::jev_plugin::test_key_for_model(request.key.expose(), &endpoint, &model).await,
                },
                Work::SummarizeNoHarness { messages, no_harness, account } => {
                    let mut reasons = Vec::new();
                    let mut result = Err(String::new());
                    let doors = no_harness.policy.doors(
                        account.as_ref().is_some_and(crate::provider::gateway_open),
                        no_harness.gemini_key.is_some(),
                    );
                    for door in doors {
                        let (provider, model) = match door {
                            coder_sdk::auto_policy::Door::Gateway => (
                                account.as_ref().map_or_else(|| Err(String::new()), Provider::gateway),
                                no_harness.model().to_owned(),
                            ),
                            coder_sdk::auto_policy::Door::GeminiKey => (
                                no_harness.gemini_key.clone().map_or_else(
                                    || Err(String::new()),
                                    |key| gemini_provider(key, &no_harness.gemini_base),
                                ),
                                no_harness.policy.no_harness.provider_id().to_owned(),
                            ),
                        };
                        result = match provider {
                            Ok(provider) => provider.stream_with_options_and_model(
                                &model, &crate::models::GenerationOptions::default(), messages.clone(),
                                &mut |_: &str| {}, &mut |_: &str| {},
                            ).await,
                            Err(why) => Err(why),
                        };
                        match &result {
                            Ok(_) => break,
                            Err(why) => reasons.push(why.clone()),
                        }
                    }
                    Update::Finished { id, result: result.map_err(|_| reasons.join("; ")) }
                }
                Work::Microcoder { messages, execution, options } => {
                    auto_route(id, messages, execution, &request.key, options, &create, &cancel, &mut text_callback, &mut model_callback, &mut event_callback).await
                }
                // A claude turn runs in its chat's own process
                // ([`Background::claude_request`]), never on a worker.
                Work::Claude { .. } => Update::Finished { id, result: Err("The claude chat has no process to run in.".into()) },
                Work::Codex { .. } => Update::Finished { id, result: Err("The Codex chat has no process to run in.".into()) },
                Work::OpenCode { messages, cwd, session, briefing, model, tools } => {
                    let (session, last_model) = session.map_or((None, None), |(id, model)| (Some(id), Some(model)));
                    let turn = crate::opencode_chat::Turn {
                        messages, cwd, session, last_model, briefing: briefing.map(|briefing| *briefing), model: model.clone(), tools,
                    };
                    let mut events = |event: RuntimeEvent| match event {
                        RuntimeEvent::Text(text) => text_callback(&text),
                        RuntimeEvent::Model(model) => model_callback(&model),
                        event => event_callback(event),
                    };
                    let mut on_session = |session: &str| {
                        let _ = sender.send(Update::OpenCodeSession { id, session: session.to_owned(), model: model.clone() });
                    };
                    let result = crate::opencode_chat::run(turn, &cancel, &mut events, &mut on_session).await;
                    Update::Finished { id, result }
                }
                Work::Acp { agent, messages, cwd, briefing } => {
                    let task = crate::codex::seed(&messages);
                    let mut events = |event: RuntimeEvent| match event {
                        RuntimeEvent::Text(text) => text_callback(&text),
                        RuntimeEvent::Model(model) => model_callback(&model),
                        event => event_callback(event),
                    };
                    // The model picked in /models for this agent, else its
                    // Coder default (#11556).
                    let model = crate::agent_models::choice(&agent.id);
                    let result = crate::bundled_runtime::acp_briefed(&agent, &task, &cwd, model.as_deref(), false, briefing.as_deref(), &cancel, &mut events).await;
                    // The model the agent named, for its pick on the rail next time.
                    if let Some(model) = result.as_ref().ok().and_then(|value| value["model"].as_str()) {
                        let now = atif::now_ms() / 1000;
                        crate::harness_pick::remember(&format!("agent:{}", agent.id), |entry| {
                            entry.model = Some(model.to_owned());
                            entry.model_at = now;
                        });
                    }
                    Update::Finished { id, result: result.map(|value| Streamed {
                        text: value["reply"].as_str().unwrap_or_default().into(),
                        model: value["model"].as_str().map_or_else(|| agent.name.clone(), str::to_owned),
                        ..Streamed::default()
                    }) }
                }
                Work::SolverIntake { messages, cwd, github, claude: false, account } => {
                    let result = match account.as_ref().map(Provider::gateway) {
                        None => Err("Solver's Intaker needs Claude Code or your OpenAgents sign-in: run coder login, or turn Claude Code on in /plugins.".to_owned()),
                        Some(Err(why)) => Err(why),
                        Some(Ok(provider)) => {
                            let mut execution = crate::issue_run::agent::coder_settings(&cwd, false);
                            if let Some(account) = &account {
                                execution.redaction_keys.push(model_access::ApiKey::new(account.token()));
                            }
                            let prompt = crate::solver::intake_task_coder(&messages, &github);
                            provider.chat_with_plugins(
                                crate::models::GATEWAY_AUTO,
                                &crate::models::GenerationOptions::default(),
                                vec![Message { role: "user".into(), content: prompt }],
                                &execution,
                                &mut text_callback, &mut model_callback,
                                &mut event_callback, &cancel,
                            ).await
                        }
                    };
                    Update::Finished { id, result }
                }
                Work::SolverIntake { messages, cwd, github, .. } => {
                    let prompt = crate::solver::intake_task(&messages, &github);
                    let mut events = |event: RuntimeEvent| match event {
                        RuntimeEvent::Text(text) => text_callback(&text),
                        RuntimeEvent::Model(model) => model_callback(&model),
                        event => event_callback(event),
                    };
                    let result = crate::claude::run_with(
                        &prompt, &cwd, &["Read", "Grep", "Glob"],
                        Some(crate::solver::intake_chat_server(&github)),
                        &cancel, &mut events,
                    ).await;
                    Update::Finished { id, result }
                }
                Work::Chat { model, options, messages, execution } if !crate::models::pinned(&model) => {
                    auto_route(id, messages, execution, &request.key, options, &create, &cancel, &mut text_callback, &mut model_callback, &mut event_callback).await
                }
                // A picked Claude or Codex model runs on this computer's plan
                // first, then the person's own key (#11550).
                Work::Chat { model, options, messages, execution } if crate::route::chat_plan_pick(&model, &execution).is_some() => {
                    let mut plan = crate::route::chat_plan_pick(&model, &execution);
                    if let Some(plan) = plan.as_mut().filter(|plan| plan.effort.is_none()) {
                        plan.effort.clone_from(&options.reasoning);
                    }
                    let started = AtomicBool::new(false);
                    let planned = {
                        let mut text = |delta: &str| {
                            if !delta.is_empty() {
                                started.store(true, Ordering::Relaxed);
                            }
                            text_callback(delta);
                        };
                        let mut events = |event: RuntimeEvent| {
                            if matches!(event, RuntimeEvent::Tool { .. } | RuntimeEvent::Delegation { .. }) {
                                started.store(true, Ordering::Relaxed);
                            }
                            event_callback(event);
                        };
                        match local_turn(id, messages.clone(), execution.clone(), plan, &cancel, &mut text, &mut model_callback, &mut events).await {
                            Update::Finished { result, .. } => result,
                            _ => unreachable!(),
                        }
                    };
                    match planned {
                        Ok(reply) => Update::Finished { id, result: Ok(reply) },
                        Err(error) if started.load(Ordering::Relaxed) || cancel.load(Ordering::Relaxed) => Update::Finished { id, result: Err(error) },
                        // The plan couldn't take it before anything ran: the
                        // person's own key does, on the same model.
                        Err(_) => match create(openrouter::ApiKey::new(request.key.expose())) {
                            Ok(provider) => Update::Finished {
                                id,
                                result: provider.chat_with_plugins(
                                    &model, &options, messages, &execution,
                                    &mut text_callback, &mut model_callback,
                                    &mut event_callback, &cancel,
                                ).await,
                            },
                            Err(error) => Update::Finished { id, result: Err(error) },
                        },
                    }
                }
                kind => {
                    let provider = match create(openrouter::ApiKey::new(request.key.expose())) {
                        Ok(provider) => provider,
                        Err(error) => {
                            failure(id, checking, &sender, error);
                            return;
                        }
                    };
                    match kind {
                        Work::Check => Update::Checked { id, result: provider.check().await },
                        Work::Summarize { model, messages } => Update::Finished {
                            id,
                            result: provider.stream_with_options_and_model(
                                &model, &crate::models::GenerationOptions::default(), messages,
                                &mut |_: &str| {}, &mut |_: &str| {},
                            ).await,
                        },
                        Work::Chat { model, options, messages, execution } => Update::Finished {
                            id,
                            result: provider.chat_with_plugins(
                                &model, &options, messages, &execution,
                                &mut text_callback, &mut model_callback,
                                &mut event_callback, &cancel,
                            ).await,
                        },
                        _ => unreachable!(),
                    }
                }
            };
            let _ = sender.send(update);
        };
        tokio::pin!(work);
        tokio::select! {
            _ = canceled => {
                cancel.store(true, Ordering::Relaxed);
                // Process-backed tools need time to cancel and reap their group.
                if checking == 0 {
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), &mut work).await;
                }
            },
            _ = &mut work => {}
        }
    });
}

/// One `auto` turn over the route's doors in `favor` order (#11374): a
/// door that produced nothing — no streamed text, no tool call — yields to
/// the next; a door that answered, or had already streamed or run a tool
/// when it failed, ends the turn so no effect is repeated. When every
/// door is out, the turn ends with what each said, in plain words.
async fn auto_route(
    id: u64,
    messages: Vec<Message>,
    execution: ExecutionSettings,
    key: &model_access::ApiKey,
    options: crate::models::GenerationOptions,
    create: &dyn Fn(openrouter::ApiKey) -> Result<Provider, String>,
    cancel: &Arc<AtomicBool>,
    text_callback: &mut (dyn FnMut(&str) + Send),
    model_callback: &mut (dyn FnMut(&str) + Send),
    event_callback: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Update {
    let favor = execution.favor;
    let plan = crate::route::plan(&execution, !key.expose().is_empty(), favor);
    let mut reasons = plan.notes.clone();
    for lane in plan.lanes.clone() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let started = AtomicBool::new(false);
        let mut text = |delta: &str| {
            if !delta.is_empty() {
                started.store(true, Ordering::Relaxed);
            }
            text_callback(delta);
        };
        let mut events = |event: RuntimeEvent| {
            if matches!(
                event,
                RuntimeEvent::Tool { .. } | RuntimeEvent::Delegation { .. }
            ) {
                started.store(true, Ordering::Relaxed);
            }
            event_callback(event);
        };
        let result: Result<Streamed, String> = match lane {
            crate::route::Lane::Gateway => {
                match gateway_turn(
                    &messages,
                    &execution,
                    cancel,
                    &mut text,
                    model_callback,
                    &mut events,
                )
                .await
                {
                    Ok(result) => result,
                    // A picked model's failure is its own (#11526): no
                    // other door takes its turn.
                    Err(why) if plan.lanes == [crate::route::Lane::Gateway] => {
                        if let Some(model) = &execution.gateway_model {
                            let name = crate::ui::rail_model_name(model);
                            let why = if why.contains("(HTTP 402)") {
                                format!(
                                    "{name} needs credit on your OpenAgents account (HTTP 402). Add credit at openagents.com, or pick another model in /models."
                                )
                            } else {
                                format!(
                                    "{name} didn't answer: {}. Pick another model in /models.",
                                    why.trim().trim_end_matches('.')
                                )
                            };
                            return Update::Finished {
                                id,
                                result: Err(why),
                            };
                        }
                        reasons.push(why);
                        continue;
                    }
                    Err(why) => {
                        reasons.push(why);
                        continue;
                    }
                }
            }
            crate::route::Lane::Local => {
                match local_turn(
                    id,
                    messages.clone(),
                    execution.clone(),
                    None,
                    cancel,
                    &mut text,
                    model_callback,
                    &mut events,
                )
                .await
                {
                    Update::Finished { result, .. } => result,
                    _ => unreachable!(),
                }
            }
            crate::route::Lane::OpenCode => {
                opencode_turn(
                    &messages,
                    &execution,
                    cancel,
                    &mut text,
                    model_callback,
                    &mut events,
                )
                .await
            }
            crate::route::Lane::GeminiKey => {
                gemini_turn(
                    &messages,
                    &execution,
                    cancel,
                    &mut text,
                    model_callback,
                    &mut events,
                )
                .await
            }
            crate::route::Lane::OpenRouter => match create(openrouter::ApiKey::new(key.expose())) {
                Ok(provider) => {
                    provider
                        .chat_with_plugins(
                            crate::route::openrouter_model(favor, execution.cheap.is_some()),
                            &options,
                            messages.clone(),
                            &execution,
                            &mut text,
                            model_callback,
                            &mut events,
                            cancel,
                        )
                        .await
                }
                Err(error) => Err(error),
            },
        };
        match result {
            Ok(reply) => {
                return Update::Finished {
                    id,
                    result: Ok(reply),
                };
            }
            Err(error) if started.load(Ordering::Relaxed) => {
                return Update::Finished {
                    id,
                    result: Err(error),
                };
            }
            Err(error) => reasons.push(error),
        }
    }
    Update::Finished {
        id,
        result: Err(plan.explain(&reasons)),
    }
}

/// `auto`'s gateway door when signed in: the OpenAgents inference
/// gateway's own router (`openagents/auto`, Vertex first). `Err` is a
/// failure before anything streamed or ran, so the route's next door takes
/// the turn without repeating an effect.
async fn gateway_turn(
    messages: &[Message],
    execution: &ExecutionSettings,
    cancel: &Arc<AtomicBool>,
    text_callback: &mut (dyn FnMut(&str) + Send),
    model_callback: &mut (dyn FnMut(&str) + Send),
    event_callback: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<Result<Streamed, String>, String> {
    let Some(account) = execution.connections.as_ref() else {
        return Err("the OpenAgents account is not signed in".into());
    };
    let provider = Provider::gateway(account)?;
    let mut execution = execution.clone();
    execution
        .redaction_keys
        .push(model_access::ApiKey::new(account.token()));
    let started = AtomicBool::new(false);
    let result = {
        let mut text = |delta: &str| {
            if !delta.is_empty() {
                started.store(true, Ordering::Relaxed);
            }
            text_callback(delta);
        };
        let mut events = |event: RuntimeEvent| {
            if matches!(
                event,
                RuntimeEvent::Tool { .. } | RuntimeEvent::Delegation { .. }
            ) {
                started.store(true, Ordering::Relaxed);
            }
            event_callback(event);
        };
        // A no-harness turn's one model (#11548), else the model the
        // person picked in /models (#11526), else the gateway's own router.
        provider
            .chat_with_plugins(
                execution
                    .no_harness
                    .as_ref()
                    .map(crate::route::NoHarness::model)
                    .or(execution.gateway_model.as_deref())
                    .or_else(|| {
                        execution
                            .cheap
                            .as_ref()
                            .and_then(|cheap| cheap.gateway_model.as_deref())
                    })
                    .unwrap_or(crate::models::GATEWAY_AUTO),
                &crate::models::GenerationOptions::default(),
                messages.to_vec(),
                &execution,
                &mut text,
                model_callback,
                &mut events,
                cancel,
            )
            .await
    };
    match result {
        Err(error) if !started.load(Ordering::Relaxed) && !cancel.load(Ordering::Relaxed) => {
            Err(error)
        }
        result => Ok(result),
    }
}

/// A no-harness turn's own-key door (#11548): the no-harness model on the
/// person's Gemini API key, through Gemini's OpenAI-compatible endpoint,
/// with the same loop and tools as the gateway's.
async fn gemini_turn(
    messages: &[Message],
    execution: &ExecutionSettings,
    cancel: &Arc<AtomicBool>,
    text_callback: &mut (dyn FnMut(&str) + Send),
    model_callback: &mut (dyn FnMut(&str) + Send),
    event_callback: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<Streamed, String> {
    let Some(no_harness) = execution.no_harness.as_ref() else {
        return Err("No Gemini API key is set.".into());
    };
    let Some(key) = no_harness.gemini_key.clone() else {
        return Err("No Gemini API key is set.".into());
    };
    let provider = gemini_provider(key.clone(), &no_harness.gemini_base)?;
    let mut execution = execution.clone();
    execution.redaction_keys.push(key);
    provider
        .chat_with_plugins(
            no_harness.policy.no_harness.provider_id(),
            &crate::models::GenerationOptions::default(),
            messages.to_vec(),
            &execution,
            text_callback,
            model_callback,
            event_callback,
            cancel,
        )
        .await
}

/// The Gemini API at `base` on `key`.
fn gemini_provider(key: model_access::ApiKey, base: &str) -> Result<Provider, String> {
    Provider::gemini_at(openrouter::ApiKey::new(key.expose()), base)
}

/// `auto`'s OpenCode door: one `opencode acp` turn on the free
/// `opencode/cat-jam-free` model (Big Pickle when OpenCode no longer offers
/// it, or the turn says it is gone), or the `provider/model` that
/// `CODER_OPENCODE_MODEL` names, in the engine's own database and
/// permissions, as a delegated OpenCode turn runs (#11359). `model` is not
/// named to `acp` — OpenCode takes it from the inline configuration the
/// turn's environment carries.
async fn opencode_turn(
    messages: &[Message],
    execution: &ExecutionSettings,
    cancel: &Arc<AtomicBool>,
    text_callback: &mut (dyn FnMut(&str) + Send),
    model_callback: &mut (dyn FnMut(&str) + Send),
    event_callback: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Result<Streamed, String> {
    let Some(agent) = execution
        .agents
        .iter()
        .find(|agent| agent.id == "opencode" && agent.enabled)
        .cloned()
    else {
        return Err("OpenCode is not installed or is turned off.".into());
    };
    let task = local_task(messages, execution)?;
    let named = std::env::var(coder_delegate::delegate::OPENCODE_MODEL_VAR)
        .ok()
        .filter(|model| !model.trim().is_empty());
    // A named model runs alone; the automatic one falls back to Big Pickle
    // when OpenCode says Cat Jam is gone (#11547).
    let candidates: Vec<&str> = match named.as_deref() {
        Some(named) => vec![named.trim()],
        None => {
            let auto = coder::delegate_door::opencode_auto_model();
            if auto == coder::delegate_door::OPENCODE_AUTO_FALLBACK {
                vec![auto]
            } else {
                vec![auto, coder::delegate_door::OPENCODE_AUTO_FALLBACK]
            }
        }
    };
    let permission = if crate::approval::gated() {
        acp_client::opencode::Permission::Workspace
    } else {
        acp_client::opencode::Permission::Full
    };
    let answered = AtomicBool::new(false);
    let mut events = |event: RuntimeEvent| match event {
        RuntimeEvent::Text(text) => {
            answered.store(true, Ordering::Relaxed);
            text_callback(&execution.redact_text(&text));
        }
        RuntimeEvent::Model(model) => model_callback(&model),
        event => event_callback(event),
    };
    let mut result = Err("OpenCode has no model to run.".to_owned());
    for (index, candidate) in candidates.iter().enumerate() {
        let model = acp_client::opencode::Model::parse(candidate)
            .map_err(|why| format!("Coder's OpenCode model is invalid: {why}"))?;
        let prior = crate::bundled_runtime::child_env();
        let mut vars = prior.clone();
        vars.extend(
            acp_client::opencode::environment(
                &model,
                permission,
                std::path::Path::new("openagents-coder-engine.db"),
            )
            .into_iter()
            .map(|(name, value)| (name.into(), value.into())),
        );
        crate::bundled_runtime::set_child_env(vars);
        result =
            crate::bundled_runtime::acp(&agent, &task, &execution.cwd, None, cancel, &mut events)
                .await;
        crate::bundled_runtime::set_child_env(prior);
        let retry = index + 1 < candidates.len()
            && !answered.load(Ordering::Relaxed)
            && result
                .as_ref()
                .is_err_and(|error| coder_delegate::delegate::opencode_model_gone(error));
        if !retry {
            break;
        }
    }
    let result = result?;
    let mut reply = Streamed {
        text: result["reply"].as_str().unwrap_or_default().into(),
        model: result["model"].as_str().unwrap_or_default().into(),
        ..Streamed::default()
    };
    // An engine login's usage is covered by its own plan or free model.
    reply.usage.cost = None;
    if reply.text.trim().is_empty() {
        return Err("OpenCode returned no answer.".into());
    }
    Ok(reply)
}

/// The local loop: the person's own model logins, then the OpenAgents cloud.
async fn local_turn(
    id: u64,
    messages: Vec<Message>,
    execution: ExecutionSettings,
    plan: Option<crate::route::PlanPick>,
    cancel: &Arc<AtomicBool>,
    text_callback: &mut (dyn FnMut(&str) + Send),
    model_callback: &mut (dyn FnMut(&str) + Send),
    event_callback: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Update {
    let cancel = cancel.clone();
    let mut events = |event| match event {
        RuntimeEvent::Text(text) => text_callback(&execution.redact_text(&text)),
        RuntimeEvent::ThinkingText(text) => {
            event_callback(RuntimeEvent::ThinkingText(execution.redact_text(&text)));
        }
        RuntimeEvent::Model(model) => model_callback(&model),
        RuntimeEvent::Tool {
            name,
            mut input,
            mut output,
            running,
        } => {
            execution.redact(&mut input);
            execution.redact(&mut output);
            event_callback(RuntimeEvent::Tool {
                name,
                input,
                output,
                running,
            });
        }
        RuntimeEvent::Delegation { .. }
        | RuntimeEvent::Tokens(_)
        | RuntimeEvent::Usage(_)
        | RuntimeEvent::Harness(_)
        | RuntimeEvent::Progress { .. }
        | RuntimeEvent::Plan(_)
        | RuntimeEvent::Thinking(_) => event_callback(event),
    };
    let result = async {
        let task = local_task(&messages, &execution)?;
        let client = execution.jev_client()?;
        // A picked Claude or Codex model on this computer's plan (#11550).
        let result = match plan {
            Some(plan) => crate::bundled_runtime::microcoder_plan(
                &task, &execution.cwd, plan.login, &plan.model, plan.effort, client,
                &execution.redaction_keys, &cancel, &mut events,
            ).await?,
            None => crate::bundled_runtime::microcoder_local(
                &task, &execution.cwd, false, execution.claude, execution.cheap.is_some(), client, &execution.redaction_keys,
                &cancel, &mut events,
            ).await?,
        };
        let mut reply = Streamed {
            text: execution.redact_text(result["reply"].as_str().unwrap_or_default()),
            model: result["model"].as_str().unwrap_or_default().into(),
            ..Streamed::default()
        };
        reply.usage.total_tokens = result["tokens"].as_u64().unwrap_or_default();
        // Local subscriptions and house-key usage are not charges to the user.
        reply.usage.cost = None;
        if result["outcome"]["ending"]["reason"].as_str() == Some("no_capacity") {
            // Every login the loop knew is out; the route's next door takes
            // the turn, and the clause names when each login is back.
            let clauses = crate::route::refusal_clauses(
                result["refusals"].as_array().map(Vec::as_slice).unwrap_or(&[]),
            );
            return Err(if clauses.is_empty() {
                "No model login on this computer can answer right now.".into()
            } else {
                format!("the model logins are out — {}", crate::route::join(&clauses))
            });
        }
        if !matches!(result["outcome"]["ending"]["reason"].as_str(), Some("finished" | "tests_held" | "checks_passed" | "asked")) {
            return Err(format!("The coding loop stopped: {}.", result["outcome"]));
        }
        if reply.text.is_empty() {
            return Err("The coding loop stopped before producing a reply. Inspect the tool results or retry with a smaller task.".into());
        }
        Ok(reply)
    }.await;
    Update::Finished { id, result }
}

fn run_brainstorm(
    id: u64,
    job: crate::brainstorm::Job,
    sender: mpsc::Sender<Update>,
    canceled: oneshot::Receiver<()>,
) {
    let generation = job.generation;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let result = match runtime {
        Err(_) => Err(brainstorm_client::Error::Transport),
        Ok(runtime) => runtime.block_on(async {
            let work = job.run();
            tokio::pin!(work);
            tokio::select! {
                biased;
                _ = canceled => {
                    job.cancellation.cancel();
                    Err(brainstorm_client::Error::Cancelled)
                }
                result = &mut work => result,
            }
        }),
    };
    let _ = sender.send(Update::BrainstormFinished {
        id,
        generation,
        result,
    });
}

/// Project the same live messages, including bounded observations, into the local route.
pub(crate) fn local_conversation(messages: &[Message], limit: usize) -> String {
    let mut context = Vec::new();
    let mut bytes = 0;
    for message in messages.iter().rev() {
        let row = format!("{}: {}", message.role, message.content);
        let separator = usize::from(!context.is_empty()) * 2;
        if bytes + row.len() + separator > limit {
            break;
        }
        bytes += row.len() + separator;
        context.push(row);
    }
    context.reverse();
    context.join("\n\n")
}

pub(crate) fn local_task(
    messages: &[Message],
    execution: &ExecutionSettings,
) -> Result<String, String> {
    const LIMIT: usize = 56 * 1024;
    // The local loop runs on the person's own Codex or Claude Code login,
    // whose own prompt names that product: Coder's identity leads (#11264).
    let mut prefix = format!(
        "Standing instructions (from the host, not the user):\n{}",
        crate::identity::prompt_with(
            crate::identity::Menu::coder_loop(
                execution.openrouter,
                execution
                    .connections
                    .as_ref()
                    .is_some_and(crate::provider::gateway_open),
                execution.claude,
            ),
            crate::models::AUTO
        )
    );
    if let Some(standing) = execution
        .instructions
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        prefix.push_str(standing);
        prefix.push('\n');
    }
    prefix.push('\n');
    // Project instructions and saved memory (#11176), within a share of the
    // local route's allowance; files over it are named, not read.
    if let Some(context) = execution
        .briefing
        .as_ref()
        .map(|briefing| briefing.render(false))
        .or_else(|| {
            execution
                .memory
                .as_ref()
                .map(|memory| memory.context(false, LIMIT / 3))
        })
        .filter(|text| !text.trim().is_empty())
    {
        prefix.push_str(&context);
        prefix.push_str("\n\n");
    }
    prefix.push_str("The conversation:\n");
    let suffix = if execution.cli {
        "\n\nThe bundled OpenAgents CLI is enabled for requested CLI work: openagents --json with an argument array's equivalent syntax. Answer questions directly from what you know and from read-only commands; read a command group's --help only when you need a command you do not know. Follow the user's authorization for effects."
    } else {
        ""
    };
    let allowance = LIMIT
        .checked_sub(prefix.len().saturating_add(suffix.len()))
        .ok_or("The host instructions exceed the local route's context allowance.")?;
    let task = execution.redact_text(&format!(
        "{prefix}{}{suffix}",
        local_conversation(messages, allowance)
    ));
    if task.len() > LIMIT {
        return Err("The redacted context exceeds the local route's allowance.".into());
    }
    Ok(task)
}

fn failure(id: u64, checking: u8, sender: &mpsc::Sender<Update>, error: String) {
    let update = match checking {
        1 => Update::Checked {
            id,
            result: Err(error),
        },
        2 => Update::CheckedJev {
            id,
            result: Err(error),
        },
        _ => Update::Finished {
            id,
            result: Err(error),
        },
    };
    let _ = sender.send(update);
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use serde_json::{Value, json};

    use super::*;
    use crate::{App, Mode, Screen, plugins::Connection};

    #[test]
    fn bash_worker_runs_without_an_inference_provider() {
        let root = tempfile::tempdir().unwrap();
        let (sender, receiver) = mpsc::channel();
        let (_cancel, canceled) = oneshot::channel();
        run_with_provider(
            Request {
                id: 42,
                key: model_access::ApiKey::new(""),
                kind: Work::Bash {
                    command: "printf composer-shell".into(),
                    cwd: root.path().to_owned(),
                },
            },
            sender,
            canceled,
            |_| panic!("Bash must not create an inference provider"),
        );
        let events = receiver.try_iter().collect::<Vec<_>>();
        assert!(events.iter().any(|event| matches!(event, Update::Tool { name, output, running: false, .. } if name == "Run" && output["output"].as_str().is_some_and(|text| text.contains("composer-shell")))));
        assert!(events.iter().any(|event| matches!(
            event,
            Update::Finished {
                id: 42,
                result: Ok(_),
                ..
            }
        )));
    }

    const FIXTURE_TOKEN: &str = "local-worker-fixture-token";
    const TEST_TIMEOUT: Duration = Duration::from_secs(3);

    fn key(app: &mut App, code: KeyCode) {
        assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
    }

    fn configured_app() -> App {
        let mut app = App::default();
        app.messages.push("demo-only-message".into());
        assert!(app.handle(Event::Paste("demo-only-draft".into())));
        app.set_mode(Mode::Live);
        key(&mut app, KeyCode::F(2));
        key(&mut app, KeyCode::Enter);
        assert!(app.handle(Event::Paste(FIXTURE_TOKEN.into())));
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Home);
        for _ in 0..app.plugins.field(false).0.chars().count() {
            key(&mut app, KeyCode::Delete);
        }
        assert!(app.handle(Event::Paste("fixture/model".into())));
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Enter);
        assert!(app.plugins.key_configured);
        assert!(app.checking_key);
        assert!(matches!(
            app.request.as_ref().map(|request| &request.kind),
            Some(Work::Check)
        ));
        app
    }

    fn fixture_listener() -> (TcpListener, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}/api/v1", listener.local_addr().unwrap());
        (listener, base)
    }

    fn accept(listener: &TcpListener) -> TcpStream {
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            match listener.accept() {
                Ok((socket, _)) => {
                    socket.set_nonblocking(false).unwrap();
                    socket.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
                    socket.set_write_timeout(Some(TEST_TIMEOUT)).unwrap();
                    return socket;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "the worker did not contact the fixture"
                    );
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("fixture accept failed: {error}"),
            }
        }
    }

    fn read_request(socket: &mut TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0, "the worker closed before sending its request");
            bytes.extend_from_slice(&buffer[..count]);
            assert!(
                bytes.len() < 64 * 1024,
                "fixture request exceeded its bound"
            );
            if let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..header_end]);
                let length = header
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + length {
                    return String::from_utf8(bytes).unwrap();
                }
            }
        }
    }

    fn authenticated(request: &str) {
        assert!(request.lines().any(|line| {
            line.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("authorization")
                    && value.trim() == format!("Bearer {FIXTURE_TOKEN}")
            })
        }));
    }

    fn headers(socket: &mut TcpStream, content_type: &str, body_length: usize) {
        write!(
            socket,
            "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {body_length}\r\nconnection: close\r\n\r\n"
        )
        .unwrap();
    }

    fn start(app: &mut App, base: &str, background: &mut Background) -> JoinHandle<()> {
        let request = app.request.take().expect("the UI queued work");
        let id = request.id;
        let base = base.to_owned();
        let (sender, receiver) = mpsc::channel();
        let (cancel, canceled) = oneshot::channel();
        let worker = thread::spawn(move || {
            run_with_provider(request, sender, canceled, |key| {
                Provider::with_base(key, &base)
            });
        });
        assert!(background.active.is_none());
        background.active = Some((id, cancel, receiver));
        worker
    }

    fn pump(app: &mut App, background: &mut Background, done: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            background.sync(app);
            if done(app) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the loopback worker did not finish its phase"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn signed_in_at(origin: &str, token: &str) -> openagents_login::Saved {
        serde_json::from_value(json!({
            "origin": origin, "account": "acct_fixture", "label": "Fixture",
            "expires_at": u64::MAX, "token": token,
        }))
        .unwrap()
    }

    fn reply_stream(model: &str, text: &str) -> String {
        format!(
            "data: {{\"model\":\"{model}\",\"choices\":[{{\"delta\":{{\"content\":\"{text}\"}},\"finish_reason\":\"stop\"}}]}}\n\ndata: [DONE]\n\n"
        )
    }

    fn auto_turn(token: &str, origin: &str) -> Request {
        let root = tempfile::tempdir().unwrap();
        let mut execution = crate::plugins::Plugins::default().execution_settings(root.keep());
        execution.connections = Some(signed_in_at(origin, token));
        Request {
            id: 7,
            key: model_access::ApiKey::new(FIXTURE_TOKEN),
            kind: Work::Chat {
                model: crate::models::DEFAULT_MODEL.into(),
                options: crate::models::GenerationOptions::default(),
                messages: vec![Message::user("hello".to_owned())],
                execution,
            },
        }
    }

    /// A signed-in `auto` turn runs on the OpenAgents gateway's own router
    /// with the account's session, and OpenRouter is never asked.
    #[test]
    fn signed_in_auto_runs_on_the_openagents_gateway() {
        let (listener, base) = fixture_listener();
        let origin = base.trim_end_matches("/api/v1").to_owned();
        let server = thread::spawn(move || {
            let mut socket = accept(&listener);
            let request = read_request(&mut socket);
            let body = reply_stream("google/gemini-3.8-flash", "From the gateway");
            headers(&mut socket, "text/event-stream", body.len());
            socket.write_all(body.as_bytes()).unwrap();
            request
        });
        let (sender, receiver) = mpsc::channel();
        let (_cancel, canceled) = oneshot::channel();
        run_with_provider(
            auto_turn("sess_gateway_ok", &origin),
            sender,
            canceled,
            |_| panic!("a signed-in auto turn the gateway answers never reaches OpenRouter"),
        );
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /api/v1/chat/completions HTTP/1.1"));
        assert!(request.contains("Bearer sess_gateway_ok"));
        let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["model"], "openagents/auto");
        assert!(receiver.try_iter().any(|update| matches!(
            update,
            Update::Finished { result: Ok(reply), .. } if reply.text == "From the gateway"
        )));
    }

    /// #11526: a gateway model picked in /models is what the gateway is
    /// asked for, and when it fails no other door takes the turn.
    #[test]
    fn a_picked_gateway_model_runs_on_the_gateway_and_its_failure_is_its_own() {
        let pick = |token: &str, origin: &str| {
            let mut turn = auto_turn(token, origin);
            if let Work::Chat { execution, .. } = &mut turn.kind {
                execution.gateway_model = Some("zai/glm-5.3-flash".into());
            }
            turn
        };
        let (listener, base) = fixture_listener();
        let origin = base.trim_end_matches("/api/v1").to_owned();
        let server = thread::spawn(move || {
            let mut socket = accept(&listener);
            let request = read_request(&mut socket);
            let body = reply_stream("zai/glm-5.3-flash", "From the pick");
            headers(&mut socket, "text/event-stream", body.len());
            socket.write_all(body.as_bytes()).unwrap();
            request
        });
        let (sender, receiver) = mpsc::channel();
        let (_cancel, canceled) = oneshot::channel();
        run_with_provider(pick("sess_gateway_pick", &origin), sender, canceled, |_| {
            panic!("a picked gateway model never reaches OpenRouter")
        });
        let request = server.join().unwrap();
        let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["model"], "zai/glm-5.3-flash");
        assert!(receiver.try_iter().any(|update| matches!(
            update,
            Update::Finished { result: Ok(reply), .. }
                if reply.text == "From the pick" && reply.model == "zai/glm-5.3-flash"
        )));

        let (listener, base) = fixture_listener();
        let origin = base.trim_end_matches("/api/v1").to_owned();
        let failing = thread::spawn(move || {
            let mut socket = accept(&listener);
            read_request(&mut socket);
            let body = r#"{"error":{"type":"bad_request","message":"no"}}"#;
            write!(
                socket,
                "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let (sender, receiver) = mpsc::channel();
        let (_cancel, canceled) = oneshot::channel();
        run_with_provider(
            pick("sess_gateway_pick_fails", &origin),
            sender,
            canceled,
            |_| panic!("a picked gateway model's failure never falls back to OpenRouter"),
        );
        failing.join().unwrap();
        let ended = receiver.try_iter().find_map(|update| match update {
            Update::Finished {
                result: Err(why), ..
            } => Some(why),
            _ => None,
        });
        let why = ended.expect("the turn ends with the pick's failure");
        assert!(why.starts_with("GLM 5.3 Flash didn't answer: "), "{why}");
        assert!(why.ends_with("Pick another model in /models."), "{why}");
    }

    /// When the gateway refuses the sign-in, the same turn goes to the
    /// OpenRouter free router, and later turns skip the gateway.
    #[test]
    fn a_refused_gateway_falls_back_to_the_free_router_and_is_not_asked_again() {
        let (gateway, gateway_base) = fixture_listener();
        let origin = gateway_base.trim_end_matches("/api/v1").to_owned();
        let (openrouter, openrouter_base) = fixture_listener();
        let refusing = thread::spawn(move || {
            let mut socket = accept(&gateway);
            read_request(&mut socket);
            let body =
                r#"{"error":{"type":"unauthorized","message":"Your API key was rejected."}}"#;
            write!(
                socket,
                "HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let answering = thread::spawn(move || {
            let mut socket = accept(&openrouter);
            let request = read_request(&mut socket);
            let body = reply_stream("vendor/free-model", "From the free router");
            headers(&mut socket, "text/event-stream", body.len());
            socket.write_all(body.as_bytes()).unwrap();
            request
        });
        let (sender, receiver) = mpsc::channel();
        let (_cancel, canceled) = oneshot::channel();
        let turn = auto_turn("sess_gateway_refused", &origin);
        run_with_provider(turn, sender, canceled, |key| {
            Provider::with_base(key, &openrouter_base)
        });
        refusing.join().unwrap();
        let request = answering.join().unwrap();
        authenticated(&request);
        let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["model"], crate::models::DEFAULT_MODEL);
        let updates = receiver.try_iter().collect::<Vec<_>>();
        assert!(updates.iter().any(|update| matches!(
            update,
            Update::Finished { result: Ok(reply), .. } if reply.text == "From the free router"
        )));
        assert!(!updates.iter().any(
            |update| matches!(update, Update::Delta { text, .. } if text.contains("rejected"))
        ));
        assert!(!crate::provider::gateway_open(&signed_in_at(
            &origin,
            "sess_gateway_refused"
        )));
        assert!(crate::provider::gateway_open(&signed_in_at(
            &origin,
            "sess_other"
        )));
    }

    /// Every connected door out: the gateway's allowance is used up, the
    /// computer's logins can't answer, and the OpenRouter key has no
    /// balance. The turn ends in one plain sentence naming what each door
    /// said — never a provider's own refusal text (#11374).
    #[test]
    fn an_auto_turn_with_every_door_out_ends_in_plain_words() {
        let (gateway, gateway_base) = fixture_listener();
        let origin = gateway_base.trim_end_matches("/api/v1").to_owned();
        let (openrouter, openrouter_base) = fixture_listener();
        let gateway_server = thread::spawn(move || {
            let mut socket = accept(&gateway);
            read_request(&mut socket);
            let body = r#"{"error":{"message":"insufficient_quota"}}"#;
            write!(
                socket,
                "HTTP/1.1 402 Payment Required\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let openrouter_server = thread::spawn(move || {
            let mut socket = accept(&openrouter);
            read_request(&mut socket);
            let body = r#"{"error":{"message":"Insufficient credits"}}"#;
            write!(
                socket,
                "HTTP/1.1 402 Payment Required\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let (sender, receiver) = mpsc::channel();
        let (_cancel, canceled) = oneshot::channel();
        run_with_provider(
            auto_turn("sess_gateway_out", &origin),
            sender,
            canceled,
            |key| Provider::with_base(key, &openrouter_base),
        );
        gateway_server.join().unwrap();
        openrouter_server.join().unwrap();
        let updates = receiver.try_iter().collect::<Vec<_>>();
        let error = updates
            .iter()
            .find_map(|update| match update {
                Update::Finished {
                    result: Err(error), ..
                } => Some(error.clone()),
                _ => None,
            })
            .expect("the turn ends with an error");
        assert!(
            error.contains("couldn't find a model that can answer"),
            "{error}"
        );
        assert!(error.contains("free allowance"), "{error}");
        assert!(error.contains("OpenRouter"), "{error}");
        // No vendor's own refusal text reaches the person.
        assert!(!error.contains("insufficient_quota"), "{error}");
        assert!(!error.contains("Insufficient credits"), "{error}");
        // The used-up allowance is skipped while it is held, not asked again.
        assert!(!crate::provider::gateway_open(&signed_in_at(
            &origin,
            "sess_gateway_out"
        )));
    }

    /// A gateway failure after the turn already streamed is handed off
    /// through the interrupted-turn handoff — the next door continues the
    /// same task rather than the turn ending on the allowance error — and
    /// the route does not start the task over somewhere else (#11374).
    #[test]
    fn a_gateway_failure_mid_turn_hands_off_instead_of_replaying() {
        let (gateway, gateway_base) = fixture_listener();
        let origin = gateway_base.trim_end_matches("/api/v1").to_owned();
        let gateway_server = thread::spawn(move || {
            let mut socket = accept(&gateway);
            read_request(&mut socket);
            let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Checking.\"}}]}\n\ndata: {\"error\":{\"code\":402,\"message\":\"insufficient_quota\"}}\n\n";
            headers(&mut socket, "text/event-stream", body.len());
            socket.write_all(body.as_bytes()).unwrap();
        });
        let (sender, receiver) = mpsc::channel();
        let (_cancel, canceled) = oneshot::channel();
        run_with_provider(
            auto_turn("sess_gateway_mid_402", &origin),
            sender,
            canceled,
            |_| panic!("a door that already produced is handed off, not started over elsewhere"),
        );
        gateway_server.join().unwrap();
        let updates = receiver.try_iter().collect::<Vec<_>>();
        let error = updates
            .iter()
            .find_map(|update| match update {
                Update::Finished {
                    result: Err(error), ..
                } => Some(error.clone()),
                _ => None,
            })
            .expect("the turn ends with the handoff's outcome");
        // The handoff ran (its providers are offline here): the failure is
        // the continuation's, not a fresh turn on another door.
        assert!(error.contains("could not continue"), "{error}");
        assert!(error.contains("free allowance"), "{error}");
        assert!(!crate::provider::gateway_open(&signed_in_at(
            &origin,
            "sess_gateway_mid_402"
        )));
    }

    #[test]
    fn saved_key_and_live_prompt_reach_the_worker_and_stream_back_to_the_app() {
        let (listener, base) = fixture_listener();
        let (finish, finish_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let mut socket = accept(&listener);
            let check = read_request(&mut socket);
            authenticated(&check);
            assert!(check.starts_with("GET /api/v1/key HTTP/1.1"));
            let key_body = r#"{"data":{"label":"local-worker-fixture-token","limit_remaining":0}}"#;
            headers(&mut socket, "application/json", key_body.len());
            socket.write_all(key_body.as_bytes()).unwrap();
            drop(socket);

            let mut socket = accept(&listener);
            let chat = read_request(&mut socket);
            authenticated(&chat);
            let first = "data: {\"model\":\"fixture/model\",\"choices\":[{\"delta\":{\"content\":\"Loopback \"}}]}\n\n";
            let last = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"reply\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5,\"cost\":0.002}}\n\n",
                "data: [DONE]\n\n"
            );
            headers(&mut socket, "text/event-stream", first.len() + last.len());
            socket.write_all(first.as_bytes()).unwrap();
            socket.flush().unwrap();
            finish_rx.recv_timeout(TEST_TIMEOUT).unwrap();
            socket.write_all(last.as_bytes()).unwrap();
            chat
        });

        let mut app = configured_app();
        let mut background = Background::default();
        let check_worker = start(&mut app, &base, &mut background);
        pump(&mut app, &mut background, |app| {
            matches!(app.plugins.connection, Connection::Verified)
        });
        check_worker.join().unwrap();
        assert!(!app.checking_key);
        assert!(background.active.is_none());
        assert_eq!(
            app.plugins.connection_label(),
            "OpenRouter API key verified"
        );

        if !app.plugins.enabled {
            key(&mut app, KeyCode::Char(' '));
        }
        assert!(app.plugins.enabled);
        key(&mut app, KeyCode::Esc);
        assert!(app.screen == Screen::Conversation);
        assert!(app.handle(Event::Paste("Loopback prompt".into())));
        key(&mut app, KeyCode::Enter);
        let chat_worker = start(&mut app, &base, &mut background);
        pump(&mut app, &mut background, |app| {
            app.live.partial == "Loopback "
        });
        assert!(app.live.busy);
        assert_eq!(app.live.partial_model.as_deref(), Some("fixture/model"));
        assert!(background.active.is_some());
        assert_eq!(app.live.entries.len(), 1);
        assert!(matches!(&app.live.entries[0], Entry::User(text) if text == "Loopback prompt"));
        finish.send(()).unwrap();
        pump(&mut app, &mut background, |app| !app.live.busy);
        chat_worker.join().unwrap();
        assert!(app.live.partial.is_empty());
        assert!(app.live.partial_model.is_none());
        assert!(app.live.notice.is_none());
        assert_eq!(app.live.tokens, 5);
        assert!(matches!(
            &app.live.entries[1],
            Entry::Assistant { text, model, .. }
                if text == "Loopback reply" && model.as_deref() == Some("fixture/model")
        ));

        let request = server.join().unwrap();
        assert!(request.starts_with("POST /api/v1/chat/completions HTTP/1.1"));
        let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["model"], "fixture/model");
        assert_eq!(body["stream"], true);
        assert_eq!(
            body["messages"].as_array().unwrap().last().unwrap(),
            &json!({"role":"user","content":"Loopback prompt"})
        );
        assert!(!request.contains("demo-only"));
        assert!(!crate::snapshot::svg(&mut app, 110, 36).contains(FIXTURE_TOKEN));
    }

    #[test]
    fn canceling_a_key_check_closes_the_worker_and_refuses_late_verification() {
        let (listener, base) = fixture_listener();
        let (arrived, arrival) = mpsc::channel();
        let server = thread::spawn(move || {
            let mut socket = accept(&listener);
            let request = read_request(&mut socket);
            authenticated(&request);
            assert!(request.starts_with("GET /api/v1/key HTTP/1.1"));
            arrived.send(()).unwrap();
            let mut byte = [0_u8; 1];
            match socket.read(&mut byte) {
                Ok(0) => true,
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => true,
                _ => false,
            }
        });
        let mut app = configured_app();
        let id = app.request_id;
        let mut background = Background::default();
        let worker = start(&mut app, &base, &mut background);
        arrival.recv_timeout(TEST_TIMEOUT).unwrap();
        app.cancel_request();
        background.sync(&mut app);
        worker.join().unwrap();
        assert!(
            server.join().unwrap(),
            "the canceled HTTP connection remained open"
        );
        assert!(background.active.is_none());
        assert!(!app.checking_key);
        app.apply_update(Update::Checked {
            id,
            result: Ok(KeyInfo {
                status: "Verified",
                limit_remaining: Some(1.0),
            }),
        });
        assert!(matches!(app.plugins.connection, Connection::Unchecked));
    }

    /// After a claude turn ends, the plan usage the process reads under
    /// that turn's number opens no new turn, so the chat and the program
    /// status go back to idle (#11462). A later turn the process starts on
    /// its own still shows as working.
    #[test]
    fn a_claude_turn_ends_idle_after_its_plan_usage_arrives() {
        use crate::claude_chat::Event as Claude;
        let mut app = App::default();
        app.set_mode(Mode::Live);
        let mut background = Background::default();
        let text = |text: &str| RuntimeEvent::Text(text.into());
        background.claude_event(
            &mut app,
            Claude::Runtime {
                turn: 1,
                event: text("Hi"),
            },
            false,
        );
        assert_eq!(app.program_status().0, "working");
        background.claude_event(
            &mut app,
            Claude::TurnEnd {
                turn: 1,
                result: Ok(Streamed::default()),
            },
            false,
        );
        assert_eq!(app.program_status().0, "idle");
        background.claude_event(
            &mut app,
            Claude::Runtime {
                turn: 1,
                event: RuntimeEvent::Plan(Default::default()),
            },
            false,
        );
        background.claude_event(
            &mut app,
            Claude::Runtime {
                turn: 1,
                event: text("late"),
            },
            false,
        );
        assert_eq!(app.program_status().0, "idle");
        assert!(!app.live.busy);
        background.claude_event(
            &mut app,
            Claude::Runtime {
                turn: 2,
                event: text("Back"),
            },
            false,
        );
        assert_eq!(app.program_status().0, "working");
    }
}
