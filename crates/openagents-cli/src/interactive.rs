//! The interactive `oa coder` session.
//!
//! Three pieces, kept apart on purpose:
//!
//! - [`CoderApp`] is the state machine. It takes keys and turn events and
//!   produces a view. It touches no terminal and no network, so a test can
//!   drive it directly and assert on the frame it renders.
//! - [`run_loop`] joins that state machine to a stream of terminal events and
//!   a channel of turn events. It is generic over both, so the loop a test
//!   runs is the loop production runs.
//! - [`runtime_actor`] owns the [`CoderRuntimeSession`] and does the work the
//!   app asks for: turns, diffs, and starting programs under a
//!   pseudoterminal. It is a task rather than a call inside the loop because
//!   `execute_turn` borrows the session for the length of a turn, and the
//!   frame has to keep drawing while that turn streams.
//!
//! The chunk callback `execute_turn` takes is `FnMut(&str) + Send + 'static`,
//! which cannot borrow the transcript. It sends each chunk down a channel
//! instead, and the loop appends it on arrival — so the reply appears as it is
//! written rather than in one block at the end.
//!
//! ## The three panes
//!
//! The middle of the frame shows the transcript, the diff inspector, or a
//! program running under a pseudoterminal. Only one of them is up at a time
//! and only the one that is up takes keys, which is why [`CoderApp::on_key`]
//! dispatches on the pane before it looks at the key.

use std::path::PathBuf;
use std::sync::Arc;

use crate::cli::CoderArgs;
use crate::composer::complete::{Completion, complete};
use crate::composer::history::History;
use crate::composer::{Composer, ComposerAction};
use crate::diff::{DiffMode, FileDiff};
use crate::pty::{DETACH, PtyControl, PtyEvent, PtyScreen, PtySession};
use crate::runtime::{CoderRuntimeSession, Lane, TurnUsage};
use crate::tools::{DelegationGate, HarnessToolRegistry};
use crate::tui::{
    BoxFrame, ChromeView, DiffPane, Entry, Middle, PtyPane, Role, composer_text_width, pty_viewport,
};

use crossterm::{
    ExecutableCommand,
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::{Stream, StreamExt};
use ratatui::Terminal;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::io::{IsTerminal, stdout};
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// The commands the composer takes, and what each one does.
///
/// One list, read by three things: the `/help` output, Tab completion, and the
/// dispatch in [`CoderApp::run_command`]. A command that is not handled cannot
/// be in this list without failing `every_listed_command_is_handled`.
pub const COMMANDS: &[(&str, &str)] = &[
    ("clear", "clear the transcript"),
    (
        "diff",
        "what changed: /diff, /diff --staged, /diff <path>, /diff <old> <new>",
    ),
    ("export", "write the transcript to a file: /export <path>"),
    ("help", "list these commands"),
    (
        "resume",
        "recent Claude Code and Codex sessions on this machine: /resume, /resume <number>",
    ),
    (
        "run",
        "run a program under a terminal in this frame: /run <command>",
    ),
];

fn command_names() -> Vec<&'static str> {
    COMMANDS.iter().map(|(name, _)| *name).collect()
}

/// A message for the runtime task.
#[derive(Debug, Clone)]
pub enum Control {
    /// Run a turn on this prompt.
    Prompt(String),
    /// Collect a diff. The words are `/diff`'s arguments, already split.
    Diff(Vec<String>),
    /// Ask the foreign-session scanner what other coding agents left on this
    /// machine. `None` lists; `Some(n)` describes the nth listed session.
    ForeignResume(Option<usize>),
    /// Start a program under a pseudoterminal of this size.
    Run {
        command: Vec<String>,
        label: String,
        cols: u16,
        rows: u16,
    },
}

/// A message from the runtime task.
#[derive(Debug, Clone)]
pub enum TurnEvent {
    /// A piece of the reply, as it was written.
    Chunk(String),
    /// The turn finished. Carries the final answer, which matters only when
    /// nothing streamed.
    Done(String),
    /// The turn failed. The session stays open.
    Failed(String),
    /// The model that answered the last turn.
    ///
    /// Reported rather than assumed. `POST /api/v1/threads` does take a
    /// `model`, but what answers is whatever the returned grant pins — a value
    /// outside the enum is refused, and a listed model whose provider is not
    /// configured is refused as `model_unavailable`. So the request is a
    /// preference and the grant is the fact, and this carries the fact.
    ///
    /// Read from `CoderRuntimeSession::last_model` rather than from the grant,
    /// because the local lane has no grant: it resolves its model with Ollama
    /// and `last_grant` stays `None` there for good reasons of its own.
    Model(String),
    /// What the last turn spent, as the server reported it.
    Usage(TurnUsage),
    /// A diff to inspect.
    Diff(Vec<FileDiff>),
    /// Something worth putting on the transcript that was not a turn.
    Notice(String),
    /// A program started, and this is how to talk to it.
    PtyOpen {
        label: String,
        control: Arc<dyn PtyControl>,
    },
    /// Bytes the program wrote.
    PtyOutput(Vec<u8>),
    /// The program ended.
    PtyExit(u32),
}

/// How often the streaming bullet flips.
const PULSE: Duration = Duration::from_millis(400);

/// How long the exit waits for the session to revoke its thread.
///
/// Long enough for a `DELETE` on a working connection, short enough that a
/// reader who has quit does not sit looking at a finished screen.
const THREAD_REVOCATION_GRACE: Duration = Duration::from_secs(15);

