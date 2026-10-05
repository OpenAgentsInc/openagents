//! The in-world terminal: an overlay over the running world with any
//! number of terminal panes in a split layout, each a real program on a
//! PTY of this computer drawn from its `coder-vt` grid.
//!
//! The first pane runs OpenAgents Terminal (`openagents terminal`) when an
//! `openagents` binary is found, and the login shell otherwise. While the
//! overlay has focus every key goes to the focused pane; a tmux-like
//! prefix, Ctrl+B, splits, moves focus, closes, zooms, opens tabs, enters
//! copy mode and search, and shows performance numbers. Ctrl+` or Cmd+T
//! (or the prefix, then Esc) gives focus back to the world. A drag selects
//! text (a double-click words, a triple-click lines), Cmd+C (Ctrl+Shift+C
//! off macOS) copies it, and programs that ask for the mouse get it unless
//! Shift is held. Read `docs/verse/in-world-terminal.md`.
//!
//! Sessions live in an in-process `coder-pty` host. Hiding the overlay
//! keeps them; exiting Verse ends every process group. Each frame applies
//! output within a time and byte budget, focused pane first, so a pane
//! printing without pause cannot stall the world.

mod copy;
pub mod draw;
pub mod glyphs;
pub mod keys;
pub mod layout;
pub mod mouse;
pub mod pty;
pub mod select;
pub mod stats;
pub mod stress;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use coder_ui::theme::Intensity;
use winit::keyboard::{Key as Logical, KeyCode, ModifiersState};

use crate::ui::{Atlas, UiBatch, UiVertex};
use layout::{Axis, Direction, Layout, PaneId, Rect};
use pty::{Program, Session, Sessions};
use select::Selection;

/// One key event, as the overlay reads it.
#[derive(Clone, Debug)]
pub struct KeyIn {
    pub code: KeyCode,
    pub logical: Logical,
    /// What the key typed, with the platform's modifiers applied.
    pub text: Option<String>,
    /// What the key types with no modifiers, when the platform says; Option
    /// as Meta sends it.
    pub plain: Option<String>,
    pub pressed: bool,
}

/// The overlay's help line.
pub const HELP: &str = "Ctrl+B then  % \" split · arrows focus · x close · z zoom · c n p tabs · o OpenAgents Terminal · [ copy · / search · ? stats · Esc world   Ctrl+` world";

/// The longest a frame spends applying output, across panes.
pub const UPDATE_BUDGET: Duration = Duration::from_millis(3);
/// The most output bytes one pane applies in a frame.
pub const PANE_BYTES: usize = 256 * 1024;
/// Atlas rows the app reserves for fallback glyphs: about a thousand at
/// the desktop's glyph size.
pub const GLYPH_ROWS: u32 = 1024;
/// How long a cursor blink phase lasts.
const BLINK: Duration = Duration::from_millis(530);
/// How long a bell lights a pane's title bar.
const FLASH: Duration = Duration::from_millis(180);

struct Pane {
    session: Session,
    label: String,
    /// The program ended and the pane says so.
    ended: bool,
    /// Lines scrolled back from the bottom.
    scroll: usize,
    /// The last drawn vertices and what they were drawn from.
    cache: Option<(CacheKey, Vec<UiVertex>)>,
    /// Selected text, by absolute line, so it follows scrolling output.
    selection: Option<Selection>,
    /// Bells seen, and when the last one lit the title bar.
    bells: u64,
    flash: Option<Instant>,
}

#[derive(Clone, Copy, PartialEq)]
struct CacheKey {
    generation: u64,
    scroll: usize,
    rect: [u32; 4],
    focused: bool,
    title: u64,
    flash: bool,
    atlas: u64,
}

struct Tab {
    layout: Layout,
    zoomed: bool,
}

/// The terminal overlay and its sessions.
pub struct Overlay {
    /// The overlay draws.
    pub open: bool,
    /// Keys go to the focused pane.
    pub focused: bool,
    /// Option sends Escape before the key's own character on macOS, as
    /// Meta, instead of composing one. On by default; the prefix, then `m`,
    /// toggles it.
    pub option_as_meta: bool,
    /// The focused pane's program may write the clipboard (OSC 52); a
    /// background pane never may, and no program can read it.
    pub clipboard_writes: bool,
    /// The last text copied, from a selection or a program.
    pub copied: Option<String>,
    sessions: Option<Sessions>,
    panes: BTreeMap<PaneId, Pane>,
    tabs: Vec<Tab>,
    active: usize,
    next: PaneId,
    prefix: bool,
    mods: ModifiersState,
    /// Where the hotbar button sits this frame, in pixels, when it shows.
    pub button: Option<Rect>,
    pointer: [f32; 2],
    /// The panes' area when last drawn, in pixels.
    area: Rect,
    cell: [f32; 2],
    /// A one-line notice, such as why the first pane is a shell.
    notice: Option<String>,
    /// What the first pane runs: OpenAgents Terminal when found.
    first: Option<Program>,
    /// Frame, parse, and latency instruments (the prefix, then `?`).
    pub stats: stats::Stats,
    /// Glyphs the atlas lacks, from fallback fonts.
    fallback: Option<glyphs::Fallback>,
    /// Which pane the next frame's output starts after the focused one.
    turn: usize,
    mouse: mouse::Mouse,
    copy: Option<copy::Copy>,
    /// The pane last told it has focus (mode 1004).
    focus_sent: Option<PaneId>,
    /// When a key last reached a pane, which restarts the cursor blink.
    typed: Instant,
}

