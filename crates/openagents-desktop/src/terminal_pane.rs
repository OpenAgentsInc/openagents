//! The Terminal beside the chat (#11180): a shell on this computer in a
//! pane of the chat window, where `coder` runs as it does in any terminal,
//! and the Agents panel under it.
//!
//! - The shell is the person's own (`$SHELL` as a login shell; `cmd.exe`
//!   on Windows) on a real PTY, opened by an in-process `coder-pty` host
//!   that only this window talks to. Its `PATH` puts the installed Coder
//!   (`~/.openagents/bin`) first and the app's own folder last, so `coder`
//!   is the one the person installed and the account it signs in with is
//!   the same one every other terminal here uses.
//! - Output is drawn by `coder-vt` on a painted surface: the grid, its
//!   colors, and the text caret. Keys typed while the pane has focus go to
//!   the shell; the window's own shortcuts (Cmd on a Mac, Ctrl+Shift
//!   elsewhere) keep working.
//! - The Agents panel reads every running Coder's agent list on this
//!   computer (`agent_fleet::board`), so a background agent started with
//!   `/agent` in this terminal, or any other, shows here with its engine,
//!   status, time, tokens and cost, and **Stop** stops it.
//!
//! The shell starts only when the window opens (never in a capture or a
//! test window), and closing the window ends it.

use crate::model::Intent;
use crate::terminal_action::Action;
use coder_pty::host::{self, Config, Host, Right, Rights};
use coder_pty::wire::{
    Attach, Body, Cause, Frame, Input, Launch, Mode, Open, Resize, Size, TerminalRef, Value,
};
use coder_vt::{Flags, Key, Modifiers, Terminal};
use rust_native::layout::display::{Font, FontFamily, Weight};
use rust_native::style::{Color, Style, TextAlign};
use rust_native::{Axis, Element, Node, TextRole};
use rust_native_desktop::input::SurfaceInput;
use rust_native_desktop::text::{Fonts, LINE_EM};
use rust_native_desktop::{Frame as Canvas, PxRect};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant, SystemTime};

/// The painted grid's resource.
pub const RESOURCE: &str = "terminal";
/// The grid's node, which takes the pane's keyboard focus when clicked.
pub const SCREEN: &str = "terminal-screen";
/// The grid's type size, in points.
pub const FONT_SIZE: f32 = 13.0;
/// Space around the grid, in points.
const PAD: f32 = 8.0;
/// Lines of history kept above the screen.
pub const SCROLLBACK: usize = 5_000;
/// The narrowest the pane is drawn, in points; a narrower window shows
/// only the button that brings it back.
pub const MIN_WIDTH: f32 = 320.0;
/// The widest the pane grows, in points.
pub const MAX_WIDTH: f32 = 960.0;
/// What the chat beside it keeps, at least, in points.
pub const CHAT_MIN: f32 = 360.0;
/// How often the Agents panel reads the agent lists.
pub const AGENTS_EVERY: Duration = Duration::from_secs(1);
/// The most agents the panel lists; the rest are counted.
pub const AGENTS_SHOWN: usize = 8;
/// The most output frames waiting for the window before the host is
/// asked to hold the rest.
const FRAMES: usize = 1024;
/// The most output bytes a second the shell sends the pane.
const RATE: u64 = 8 << 20;
/// Who opens and types into the shell: this window, the only client of its
/// host. Any 64-hex-character name will do; nothing outside the process
/// sees it.
const OWNER: &str = "0000000000000000000000000000000000000000000000000000000000000001";
/// The one folder the host lets a shell start in, by name.
const WORKSPACE: &str = "0000000000000000000000000000000000000000000000000000000000000002";

/// What the pane says when the shell can't start.
const NOT_STARTED: &str = "The shell didn't start. Try New shell.";

/// The window is this host's only client, and it may do anything.
struct Owner;

impl Rights for Owner {
    fn holds(&self, principal: &str, _: Right) -> bool {
        principal == OWNER
    }
}

/// A fresh request ID: 64 lowercase hexadecimal characters.
fn id() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// The shell the pane runs, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shell {
    /// The program, by absolute path.
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The folder it starts in.
    pub dir: PathBuf,
    /// Its `PATH`.
    pub path: String,
}

