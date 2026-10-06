//! The default view: one fixed sheet, laid out like a page of paper
//! (`docs/terminal/design-principles.md`). Everything is anchored to the
//! frame: a status area at the top that is always visible, the transcript
//! with its scroll bar, one input line, and a strip of labeled keys. Nothing
//! floats, slides, or blinks, and every character on it is ASCII.
//!
//! The input line belongs to the terminal. At Enter it decides on this
//! computer whether the line is a shell command or a question
//! ([`crate::route`]); a command goes to the shell running underneath, and
//! a question goes to OpenAgents. Shell output and answers interleave in the
//! transcript as plain blocks. While a command runs, keys go to it.

use crate::KeyIn;
use crate::application::Application;
use crate::ascii::{ascii, plain};
use crate::bridge::Request;
use crate::context::Context;
use crate::input::{KeyCode, Logical, NamedKey};
use crate::layout::PaneId;
use crate::route::{Route, Table};
use crate::smart::{Worker, id, scrub};
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::Instant;

/// The key strip, always shown on the sheet's last row. It fits the
/// sheet's 120 columns; PGUP and PGDN scroll beside the scroll bar.
/// The key strip on the rules page.
pub const RULE_KEYS: &str = "F1 HELP  F10 QUIT  F11 RETURN  UP DOWN PICK  ENTER PAUSE OR RESUME, THEN CONFIRM  ESC REJECT OR RETURN";

/// The key strip on the Gym page.
pub const GYM_KEYS: &str = "F1 HELP  F2 RESULTS OR PLUGINS  F10 QUIT  F12 RETURN  UP DOWN PICK  ENTER OPEN  ESC RETURN  PGUP PGDN SCROLL";

/// The key strip on the files page.
pub const FILE_KEYS: &str =
    "F1 HELP  F2 RETURN  F10 QUIT  UP DOWN PICK  ENTER OPEN  ESC RETURN  PGUP PGDN SCROLL";

/// The key strip on the run page.
pub const RUN_KEYS: &str = "F1 HELP  F2 FILES  F7 CANCEL RUN  F9 RETURN  F10 QUIT  ENTER STEER OR CONFIRM  ESC REJECT OR RETURN  PGUP PGDN SCROLL";

pub const KEYS: &str = "F1 HELP F2 CONTEXT F3 COPY F4 THREAD F5 SHELL F6 ASK F7 FIX F8 PANES F9 RUN F10 QUIT F11 RULES ENTER CONFIRM ESC REJECT";

const HELP: &[&str] = &[
    "HELP (F1 or ESC returns to the transcript)",
    "",
    "Type a command or a question on the input line and press ENTER.",
    "The label before the line says what ENTER does with it, decided on",
    "this computer: SHELL runs it in your shell; ASK sends it to OpenAgents.",
    "",
    "F2   attach or detach the last failed command's output for the next question",
    "F3   copy the last command and its output",
    "F4   show the conversation your questions go to; F4 or ESC returns.",
    "     There, ENTER sends the line to that conversation",
    "F5   run the input line as a shell command, whatever its label says",
    "F6   send the input line to OpenAgents; on an empty line, ask about the",
    "     last failed command; what it already did stays done",
    "F7   after a command the shell did not find, put the closest command",
    "     on the input line, without running it; again for the next one",
    "F8   switch to panes and tabs; F8 there returns to this sheet",
    "F9   show the Coder run the conversation started; F9 or ESC returns.",
    "     There, ENTER steers it with the line and F7 cancels it; ENTER",
    "     confirms either one and ESC rejects it; F2 lists the files it changed,",
    "     and ENTER shows the one picked, checked against the run's digest",
    "F10  quit",
    "F11  show this computer's background rules; F11 or ESC returns. There,",
    "     ENTER pauses or resumes the rule picked, after ENTER confirms it",
    "F12  show the plugin test results under this directory; F12 or ESC returns.",
    "     There, ENTER recomputes the one picked from its retained attempts and",
    "     shows whether they agree with its report. It runs and publishes nothing.",
    "     F2 there lists the installed plugins by exact release; ENTER opens the",
    "     newest result for exactly that release.",
    "     (The key strip is full, so F12 is named here.)",
    "",
    "A proposed command waits in the transcript. ENTER on an empty input line",
    "confirms it and runs it in your shell; ESC rejects it.",
    "While a command runs, keys go to it; CTRL+C interrupts it.",
    "PGUP and PGDN scroll the transcript. UP and DOWN recall earlier lines.",
];

/// How loud a span is: the four whites of the ladder, and reversed text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Loud,
    Present,
    Quiet,
    Receded,
    /// Dark text on a lit cell, for a program's reverse video.
    Reversed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub tone: Tone,
}

/// The whole sheet as rows of ASCII spans, and where the caret is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sheet {
    pub rows: Vec<Vec<Span>>,
    pub caret: Option<(usize, usize)>,
}

impl Sheet {
    /// One row's text.
    #[must_use]
    pub fn row_text(&self, row: usize) -> String {
        self.rows
            .get(row)
            .map(|spans| spans.iter().map(|span| span.text.as_str()).collect())
            .unwrap_or_default()
    }
    /// All rows' text, one per line.
    #[must_use]
    pub fn text(&self) -> String {
        (0..self.rows.len())
            .map(|row| self.row_text(row))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pending,
    Confirmed,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A shell command and its output, by block id.
    Block(u64),
    Ask {
        text: String,
        attached: Option<String>,
    },
    Answer(String),
    Proposal {
        key: String,
        command: String,
        verdict: Verdict,
    },
    Note(String),
}

/// One transcript line before wrapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub tone: Tone,
}

fn line(text: impl AsRef<str>, tone: Tone) -> Line {
    Line {
        text: ascii(text.as_ref()),
        tone,
    }
}

pub struct Paper {
    /// The sheet is the view; off, the panes and tabs are.
    pub on: bool,
    pub input: String,
    /// The caret, in characters from the input's start.
    pub cursor: usize,
    pub history: Vec<String>,
    browsing: Option<usize>,
    pub entries: Vec<Entry>,
    /// Bumped whenever finished transcript content changes.
    revision: u64,
    seen_block: u64,
    finished: usize,
    /// Transcript lines scrolled back from the bottom.
    pub scroll: usize,
    pub help: bool,
    /// The last failed command's output goes with the next question.
    pub attach: bool,
    attached_block: Option<u64>,
    pub queue: VecDeque<String>,
    pub door: Option<String>,
    pub git: Option<String>,
    git_rx: Option<Receiver<(PaneId, String, String)>>,
    git_for: Option<(usize, String)>,
    pub table: Table,
    table_raw: Option<String>,
    /// The program grid the transcript region holds: rows and columns.
    pub grid: (u16, u16),
    /// When the last key changed the input line, until a frame shows it.
    pub typed_at: Option<Instant>,
    /// Key-to-presented-frame times, in milliseconds.
    pub latencies: Vec<f64>,
    /// F10 asked to quit; the mount decides what that means.
    pub quit: bool,
    /// The proposal whose first CONFIRM warned that it may change things;
    /// the next CONFIRM runs it.
    pub warned: Option<String>,
    /// The conversation page (F4), drawn in place of the transcript.
    pub thread: crate::thread::Page,
    /// The run page (F9), drawn in place of the transcript.
    pub run: crate::run::Page,
    /// The run's files page (F2 on the run page).
    pub files: crate::files::Page,
    /// The background rules page (F11).
    pub rules: crate::rules::Page,
    /// The Gym page (F12): retained plugin evaluations.
    pub gym: crate::gym::Page,
    cache: Option<(u64, usize, usize, Vec<Line>)>,
}

impl Default for Paper {
    fn default() -> Self {
        Self {
            on: false,
            input: String::new(),
            cursor: 0,
            history: Vec::new(),
            browsing: None,
            entries: Vec::new(),
            revision: 0,
            seen_block: 0,
            finished: 0,
            scroll: 0,
            help: false,
            attach: true,
            attached_block: None,
            queue: VecDeque::new(),
            door: None,
            git: None,
            git_rx: None,
            git_for: None,
            table: Table::default(),
            table_raw: None,
            grid: (24, 80),
            typed_at: None,
            latencies: Vec::new(),
            quit: false,
            warned: None,
            thread: crate::thread::Page::default(),
            run: crate::run::Page::default(),
            files: crate::files::Page::default(),
            rules: crate::rules::Page::default(),
            gym: crate::gym::Page::default(),
            cache: None,
        }
    }
}