/// The diff inspector's state.
struct DiffView {
    files: Vec<FileDiff>,
    index: usize,
    mode: DiffMode,
    scroll: usize,
    /// The rows as last rendered, and the width and mode they were rendered
    /// for. Kept so scrolling does not re-diff, and rebuilt when any of the
    /// three change.
    rows: Vec<Line<'static>>,
    rendered_for: (usize, usize, DiffMode),
}

impl DiffView {
    fn new(files: Vec<FileDiff>) -> Self {
        Self {
            files,
            index: 0,
            mode: DiffMode::Unified,
            scroll: 0,
            rows: Vec::new(),
            rendered_for: (usize::MAX, usize::MAX, DiffMode::Unified),
        }
    }

    fn rows(&mut self, width: usize) -> &[Line<'static>] {
        let key = (self.index, width, self.mode);
        if key != self.rendered_for {
            self.rows = crate::diff::render(&self.files[self.index], self.mode, width);
            self.rendered_for = key;
        }
        &self.rows
    }
}

/// A program running under a pseudoterminal, inside the frame.
struct PtyView {
    label: String,
    screen: PtyScreen,
    control: Arc<dyn PtyControl>,
    exit: Option<u32>,
}

pub struct CoderApp {
    title: String,
    entries: Vec<Entry>,
    composer: Composer,
    history: History,
    /// What Tab last found, when it found more than one candidate.
    completions: Vec<String>,
    /// The directory paths are completed in and commands are run in.
    cwd: PathBuf,
    /// The lane this session was started on, as the status bar names it.
    lane: String,
    /// The model the last turn answered from. Unknown until a turn has run.
    model: Option<String>,
    usage: TurnUsage,
    busy: bool,
    pulse: bool,
    scrollback: usize,
    should_exit: bool,
    /// The size of the last frame drawn, which is what a child is told.
    size: Rect,
    diff: Option<DiffView>,
    pty: Option<PtyView>,
}