/// The shell for this person: `$SHELL` as a login shell (zsh on a Mac,
/// bash elsewhere, when it is unset), or `%ComSpec%` on Windows, starting
/// in `home`, with Coder's install folder first on `PATH` and the app's
/// own folder (`coder`'s, when the app carries one) last.
#[must_use]
pub fn shell(env: &dyn Fn(&str) -> Option<String>, home: &Path, coder: Option<&Path>) -> Shell {
    let windows = cfg!(windows);
    let absolute = |value: String| Some(PathBuf::from(value)).filter(|path| path.is_absolute());
    let program = if windows {
        env("ComSpec")
            .and_then(absolute)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\cmd.exe"))
    } else {
        env("SHELL").and_then(absolute).unwrap_or_else(|| {
            PathBuf::from(if cfg!(target_os = "macos") {
                "/bin/zsh"
            } else {
                "/bin/bash"
            })
        })
    };
    let args = if windows {
        Vec::new()
    } else {
        vec!["-l".to_owned()]
    };
    let separator = if windows { ";" } else { ":" };
    let installed = env("OPENAGENTS_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".openagents"))
        .join("bin");
    let mut parts: Vec<String> = vec![installed.display().to_string()];
    for part in env("PATH").unwrap_or_default().split(separator) {
        if !part.is_empty() && !parts.iter().any(|seen| seen == part) {
            parts.push(part.to_owned());
        }
    }
    if let Some(folder) = coder.and_then(Path::parent) {
        let folder = folder.display().to_string();
        if !parts.contains(&folder) {
            parts.push(folder);
        }
    }
    Shell {
        program,
        args,
        dir: home.to_path_buf(),
        path: parts.join(separator),
    }
}

/// A shell on a PTY, from this window's own host.
pub struct Pty {
    host: Host,
    terminal: TerminalRef,
    frames: mpsc::Receiver<Frame>,
}

impl std::fmt::Debug for Pty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pty")
            .field("terminal", &self.terminal.terminal)
            .finish_non_exhaustive()
    }
}

impl Pty {
    /// Opens `shell` at `rows` by `cols` and follows its output. `wake`
    /// runs whenever output arrives, off the window's thread.
    ///
    /// # Errors
    /// The shell can't start here; the text says why.
    pub fn open(
        shell: &Shell,
        rows: u16,
        cols: u16,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Pty, String> {
        let dir = shell
            .dir
            .canonicalize()
            .unwrap_or_else(|_| shell.dir.clone());
        let mut config = Config::new().workspace(WORKSPACE, dir);
        config.shell = shell.program.clone();
        config.shell_args = shell.args.clone();
        config.rate_max = RATE;
        config.terminals_max = 1;
        config.base_env.retain(|(name, _)| name != "PATH");
        config.base_env.push(("PATH".into(), shell.path.clone()));
        config
            .base_env
            .push(("OPENAGENTS_DESKTOP_TERMINAL".into(), "1".into()));
        let host = Host::new(config, Arc::new(Owner));
        let open = Open::new(id(), WORKSPACE, "", Launch::Shell, Size::new(rows, cols));
        let terminal = match host.open(OWNER, &open) {
            Ok((_, Value::Opened { terminal, .. })) => terminal,
            // The host's own words name its internals; the pane says it
            // plainly (#11031).
            Ok(_) | Err(_) => return Err(NOT_STARTED.into()),
        };
        let (sink, receiver) = host::channel(FRAMES);
        let attach = Attach::new(id(), terminal.clone(), Mode::Interact, 0, RATE);
        if host.attach(OWNER, &attach, Box::new(sink)).is_err() {
            return Err(NOT_STARTED.into());
        }
        let (sender, frames) = mpsc::channel();
        std::thread::Builder::new()
            .name("terminal-output".into())
            .spawn(move || {
                while let Ok(frame) = receiver.recv() {
                    if sender.send(frame).is_err() {
                        break;
                    }
                    wake();
                }
            })
            .map_err(|_| NOT_STARTED.to_owned())?;
        Ok(Pty {
            host,
            terminal,
            frames,
        })
    }

    /// Sends typed bytes to the shell, a long paste in pieces the host
    /// takes.
    pub fn write(&self, bytes: &[u8]) {
        for piece in bytes.chunks(coder_pty::wire::INPUT_MAX) {
            let _ = self.host.input(
                OWNER,
                &Input::new(id(), self.terminal.clone(), piece.to_vec()),
            );
        }
    }

    /// Tells the shell its new size.
    pub fn resize(&self, rows: u16, cols: u16) {
        let _ = self.host.resize(
            OWNER,
            &Resize::new(id(), self.terminal.clone(), Size::new(rows, cols)),
        );
    }

    /// The output that arrived since the last call.
    #[must_use]
    pub fn drain(&self) -> Vec<Frame> {
        self.frames.try_iter().collect()
    }
}

/// Where the shell stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Not started: a capture, or the window has not opened yet.
    Idle,
    Running,
    /// It ended; the text says how.
    Ended(String),
    /// It couldn't start; the text says why.
    Failed(String),
}

