//! The mouse over the overlay: focus, selection by drag, double-click,
//! and triple-click, hyperlinks under Cmd (Ctrl off macOS), the wheel, and
//! reports to programs that asked for the mouse. Shift keeps the mouse for
//! selection even then.

use std::time::{Duration, Instant};

use coder_vt::{MouseButton, MouseEvent, MouseKind, MouseMode};

use super::layout::{self, PaneId};
use super::select::{self, Point, Selection, Unit};
use super::{Overlay, scroll};

/// A mouse button the overlay reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

impl Button {
    fn vt(self) -> MouseButton {
        match self {
            Button::Left => MouseButton::Left,
            Button::Middle => MouseButton::Middle,
            Button::Right => MouseButton::Right,
        }
    }
}

/// Clicks this close together, on the same cell, count as one gesture.
const MULTI_CLICK: Duration = Duration::from_millis(400);

/// What the mouse is doing.
#[derive(Debug, Default)]
pub struct Mouse {
    /// A drag that selects in this pane.
    dragging: Option<PaneId>,
    /// A button held down and reported to this pane's program.
    reporting: Option<(PaneId, Button)>,
    /// The last press: when, where, and how many clicks it made.
    last_click: Option<(Instant, PaneId, Point, u8)>,
    /// The cell a motion report last named, so motion within it is quiet.
    last_cell: Option<(PaneId, usize, usize)>,
    /// Wheel movement not yet a whole line.
    wheel: f32,
}

impl Overlay {
    /// The pane under `point`, or the zoomed pane.
    fn pane_at(&self, point: [f32; 2]) -> Option<PaneId> {
        let tab = self.tabs.get(self.active)?;
        if tab.zoomed {
            return Some(tab.layout.focus());
        }
        tab.layout.pane_at(self.area, point)
    }

    /// The screen cell of pane `id` under `point`, clamped to its grid,
    /// and whether `point` was above or below the grid (-1, 0, or 1).
    fn cell_at(&self, id: PaneId, point: [f32; 2]) -> Option<(usize, usize, i8)> {
        let rect = self.rect_of(id)?;
        let pane = self.panes.get(&id)?;
        let inner = layout::inner(rect, self.cell);
        let [cw, ch] = self.cell;
        let (rows, cols) = (pane.session.vt.rows(), pane.session.vt.cols());
        let fy = (point[1] - inner.y) / ch;
        let edge = if fy < 0.0 {
            -1
        } else if fy >= rows as f32 {
            1
        } else {
            0
        };
        let row = (fy.max(0.0) as usize).min(rows - 1);
        let col = (((point[0] - inner.x) / cw).max(0.0) as usize).min(cols - 1);
        Some((row, col, edge))
    }

    /// The point `point` names in pane `id`'s scrollback and screen.
    fn point_at(&self, id: PaneId, point: [f32; 2]) -> Option<(Point, i8)> {
        let (row, col, edge) = self.cell_at(id, point)?;
        let pane = self.panes.get(&id)?;
        let vt = &pane.session.vt;
        let line = select::absolute(vt, select::top(vt, pane.scroll) + row);
        Some((Point { line, col }, edge))
    }

    /// Whether pane `id`'s program takes the mouse now: it asked, and
    /// Shift is not held.
    fn reports(&self, id: PaneId) -> bool {
        !self.mods.shift_key()
            && self
                .panes
                .get(&id)
                .is_some_and(|p| p.session.vt.mouse_mode() != MouseMode::Off)
    }

    fn report(&mut self, id: PaneId, kind: MouseKind, point: [f32; 2]) {
        let Some((row, col, _)) = self.cell_at(id, point) else {
            return;
        };
        let modifiers = coder_vt::Modifiers {
            ctrl: self.mods.control_key(),
            alt: self.mods.alt_key(),
            shift: self.mods.shift_key(),
        };
        let event = MouseEvent {
            kind,
            row,
            col,
            modifiers,
        };
        let bytes = self
            .panes
            .get(&id)
            .and_then(|pane| pane.session.vt.mouse(event));
        if let Some(bytes) = bytes {
            self.mouse.last_cell = Some((id, row, col));
            self.send_to(id, &bytes);
        }
    }

    /// A left press at `point` in pixels. Inside the overlay it focuses
    /// the pane there and the press is the overlay's; outside, focus goes
    /// back to the world.
    pub fn press(&mut self, point: [f32; 2]) -> bool {
        self.button(Button::Left, true, point)
    }

    /// A left release at `point`; true when the overlay had the press.
    pub fn release(&mut self, point: [f32; 2]) -> bool {
        self.button(Button::Left, false, point)
    }

