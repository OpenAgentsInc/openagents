//! The in-world terminal: an overlay over the running world with any
//! number of terminal panes in a split layout, each a real program on a
//! PTY of this computer drawn from its `coder-vt` grid.
//!
//! The first pane runs OpenAgents Terminal (`openagents terminal`) when an
//! `openagents` binary is found, and the login shell otherwise. While the
//! overlay has focus every key goes to the focused pane; a tmux-like
//! prefix, Ctrl+B, splits, moves focus, closes, zooms, and opens tabs.
//! Ctrl+` or Cmd+T (or the prefix, then Esc) gives focus back to the
//! world. Read `docs/verse/in-world-terminal.md`.
//!
//! Sessions live in an in-process `coder-pty` host. Hiding the overlay
//! keeps them; exiting Verse ends every process group.

pub use crate::terminal_control as control;
pub mod draw;
pub mod layout;
pub mod pty;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use coder_ui::theme::Intensity;
use winit::keyboard::{Key as Logical, KeyCode, ModifiersState, NamedKey};

use crate::ui::{Atlas, UiBatch, UiVertex};
use layout::{Axis, Direction, Layout, PaneId, Rect};
use pty::{Program, Session, Sessions};

/// One key event, as the overlay reads it.
#[derive(Clone, Debug)]
pub struct KeyIn {
    pub code: KeyCode,
    pub logical: Logical,
    /// What the key typed, with the platform's modifiers applied.
    pub text: Option<String>,
    pub pressed: bool,
}

/// The overlay's help line.
pub const HELP: &str = "Ctrl+B then  % \" split · arrows focus · x close · z zoom · c n p tabs · o OpenAgents Terminal · Esc world   Ctrl+` world";

struct Pane {
    session: Session,
    label: String,
    /// The program ended and the pane says so.
    ended: bool,
    /// Lines scrolled back from the bottom.
    scroll: usize,
    /// The last drawn vertices and what they were drawn from.
    cache: Option<(CacheKey, Vec<UiVertex>)>,
}

#[derive(Clone, Copy, PartialEq)]
struct CacheKey {
    generation: u64,
    scroll: usize,
    rect: [u32; 4],
    focused: bool,
    title: u64,
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
    /// The control socket, when one listens.
    control: Option<control::Listener>,
    /// The focus the world last saw, so it can notice a change made over
    /// the socket.
    seen_focused: bool,
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
            control: None,
            seen_focused: false,
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
        let id = self.tabs.get(self.active)?.layout.focus();
        self.panes.get(&id).map(|pane| pane.session.vt.text())
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

    fn grid_size(&self, rect: Rect) -> (u16, u16) {
        let bar = self.cell[1] + 4.0;
        layout::cells(rect.w - 8.0, rect.h - bar - 6.0, self.cell[0], self.cell[1])
    }