/// What a key typed into the pane does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Typed {
    /// These bytes go to the shell.
    Bytes(Vec<u8>),
    /// Paste the clipboard's text.
    Paste,
    /// Copy the screen's text.
    Copy,
    /// Not the pane's: the window handles it.
    Unhandled,
}

/// The bytes, or the clipboard action, a key typed into the pane stands
/// for. `key` is the window's name for it (`Enter`, `ArrowUp`, `a`),
/// `text` what it types. A Mac keeps Cmd for the window and sends Ctrl;
/// elsewhere Ctrl goes to the shell and Ctrl+Shift+C and V copy and paste.
#[must_use]
pub fn typed(
    vt: &Terminal,
    key: &str,
    text: Option<&str>,
    control: bool,
    command: bool,
    alt: bool,
    shift: bool,
) -> Typed {
    let mac = cfg!(target_os = "macos");
    let letter = |name: &str| key.eq_ignore_ascii_case(name);
    // The window reports Cmd or Ctrl as `command`; Cmd alone is the Mac's.
    if mac && command && !control {
        return if letter("v") {
            Typed::Paste
        } else if letter("c") {
            Typed::Copy
        } else {
            Typed::Unhandled
        };
    }
    if !mac && control && shift {
        if letter("v") {
            return Typed::Paste;
        }
        if letter("c") {
            return Typed::Copy;
        }
    }
    let modifiers = Modifiers {
        ctrl: control,
        alt,
        shift,
    };
    let named = match key {
        "Enter" => Some(Key::Enter),
        "Tab" if shift => Some(Key::BackTab),
        "Tab" => Some(Key::Tab),
        "Backspace" => Some(Key::Backspace),
        "Escape" => Some(Key::Escape),
        "ArrowUp" => Some(Key::Up),
        "ArrowDown" => Some(Key::Down),
        "ArrowLeft" => Some(Key::Left),
        "ArrowRight" => Some(Key::Right),
        "Home" => Some(Key::Home),
        "End" => Some(Key::End),
        "PageUp" => Some(Key::PageUp),
        "PageDown" => Some(Key::PageDown),
        "Insert" => Some(Key::Insert),
        "Delete" => Some(Key::Delete),
        "Space" => Some(Key::Char(' ')),
        _ => key
            .strip_prefix('F')
            .and_then(|n| n.parse::<u8>().ok())
            .filter(|n| (1..=24).contains(n))
            .map(Key::F),
    };
    if let Some(named) = named {
        // Shift alone changes only Tab; a shifted arrow stays an arrow
        // with its modifier, as xterm sends it.
        let modifiers = if matches!(named, Key::BackTab) {
            Modifiers {
                shift: false,
                ..modifiers
            }
        } else {
            modifiers
        };
        return Typed::Bytes(vt.key(named, modifiers));
    }
    let mut chars = key.chars();
    if control && let (Some(c), None) = (chars.next(), chars.next()) {
        return Typed::Bytes(vt.key(Key::Char(c.to_ascii_lowercase()), modifiers));
    }
    match text.filter(|text| !text.is_empty() && !text.chars().any(char::is_control)) {
        Some(text) => {
            let mut bytes = Vec::new();
            // Alt as Meta elsewhere; a Mac's Option types its own letters.
            let meta = alt && !mac;
            for c in text.chars() {
                bytes.extend(vt.key(
                    Key::Char(c),
                    Modifiers {
                        ctrl: false,
                        alt: meta,
                        shift: false,
                    },
                ));
            }
            Typed::Bytes(bytes)
        }
        None => Typed::Unhandled,
    }
}

/// One agent the panel lists, and the `coder` process that runs it.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentLine {
    pub pid: u32,
    pub row: agent_fleet::AgentRow,
}

