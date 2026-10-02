//! The screen's state, and what each key and each client event does to it.
//!
//! Nothing here touches the terminal or the client. A key or an event
//! changes the state and may return [`Action`]s; the input loop
//! ([`crate::screen`]) runs them. Tests drive this directly.

use std::collections::HashMap;

use coder_terminal::components::card::Card;
use coder_terminal::components::run::RunRow;
use coder_terminal::components::turn::Who;
use coder_terminal::{ComposerAction, Editor, Intensity, Ladder, Scrollback, handle_key};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use openagents_chat::basic_chats::Summary;
use openagents_chat::basic_coder::{Role, Turn};
use openagents_chat::client::{Event, Kind, Op, Start};
use openagents_chat::coder_events::{self, CoderEvent, Line as CoderLine};
use openagents_chat::router::{Context, EngineState, Meta, Offer};
use openagents_chat::tool_groups::Stretch;
use ratatui::text::Line;

use crate::rows::{self, Row};
use crate::slash::{self, Draft, Slash};

/// What the screen is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Waiting for the person.
    Idle,
    /// A message's reply streams.
    Replying,
    /// The thread's Coder run streams.
    Following,
    /// Another operation runs (opening a thread, stopping, answering).
    Working,
}

/// A list over the transcript.
#[derive(Clone, Debug, PartialEq)]
pub enum Overlay {
    /// The thread list, in the shared chat-list order.
    Threads { rows: Vec<Summary>, selected: usize },
    /// The published plugins.
    Plugins {
        rows: Vec<(String, String)>,
        selected: usize,
    },
}

/// What the input loop does next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Run a client operation and stream its events.
    Run(Op),
    /// Stop the thread's Coder task while the current operation follows it.
    StopRun { task: String },
    /// Stop receiving the reply that streams now.
    Interrupt,
    /// Open a thread: its turns, then its Coder run.
    Open(String),
    /// Start a new thread.
    New,
    /// Show the thread list.
    Threads,
    /// Archive a thread, then show the list again.
    Archive(String),
    /// Save the thread as an ATIF trajectory.
    Export,
    /// Show a pairing QR code.
    Connect,
    /// Cancel the pairing code on the screen.
    CancelInvite,
    /// Show the published plugins.
    Plugins,
    /// Show the Coder settings.
    Settings,
    /// Install the host as a service, so chats sync with the phone.
    Sync,
    /// Close the screen.
    Quit,
}

type Wrap = Box<dyn Fn(&Row, usize) -> Vec<Line<'static>> + Send>;

/// The screen's state.
pub struct App {
    pub ladder: Ladder,
    /// The open thread's ID.
    pub thread: String,
    /// The open thread has no message yet: the next send creates it.
    pub fresh: bool,
    /// Which backend holds the threads.
    pub backend: Kind,
    /// The folder the screen runs in, as the status line names it.
    pub folder: String,
    pub transcript: Scrollback<Row, Line<'static>, Wrap>,
    pub editor: Editor,
    pub phase: Phase,
    /// The reply so far, while one streams.
    pub partial: String,
    /// The run's latest progress, drawn live under the transcript.
    pub progress: Option<RunRow>,
    /// The thread's Coder task, once it has one.
    pub task: Option<String>,
    /// The task's turn is running.
    pub running: bool,
    /// Coder asked a question and waits: the composer answers it.
    pub asked: bool,
    /// The last reply offered a Coder run that waits for Enter.
    pub offer: bool,
    /// Who runs Coder now, for the status line.
    pub engine: Option<String>,
    /// A run is starting on this engine and has not said it works yet:
    /// the rail says "Starting Grok Build…" (#10115).
    pub starting: Option<String>,
    pub overlay: Option<Overlay>,
    /// A pairing code is on the screen.
    pub pairing: bool,
    /// Rows scrolled back from the bottom.
    pub scroll: usize,
    /// One Ctrl+C on an empty draft: the next one quits.
    pub armed: bool,
    /// The next detach is the screen's own (to open a list or send), so it
    /// says nothing.
    pub quiet_detach: bool,
    /// The highest event of each task already shown, so following a run
    /// again never repeats a row.
    seen: HashMap<String, u64>,
    /// Frames drawn, for the spinner.
    pub tick: u64,
    /// Tool calls show expanded: each call with its output (Ctrl+O,
    /// `/expand`). Condensed by default.
    pub expanded: bool,
    /// The prompt just sent, for the screen to save.
    sent: Option<String>,
}