    /// A mouse button at `point` in pixels. Returns whether it was the
    /// overlay's; when it was, the world must not see it.
    pub fn button(&mut self, button: Button, pressed: bool, point: [f32; 2]) -> bool {
        if !pressed {
            return self.lift(button, point);
        }
        if !self.open {
            return false;
        }
        if !self.bounds().contains(point) {
            self.focused = false;
            return false;
        }
        self.focused = true;
        let Some(id) = self.pane_at(point) else {
            return true;
        };
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.layout.set_focus(id);
        }
        if self.copy.as_ref().is_some_and(|c| c.pane != id) {
            self.copy = None;
        }
        // Cmd+click (Ctrl+click off macOS) opens a hyperlink.
        let link_held = if cfg!(target_os = "macos") {
            self.mods.super_key()
        } else {
            self.mods.control_key()
        };
        if button == Button::Left
            && link_held
            && let Some(target) = self.link_at(id, point)
        {
            self.open_link(&target);
            return true;
        }
        if self.reports(id) {
            self.mouse.reporting = Some((id, button));
            self.report(id, MouseKind::Press(button.vt()), point);
            return true;
        }
        if button != Button::Left {
            return true;
        }
        let Some((at, _)) = self.point_at(id, point) else {
            return true;
        };
        let now = Instant::now();
        let clicks = match self.mouse.last_click {
            Some((when, pane, last, n))
                if pane == id && last == at && now.duration_since(when) < MULTI_CLICK =>
            {
                n % 3 + 1
            }
            _ => 1,
        };
        self.mouse.last_click = Some((now, id, at, clicks));
        let unit = match clicks {
            1 => Unit::Char,
            2 => Unit::Word,
            _ => Unit::Line,
        };
        // Shift extends the selection there is.
        let pane = self.panes.get_mut(&id);
        if let Some(pane) = pane {
            pane.selection = match pane.selection {
                Some(mut selection) if self.mods.shift_key() && clicks == 1 => {
                    selection.head = at;
                    Some(selection)
                }
                _ => Some(Selection::at(at, unit)),
            };
        }
        self.mouse.dragging = Some(id);
        true
    }

    fn lift(&mut self, button: Button, point: [f32; 2]) -> bool {
        if let Some((id, held)) = self.mouse.reporting
            && held == button
        {
            self.mouse.reporting = None;
            self.report(id, MouseKind::Release(button.vt()), point);
            return true;
        }
        if button == Button::Left
            && let Some(id) = self.mouse.dragging.take()
        {
            if let Some(pane) = self.panes.get_mut(&id)
                && pane.selection.is_some_and(|s| s.empty())
            {
                pane.selection = None;
            }
            return true;
        }
        false
    }

    /// The pointer moved to `point`: a drag grows the selection, scrolling
    /// back or forward past the pane's edge, and programs that follow the
    /// mouse hear about it.
    pub fn moved(&mut self, point: [f32; 2]) {
        if !self.open {
            return;
        }
        if let Some(id) = self.mouse.dragging {
            let Some((at, edge)) = self.point_at(id, point) else {
                return;
            };
            if let Some(pane) = self.panes.get_mut(&id) {
                if edge != 0 {
                    scroll(pane, -f32::from(edge));
                }
                if let Some(selection) = &mut pane.selection {
                    selection.head = at;
                }
            }
            return;
        }
        let held = self.mouse.reporting;
        let id = match held {
            Some((id, _)) => id,
            None => match self.pane_at(point) {
                Some(id) if Some(id) == self.focus_id() && self.focused => id,
                _ => return,
            },
        };
        let mode = self.panes.get(&id).map(|p| p.session.vt.mouse_mode());
        let wants = match mode {
            Some(MouseMode::Motion) => true,
            Some(MouseMode::Drag) => held.is_some(),
            _ => false,
        };
        if !wants || self.mods.shift_key() {
            return;
        }
        let Some((row, col, _)) = self.cell_at(id, point) else {
            return;
        };
        if self.mouse.last_cell == Some((id, row, col)) {
            return;
        }
        let button = held.map(|(_, b)| b.vt());
        self.report(id, MouseKind::Motion(button), point);
    }

    /// The mouse wheel at `point`: reports to a program that asked for the
    /// mouse, sends arrow keys on another program's alternate screen, and
    /// scrolls back otherwise. Returns whether it was the overlay's.
    pub fn wheel(&mut self, point: [f32; 2], lines: f32) -> bool {
        if self.paper.on && self.open {
            // The sheet scrolls its transcript, a whole line at a time.
            let step = lines.round() as isize;
            self.paper.scroll = self.paper.scroll.saturating_add_signed(step);
            return true;
        }
        if !self.open || !self.bounds().contains(point) {
            return false;
        }
        let Some(id) = self.pane_at(point) else {
            return true;
        };
        if self.reports(id) {
            self.mouse.wheel += lines;
            let whole = self.mouse.wheel.trunc();
            self.mouse.wheel -= whole;
            let button = if whole > 0.0 {
                MouseButton::WheelUp
            } else {
                MouseButton::WheelDown
            };
            for _ in 0..(whole.abs() as usize).min(30) {
                self.report(id, MouseKind::Press(button), point);
            }
            return true;
        }
        let Some(pane) = self.panes.get_mut(&id) else {
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

    /// The hyperlink target of the cell under `point` in pane `id`.
    fn link_at(&self, id: PaneId, point: [f32; 2]) -> Option<String> {
        let (at, _) = self.point_at(id, point)?;
        let vt = &self.panes.get(&id)?.session.vt;
        let row = vt.line(select::index(vt, at.line)?)?;
        let link = row.cells.get(at.col)?.attrs.link;
        vt.link(link).map(str::to_owned)
    }

    /// Opens `target` with the system's handler, for the web, mail, and
    /// local files only.
    fn open_link(&mut self, target: &str) {
        if !link_allowed(target) {
            self.notice = Some(format!(
                "not opening {target}: only web, mail, and file links open"
            ));
            return;
        }
        self.notice = Some(format!("opening {target}"));
        if let Err(error) = self.sessions().0.open_link(target) {
            self.notice = Some(error);
        }
    }
}

/// Whether a program's hyperlink may be opened: web, mail, and file links,
/// and nothing that could run a command.
#[must_use]
pub fn link_allowed(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    ["https://", "http://", "mailto:", "file://"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
        && !target.chars().any(char::is_control)
}
