//! Shared terminal application state, driven by application inputs and injected services.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::input::{KeyCode, KeyIn, Logical, ModifiersState};

use crate::{control, copy, keys, layout, mouse, pty, select, smart, stats};
use layout::{Axis, Direction, Layout, PaneId, Rect};
use pty::{Program, Session, Sessions};
use select::Selection;

/// The overlay's help line.
pub const HELP: &str = "Enter runs a command or asks OpenAgents   Ctrl+B then: j/k blocks · y copy · d collapse · r rerun · % \" split · arrows focus · x close · z zoom · c n p tabs · o OpenAgents Terminal · [ copy · / search · ? stats · Esc world   Ctrl+` world";

/// The longest a frame spends applying output, across panes.
pub const UPDATE_BUDGET: Duration = Duration::from_millis(3);
/// The most output bytes one pane applies in a frame.
pub const PANE_BYTES: usize = 256 * 1024;

pub struct Pane {
    pub session: Session,
    pub label: String,
    /// The program ended and the pane says so.
    pub ended: bool,
    /// Lines scrolled back from the bottom.
    pub scroll: usize,
    /// Presentation revision for a mount's cached drawing.
    pub render_revision: u64,
    /// Selected text, by absolute line, so it follows scrolling output.
    pub selection: Option<Selection>,
    /// Bells seen, and when the last one lit the title bar.
    pub bells: u64,
    pub flash: Option<Instant>,
    /// The agent typing into this pane, when one drives it; its title
    /// says `driven by NAME`. Any key the person presses here takes the
    /// pane back at once (`docs/terminal/smart-terminal.md`, "Agents
    /// attached to panes").
    pub typist: Option<String>,
    /// The person took this pane back from its typist; the agent's driver
    /// reads and clears it.
    pub taken_back: bool,
}

pub struct Tab {
    pub layout: Layout,
    pub zoomed: bool,
}

/// The terminal overlay and its sessions.
pub struct Application {
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
    pub sessions: Option<Sessions>,
    pub panes: BTreeMap<PaneId, Pane>,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub next: PaneId,
    pub smart: smart::Smart,
    pub prefix: bool,
    pub mods: ModifiersState,
    /// Where the hotbar button sits this frame, in pixels, when it shows.
    pub button: Option<Rect>,
    pub pointer: [f32; 2],
    /// The panes' area when last drawn, in pixels.
    pub area: Rect,
    pub cell: [f32; 2],
    /// A one-line notice, such as why the first pane is a shell.
    pub notice: Option<String>,
    /// What the first pane runs; the login shell by default.
    pub first: Option<Program>,
    /// The focus the world last saw, so it can notice a change made over
    /// the socket.
    pub seen_focused: bool,
    /// Frame, parse, and latency instruments (the prefix, then `?`).
    pub stats: stats::Stats,
    /// Which pane the next frame's output starts after the focused one.
    pub turn: usize,
    pub mouse: mouse::Mouse,
    pub copy: Option<copy::Copy>,
    /// The pane last told it has focus (mode 1004).
    pub focus_sent: Option<PaneId>,
    /// When a key last reached a pane, which restarts the cursor blink.
    pub typed: Instant,
    /// The fixed sheet, the default view of a mount (`paper`).
    pub paper: crate::paper::Paper,
}

impl std::fmt::Debug for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Application")
            .field("open", &self.open)
            .field("focused", &self.focused)
            .field("panes", &self.panes.len())
            .finish_non_exhaustive()
    }
}