impl Paper {
    fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
        self.revision += 1;
        self.scroll = 0;
    }
    fn insert(&mut self, text: &str) {
        let text = ascii(text).replace('\n', " ");
        let at = self
            .input
            .char_indices()
            .nth(self.cursor)
            .map_or(self.input.len(), |(at, _)| at);
        self.input.insert_str(at, &text);
        self.cursor += text.chars().count();
        if self.input.len() > 8192 {
            self.input.truncate(8192);
            self.cursor = self.cursor.min(self.input.chars().count());
        }
        self.typed_at = Some(Instant::now());
    }
    fn set_input(&mut self, text: String) {
        self.cursor = text.chars().count();
        self.input = text;
        self.typed_at = Some(Instant::now());
    }
    /// Records the time from the last edit to a presented frame.
    pub fn presented(&mut self) {
        if let Some(at) = self.typed_at.take()
            && self.latencies.len() < 100_000
        {
            self.latencies.push(at.elapsed().as_secs_f64() * 1000.0);
        }
    }
}

impl Application {
    /// The pane the sheet shows: the focused one.
    fn paper_pane(&self) -> Option<PaneId> {
        self.focus_id()
    }

    /// Whether keys go to a running program instead of the input line.
    #[must_use]
    pub fn paper_running(&self) -> bool {
        self.paper_pane()
            .and_then(|id| self.panes.get(&id))
            .is_some_and(|pane| {
                !pane.ended
                    && (!pane.session.blocks.at_prompt || pane.session.vt.alternate_screen())
            })
    }

    /// What ENTER does with the input line now.
    #[must_use]
    pub fn paper_route(&self) -> Option<Route> {
        let line = self.paper.input.trim();
        (!line.is_empty()).then(|| self.paper.table.classify(line).route)
    }

    /// Handles a key on the sheet. Returns whether the sheet took it.
    pub fn paper_key(&mut self, key: &KeyIn) -> bool {
        let named = match &key.logical {
            Logical::Named(named) => Some(*named),
            _ => None,
        };
        match named {
            Some(NamedKey::F1) => {
                self.paper.help = !self.paper.help;
                return true;
            }
            Some(NamedKey::F12) => {
                self.paper_gym_toggle();
                return true;
            }
            Some(NamedKey::F11) => {
                self.paper.gym.open = false;
                let page = &mut self.paper.rules;
                page.open = !page.open;
                if page.open {
                    self.paper.help = false;
                    page.dirty = true;
                    page.change = None;
                    self.paper_rules_poll();
                }
                return true;
            }
            Some(NamedKey::F2) if self.paper.gym.open => {
                self.paper_gym_components();
                return true;
            }
            Some(NamedKey::F2) if self.paper.files.open => {
                self.paper.files.open = false;
                return true;
            }
            Some(NamedKey::F2) if self.paper.run.open => {
                self.paper_files_open();
                return true;
            }
            Some(NamedKey::F2) => {
                self.paper.attach = !self.paper.attach;
                return true;
            }
            Some(NamedKey::F3) => {
                self.copy_block();
                self.notice = Some("Copied the last command and its output.".into());
                return true;
            }
            Some(NamedKey::F4) => {
                self.paper_thread_toggle();
                return true;
            }
            Some(NamedKey::F9) => {
                self.paper_run_toggle();
                return true;
            }
            Some(NamedKey::F7) if self.paper.run.open => {
                self.paper_run_cancel();
                return true;
            }
            Some(NamedKey::F5) => {
                let line = self.paper.input.trim().to_owned();
                if !line.is_empty() && !self.paper_running() {
                    self.paper_take_line();
                    self.paper_shell(&line);
                }
                return true;
            }
            Some(NamedKey::F6) => {
                let line = self.paper.input.trim().to_owned();
                if line.is_empty() {
                    if let Some(command) = self
                        .last_block()
                        .filter(|b| b.status.is_some_and(|s| s != 0))
                        .map(|b| b.command.clone())
                    {
                        self.paper_ask(&format!("Why did `{command}` fail?"));
                    }
                } else {
                    self.paper_take_line();
                    self.paper_ask(&line);
                }
                return true;
            }
            Some(NamedKey::F7) => {
                // An empty line, or one F7 filled, takes the next choice;
                // a line the person typed stays theirs.
                let filled =
                    self.smart.correction.as_ref().is_some_and(|correction| {
                        correction.choices.lines.contains(&self.paper.input)
                    });
                if self.paper.input.trim().is_empty() || filled {
                    match self.next_correction() {
                        Some(line) => self.paper.set_input(line),
                        None => {
                            self.notice =
                                Some("No correction is offered for the last command.".into());
                        }
                    }
                }
                return true;
            }
            Some(NamedKey::F8) => {
                self.paper.on = false;
                return true;
            }
            Some(NamedKey::F10) => {
                self.paper.quit = true;
                return true;
            }
            Some(NamedKey::PageUp | NamedKey::PageDown) if self.paper.gym.open => {
                let half = self.paper.grid.0 as usize / 2;
                let page = &mut self.paper.gym;
                page.scroll = if named == Some(NamedKey::PageUp) {
                    page.scroll.saturating_sub(half)
                } else {
                    page.scroll.saturating_add(half)
                };
                return true;
            }
            Some(NamedKey::PageUp | NamedKey::PageDown) if self.paper.rules.open => {
                let half = self.paper.grid.0 as usize / 2;
                let page = &mut self.paper.rules;
                page.scroll = if named == Some(NamedKey::PageUp) {
                    page.scroll.saturating_sub(half)
                } else {
                    page.scroll.saturating_add(half)
                };
                return true;
            }
            Some(NamedKey::PageUp) if self.paper.files.open => {
                let half = self.paper.grid.0 as usize / 2;
                self.paper.files.scroll = self.paper.files.scroll.saturating_sub(half);
                return true;
            }
            Some(NamedKey::PageDown) if self.paper.files.open => {
                let half = self.paper.grid.0 as usize / 2;
                self.paper.files.scroll = self.paper.files.scroll.saturating_add(half);
                return true;
            }
            Some(NamedKey::PageUp) => {
                let half = self.paper.grid.0 as usize / 2;
                let scroll = if self.paper.run.open {
                    &mut self.paper.run.scroll
                } else if self.paper.thread.open {
                    &mut self.paper.thread.scroll
                } else {
                    &mut self.paper.scroll
                };
                *scroll = scroll.saturating_add(half);
                return true;
            }
            Some(NamedKey::PageDown) => {
                let half = self.paper.grid.0 as usize / 2;
                let scroll = if self.paper.run.open {
                    &mut self.paper.run.scroll
                } else if self.paper.thread.open {
                    &mut self.paper.thread.scroll
                } else {
                    &mut self.paper.scroll
                };
                *scroll = scroll.saturating_sub(half);
                return true;
            }
            _ => {}
        }
        if self.paper.help {
            if key.code == KeyCode::Escape {
                self.paper.help = false;
            }
            return true;
        }
        let enter = matches!(key.code, KeyCode::Enter | KeyCode::NumpadEnter);
        // CONFIRM and REJECT act on a pending proposal from an empty line.
        if let Some((_, proposal)) = self.smart.pending.clone() {
            if key.code == KeyCode::Escape {
                self.smart.pending = None;
                self.paper_verdict(&proposal, Verdict::Rejected);
                return true;
            }
            if enter && self.paper.input.trim().is_empty() {
                self.smart_key(key);
                if self
                    .smart
                    .execution
                    .as_ref()
                    .is_some_and(|(_, executing, _, _)| *executing == proposal)
                {
                    self.paper_verdict(&proposal, Verdict::Confirmed);
                    self.paper.warned = None;
                } else if self.smart.pending.is_some() {
                    self.paper.warned = Some(proposal);
                }
                return true;
            }
        }
        if self.paper_running() {
            if let Some(bytes) = self.encode(key) {
                self.send(&bytes);
            }
            return true;
        }
        if self.paper.gym.open {
            self.paper_gym_key(key.code, enter);
            return true;
        }
        if self.paper.rules.open {
            self.paper_rules_key(key.code, enter);
            return true;
        }
        if self.paper.files.open {
            self.paper_files_key(key.code, enter);
            return true;
        }
        // On the run page, CONFIRM sends an armed command, and REJECT
        // drops it or returns.
        if self.paper.run.open {
            let armed = self
                .paper
                .run
                .command
                .as_ref()
                .is_some_and(|command| command.state == crate::run::Sent::Armed);
            if key.code == KeyCode::Escape {
                if armed {
                    self.paper.run.command = None;
                } else {
                    self.paper.run.open = false;
                }
                return true;
            }
            if enter {
                let line = self.paper.input.trim().to_owned();
                if armed || line.is_empty() {
                    self.paper_run_send();
                } else {
                    self.paper_take_line();
                    self.paper_run_steer(&line);
                }
                return true;
            }
        }
        // REJECT on the thread page returns to the transcript.
        if self.paper.thread.open && key.code == KeyCode::Escape {
            self.paper.thread.open = false;
            return true;
        }
        let ctrl = self.mods.control_key();
        match (key.code, named) {
            (KeyCode::Enter | KeyCode::NumpadEnter, _) => self.paper_submit(),
            (KeyCode::Escape, _) => self.paper.set_input(String::new()),
            (KeyCode::Backspace, _) | (_, Some(NamedKey::Backspace)) => {
                if self.paper.cursor > 0 {
                    self.paper.cursor -= 1;
                    let at = self
                        .paper
                        .input
                        .char_indices()
                        .nth(self.paper.cursor)
                        .map(|(at, _)| at);
                    if let Some(at) = at {
                        self.paper.input.remove(at);
                    }
                    self.paper.typed_at = Some(Instant::now());
                }
            }
            (_, Some(NamedKey::Delete)) => {
                let at = self
                    .paper
                    .input
                    .char_indices()
                    .nth(self.paper.cursor)
                    .map(|(at, _)| at);
                if let Some(at) = at {
                    self.paper.input.remove(at);
                    self.paper.typed_at = Some(Instant::now());
                }
            }
            (KeyCode::ArrowLeft, _) => self.paper.cursor = self.paper.cursor.saturating_sub(1),
            (KeyCode::ArrowRight, _) => {
                self.paper.cursor = (self.paper.cursor + 1).min(self.paper.input.chars().count());
            }
            (_, Some(NamedKey::Home)) => self.paper.cursor = 0,
            (_, Some(NamedKey::End)) => self.paper.cursor = self.paper.input.chars().count(),
            (KeyCode::KeyA, _) if ctrl => self.paper.cursor = 0,
            (KeyCode::KeyE, _) if ctrl => self.paper.cursor = self.paper.input.chars().count(),
            (KeyCode::KeyU | KeyCode::KeyC, _) if ctrl => self.paper.set_input(String::new()),
            (KeyCode::KeyK, _) if ctrl => {
                let keep: String = self.paper.input.chars().take(self.paper.cursor).collect();
                self.paper.input = keep;
            }
            (KeyCode::ArrowUp, _) => self.paper_history(true),
            (KeyCode::ArrowDown, _) => self.paper_history(false),
            _ if ctrl || self.mods.super_key() => {}
            _ => {
                if let Some(text) = &key.text {
                    let text: String = text.chars().filter(|c| !c.is_control()).collect();
                    if !text.is_empty() {
                        self.paper.insert(&text);
                    }
                }
            }
        }
        true
    }