impl App {
    pub fn new(ladder: Ladder, thread: String, fresh: bool, backend: Kind, folder: String) -> Self {
        Self {
            ladder,
            thread,
            fresh,
            backend,
            folder,
            transcript: transcript(ladder),
            editor: Editor::new(),
            phase: Phase::Idle,
            partial: String::new(),
            progress: None,
            task: None,
            running: false,
            asked: false,
            offer: false,
            engine: None,
            starting: None,
            overlay: None,
            pairing: false,
            scroll: 0,
            armed: false,
            quiet_detach: false,
            seen: HashMap::new(),
            sent: None,
            tick: 0,
            expanded: false,
        }
    }

    /// Add a row to the transcript, and show the bottom.
    pub fn push(&mut self, row: Row) {
        self.transcript.push(row);
        self.scroll = 0;
    }

    pub fn note(&mut self, text: impl Into<String>) {
        self.push(Row::note(text));
    }

    pub fn loud(&mut self, text: impl Into<String>) {
        self.push(Row::loud(text));
    }

    /// Clear the transcript and everything about the thread, for another.
    pub fn switch(&mut self, thread: String, fresh: bool) {
        self.thread = thread;
        self.fresh = fresh;
        self.transcript = transcript(self.ladder);
        self.partial.clear();
        self.progress = None;
        self.task = None;
        self.running = false;
        self.asked = false;
        self.offer = false;
        self.engine = None;
        self.starting = None;
        self.seen.clear();
        self.scroll = 0;
    }

    /// Show a thread's turns, as opening it does.
    pub fn show_turns(&mut self, turns: &[Turn]) {
        for turn in turns {
            match turn.role {
                Role::User => self.push(Row::Turn(Who::You, turn.text.clone())),
                _ => {
                    self.push(Row::Turn(Who::OpenAgents, turn.text.clone()));
                    if turn.stopped {
                        self.note(openagents_chat::client::STOPPED);
                    }
                }
            }
        }
    }

    /// The welcome card: where the threads live, the project, the coding
    /// agents, and the offer to keep chats in sync with the phone.
    pub fn welcome(&mut self, context: &Context, resumed: Option<&str>) {
        self.push(Row::Card(welcome(self.backend, context, resumed)));
    }

    /// Whether an operation runs on the client now.
    pub fn busy(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// Whether the run should be followed again once the client is free:
    /// it still runs and nothing streams it.
    pub fn wants_follow(&self) -> bool {
        self.running && self.task.is_some() && self.phase == Phase::Idle
    }

    /// One key. `width` is the screen's.
    pub fn key(&mut self, key: &KeyEvent, width: u16) -> Vec<Action> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let armed = std::mem::take(&mut self.armed);
        if self.overlay.is_some() {
            return self.overlay_key(key);
        }
        match (key.code, ctrl, alt) {
            (KeyCode::Char('c'), true, _) => {
                if !self.editor.is_empty() {
                    self.editor.take();
                    return Vec::new();
                }
                if armed {
                    return vec![Action::Quit];
                }
                self.armed = true;
                self.note(if self.running {
                    "Press Ctrl+C again to quit. Coder keeps working."
                } else {
                    "Press Ctrl+C again to quit."
                });
                Vec::new()
            }
            (KeyCode::Char('d'), true, _) if self.editor.is_empty() => vec![Action::Quit],
            (KeyCode::Esc, _, _) => self.stop(),
            (KeyCode::Char('t'), true, _) => vec![Action::Threads],
            (KeyCode::Char('o'), true, _) => {
                self.toggle_tools();
                Vec::new()
            }
            (KeyCode::Char('s'), true, _) => vec![Action::Sync],
            (KeyCode::PageUp, _, _) => {
                self.scroll = self.scroll.saturating_add(10);
                Vec::new()
            }
            (KeyCode::PageDown, _, _) => {
                self.scroll = self.scroll.saturating_sub(10);
                Vec::new()
            }
            _ => match handle_key(&mut self.editor, usize::from(width), key) {
                ComposerAction::Submitted(draft) => {
                    if !draft.trim().is_empty() {
                        self.sent = Some(draft.clone());
                    }
                    self.submit(&draft)
                }
                _ => Vec::new(),
            },
        }
    }