impl AgentLine {
    /// The row's words: name, engine, status, time, tokens and cost.
    #[must_use]
    pub fn words(&self, now_ms: u64) -> String {
        let row = &self.row;
        let mut parts = vec![
            row.name.clone(),
            engine_name(&row.engine).to_owned(),
            row.status.word().to_owned(),
            agent_fleet::elapsed_words(row.elapsed_seconds(now_ms)),
        ];
        if row.tokens > 0 {
            parts.push(format!("{} tokens", agent_fleet::token_words(row.tokens)));
        }
        if let Some(cost) = row.cost_usd {
            parts.push(agent_fleet::dollars(cost));
        }
        parts.join(" · ")
    }
}

/// An engine as the panel names it.
#[must_use]
pub fn engine_name(engine: &str) -> &str {
    match engine {
        "codex" => "Codex",
        "claude" | "claude-code" => "Claude Code",
        "grok" | "grok-build" => "Grok Build",
        // Coder's own engine; its inner name is not shown (#11031).
        "microcoder" | "coder" => "Coder",
        other if crate::words::banned_in(other).is_empty() => other,
        _ => "Coder",
    }
}

/// Every running Coder's agents under `dir`, running ones first, then the
/// newest.
#[must_use]
pub fn agents(dir: &Path, alive: impl Fn(u32, SystemTime) -> bool) -> Vec<AgentLine> {
    let mut lines: Vec<AgentLine> = agent_fleet::board::read(dir, alive)
        .into_iter()
        .flat_map(|board| {
            let pid = board.pid;
            board
                .agents
                .into_iter()
                .map(move |row| AgentLine { pid, row })
        })
        .collect();
    lines.sort_by(|a, b| {
        let running = |line: &AgentLine| line.row.status == agent_fleet::Status::Running;
        running(b)
            .cmp(&running(a))
            .then(b.row.started_ms.cmp(&a.row.started_ms))
    });
    lines
}

/// Whether the `coder` that wrote a board still runs: asked of the system
/// on Unix, judged by the board's age elsewhere.
#[must_use]
pub fn alive(pid: u32, modified: SystemTime) -> bool {
    #[cfg(unix)]
    {
        let _ = modified;
        let Ok(pid) = i32::try_from(pid) else {
            return false;
        };
        // SAFETY: signal 0 only asks whether the process exists.
        let answer = unsafe { libc::kill(pid, 0) };
        answer == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        agent_fleet::board::alive_by_age(modified, SystemTime::now())
    }
}

/// The Terminal pane: its shell, its screen, and the Agents panel.
pub struct TerminalPane {
    /// Whether it shows beside the chat.
    pub open: bool,
    /// Whether keys go to the shell.
    pub focused: bool,
    pub phase: Phase,
    vt: Terminal,
    pty: Option<Pty>,
    /// Lines scrolled back from the live screen; 0 follows the output.
    scroll: usize,
    revision: u64,
    fonts: Fonts,
    /// The pane's width in points, as the window last fit it.
    width: f32,
    /// The window's height in points, and its scale.
    window: (f32, f32),
    /// Where the agent lists are, when this computer has a home folder.
    board: Option<PathBuf>,
    agents: Vec<AgentLine>,
    agents_read: Option<Instant>,
    /// Whether a board's `coder` still runs ([`alive`]).
    pub alive: fn(u32, SystemTime) -> bool,
}

impl std::fmt::Debug for TerminalPane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalPane")
            .field("open", &self.open)
            .field("focused", &self.focused)
            .field("phase", &self.phase)
            .field("agents", &self.agents.len())
            .finish_non_exhaustive()
    }
}

/// The home folder: `HOME`, or `USERPROFILE` on Windows.
#[must_use]
pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
}

impl TerminalPane {
    /// A pane, shown, whose Agents panel reads the lists under `board`.
    #[must_use]
    pub fn new(board: Option<PathBuf>) -> Self {
        Self {
            open: true,
            focused: false,
            phase: Phase::Idle,
            vt: Terminal::new(24, 80, SCROLLBACK),
            pty: None,
            scroll: 0,
            revision: 1,
            fonts: Fonts::new(),
            width: 0.0,
            window: (840.0, 1.0),
            board,
            agents: Vec::new(),
            agents_read: None,
            alive,
        }
    }

    /// The pane for this person: agents from `~/.openagents/agents`.
    #[must_use]
    pub fn for_home() -> Self {
        Self::new(home().map(|home| agent_fleet::board::dir(&home.join(".openagents"))))
    }

    fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// Starts `shell`, unless one runs. `wake` runs when output arrives.
    pub fn start(&mut self, shell: &Shell, wake: impl Fn() + Send + 'static) {
        if self.pty.is_some() {
            return;
        }
        let (rows, cols) = (self.vt.rows() as u16, self.vt.cols() as u16);
        match Pty::open(shell, rows, cols, wake) {
            Ok(pty) => {
                self.pty = Some(pty);
                self.phase = Phase::Running;
            }
            Err(why) => self.phase = Phase::Failed(why),
        }
        self.touch();
    }

    /// Ends the shell, so [`Self::start`] opens a new one on a clean
    /// screen.
    pub fn reset(&mut self) {
        self.pty = None;
        self.vt = Terminal::new(self.vt.rows(), self.vt.cols(), SCROLLBACK);
        self.scroll = 0;
        self.phase = Phase::Idle;
        self.touch();
    }

    /// Applies the output that arrived, and answers what the program asked
    /// of the terminal. Returns whether the screen changed.
    pub fn poll(&mut self) -> bool {
        let Some(pty) = &self.pty else {
            return false;
        };
        let frames = pty.drain();
        if frames.is_empty() {
            return false;
        }
        for frame in frames {
            self.apply(frame.body);
        }
        if let Some(pty) = &self.pty {
            let replies = self.vt.take_replies();
            pty.write(&replies);
        }
        self.touch();
        true
    }

    /// One frame of the shell's output.
    pub fn apply(&mut self, body: Body) {
        match body {
            Body::Output { data, .. } => {
                self.vt.feed(&data);
                // New output brings a scrolled-back screen to the live end.
                self.scroll = 0;
            }
            Body::Exit { exit, .. } => {
                self.phase = Phase::Ended(match (exit.cause, exit.code, exit.signal) {
                    (Cause::Exited, Some(0), _) | (Cause::Closed, ..) => {
                        "The shell exited.".to_owned()
                    }
                    (Cause::Exited, Some(code), _) => format!("The shell exited with code {code}."),
                    (Cause::Exited, _, Some(signal)) => {
                        format!("The shell ended on signal {signal}.")
                    }
                    (Cause::IdleExpired, ..) => "The shell ended after a long idle time.".into(),
                    _ => "The shell exited.".to_owned(),
                });
            }
            Body::Gap { .. } => self.vt.mark("[some output was too fast to show]"),
            Body::Detached { .. } => {
                if self.phase == Phase::Running {
                    self.phase = Phase::Ended("The shell exited.".into());
                }
            }
            _ => {}
        }
    }

    /// Reads the agent lists again when a second has passed. Returns
    /// whether the panel changed.
    pub fn refresh_agents(&mut self, now: Instant) -> bool {
        if !self.open
            || self
                .agents_read
                .is_some_and(|read| now.saturating_duration_since(read) < AGENTS_EVERY)
        {
            return false;
        }
        self.agents_read = Some(now);
        let lines = self
            .board
            .as_deref()
            .map_or_else(Vec::new, |dir| agents(dir, self.alive));
        if lines == self.agents {
            return false;
        }
        self.agents = lines;
        true
    }

    /// When the panel next reads the agent lists.
    #[must_use]
    pub fn next_wake(&self, now: Instant) -> Option<Instant> {
        self.open
            .then(|| self.agents_read.map_or(now, |read| read + AGENTS_EVERY))
    }

    /// The agents the panel lists.
    #[must_use]
    pub fn agent_lines(&self) -> &[AgentLine] {
        &self.agents
    }

    /// Asks the `coder` running agent `id` to stop it; it stops at the end
    /// of its current step, and the panel shows it then.
    pub fn stop(&mut self, pid: u32, id: &str) {
        if let Some(dir) = &self.board {
            let _ = agent_fleet::board::request_stop(dir, pid, id);
        }
        self.agents_read = None;
    }

    /// A key typed while the pane has focus. Bytes go straight to the
    /// shell; a clipboard action is the window's to carry out.
    pub fn key(
        &mut self,
        key: &str,
        text: Option<&str>,
        control: bool,
        command: bool,
        alt: bool,
        shift: bool,
    ) -> Typed {
        let typed = typed(&self.vt, key, text, control, command, alt, shift);
        if let Typed::Bytes(bytes) = &typed {
            self.send(bytes);
        }
        typed
    }

    /// Text an input method composed.
    pub fn commit(&mut self, text: &str) {
        let bytes: Vec<u8> = text
            .chars()
            .filter(|c| !c.is_control())
            .flat_map(|c| self.vt.key(Key::Char(c), Modifiers::NONE))
            .collect();
        self.send(&bytes);
    }