    /// A paste: into the input line, or to the running program.
    pub fn paper_paste(&mut self, text: &str) {
        if self.paper_running() {
            let bytes = match self.focused_pane() {
                Some(pane) => pane.session.vt.paste(text),
                None => return,
            };
            self.send(&bytes);
        } else {
            self.paper.insert(text);
        }
    }

    fn paper_history(&mut self, back: bool) {
        let history = &self.paper.history;
        if history.is_empty() {
            return;
        }
        let at = match (self.paper.browsing, back) {
            (None, true) => Some(history.len() - 1),
            (None, false) => None,
            (Some(at), true) => Some(at.saturating_sub(1)),
            (Some(at), false) if at + 1 < history.len() => Some(at + 1),
            (Some(_), false) => None,
        };
        self.paper.browsing = at;
        let text = at.map(|at| history[at].clone()).unwrap_or_default();
        self.paper.set_input(text);
    }

    fn paper_take_line(&mut self) -> String {
        let line = std::mem::take(&mut self.paper.input).trim().to_owned();
        self.paper.cursor = 0;
        self.paper.browsing = None;
        if !line.is_empty() && self.paper.history.last() != Some(&line) {
            self.paper.history.push(line.clone());
        }
        self.paper.typed_at = Some(Instant::now());
        line
    }

    fn paper_submit(&mut self) {
        let route = self.paper_route();
        let line = self.paper_take_line();
        if line.is_empty() {
            return;
        }
        // On the thread page, a line is a message to that thread.
        if self.paper.thread.open {
            self.paper_ask(&line);
            return;
        }
        match route {
            Some(Route::Ask) => self.paper_ask(&line),
            _ => self.paper_shell(&line),
        }
    }

    /// F9: shows the Coder run the conversation started, or returns.
    fn paper_run_toggle(&mut self) {
        self.paper.files.open = false;
        self.paper.rules.open = false;
        self.paper.gym.open = false;
        if self.paper.run.open {
            self.paper.run.open = false;
            return;
        }
        if let Some(link) = self.paper.thread.run().cloned() {
            self.paper.help = false;
            self.paper.run.show(&link.task, &link.host);
            self.paper_run_poll();
            return;
        }
        let thread = self
            .paper_pane()
            .and_then(|pane| self.smart.threads.get(&pane).cloned());
        match thread {
            // The conversation's run is in the thread; read it first.
            Some(thread) if !self.paper.thread.open || self.paper.thread.shown.is_none() => {
                self.paper.help = false;
                self.paper.thread.show(&thread);
                self.paper_thread_read();
                self.notice = Some("Reading the conversation; F9 again shows its run.".into());
            }
            Some(_) => {
                self.notice = Some("This conversation started no Coder run.".into());
            }
            None => {
                self.notice = Some("No conversation yet; ask OpenAgents something first.".into());
            }
        }
    }

    /// A key on the rules page: the arrows pick, ENTER arms pausing or
    /// resuming the picked rule and then confirms it, and ESC rejects or
    /// returns.
    fn paper_rules_key(&mut self, code: KeyCode, enter: bool) {
        let now = unix_now();
        let page = &mut self.paper.rules;
        let armed = page
            .change
            .as_ref()
            .is_some_and(|change| change.state == crate::rules::Changed::Armed);
        let count = match &page.shown {
            Some(Ok(rules)) => rules.rules.len(),
            _ => 0,
        };
        match code {
            KeyCode::Escape if armed => page.change = None,
            KeyCode::Escape => page.open = false,
            KeyCode::ArrowUp if !armed => page.selected = page.selected.saturating_sub(1),
            KeyCode::ArrowDown if !armed => {
                page.selected = (page.selected + 1).min(count.saturating_sub(1));
            }
            _ if enter && armed => {
                if page.sending.is_some() {
                    return;
                }
                let Some(change) = page.change.as_mut() else {
                    return;
                };
                change.state = crate::rules::Changed::Sending;
                let (verb, id) = (change.verb, change.id.clone());
                let sending = self.sessions().0.rule_command(verb, &id);
                self.paper.rules.sending = Some(sending);
            }
            _ if enter => match page.picked() {
                Some(rule) if rule.error.is_some() => {
                    self.notice = Some("A rule that can't be read can't be changed here.".into());
                }
                Some(rule) => {
                    page.change = Some(crate::rules::Change {
                        id: rule.id.clone(),
                        verb: if rule.running(now) { "pause" } else { "resume" },
                        state: crate::rules::Changed::Armed,
                    });
                }
                None => {}
            },
            _ => {}
        }
    }

