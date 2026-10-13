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
    /// The harness plugin this chat's turns run on, once its first turn
    /// was routed ([`App::route_turn`](crate::App::route_turn)); follow-ups
    /// stay on it (#11453).
    pub runs_on: Option<crate::harness_plugin::Harness>,
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

impl Chat {
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
        self.rounds.push((self.entries.len(), usage));
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

    pub fn stop_tools(&mut self, reason: &str) {
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
    /// A turn of solver's conversational Intaker: Haiku with a read-only
    /// `gh`, helping the person find or pick the issue to solve.
    SolverIntake {
        messages: Vec<Message>,
        cwd: std::path::PathBuf,
        github: String,
    },
    /// A summary of the older part of the chat (#11179), written by the
    /// chat's model without tools; nothing streams into the transcript.
    Summarize {
        model: String,
        messages: Vec<Message>,
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
    Model {
        id: u64,
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
            | Self::Model { id, .. }
            | Self::Finished { id, .. } => *id,
        }
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
        if let Some(request) = app
            .request
            .take_if(|request| matches!(request.kind, Work::Claude { .. }))
        {
            self.cancel();
            self.claude_request(request, app);
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
            self.claude_event(app, event);
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
    fn claude_event(&mut self, app: &mut crate::App, event: crate::claude_chat::Event) {
        match event {
            crate::claude_chat::Event::Session(session) => {
                app.live.claude_session = Some(ClaudeSession {
                    id: session,
                    users: app.live.user_count(),
                });
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
                    self.claude_turn = None;
                    app.apply_update(Update::Finished { id, result });
                }
            }
            crate::claude_chat::Event::Exited => {
                if let Some((id, _)) = self.claude_turn.take() {
                    app.apply_update(Update::Finished {
                        id,
                        result: Err("Claude Code stopped before the turn ended.".into()),
                    });
                }
            }
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
                Work::CheckJev { endpoint, model } => Update::CheckedJev {
                    id,
                    result: crate::jev_plugin::test_key_for_model(request.key.expose(), &endpoint, &model).await,
                },
                Work::Microcoder { messages, execution, options } => {
                    auto_route(id, messages, execution, &request.key, options, &create, &cancel, &mut text_callback, &mut model_callback, &mut event_callback).await
                }
                // A claude turn runs in its chat's own process
                // ([`Background::claude_request`]), never on a worker.
                Work::Claude { .. } => Update::Finished { id, result: Err("The claude chat has no process to run in.".into()) },
                Work::SolverIntake { messages, cwd, github } => {
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
            crate::route::Lane::OpenRouter => match create(openrouter::ApiKey::new(key.expose())) {
                Ok(provider) => {
                    provider
                        .chat_with_plugins(
                            crate::route::openrouter_model(favor),
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
        provider
            .chat_with_plugins(
                crate::models::GATEWAY_AUTO,
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

/// `auto`'s OpenCode door: one `opencode acp` turn on the free
/// `opencode/big-pickle` model, or the `provider/model` that
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
    let model = acp_client::opencode::Model::parse(
        named
            .as_deref()
            .map_or(coder::delegate_door::OPENCODE_AUTO_MODEL, str::trim),
    )
    .map_err(|why| format!("Coder's OpenCode model is invalid: {why}"))?;
    let permission = if crate::approval::gated() {
        acp_client::opencode::Permission::Workspace
    } else {
        acp_client::opencode::Permission::Full
    };
    let mut events = |event: RuntimeEvent| match event {
        RuntimeEvent::Text(text) => text_callback(&execution.redact_text(&text)),
        RuntimeEvent::Model(model) => model_callback(&model),
        event => event_callback(event),
    };
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
    let result =
        crate::bundled_runtime::acp(&agent, &task, &execution.cwd, None, cancel, &mut events).await;
    crate::bundled_runtime::set_child_env(prior);
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
    cancel: &Arc<AtomicBool>,
    text_callback: &mut (dyn FnMut(&str) + Send),
    model_callback: &mut (dyn FnMut(&str) + Send),
    event_callback: &mut (dyn FnMut(RuntimeEvent) + Send),
) -> Update {
    let cancel = cancel.clone();
    let mut events = |event| match event {
        RuntimeEvent::Text(text) => text_callback(&execution.redact_text(&text)),
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
        let result = crate::bundled_runtime::microcoder_local(
            &task, &execution.cwd, false, client, &execution.redaction_keys,
            &cancel, &mut events,
        ).await?;
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
        crate::identity::prompt(crate::models::AUTO)
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
        );
        assert_eq!(app.program_status().0, "working");
        background.claude_event(
            &mut app,
            Claude::TurnEnd {
                turn: 1,
                result: Ok(Streamed::default()),
            },
        );
        assert_eq!(app.program_status().0, "idle");
        background.claude_event(
            &mut app,
            Claude::Runtime {
                turn: 1,
                event: RuntimeEvent::Plan(Default::default()),
            },
        );
        background.claude_event(
            &mut app,
            Claude::Runtime {
                turn: 1,
                event: text("late"),
            },
        );
        assert_eq!(app.program_status().0, "idle");
        assert!(!app.live.busy);
        background.claude_event(
            &mut app,
            Claude::Runtime {
                turn: 2,
                event: text("Back"),
            },
        );
        assert_eq!(app.program_status().0, "working");
    }
}