    /// Pastes `text`, bracketed when the program asked for that.
    pub fn paste(&mut self, text: &str) {
        let bytes = self.vt.paste(text);
        self.send(&bytes);
    }

    /// The screen's text, for Copy.
    #[must_use]
    pub fn text(&self) -> String {
        self.vt.text()
    }

    fn send(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(pty) = &self.pty {
            pty.write(bytes);
        }
        if self.scroll != 0 {
            self.scroll = 0;
            self.touch();
        }
    }

    /// Takes or leaves the keyboard.
    pub fn focus(&mut self, focused: bool) {
        if self.focused != focused {
            self.focused = focused;
            if let Some(pty) = &self.pty
                && let Some(report) = self.vt.focus(focused)
            {
                pty.write(&report);
            }
            self.touch();
        }
    }

    /// Fits the pane to `available` points of the content area: half of
    /// it, from [`MIN_WIDTH`] to [`MAX_WIDTH`], leaving the chat
    /// [`CHAT_MIN`]. Returns the width it takes, 0 when it is hidden or
    /// the window is too narrow.
    pub fn fit(&mut self, available: f32) -> f32 {
        let width = (available * 0.5)
            .clamp(MIN_WIDTH, MAX_WIDTH)
            .min(available - CHAT_MIN);
        self.width = if self.open && width >= MIN_WIDTH {
            width.floor()
        } else {
            0.0
        };
        self.width
    }

    /// The window's height in points and its scale.
    pub fn set_window(&mut self, height: f32, scale: f32) {
        self.window = (height, scale.max(0.5));
    }