    /// Takes a finished read or change for the rules page, and reads the
    /// rules again when they changed.
    fn paper_rules_poll(&mut self) {
        use std::sync::mpsc::TryRecvError;
        let page = &mut self.paper.rules;
        if let Some(sending) = &page.sending {
            let answer = match sending.try_recv() {
                Ok(answer) => Some(answer),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some(Err("the host ended without an answer".into()))
                }
            };
            if let Some(answer) = answer {
                if let Some(change) = &mut page.change {
                    change.state = match answer {
                        Ok(()) => crate::rules::Changed::Done,
                        Err(why) => crate::rules::Changed::Refused(ascii(&why)),
                    };
                }
                page.sending = None;
                page.dirty = true;
            }
        }
        if let Some(reading) = &page.reading {
            match reading.try_recv() {
                Ok(read) => {
                    page.shown = Some(read);
                    page.reading = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    page.shown = Some(Err("the host ended without an answer".into()));
                    page.reading = None;
                }
            }
        }
        if page.open && page.dirty && page.reading.is_none() {
            let reading = self.sessions().0.read_rules();
            let page = &mut self.paper.rules;
            page.reading = Some(reading);
            page.dirty = false;
            page.reads += 1;
        }
    }

    /// F12: lists the plugin test results under the shell's directory, or
    /// returns. Listing only reads.
    fn paper_gym_toggle(&mut self) {
        let page = &mut self.paper.gym;
        page.open = !page.open;
        if !page.open {
            return;
        }
        self.paper.help = false;
        self.paper.rules.open = false;
        let root = self
            .paper_pane()
            .and_then(|pane| self.panes.get(&pane))
            .and_then(|pane| pane.session.blocks.cwd.clone())
            .unwrap_or_else(|| ".".into());
        let listing = self.sessions().0.read_studies(&root);
        let page = &mut self.paper.gym;
        page.root = Some(root);
        page.listed = None;
        page.listing = Some(listing);
        page.viewing = None;
        page.shown = None;
        page.reading = None;
        page.scroll = 0;
        page.reads += 1;
        page.components = false;
        page.held = None;
        page.holding = None;
    }

    /// F2 on the Gym page: the installed plugins by exact release, or back
    /// to the results. Reading them only reads.
    fn paper_gym_components(&mut self) {
        let page = &mut self.paper.gym;
        if page.viewing.is_some() {
            return;
        }
        page.components = !page.components;
        page.scroll = 0;
        if !page.components {
            return;
        }
        let root = page.root.clone().unwrap_or_else(|| ".".into());
        let holding = self.sessions().0.read_components(&root);
        let page = &mut self.paper.gym;
        page.held = None;
        page.holding = Some(holding);
        page.reads += 1;
    }

    /// A key on the Gym page: the arrows pick or scroll, ENTER recomputes
    /// the picked study (again, on its page) or opens the newest result for
    /// the picked plugin's exact release, and ESC steps back.
    fn paper_gym_key(&mut self, code: KeyCode, enter: bool) {
        let page = &mut self.paper.gym;
        let components = page.components && page.viewing.is_none();
        let count = if components {
            match &page.held {
                Some(Ok(held)) => held.plugins.len(),
                _ => 0,
            }
        } else {
            match &page.listed {
                Some(Ok(studies)) => studies.studies.len(),
                _ => 0,
            }
        };
        let dir = match (&page.viewing, components) {
            (Some(dir), _) => Some(dir.clone()),
            (None, true) => page
                .picked_component()
                .and_then(|component| component.exact())
                .map(|evidence| evidence.dir.clone()),
            (None, false) => page.picked().map(|study| study.dir.clone()),
        };
        match code {
            KeyCode::Escape if page.viewing.is_some() => {
                page.viewing = None;
                page.shown = None;
                page.reading = None;
                page.scroll = 0;
            }
            KeyCode::Escape if components => {
                page.components = false;
                page.scroll = 0;
            }
            KeyCode::Escape => page.open = false,
            KeyCode::ArrowUp if page.viewing.is_some() => {
                page.scroll = page.scroll.saturating_sub(1);
            }
            KeyCode::ArrowDown if page.viewing.is_some() => {
                page.scroll = page.scroll.saturating_add(1);
            }
            KeyCode::ArrowUp if components => {
                page.component = page.component.saturating_sub(1);
            }
            KeyCode::ArrowDown if components => {
                page.component = (page.component + 1).min(count.saturating_sub(1));
            }
            KeyCode::ArrowUp => page.selected = page.selected.saturating_sub(1),
            KeyCode::ArrowDown => page.selected = (page.selected + 1).min(count.saturating_sub(1)),
            _ if enter => {
                let Some(dir) = dir else {
                    if components && page.picked_component().is_some() {
                        self.notice = Some(
                            "No result tested exactly this release; its test command is listed."
                                .into(),
                        );
                    }
                    return;
                };
                if page.reading.is_some() {
                    return;
                }
                let reading = self.sessions().0.read_study(&dir);
                let page = &mut self.paper.gym;
                if page.viewing.as_deref() != Some(dir.as_str()) {
                    page.scroll = 0;
                }
                page.viewing = Some(dir);
                page.shown = None;
                page.reading = Some(reading);
                page.reads += 1;
            }
            _ => {}
        }
    }

    /// Takes a finished listing, plugin read, or study for the Gym page.
    fn paper_gym_poll(&mut self) {
        use std::sync::mpsc::TryRecvError;
        let page = &mut self.paper.gym;
        if let Some(holding) = &page.holding {
            match holding.try_recv() {
                Ok(read) => {
                    page.held = Some(read);
                    page.holding = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    page.held = Some(Err("the reader ended without an answer".into()));
                    page.holding = None;
                }
            }
        }
        if let Some(listing) = &page.listing {
            match listing.try_recv() {
                Ok(read) => {
                    page.listed = Some(read);
                    page.listing = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    page.listed = Some(Err("the reader ended without an answer".into()));
                    page.listing = None;
                }
            }
        }
        if let Some(reading) = &page.reading {
            match reading.try_recv() {
                Ok(read) => {
                    page.shown = Some(read);
                    page.reading = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    page.shown = Some(Err("the reader ended without an answer".into()));
                    page.reading = None;
                }
            }
        }
    }

    /// F2 on the run page: lists the files the run changed, from its own
    /// manifest.
    fn paper_files_open(&mut self) {
        let run = match (&self.paper.run.task, &self.paper.run.shown) {
            (Some((task, host)), Some(Ok(run))) if host == "local" => Some((task.clone(), run)),
            _ => None,
        };
        let Some((task, run)) = run else {
            self.notice = Some("The run's files show once the run is read here.".into());
            return;
        };
        let manifest = run
            .artifacts
            .as_ref()
            .and_then(crate::files::Manifest::from_view);
        self.paper.files.show(&task, manifest);
    }

    /// A key on the files page: the arrows pick or scroll, ENTER opens the
    /// picked file or reads it again, and ESC steps back.
    fn paper_files_key(&mut self, code: KeyCode, enter: bool) {
        let page = &mut self.paper.files;
        let count = page.manifest.as_ref().map_or(0, |m| m.changes.len());
        match code {
            KeyCode::Escape if page.viewing.is_some() => {
                page.viewing = None;
                page.shown = None;
                page.reading = None;
                page.scroll = 0;
            }
            KeyCode::Escape => page.open = false,
            KeyCode::ArrowUp if page.viewing.is_some() => {
                page.scroll = page.scroll.saturating_sub(1);
            }
            KeyCode::ArrowDown if page.viewing.is_some() => {
                page.scroll = page.scroll.saturating_add(1);
            }
            KeyCode::ArrowUp => page.selected = page.selected.saturating_sub(1),
            KeyCode::ArrowDown => {
                page.selected = (page.selected + 1).min(count.saturating_sub(1));
            }
            _ if enter => {
                let again = matches!(page.shown, Some(Err(crate::files::Unread::Unavailable(_))));
                if page.viewing.is_none() && count > 0 {
                    page.viewing = Some(page.selected);
                    page.shown = None;
                    page.scroll = 0;
                    self.paper_files_read();
                } else if again && page.reading.is_none() {
                    self.paper_files_read();
                }
            }
            _ => {}
        }
    }

    /// Reads the viewed file's retained bytes, when it has some.
    fn paper_files_read(&mut self) {
        let page = &self.paper.files;
        let (Some(task), Some(change)) = (
            page.task.clone(),
            page.viewing
                .and_then(|index| page.manifest.as_ref()?.changes.get(index).cloned()),
        ) else {
            return;
        };
        if crate::files::unreadable(&change).is_some() {
            return;
        }
        let digest = change.digest.clone().unwrap_or_default();
        let reading = self
            .sessions()
            .0
            .read_artifact(&task, &change.path, &digest);
        let page = &mut self.paper.files;
        page.shown = None;
        page.reading = Some(reading);
        page.reads += 1;
    }

    /// Takes a finished file read.
    fn paper_files_poll(&mut self) {
        let page = &mut self.paper.files;
        if let Some(reading) = &page.reading {
            match reading.try_recv() {
                Ok(read) => {
                    page.shown = Some(read);
                    page.reading = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    page.shown = Some(Err(crate::files::Unread::Unavailable(
                        "the task owner ended without an answer".into(),
                    )));
                    page.reading = None;
                }
            }
        }
    }

    /// F7 on the run page: arms cancelling the run, or sends a command
    /// whose outcome is unknown again, as the same command.
    fn paper_run_cancel(&mut self) {
        let page = &mut self.paper.run;
        if let Some(command) = &mut page.command
            && matches!(command.state, crate::run::Sent::Unknown(_))
        {
            command.state = crate::run::Sent::Armed;
            return;
        }
        if !page.controls() {
            self.notice = Some("This run offers no controls here.".into());
            return;
        }
        if let (Some((task, _)), Some(Ok(run))) = (&page.task, &page.shown) {
            page.command = Some(crate::run::Command::new(task, run.task.revision, None));
        }
    }

    /// ENTER with a line on the run page: arms steering the run with it.
    fn paper_run_steer(&mut self, line: &str) {
        let page = &mut self.paper.run;
        if !page.controls() {
            self.notice = Some("This run offers no controls here.".into());
            return;
        }
        if let (Some((task, _)), Some(Ok(run))) = (&page.task, &page.shown) {
            page.command = Some(crate::run::Command::new(
                task,
                run.task.revision,
                Some(line),
            ));
        }
    }

    /// CONFIRM on the run page: sends the armed command once, or the same
    /// command again when its outcome is unknown.
    fn paper_run_send(&mut self) {
        let Some(command) = self.paper.run.command.as_mut() else {
            return;
        };
        if !matches!(
            command.state,
            crate::run::Sent::Armed | crate::run::Sent::Unknown(_)
        ) || self.paper.run.sending.is_some()
        {
            return;
        }
        command.state = crate::run::Sent::Sending;
        let (verb, bytes) = (command.verb, command.bytes.clone());
        self.paper.run.sending = Some(self.sessions().0.task_command(verb, &bytes));
    }

    /// Takes a finished read or command answer for the run page, and reads
    /// the run again when that is due.
    fn paper_run_poll(&mut self) {
        use std::sync::mpsc::TryRecvError;
        let page = &mut self.paper.run;
        if let Some(sending) = &page.sending {
            let answer = match sending.try_recv() {
                Ok(answer) => Some(answer),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(crate::run::Sent::Unknown(
                    "the task owner ended without an answer".into(),
                )),
            };
            if let Some(answer) = answer {
                if let Some(command) = &mut page.command {
                    command.state = answer;
                }
                page.sending = None;
                page.dirty = true;
            }
        }
        if let Some(reading) = &page.reading {
            match reading.try_recv() {
                Ok(read) => {
                    page.shown = Some(read);
                    page.reading = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    page.shown = Some(Err(crate::run::Unread::Unavailable(
                        "the task owner ended without an answer".into(),
                    )));
                    page.reading = None;
                }
            }
        }
        let now = Instant::now();
        if !self.paper.run.due(now) {
            return;
        }
        let Some((task, _)) = self.paper.run.task.clone() else {
            return;
        };
        let reading = self.sessions().0.read_run(&task);
        let page = &mut self.paper.run;
        page.reading = Some(reading);
        page.dirty = false;
        page.read_at = Some(now);
        page.reads += 1;
    }

    /// F4: shows the conversation this sheet's questions go to, or returns
    /// to the transcript. Showing it only reads the thread.
    fn paper_thread_toggle(&mut self) {
        self.paper.files.open = false;
        self.paper.rules.open = false;
        self.paper.gym.open = false;
        if self.paper.thread.open {
            self.paper.thread.open = false;
            return;
        }
        let thread = self
            .paper_pane()
            .and_then(|pane| self.smart.threads.get(&pane).cloned());
        match thread {
            Some(thread) => {
                self.paper.help = false;
                self.paper.thread.show(&thread);
                self.paper_thread_read();
            }
            None => {
                self.notice = Some("No conversation yet; ask OpenAgents something first.".into());
            }
        }
    }

    /// Takes a finished read of the page's thread, and reads it again when
    /// that is due.
    fn paper_thread_read(&mut self) {
        let page = &mut self.paper.thread;
        if let Some(reading) = &page.reading {
            match reading.try_recv() {
                Ok(read) => {
                    page.shown = Some(read);
                    page.reading = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    page.shown = Some(Err(crate::thread::Unread::Unavailable(
                        "the chat client ended without an answer".into(),
                    )));
                    page.reading = None;
                }
            }
        }
        let now = Instant::now();
        if !self.paper.thread.due(now) {
            return;
        }
        let Some(thread) = self.paper.thread.thread.clone() else {
            return;
        };
        let reading = self.sessions().0.read_thread(&thread);
        let page = &mut self.paper.thread;
        page.reading = Some(reading);
        page.dirty = false;
        page.read_at = Some(now);
        page.reads += 1;
    }

    /// Runs `line` in the shell, as if typed at its prompt.
    pub fn paper_shell(&mut self, line: &str) {
        let Some(id) = self.paper_pane() else {
            return;
        };
        self.paper.scroll = 0;
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\r');
        self.send_to(id, &bytes);
    }

    /// Sends `text` to OpenAgents now, or after the request in flight.
    pub fn paper_ask(&mut self, text: &str) {
        let busy = !self.smart.workers.is_empty() || self.smart.execution.is_some();
        if busy {
            self.paper.queue.push_back(text.to_owned());
            self.paper.push(Entry::Ask {
                text: text.to_owned(),
                attached: Some("queued".into()),
            });
            return;
        }
        self.paper_start(text);
    }

    fn last_block(&self) -> Option<&crate::blocks::Block> {
        self.paper_pane()
            .and_then(|id| self.panes.get(&id))
            .and_then(|pane| pane.session.blocks.records.back())
    }

    /// The block the next question would carry.
    fn paper_attachable(&self) -> Option<&crate::blocks::Block> {
        self.last_block().filter(|block| {
            self.paper.attach
                && block.end.is_some()
                && block.status.is_some_and(|status| status != 0)
                && self.paper.attached_block != Some(block.id)
        })
    }

    fn paper_start(&mut self, text: &str) {
        let Some(pane_id) = self.paper_pane() else {
            return;
        };
        let mut context = Context::default();
        let mut attached = None;
        if let Some(pane) = self.panes.get(&pane_id)
            && let Some(binding) = pane.session.binding(String::new())
        {
            context.directory = Some(binding.cwd);
        }
        if let Some(block) = self.paper_attachable().cloned() {
            context.attach(&block, &scrub);
            attached = Some(format!(
                "block {} `{}` exit {}",
                block.id,
                ascii(&block.command),
                block.status.unwrap_or_default()
            ));
            self.paper.attached_block = Some(block.id);
        }
        // An Ask entry already exists for a queued question.
        let queued = matches!(self.paper.entries.last(), Some(Entry::Ask { text: last, attached: Some(state) }) if last == text && state == "queued");
        if queued {
            if let Some(Entry::Ask {
                attached: state, ..
            }) = self.paper.entries.last_mut()
            {
                *state = attached.clone();
            }
            self.paper.revision += 1;
        } else {
            self.paper.push(Entry::Ask {
                text: text.to_owned(),
                attached: attached.clone(),
            });
        }
        let Some(binding) = self
            .panes
            .get(&pane_id)
            .and_then(|pane| pane.session.binding(context.identity()))
        else {
            self.paper.push(Entry::Note(
                "The shell's directory is unknown, so the question was not sent.".into(),
            ));
            return;
        };
        let thread = self.smart.threads.get(&pane_id).cloned();
        let request = Request {
            thread: thread.clone().unwrap_or_else(id),
            request: id(),
            new: thread.is_none(),
            text: text.to_owned(),
            context,
            binding,
        };
        if let Err(why) = request.message() {
            self.paper.push(Entry::Note(format!("Not sent: {why}.")));
            return;
        }
        match Worker::start(self.sessions().0.as_ref(), pane_id, request) {
            Ok(worker) => self.smart.workers.push(worker),
            Err(why) => self
                .paper
                .push(Entry::Note(format!("Not sent: {}", ascii(&why)))),
        }
    }

    fn paper_verdict(&mut self, proposal: &str, verdict: Verdict) {
        for entry in self.paper.entries.iter_mut().rev() {
            if let Entry::Proposal {
                key,
                verdict: state,
                ..
            } = entry
                && key == proposal
            {
                *state = verdict;
                break;
            }
        }
        self.paper.revision += 1;
    }

    /// Follows the shell: new blocks, finished blocks, the command table,
    /// and the git summary; starts a queued question when the last ends.
    pub fn paper_tick(&mut self) {
        self.paper_thread_read();
        self.paper_run_poll();
        self.paper_files_poll();
        self.paper_rules_poll();
        self.paper_gym_poll();
        let Some(pane_id) = self.paper_pane() else {
            return;
        };
        let (ids, finished, table, cwd) = match self.panes.get(&pane_id) {
            Some(pane) => {
                let blocks = &pane.session.blocks;
                (
                    blocks
                        .records
                        .iter()
                        .map(|block| block.id)
                        .filter(|id| *id > self.paper.seen_block)
                        .collect::<Vec<_>>(),
                    blocks
                        .records
                        .iter()
                        .filter(|block| block.end.is_some())
                        .count(),
                    blocks.table.clone(),
                    blocks.cwd.clone(),
                )
            }
            None => return,
        };
        for id in ids {
            self.paper.seen_block = id;
            self.paper.push(Entry::Block(id));
        }
        if finished != self.paper.finished {
            self.paper.finished = finished;
            self.paper.revision += 1;
        }
        if table != self.paper.table_raw {
            self.paper.table = table.as_deref().map(Table::parse).unwrap_or_default();
            self.paper.table_raw = table;
        }
        if let Some(cwd) = cwd
            && self.paper.git_for.as_ref() != Some(&(finished, cwd.clone()))
        {
            self.paper.git_for = Some((finished, cwd.clone()));
            self.paper.git_rx = Some(self.sessions().0.git_summary(pane_id, cwd));
        }
        if let Some(Ok((_, _, summary))) = self.paper.git_rx.as_ref().map(Receiver::try_recv) {
            self.paper.git = Some(git_state(&summary));
            self.paper.git_rx = None;
        }
        if self.smart.workers.is_empty()
            && self.smart.execution.is_none()
            && self.smart.pending.is_none()
            && let Some(next) = self.paper.queue.pop_front()
        {
            self.paper_start(&next);
        }
    }

    /// An answer, a door, or a proposal arrived for the sheet.
    pub(crate) fn paper_message(&mut self, message: &crate::bridge::Message) {
        use crate::bridge::Message;
        match message {
            Message::Answer(text) => {
                // A typed plan is data for a proposal, never transcript text.
                let text: String = text
                    .lines()
                    .filter(|line| !line.trim_start().starts_with("{\"v\""))
                    .collect::<Vec<_>>()
                    .join("\n");
                let text = plain(&text);
                if !text.trim().is_empty() {
                    self.paper.push(Entry::Answer(text));
                }
                self.paper.thread.dirty = true;
            }
            Message::Door(door) => self.paper.door = Some(ascii(door)),
            _ => {}
        }
    }

    pub(crate) fn paper_offered(&mut self, key: &str, command: &str) {
        self.paper.push(Entry::Proposal {
            key: key.to_owned(),
            command: ascii(command),
            verdict: Verdict::Pending,
        });
    }

    pub(crate) fn paper_failed(&mut self) {
        self.paper.thread.dirty = true;
        self.paper.push(Entry::Note(
            "OpenAgents did not answer; the request ended without a reply.".into(),
        ));
    }

    /// The transcript wrapped to `width`: finished content stays cached
    /// on the sheet; what a running command prints is read live.
    fn paper_wrapped(&mut self, width: usize) -> Vec<Line> {
        let pane_id = self.paper_pane();
        let pane = pane_id.and_then(|id| self.panes.get(&id));
        let first_live = self
            .paper
            .entries
            .iter()
            .position(|entry| match entry {
                Entry::Block(id) => pane
                    .and_then(|pane| pane.session.blocks.get(*id))
                    .is_some_and(|block| block.end.is_none()),
                _ => false,
            })
            .unwrap_or(self.paper.entries.len());
        let fresh = !matches!(&self.paper.cache, Some((revision, live, cached_width, _))
            if (*revision, *live, *cached_width) == (self.paper.revision, first_live, width));
        if fresh {
            let mut lines = Vec::new();
            for entry in &self.paper.entries[..first_live] {
                entry_lines(entry, pane, &mut lines);
            }
            let mut wrapped = Vec::new();
            for line in &lines {
                wrap(line, width, &mut wrapped);
            }
            self.paper.cache = Some((self.paper.revision, first_live, width, wrapped));
        }
        let mut lines = Vec::new();
        for entry in &self.paper.entries[first_live..] {
            entry_lines(entry, pane, &mut lines);
        }
        let mut live = Vec::new();
        for line in &lines {
            wrap(line, width, &mut live);
        }
        live
    }

    /// The whole sheet: `columns` by `rows` cells of ASCII. `clock` and
    /// `load` are the computer's state as the mount reads it.
    pub fn paper_sheet(&mut self, columns: usize, rows: usize, clock: &str, load: &str) -> Sheet {
        let columns = columns.max(40);
        let rows = rows.max(12);
        let inner = columns - 4;
        let transcript_rows = rows - 9;
        let text_width = inner - 2;
        self.paper.grid = (transcript_rows as u16, text_width as u16);
        let border = || {
            vec![Span {
                text: format!("+{}+", "-".repeat(columns - 2)),
                tone: Tone::Quiet,
            }]
        };
        let framed = |spans: Vec<Span>| {
            let used: usize = spans.iter().map(|span| span.text.len()).sum();
            let mut row = vec![Span {
                text: "| ".into(),
                tone: Tone::Quiet,
            }];
            row.extend(spans);
            row.push(Span {
                text: " ".repeat(inner.saturating_sub(used)),
                tone: Tone::Quiet,
            });
            row.push(Span {
                text: " |".into(),
                tone: Tone::Quiet,
            });
            row
        };
        let mut sheet = Sheet::default();
        sheet.rows.push(border());
        let [first, second, third] = self.paper_status(inner, clock, load);
        for (text, tone) in [
            (first, Tone::Loud),
            (second, Tone::Loud),
            (third, Tone::Present),
        ] {
            sheet.rows.push(framed(vec![Span { text, tone }]));
        }
        sheet.rows.push(border());
        // The transcript, or a full-screen program's screen, or the help.
        let pane = self.paper_pane().and_then(|id| self.panes.get(&id));
        let screen = pane.filter(|pane| pane.session.vt.alternate_screen());
        let mut body: Vec<Vec<Span>> = Vec::new();
        let mut bar: Option<(usize, usize)> = None;
        if self.paper.help {
            for text in HELP.iter().take(transcript_rows) {
                body.push(vec![Span {
                    text: fit(text, text_width),
                    tone: Tone::Present,
                }]);
            }
        } else if self.paper.gym.open || self.paper.rules.open || self.paper.files.open {
            // A list reads from its top: the scroll counts lines down.
            let mut wrapped = Vec::new();
            let lines = if self.paper.gym.open {
                crate::gym::lines(&self.paper.gym, unix_now())
            } else if self.paper.rules.open {
                crate::rules::lines(&self.paper.rules, unix_now())
            } else {
                crate::files::lines(&self.paper.files)
            };
            for (text, tone) in lines {
                wrap(&line(text, tone), text_width, &mut wrapped);
            }
            let total = wrapped.len();
            let rules = self.paper.rules.open;
            let (scroll, viewing) = if self.paper.gym.open {
                (&mut self.paper.gym.scroll, self.paper.gym.viewing.is_some())
            } else if rules {
                (&mut self.paper.rules.scroll, false)
            } else {
                (
                    &mut self.paper.files.scroll,
                    self.paper.files.viewing.is_some(),
                )
            };
            if !viewing {
                // Keep the picked file in view.
                let header = wrapped
                    .iter()
                    .position(|line| line.text.starts_with("> "))
                    .unwrap_or(0);
                if header < *scroll {
                    *scroll = header;
                } else if header >= *scroll + transcript_rows {
                    *scroll = header + 1 - transcript_rows;
                }
            }
            *scroll = (*scroll).min(total.saturating_sub(transcript_rows));
            let start = *scroll;
            let end = (start + transcript_rows).min(total);
            for line in &wrapped[start..end] {
                body.push(vec![Span {
                    text: line.text.clone(),
                    tone: line.tone,
                }]);
            }
            bar = Some((start, total));
        } else if self.paper.run.open || self.paper.thread.open {
            let mut wrapped = Vec::new();
            let lines = if self.paper.run.open {
                crate::run::lines(&self.paper.run)
            } else {
                crate::thread::lines(&self.paper.thread)
            };
            for (text, tone) in lines {
                wrap(&line(text, tone), text_width, &mut wrapped);
            }
            let total = wrapped.len();
            let scroll = if self.paper.run.open {
                &mut self.paper.run.scroll
            } else {
                &mut self.paper.thread.scroll
            };
            *scroll = (*scroll).min(total.saturating_sub(transcript_rows));
            let end = total - *scroll;
            let start = end.saturating_sub(transcript_rows);
            for line in &wrapped[start..end] {
                body.push(vec![Span {
                    text: line.text.clone(),
                    tone: line.tone,
                }]);
            }
            bar = Some((start, total));
        } else if let Some(pane) = screen {
            let vt = &pane.session.vt;
            for row in 0..transcript_rows.min(vt.rows()) {
                body.push(cells(vt.row(row), text_width));
            }
            if pane.session.vt.cursor_visible() {
                let (row, col) = vt.cursor();
                if row < transcript_rows && col < text_width {
                    sheet.caret = Some((5 + row, 2 + col));
                }
            }
        } else {
            let live = self.paper_wrapped(text_width);
            let cached = self
                .paper
                .cache
                .as_ref()
                .map_or(&[][..], |cache| &cache.3[..]);
            let total = cached.len() + live.len();
            self.paper.scroll = self.paper.scroll.min(total.saturating_sub(transcript_rows));
            let end = total - self.paper.scroll;
            let start = end.saturating_sub(transcript_rows);
            for index in start..end {
                let line = if index < cached.len() {
                    &cached[index]
                } else {
                    &live[index - cached.len()]
                };
                body.push(vec![Span {
                    text: line.text.clone(),
                    tone: line.tone,
                }]);
            }
            bar = Some((start, total));
            // A running command's caret, at the end of its output.
            if self.paper_running() && self.paper.scroll == 0 && !body.is_empty() {
                let last = body.len() - 1;
                let col = body[last][0].text.len().min(text_width - 1);
                sheet.caret = Some((5 + last, 2 + col));
            }
        }
        // The scroll bar, always drawn: a track and a thumb.
        let (thumb_from, thumb_to) = match bar {
            Some((start, total)) if total > transcript_rows => {
                let size = (transcript_rows * transcript_rows / total).max(1);
                let from = start * transcript_rows / total;
                (from, (from + size).min(transcript_rows))
            }
            _ => (0, transcript_rows),
        };
        for row in 0..transcript_rows {
            let mut spans = body.get(row).cloned().unwrap_or_default();
            let used: usize = spans.iter().map(|span| span.text.len()).sum();
            spans.push(Span {
                text: " ".repeat(text_width.saturating_sub(used) + 1),
                tone: Tone::Quiet,
            });
            let thumb = (thumb_from..thumb_to).contains(&row);
            spans.push(Span {
                text: if thumb { "#" } else { "|" }.into(),
                tone: if thumb { Tone::Present } else { Tone::Receded },
            });
            sheet.rows.push(framed(spans));
        }
        sheet.rows.push(border());
        // The input line.
        let (label, tone) = self.paper_label();
        let caret_row = sheet.rows.len();
        let mut spans = vec![Span {
            text: label.clone(),
            tone: Tone::Quiet,
        }];
        if self.paper_running()
            || (self.smart.pending.is_some() && self.paper.input.is_empty())
            || (self.paper.run.open && self.paper_run_armed())
            || self.paper.files.open
            || self.paper.rules.open
            || self.paper.gym.open
        {
            spans.push(Span {
                text: fit(&self.paper_input_hint(), inner - label.len()),
                tone,
            });
        } else {
            let room = inner - label.len() - 1;
            let chars: Vec<char> = self.paper.input.chars().collect();
            let skip = self.paper.cursor.saturating_sub(room);
            let shown: String = chars.iter().skip(skip).take(room).collect();
            spans.push(Span { text: shown, tone });
            sheet.caret = sheet.caret.or(Some((
                caret_row,
                2 + label.len() + self.paper.cursor - skip,
            )));
            if self.paper_running() {
                sheet.caret = None;
            }
        }
        sheet.rows.push(framed(spans));
        sheet.rows.push(border());
        sheet.rows.push(vec![Span {
            text: fit(
                if self.paper.gym.open {
                    GYM_KEYS
                } else if self.paper.rules.open {
                    RULE_KEYS
                } else if self.paper.files.open {
                    FILE_KEYS
                } else if self.paper.run.open {
                    RUN_KEYS
                } else {
                    KEYS
                },
                columns,
            ),
            tone: Tone::Quiet,
        }]);
        sheet
    }

    fn paper_label(&self) -> (String, Tone) {
        if self.paper_running() {
            return ("RUN  ".into(), Tone::Present);
        }
        if self.smart.pending.is_some() && self.paper.input.is_empty() {
            return ("CONFIRM? ".into(), Tone::Loud);
        }
        if self.paper.gym.open {
            return ("GYM > ".into(), Tone::Quiet);
        }
        if self.paper.rules.open {
            return ("RULES > ".into(), Tone::Quiet);
        }
        if self.paper.files.open {
            return ("FILES > ".into(), Tone::Quiet);
        }
        if self.paper.run.open {
            return if self.paper_run_armed() {
                ("CONFIRM? ".into(), Tone::Loud)
            } else {
                ("STEER > ".into(), Tone::Loud)
            };
        }
        if self.paper.thread.open {
            return ("REPLY > ".into(), Tone::Loud);
        }
        match self.paper_route() {
            Some(Route::Ask) => ("ASK   > ".into(), Tone::Loud),
            _ => ("SHELL > ".into(), Tone::Loud),
        }
    }

    fn paper_run_armed(&self) -> bool {
        self.paper
            .run
            .command
            .as_ref()
            .is_some_and(|command| command.state == crate::run::Sent::Armed)
    }

    fn paper_input_hint(&self) -> String {
        if self.paper.gym.open {
            return if self.paper.gym.viewing.is_some() {
                "UP DOWN scroll, ENTER recomputes again, ESC returns to the list".into()
            } else if self.paper.gym.components {
                "UP DOWN pick a plugin, ENTER opens its newest result, ESC returns".into()
            } else {
                "UP DOWN pick a result, ENTER recomputes it, ESC returns".into()
            };
        }
        if self.paper.rules.open {
            return match &self.paper.rules.change {
                Some(change) if change.state == crate::rules::Changed::Armed => format!(
                    "{} rule {}   ENTER confirms, ESC rejects",
                    change.verb,
                    ascii(&change.id)
                ),
                _ => "UP DOWN pick a rule, ENTER pauses or resumes it, ESC returns".into(),
            };
        }
        if self.paper.files.open {
            return if self.paper.files.viewing.is_some() {
                "UP DOWN scroll, ESC returns to the list".into()
            } else {
                "UP DOWN pick a file, ENTER shows it, ESC returns to the run".into()
            };
        }
        if self.paper.run.open
            && let Some(command) = &self.paper.run.command
            && command.state == crate::run::Sent::Armed
        {
            let task = self
                .paper
                .run
                .task
                .as_ref()
                .map_or("", |(task, _)| task.as_str());
            return if command.verb == "cancel" {
                format!("cancel run {task}   ENTER confirms, ESC rejects")
            } else {
                format!("steer run {task} with your line   ENTER confirms, ESC rejects")
            };
        }
        if self.paper_running() {
            let command = self
                .last_block()
                .map(|block| ascii(&block.command))
                .unwrap_or_default();
            return format!("{command} is running; keys go to it, CTRL+C interrupts it");
        }
        match &self.smart.pending {
            Some((_, key)) => {
                let command = self
                    .smart
                    .book
                    .entries
                    .get(key)
                    .map(|entry| ascii(&entry.proposal.command))
                    .unwrap_or_default();
                if self.paper.warned.as_ref() == Some(key) {
                    format!("{command}   may change files: ENTER again runs it, ESC rejects it")
                } else {
                    format!("{command}   ENTER confirms and runs it, ESC rejects it")
                }
            }
            None => String::new(),
        }
    }

    /// The three status rows: the computer's state, always shown.
    fn paper_status(&self, width: usize, clock: &str, load: &str) -> [String; 3] {
        let pane = self.paper_pane().and_then(|id| self.panes.get(&id));
        let cwd = pane
            .and_then(|pane| pane.session.blocks.cwd.clone())
            .map(|cwd| short_home(&cwd))
            .unwrap_or_else(|| "?".into());
        let exit = match pane.and_then(|pane| pane.session.blocks.records.back()) {
            Some(block) if block.end.is_none() => "running".to_owned(),
            Some(block) => block.status.map_or("?".into(), |status| status.to_string()),
            None => "-".into(),
        };
        let request = if !self.smart.workers.is_empty() {
            "running"
        } else if self.smart.execution.is_some() {
            "confirmed command running"
        } else {
            "idle"
        };
        let pending = usize::from(self.smart.pending.is_some());
        let git = self.paper.git.as_deref().unwrap_or("-");
        let tail = format!("  GIT {git}  EXIT {exit}");
        let room = width.saturating_sub(tail.len() + 4);
        let cwd = ascii(&cwd);
        let cwd = if cwd.len() > room {
            format!("...{}", &cwd[cwd.len() - room.saturating_sub(3)..])
        } else {
            cwd
        };
        let first = format!("DIR {cwd}{tail}");
        let second = format!(
            "REQUEST {}  QUEUE {}  PENDING {}  DOOR {}  LOAD {}  TIME {}",
            request,
            self.paper.queue.len(),
            pending,
            self.paper.door.as_deref().unwrap_or("openagents"),
            load,
            clock,
        );
        let context = match self.paper_attachable() {
            Some(block) => format!(
                "CONTEXT block {} `{}` exit {} (F2 detaches)",
                block.id,
                ascii(&block.command),
                block.status.unwrap_or_default()
            ),
            None if !self.paper.attach => {
                "CONTEXT off (F2 attaches the last failed command)".into()
            }
            None => "CONTEXT directory only".into(),
        };
        let last = match (&self.notice, self.last_block()) {
            (_, Some(block)) if block.status == Some(127) => match self
                .smart
                .correction
                .as_ref()
                .filter(|correction| correction.block == block.id)
            {
                Some(correction) => format!(
                    "LAST not a command; F7 types `{}`, F6 asks OpenAgents",
                    ascii(&correction.choices.lines[0])
                ),
                None => "LAST not a command; F6 asks OpenAgents about it".to_owned(),
            },
            (Some(notice), _) => format!("LAST {}", ascii(notice)),
            _ => "LAST -".into(),
        };
        [
            fit(&first, width),
            fit(&second, width),
            fit(&format!("{context}   {last}"), width),
        ]
    }
}