impl CoderApp {
    pub fn new(title: &str, lane: &Lane) -> Self {
        let entries = vec![Entry {
            role: Role::Notice,
            // Every claim here is one this screen keeps. The old welcome text
            // invited the reader to type into a session that discarded keys.
            text: "Type a prompt and press Enter. The reply streams in below \
                   as the model writes it. `/help` lists the commands."
                .to_string(),
            settled: true,
        }];
        Self {
            title: title.to_string(),
            entries,
            composer: Composer::new(),
            history: History::new(),
            completions: Vec::new(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            lane: lane_label(lane),
            model: None,
            usage: TurnUsage::default(),
            busy: false,
            pulse: true,
            scrollback: 0,
            should_exit: false,
            size: Rect::new(0, 0, 80, 24),
            diff: None,
            pty: None,
        }
    }

    /// Keep this session's prompts in `path` and start with what is in it.
    ///
    /// Separate from [`CoderApp::new`] so that a test drives a session whose
    /// history is in memory and writes nothing to the reader's home.
    pub fn with_history_file(mut self, path: PathBuf) -> Self {
        self.history = History::load(path);
        self
    }

    /// Complete paths in `cwd` and run commands there.
    pub fn with_working_directory(mut self, cwd: PathBuf) -> Self {
        self.cwd = cwd;
        self
    }

    /// The model the last turn answered from, if a turn has run.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn usage(&self) -> TurnUsage {
        self.usage
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Whether a program is running under a pseudoterminal in this frame.
    pub fn running(&self) -> bool {
        self.pty.as_ref().is_some_and(|pty| pty.exit.is_none())
    }

    /// The exit code of the program in the pane, once it has one.
    ///
    /// For a caller waiting on a program rather than asserting about one: what
    /// the reader sees is the frame, and the frame is what the tests assert.
    pub fn pty_exit(&self) -> Option<u32> {
        self.pty.as_ref().and_then(|pty| pty.exit)
    }

    /// What the program has drawn so far, as text.
    pub fn pty_text(&self) -> Option<String> {
        self.pty.as_ref().map(|pty| pty.screen.text())
    }

    /// Whether the diff inspector is up.
    pub fn inspecting(&self) -> bool {
        self.diff.is_some()
    }

    /// The transcript as text, for `--export` and `/export`.
    pub fn transcript(&self) -> String {
        self.entries
            .iter()
            .filter(|e| !e.text.is_empty())
            .map(|e| {
                let who = match e.role {
                    Role::You => "you",
                    Role::Assistant => "coder",
                    Role::Tool => "tool",
                    Role::Notice => "note",
                    Role::Error => "error",
                };
                format!("[{who}] {}", e.text)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn push(&mut self, role: Role, text: impl Into<String>) {
        self.entries.push(Entry::new(role, text));
        // New material at the bottom pulls the view back to the bottom.
        self.scrollback = 0;
    }

    /// Send a prompt, whatever its source: the composer, or `--prompt`.
    pub fn submit(&mut self, prompt: String, control: &UnboundedSender<Control>) {
        let prompt = prompt.trim_end().to_string();
        if prompt.is_empty() {
            return;
        }
        self.history.record(&prompt);
        self.completions.clear();

        if crate::composer::is_local_slash_input(&prompt, COMMANDS) {
            self.run_command(&prompt, control);
            return;
        }

        self.push(Role::You, prompt.clone());
        self.entries.push(Entry::streaming(Role::Assistant));
        self.busy = true;
        self.scrollback = 0;
        if control.send(Control::Prompt(prompt)).is_err() {
            self.finish_turn();
            self.push(
                Role::Error,
                "The runtime task is gone, so this prompt was not sent. Restart the session.",
            );
        }
    }

    /// Run one of the session's own commands.
    ///
    /// Known commands and bare command-shaped typos come here. Slash-prefixed
    /// paths and messages go to the model.
    fn run_command(&mut self, line: &str, control: &UnboundedSender<Control>) {
        let words = crate::pty::split_command(line.trim_start_matches('/'));
        let Some(name) = words.first().cloned() else {
            self.push(Role::Error, "A command needs a name. Try `/help`.");
            return;
        };
        let arguments = &words[1..];
        self.push(Role::You, line.to_string());

        match name.as_str() {
            "help" => {
                let listed = COMMANDS
                    .iter()
                    .map(|(name, what)| format!("/{name} — {what}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                self.push(Role::Notice, listed);
            }
            "clear" => {
                self.entries.clear();
                self.scrollback = 0;
            }
            "export" => match arguments.first() {
                None => self.push(Role::Error, "`/export` needs a path: `/export notes.txt`."),
                Some(path) => match std::fs::write(path, self.transcript()) {
                    Ok(()) => self.push(Role::Notice, format!("Transcript written to {path}.")),
                    Err(error) => {
                        self.push(Role::Error, format!("Could not write {path}: {error}"))
                    }
                },
            },
            "diff" => {
                if control.send(Control::Diff(arguments.to_vec())).is_err() {
                    self.push(Role::Error, "The runtime task is gone.");
                }
            }
            "resume" => {
                // A bare `/resume` lists; `/resume <n>` picks. A word that is
                // not a positive number is refused rather than read as a list
                // request, because silently listing after a mistyped pick is
                // how someone resumes the wrong session.
                let selection = match arguments.first() {
                    None => None,
                    Some(word) => match word.parse::<usize>() {
                        Ok(number) if number >= 1 => Some(number),
                        _ => {
                            self.push(
                                Role::Error,
                                format!(
                                    "`/resume` takes a number from the list: `/resume 1`. `{word}` is not one."
                                ),
                            );
                            return;
                        }
                    },
                };
                if control.send(Control::ForeignResume(selection)).is_err() {
                    self.push(Role::Error, "The runtime task is gone.");
                }
            }
            "run" => {
                // The words after `/run` are the command, but a line a shell
                // would change the meaning of is given to a shell instead, so
                // `/run ls | wc -l` runs what it looks like it runs.
                let rest = line
                    .trim_start_matches('/')
                    .strip_prefix("run")
                    .unwrap_or("")
                    .trim();
                if rest.is_empty() {
                    self.push(Role::Error, "`/run` needs a command: `/run git status`.");
                    return;
                }
                let command = if crate::pty::needs_a_shell(rest) {
                    crate::pty::shell_command(rest)
                } else {
                    crate::pty::split_command(rest)
                };
                let (cols, rows) = pty_viewport(self.size);
                let message = Control::Run {
                    command,
                    label: rest.to_string(),
                    cols,
                    rows,
                };
                if control.send(message).is_err() {
                    self.push(Role::Error, "The runtime task is gone.");
                }
            }
            other => self.push(
                Role::Error,
                format!("There is no `/{other}`. `/help` lists the commands."),
            ),
        }
    }

    /// Settle whatever was streaming and take the composer off hold.
    fn finish_turn(&mut self) {
        if let Some(last) = self.entries.last_mut() {
            if !last.settled {
                last.settled = true;
            }
        }
        self.busy = false;
    }

    pub fn on_turn_event(&mut self, event: TurnEvent) {
        match event {
            TurnEvent::Chunk(chunk) => {
                match self.entries.last_mut() {
                    Some(last) if !last.settled => last.text.push_str(&chunk),
                    // A chunk with no open turn to attach to still belongs on
                    // the transcript rather than in the bin.
                    _ => self.entries.push(Entry {
                        role: Role::Assistant,
                        text: chunk,
                        settled: false,
                    }),
                }
                self.scrollback = 0;
            }
            TurnEvent::Done(answer) => {
                // `execute_turn` returns the last step's text, which has
                // already streamed. It is the fallback for the paths that
                // return without streaming anything.
                if let Some(last) = self.entries.last_mut() {
                    if !last.settled && last.text.is_empty() {
                        last.text = answer;
                    }
                }
                self.finish_turn();
            }
            TurnEvent::Failed(message) => {
                self.finish_turn();
                self.push(Role::Error, format!("Turn failed: {message}"));
            }
            TurnEvent::Model(model) => self.model = Some(model),
            TurnEvent::Usage(usage) => self.usage = usage,
            TurnEvent::Notice(message) => self.push(Role::Notice, message),
            TurnEvent::Diff(files) => {
                if files.is_empty() {
                    self.push(Role::Notice, "Nothing has changed.");
                } else {
                    self.diff = Some(DiffView::new(files));
                }
            }
            TurnEvent::PtyOpen { label, control } => {
                let (cols, rows) = pty_viewport(self.size);
                self.pty = Some(PtyView {
                    label,
                    screen: PtyScreen::new(cols, rows),
                    control,
                    exit: None,
                });
            }
            TurnEvent::PtyOutput(bytes) => {
                if let Some(pty) = self.pty.as_mut() {
                    pty.screen.feed(&bytes);
                }
            }
            TurnEvent::PtyExit(code) => {
                if let Some(pty) = self.pty.as_mut() {
                    pty.exit = Some(code);
                }
            }
        }
    }

    pub fn tick(&mut self) {
        if self.busy {
            self.pulse = !self.pulse;
        } else {
            self.pulse = true;
        }
    }

    /// Note the size of the frame, and tell a running program about it.
    ///
    /// This is where a terminal resize becomes a `SIGWINCH` for the child: the
    /// emulated screen is resized, and only if that changed anything is the
    /// pseudoterminal's own window size set, which is the call the kernel
    /// turns into the signal.
    pub fn on_size(&mut self, area: Rect) {
        self.size = area;
        let (cols, rows) = pty_viewport(area);
        if let Some(pty) = self.pty.as_mut() {
            if pty.screen.resize(cols, rows) {
                pty.control.resize(cols, rows);
            }
        }
    }

    pub fn on_key(&mut self, key: &KeyEvent, width: u16, control: &UnboundedSender<Control>) {
        // A key release reported by an enhanced protocol is not a keystroke.
        if key.kind == KeyEventKind::Release {
            return;
        }
        // Whichever pane is up takes the keyboard first. A program under a
        // pseudoterminal takes all of it — including Esc and Ctrl+C, which a
        // full-screen program needs — so the only key held back is the one
        // that takes the keyboard away again.
        if self.pty.is_some() {
            self.on_pty_key(key);
            return;
        }
        if self.diff.is_some() {
            self.on_diff_key(key, width);
            return;
        }
        self.on_transcript_key(key, width, control);
    }

    fn on_pty_key(&mut self, key: &KeyEvent) {
        let detach = key.code == DETACH.code && key.modifiers == DETACH.modifiers;
        let exit = self.pty.as_ref().and_then(|pty| pty.exit);

        // A program that has ended leaves its screen up, because that screen
        // is usually the answer. Dismissing it is what these keys do.
        if let Some(code) = exit {
            if detach || matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                let label = self.pty.take().map(|pty| pty.label).unwrap_or_default();
                self.push(
                    Role::Notice,
                    if code == 0 {
                        format!("`{label}` finished.")
                    } else {
                        format!("`{label}` exited with code {code}.")
                    },
                );
            }
            return;
        }

        if detach {
            if let Some(pty) = self.pty.take() {
                pty.control.kill();
                self.push(Role::Notice, format!("Stopped `{}`.", pty.label));
            }
            return;
        }

        // Everything else is the program's. It is encoded as the bytes a
        // terminal would have sent, in the cursor mode the program asked for.
        if let Some(pty) = self.pty.as_ref() {
            if let Some(bytes) = crate::pty::encode_key(key, pty.screen.application_cursor()) {
                pty.control.write(&bytes);
            }
        }
    }

    fn on_diff_key(&mut self, key: &KeyEvent, width: u16) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.diff = None;
                return;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_exit = true;
                return;
            }
            _ => {}
        }

        let page = usize::from(self.size.height.saturating_sub(10)).max(1);
        let Some(diff) = self.diff.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Char('v') => {
                diff.mode = diff.mode.toggled();
                diff.scroll = 0;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let total = diff.files.len();
                diff.index = if key.code == KeyCode::Tab {
                    (diff.index + 1) % total
                } else {
                    (diff.index + total - 1) % total
                };
                diff.scroll = 0;
            }
            KeyCode::Down => diff.scroll = diff.scroll.saturating_add(1),
            KeyCode::Up => diff.scroll = diff.scroll.saturating_sub(1),
            KeyCode::PageDown => diff.scroll = diff.scroll.saturating_add(page),
            KeyCode::PageUp => diff.scroll = diff.scroll.saturating_sub(page),
            KeyCode::Home => diff.scroll = 0,
            _ => {}
        }
        // Scrolling past the last row would leave an empty pane with no way of
        // telling that it is the end rather than a failure to draw.
        let last = diff.rows(diff_body_width(width)).len().saturating_sub(1);
        diff.scroll = diff.scroll.min(last);
    }

    fn on_transcript_key(
        &mut self,
        key: &KeyEvent,
        width: u16,
        control: &UnboundedSender<Control>,
    ) {
        match key.code {
            KeyCode::Esc => {
                self.should_exit = true;
                return;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_exit = true;
                return;
            }
            KeyCode::PageUp => {
                self.scrollback = self.scrollback.saturating_add(5);
                return;
            }
            KeyCode::PageDown => {
                self.scrollback = self.scrollback.saturating_sub(5);
                return;
            }
            _ => {}
        }

        if self.busy {
            // The composer is on hold. Saying so is the point of the pane's
            // title; swallowing the key here is what makes that true.
            return;
        }

        if key.code == KeyCode::Tab && key.modifiers.is_empty() {
            self.on_tab();
            return;
        }

        match self.composer.handle_key(key, composer_text_width(width)) {
            ComposerAction::Submit(text) => self.submit(text, control),
            ComposerAction::Redraw => {
                // Editing ends a history walk: the next Up starts again from
                // what is now in the composer rather than from where the walk
                // had got to.
                self.history.stop_walking();
                self.completions.clear();
                self.scrollback = 0;
            }
            ComposerAction::Moved => self.completions.clear(),
            ComposerAction::Ignored => match key.code {
                // Up and Down reach the input history once the caret has run
                // out of composer to move through. Scrolling the transcript is
                // PgUp and PgDn, which the status bar names.
                KeyCode::Up => {
                    if let Some(prompt) = self.history.previous(self.composer.text()) {
                        self.composer.set_text(&prompt);
                        self.completions.clear();
                    }
                }
                KeyCode::Down => {
                    if let Some(prompt) = self.history.forward() {
                        self.composer.set_text(&prompt);
                        self.completions.clear();
                    }
                }
                _ => {}
            },
        }
    }

    /// Tab: complete the word at the caret.
    fn on_tab(&mut self) {
        let Completion { insert, candidates } = complete(
            self.composer.text(),
            self.composer.cursor_byte(),
            &command_names(),
            &self.cwd,
        );
        if !insert.is_empty() {
            self.composer.insert_str(&insert);
            self.history.stop_walking();
        }
        self.completions = candidates;
    }

    /// Draw one frame.
    pub fn draw<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> std::io::Result<()> {
        let area = terminal
            .size()
            .map(|size| Rect::new(0, 0, size.width, size.height))?;
        self.on_size(area);

        // The diff rows are built before the frame because the renderer takes
        // them borrowed, and building them needs the view mutably.
        let body = diff_body_width(area.width);
        let diff_rows: Vec<Line<'static>> = match self.diff.as_mut() {
            Some(diff) => diff.rows(body).to_vec(),
            None => Vec::new(),
        };

        let text_width = composer_text_width(area.width);
        let rows = self.composer.rows(text_width);
        let cursor = self.composer.cursor_rowcol(text_width);
        let completions = self.completions.clone();

        let middle = match (&self.diff, &self.pty) {
            (_, Some(pty)) => Middle::Pty(PtyPane {
                command: &pty.label,
                screen: &pty.screen,
                exit: pty.exit,
            }),
            (Some(diff), None) => Middle::Diff(DiffPane {
                path: &diff.files[diff.index].path,
                position: (diff.index, diff.files.len()),
                mode: diff.mode,
                rows: &diff_rows,
                scroll: diff.scroll,
            }),
            (None, None) => Middle::Transcript,
        };

        let frame = BoxFrame::new(&self.title);
        let view = ChromeView {
            title: &self.title,
            entries: &self.entries,
            middle,
            composer_rows: &rows,
            composer_cursor: cursor,
            completions: &completions,
            model: self.model.as_deref(),
            lane: &self.lane,
            usage: self.usage,
            busy: self.busy,
            pulse: self.pulse,
            scrollback: self.scrollback,
        };
        terminal.draw(|f| frame.render(f, f.area(), &view))?;
        Ok(())
    }
}

