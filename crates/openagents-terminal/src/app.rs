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
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use openagents_chat::basic_coder::{Role, Turn};
use openagents_chat::client::{Event, Kind, Op, Start};
use openagents_chat::coder_events::{self, CoderEvent, Line as CoderLine};
use openagents_chat::router::{Context, EngineState, Meta, Offer};
use openagents_chat::tool_groups::Stretch;
use ratatui::text::Line;

use crate::picker::{Picked, Picker};
use crate::rows::{self, Row};
use crate::slash::{self, Draft, Slash};
use crate::view::{FileView, RunView, Selection, Shown};

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
    /// The thread picker (`/resume`, Ctrl+T).
    Threads(Picker),
    /// The published plugins.
    Plugins {
        rows: Vec<crate::Plugin>,
        selected: usize,
    },
    /// The Coder settings, each turned on or off in place.
    Settings {
        settings: crate::Settings,
        selected: usize,
    },
    /// The background rules (`/background`).
    Background {
        rows: Vec<crate::BackgroundRow>,
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
    /// Show the thread picker.
    Threads,
    /// Open the thread `/resume ARG` names (an ID, an ID prefix, or a
    /// title), else the picker narrowed to it.
    Resume(String),
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
    /// Turn the installed plugin `id` on or off.
    TurnPlugin { id: String, name: String, on: bool },
    /// Install the published plugin `id` on this computer, off.
    InstallPlugin { id: String, name: String },
    /// Run the installed plugin `key` with `request`.
    RunPlugin {
        key: String,
        name: String,
        request: String,
    },
    /// Show the background rules.
    Background,
    /// Show, dry-run, run, pause, resume, or read the log of a background
    /// rule.
    BackgroundAct {
        id: String,
        act: crate::BackgroundAct,
    },
    /// Copy Claude Code and Codex sessions in as threads.
    Import,
    /// Show the Coder settings.
    Settings,
    /// Turn the setting `key` on or off.
    Change { key: String, on: bool },
    /// Install the host as a service, so chats sync with the phone.
    Sync,
    /// Put this text on the terminal's clipboard.
    Copy(String),
    /// Send `text` to the running task (steering).
    Steer { task: String, text: String },
    /// Open the file a clicked word names, if it is one, at `line`.
    OpenFile { path: String, line: Option<usize> },
    /// Close the screen.
    Quit,
}

pub(crate) type Wrap = Box<dyn Fn(&Row, usize) -> Vec<Line<'static>> + Send>;

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
    /// Another computer's name, when the threads are that computer's.
    pub computer: Option<String>,
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
    /// The last reply proposed this `openagents` command, which changes
    /// something here and waits for Enter (#10170).
    pub command: Option<Vec<String>>,
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
    /// Ticks of the 30 fps animation clock, for the spinner
    /// ([`coder_terminal::grok_spinner`]).
    pub tick: u64,
    /// What is happening while nothing else shows it, beside the spinner:
    /// "Starting Grok Build…", "Grok Build connected · grok-4.7",
    /// "Thinking…", and the tick it began at, for its timer.
    pub activity: Option<(String, u64)>,
    /// Tool calls show expanded: each call with its output (Ctrl+O,
    /// `/expand`). Condensed by default.
    pub expanded: bool,
    /// The prompt just sent, for the screen to save.
    sent: Option<String>,
    /// What the last Ctrl+Y copied: 0 the reply, n its nth code block from
    /// the end. The next Ctrl+Y copies the one before.
    copied: Option<usize>,
    /// The chat cannot be reached: the reply is asked for again in this
    /// many seconds.
    pub offline: Option<u64>,
    /// A plugin picked from the list: the next message is its request.
    pub plugin: Option<crate::Plugin>,
    /// The background rule whose dry run was just shown: `r` on it in
    /// `/background` runs it for real.
    pub background_armed: Option<String>,
    /// When the last background notification shown was sent.
    pub notice_seen: u64,
    /// The thread's Coder run alone, every call with its output, for the
    /// run view.
    pub run_log: Scrollback<Row, Line<'static>, Wrap>,
    /// The run view is open (Ctrl+R, `/run`).
    pub run_view: Option<RunView>,
    /// A file is open, read only.
    pub file: Option<FileView>,
    /// The text the mouse is selecting or selected.
    pub selection: Option<Selection>,
    /// What the last frame drew where text can be selected.
    pub shown: Shown,
    /// Messages sent to the run that it has not read yet.
    steering: usize,
    /// A message started this turn: the turn before it ending does not
    /// end the run.
    next_turn: Option<usize>,
    /// The run's worktree, where the paths it names are.
    pub worktree: Option<String>,
    /// Each Coder run this thread started, for the rail under the
    /// composer; a run's number is its place here, from one.
    pub delegations: Vec<crate::rail::Delegation>,
    /// The rail row the keyboard is on, by its number.
    pub rail: Option<usize>,
}