/// Seconds since the epoch, for how long ago a rule ran.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn short_home(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path == home => "~".into(),
        Ok(home) if !home.is_empty() && path.starts_with(&format!("{home}/")) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_owned(),
    }
}

/// `git status --short --branch` as `branch` and `*` when dirty.
fn git_state(summary: &str) -> String {
    let mut lines = summary.lines();
    let branch = lines
        .next()
        .and_then(|line| line.strip_prefix("## "))
        .map(|line| {
            line.split("...")
                .next()
                .unwrap_or(line)
                .split(' ')
                .next()
                .unwrap_or(line)
        })
        .unwrap_or("-");
    let dirty = lines.any(|line| !line.trim().is_empty());
    format!("{}{}", ascii(branch), if dirty { "*" } else { "" })
}

/// `text` cut or padded to exactly `width` columns.
fn fit(text: &str, width: usize) -> String {
    let text = ascii(text).replace('\n', " ");
    let mut out: String = text.chars().take(width).collect();
    let len = out.chars().count();
    out.push_str(&" ".repeat(width - len));
    out
}

fn wrap(line: &Line, width: usize, out: &mut Vec<Line>) {
    let text = &line.text;
    if text.is_empty() {
        out.push(Line {
            text: String::new(),
            tone: line.tone,
        });
        return;
    }
    // Break at the last space that fits, else mid-word.
    let width = width.max(1);
    let mut rest = text.as_str();
    while rest.len() > width {
        let cut = rest[..=width]
            .rfind(' ')
            .filter(|at| *at > 0)
            .unwrap_or(width);
        out.push(Line {
            text: rest[..cut].trim_end().to_owned(),
            tone: line.tone,
        });
        rest = rest[cut..].trim_start_matches(' ');
    }
    out.push(Line {
        text: rest.to_owned(),
        tone: line.tone,
    });
}