impl std::fmt::Debug for Overlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Overlay")
            .field("open", &self.open)
            .field("focused", &self.focused)
            .field("panes", &self.panes.len())
            .finish_non_exhaustive()
    }
}

impl Default for Overlay {
    fn default() -> Self {
        Overlay::new()
    }
}

impl Overlay {
    #[must_use]
    pub fn new() -> Self {
        Overlay {
            open: false,
            focused: false,
            option_as_meta: true,
            clipboard_writes: true,
            copied: None,
            sessions: None,
            panes: BTreeMap::new(),
            tabs: Vec::new(),
            active: 0,
            next: 1,
            prefix: false,
            mods: ModifiersState::empty(),
            button: None,
            pointer: [-1.0, -1.0],
            area: Rect::new(0.0, 0.0, 960.0, 600.0),
            cell: [9.0, 18.0],
            notice: None,
            first: None,
            stats: stats::Stats::default(),
            fallback: None,
            turn: 0,
            mouse: mouse::Mouse::default(),
            copy: None,
            focus_sent: None,
            typed: Instant::now(),
        }
    }

    /// An overlay whose sessions start in `root`, which is also their
    /// home, with `shell`, and whose first pane runs `first`. Tests and
    /// captures use it.
    #[must_use]
    pub fn with(root: &std::path::Path, shell: std::path::PathBuf, first: Program) -> Self {
        let mut overlay = Overlay::new();
        overlay.sessions = Some(Sessions::isolated(root, shell));
        overlay.first = Some(first);
        overlay
    }

    /// Opens the overlay with focus, or hides it when it is open.
    pub fn toggle(&mut self) {
        if self.open {
            self.open = false;
            self.focused = false;
        } else {
            self.open = true;
            self.focused = true;
        }
        self.prefix = false;
    }

    /// Records the modifier keys held.
    pub fn modifiers(&mut self, mods: ModifiersState) {
        self.mods = mods;
    }

    /// How many panes run, across tabs.
    #[must_use]
    pub fn panes(&self) -> usize {
        self.panes.len()
    }

    /// The focused pane's visible text, for tests and diagnostics.
    #[must_use]
    pub fn focused_text(&self) -> Option<String> {
        let id = self.focus_id()?;
        self.panes.get(&id).map(|pane| pane.session.vt.text())
    }