    fn overlay_key(&mut self, key: &KeyEvent) -> Vec<Action> {
        let Some(overlay) = &mut self.overlay else {
            return Vec::new();
        };
        let (len, selected) = match overlay {
            Overlay::Threads { rows, selected } => (rows.len(), selected),
            Overlay::Plugins { rows, selected } => (rows.len(), selected),
        };
        match key.code {
            KeyCode::Esc => {
                self.overlay = None;
                Vec::new()
            }
            KeyCode::Up => {
                *selected = selected.saturating_sub(1);
                Vec::new()
            }
            KeyCode::Down => {
                *selected = (*selected + 1).min(len.saturating_sub(1));
                Vec::new()
            }
            KeyCode::Enter => match overlay {
                Overlay::Threads { rows, selected } => {
                    let id = rows.get(*selected).map(|row| row.id.clone());
                    self.overlay = None;
                    id.map(Action::Open).into_iter().collect()
                }
                Overlay::Plugins { .. } => {
                    self.overlay = None;
                    Vec::new()
                }
            },
            KeyCode::Char('n') if matches!(overlay, Overlay::Threads { .. }) => {
                self.overlay = None;
                vec![Action::New]
            }
            KeyCode::Char('a') => match overlay {
                Overlay::Threads { rows, selected } => rows
                    .get(*selected)
                    .map(|row| Action::Archive(row.id.clone()))
                    .into_iter()
                    .collect(),
                Overlay::Plugins { .. } => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// Esc and `/stop`: stop what streams now, the most recent first.
    fn stop(&mut self) -> Vec<Action> {
        match self.phase {
            Phase::Replying => vec![Action::Interrupt],
            Phase::Following | Phase::Idle if self.running && self.task.is_some() => {
                let task = self.task.clone().unwrap_or_default();
                if self.phase == Phase::Following {
                    vec![Action::StopRun { task }]
                } else {
                    vec![Action::Run(Op::Stop {
                        thread: self.thread.clone(),
                    })]
                }
            }
            _ if self.pairing => vec![Action::CancelInvite],
            _ => Vec::new(),
        }
    }

    /// A submitted draft: a slash command, an answer to Coder, an accepted
    /// offer, or a message for the chat.
    /// The prompt the last key sent, once: the screen saves it.
    pub fn take_sent(&mut self) -> Option<String> {
        self.sent.take()
    }

    pub fn submit(&mut self, draft: &str) -> Vec<Action> {
        match slash::parse(draft) {
            Draft::Empty if self.offer && !self.busy() => {
                self.offer = false;
                vec![Action::Run(Op::RunCoder {
                    thread: self.thread.clone(),
                })]
            }
            Draft::Empty => Vec::new(),
            Draft::Unknown(word) => {
                self.note(format!("{word} is not a command; /help lists them."));
                Vec::new()
            }
            Draft::Command(slash) => self.command(slash),
            Draft::Message(text) => self.send(text),
        }
    }

    fn command(&mut self, slash: Slash) -> Vec<Action> {
        match slash {
            Slash::New => vec![Action::New],
            Slash::Threads => vec![Action::Threads],
            Slash::Stop => {
                let actions = self.stop();
                if actions.is_empty() {
                    self.note("Nothing is running.");
                }
                actions
            }
            Slash::Export => vec![Action::Export],
            Slash::Settings => vec![Action::Settings],
            Slash::Connect => vec![Action::Connect],
            Slash::Plugins => vec![Action::Plugins],
            Slash::Expand => {
                self.toggle_tools();
                Vec::new()
            }
            Slash::Help => {
                self.push(Row::Card(help()));
                Vec::new()
            }
            Slash::Quit => vec![Action::Quit],
        }
    }

    /// A message: an answer when Coder waits on one, else a send.
    fn send(&mut self, text: String) -> Vec<Action> {
        if self.phase == Phase::Replying || self.phase == Phase::Working {
            self.note("Wait for this reply, or press Esc to stop it.");
            self.editor.insert_str(&text);
            return Vec::new();
        }
        self.push(Row::Turn(Who::You, text.clone()));
        self.offer = false;
        if self.asked && self.task.is_some() {
            self.asked = false;
            return vec![Action::Run(Op::Answer {
                thread: self.thread.clone(),
                text,
            })];
        }
        let new = std::mem::take(&mut self.fresh);
        vec![Action::Run(Op::Send {
            thread: self.thread.clone(),
            new,
            text,
            start: Start::Settings,
            timeout: openagents_chat::client::DEFAULT_TIMEOUT,
        })]
    }

    /// Expand every stretch of tool calls, or condense them all again.
    pub fn toggle_tools(&mut self) {
        self.expanded = !self.expanded;
        for row in self.transcript.lines_mut() {
            if let Row::Tools { expanded, .. } = row {
                *expanded = self.expanded;
            }
        }
    }

    /// An operation is about to run.
    pub fn began(&mut self, op: &Op) {
        self.phase = match op {
            Op::Send { .. } => Phase::Replying,
            Op::Follow { .. } => Phase::Following,
            Op::RunCoder { .. } | Op::Answer { .. } => Phase::Following,
            Op::Stop { .. } => Phase::Working,
        };
        self.partial.clear();
    }

    /// The operation ended.
    pub fn ended(&mut self, failed: Option<String>) {
        self.phase = Phase::Idle;
        self.starting = None;
        self.partial.clear();
        if !self.running {
            self.progress = None;
        }
        if let Some(message) = failed {
            self.loud(message);
        }
    }

    /// One event of the running operation.
    pub fn event(&mut self, event: Event) {
        match event {
            Event::Migrated { moved } => self.note(format!(
                "Moved {moved} thread{} kept without a host into this computer's host.",
                if moved == 1 { "" } else { "s" }
            )),
            Event::Kept { home, message } => self.note(format!(
                "Threads in {} stay there for now: {message}",
                home.display()
            )),
            Event::Accepted { .. } => {}
            Event::Partial { text, .. } => self.partial = text,
            Event::Reply {
                reply,
                computer,
                running,
                ..
            } => {
                self.partial.clear();
                self.push(Row::Turn(Who::OpenAgents, reply.text.clone()));
                let meta = reply.meta.clone().unwrap_or_default();
                self.notes(&meta, computer && !running, running);
                if running {
                    self.phase = Phase::Following;
                    // The rail says who is starting from the reply on: a
                    // start never leaves the screen silent (#10115).
                    self.starting = Some(
                        meta.runner
                            .as_ref()
                            .and_then(|runner| runner.provider())
                            .map_or_else(|| "Starting Coder…".to_owned(), coder_events::starting),
                    );
                }
            }
            Event::ReplyFailed {
                message, partial, ..
            } => {
                self.partial.clear();
                if let Some(text) = partial.filter(|text| !text.trim().is_empty()) {
                    self.push(Row::Turn(Who::OpenAgents, text));
                }
                self.loud(message);
            }
            Event::Failure { message, .. } => self.loud(message),
            Event::Starting { engine, .. } => {
                self.starting = Some(coder_events::starting(&engine));
            }
            Event::Coder {
                accepted,
                message,
                task,
                quiet,
                ..
            } => {
                if accepted {
                    if !quiet {
                        self.note(message);
                    }
                    // The issue flow runs in this process (docs/terminal,
                    // decision 4): say so before the person quits.
                    if task.as_ref().and_then(|task| task.get("issue")).is_some() {
                        self.push(Row::Note(
                            "This issue flow runs inside this screen: keep it open until the flow \
                             lands; quitting first leaves the issue claimed without its closing \
                             comment."
                                .into(),
                            Intensity::ThreeQuarters,
                        ));
                    }
                    if let Some(id) = task
                        .as_ref()
                        .and_then(|task| task.get("task"))
                        .and_then(|task| task.as_str())
                    {
                        self.task = Some(id.to_owned());
                        self.running = true;
                    }
                } else {
                    self.starting = None;
                    self.loud(message);
                }
            }
            Event::Unbound { why, .. } => self.loud(format!(
                "Coder started, but the thread could not record its task ({why})."
            )),
            Event::Line(line) => self.line(*line),
            Event::TaskUnreadable { task, message, .. } => {
                self.running = false;
                self.loud(format!("Cannot read task {task}: {message}"));
            }
            Event::Lost => {
                self.running = false;
                self.loud("The task could not be read.");
            }
            Event::Stop {
                requested, message, ..
            } => {
                if requested {
                    self.note(message);
                } else {
                    self.loud(message);
                }
            }
            Event::Stopping { why } => self.note(format!(
                "Stopping: Coder ends the issue flow at its next step and says so on the issue.{}",
                why.map(|why| format!(" ({why})")).unwrap_or_default()
            )),
            Event::Detached { .. } => {
                if !std::mem::take(&mut self.quiet_detach) {
                    self.note(
                        "Stopped following. Coder keeps working; Esc stops it, and opening \
                         this thread follows it again.",
                    );
                }
            }
        }
    }

    /// One event of the thread's Coder task.
    fn line(&mut self, line: CoderLine) {
        let seen = self.seen.entry(line.task.clone()).or_insert(0);
        if line.seq != 0 && line.seq <= *seen {
            return;
        }
        *seen = line.seq.max(*seen);
        self.task = Some(line.task.clone());
        match &line.event {
            CoderEvent::Progress(progress) => {
                self.running = true;
                self.progress = Some(rows::progress_row(progress));
                return;
            }
            CoderEvent::CoderStarted(started) => {
                self.running = true;
                self.asked = false;
                self.starting = None;
                self.engine = Some(rows::provider(&started.provider));
                // One short line (#10115), and who runs only when that is
                // news: another engine than asked, or one passed over.
                self.push(Row::Note(started.line(), Intensity::ThreeQuarters));
                if let Some(news) = started.news() {
                    self.note(news);
                }
            }
            CoderEvent::Question(_) | CoderEvent::Approval(_) => self.asked = true,
            _ => {}
        }
        if line.event.ends_turn() {
            self.running = false;
            self.progress = None;
            if let Some(Row::Tools { stretch, .. }) = self.transcript.last_mut() {
                stretch.settle();
            }
        }
        // A command, a tool call, a thought, or what one returned joins the
        // open stretch, or starts one (#10117). A reply draws no row here,
        // so it never splits one.
        if let Some(Row::Tools { stretch, .. }) = self.transcript.last_mut()
            && stretch.push(line.seq, &line.event)
        {
            self.scroll = 0;
            return;
        }
        let mut stretch = Stretch::default();
        if stretch.push(line.seq, &line.event) {
            self.push(Row::Tools {
                stretch,
                expanded: self.expanded,
            });
            return;
        }
        if let Some(row) = rows::run_row(&line.event) {
            self.push(Row::Run(row));
        }
    }

    /// What the router said beside a reply: offers and who would run
    /// Coder. Suggested follow-ups are the apps' chips, not shown here.
    fn notes(&mut self, meta: &Meta, offered: bool, running: bool) {
        let mut coder = offered;
        for offer in &meta.offers {
            match offer {
                Offer::RunCoder => coder = !running,
                Offer::OpenScreen { screen } => {
                    self.note(format!("Open {screen:?} in the OpenAgents app."));
                }
                Offer::Cli { argv, .. } => {
                    self.note(format!("Run: {}", Offer::command_line(argv)));
                }
                Offer::StartEval { .. } => {
                    self.note("Run this test set from the OpenAgents app.");
                }
                Offer::PublishEval { .. } => {
                    self.note("Add this result to the Gym from the OpenAgents app.");
                }
                Offer::OpenPresentation { .. } => {
                    self.note(openagents_chat::router::PRESENTATION_ELSEWHERE);
                }
            }
        }
        for card in &meta.cards {
            let kind = card
                .get("kind")
                .or_else(|| card.get("type"))
                .and_then(|kind| kind.as_str())
                .unwrap_or("result");
            self.note(format!(
                "This reply has a {kind} card; open it on your phone or in the desktop app."
            ));
        }
        if coder {
            self.offer = true;
            self.push(Row::Note(
                "Enter to run Coder.".to_owned(),
                Intensity::ThreeQuarters,
            ));
        }
        // Who runs it, only when that says more than the reply did: another
        // engine than the one asked for, one passed over, or none ready.
        if let Some(runner) = meta.runner.as_ref().filter(|runner| !runner.plain()) {
            self.note(runner.text());
        }
    }

    /// The status rail's left text: what is happening.
    pub fn status(&self) -> String {
        match self.phase {
            _ if self.overlay.is_some() => "Esc closes the list".into(),
            Phase::Replying => "replying · Esc stops".into(),
            Phase::Working => "working".into(),
            _ if self.asked => "Coder asks · type your answer".into(),
            _ if self.starting.is_some() => self.starting.clone().unwrap_or_default(),
            _ if self.running => match &self.progress {
                Some(RunRow::Progress {
                    step,
                    percent,
                    seconds,
                }) => format!(
                    "Coder · step {step}{} · {} · Esc stops",
                    percent
                        .map(|percent| format!(" · ≈{percent}% done"))
                        .unwrap_or_default(),
                    coder_terminal::components::run::elapsed(*seconds)
                ),
                _ => "Coder is working · Esc stops".into(),
            },
            _ if self.offer => "Enter starts Coder".into(),
            _ => "ready".into(),
        }
    }

    /// The bottom rail's right text: the engine and the thread.
    pub fn tail(&self) -> String {
        let thread = if self.fresh {
            "new thread".to_owned()
        } else {
            format!("thread {}", &self.thread[..self.thread.len().min(8)])
        };
        match &self.engine {
            Some(engine) => format!("{engine} · {thread}"),
            None => thread,
        }
    }
}

fn transcript(ladder: Ladder) -> Scrollback<Row, Line<'static>, Wrap> {
    let wrap: Wrap = Box::new(move |row: &Row, width: usize| {
        rows::lines(row, u16::try_from(width).unwrap_or(u16::MAX), ladder)
    });
    Scrollback::new(wrap)
}

/// The welcome card: the version, then three short facts, as the old Coder
/// Terminal's was. No prose and no key legend: `/help` lists the keys.
pub fn welcome(backend: Kind, context: &Context, _resumed: Option<&str>) -> Card {
    let project = context
        .project
        .as_ref()
        .map_or_else(|| "none".to_owned(), |project| project.name.clone());
    let ready: Vec<String> = match &context.computer {
        Some(openagents_chat::router::Computer::Here { engines, .. }) => engines
            .iter()
            .filter(|engine| engine.state == EngineState::Ready)
            .map(|engine| rows::provider(&engine.engine))
            .collect(),
        _ => Vec::new(),
    };
    let agents = if ready.is_empty() {
        "none signed in".to_owned()
    } else {
        ready.join(" · ")
    };
    let chats = match backend {
        Kind::Host => "synced",
        Kind::InProcess => "this computer · Ctrl+S to sync",
        Kind::Scratch => "scratch",
    };
    Card {
        title: format!("OpenAgents v{}", env!("CARGO_PKG_VERSION")),
        rows: vec![
            ("Project".into(), project),
            ("Agents".into(), agents),
            ("Chats".into(), chats.into()),
        ],
        body: Vec::new(),
        art: Vec::new(),
        keys: Vec::new(),
    }
}

/// The `/help` card.
pub fn help() -> Card {
    let mut rows: Vec<(String, String)> = Slash::ALL
        .iter()
        .map(|slash| (format!("/{}", slash.word()), slash.about().to_owned()))
        .collect();
    rows.extend([
        (
            "Enter".to_owned(),
            "send; on an empty line, start the Coder run OpenAgents offered".to_owned(),
        ),
        (
            "Esc".to_owned(),
            "stop the reply, or stop the Coder run".to_owned(),
        ),
        ("Ctrl+T".to_owned(), "threads".to_owned()),
        (
            "Ctrl+O".to_owned(),
            "expand or condense tool calls".to_owned(),
        ),
        (
            "Ctrl+S".to_owned(),
            "keep this computer's chats in sync with your phone".to_owned(),
        ),
        ("PageUp/Down".to_owned(), "scroll".to_owned()),
        (
            "Ctrl+C twice".to_owned(),
            "quit; a Coder run keeps going".to_owned(),
        ),
    ]);
    Card {
        title: "Commands and keys".into(),
        rows,
        body: vec![
            "Everything else you type goes to OpenAgents. Press Esc to stop a reply or a Coder run."
                .into(),
        ],
        art: Vec::new(),
        keys: Vec::new(),
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