    /// Changes whenever the screen should paint again.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.revision
    }

    /// The grid's size within `available` points.
    #[must_use]
    pub fn surface_size(&self, available: f32) -> (f32, f32) {
        let agents = (self.agents.len().min(AGENTS_SHOWN) as f32 + 2.0) * 30.0;
        (
            self.width.min(available).max(1.0),
            (self.window.0 - 38.0 - 56.0 - agents - 40.0).max(160.0),
        )
    }

    /// A click on the screen takes the keyboard; the wheel scrolls back
    /// through the history.
    pub fn input(&mut self, event: SurfaceInput) -> bool {
        match event {
            SurfaceInput::Down { .. } => {
                self.focus(true);
                true
            }
            SurfaceInput::Wheel { dy, .. } => {
                if self.vt.alternate_screen() {
                    return false;
                }
                let lines = (dy / (FONT_SIZE * LINE_EM)).round() as i64;
                let back = self.vt.scrollback_len() as i64;
                let scroll = (self.scroll as i64 + lines).clamp(0, back) as usize;
                if scroll != self.scroll {
                    self.scroll = scroll;
                    self.touch();
                }
                true
            }
            _ => false,
        }
    }

    fn font(weight: Weight) -> Font {
        Font {
            size: FONT_SIZE,
            weight,
            family: FontFamily::PaperMono,
            italic: false,
            mono: true,
        }
    }

    /// Paints the screen into `rect`, sizing the grid (and the shell's
    /// PTY) to it first.
    pub fn paint(&mut self, frame: &mut Canvas, rect: PxRect) {
        let scale = self.window.1;
        let look = openagents_chat_app::visual::current();
        let (background, foreground) = (look.panel, look.text);
        frame.fill(rect, 8.0 * scale, background);
        let regular = Self::font(Weight::Regular);
        let cell_w = self.fonts.advance("M", regular).max(1.0);
        let line = (FONT_SIZE * LINE_EM).round();
        let cols = ((rect.w / scale - 2.0 * PAD) / cell_w)
            .floor()
            .clamp(10.0, 1000.0) as usize;
        let rows = ((rect.h / scale - 2.0 * PAD) / line)
            .floor()
            .clamp(2.0, 500.0) as usize;
        if (rows, cols) != (self.vt.rows(), self.vt.cols()) {
            self.vt.resize(rows, cols);
            if let Some(pty) = &self.pty {
                pty.resize(rows as u16, cols as u16);
            }
        }
        let (cell_px, line_px) = (cell_w * scale, line * scale);
        let left = rect.x + PAD * scale;
        let top = rect.y + PAD * scale;
        let back = self.vt.scrollback_len();
        let first = back.saturating_sub(self.scroll);
        let rows_shown: Vec<coder_vt::Row> = (0..self.vt.rows())
            .filter_map(|row| self.vt.line(first + row).cloned())
            .collect();
        for (index, row) in rows_shown.iter().enumerate() {
            let y = top + index as f32 * line_px;
            let mut col = 0usize;
            for run in row.runs() {
                let x = left + col as f32 * cell_px;
                let width = run.columns as f32 * cell_px;
                col += run.columns;
                let flags = run.attrs.flags;
                let mut fg = color(run.attrs.fg, foreground);
                let mut bg = match run.attrs.bg {
                    coder_vt::Color::Default => None,
                    other => Some(color(other, background)),
                };
                if flags.contains(Flags::INVERSE) {
                    let swapped = bg.unwrap_or(background);
                    bg = Some(fg);
                    fg = swapped;
                }
                if flags.contains(Flags::DIM) || flags.contains(Flags::MARKER) {
                    fg = mix(fg, background, 0.45);
                }
                if let Some(bg) = bg {
                    frame.fill(
                        PxRect {
                            x,
                            y,
                            w: width,
                            h: line_px,
                        },
                        0.0,
                        bg,
                    );
                }
                if flags.contains(Flags::HIDDEN) {
                    continue;
                }
                let trimmed = run.text.trim_start_matches(' ');
                let leading = run.text.len() - trimmed.len();
                let trimmed = trimmed.trim_end();
                if trimmed.is_empty() {
                    continue;
                }
                let font = if flags.contains(Flags::BOLD) {
                    Self::font(Weight::Bold)
                } else {
                    regular
                };
                let text_x = x + leading as f32 * cell_px;
                let paragraph = self.fonts.paragraph(trimmed, font, None);
                self.fonts.draw(
                    frame,
                    &paragraph,
                    text_x,
                    y,
                    paragraph.width,
                    TextAlign::Start,
                    scale,
                    fg,
                );
                if flags.contains(Flags::UNDERLINE) || run.attrs.link != 0 {
                    let under = y + line_px - 2.0 * scale;
                    frame.line(
                        (text_x, under),
                        (text_x + trimmed.chars().count() as f32 * cell_px, under),
                        scale,
                        fg,
                    );
                }
                if flags.contains(Flags::STRIKE) {
                    let middle = y + line_px / 2.0;
                    frame.line(
                        (text_x, middle),
                        (text_x + trimmed.chars().count() as f32 * cell_px, middle),
                        scale,
                        fg,
                    );
                }
            }
        }
        // The text caret, on the live screen while the program shows it.
        if self.scroll == 0 && self.vt.cursor_visible() && self.phase == Phase::Running {
            let (row, col) = self.vt.cursor();
            let caret = PxRect {
                x: left + col as f32 * cell_px,
                y: top + row as f32 * line_px,
                w: cell_px.max(1.0),
                h: line_px,
            };
            if self.focused {
                frame.fill(caret, 0.0, mix(foreground, background, 0.35));
            } else {
                frame.stroke(caret, 0.0, scale, mix(foreground, background, 0.4));
            }
        }
    }

    /// The pane beside `chat`: the chat and the pane side by side, or,
    /// while the pane is hidden or the window too narrow, the chat and a
    /// button that brings it back.
    #[must_use]
    pub fn beside(&self, chat: Node<Intent>) -> Node<Intent> {
        let side = if self.open && self.width > 0.0 {
            self.pane(agent_fleet::now_ms())
        } else {
            let mut rail = stack(
                "terminal-rail",
                Axis::Vertical,
                vec![button("terminal-show", "Terminal", Action::Show)],
            );
            rail.style.intrinsic_width = Some(true);
            rail
        };
        let mut split = stack("terminal-split", Axis::Horizontal, vec![chat, side]);
        split.style.fill_height = Some(true);
        split.style.gap_points = Some(8);
        split
    }

    fn status(&self) -> String {
        match &self.phase {
            Phase::Idle => "Starting a shell…".into(),
            Phase::Running if self.scroll > 0 => {
                format!("{} lines back. Type to return.", self.scroll)
            }
            Phase::Running => "Type coder to start Coder. The agents it starts show below.".into(),
            Phase::Ended(text) | Phase::Failed(text) => text.clone(),
        }
    }

    /// The pane: its header, the screen, and the Agents panel.
    #[must_use]
    pub fn pane(&self, now_ms: u64) -> Node<Intent> {
        let mut header = vec![
            text("terminal-title", "Terminal", TextRole::Heading),
            text("terminal-status", &self.status(), TextRole::Status),
        ];
        if matches!(self.phase, Phase::Ended(_) | Phase::Failed(_)) {
            header.push(button("terminal-restart", "New shell", Action::Restart));
        }
        header.push(button("terminal-hide", "Hide", Action::Hide));
        let mut screen = node(
            SCREEN,
            Element::Surface {
                label: "Terminal".into(),
                resource: RESOURCE.into(),
            },
        );
        screen.style.fill_height = Some(true);
        let mut children = vec![
            stack("terminal-header", Axis::Horizontal, header),
            screen,
            text("terminal-agents-title", "Agents", TextRole::Heading),
        ];
        if self.agents.is_empty() {
            children.push(text(
                "terminal-agents-none",
                "No agents running. Start one in coder with /agent ENGINE TASK.",
                TextRole::Status,
            ));
        }
        for line in self.agents.iter().take(AGENTS_SHOWN) {
            let key = format!("terminal-agent-{}-{}", line.pid, line.row.id);
            let mut row = vec![text(
                &format!("{key}-line"),
                &line.words(now_ms),
                TextRole::Body,
            )];
            if line.row.status == agent_fleet::Status::Running {
                row.push(button(
                    &format!("{key}-stop"),
                    "Stop",
                    Action::Stop {
                        pid: line.pid,
                        agent: line.row.id.clone(),
                    },
                ));
            }
            children.push(stack(&key, Axis::Horizontal, row));
        }
        if self.agents.len() > AGENTS_SHOWN {
            children.push(text(
                "terminal-agents-more",
                &format!("and {} more", self.agents.len() - AGENTS_SHOWN),
                TextRole::Status,
            ));
        }
        let mut pane = stack("terminal-pane", Axis::Vertical, children);
        pane.style.intrinsic_width = Some(true);
        pane.style.fill_height = Some(true);
        pane.style.gap_points = Some(6);
        pane
    }
}