/// The width the diff inspector's rows are built for, inside a frame that wide.
fn diff_body_width(frame_width: u16) -> usize {
    usize::from(frame_width).saturating_sub(2).max(8)
}

/// The lane, as the status bar names it: what it is called, and its tier when
/// it belongs to one.
///
/// A model named directly belongs to no tier, which is a different answer from
/// `auto` and is worth not inventing one for.
fn lane_label(lane: &Lane) -> String {
    match lane.tier() {
        Some(tier) => format!("{} ({tier})", lane.label()),
        None => lane.label(),
    }
}

/// Drive the session until the reader exits or the terminal event stream ends.
///
/// `keepalive` is a sender for the turn channel that this function holds for
/// its own lifetime, so `turns.recv()` cannot resolve to `None` and spin the
/// loop when the runtime task ends.
pub async fn run_loop<B, S>(
    terminal: &mut Terminal<B>,
    app: &mut CoderApp,
    events: &mut S,
    control: UnboundedSender<Control>,
    turns: &mut UnboundedReceiver<TurnEvent>,
    keepalive: UnboundedSender<TurnEvent>,
) -> std::io::Result<()>
where
    B: Backend,
    S: Stream<Item = std::io::Result<Event>> + Unpin,
{
    let _keepalive = keepalive;
    let mut ticker = tokio::time::interval(PULSE);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        app.draw(terminal)?;
        if app.should_exit() {
            return Ok(());
        }

        let width = terminal.size()?.width;

        tokio::select! {
            event = events.next() => match event {
                Some(Ok(Event::Key(key))) => app.on_key(&key, width, &control),
                // A resize is drawn on the next pass, and `draw` is what tells
                // a running child its new size.
                Some(Ok(Event::Resize(_, _))) => {}
                Some(Ok(_)) => {}
                // A terminal that has gone away is an exit, not an error to
                // report into a screen nobody can see.
                Some(Err(_)) | None => return Ok(()),
            },
            turn = turns.recv() => {
                if let Some(event) = turn {
                    app.on_turn_event(event);
                }
            },
            _ = ticker.tick() => app.tick(),
        }
    }
}

