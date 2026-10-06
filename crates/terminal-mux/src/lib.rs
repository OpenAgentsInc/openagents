//! Optional TTY projection. Host attachments own authority, query replies,
//! grid dimensions, and process lifetime; this client owns only its layout.
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};
use terminal_core::pty::{Program, Session, Sessions};
use web_time::{Duration, Instant};

/// Resource cards retain their type without granting a TTY execution path.
#[derive(Clone, Copy, Debug)]
pub enum Resource {
    Thread,
    Run,
    Artifact,
}

pub const MAX_PANES: usize = 8;
pub const FRAME_BYTES: usize = 64 * 1024;
pub const FRAME_TIME: Duration = Duration::from_millis(8);

pub struct Pane {
    pub transport: Sessions,
    pub session: Session,
    scroll: usize,
    requested: Option<(u16, u16)>,
}
pub struct Mux {
    pub panes: Vec<Pane>,
    pub active: usize,
    pub split: Option<Direction>,
    prefix: bool,
    pub detached: bool,
    pub notice: String,
}
impl Mux {
    /// Each transport must attach its explicitly admitted reference.
    pub fn attach(transports: Vec<Sessions>) -> Result<Self, String> {
        if transports.is_empty() || transports.len() > MAX_PANES {
            return Err("Attach between one and eight admitted terminals.".into());
        }
        let mut panes = Vec::new();
        for transport in transports {
            let session = transport.open(&Program::Shell, 24, 80)?;
            panes.push(Pane {
                transport,
                session,
                scroll: 0,
                requested: None,
            });
        }
        Ok(Self { panes, active: 0, split: None, prefix: false, detached: false,
            notice: "Ctrl+B: n/p tab, %/\" split, o focus, j/k blocks, r live, d detach; twice forwards prefix. TTY: cropped grid; no images or resource activation.".into() })
    }
    /// One global time and byte budget, starting at the focused pane.
    pub fn pump(&mut self) -> u64 {
        let deadline = Instant::now() + FRAME_TIME;
        let mut used = 0;
        for offset in 0..self.panes.len() {
            if used >= FRAME_BYTES as u64 || Instant::now() >= deadline {
                break;
            }
            let index = (self.active + offset) % self.panes.len();
            let pane = &mut self.panes[index];
            used += pane
                .transport
                .pump(&mut pane.session, FRAME_BYTES - used as usize, deadline)
                .1;
        }
        used
    }
    fn send(&mut self, bytes: &[u8]) {
        if bytes.len() > coder_pty::wire::INPUT_MAX {
            self.notice = "Input exceeds the host request limit; nothing was sent.".into();
            return;
        }
        let pane = &self.panes[self.active];
        if pane.session.input_available() && pane.session.exited.is_none() {
            pane.transport.input(&pane.session, bytes);
        } else {
            self.notice =
                "Input refused: this attachment is not current typist. Input is never queued."
                    .into();
        }
    }
    pub fn event(&mut self, event: Event, area: Rect) {
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => self.key(key),
            Event::Paste(text) => {
                let bytes = coder_vt::encode_paste(
                    &text,
                    self.panes[self.active].session.vt.bracketed_paste(),
                );
                self.send(&bytes);
            }
            Event::Mouse(mouse) => {
                use coder_vt::{MouseButton as V, MouseKind as M};
                use crossterm::event::{MouseButton as B, MouseEventKind as K};
                let regions = self.regions(area);
                for (index, rect) in regions {
                    let inner = Block::default().borders(Borders::ALL).inner(rect);
                    if !inner.contains((mouse.column, mouse.row).into()) {
                        continue;
                    }
                    self.active = index;
                    let button = |b| match b {
                        B::Left => V::Left,
                        B::Right => V::Right,
                        B::Middle => V::Middle,
                    };
                    let kind = match mouse.kind {
                        K::Down(b) => M::Press(button(b)),
                        K::Up(b) => M::Release(button(b)),
                        K::Drag(b) => M::Motion(Some(button(b))),
                        K::Moved => M::Motion(None),
                        K::ScrollUp => M::Press(V::WheelUp),
                        K::ScrollDown => M::Press(V::WheelDown),
                        K::ScrollLeft => M::Press(V::WheelLeft),
                        K::ScrollRight => M::Press(V::WheelRight),
                    };
                    if self.panes[index].scroll != 0 {
                        break;
                    }
                    let bytes = self.panes[index].session.vt.mouse(coder_vt::MouseEvent {
                        kind,
                        row: (mouse.row - inner.y) as usize,
                        col: (mouse.column - inner.x) as usize,
                        modifiers: modifiers(mouse.modifiers),
                    });
                    if let Some(bytes) = bytes {
                        self.send(&bytes);
                    }
                    break;
                }
            }
            Event::FocusGained | Event::FocusLost => {
                if let Some(bytes) = self.panes[self.active]
                    .session
                    .vt
                    .focus(matches!(event, Event::FocusGained))
                {
                    self.send(&bytes);
                }
            }
            _ => {}
        }
    }
    fn key(&mut self, key: KeyEvent) {
        let prefix =
            key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL);
        if key.kind == KeyEventKind::Repeat && (prefix || self.prefix) {
            return;
        }
        if self.prefix {
            self.prefix = false;
            if prefix {
                self.send(&[2]);
                return;
            }
            match key.code {
                KeyCode::Char('d') => self.detached = true,
                KeyCode::Char('n' | 'o') => self.active = (self.active + 1) % self.panes.len(),
                KeyCode::Char('p') => {
                    self.active = (self.active + self.panes.len() - 1) % self.panes.len()
                }
                KeyCode::Char('%') => self.split = Some(Direction::Horizontal),
                KeyCode::Char('"') => self.split = Some(Direction::Vertical),
                KeyCode::Char('z') => self.split = None,
                KeyCode::Char('r') => self.panes[self.active].scroll = 0,
                KeyCode::Char('t') => self.resource(Resource::Thread),
                KeyCode::Char('u') => self.resource(Resource::Run),
                KeyCode::Char('f') => self.resource(Resource::Artifact),
                KeyCode::Up | KeyCode::Char('k') => self.block(true),
                KeyCode::Down | KeyCode::Char('j') => self.block(false),
                KeyCode::Char(c @ '1'..='8') => {
                    let i = c as usize - '1' as usize;
                    if i < self.panes.len() {
                        self.active = i;
                    }
                }
                _ => self.notice = "Unknown prefix command; no bytes were sent.".into(),
            }
            return;
        }
        if prefix {
            self.prefix = true;
            return;
        }
        let mapped = match key.code {
            KeyCode::Char(c) => coder_vt::Key::Char(c),
            KeyCode::Enter => coder_vt::Key::Enter,
            KeyCode::Tab => coder_vt::Key::Tab,
            KeyCode::BackTab => coder_vt::Key::BackTab,
            KeyCode::Backspace => coder_vt::Key::Backspace,
            KeyCode::Esc => coder_vt::Key::Escape,
            KeyCode::Up => coder_vt::Key::Up,
            KeyCode::Down => coder_vt::Key::Down,
            KeyCode::Left => coder_vt::Key::Left,
            KeyCode::Right => coder_vt::Key::Right,
            KeyCode::Home => coder_vt::Key::Home,
            KeyCode::End => coder_vt::Key::End,
            KeyCode::PageUp => coder_vt::Key::PageUp,
            KeyCode::PageDown => coder_vt::Key::PageDown,
            KeyCode::Insert => coder_vt::Key::Insert,
            KeyCode::Delete => coder_vt::Key::Delete,
            KeyCode::F(n) => coder_vt::Key::F(n),
            _ => return,
        };
        self.panes[self.active].scroll = 0;
        self.send(
            &self.panes[self.active]
                .session
                .vt
                .key(mapped, modifiers(key.modifiers)),
        );
    }
    pub fn resource(&mut self, resource: Resource) {
        self.notice = match resource {
            Resource::Thread => "Thread resource: open the retained thread client; this TTY grants no chat or proposal authority.",
            Resource::Run => "Run resource: open the retained task client; this TTY grants no task-control authority.",
            Resource::Artifact => "Artifact resource: use the exact task manifest and digest in the retained artifact client; no file was opened.",
        }.into();
    }
    fn block(&mut self, previous: bool) {
        let pane = &mut self.panes[self.active];
        let top = pane.session.vt.scrollback_len().saturating_sub(pane.scroll);
        let Some(_journal) = &pane.session.journal else {
            self.notice = "Host command blocks are unavailable; no resource was activated.".into();
            return;
        };
        let anchors = pane.session.blocks.records.iter().filter_map(|block| {
            usize::try_from(
                block
                    .start
                    .line
                    .saturating_sub(pane.session.vt.history_dropped()),
            )
            .ok()
        });
        let target = if previous {
            anchors.filter(|row| *row < top).max()
        } else {
            anchors.filter(|row| *row > top).min()
        };
        if let Some(row) = target {
            pane.scroll = pane.session.vt.scrollback_len().saturating_sub(row);
        }
    }
    fn regions(&self, area: Rect) -> Vec<(usize, Rect)> {
        let body = Rect {
            y: area.y.saturating_add(1),
            height: area.height.saturating_sub(2),
            ..area
        };
        if let Some(direction) = self.split.filter(|_| self.panes.len() > 1) {
            let halves = Layout::default()
                .direction(direction)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(body);
            vec![
                (self.active, halves[0]),
                ((self.active + 1) % self.panes.len(), halves[1]),
            ]
        } else {
            vec![(self.active, body)]
        }
    }
    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let tabs = self
            .panes
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "{}{}:{}",
                    if i == self.active { "*" } else { " " },
                    i + 1,
                    p.session.vt.title()
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        frame.render_widget(
            Paragraph::new(tabs),
            Rect {
                height: area.height.min(1),
                ..area
            },
        );
        frame.render_widget(
            Paragraph::new(self.notice.clone()),
            Rect {
                y: area.bottom().saturating_sub(1),
                height: area.height.min(1),
                ..area
            },
        );
        for (index, rect) in self.regions(area) {
            let pane = &mut self.panes[index];
            let availability = if pane.session.input_available() {
                "typist"
            } else {
                "read only"
            };
            let title = format!(
                "{} · {} · {}",
                index + 1,
                availability,
                pane.session
                    .exited
                    .as_ref()
                    .or(pane.session.status.as_ref())
                    .map(String::as_str)
                    .unwrap_or("connecting")
            );
            let border = Block::default().borders(Borders::ALL).title(title);
            let inner = border.inner(rect);
            frame.render_widget(border, rect);
            let size = (inner.height.max(1), inner.width.max(1));
            if pane.requested != Some(size) && pane.session.input_available() {
                pane.transport.resize(&mut pane.session, size.0, size.1);
                pane.requested = Some(size);
            }
            let start = pane.session.vt.scrollback_len().saturating_sub(pane.scroll);
            let buffer = frame.buffer_mut();
            for y in 0..inner.height as usize {
                let Some(row) = pane.session.vt.line(start + y) else {
                    continue;
                };
                for (x, cell) in row.cells.iter().take(inner.width as usize).enumerate() {
                    if cell.width == 0 || (cell.width == 2 && x + 1 >= inner.width as usize) {
                        continue;
                    }
                    let mut text = cell.ch.to_string();
                    text.extend(cell.combining.iter().copied());
                    buffer[(inner.x + x as u16, inner.y + y as u16)]
                        .set_symbol(&text)
                        .set_style(style(cell.attrs));
                }
            }
            if index == self.active && pane.scroll == 0 && pane.session.vt.cursor_visible() {
                let (row, col) = pane.session.vt.cursor();
                if row < inner.height as usize && col < inner.width as usize {
                    frame.set_cursor_position((inner.x + col as u16, inner.y + row as u16));
                }
            }
        }
    }
}
fn modifiers(m: KeyModifiers) -> coder_vt::Modifiers {
    coder_vt::Modifiers {
        ctrl: m.contains(KeyModifiers::CONTROL),
        alt: m.contains(KeyModifiers::ALT),
        shift: m.contains(KeyModifiers::SHIFT),
    }
}
fn style(attrs: coder_vt::Attrs) -> Style {
    fn color(c: coder_vt::Color) -> Color {
        match c {
            coder_vt::Color::Default => Color::Reset,
            coder_vt::Color::Indexed(n) => Color::Indexed(n),
            coder_vt::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
        }
    }
    let mut s = Style::default().fg(color(attrs.fg)).bg(color(attrs.bg));
    for (flag, modifier) in [
        (coder_vt::Flags::BOLD, Modifier::BOLD),
        (coder_vt::Flags::DIM, Modifier::DIM),
        (coder_vt::Flags::ITALIC, Modifier::ITALIC),
        (coder_vt::Flags::UNDERLINE, Modifier::UNDERLINED),
        (coder_vt::Flags::INVERSE, Modifier::REVERSED),
        (coder_vt::Flags::HIDDEN, Modifier::HIDDEN),
        (coder_vt::Flags::STRIKE, Modifier::CROSSED_OUT),
    ] {
        if attrs.flags.contains(flag) {
            s = s.add_modifier(modifier);
        }
    }
    s
}