    fn rect_of(&self, pane: PaneId) -> Option<Rect> {
        let tab = self.tabs.get(self.active)?;
        if tab.zoomed {
            return (tab.layout.focus() == pane).then_some(self.area);
        }
        tab.layout
            .rects(self.area)
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

    /// Listens for requests on the control socket at `path`, so
    /// `openagents verse terminal` can drive this overlay.
    ///
    /// # Errors
    /// When the socket cannot be bound.
    pub fn listen(&mut self, path: &std::path::Path) -> Result<(), String> {
        self.control = Some(control::Listener::bind(path)?);
        Ok(())
    }

    /// Where the control socket listens, when it does.
    #[must_use]
    pub fn control_path(&self) -> Option<&std::path::Path> {
        self.control.as_ref().map(control::Listener::path)
    }

    /// Whether focus changed since the last call: the world stops the
    /// character when the socket gave the overlay focus.
    pub fn focus_changed(&mut self) -> Option<bool> {
        if self.seen_focused == self.focused {
            return None;
        }
        self.seen_focused = self.focused;
        Some(self.focused)
    }

    /// Answers the requests the control socket holds. Runs between frames.
    fn serve(&mut self) {
        let mut served = 0;
        while served < 32 {
            let Some((request, reply)) = self.control.as_ref().and_then(control::Listener::next)
            else {
                return;
            };
            let value = match self.apply(&request) {
                Ok(mut value) => {
                    if let Some(map) = value.as_object_mut() {
                        map.insert("ok".into(), serde_json::Value::Bool(true));
                    }
                    value
                }
                Err(error) => serde_json::json!({ "ok": false, "error": error }),
            };
            let _ = reply.try_send(value);
            served += 1;
        }
    }

    /// Applies one control request and describes the result.
    ///
    /// # Errors
    /// When the request names a pane, key, axis, or action that does not
    /// exist, or a program that cannot be found.
    pub fn apply(&mut self, request: &control::Request) -> Result<serde_json::Value, String> {
        use control::Request;
        match request {
            Request::Status => Ok(self.status()),
            Request::Open => {
                self.open = true;
                self.focused = true;
                self.ensure_started();
                if self.tabs.is_empty() {
                    return Err(self
                        .notice
                        .clone()
                        .unwrap_or_else(|| "the first pane did not start".into()));
                }
                Ok(self.status())
            }
            Request::Hide => {
                self.open = false;
                self.focused = false;
                Ok(self.status())
            }
            Request::Split { axis, program } => {
                let axis = match axis.as_str() {
                    "cols" | "columns" | "vertical" | "v" => Axis::Columns,
                    "rows" | "horizontal" | "h" => Axis::Rows,
                    other => return Err(format!("axis is rows or cols, not `{other}`")),
                };
                let program = program_from(program)?;
                let before = self.panes.len();
                self.open = true;
                self.focused = true;
                self.ensure_started();
                if self.panes.len() == before {
                    self.split(axis, &program);
                }
                if self.panes.len() == before {
                    return Err(self
                        .notice
                        .clone()
                        .unwrap_or_else(|| "the pane did not start".into()));
                }
                Ok(self.status())
            }
            Request::Focus { direction, pane } => {
                if let Some(id) = pane {
                    let Some(index) = self.tabs.iter().position(|t| t.layout.panes().contains(id))
                    else {
                        return Err(format!("no pane {id}"));
                    };
                    self.active = index;
                    self.tabs[index].layout.set_focus(*id);
                } else if let Some(direction) = direction {
                    let direction = match direction.as_str() {
                        "left" => Direction::Left,
                        "right" => Direction::Right,
                        "up" => Direction::Up,
                        "down" => Direction::Down,
                        other => {
                            return Err(format!(
                                "direction is left, right, up, or down, not `{other}`"
                            ));
                        }
                    };
                    let area = self.area;
                    let Some(tab) = self.tabs.get_mut(self.active) else {
                        return Err("no pane is open".into());
                    };
                    if !tab.layout.move_focus(direction, area) {
                        return Err(format!("no pane {direction:?} of the focused one"));
                    }
                } else {
                    return Err("focus needs a direction or a pane".into());
                }
                self.open = true;
                self.focused = true;
                Ok(self.status())
            }
            Request::Close => {
                if self.tabs.is_empty() {
                    return Err("no pane is open".into());
                }
                self.close_focused();
                Ok(self.status())
            }
            Request::Send { text } => {
                if self.focused_pane().is_none() {
                    return Err("no pane is open".into());
                }
                self.paste(text);
                Ok(serde_json::json!({ "sent": text.len() }))
            }
            Request::Key { name } => {
                let bytes = self.key_named(name)?;
                self.send(&bytes);
                Ok(serde_json::json!({ "key": name, "bytes": bytes.len() }))
            }
            Request::Read { pane } => {
                let id = match pane {
                    Some(id) => *id,
                    None => self
                        .tabs
                        .get(self.active)
                        .map(|t| t.layout.focus())
                        .ok_or("no pane is open")?,
                };
                let pane = self.panes.get(&id).ok_or_else(|| format!("no pane {id}"))?;
                let (row, col) = pane.session.vt.cursor();
                Ok(serde_json::json!({
                    "pane": id,
                    "label": pane.label,
                    "text": pane.session.vt.text(),
                    "generation": pane.session.vt.generation(),
                    "cursor": [row, col],
                    "exited": pane.session.exited,
                }))
            }
            Request::Tab { action } => {
                match action.as_str() {
                    "new" => {
                        self.open = true;
                        self.focused = true;
                        let program = self.first_program();
                        self.new_tab(&program);
                    }
                    "next" if !self.tabs.is_empty() => {
                        self.active = (self.active + 1) % self.tabs.len();
                    }
                    "prev" if !self.tabs.is_empty() => {
                        self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
                    }
                    "next" | "prev" => return Err("no tab is open".into()),
                    other => {
                        return Err(format!("tab action is new, next, or prev, not `{other}`"));
                    }
                }
                Ok(self.status())
            }
            Request::Zoom => {
                let Some(tab) = self.tabs.get_mut(self.active) else {
                    return Err("no pane is open".into());
                };
                tab.zoomed = !tab.zoomed;
                Ok(self.status())
            }
        }
    }

    /// The overlay, its tabs, and its panes, as JSON.
    #[must_use]
    pub fn status(&self) -> serde_json::Value {
        let focus = self.tabs.get(self.active).map(|t| t.layout.focus());
        let tabs: Vec<serde_json::Value> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                serde_json::json!({
                    "index": index,
                    "active": index == self.active,
                    "zoomed": tab.zoomed,
                    "focus": tab.layout.focus(),
                    "panes": tab.layout.panes(),
                })
            })
            .collect();
        let panes: Vec<serde_json::Value> = self
            .panes
            .iter()
            .map(|(id, pane)| {
                let tab = self.tabs.iter().position(|t| t.layout.panes().contains(id));
                let rect = tab
                    .filter(|t| *t == self.active)
                    .and_then(|_| self.rect_of(*id))
                    .map(|r| [r.x, r.y, r.w, r.h]);
                serde_json::json!({
                    "id": id,
                    "tab": tab,
                    "label": pane.label,
                    "cwd": pane.session.cwd,
                    "rows": pane.session.vt.rows(),
                    "cols": pane.session.vt.cols(),
                    "focused": focus == Some(*id),
                    "exited": pane.session.exited,
                    "rect": rect,
                })
            })
            .collect();
        serde_json::json!({
            "open": self.open,
            "focused": self.focused,
            "active_tab": self.active,
            "tabs": tabs,
            "panes": panes,
            "notice": self.notice,
            "socket": self.control_path(),
        })
    }

    /// The bytes a named key sends: `enter`, `tab`, `escape`, `up`,
    /// `f5`, `space`, or a modifier chord such as `ctrl-c` or `alt-x`.
    fn key_named(&mut self, name: &str) -> Result<Vec<u8>, String> {
        use coder_vt::{Key, Modifiers};
        let mut modifiers = Modifiers {
            ctrl: false,
            alt: false,
            shift: false,
        };
        let lower = name.to_ascii_lowercase();
        let mut parts: Vec<&str> = lower.split(['-', '+']).filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            return Err("key needs a name".into());
        }
        let last = parts.pop().unwrap_or_default();
        for part in parts {
            match part {
                "ctrl" | "control" | "c" => modifiers.ctrl = true,
                "alt" | "meta" | "m" => modifiers.alt = true,
                "shift" | "s" => modifiers.shift = true,
                other => return Err(format!("unknown modifier `{other}`")),
            }
        }
        let key = match last {
            "enter" | "return" | "cr" => Key::Enter,
            "tab" => Key::Tab,
            "backtab" => Key::BackTab,
            "backspace" | "bs" => Key::Backspace,
            "escape" | "esc" => Key::Escape,
            "up" => Key::Up,
            "down" => Key::Down,
            "left" => Key::Left,
            "right" => Key::Right,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" | "pgup" => Key::PageUp,
            "pagedown" | "pgdn" => Key::PageDown,
            "insert" => Key::Insert,
            "delete" | "del" => Key::Delete,
            "space" => Key::Char(' '),
            f if f.len() >= 2
                && f.starts_with('f')
                && f[1..].chars().all(|c| c.is_ascii_digit()) =>
            {
                let n: u8 = f[1..]
                    .parse()
                    .map_err(|_| format!("unknown key `{name}`"))?;
                if !(1..=12).contains(&n) {
                    return Err(format!("function keys are f1 through f12, not `{name}`"));
                }
                Key::F(n)
            }
            c if c.chars().count() == 1 => Key::Char(c.chars().next().unwrap_or(' ')),
            _ => return Err(format!("unknown key `{name}`")),
        };
        let pane = self.focused_pane().ok_or("no pane is open")?;
        Ok(pane.session.vt.key(key, modifiers))
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
        let Some(id) = self.tabs.get(self.active).map(|t| t.layout.focus()) else {
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
        let id = self.tabs.get(self.active)?.layout.focus();
        self.panes.get_mut(&id)
    }

    fn send(&mut self, bytes: &[u8]) {
        let Some(id) = self.tabs.get(self.active).map(|t| t.layout.focus()) else {
            return;
        };
        if let (Some(pane), Some(sessions)) = (self.panes.get_mut(&id), &self.sessions) {
            pane.scroll = 0;
            sessions.input(&pane.session, bytes);
        }
    }

    /// Types `text` into the focused pane as a paste.
    pub fn paste(&mut self, text: &str) {
        let bytes = match self.focused_pane() {
            Some(pane) => pane.session.vt.paste(text),
            None => return,
        };
        self.send(&bytes);
    }

    /// Handles a key. Returns whether the overlay took it; when it did,
    /// the world must not see it.
    pub fn key(&mut self, key: &KeyIn) -> bool {
        let ctrl = self.mods.control_key();
        let cmd = self.mods.super_key();
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
        if cmd {
            match key.code {
                KeyCode::KeyT => self.focused = false,
                KeyCode::KeyV => {
                    if let Some(text) = clipboard() {
                        self.paste(&text);
                    }
                }
                _ => {}
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
        if self.mods.shift_key() && matches!(key.code, KeyCode::PageUp | KeyCode::PageDown) {
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
        use coder_vt::{Key, Modifiers};
        let modifiers = Modifiers {
            ctrl: self.mods.control_key(),
            alt: self.mods.alt_key(),
            shift: self.mods.shift_key(),
        };
        let named = match &key.logical {
            Logical::Named(named) => match named {
                NamedKey::Enter => Some(Key::Enter),
                NamedKey::Tab if modifiers.shift => Some(Key::BackTab),
                NamedKey::Tab => Some(Key::Tab),
                NamedKey::Backspace => Some(Key::Backspace),
                NamedKey::Escape => Some(Key::Escape),
                NamedKey::ArrowUp => Some(Key::Up),
                NamedKey::ArrowDown => Some(Key::Down),
                NamedKey::ArrowLeft => Some(Key::Left),
                NamedKey::ArrowRight => Some(Key::Right),
                NamedKey::Home => Some(Key::Home),
                NamedKey::End => Some(Key::End),
                NamedKey::PageUp => Some(Key::PageUp),
                NamedKey::PageDown => Some(Key::PageDown),
                NamedKey::Insert => Some(Key::Insert),
                NamedKey::Delete => Some(Key::Delete),
                NamedKey::Space => Some(Key::Char(' ')),
                NamedKey::F1 => Some(Key::F(1)),
                NamedKey::F2 => Some(Key::F(2)),
                NamedKey::F3 => Some(Key::F(3)),
                NamedKey::F4 => Some(Key::F(4)),
                NamedKey::F5 => Some(Key::F(5)),
                NamedKey::F6 => Some(Key::F(6)),
                NamedKey::F7 => Some(Key::F(7)),
                NamedKey::F8 => Some(Key::F(8)),
                NamedKey::F9 => Some(Key::F(9)),
                NamedKey::F10 => Some(Key::F(10)),
                NamedKey::F11 => Some(Key::F(11)),
                NamedKey::F12 => Some(Key::F(12)),
                _ => None,
            },
            _ => None,
        };
        let pane = self.focused_pane()?;
        let vt = &pane.session.vt;
        if let Some(named) = named {
            // Shift alone is in the key itself for these.
            let modifiers = Modifiers {
                shift: modifiers.shift && !matches!(named, Key::BackTab | Key::Char(' ')),
                ..modifiers
            };
            return Some(vt.key(named, modifiers));
        }
        if modifiers.ctrl {
            let c = match &key.logical {
                Logical::Character(s) => s.chars().next(),
                _ => None,
            }?;
            return Some(vt.key(
                Key::Char(c),
                Modifiers {
                    shift: false,
                    ..modifiers
                },
            ));
        }
        let text = key
            .text
            .as_deref()
            .filter(|t| !t.is_empty())
            .or(match &key.logical {
                Logical::Character(s) => Some(s.as_str()),
                _ => None,
            })?;
        let mut bytes = Vec::new();
        if modifiers.alt && !cfg!(target_os = "macos") {
            bytes.push(0x1b);
        }
        bytes.extend_from_slice(text.as_bytes());
        Some(bytes)
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
                    let axis = self
                        .tabs
                        .get(self.active)
                        .and_then(|t| self.rect_of(t.layout.focus()))
                        .map_or(Axis::Columns, |r| {
                            if r.w >= r.h * 1.6 {
                                Axis::Columns
                            } else {
                                Axis::Rows
                            }
                        });
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

    /// A mouse press at `point` in pixels. Inside the overlay it focuses
    /// the pane there and the press is the overlay's; outside, focus goes
    /// back to the world.
    pub fn press(&mut self, point: [f32; 2]) -> bool {
        if !self.open {
            return false;
        }
        if !self.bounds().contains(point) {
            self.focused = false;
            return false;
        }
        self.focused = true;
        if let Some(tab) = self.tabs.get_mut(self.active)
            && !tab.zoomed
            && let Some(id) = tab.layout.pane_at(self.area, point)
        {
            tab.layout.set_focus(id);
        }
        true
    }

    /// The mouse wheel at `point`: scrolls the pane under it back, or on
    /// the alternate screen sends arrow keys. Returns whether it was the
    /// overlay's.
    pub fn wheel(&mut self, point: [f32; 2], lines: f32) -> bool {
        if !self.open || !self.bounds().contains(point) {
            return false;
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return true;
        };
        let id = if tab.zoomed {
            Some(tab.layout.focus())
        } else {
            tab.layout.pane_at(self.area, point)
        };
        let Some(pane) = id.and_then(|id| self.panes.get_mut(&id)) else {
            return true;
        };
        if pane.session.vt.alternate_screen() {
            let key = if lines > 0.0 {
                coder_vt::Key::Up
            } else {
                coder_vt::Key::Down
            };
            let count = (lines.abs() * 3.0).round().clamp(1.0, 30.0) as usize;
            let bytes: Vec<u8> = (0..count)
                .flat_map(|_| pane.session.vt.key(key, coder_vt::Modifiers::NONE))
                .collect();
            if let Some(sessions) = &self.sessions {
                sessions.input(&pane.session, &bytes);
            }
        } else {
            scroll(pane, lines * 3.0);
        }
        true
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

    /// Applies every session's output, removes panes whose program ended,
    /// and fits each visible grid to its rectangle.
    pub fn tick(&mut self) {
        self.serve();
        let Some(sessions) = &self.sessions else {
            return;
        };
        // A pane whose program ended stays, with a note, until a key
        // closes it, so a program that fails to start shows why.
        for pane in self.panes.values_mut() {
            sessions.pump(&mut pane.session);
            if let Some(exit) = &pane.session.exited
                && !pane.ended
            {
                pane.ended = true;
                let note = format!("[{} {exit}: press any key to close this pane]", pane.label);
                pane.session.vt.mark(&note);
            }
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let rects: Vec<(PaneId, Rect)> = if tab.zoomed {
            vec![(tab.layout.focus(), self.area)]
        } else {
            tab.layout.rects(self.area)
        };
        for (id, rect) in rects {
            let (rows, cols) = self.grid_size(rect);
            if let (Some(pane), Some(sessions)) = (self.panes.get_mut(&id), &self.sessions) {
                sessions.resize(&mut pane.session, rows, cols);
            }
        }
    }

    /// Draws the overlay over a window of `size` pixels with `atlas`, and
    /// the terminal's hotbar button when it has a place.
    pub fn draw(&mut self, batch: &mut UiBatch, atlas: &Atlas, size: [f32; 2]) {
        self.draw_overlay(batch, atlas, size);
        if let Some(rect) = self.button {
            self.draw_button(batch, atlas, size, rect);
        }
    }

    /// Records where the pointer is, in pixels, for the button's card.
    pub fn pointer(&mut self, point: [f32; 2]) {
        self.pointer = point;
    }

    /// Whether `point` is on the terminal's hotbar button.
    #[must_use]
    pub fn on_button(&self, point: [f32; 2]) -> bool {
        self.button.is_some_and(|rect| rect.contains(point))
    }

    /// The button: a hotbar slot with a prompt icon and its key, and a
    /// card over it while the pointer rests on it.
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
        let x = (rect.x + rect.w - width).clamp(4.0, (size[0] - width - 4.0).max(4.0));
        let y = (rect.y - height - 8.0).max(4.0);
        batch.rect(atlas, x, y, width, height, draw::field(0.95));
        batch.frame(
            atlas,
            x,
            y,
            width,
            height,
            1.0,
            draw::white(Intensity::Half, 1.0),
        );
        let mut line_y = y + ch / 2.0;
        for (line, step) in lines {
            batch.text(atlas, x + cw, line_y, line, draw::white(step, 1.0));
            line_y += ch + 2.0;
        }
    }

    fn draw_overlay(&mut self, batch: &mut UiBatch, atlas: &Atlas, size: [f32; 2]) {
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
        let state = if self.prefix {
            "prefix: waiting for a command".to_owned()
        } else if self.focused {
            "typing goes to the focused pane".to_owned()
        } else {
            "the world has focus: click a pane or press Ctrl+`".to_owned()
        };
        batch.text(
            atlas,
            x + cw,
            y,
            &state,
            draw::white(
                if self.prefix {
                    Intensity::Full
                } else {
                    Intensity::Half
                },
                1.0,
            ),
        );
        let help_y = self.area.y + self.area.h + 6.0;
        let help = self.notice.clone().unwrap_or_else(|| HELP.to_owned());
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
        let rects: Vec<(PaneId, Rect)> = if tab.zoomed {
            vec![(focus, self.area)]
        } else {
            tab.layout.rects(self.area)
        };
        let zoomed = tab.zoomed;
        let pane_focused = self.focused;
        for (id, rect) in rects {
            let Some(pane) = self.panes.get_mut(&id) else {
                continue;
            };
            let focused = id == focus && pane_focused;
            let vt = &pane.session.vt;
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
            let key = CacheKey {
                generation: vt.generation(),
                scroll: pane.scroll,
                rect: [rect.x, rect.y, rect.w, rect.h].map(f32::to_bits),
                focused,
                title: hash(&(title.as_str(), detail.as_str())),
            };
            if let Some((cached, vertices)) = &pane.cache
                && *cached == key
            {
                batch.vertices.extend_from_slice(vertices);
                continue;
            }
            let mut own = UiBatch::default();
            let inner = draw::chrome(&mut own, atlas, rect, &title, &detail, focused);
            let rows = visible(vt, pane.scroll);
            let cursor = (pane.scroll == 0 && vt.cursor_visible()).then(|| vt.cursor());
            draw::grid(
                &mut own,
                atlas,
                [inner.x, inner.y],
                &draw::Grid {
                    rows,
                    cursor,
                    focused,
                },
            );
            batch.vertices.extend_from_slice(&own.vertices);
            pane.cache = Some((key, own.vertices));
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

/// The program a command line names: nothing for the shell, else a
/// program found as an absolute path or on `PATH`, with its arguments.
fn program_from(words: &[String]) -> Result<Program, String> {
    let Some((name, args)) = words.split_first() else {
        return Ok(Program::Shell);
    };
    let path = std::path::Path::new(name);
    let program = if path.is_absolute() {
        path.is_file()
            .then(|| path.to_path_buf())
            .ok_or_else(|| format!("{name} is not a file"))?
    } else {
        pty::candidates(name)
            .into_iter()
            .next()
            .ok_or_else(|| format!("{name} was not found on PATH"))?
    };
    Ok(Program::Command {
        program,
        args: args.to_vec(),
        label: words.join(" "),
    })
}

fn hash(value: &impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn scroll(pane: &mut Pane, lines: f32) {
    let most = pane.session.vt.scrollback().len();
    let next = pane.scroll as f32 + lines;
    pane.scroll = (next.round().max(0.0) as usize).min(most);
}

/// The rows a pane shows, `scroll` lines back from the bottom.
fn visible(vt: &coder_vt::Terminal, scroll: usize) -> Vec<&coder_vt::Row> {
    let screen = vt.screen();
    if scroll == 0 {
        return screen.iter().collect();
    }
    let back: Vec<&coder_vt::Row> = vt.scrollback().collect();
    let total = back.len() + screen.len();
    let end = total - scroll.min(back.len());
    let start = end.saturating_sub(screen.len());
    back.into_iter()
        .chain(screen.iter())
        .skip(start)
        .take(end - start)
        .collect()
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
    arboard::Clipboard::new().ok()?.get_text().ok()
}