/// End a session's thread, and say what it cost and what went unrecorded.
///
/// The ending is a report — what the session's last turn actually did — and
/// not a `DELETE`, which would file every session here as a cancellation
/// whatever it answered (issue #106).
///
/// The lines land on stdout rather than in a screen because both callers reach
/// here after their screen is gone. Silence when the session held no thread:
/// the local lane has nothing to end and nothing was billed.
pub async fn close_and_report(session: &mut CoderRuntimeSession) {
    match session.finish().await {
        Ok(spent) => {
            if let Some(line) = session.spend_line(spent) {
                println!("{line}");
            }
        }
        Err(error) => eprintln!("oa: the thread was not ended: {error}"),
    }
    for failure in &session.record_failures {
        eprintln!("oa: {failure}");
    }
}

/// Own the session and do what the app asks for.
///
/// Returns once the app has dropped its control channel, having revoked the
/// session's thread on the way out. The caller awaits that rather than
/// aborting the task: an aborted actor drops its session, and the `Drop` impl
/// only spawns a revocation that the exiting process may never poll.
pub async fn runtime_actor(
    mut session: CoderRuntimeSession,
    mut control: UnboundedReceiver<Control>,
    events: UnboundedSender<TurnEvent>,
) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    while let Some(message) = control.recv().await {
        match message {
            Control::Prompt(prompt) => {
                let sink = events.clone();
                let result = session
                    .execute_turn(&prompt, move |chunk| {
                        let _ = sink.send(TurnEvent::Chunk(chunk.to_string()));
                    })
                    .await;
                // Report what answered and what it cost before the turn
                // settles, so the status bar is right by the time the composer
                // comes off hold. `last_model` and not the grant: the local
                // lane resolves its model with Ollama and holds no grant.
                if let Some(model) = &session.last_model {
                    let _ = events.send(TurnEvent::Model(model.clone()));
                }
                let _ = events.send(TurnEvent::Usage(session.last_usage));
                let event = match result {
                    Ok(answer) => TurnEvent::Done(answer),
                    Err(error) => TurnEvent::Failed(error.to_string()),
                };
                if events.send(event).is_err() {
                    break;
                }
            }
            Control::Diff(arguments) => {
                let event = match collect_diff(&arguments, &cwd).await {
                    Ok(files) => TurnEvent::Diff(files),
                    Err(why) => TurnEvent::Notice(why),
                };
                if events.send(event).is_err() {
                    break;
                }
            }
            Control::ForeignResume(selection) => {
                // The scan compiles and runs a wasm guest and walks two state
                // directories, all of it synchronous. On a blocking thread so
                // the frame keeps drawing while it works.
                let here = cwd.clone();
                let home = crate::auth::home_directory();
                let scanned = tokio::task::spawn_blocking(move || {
                    crate::foreign_resume::foreign_resume_turn(&here, &home, selection)
                })
                .await;
                let notice = match scanned {
                    Ok(text) => text,
                    Err(error) => format!("The foreign-session scan did not finish: {error}"),
                };
                if events.send(TurnEvent::Notice(notice)).is_err() {
                    return;
                }
            }
            Control::Run {
                command,
                label,
                cols,
                rows,
            } => match PtySession::spawn(&command, Some(cwd.clone()), cols, rows) {
                Err(error) => {
                    let _ = events.send(TurnEvent::Notice(format!("Could not run it: {error}")));
                }
                Ok((session, mut output)) => {
                    let opened = events.send(TurnEvent::PtyOpen {
                        label,
                        control: session,
                    });
                    if opened.is_err() {
                        break;
                    }
                    // Forwarded from a task of its own so this actor stays
                    // able to answer while the program runs.
                    let sink = events.clone();
                    tokio::spawn(async move {
                        while let Some(event) = output.recv().await {
                            let message = match event {
                                PtyEvent::Output(bytes) => TurnEvent::PtyOutput(bytes),
                                PtyEvent::Exit(code) => TurnEvent::PtyExit(code),
                            };
                            if sink.send(message).is_err() {
                                return;
                            }
                        }
                    });
                }
            },
        }
    }
    // The app has gone. Revoke the thread here, awaited, rather than leaving
    // it to `Drop` in an aborted task.
    close_and_report(&mut session).await;
}