impl App {
    pub fn new(ladder: Ladder, thread: String, fresh: bool, backend: Kind, folder: String) -> Self {
        Self {
            ladder,
            thread,
            fresh,
            backend,
            folder,
            computer: None,
            transcript: transcript(ladder),
            editor: Editor::new(),
            phase: Phase::Idle,
            partial: String::new(),
            progress: None,
            task: None,
            running: false,
            asked: false,
            offer: false,
            command: None,
            engine: None,
            starting: None,
            overlay: None,
            pairing: false,
            scroll: 0,
            armed: false,
            quiet_detach: false,
            seen: HashMap::new(),
            sent: None,
            copied: None,
            tick: 0,
            activity: None,
            expanded: false,
            offline: None,
            plugin: None,
            background_armed: None,
            notice_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
            run_log: transcript(ladder),
            run_view: None,
            file: None,
            selection: None,
            shown: Shown::default(),
            steering: 0,
            next_turn: None,
            worktree: None,
            delegations: Vec::new(),
            rail: None,
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
        self.command = None;
        self.engine = None;
        self.starting = None;
        self.activity = None;
        self.seen.clear();
        self.scroll = 0;
        self.run_log = transcript(self.ladder);
        self.run_view = None;
        self.steering = 0;
        self.next_turn = None;
        self.worktree = None;
        self.delegations.clear();
        self.rail = None;
    }

    /// Add a row to the run view's log, and show its bottom.
    fn push_run(&mut self, row: Row) {
        self.track_row(&row);
        self.run_log.push(row);
        if let Some(view) = &mut self.run_view {
            view.scroll = 0;
        }
    }

    /// A row of the run, in the transcript and the run view.
    fn push_both(&mut self, row: Row) {
        self.push_run(row.clone());
        self.push(row);
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
    /// agents, the background watchers running here, and the offer to keep
    /// chats in sync with the phone.
    pub fn welcome(&mut self, context: &Context, resumed: Option<&str>, watchers: &[String]) {
        let mut card = welcome(self.backend, context, resumed);
        if let Some(computer) = &self.computer {
            // Its Coder runs there, so this computer's project, agents,
            // and watchers say nothing about it.
            card.rows = vec![("Computer".into(), computer.clone())];
        } else if let Some(line) = openagents_chat_app::watchers::line(watchers) {
            card.rows.push(("Running".into(), line));
        }
        self.push(Row::Card(card));
    }

    /// Whether an operation runs on the client now.
    pub fn busy(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// Whether a spinner is on the screen: the composer's while anything
    /// runs, and the live line's.
    #[must_use]
    pub fn animating(&self) -> bool {
        self.busy() || self.live_status().is_some() || self.rail_animating()
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
        let copied = std::mem::take(&mut self.copied);
        self.selection = None;
        if self.overlay.is_some() {
            return self.overlay_key(key);
        }
        if self.file.is_some() {
            self.file_key(key);
            return Vec::new();
        }
        if let Some(actions) = self.rail_key(key) {
            return actions;
        }
        match (key.code, ctrl, alt) {
            (KeyCode::Char('r'), true, _) => {
                self.toggle_run();
                Vec::new()
            }
            (KeyCode::Esc, _, _) if self.run_view.is_some() && self.plugin.is_none() => {
                self.run_view = None;
                Vec::new()
            }
            (KeyCode::PageUp, _, _) if self.run_view.is_some() => {
                self.scroll_view(10);
                Vec::new()
            }
            (KeyCode::PageDown, _, _) if self.run_view.is_some() => {
                self.scroll_view(-10);
                Vec::new()
            }
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
            (KeyCode::Esc, _, _) if self.command.is_some() && !self.busy() => {
                self.command = None;
                self.note("The command was not run.");
                Vec::new()
            }
            (KeyCode::Esc, _, _) if self.plugin.is_some() => {
                self.plugin = None;
                self.note("No plugin runs.");
                Vec::new()
            }
            (KeyCode::Esc, _, _) => self.stop(),
            (KeyCode::Char('t'), true, _) => vec![Action::Threads],
            (KeyCode::Char('n'), true, _) => vec![Action::New],
            (KeyCode::Char('y'), true, _) => self.copy(copied),
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
        if let Overlay::Threads(picker) = overlay {
            return match picker.key(key) {
                Picked::Nothing => Vec::new(),
                Picked::Close => {
                    self.overlay = None;
                    Vec::new()
                }
                Picked::Open(id) => {
                    self.overlay = None;
                    vec![Action::Open(id)]
                }
                Picked::New => {
                    self.overlay = None;
                    vec![Action::New]
                }
                Picked::Archive(id) => vec![Action::Archive(id)],
                Picked::Copy(id) => {
                    self.note("Copied the thread ID.");
                    vec![Action::Copy(id)]
                }
            };
        }
        if key.code == KeyCode::Esc {
            self.overlay = None;
            return Vec::new();
        }
        match overlay {
            Overlay::Threads(_) => Vec::new(),
            Overlay::Plugins { rows, selected } => {
                match key.code {
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Down => *selected = (*selected + 1).min(rows.len().saturating_sub(1)),
                    KeyCode::Enter => {
                        let picked = rows.get(*selected).cloned();
                        self.overlay = None;
                        if let Some(plugin) = picked {
                            if let (None, Some(id)) = (&plugin.key, &plugin.id) {
                                self.note(format!(
                                    "Installing {}; it starts off, and Space in /plugins turns it on.",
                                    plugin.name
                                ));
                                return vec![Action::InstallPlugin {
                                    id: id.clone(),
                                    name: plugin.name,
                                }];
                            }
                            self.pick(plugin);
                        }
                    }
                    KeyCode::Char(' ') => {
                        let picked = rows.get(*selected).cloned();
                        if let Some(crate::Plugin {
                            id: Some(id),
                            on: Some(on),
                            name,
                            ..
                        }) = picked
                        {
                            self.overlay = None;
                            return vec![Action::TurnPlugin { id, name, on: !on }];
                        }
                    }
                    _ => {}
                }
                Vec::new()
            }
            Overlay::Settings { settings, selected } => {
                match key.code {
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Down => {
                        *selected = (*selected + 1).min(settings.choices.len().saturating_sub(1));
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => return self.flip(),
                    _ => {}
                }
                Vec::new()
            }
            Overlay::Background { rows, selected } => {
                let picked = rows.get(*selected).cloned();
                let act = match key.code {
                    KeyCode::Up => {
                        *selected = selected.saturating_sub(1);
                        None
                    }
                    KeyCode::Down => {
                        *selected = (*selected + 1).min(rows.len().saturating_sub(1));
                        None
                    }
                    KeyCode::Enter => Some(crate::BackgroundAct::Show),
                    // A run shows its dry run first; `r` again runs it.
                    KeyCode::Char('r') => Some(
                        if picked.as_ref().map(|row| &row.id) == self.background_armed.as_ref() {
                            crate::BackgroundAct::Run
                        } else {
                            crate::BackgroundAct::DryRun
                        },
                    ),
                    KeyCode::Char('p') => Some(if picked.as_ref().is_some_and(|row| row.paused) {
                        crate::BackgroundAct::Resume
                    } else {
                        crate::BackgroundAct::Pause
                    }),
                    KeyCode::Char('l') => Some(crate::BackgroundAct::Log),
                    _ => None,
                };
                match (act, picked) {
                    (Some(act), Some(row)) => {
                        self.overlay = None;
                        self.background_armed = None;
                        vec![Action::BackgroundAct { id: row.id, act }]
                    }
                    _ => Vec::new(),
                }
            }
        }
    }

    /// A plugin picked from the list: one installed here waits for its
    /// request; one only published says so.
    fn pick(&mut self, plugin: crate::Plugin) {
        if plugin.key.is_none() {
            self.note(format!(
                "{} is published but not installed on this computer, so it cannot run here.",
                plugin.name
            ));
            return;
        }
        self.note(format!(
            "Type what to ask {} and press Enter; it runs on this folder and reads files only. Esc cancels.",
            plugin.name
        ));
        self.plugin = Some(plugin);
    }

    /// Turn the selected setting on or off.
    fn flip(&mut self) -> Vec<Action> {
        let Some(Overlay::Settings { settings, selected }) = &self.overlay else {
            return Vec::new();
        };
        let Some(choice) = settings.choices.get(*selected).cloned() else {
            return Vec::new();
        };
        if let (false, Some(why)) = (choice.on, &choice.blocked) {
            self.note(why.clone());
            return Vec::new();
        }
        vec![Action::Change {
            key: choice.key,
            on: !choice.on,
        }]
    }

    /// The settings after a change, or as first read: the list shows them,
    /// keeping its place.
    pub fn settings(&mut self, settings: crate::Settings) {
        let selected = match &self.overlay {
            Some(Overlay::Settings { selected, .. }) => *selected,
            _ => 0,
        }
        .min(settings.choices.len().saturating_sub(1));
        self.overlay = Some(Overlay::Settings { settings, selected });
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
    /// Ctrl+Y: copy the last reply; pressed again, its code blocks from the
    /// last up, then the reply again.
    fn copy(&mut self, copied: Option<usize>) -> Vec<Action> {
        let reply = self
            .transcript
            .lines()
            .filter_map(|row| match row {
                Row::Turn(Who::OpenAgents, text) => Some(text.clone()),
                _ => None,
            })
            .last();
        let Some(reply) = reply else {
            self.note("No reply to copy yet.");
            return Vec::new();
        };
        let blocks = coder_terminal::markdown::code_blocks(&reply);
        let step = copied.map_or(0, |step| (step + 1) % (blocks.len() + 1));
        self.copied = Some(step);
        if step == 0 {
            self.note("Copied the reply.");
            return vec![Action::Copy(reply)];
        }
        let index = blocks.len() - step;
        self.note(format!(
            "Copied code block {} of {}.",
            index + 1,
            blocks.len()
        ));
        vec![Action::Copy(blocks[index].clone())]
    }

    /// The prompt the last key sent, once: the screen saves it.
    pub fn take_sent(&mut self) -> Option<String> {
        self.sent.take()
    }

    pub fn submit(&mut self, draft: &str) -> Vec<Action> {
        // A picked plugin takes the next message, or an empty line, as its
        // request.
        if let Some(plugin) = self.plugin.take() {
            match slash::parse(draft) {
                Draft::Message(_) | Draft::Empty => {
                    let request = draft.trim().to_owned();
                    if !request.is_empty() {
                        self.push(Row::Turn(Who::You, request.clone()));
                    }
                    return vec![Action::RunPlugin {
                        key: plugin.key.unwrap_or_default(),
                        name: plugin.name,
                        request,
                    }];
                }
                _ => self.note(format!("{} was not run.", plugin.name)),
            }
        }
        match slash::parse(draft) {
            Draft::Empty if self.command.is_some() && !self.busy() => {
                self.command = None;
                vec![Action::Run(Op::RunCommand {
                    thread: self.thread.clone(),
                })]
            }
            Draft::Empty if self.offer && !self.busy() => {
                self.offer = false;
                vec![Action::Run(Op::RunCoder {
                    thread: self.thread.clone(),
                })]
            }
            Draft::Empty => Vec::new(),
            Draft::Unknown(word) => {
                self.note(format!("{word} is not a command."));
                self.push(Row::Card(commands(&word)));
                Vec::new()
            }
            Draft::Command(slash) => self.command(slash),
            Draft::Open(number) => {
                self.open_delegation(number);
                Vec::new()
            }
            Draft::With(Slash::Resume, arg) => vec![Action::Resume(arg)],
            Draft::With(slash, _) => self.command(slash),
            Draft::Message(text) => self.send(text),
        }
    }

    fn command(&mut self, slash: Slash) -> Vec<Action> {
        match slash {
            Slash::New => vec![Action::New],
            Slash::Threads | Slash::Resume => vec![Action::Threads],
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
            Slash::Background => vec![Action::Background],
            Slash::Import => vec![Action::Import],
            Slash::Expand => {
                self.toggle_tools();
                Vec::new()
            }
            Slash::Help => {
                self.push(Row::Card(help()));
                Vec::new()
            }
            Slash::Run => {
                self.toggle_run();
                Vec::new()
            }
            Slash::Open => {
                match self.rail_numbers().as_slice() {
                    [only] => self.open_delegation(*only),
                    _ => self.open_delegation(0),
                }
                Vec::new()
            }
            Slash::Quit => vec![Action::Quit],
        }
    }

    /// A message: an answer when Coder waits on one, else a send.
    fn send(&mut self, text: String) -> Vec<Action> {
        // The run view's composer talks to the run: an answer when it asks,
        // else a message it reads at its next step.
        // A run opened from the rail that is not the thread's current
        // one takes the message the same way, in its own log.
        if let Some(held) = self.viewed()
            && self.task.as_deref() != Some(held.task.as_str())
        {
            let task = held.task.clone();
            let number = self.run_view.and_then(|view| view.number).unwrap_or(1);
            self.delegations[number - 1]
                .log
                .push(Row::Turn(Who::You, text.clone()));
            return vec![Action::Steer { task, text }];
        }
        if self.run_view.is_some()
            && !self.asked
            && let Some(task) = self.task.clone()
        {
            self.push_both(Row::Turn(Who::You, text.clone()));
            self.steering += 1;
            return vec![Action::Steer { task, text }];
        }
        if self.phase == Phase::Replying || self.phase == Phase::Working {
            self.note("Wait for this reply, or press Esc to stop it.");
            self.editor.insert_str(&text);
            return Vec::new();
        }
        self.push(Row::Turn(Who::You, text.clone()));
        self.offer = false;
        self.command = None;
        if self.asked && self.task.is_some() {
            self.asked = false;
            self.push_run(Row::Turn(Who::You, text.clone()));
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

    /// Expand every stretch of tool calls and every run result's changes,
    /// or condense them all again.
    pub fn toggle_tools(&mut self) {
        self.expanded = !self.expanded;
        for row in self.transcript.lines_mut() {
            match row {
                Row::Tools { expanded, .. } | Row::Run(RunRow::Result { expanded, .. }) => {
                    *expanded = self.expanded
                }
                _ => {}
            }
        }
    }

    /// An operation is about to run.
    pub fn began(&mut self, op: &Op) {
        self.phase = match op {
            Op::Send { .. } => Phase::Replying,
            Op::Follow { .. } => Phase::Following,
            Op::RunCoder { .. } | Op::Answer { .. } => Phase::Following,
            Op::Stop { .. } | Op::RunCommand { .. } => Phase::Working,
        };
        self.partial.clear();
        if let Op::Send { .. } = op {
            self.doing("Replying…");
        }
    }

    /// Say what is happening now beside the spinner; the same words keep
    /// their timer.
    fn doing(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.activity.as_ref().is_none_or(|(now, _)| *now != text) {
            self.activity = Some((text, self.tick));
        }
    }

    /// The spinner's line while something is in progress and nothing
    /// streams it: what is happening and for how long.
    #[must_use]
    pub fn live_status(&self) -> Option<(&str, std::time::Duration)> {
        let (text, since) = self.activity.as_ref()?;
        let shown = self.partial.is_empty()
            && !self.asked
            && (self.running || self.phase == Phase::Replying || self.starting.is_some());
        shown.then(|| {
            (
                text.as_str(),
                coder_terminal::grok_spinner::elapsed(self.tick.saturating_sub(*since)),
            )
        })
    }

    /// The operation ended.
    pub fn ended(&mut self, failed: Option<String>) {
        self.phase = Phase::Idle;
        self.offline = None;
        self.starting = None;
        self.partial.clear();
        if !self.running {
            self.progress = None;
            self.activity = None;
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
                // The offer is what `run-coder` accepts, read the same way
                // (#10170): the computer lane alone is not an offer.
                let offered = openagents_chat::delegation::offered(reply.meta.as_ref(), computer);
                let meta = reply.meta.clone().unwrap_or_default();
                self.notes(&meta, offered && !running, running);
                if running {
                    self.phase = Phase::Following;
                    // The rail says who is starting from the reply on: a
                    // start never leaves the screen silent (#10115).
                    let starting = meta
                        .runner
                        .as_ref()
                        .and_then(|runner| runner.provider())
                        .map_or_else(|| "Starting Coder…".to_owned(), coder_events::starting);
                    self.doing(starting.clone());
                    self.starting = Some(starting);
                } else if !self.running {
                    self.activity = None;
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
                let starting = coder_events::starting(&engine);
                self.doing(starting.clone());
                self.starting = Some(starting);
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
                    if let Some(id) = task
                        .as_ref()
                        .and_then(|task| task.get("task"))
                        .and_then(|task| task.as_str())
                    {
                        self.task = Some(id.to_owned());
                        self.running = true;
                        self.delegated(id);
                    }
                } else {
                    self.starting = None;
                    self.activity = None;
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
            Event::Offline { retry_in, .. } => {
                if self.offline.is_none() {
                    self.loud("Reconnecting to OpenAgents… (Esc stops)");
                }
                self.offline = Some(retry_in);
            }
            Event::Online { .. } => {
                if self.offline.take().is_some() {
                    self.note("Connected again.");
                }
            }
            Event::Command { argv, confirm, .. } => {
                let line = Offer::command_line(&argv);
                if confirm {
                    self.push(Row::Note(
                        format!("Enter runs {line} · Esc cancels"),
                        Intensity::ThreeQuarters,
                    ));
                    self.command = Some(argv);
                } else {
                    self.doing(format!("Running {line}…"));
                }
            }
            Event::Ran {
                argv, ok, output, ..
            } => {
                self.activity = None;
                let line = Offer::command_line(&argv);
                let shown = if output.trim().is_empty() {
                    format!("{line} printed nothing.")
                } else {
                    format!("{line}\n\n```\n{output}\n```")
                };
                self.push(Row::Turn(Who::OpenAgents, shown));
                if !ok {
                    self.loud(format!("{line} failed."));
                }
            }
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
    /// Where a replayed event, already shown, leaves the run.
    fn replayed(&mut self, event: &CoderEvent) {
        match event {
            CoderEvent::CoderStarted(_) => {
                self.running = true;
                self.asked = false;
                self.starting = None;
            }
            event if event.ends_turn() => {
                let replaced = self
                    .next_turn
                    .is_some_and(|next| openagents_chat::client::turn_of(event) < next);
                self.running = replaced;
                self.asked = matches!(event, CoderEvent::Question(_) | CoderEvent::Approval(_));
                self.progress = None;
                self.activity = None;
            }
            _ => {}
        }
    }

    fn line(&mut self, line: CoderLine) {
        let seen = self.seen.entry(line.task.clone()).or_insert(0);
        if line.seq != 0 && line.seq <= *seen {
            // A follow that starts again replays the task from its first
            // event. Its rows are already here, but where the run stands
            // still comes from them: a replay of a turn that ended must
            // not leave the screen working with nothing coming.
            self.replayed(&line.event);
            let ended = self.finished(&line.event);
            self.track(&line, false, ended);
            return;
        }
        *seen = line.seq.max(*seen);
        self.task = Some(line.task.clone());
        let ended = self.finished(&line.event);
        self.track(&line, true, ended);
        match &line.event {
            CoderEvent::Status(status) => {
                self.running = true;
                self.doing(status.text.clone());
                return;
            }
            CoderEvent::Step(step) => match step.kind {
                coder_events::StepKind::Command | coder_events::StepKind::ToolCall => {
                    self.doing("Running…");
                }
                coder_events::StepKind::Message => {}
                _ => self.doing("Working…"),
            },
            CoderEvent::Output(_) => self.doing("Working…"),
            _ => {}
        }
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
                // The run is launched; its engine has not said anything yet.
                self.doing(coder_events::starting(&started.provider));
                self.engine = Some(rows::provider(&started.provider));
                self.worktree = Some(started.worktree.clone());
                if self.next_turn.is_some_and(|next| started.turn >= next) {
                    self.next_turn = None;
                }
                // One short line (#10115), and who runs only when that is
                // news: another engine than asked, or one passed over.
                self.push_both(Row::Note(started.line(), Intensity::ThreeQuarters));
                if let Some(news) = started.news() {
                    self.push_both(Row::note(news));
                }
            }
            CoderEvent::Question(_) | CoderEvent::Approval(_) => self.asked = true,
            CoderEvent::Step(step)
                if step.kind == coder_events::StepKind::Message && self.steering > 0 =>
            {
                self.steering -= 1;
                self.push_both(Row::note("Coder read your message."));
            }
            _ => {}
        }
        if line.event.ends_turn() {
            // A turn that a message replaced ends; the run goes on with
            // the next one.
            let replaced = self
                .next_turn
                .is_some_and(|next| openagents_chat::client::turn_of(&line.event) < next);
            self.running = replaced;
            self.progress = None;
            self.activity = None;
            for log in [&mut self.transcript, &mut self.run_log] {
                if let Some(Row::Tools { stretch, .. }) = log.last_mut() {
                    stretch.settle();
                }
            }
        }
        // A command, a tool call, a thought, or what one returned joins the
        // open stretch, or starts one (#10117). A reply draws no row here,
        // so it never splits one. The run view shows every call with its
        // output.
        if grow(&mut self.transcript, &line, self.expanded) {
            self.scroll = 0;
        }
        if grow(&mut self.run_log, &line, true)
            && let Some(view) = &mut self.run_view
        {
            view.scroll = 0;
        }
    }

    /// Whether `event` ends its run's turn for good: not a turn a message
    /// replaced, and not a question that waits for an answer.
    fn finished(&self, event: &CoderEvent) -> bool {
        event.ends_turn()
            && !matches!(event, CoderEvent::Question(_) | CoderEvent::Approval(_))
            && !self
                .next_turn
                .is_some_and(|next| openagents_chat::client::turn_of(event) < next)
    }

    /// Open the run view, or close it.
    pub fn toggle_run(&mut self) {
        if self.run_view.take().is_some() {
            return;
        }
        if self.task.is_none() {
            self.note("This thread has no Coder run yet.");
            return;
        }
        self.run_view = Some(RunView::default());
    }

    /// Scroll the run view back (`rows` over 0) or forward.
    fn scroll_view(&mut self, rows: isize) {
        if let Some(view) = &mut self.run_view {
            view.scroll = view.scroll.saturating_add_signed(rows);
        }
    }

    /// A key while a file is open: scroll it, or close it.
    fn file_key(&mut self, key: &KeyEvent) {
        let height = self.shown.rows.len().max(1);
        let Some(file) = &mut self.file else {
            return;
        };
        let page = isize::try_from(height.saturating_sub(1).max(1)).unwrap_or(1);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.file = None,
            KeyCode::Up => file.scroll(-1, height),
            KeyCode::Down => file.scroll(1, height),
            KeyCode::PageUp => file.scroll(-page, height),
            KeyCode::PageDown | KeyCode::Char(' ') => file.scroll(page, height),
            KeyCode::Home => file.top = 0,
            KeyCode::End => file.scroll(isize::MAX, height),
            _ => {}
        }
    }

    /// A file the screen opened: it fills the screen until Esc.
    pub fn show_file(&mut self, file: FileView) {
        self.selection = None;
        self.file = Some(file);
    }

    /// Where the paths a reply or a run names are: the run's worktree
    /// first, then the folder the screen runs in.
    pub fn bases(&self, folder: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
        self.worktree
            .iter()
            .map(std::path::PathBuf::from)
            .chain(folder.map(std::path::Path::to_path_buf))
            .collect()
    }

    /// One mouse event: the wheel scrolls; a drag selects and letting go
    /// copies; a click on a file's path opens it.
    pub fn mouse(&mut self, mouse: &MouseEvent) -> Vec<Action> {
        if self.overlay.is_some() {
            return Vec::new();
        }
        let at = (mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let rows: isize = if mouse.kind == MouseEventKind::ScrollUp {
                    3
                } else {
                    -3
                };
                let height = self.shown.rows.len();
                if let Some(file) = &mut self.file {
                    file.scroll(-rows, height);
                } else if self.run_view.is_some() {
                    self.scroll_view(rows);
                } else {
                    self.scroll = self.scroll.saturating_add_signed(rows);
                }
                Vec::new()
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.selection = self.shown.contains(at.0, at.1).then_some(Selection {
                    anchor: at,
                    head: at,
                });
                Vec::new()
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(selection) = &mut self.selection {
                    let area = self.shown.area;
                    selection.head = (
                        at.0.clamp(area.left(), area.right().saturating_sub(1)),
                        at.1.clamp(area.top(), area.bottom().saturating_sub(1)),
                    );
                }
                Vec::new()
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let Some(selection) = self.selection else {
                    return Vec::new();
                };
                if selection.is_click() {
                    self.selection = None;
                    return self
                        .shown
                        .word(at.0, at.1)
                        .and_then(|word| crate::view::reference(&word))
                        .map(|(path, line)| Action::OpenFile { path, line })
                        .into_iter()
                        .collect();
                }
                let text = self.shown.text(&selection);
                if text.trim().is_empty() {
                    self.selection = None;
                    return Vec::new();
                }
                vec![Action::Copy(text)]
            }
            _ => Vec::new(),
        }
    }

    /// A message reached the run: at its next step, or as the turn it
    /// starts.
    pub fn steered(&mut self, result: Result<openagents_chat::client::Steering, String>) {
        use openagents_chat::client::Steering;
        match result {
            Ok(Steering::NextStep) => {
                self.push_both(Row::note("Sent. Coder reads it at its next step."))
            }
            Ok(Steering::NextTurn(turn)) => {
                self.push_both(Row::note("Sent. Coder starts its next turn with it."));
                self.next_turn = Some(turn);
                self.running = true;
            }
            Err(why) => {
                self.steering = self.steering.saturating_sub(1);
                self.push_both(Row::loud(why));
            }
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
                // The client runs or offers the reply's command itself
                // (`Event::Command`).
                Offer::Cli { argv, .. } if meta.command.is_none() => {
                    self.note(format!("Run: {}", Offer::command_line(argv)));
                }
                Offer::Cli { .. } => {}
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
            _ if self.file.is_some() => "Esc closes the file".into(),
            _ if self.viewed().is_some() && !self.asked => {
                let number = self.run_view.and_then(|view| view.number).unwrap_or(1);
                let held = &self.delegations[number - 1];
                let state = if held.running { "working" } else { "ended" };
                format!(
                    "Coder run {number} · {} · {state} · Enter sends it your message · Esc back",
                    held.agent
                )
            }
            _ if self.rail.is_some() => {
                "Enter opens the run full screen · Up and Down move · Esc back".into()
            }
            _ if self.run_view.is_some() && !self.asked => {
                let state = if self.running { "working" } else { "ended" };
                format!("Coder run · {state} · Enter sends it your message · Esc back")
            }
            _ if self.plugin.is_some() => format!(
                "plugin {} · type what to ask · Esc cancels",
                self.plugin
                    .as_ref()
                    .map_or("", |plugin| plugin.name.as_str())
            ),
            Phase::Replying => match self.offline {
                Some(_) => "reconnecting · Esc stops".into(),
                None => "replying · Esc stops".into(),
            },
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
                _ => "Working · Esc stops".into(),
            },
            _ if self.command.is_some() => "Enter runs the command · Esc cancels".into(),
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

/// Adds a run event to `log`: it joins the open stretch of tool calls,
/// starts one, or draws its own row. Whether `log` changed.
pub(crate) fn grow(
    log: &mut Scrollback<Row, Line<'static>, Wrap>,
    line: &CoderLine,
    expanded: bool,
) -> bool {
    if let Some(Row::Tools { stretch, .. }) = log.last_mut()
        && stretch.push(line.seq, &line.event)
    {
        return true;
    }
    let mut stretch = Stretch::default();
    if stretch.push(line.seq, &line.event) {
        log.push(Row::Tools { stretch, expanded });
        return true;
    }
    let Some(mut row) = rows::run_row(&line.event) else {
        return false;
    };
    if let RunRow::Result { expanded: open, .. } = &mut row {
        *open = expanded;
    }
    log.push(Row::Run(row));
    true
}

pub(crate) fn transcript(ladder: Ladder) -> Scrollback<Row, Line<'static>, Wrap> {
    let wrap: Wrap = Box::new(move |row: &Row, width: usize| {
        rows::lines(row, u16::try_from(width).unwrap_or(u16::MAX), ladder)
    });
    Scrollback::new(wrap)
}

/// The welcome card: the version, then three short facts, as the old Coder
/// Terminal's was. No prose and no key legend: `/help` lists the keys.
pub fn welcome(_backend: Kind, context: &Context, _resumed: Option<&str>) -> Card {
    let project = context.project.as_ref().map_or_else(
        || "none".to_owned(),
        |project| {
            project
                .path
                .as_deref()
                .map_or_else(|| project.name.clone(), home_relative)
        },
    );
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
    Card {
        title: title(),
        rows: vec![("Project".into(), project), ("Agents".into(), agents)],
        body: Vec::new(),
        art: Vec::new(),
        keys: Vec::new(),
    }
}

/// What an unknown `/word` shows: the commands that start as it does, or
/// all of them when none do.
pub fn commands(word: &str) -> Card {
    let typed = word.trim_start_matches('/');
    let mut matching: Vec<Slash> = Vec::new();
    for end in (1..=typed.len()).rev() {
        let Some(prefix) = typed.get(..end) else {
            continue;
        };
        matching = Slash::ALL
            .into_iter()
            .filter(|slash| slash.word().starts_with(prefix))
            .collect();
        if !matching.is_empty() {
            break;
        }
    }
    if matching.is_empty() {
        matching = Slash::ALL.to_vec();
    }
    Card {
        title: "Commands".into(),
        rows: matching
            .into_iter()
            .map(|slash| (slash.usage(), slash.about().to_owned()))
            .collect(),
        body: Vec::new(),
        art: Vec::new(),
        keys: Vec::new(),
    }
}

/// The `/help` card.
pub fn help() -> Card {
    let mut rows: Vec<(String, String)> = Slash::ALL
        .iter()
        .map(|slash| (slash.usage(), slash.about().to_owned()))
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
        ("Ctrl+N".to_owned(), "new thread".to_owned()),
        ("Ctrl+T".to_owned(), "resume a thread".to_owned()),
        (
            "Ctrl+R".to_owned(),
            "the Coder run full screen; what you type there goes to the run".to_owned(),
        ),
        (
            "Up".to_owned(),
            "on an empty line, into the rail of Coder runs; Down back, Enter opens one".to_owned(),
        ),
        (
            "Alt+1…9".to_owned(),
            "open that Coder run full screen".to_owned(),
        ),
        (
            "Mouse".to_owned(),
            "drag to select and copy; click a file's path to read the file".to_owned(),
        ),
        (
            "Ctrl+Y".to_owned(),
            "copy the last reply; again, each code block in it".to_owned(),
        ),
        (
            "Ctrl+O".to_owned(),
            "expand or condense tool calls and a run's changes".to_owned(),
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

/// The welcome card's title: the version for a published release, which
/// `scripts/release/terminal.sh` builds with `OPENAGENTS_RELEASE=1`, and
/// "dev build" for any other build, so a local build never passes for the
/// released version (owner, 2026-10-02).
fn title() -> String {
    match option_env!("OPENAGENTS_RELEASE") {
        Some("1") => format!("OpenAgents v{}", env!("CARGO_PKG_VERSION")),
        _ => "OpenAgents dev build".to_owned(),
    }
}

/// `path` with the home folder written `~`, as a shell prompt writes it:
/// `/home/me/openagents` is `~/openagents`.
pub fn home_relative(path: &str) -> String {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let home = home.trim_end_matches(['/', '\\']);
    match path.strip_prefix(home) {
        Some("") if !home.is_empty() => "~".to_owned(),
        Some(rest) if !home.is_empty() && rest.starts_with(['/', '\\']) => format!("~{rest}"),
        _ => path.to_owned(),
    }
}