/// A program's screen row as spans: bold is loud, dim is quiet, reverse
/// video is reversed, and everything else is present. Colors map to these.
fn cells(row: Option<&coder_vt::Row>, width: usize) -> Vec<Span> {
    use coder_vt::Flags;
    let mut spans: Vec<Span> = Vec::new();
    let Some(row) = row else {
        return spans;
    };
    let mut used = 0;
    for cell in &row.cells {
        if used >= width {
            break;
        }
        if cell.width == 0 {
            continue;
        }
        let flags = cell.attrs.flags;
        let tone = if flags.contains(Flags::INVERSE) {
            Tone::Reversed
        } else if flags.contains(Flags::BOLD) {
            Tone::Loud
        } else if flags.contains(Flags::DIM) {
            Tone::Quiet
        } else {
            Tone::Present
        };
        let mut text = crate::ascii::char_ascii(cell.ch).to_owned();
        if text.len() != 1 {
            text = if text.is_empty() {
                " ".into()
            } else {
                text[..1].to_owned()
            };
        }
        // A wide character keeps its two columns.
        if cell.width == 2 {
            text.push(' ');
        }
        used += text.len();
        match spans.last_mut() {
            Some(last) if last.tone == tone => last.text.push_str(&text),
            _ => spans.push(Span { text, tone }),
        }
    }
    spans
}