    /// The focused pane's selected text, if it has a selection.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let pane = self.panes.get(&self.focus_id()?)?;
        let selection = pane.selection.filter(|s| !s.empty())?;
        Some(selection.text(&pane.session.vt))
    }

    fn focus_id(&self) -> Option<PaneId> {
        self.tabs.get(self.active).map(|t| t.layout.focus())
    }

    fn sessions(&mut self) -> &Sessions {
        self.sessions.get_or_insert_with(Sessions::for_user)
    }

    /// The program the first pane runs.
    fn first_program(&mut self) -> Program {
        if let Some(first) = &self.first {
            return first.clone();
        }
        let first = Program::openagents_terminal().unwrap_or_else(|| {
            self.notice = Some(
                "no openagents with the terminal command was found (workspace build, PATH, ~/.openagents/bin); this pane is your login shell"
                    .into(),
            );
            Program::Shell
        });
        self.first = Some(first.clone());
        first
    }

    /// The button's square in pixels: right of `tray` (a hotbar's frame
    /// in pixels) when there is one, else in the bottom-right corner.
    #[must_use]
    pub fn button_for(size: [f32; 2], scale: f32, tray: Option<[f32; 4]>) -> Rect {
        match tray {
            Some([x, y, w, h]) => {
                let inset = h * 0.15;
                Rect::new(x + w + inset, y + inset, h - 2.0 * inset, h - 2.0 * inset)
            }
            None => {
                let edge = 44.0 * scale;
                let margin = 12.0 * scale;
                Rect::new(size[0] - edge - margin, size[1] - edge - margin, edge, edge)
            }
        }
    }

    /// The panes' area in a window of `size` pixels: most of the screen,
    /// above the hotbar.
    #[must_use]
    pub fn area_for(size: [f32; 2], cell: [f32; 2]) -> Rect {
        let x = (size[0] * 0.04).round();
        let top = (size[1] * 0.05).round() + cell[1] + 8.0;
        let bottom = (size[1] * 0.82).round() - cell[1] - 6.0;
        Rect::new(x, top, size[0] - 2.0 * x, (bottom - top).max(cell[1] * 4.0))
    }

    /// Sizes the overlay for a window of `size` pixels drawn with `atlas`
    /// before its first draw.
    pub fn fit(&mut self, atlas: &Atlas, size: [f32; 2]) {
        self.cell = draw::cell_size(atlas);
        self.area = Overlay::area_for(size, self.cell);
    }

    fn grid_size(&self, rect: Rect) -> (u16, u16) {
        let bar = self.cell[1] + 4.0;
        layout::cells(rect.w - 8.0, rect.h - bar - 6.0, self.cell[0], self.cell[1])
    }

    /// The panes of the active tab that show, with their rectangles.
    fn shown(&self) -> Vec<(PaneId, Rect)> {
        let Some(tab) = self.tabs.get(self.active) else {
            return Vec::new();
        };
        if tab.zoomed {
            vec![(tab.layout.focus(), self.area)]
        } else {
            tab.layout.rects(self.area)
        }
    }

    fn rect_of(&self, pane: PaneId) -> Option<Rect> {
        self.shown()
            .into_iter()
            .find(|(id, _)| *id == pane)
            .map(|(_, rect)| rect)
    }

    fn spawn(&mut self, program: &Program, rows: u16, cols: u16) -> Option<PaneId> {
        let sessions = self.sessions();
        let label = program.label(sessions.shell());
        match sessions.open(program, rows, cols) {
            Ok(session) => {
                let id = self.next;
                self.next += 1;
                self.panes.insert(
                    id,
                    Pane {
                        session,
                        label,
                        ended: false,
                        scroll: 0,
                        cache: None,
                        selection: None,
                        bells: 0,
                        flash: None,
                    },
                );
                Some(id)
            }
            Err(error) => {
                self.notice = Some(format!("the terminal did not start: {error}"));
                None
            }
        }
    }

    /// Opens a tab with one pane running `program`.
    fn new_tab(&mut self, program: &Program) {
        let (rows, cols) = self.grid_size(self.area);
        if let Some(id) = self.spawn(program, rows, cols) {
            self.tabs.push(Tab {
                layout: Layout::new(id),
                zoomed: false,
            });
            self.active = self.tabs.len() - 1;
        }
    }

    /// Starts the first tab when none runs.
    pub fn ensure_started(&mut self) {
        if self.tabs.is_empty() {
            let program = self.first_program();
            self.new_tab(&program);
        }
    }

    /// Splits the focused pane along `axis` with a new pane running
    /// `program`.
    pub fn split(&mut self, axis: Axis, program: &Program) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            self.new_tab(program);
            return;
        };
        tab.zoomed = false;
        let mut preview = tab.layout.clone();
        preview.split(axis, PaneId::MAX);
        let rect = preview
            .rects(self.area)
            .into_iter()
            .find(|(id, _)| *id == PaneId::MAX)
            .map_or(self.area, |(_, rect)| rect);
        let (rows, cols) = self.grid_size(rect);
        if let Some(id) = self.spawn(program, rows, cols)
            && let Some(tab) = self.tabs.get_mut(self.active)
        {
            tab.layout.split(axis, id);
        }
    }

    /// Ends the focused pane's program and removes the pane.
    pub fn close_focused(&mut self) {
        let Some(id) = self.focus_id() else {
            return;
        };
        if let (Some(pane), Some(sessions)) = (self.panes.get(&id), &self.sessions) {
            sessions.close(&pane.session);
        }
        self.remove(id);
    }

    /// Removes pane `id` from its tab, the tab when it was its last pane,
    /// and hides the overlay when no tab is left.
    fn remove(&mut self, id: PaneId) {
        self.panes.remove(&id);
        self.stats.forget(id);
        if self.copy.as_ref().is_some_and(|c| c.pane == id) {
            self.copy = None;
        }
        if self.focus_sent == Some(id) {
            self.focus_sent = None;
        }
        let Some(index) = self
            .tabs
            .iter()
            .position(|t| t.layout.panes().contains(&id))
        else {
            return;
        };
        if !self.tabs[index].layout.close(id) {
            self.tabs.remove(index);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len().saturating_sub(1);
            }
        }
        if self.tabs.is_empty() {
            self.open = false;
            self.focused = false;
        }
    }

    fn focused_pane(&mut self) -> Option<&mut Pane> {
        let id = self.focus_id()?;
        self.panes.get_mut(&id)
    }

    /// Sends `bytes` to pane `id`'s program.
    fn send_to(&mut self, id: PaneId, bytes: &[u8]) {
        if let (Some(pane), Some(sessions)) = (self.panes.get(&id), &self.sessions) {
            sessions.input(&pane.session, bytes);
        }
    }

    fn send(&mut self, bytes: &[u8]) {
        let Some(id) = self.focus_id() else {
            return;
        };
        self.typed = Instant::now();
        if let (Some(pane), Some(sessions)) = (self.panes.get_mut(&id), &self.sessions) {
            pane.scroll = 0;
            if self.stats.active() {
                self.stats.key_sent(id, pane.session.vt.generation());
            }
            sessions.input(&pane.session, bytes);
        }
    }

    /// Types `text` into the focused pane as a paste, bracketed when its
    /// program asked (mode 2004).
    pub fn paste(&mut self, text: &str) {
        let bytes = match self.focused_pane() {
            Some(pane) => pane.session.vt.paste(text),
            None => return,
        };
        self.send(&bytes);
    }

    /// Copies the focused pane's selection to the clipboard. Returns
    /// whether there was one.
    pub fn copy_selection(&mut self) -> bool {
        let Some(text) = self.selected_text() else {
            return false;
        };
        self.set_clipboard(text);
        true
    }

    /// Puts `text` on the system clipboard (never in tests) and keeps it
    /// as [`Overlay::copied`].
    fn set_clipboard(&mut self, text: String) {
        #[cfg(not(test))]
        if let Err(error) = arboard::Clipboard::new().and_then(|mut c| c.set_text(text.clone())) {
            self.notice = Some(format!("the clipboard refused the copy: {error}"));
        }
        self.copied = Some(text);
    }

    /// Handles a key. Returns whether the overlay took it; when it did,
    /// the world must not see it.
    pub fn key(&mut self, key: &KeyIn) -> bool {
        let ctrl = self.mods.control_key();
        let cmd = self.mods.super_key();
        let shift = self.mods.shift_key();
        // Ctrl+` opens the overlay from the world, or gives focus back.
        if key.code == KeyCode::Backquote && ctrl {
            if key.pressed {
                if self.focused {
                    self.focused = false;
                } else {
                    self.open = true;
                    self.focused = true;
                }
                self.prefix = false;
            }
            return true;
        }
        if !self.open || !self.focused {
            return false;
        }
        if !key.pressed {
            return true;
        }
        let macos = cfg!(target_os = "macos");
        let chord = |code: KeyCode| {
            key.code == code && ((macos && cmd) || (!macos && ctrl && shift && !cmd))
        };
        if chord(KeyCode::KeyC) {
            self.copy_selection();
            return true;
        }
        if chord(KeyCode::KeyV) {
            if let Some(text) = clipboard() {
                self.paste(&text);
            }
            return true;
        }
        if cmd {
            if key.code == KeyCode::KeyT {
                self.focused = false;
            }
            return true;
        }
        if is_modifier(key.code) {
            return true;
        }
        if self.prefix {
            self.prefix = false;
            self.command(key);
            return true;
        }
        if ctrl && key.code == KeyCode::KeyB {
            self.prefix = true;
            return true;
        }
        if self.copy.is_some() {
            self.copy_key(key);
            return true;
        }
        if shift && matches!(key.code, KeyCode::PageUp | KeyCode::PageDown) {
            let up = key.code == KeyCode::PageUp;
            let page = self.area.h / self.cell[1].max(1.0) / 2.0;
            self.scroll_focused(if up { page } else { -page });
            return true;
        }
        if self.focused_pane().is_some_and(|pane| pane.ended) {
            self.close_focused();
            return true;
        }
        if let Some(bytes) = self.encode(key) {
            self.send(&bytes);
        }
        true
    }

    /// Bytes a key sends to the focused pane's program.
    fn encode(&mut self, key: &KeyIn) -> Option<Vec<u8>> {
        let modifiers = coder_vt::Modifiers {
            ctrl: self.mods.control_key(),
            alt: self.mods.alt_key(),
            shift: self.mods.shift_key(),
        };
        let meta = self.option_as_meta;
        let pane = self.focused_pane()?;
        keys::encode(
            key,
            modifiers,
            &pane.session.vt,
            meta,
            cfg!(target_os = "macos"),
        )
    }

    /// Runs one prefix command.
    fn command(&mut self, key: &KeyIn) {
        let area = self.area;
        let typed = match &key.logical {
            Logical::Character(s) => s.chars().next(),
            _ => None,
        };
        let direction = match key.code {
            KeyCode::ArrowLeft => Some(Direction::Left),
            KeyCode::ArrowRight => Some(Direction::Right),
            KeyCode::ArrowUp => Some(Direction::Up),
            KeyCode::ArrowDown => Some(Direction::Down),
            _ => None,
        };
        if let Some(direction) = direction {
            if let Some(tab) = self.tabs.get_mut(self.active) {
                tab.layout.move_focus(direction, area);
            }
            return;
        }
        if key.code == KeyCode::Escape {
            self.focused = false;
            return;
        }
        if self.mods.control_key() && key.code == KeyCode::KeyB {
            self.send(&[0x02]);
            return;
        }
        match typed {
            Some('?') => self.stats.shown = !self.stats.shown,
            Some('[') => self.enter_copy(false),
            Some('/') => self.enter_copy(true),
            Some('m') => {
                self.option_as_meta = !self.option_as_meta;
                self.notice = Some(if self.option_as_meta {
                    "Option sends Meta (Escape and the key)".into()
                } else {
                    "Option composes characters".into()
                });
            }
            Some('%') => self.split(Axis::Columns, &Program::Shell),
            Some('"') => self.split(Axis::Rows, &Program::Shell),
            Some('x') => self.close_focused(),
            Some('z') => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.zoomed = !tab.zoomed;
                }
            }
            Some('c') => self.new_tab(&Program::Shell),
            Some('n') if !self.tabs.is_empty() => {
                self.active = (self.active + 1) % self.tabs.len();
            }
            Some('p') if !self.tabs.is_empty() => {
                self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
            }
            Some('o') => match Program::openagents_terminal() {
                Some(program) => {
                    let axis = self.focus_id().and_then(|id| self.rect_of(id)).map_or(
                        Axis::Columns,
                        |r| {
                            if r.w >= r.h * 1.6 {
                                Axis::Columns
                            } else {
                                Axis::Rows
                            }
                        },
                    );
                    self.split(axis, &program);
                }
                None => {
                    self.notice = Some("no openagents with the terminal command was found".into());
                }
            },
            _ => {}
        }
    }

    fn scroll_focused(&mut self, lines: f32) {
        if let Some(pane) = self.focused_pane() {
            scroll(pane, lines);
        }
    }

    /// The whole overlay's rectangle, header and help included.
    fn bounds(&self) -> Rect {
        let pad = self.cell[1] + 8.0;
        Rect::new(
            self.area.x - 6.0,
            self.area.y - pad,
            self.area.w + 12.0,
            self.area.h + 2.0 * pad,
        )
    }

    /// Applies the sessions' output within this frame's budget, notes
    /// panes whose program ended, honors clipboard writes, lights bells,
    /// reports focus changes, and fits each visible grid to its rectangle.
    pub fn tick(&mut self) {
        let Some(sessions) = &self.sessions else {
            return;
        };
        let started = Instant::now();
        let deadline = started + UPDATE_BUDGET;
        let measure = self.stats.active();
        // The focused pane first, for typing; then the rest in a turn that
        // moves each frame, so every busy pane gets its share.
        let focus = self.tabs.get(self.active).map(|t| t.layout.focus());
        let mut order: Vec<PaneId> = self.panes.keys().copied().collect();
        if !order.is_empty() {
            self.turn = (self.turn + 1) % order.len();
            order.rotate_left(self.turn);
        }
        if let Some(focus) = focus
            && let Some(at) = order.iter().position(|id| *id == focus)
        {
            let id = order.remove(at);
            order.insert(0, id);
        }
        let mut writes = Vec::new();
        for id in order {
            let Some(pane) = self.panes.get_mut(&id) else {
                continue;
            };
            let at = measure.then(Instant::now);
            let (_, bytes) = sessions.pump(&mut pane.session, PANE_BYTES, deadline);
            if let Some(at) = at {
                self.stats.add_parse(id, bytes, at.elapsed());
            }
            // A pane whose program ended stays, with a note, until a key
            // closes it, so a program that fails to start shows why.
            if let Some(exit) = &pane.session.exited
                && !pane.ended
            {
                pane.ended = true;
                let note = format!("[{} {exit}: press any key to close this pane]", pane.label);
                pane.session.vt.mark(&note);
            }
            if pane.session.vt.bells() != pane.bells {
                pane.bells = pane.session.vt.bells();
                pane.flash = Some(Instant::now());
            }
            if let Some(text) = pane.session.vt.take_clipboard() {
                writes.push((pane.label.clone(), text, Some(id) == focus));
            }
        }
        for (label, text, focused) in writes {
            if self.clipboard_writes && focused {
                self.notice = Some(format!(
                    "{label} copied {} characters to the clipboard",
                    text.chars().count()
                ));
                self.set_clipboard(text);
            } else {
                self.notice = Some(format!(
                    "refused a clipboard write from {label}, which does not have focus"
                ));
            }
        }
        self.report_focus();
        for (id, rect) in self.shown() {
            let (rows, cols) = self.grid_size(rect);
            if let (Some(pane), Some(sessions)) = (self.panes.get_mut(&id), &self.sessions) {
                sessions.resize(&mut pane.session, rows, cols);
            }
        }
        if measure {
            self.stats.add_update(started.elapsed());
        }
    }

    /// Tells panes that asked (mode 1004) when they gain or lose focus.
    fn report_focus(&mut self) {
        let now = if self.open && self.focused {
            self.focus_id()
        } else {
            None
        };
        if now == self.focus_sent {
            return;
        }
        let before = std::mem::replace(&mut self.focus_sent, now);
        for (id, focused) in [(before, false), (now, true)] {
            let Some(id) = id else { continue };
            let report = self
                .panes
                .get(&id)
                .and_then(|pane| pane.session.vt.focus(focused));
            if let Some(report) = report {
                self.send_to(id, &report);
            }
        }
    }

    /// The frame that drew the overlay finished; `started` is when it
    /// began on the main thread.
    pub fn frame_done(&mut self, started: Instant) {
        self.stats.frame_done(started);
    }

    /// Opens panes running `programs` in one tab, each splitting the
    /// largest pane along its longer side, and focuses the first. A stress
    /// run uses it.
    pub fn open_grid(&mut self, programs: &[Program]) -> Vec<PaneId> {
        let mut ids = Vec::new();
        let Some((first, rest)) = programs.split_first() else {
            return ids;
        };
        self.open = true;
        self.focused = true;
        self.new_tab(first);
        let Some(tab) = self.tabs.get(self.active) else {
            return ids;
        };
        ids.push(tab.layout.focus());
        for program in rest {
            let Some(tab) = self.tabs.get_mut(self.active) else {
                break;
            };
            let Some((largest, rect)) = tab
                .layout
                .rects(self.area)
                .into_iter()
                .max_by(|a, b| (a.1.w * a.1.h).total_cmp(&(b.1.w * b.1.h)))
            else {
                break;
            };
            tab.layout.set_focus(largest);
            let axis = if rect.w >= rect.h * 2.0 {
                Axis::Columns
            } else {
                Axis::Rows
            };
            self.split(axis, program);
            if let Some(tab) = self.tabs.get(self.active) {
                ids.push(tab.layout.focus());
            }
        }
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.layout.set_focus(ids[0]);
        }
        ids
    }

    /// The focused pane's grid generation, for a stress run's checks.
    #[must_use]
    pub fn focused_generation(&self) -> Option<u64> {
        let id = self.focus_id()?;
        self.panes.get(&id).map(|pane| pane.session.vt.generation())
    }

    /// Draws the overlay over a window of `size` pixels with `atlas`, and
    /// the terminal's hotbar button when it has a place. Characters the
    /// atlas lacks are rasterized into it from fallback fonts; a renderer
    /// uploads it again when its [`Atlas::revision`] changes.
    pub fn draw(&mut self, batch: &mut UiBatch, atlas: &mut Atlas, size: [f32; 2]) {
        self.draw_overlay(batch, atlas, size);
        if let Some(rect) = self.button {
            self.draw_button(batch, atlas, size, rect);
        }
    }

    /// Records where the pointer is, in pixels: for the button's card, a
    /// drag that selects, and programs that follow the mouse.
    pub fn pointer(&mut self, point: [f32; 2]) {
        self.pointer = point;
        self.moved(point);
    }

    /// Whether `point` is on the terminal's hotbar button.
    #[must_use]
    pub fn on_button(&self, point: [f32; 2]) -> bool {
        self.button.is_some_and(|rect| rect.contains(point))
    }

    /// The button: a hotbar slot with a prompt icon and its key, and a
    /// card beside it while the pointer rests on it. The card never covers
    /// the open overlay.
    fn draw_button(&self, batch: &mut UiBatch, atlas: &Atlas, size: [f32; 2], rect: Rect) {
        let [cw, ch] = draw::cell_size(atlas);
        let lit = self.open || rect.contains(self.pointer);
        batch.rect(atlas, rect.x, rect.y, rect.w, rect.h, draw::field(0.85));
        batch.frame(
            atlas,
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            (rect.w / 32.0).round().max(1.0),
            draw::white(
                if lit {
                    Intensity::Full
                } else {
                    Intensity::Half
                },
                1.0,
            ),
        );
        let icon = ">_";
        let x = rect.x + (rect.w - 2.0 * cw) / 2.0;
        let y = rect.y + (rect.h - ch) / 2.0;
        let color = draw::white(
            if lit {
                Intensity::Full
            } else {
                Intensity::ThreeQuarters
            },
            1.0,
        );
        batch.text(atlas, x, y, icon, color);
        batch.text(atlas, x + 1.0, y, icon, color);
        batch.text(
            atlas,
            rect.x + 3.0,
            rect.y + 1.0,
            "T",
            draw::white(Intensity::Half, 1.0),
        );
        if !rect.contains(self.pointer) {
            return;
        }
        let lines = [
            ("Terminal", Intensity::Full),
            (
                "Opens terminals over the world: OpenAgents Terminal first,",
                Intensity::ThreeQuarters,
            ),
            (
                "then splits with your shell. Ctrl+B is the prefix.",
                Intensity::ThreeQuarters,
            ),
            ("T or click · Ctrl+` focus", Intensity::Half),
        ];
        let width = lines
            .iter()
            .map(|(line, _)| line.chars().count())
            .max()
            .unwrap_or(0) as f32
            * cw
            + 2.0 * cw;
        let height = lines.len() as f32 * (ch + 2.0) + ch;
        let card = Overlay::card_for(
            size,
            rect,
            [width, height],
            self.open.then(|| self.bounds()),
        );
        batch.rect(atlas, card.x, card.y, width, height, draw::field(0.95));
        batch.frame(
            atlas,
            card.x,
            card.y,
            width,
            height,
            1.0,
            draw::white(Intensity::Half, 1.0),
        );
        let mut line_y = card.y + ch / 2.0;
        for (line, step) in lines {
            batch.text(atlas, card.x + cw, line_y, line, draw::white(step, 1.0));
            line_y += ch + 2.0;
        }
    }

    /// Where the button's card of `card` size goes in a window of `size`:
    /// above the button, unless that would cover `overlay`, the open
    /// overlay's bounds; then beside the button, right of it when there is
    /// room and left otherwise.
    #[must_use]
    pub fn card_for(size: [f32; 2], button: Rect, card: [f32; 2], overlay: Option<Rect>) -> Rect {
        let [width, height] = card;
        let x = (button.x + button.w - width).clamp(4.0, (size[0] - width - 4.0).max(4.0));
        let y = (button.y - height - 8.0).max(4.0);
        let above = Rect::new(x, y, width, height);
        let Some(overlay) = overlay else {
            return above;
        };
        let below_overlay = overlay.y + overlay.h + 4.0;
        if y >= below_overlay {
            return above;
        }
        // Beside the button, its bottom level with the button's, and below
        // the overlay where the window has room.
        let y = (button.y + button.h - height)
            .max(below_overlay)
            .min((size[1] - height - 4.0).max(4.0));
        let right = button.x + button.w + 8.0;
        let x = if right + width <= size[0] - 4.0 {
            right
        } else {
            (button.x - width - 8.0).max(4.0)
        };
        Rect::new(x, y, width, height)
    }

    fn draw_overlay(&mut self, batch: &mut UiBatch, atlas: &mut Atlas, size: [f32; 2]) {
        if !self.open {
            // Sessions keep running hidden; their output still applies.
            self.tick();
            return;
        }
        self.cell = draw::cell_size(atlas);
        self.area = Overlay::area_for(size, self.cell);
        self.ensure_started();
        if self.tabs.is_empty() {
            if let Some(notice) = &self.notice {
                eprintln!("verse: {notice}");
            }
            self.open = false;
            self.focused = false;
            return;
        }
        self.tick();
        if !self.open {
            return;
        }
        let drawing = self.stats.active().then(Instant::now);
        self.draw_panes(batch, atlas);
        if let Some(drawing) = drawing {
            self.stats.add_draw(drawing.elapsed());
        }
    }

    /// The header's state text.
    fn state_line(&self) -> String {
        if self.stats.shown {
            return self.stats.line();
        }
        if let Some(copy) = &self.copy {
            return copy.state();
        }
        if self.prefix {
            "prefix: waiting for a command".to_owned()
        } else if self.focused {
            "typing goes to the focused pane".to_owned()
        } else {
            "the world has focus: click a pane or press Ctrl+`".to_owned()
        }
    }

    fn draw_panes(&mut self, batch: &mut UiBatch, atlas: &mut Atlas) {
        let bounds = self.bounds();
        batch.rect(
            atlas,
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            draw::field(0.94),
        );
        batch.frame(
            atlas,
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            1.0,
            draw::white(
                if self.focused {
                    Intensity::Half
                } else {
                    Intensity::Quarter
                },
                1.0,
            ),
        );
        // The header: tabs, the prefix state, and focus.
        let [cw, ch] = self.cell;
        let mut x = bounds.x + cw;
        let y = bounds.y + 4.0;
        x += batch.text(atlas, x, y, "Terminal", draw::white(Intensity::Full, 1.0)) + 2.0 * cw;
        for index in 0..self.tabs.len() {
            let label = format!("{}", index + 1);
            let step = if index == self.active {
                Intensity::Full
            } else {
                Intensity::Half
            };
            if index == self.active {
                batch.rect(
                    atlas,
                    x - cw * 0.5,
                    y - 1.0,
                    cw * (label.len() as f32 + 1.0),
                    ch + 2.0,
                    draw::white(Intensity::Quarter, 0.6),
                );
            }
            x += batch.text(atlas, x, y, &label, draw::white(step, 1.0)) + cw;
        }
        let state = self.state_line();
        let columns = ((bounds.x + bounds.w - x - 2.0 * cw) / cw).max(0.0) as usize;
        let state: String = state.chars().take(columns).collect();
        batch.text(
            atlas,
            x + cw,
            y,
            &state,
            draw::white(
                if self.prefix || self.copy.is_some() || self.stats.shown {
                    Intensity::Full
                } else {
                    Intensity::Half
                },
                1.0,
            ),
        );
        let help_y = self.area.y + self.area.h + 6.0;
        let help = match (&self.copy, &self.notice) {
            (Some(_), _) => copy::HELP.to_owned(),
            (None, Some(notice)) => notice.clone(),
            (None, None) => HELP.to_owned(),
        };
        let columns = ((bounds.w - 2.0 * cw) / cw).max(0.0) as usize;
        let help: String = help.chars().take(columns).collect();
        batch.text(
            atlas,
            bounds.x + cw,
            help_y,
            &help,
            draw::white(Intensity::Half, 1.0),
        );

        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let focus = tab.layout.focus();
        let zoomed = tab.zoomed;
        let pane_focused = self.focused;
        let now = Instant::now();
        let blink_on = (now.duration_since(self.typed).as_millis() / BLINK.as_millis()) % 2 == 0;
        let copy_at = self.copy.as_ref().map(|c| (c.pane, c.cursor));
        let shown = self.shown();
        let fallback = self.fallback.get_or_insert_with(glyphs::Fallback::new);
        for (id, rect) in shown {
            let Some(pane) = self.panes.get_mut(&id) else {
                continue;
            };
            let focused = id == focus && pane_focused;
            let vt = &pane.session.vt;
            self.stats.drawn(id, vt.generation());
            let title = if vt.title().is_empty() {
                pane.label.clone()
            } else {
                format!("{} — {}", pane.label, vt.title())
            };
            let mut detail = pane.session.cwd.clone().unwrap_or_default();
            if pane.scroll > 0 {
                detail = format!("{detail}  [scrolled back {} lines]", pane.scroll);
            }
            if zoomed {
                detail = format!("{detail}  [zoomed]");
            }
            let flash = pane.flash.is_some_and(|at| now.duration_since(at) < FLASH);
            let rows = visible(vt, pane.scroll);
            let mut key = CacheKey {
                generation: vt.generation(),
                scroll: pane.scroll,
                rect: [rect.x, rect.y, rect.w, rect.h].map(f32::to_bits),
                focused,
                title: hash(&(title.as_str(), detail.as_str())),
                flash,
                atlas: atlas.revision(),
            };
            let cached = pane.cache.as_ref().is_some_and(|(k, _)| *k == key);
            if cached {
                if let Some((_, vertices)) = &pane.cache {
                    batch.vertices.extend_from_slice(vertices);
                }
            } else {
                // Characters the atlas lacks come from fallback fonts first.
                for row in &rows {
                    let unusual = |c: &&coder_vt::Cell| {
                        c.width != 0 && (!c.ch.is_ascii() || !c.combining.is_empty())
                    };
                    for cell in row.cells.iter().filter(unusual) {
                        let c = draw::substitute(cell.ch);
                        if !c.is_ascii() && !draw::shaped(c) {
                            fallback.ensure(atlas, c);
                        }
                        for &mark in &cell.combining {
                            fallback.ensure(atlas, mark);
                        }
                    }
                }
                key.atlas = atlas.revision();
                let mut own = UiBatch::default();
                let inner = draw::chrome(&mut own, atlas, rect, &title, &detail, focused, flash);
                draw::grid(
                    &mut own,
                    atlas,
                    [inner.x, inner.y],
                    &draw::Grid { rows: rows.clone() },
                );
                batch.vertices.extend_from_slice(&own.vertices);
                pane.cache = Some((key, own.vertices));
            }
            let inner = draw::inner(rect, [cw, ch]);
            let top = select::top(vt, pane.scroll);
            // The selection, over the text.
            if let Some(selection) = pane.selection.filter(|s| !s.empty()) {
                for (r, row) in rows.iter().enumerate() {
                    let line = select::absolute(vt, top + r);
                    if let Some((from, to)) = selection.columns(vt, line, row.cells.len()) {
                        batch.rect(
                            atlas,
                            inner.x + from as f32 * cw,
                            inner.y + r as f32 * ch,
                            (to - from) as f32 * cw,
                            ch,
                            draw::white(Intensity::ThreeQuarters, 0.35),
                        );
                    }
                }
            }
            // Copy mode's cursor, or the program's.
            if let Some((copy_pane, point)) = copy_at
                && copy_pane == id
            {
                if let Some(index) = select::index(vt, point.line)
                    && index >= top
                    && index < top + rows.len()
                {
                    let r = index - top;
                    batch.frame(
                        atlas,
                        inner.x + point.col as f32 * cw,
                        inner.y + r as f32 * ch,
                        cw,
                        ch,
                        2.0,
                        draw::white(Intensity::Full, 1.0),
                    );
                }
            } else if pane.scroll == 0 && vt.cursor_visible() {
                let style = vt.cursor_style();
                let (row, col) = vt.cursor();
                if !(focused && style.blink && !blink_on) {
                    draw::cursor(
                        batch,
                        atlas,
                        [inner.x + col as f32 * cw, inner.y + row as f32 * ch],
                        vt.row(row).and_then(|r| r.cells.get(col)),
                        style.shape,
                        focused,
                    );
                }
            }
        }
    }

    /// Ends every session's process group.
    pub fn shutdown(&mut self) {
        if let Some(sessions) = &self.sessions {
            sessions.shutdown();
        }
        self.panes.clear();
        self.tabs.clear();
        self.open = false;
        self.focused = false;
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn hash(value: &impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn scroll(pane: &mut Pane, lines: f32) {
    let most = pane.session.vt.scrollback_len();
    let next = pane.scroll as f32 + lines;
    pane.scroll = (next.round().max(0.0) as usize).min(most);
}

/// The rows a pane shows, `scroll` lines back from the bottom.
fn visible(vt: &coder_vt::Terminal, scroll: usize) -> Vec<&coder_vt::Row> {
    let top = select::top(vt, scroll);
    (top..top + vt.rows()).filter_map(|i| vt.line(i)).collect()
}

fn is_modifier(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
            | KeyCode::CapsLock
            | KeyCode::Fn
    )
}

fn clipboard() -> Option<String> {
    #[cfg(test)]
    return None;
    #[cfg(not(test))]
    arboard::Clipboard::new().ok()?.get_text().ok()
}