/// Collect the diff `/diff` asked for.
///
/// With two paths that both exist, the two files are compared directly. With
/// anything else, git is asked — because for a working tree git already knows
/// what the index and `HEAD` hold, and recomputing that from files on disk
/// would answer a different question.
pub async fn collect_diff(
    arguments: &[String],
    cwd: &std::path::Path,
) -> Result<Vec<FileDiff>, String> {
    if arguments.len() == 2 {
        let (old, new) = (cwd.join(&arguments[0]), cwd.join(&arguments[1]));
        if old.is_file() && new.is_file() {
            let read = |path: &std::path::Path| {
                std::fs::read_to_string(path)
                    .map_err(|error| format!("Could not read {}: {error}", path.display()))
            };
            let (before, after) = (read(&old)?, read(&new)?);
            return Ok(vec![FileDiff {
                path: arguments[1].clone(),
                renamed_from: Some(arguments[0].clone()),
                hunks: crate::diff::compare(&before, &after, crate::diff::CONTEXT),
                note: None,
            }]);
        }
    }

    let mut git: Vec<String> = vec!["--no-pager".into(), "diff".into()];
    let staged = arguments
        .iter()
        .any(|word| word == "--staged" || word == "--cached");
    if staged {
        git.push("--cached".into());
    } else {
        // Against `HEAD` rather than the index, so `/diff` answers "what have
        // I changed since the last commit" whether or not anything is staged.
        git.push("HEAD".into());
    }
    let paths: Vec<&String> = arguments
        .iter()
        .filter(|word| !word.starts_with("--"))
        .collect();
    if !paths.is_empty() {
        git.push("--".into());
        git.extend(paths.into_iter().cloned());
    }

    let output = tokio::process::Command::new("git")
        .args(&git)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|error| format!("Could not run git: {error}"))?;

    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        let message = message.trim();
        return Err(if message.is_empty() {
            "git could not produce a diff here.".to_string()
        } else {
            format!("git said: {message}")
        });
    }
    Ok(crate::diff::parse_unified(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// Put the terminal back however this function is left, including by a panic
/// or by an error on the way out of a turn.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> std::io::Result<Self> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = stdout().execute(crossterm::cursor::Show);
    }
}