fn node(key: &str, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

fn stack(key: &str, axis: Axis, children: Vec<Node<Intent>>) -> Node<Intent> {
    node(key, Element::Stack { axis, children })
}

fn text(key: &str, value: &str, role: TextRole) -> Node<Intent> {
    let mut node = node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    );
    let step = match role {
        TextRole::Heading => oa_tokens::typography::text::SM,
        TextRole::Status => oa_tokens::typography::text::XS,
        _ => oa_tokens::typography::text::SM,
    };
    node.style.text_size = Some(step.size as u16);
    node.style.line_height = Some(step.line_height as u16);
    node
}

fn button(key: &str, label: &str, action: Action) -> Node<Intent> {
    let mut node = node(
        key,
        Element::Button {
            label: label.into(),
            enabled: true,
            icon: None,
            shortcut: None,
            intent: Intent::Terminal { action },
        },
    );
    node.style.radius = Some(6);
    node.style.min_height = Some(26);
    node.style.button_padding = Some([10, 3]);
    node
}

/// `a` mixed toward `b` by `t`.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let channel = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color {
        red: channel(a.red, b.red),
        green: channel(a.green, b.green),
        blue: channel(a.blue, b.blue),
        alpha: a.alpha,
    }
}

/// A grid color on the screen: the default is `default`, the 256 indexed
/// colors are xterm's, and 24-bit colors are as given.
#[must_use]
pub fn color(color: coder_vt::Color, default: Color) -> Color {
    match color {
        coder_vt::Color::Default => default,
        coder_vt::Color::Rgb(r, g, b) => Color::rgb(r, g, b),
        coder_vt::Color::Indexed(index) => {
            let (r, g, b) = indexed(index);
            Color::rgb(r, g, b)
        }
    }
}

/// xterm's 256 colors.
#[must_use]
pub fn indexed(index: u8) -> (u8, u8, u8) {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 49, 49),
        (13, 188, 121),
        (229, 229, 16),
        (36, 114, 200),
        (188, 63, 188),
        (17, 168, 205),
        (229, 229, 229),
        (102, 102, 102),
        (241, 76, 76),
        (35, 209, 139),
        (245, 245, 67),
        (59, 142, 234),
        (214, 112, 214),
        (41, 184, 219),
        (255, 255, 255),
    ];
    match index {
        0..=15 => BASE[usize::from(index)],
        16..=231 => {
            let i = index - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            (level(i / 36), level(i / 6 % 6), level(i % 6))
        }
        _ => {
            let gray = 8 + 10 * (index - 232);
            (gray, gray, gray)
        }
    }
}

#[cfg(test)]
#[path = "terminal_pane_tests.rs"]
mod tests;