fn entry_lines(entry: &Entry, pane: Option<&crate::application::Pane>, lines: &mut Vec<Line>) {
    match entry {
        Entry::Block(id) => {
            let Some(pane) = pane else { return };
            let Some(block) = pane.session.blocks.get(*id) else {
                return;
            };
            lines.push(line(format!("$ {}", block.command), Tone::Loud));
            let output = if block.end.is_some() {
                block.output.clone()
            } else {
                crate::blocks::live(&pane.session.vt, block)
            };
            for text in output.lines() {
                lines.push(line(text, Tone::Present));
            }
            if block.truncated {
                lines.push(line("[earlier output was not kept]", Tone::Quiet));
            }
            if block.end.is_some() {
                let seconds = block.elapsed_ms.unwrap_or_default() as f64 / 1000.0;
                let status = match block.status {
                    Some(0) => format!("[ok, {seconds:.1} s]"),
                    Some(code) => format!("[exit {code}, {seconds:.1} s]"),
                    None => format!("[ended, status unknown, {seconds:.1} s]"),
                };
                lines.push(line(status, Tone::Quiet));
            }
            lines.push(line("", Tone::Quiet));
        }
        Entry::Ask { text, attached } => {
            lines.push(line(format!("ASK: {text}"), Tone::Loud));
            if let Some(attached) = attached {
                lines.push(line(format!("     with {attached}"), Tone::Quiet));
            }
        }
        Entry::Answer(text) => {
            for (index, text) in text.lines().enumerate() {
                let lead = if index == 0 {
                    "OPENAGENTS: "
                } else {
                    "            "
                };
                lines.push(line(format!("{lead}{text}"), Tone::Present));
            }
            lines.push(line("", Tone::Quiet));
        }
        Entry::Proposal {
            command, verdict, ..
        } => {
            let text = match verdict {
                Verdict::Pending => format!("PROPOSED: {command}   [ENTER] confirm  [ESC] reject"),
                Verdict::Confirmed => format!("CONFIRMED: {command}"),
                Verdict::Rejected => format!("REJECTED: {command}"),
            };
            lines.push(line(
                text,
                if *verdict == Verdict::Pending {
                    Tone::Loud
                } else {
                    Tone::Quiet
                },
            ));
            lines.push(line("", Tone::Quiet));
        }
        Entry::Note(text) => {
            lines.push(line(format!("NOTE: {text}"), Tone::Quiet));
            lines.push(line("", Tone::Quiet));
        }
    }
}