/// A panic inside the alternate screen would otherwise leave the terminal in
/// raw mode with the backtrace painted somewhere the reader cannot scroll to.
fn install_panic_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = stdout().execute(LeaveAlternateScreen);
            let _ = stdout().execute(crossterm::cursor::Show);
            previous(info);
        }));
    });
}

/// The tools a session a person sits in gets.
///
/// An interactive session may start children on the same terms a headless one
/// does: same lane, same credential, and the children do not get the tool
/// themselves. This lives in one function so both entry points here share it —
/// they each built the registry separately before, both passed `None`, and the
/// result was that `delegate` worked headless and was missing from the only
/// session anyone actually types into.
///
/// The discovered ACP agents ride on the gate, so naming one in a `delegate`
/// call hands the whole task to it on its own bill; a machine with none just
/// gets the plain fan-out wording.
async fn session_tools(
    lane_name: &str,
    api_base: &str,
    token: &Option<String>,
    child: crate::delegate::ChildOptions,
) -> HarnessToolRegistry {
    // An interactive session has an operator at the keyboard, so the
    // read-only mount tier is granted here: `capability` may load the
    // foreign-session scanner and conversation readers without a refusal the
    // operator would have to work around by hand.
    HarnessToolRegistry::with_delegation(
        None,
        DelegationGate {
            lane: lane_name.to_string(),
            user_token: token.clone(),
            api_base: Some(api_base.to_string()),
            max_count: crate::delegate::MAX_DELEGATE_COUNT,
            child,
            acp_agents: crate::coder::acp::find_agents().await.unwrap_or_default(),
            acp_spent: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
    )
    .allowing_plugin_mounts()
}

pub async fn run_tui(
    args: CoderArgs,
    api_base: String,
    token: Option<String>,
    repository: Option<String>,
    resumed: Option<crate::resume::Resumption>,
) -> Result<(), Box<dyn std::error::Error>> {
    let lane_name = args.lane_name()?;
    let lane = Lane::from_str(&lane_name);

    // `--plain` asks for the line-oriented path even on a terminal. Without a
    // terminal there is no full-screen session to run either way.
    if args.plain || !is_terminal() {
        return run_without_a_terminal(args, api_base, token, repository, lane, resumed).await;
    }

    let tools = session_tools(&lane_name, &api_base, &token, args.child_options()).await;
    let mut session = CoderRuntimeSession::new(lane.clone(), Some(api_base), token, tools);
    session.reasoning = args.reasoning.clone();
    session.ollama_num_ctx = args.num_ctx.or_else(|| {
        std::env::var("OPENAGENTS_OLLAMA_NUM_CTX")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
    });
    session.repository = repository;
    // A resumed thread is adopted before the screen is entered, so its refusal
    // is readable rather than painted over and wiped on exit.
    if let Some(resumption) = &resumed {
        crate::resume::apply(&mut session, resumption).await?;
        println!(
            "{}",
            crate::resume::resumed_line(
                resumption,
                session.last_model.as_deref().unwrap_or("an unnamed model")
            )
        );
    }

    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (event_tx, mut event_rx) = unbounded_channel::<TurnEvent>();
    let runtime = tokio::spawn(runtime_actor(session, control_rx, event_tx.clone()));

    let mut app =
        CoderApp::new("openagents coder", &lane).with_history_file(History::default_path());
    if let Some(prompt) = args.prompt.clone() {
        app.submit(prompt, &control_tx);
    }

    install_panic_hook();
    let result = {
        let _guard = TerminalGuard::enter()?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;
        let mut events = EventStream::new();
        run_loop(
            &mut terminal,
            &mut app,
            &mut events,
            control_tx,
            &mut event_rx,
            event_tx,
        )
        .await
    };

    // `run_loop` has taken the control sender with it, so the actor's receiver
    // is closed and the actor is on its way out with a revocation to make.
    // Awaited rather than aborted: aborting drops the session inside a dead
    // task, where the `Drop` impl can only spawn a DELETE this process may
    // exit before polling. Bounded, because a turn still streaming would
    // otherwise hold the exit for as long as the model wants — and in that
    // case `Drop`'s best effort is what is left, which is what it is for.
    if tokio::time::timeout(THREAD_REVOCATION_GRACE, runtime)
        .await
        .is_err()
    {
        eprintln!(
            "oa: the session was still working after {}s, so its thread was left to the \
             best-effort revocation.",
            THREAD_REVOCATION_GRACE.as_secs()
        );
    }
    result?;

    if let Some(path) = args.export {
        std::fs::write(&path, app.transcript())?;
        println!("Transcript written to {path}");
    }
    Ok(())
}

/// The `--export` transcript for a one-turn run.
///
/// The same two-role, `[who] text` shape [`CoderApp::transcript`] writes, so a
/// transcript from a plain or headless run reads like one from the session.
pub fn transcript_of(prompt: &str, answer: &str) -> String {
    let mut parts = Vec::new();
    if !prompt.is_empty() {
        parts.push(format!("[you] {prompt}"));
    }
    if !answer.trim().is_empty() {
        parts.push(format!("[coder] {}", answer.trim_end()));
    }
    parts.join("\n\n")
}

/// Line-oriented output: one turn, printed as lines, with no cursor control.
///
/// This is both the no-terminal path and what `--plain` asks for on a
/// terminal. It emits no escape sequences at all, so it can be piped to a file
/// or read by something that is not a terminal emulator.
///
/// The version this replaces printed `Coder response: Interactive session
/// initialized in non-TTY mode.` and never called the runtime at all.
async fn run_without_a_terminal(
    args: CoderArgs,
    api_base: String,
    token: Option<String>,
    repository: Option<String>,
    lane: Lane,
    resumed: Option<crate::resume::Resumption>,
) -> Result<(), Box<dyn std::error::Error>> {
    let lane_name = args.lane_name()?;
    let Some(prompt) = args.prompt.clone() else {
        eprintln!(
            "`oa coder` needs a terminal for an interactive session. \
             Give it a prompt, or use `--headless`, to run one turn here."
        );
        return Ok(());
    };

    let tools = session_tools(&lane_name, &api_base, &token, args.child_options()).await;
    let mut session = CoderRuntimeSession::new(lane, Some(api_base), token, tools);
    session.reasoning = args.reasoning.clone();
    session.ollama_num_ctx = args.num_ctx.or_else(|| {
        std::env::var("OPENAGENTS_OLLAMA_NUM_CTX")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
    });
    session.repository = repository;
    if let Some(resumption) = &resumed {
        crate::resume::apply(&mut session, resumption).await?;
        println!(
            "{}",
            crate::resume::resumed_line(
                resumption,
                session.last_model.as_deref().unwrap_or("an unnamed model")
            )
        );
    }
    // The reply is printed as it streams. `execute_turn` also returns the last
    // step's text, which is the same text — so it is printed only when nothing
    // streamed, which is how the offline paths still say something.
    let streamed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let saw = std::sync::Arc::clone(&streamed);
    let answered = session
        .execute_turn(&prompt, move |chunk| {
            use std::io::Write;
            saw.store(true, std::sync::atomic::Ordering::Relaxed);
            print!("{chunk}");
            let _ = std::io::stdout().flush();
        })
        .await
        .map_err(|e| e.to_string());
    // Awaited, and awaited whether the turn worked or not. This path used to
    // return on the failure and leave the revocation to `Drop`, which spawns a
    // best-effort DELETE that may never be polled before the process exits —
    // and a thread left open holds its grant's remaining budget.
    close_and_report(&mut session).await;
    let answer = answered?;
    if !streamed.load(std::sync::atomic::Ordering::Relaxed) {
        print!("{answer}");
    }
    println!();

    // `--export` used to be read only by the full-screen session, so a plain,
    // piped, or headless run that asked for a transcript got none and was told
    // nothing.
    if let Some(path) = args.export.as_deref() {
        std::fs::write(path, transcript_of(&prompt, &answer))
            .map_err(|error| format!("could not write the transcript to {path}: {error}"))?;
        println!("Transcript written to {path}");
    }
    Ok(())
}

fn is_terminal() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both entry points in this file once built their own registry with
    /// `HarnessToolRegistry::new(None)`, which carries no delegation gate. The
    /// symptom was subtle: `oa coder --headless` could start children and the
    /// interactive session silently could not, because a missing tool looks
    /// exactly like a model choosing not to call it.
    #[tokio::test]
    async fn an_interactive_session_can_delegate() {
        let tools = session_tools(
            "glm-5.3-flash",
            "https://example.invalid/api/v1",
            &Some("token".to_string()),
            Default::default(),
        )
        .await;
        let names: Vec<String> = tools.list_tools().into_iter().map(|t| t.name).collect();
        assert!(
            names.iter().any(|n| n == "delegate"),
            "an interactive session got no `delegate` tool; it has {:?}",
            names
        );
    }

    /// The gate carries the lane and credential children spend against. A gate
    /// that exists but names the wrong lane would start children on the default
    /// lane while the session runs on another, which is worse than no gate.
    #[tokio::test]
    async fn the_gate_carries_this_sessions_lane_and_credential() {
        let tools = session_tools(
            "claude",
            "https://example.invalid/api/v1",
            &Some("secret-token".to_string()),
            Default::default(),
        )
        .await;
        let gate = tools
            .delegation
            .as_ref()
            .expect("an interactive session must carry a delegation gate");
        assert_eq!(gate.lane, "claude");
        assert_eq!(gate.user_token.as_deref(), Some("secret-token"));
        assert_eq!(gate.max_count, crate::delegate::MAX_DELEGATE_COUNT);
    }

    /// The status bar names the lane and, when the lane has one, its tier.
    #[test]
    fn the_lane_label_carries_the_tier_only_when_there_is_one() {
        assert_eq!(lane_label(&Lane::Flash), "Coder Flash (flash)");
        assert_eq!(lane_label(&Lane::Free), "Coder Free (free)");
        assert_eq!(
            lane_label(&Lane::Local(String::new())),
            "Coder Local (local)"
        );
        // A model named directly belongs to no tier, so none is invented.
        assert_eq!(
            lane_label(&Lane::Named("glm-5.3-flash".to_string())),
            "Coder (glm-5.3-flash)"
        );
        assert_eq!(
            lane_label(&Lane::Named("some-model".to_string())),
            "Coder (some-model)"
        );
    }

    /// Every command the composer offers has to be one the dispatch handles.
    /// `/help` lists this table and Tab completes from it, so an entry with no
    /// arm behind it would be advertised and inert.
    #[test]
    fn every_listed_command_is_handled() {
        let (tx, _rx) = unbounded_channel();
        for (name, _) in COMMANDS {
            let mut app = CoderApp::new("t", &Lane::default());
            app.run_command(&format!("/{name}"), &tx);
            let said = app.transcript();
            assert!(
                !said.contains(&format!("There is no `/{name}`")),
                "`/{name}` is listed and not handled"
            );
        }
    }
}