impl Application {
    #[must_use]
    pub fn new(sessions: Sessions) -> Self {
        Application {
            open: false,
            focused: false,
            option_as_meta: true,
            clipboard_writes: true,
            copied: None,
            sessions: Some(sessions),
            panes: BTreeMap::new(),
            tabs: Vec::new(),
            active: 0,
            next: 1,
            smart: smart::Smart::default(),
            prefix: false,
            mods: ModifiersState::empty(),
            button: None,
            pointer: [-1.0, -1.0],
            area: Rect::new(0.0, 0.0, 960.0, 600.0),
            cell: [9.0, 18.0],
            notice: None,
            first: None,
            seen_focused: false,
            stats: stats::Stats::default(),
            turn: 0,
            mouse: mouse::Mouse::default(),
            copy: None,
            focus_sent: None,
            typed: Instant::now(),
            paper: crate::paper::Paper::default(),
        }
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

    pub fn focus_id(&self) -> Option<PaneId> {
        self.tabs.get(self.active).map(|t| t.layout.focus())
    }

    pub fn sessions(&mut self) -> &Sessions {
        self.sessions
            .as_ref()
            .expect("a session transport is installed")
    }

    /// The program the first pane runs.
    pub fn first_program(&mut self) -> Program {
        if let Some(first) = &self.first {
            return first.clone();
        }
        let first = Program::Shell;
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

    pub fn grid_size(&self, rect: Rect) -> (u16, u16) {
        let bar = self.cell[1] + 4.0;
        layout::cells(rect.w - 8.0, rect.h - bar - 6.0, self.cell[0], self.cell[1])
    }

    /// The panes of the active tab that show, with their rectangles.
    pub fn shown(&self) -> Vec<(PaneId, Rect)> {
        let Some(tab) = self.tabs.get(self.active) else {
            return Vec::new();
        };
        if tab.zoomed {
            vec![(tab.layout.focus(), self.area)]
        } else {
            tab.layout.rects(self.area)
        }
    }

    pub fn rect_of(&self, pane: PaneId) -> Option<Rect> {
        self.shown()
            .into_iter()
            .find(|(id, _)| *id == pane)
            .map(|(_, rect)| rect)
    }

    pub fn spawn(&mut self, program: &Program, rows: u16, cols: u16) -> Option<PaneId> {
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
                        render_revision: 0,
                        selection: None,
                        bells: 0,
                        flash: None,
                        typist: None,
                        taken_back: false,
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
    pub fn new_tab(&mut self, program: &Program) {
        let (rows, cols) = self.grid_size(self.area);
        if let Some(id) = self.spawn(program, rows, cols) {
            self.tabs.push(Tab {
                layout: Layout::new(id),
                zoomed: false,
            });
            self.active = self.tabs.len() - 1;
        }
    }

    /// Opens a tab whose one pane, a login shell, the agent `typist`
    /// drives, and shows it as panes without taking focus from the world.
    /// Returns the pane, or `None` when the shell did not start.
    pub fn open_typist(&mut self, typist: &str) -> Option<PaneId> {
        let (rows, cols) = self.grid_size(self.area);
        let id = self.spawn(&Program::Shell, rows, cols)?;
        if let Some(pane) = self.panes.get_mut(&id) {
            pane.typist = Some(typist.to_string());
        }
        self.tabs.push(Tab {
            layout: Layout::new(id),
            zoomed: false,
        });
        self.active = self.tabs.len() - 1;
        self.open = true;
        self.paper.on = false;
        Some(id)
    }

    /// Shows pane `id`'s tab and focuses the pane within it, without
    /// giving the overlay keyboard focus. Returns false when no such pane
    /// is open.
    pub fn show_pane(&mut self, id: PaneId) -> bool {
        let Some(index) = self
            .tabs
            .iter()
            .position(|t| t.layout.panes().contains(&id))
        else {
            return false;
        };
        self.active = index;
        self.tabs[index].layout.set_focus(id);
        self.open = true;
        self.paper.on = false;
        true
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
                let program = self.program_from(program)?;
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
            Request::Press { name } => {
                use crate::input::{KeyCode, Logical, NamedKey};
                let (code, named) = match name.as_str() {
                    "enter" => (KeyCode::Enter, NamedKey::Enter),
                    "up" => (KeyCode::ArrowUp, NamedKey::ArrowUp),
                    "escape" => (KeyCode::Escape, NamedKey::Escape),
                    _ => return Err(format!("press takes enter, up, or escape, not `{name}`")),
                };
                if self.smart.pending.is_some() {
                    return Err("a pending proposal waits for a key on the keyboard".into());
                }
                let mut key = crate::KeyIn {
                    code,
                    logical: Logical::Named(named),
                    text: None,
                    plain: None,
                    pressed: true,
                    repeat: false,
                    synthetic: false,
                };
                // The press goes to the focused pane even while the window
                // is in the background.
                let (open, focused) = (self.open, self.focused);
                self.open = true;
                self.focused = true;
                self.key(&key);
                key.pressed = false;
                self.key(&key);
                (self.open, self.focused) = (open, focused);
                Ok(serde_json::json!({ "pressed": name }))
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
                    "input": self.paper.on.then(|| &self.paper.input),
                    "input_route": self.paper.on.then(|| match self.paper_route() {
                        Some(crate::route::Route::Ask) => "ask",
                        Some(crate::route::Route::Shell) => "shell",
                        None => "empty",
                    }),
                    "running": self.paper_running(),
                    // What Enter would do with the prompt line: shell or ask.
                    "routing": self.routing(id).map(|decision| decision.label()),
                    "notice": self.notice,
                    "draft": self.smart.draft.as_ref().filter(|draft| draft.pane == id).map(|draft| &draft.text),
                    "pending": self.smart.pending.as_ref().filter(|(pane, _)| *pane == id).map(|(_, key)| &self.smart.book.entries[key].proposal.command),
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
            "socket": serde_json::Value::Null,
        })
    }

    /// The bytes a named key sends: `enter`, `tab`, `escape`, `up`,
    /// `f5`, `space`, or a modifier chord such as `ctrl-c` or `alt-x`.
    pub fn key_named(&mut self, name: &str) -> Result<Vec<u8>, String> {
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
    pub fn remove(&mut self, id: PaneId) {
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

    pub fn focused_pane(&mut self) -> Option<&mut Pane> {
        let id = self.focus_id()?;
        self.panes.get_mut(&id)
    }

    /// Sends `bytes` to pane `id`'s program.
    pub fn send_to(&mut self, id: PaneId, bytes: &[u8]) {
        if let (Some(pane), Some(sessions)) = (self.panes.get(&id), &self.sessions) {
            sessions.input(&pane.session, bytes);
        }
    }

    pub fn send(&mut self, bytes: &[u8]) {
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
        if self.paper.on {
            self.paper_paste(text);
            return;
        }
        let focus = self.focus_id();
        if let Some(draft) = &mut self.smart.draft {
            if Some(draft.pane) == focus {
                draft
                    .text
                    .extend(text.chars().filter(|ch| !ch.is_control()));
                draft.text.truncate(draft.text.floor_char_boundary(8192));
                return;
            }
        }
        if self
            .smart
            .pending
            .as_ref()
            .is_some_and(|(pane, _)| Some(*pane) == focus)
        {
            return;
        }
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

    pub fn set_clipboard(&mut self, text: String) {
        self.copied = Some(text.clone());
        if let Err(error) = self.sessions().0.copy(&text) {
            self.notice = Some(error);
        }
    }

    /// Handles a key. Returns whether the overlay took it; when it did,
    /// the world must not see it.
    pub fn key(&mut self, key: &KeyIn) -> bool {
        if matches!(key.code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if key.pressed
                && (key.repeat || key.synthetic)
                && (self.smart.pending.is_some() || self.smart.draft.is_some())
            {
                return true;
            }
            let was_down = self.smart.enter_down;
            self.smart.enter_down = key.pressed;
            if was_down
                && key.pressed
                && (self.smart.pending.is_some() || self.smart.draft.is_some())
            {
                return true;
            }
        }
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
        // A key the person presses in a pane an agent drives takes it
        // back before the key does anything else.
        if !key.synthetic
            && !is_modifier(key.code)
            && let Some(pane) = self.focused_pane()
            && pane.typist.take().is_some()
        {
            pane.taken_back = true;
            pane.render_revision += 1;
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
            if let Some(text) = self.sessions().0.clipboard() {
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
        if self.paper.on {
            return self.paper_key(key);
        }
        if key.logical == Logical::Named(crate::input::NamedKey::F8) {
            self.paper.on = true;
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
        if self.smart_key(key) {
            return true;
        }
        if let Some(bytes) = self.encode(key) {
            self.send(&bytes);
        }
        true
    }

    /// Bytes a key sends to the focused pane's program.
    pub fn encode(&mut self, key: &KeyIn) -> Option<Vec<u8>> {
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
    pub fn command(&mut self, key: &KeyIn) {
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
            Some('a') => self.ask(String::new()),
            Some('j') => self.block_move(false),
            Some('k') => self.block_move(true),
            Some('y') => self.copy_block(),
            Some('d') => self.collapse_block(),
            Some('r') => self.rerun_block(),
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
            Some('o') => match self.sessions().0.thread_program() {
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

    pub fn scroll_focused(&mut self, lines: f32) {
        if let Some(pane) = self.focused_pane() {
            scroll(pane, lines);
        }
    }

    /// The whole overlay's rectangle, header and help included.
    pub fn bounds(&self) -> Rect {
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
        self.smart_tick();
        self.report_focus();
        if self.paper.on {
            // The sheet's transcript region is the program's whole grid.
            self.paper_tick();
            let (rows, cols) = self.paper.grid;
            if let Some(id) = self.focus_id()
                && let (Some(pane), Some(sessions)) = (self.panes.get_mut(&id), &self.sessions)
            {
                sessions.resize(&mut pane.session, rows, cols);
            }
        } else {
            for (id, rect) in self.shown() {
                let (rows, cols) = self.grid_size(rect);
                if let (Some(pane), Some(sessions)) = (self.panes.get_mut(&id), &self.sessions) {
                    sessions.resize(&mut pane.session, rows, cols);
                }
            }
        }
        if measure {
            self.stats.add_update(started.elapsed());
        }
    }

    /// Tells panes that asked (mode 1004) when they gain or lose focus.
    pub fn report_focus(&mut self) {
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
        // A grid of panes is the panes view, not the sheet.
        self.paper.on = false;
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

impl Drop for Application {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(crate) fn scroll(pane: &mut Pane, lines: f32) {
    let most = pane.session.vt.scrollback_len();
    let next = pane.scroll as f32 + lines;
    pane.scroll = (next.round().max(0.0) as usize).min(most);
}

/// The rows a pane shows, `scroll` lines back from the bottom.

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

impl Application {
    /// The program a command line names: nothing for the shell, else a
    /// program found as an absolute path or on `PATH`, with its arguments.
    fn program_from(&self, words: &[String]) -> Result<Program, String> {
        let Some((name, args)) = words.split_first() else {
            return Ok(Program::Shell);
        };
        let path = std::path::Path::new(name);
        let program = if path.is_absolute() {
            path.is_file()
                .then(|| path.to_path_buf())
                .ok_or_else(|| format!("{name} is not a file"))?
        } else {
            self.sessions
                .as_ref()
                .expect("transport")
                .0
                .resolve(name)
                .ok_or_else(|| format!("{name} was not found on PATH"))?
        };
        Ok(Program::Command {
            program,
            args: args.to_vec(),
            label: words.join(" "),
        })
    }
}

impl Application {
    pub fn fit_metrics(&mut self, cell: [f32; 2], size: [f32; 2]) {
        self.cell = cell;
        self.area = Self::area_for(size, cell);
    }
}
