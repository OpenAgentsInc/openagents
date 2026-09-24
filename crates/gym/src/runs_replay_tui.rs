//! A task/attempt picker and two full transcripts sharing one replay clock.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::time::Duration;

use coder_terminal::{Intensity, Ladder, frame, wrap_rows};
use ratatui::{buffer::Buffer, layout::Rect};

use crate::runs::{Agent, Catalog};
use crate::runs_replay::{Playback, Replay, Source};
use crate::runs_tui::{Key, Reply};

struct Wrapped {
    width: usize,
    rows: Vec<(u64, String, bool)>,
}
struct Screen {
    source: Source,
    replay: Replay,
    rows: RefCell<Wrapped>,
    scroll: Cell<usize>,
    follow: Cell<bool>,
    details: bool,
}
impl Screen {
    fn new(source: Source) -> Result<Self, String> {
        let replay = Replay::load(&source)?;
        Ok(Self {
            source,
            replay,
            rows: RefCell::new(Wrapped {
                width: 0,
                rows: Vec::new(),
            }),
            scroll: Cell::new(0),
            follow: Cell::new(true),
            details: false,
        })
    }
    fn render(&self, area: Rect, buf: &mut Buffer, ladder: Ladder, elapsed: u64, focused: bool) {
        if area.width < 4 || area.height < 8 {
            return;
        }
        frame(
            area,
            buf,
            ladder.style(if focused {
                Intensity::Full
            } else {
                Intensity::Quarter
            }),
        );
        let inner = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
        line(buf, inner, 0, &self.source.label(), ladder, Intensity::Full);
        line(
            buf,
            inner,
            1,
            &self.source.description(),
            ladder,
            Intensity::Half,
        );
        let shown = self
            .replay
            .events
            .partition_point(|e| e.elapsed_ms <= elapsed);
        line(
            buf,
            inner,
            2,
            &format!(
                "{} / {} events · {} recorded · {} estimated · {}",
                shown,
                self.replay.events.len(),
                self.replay.recorded,
                self.replay.estimated,
                if elapsed >= self.replay.duration_ms {
                    "finished"
                } else {
                    "replaying"
                }
            ),
            ladder,
            Intensity::Half,
        );
        line(
            buf,
            inner,
            3,
            &self.replay.origin,
            ladder,
            Intensity::Quarter,
        );
        let text = Rect::new(
            inner.x,
            inner.y + 5,
            inner.width,
            inner.height.saturating_sub(5),
        );
        let mut wrapped = self.rows.borrow_mut();
        if wrapped.width != usize::from(text.width) {
            wrapped.width = usize::from(text.width);
            wrapped.rows.clear();
            for event in &self.replay.events {
                wrapped.rows.push((
                    event.elapsed_ms,
                    format!(
                        "{}  {}  [{}]",
                        clock(event.elapsed_ms),
                        event.title,
                        event.timing
                    ),
                    true,
                ));
                let body = if self.details {
                    &event.record
                } else {
                    &event.text
                };
                for span in wrap_rows(body, usize::from(text.width)) {
                    wrapped
                        .rows
                        .push((event.elapsed_ms, body[span].to_owned(), false));
                }
                wrapped.rows.push((event.elapsed_ms, String::new(), false));
            }
        }
        let available = wrapped
            .rows
            .partition_point(|(time, _, _)| *time <= elapsed);
        let max = available.saturating_sub(usize::from(text.height));
        let scroll = if self.follow.get() {
            max
        } else {
            self.scroll.get().min(max)
        };
        self.scroll.set(scroll);
        for (i, (_, value, header)) in wrapped
            .rows
            .iter()
            .take(available)
            .skip(scroll)
            .take(usize::from(text.height))
            .enumerate()
        {
            line(
                buf,
                text,
                i as u16,
                value,
                ladder,
                if *header {
                    Intensity::Full
                } else {
                    Intensity::Half
                },
            );
        }
    }
}

pub struct Pane {
    local: Vec<Source>,
    public: Vec<Source>,
    tasks: Vec<String>,
    task: usize,
    cursors: [usize; 2],
    focus: usize,
    query: String,
    typing: bool,
    all_agents: bool,
    right_local: bool,
    screens: Option<[Option<Screen>; 2]>,
    pub clock: Playback,
    pub errors: Vec<String>,
}

impl Pane {
    pub fn new(catalog: &Catalog, selected: Option<&crate::runs::Run>) -> Self {
        let (local, public, errors) = crate::runs_replay::sources(catalog);
        Self::from_sources(local, public, errors, selected)
    }
    fn from_sources(
        local: Vec<Source>,
        public: Vec<Source>,
        errors: Vec<String>,
        selected: Option<&crate::runs::Run>,
    ) -> Self {
        let tasks: Vec<_> = local
            .iter()
            .chain(&public)
            .map(|s| s.task().to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let task = selected
            .and_then(|run| tasks.iter().position(|task| task == &run.task))
            .unwrap_or(0);
        let all_agents = selected.is_some_and(|r| r.agent != Agent::CoderOne);
        let mut pane = Self {
            local,
            public,
            tasks,
            task,
            cursors: [0; 2],
            focus: 0,
            query: String::new(),
            typing: false,
            all_agents,
            right_local: false,
            screens: None,
            clock: Playback::new(0),
            errors,
        };
        if let Some(run) = selected {
            pane.cursors[0] = pane
                .choices(0)
                .iter()
                .position(|s| s.id() == run.id())
                .unwrap_or(0);
        }
        pane
    }
    fn visible_tasks(&self) -> Vec<&str> {
        self.tasks
            .iter()
            .filter(|task| task.contains(&self.query.to_lowercase()))
            .map(String::as_str)
            .collect()
    }
    fn task_name(&self) -> Option<&str> {
        self.visible_tasks().get(self.task).copied()
    }
    fn choices(&self, side: usize) -> Vec<&Source> {
        let sources = if side == 0 || self.right_local {
            &self.local
        } else {
            &self.public
        };
        sources
            .iter()
            .filter(|s| Some(s.task()) == self.task_name())
            .filter(|s| {
                side == 1
                    || self.all_agents
                    || matches!(s, Source::Local(run) if run.agent == Agent::CoderOne)
            })
            .collect()
    }
    pub fn active(&self) -> bool {
        self.screens.is_some()
    }
    pub fn advance(&mut self, elapsed: Duration) {
        if self.active() {
            self.clock.advance(elapsed);
        }
    }
    fn start(&mut self) {
        let selected: Vec<Option<Source>> = (0..2)
            .map(|side| {
                self.choices(side)
                    .get(self.cursors[side])
                    .map(|s| (*s).clone())
            })
            .collect();
        let mut screens = [None, None];
        self.errors.clear();
        for (side, source) in selected.into_iter().enumerate() {
            if let Some(source) = source {
                match Screen::new(source) {
                    Ok(screen) => {
                        self.errors.extend(screen.replay.warnings.clone());
                        screens[side] = Some(screen);
                    }
                    Err(error) => self.errors.push(error),
                }
            }
        }
        if screens.iter().all(Option::is_none) {
            return;
        }
        self.clock = Playback::new(
            screens
                .iter()
                .flatten()
                .map(|s| s.replay.duration_ms)
                .max()
                .unwrap_or(0),
        );
        self.screens = Some(screens);
        self.focus = 0;
    }
    /// Returns `None` when Escape leaves head-to-head mode.
    pub fn key(&mut self, key: Key) -> Option<Reply> {
        if self.typing {
            match key {
                Key::Char(c) => self.query.push(c),
                Key::Backspace => {
                    self.query.pop();
                }
                Key::Back => {
                    self.query.clear();
                    self.typing = false;
                }
                Key::Enter => self.typing = false,
                _ => {}
            }
            self.task = 0;
            self.cursors = [0; 2];
            return Some(Reply::Handled);
        }
        if let Some(screens) = &mut self.screens {
            match key {
                Key::Back => {
                    self.screens = None;
                    self.focus = 0;
                    return Some(Reply::Handled);
                }
                Key::Char('q') => return Some(Reply::Quit),
                Key::Tab => self.focus = 1 - self.focus,
                Key::Char('d') => {
                    if let Some(screen) = &mut screens[self.focus] {
                        screen.details = !screen.details;
                        screen.rows.borrow_mut().width = 0;
                        screen.scroll.set(0);
                        screen.follow.set(false);
                    }
                }
                Key::Char(' ') => self.clock.playing = !self.clock.playing,
                Key::Char('+' | '=') => self.clock.faster(),
                Key::Char('-' | '_') => self.clock.slower(),
                Key::Char('r') | Key::Home => {
                    self.clock.elapsed_ms = 0.0;
                    for screen in screens.iter().flatten() {
                        screen.follow.set(true);
                    }
                }
                Key::Char(']') | Key::Right => self.clock.seek(30_000),
                Key::Char('[') | Key::Left => self.clock.seek(-30_000),
                Key::End => {
                    self.clock.elapsed_ms = self.clock.duration_ms as f64;
                    self.clock.playing = false;
                }
                Key::Char('n' | 'b') => {
                    let now = self.clock.elapsed_ms as u64;
                    let times = screens
                        .iter()
                        .flatten()
                        .flat_map(|s| s.replay.events.iter().map(|e| e.elapsed_ms));
                    let next = if key == Key::Char('n') {
                        times.filter(|t| *t > now).min()
                    } else {
                        times.filter(|t| *t < now).max()
                    };
                    if let Some(next) = next {
                        self.clock.elapsed_ms = next as f64;
                    }
                }
                _ => {
                    if let Some(screen) = &screens[self.focus] {
                        let at = screen.scroll.get();
                        match key {
                            Key::Up | Key::Char('k') => {
                                screen.follow.set(false);
                                screen.scroll.set(at.saturating_sub(1));
                            }
                            Key::Down | Key::Char('j') => {
                                screen.follow.set(false);
                                screen.scroll.set(at.saturating_add(1));
                            }
                            Key::PageUp => {
                                screen.follow.set(false);
                                screen.scroll.set(at.saturating_sub(20));
                            }
                            Key::PageDown => {
                                screen.follow.set(false);
                                screen.scroll.set(at.saturating_add(20));
                            }
                            Key::Char('g') => {
                                screen.follow.set(false);
                                screen.scroll.set(0);
                            }
                            Key::Char('G' | 'f') => screen.follow.set(true),
                            _ => {}
                        }
                    }
                }
            }
            return Some(Reply::Handled);
        }
        match key {
            Key::Back => return None,
            Key::Char('q') => return Some(Reply::Quit),
            Key::Tab => self.focus = (self.focus + 1) % 3,
            Key::Left => self.focus = self.focus.saturating_sub(1),
            Key::Right => self.focus = (self.focus + 1).min(2),
            Key::Enter => self.start(),
            Key::Char('/') => self.typing = true,
            Key::Char('a') => {
                self.all_agents = !self.all_agents;
                self.cursors[0] = 0;
            }
            Key::Char('o') => {
                self.right_local = !self.right_local;
                self.cursors[1] = 0;
            }
            Key::Char('c') => {
                self.query.clear();
                self.task = 0;
                self.cursors = [0; 2];
            }
            key => {
                let length = if self.focus == 0 {
                    self.visible_tasks().len()
                } else {
                    self.choices(self.focus - 1).len()
                };
                let cursor = if self.focus == 0 {
                    &mut self.task
                } else {
                    &mut self.cursors[self.focus - 1]
                };
                let before = *cursor;
                match key {
                    Key::Up | Key::Char('k') => *cursor = cursor.saturating_sub(1),
                    Key::Down | Key::Char('j') => {
                        *cursor = (*cursor + 1).min(length.saturating_sub(1))
                    }
                    Key::PageUp => *cursor = cursor.saturating_sub(10),
                    Key::PageDown => *cursor = (*cursor + 10).min(length.saturating_sub(1)),
                    Key::Home => *cursor = 0,
                    Key::End => *cursor = length.saturating_sub(1),
                    _ => {}
                }
                if self.focus == 0 && before != *cursor {
                    self.cursors = [0; 2];
                }
            }
        }
        Some(Reply::Handled)
    }
    pub fn render(&self, area: Rect, buf: &mut Buffer, ladder: Ladder) {
        if area.width < 60 || area.height < 14 {
            line(
                buf,
                area,
                0,
                "Head-to-head needs at least 60 columns and 14 rows",
                ladder,
                Intensity::Full,
            );
            return;
        }
        let bottom = area.height - 1;
        if let Some(screens) = &self.screens {
            line(
                buf,
                area,
                0,
                &format!(
                    "Head-to-head · {} · {} / {} · {}× · Tab selects {}",
                    if self.clock.playing {
                        "playing"
                    } else {
                        "paused"
                    },
                    clock(self.clock.elapsed_ms as u64),
                    clock(self.clock.duration_ms),
                    self.clock.speed(),
                    if self.focus == 0 { "right" } else { "left" }
                ),
                ladder,
                Intensity::Full,
            );
            line(
                buf,
                area,
                1,
                "Space play/pause · +/- speed · ←/→ seek 30s · n/b event · r restart · End complete",
                ladder,
                Intensity::Half,
            );
            let left = Rect::new(area.x, area.y + 2, area.width / 2, area.height - 5);
            let right = Rect::new(left.right(), left.y, area.width - left.width, left.height);
            for (index, side) in [left, right].into_iter().enumerate() {
                if let Some(screen) = &screens[index] {
                    screen.render(
                        side,
                        buf,
                        ladder,
                        self.clock.elapsed_ms as u64,
                        self.focus == index,
                    );
                } else {
                    frame(side, buf, ladder.style(Intensity::Quarter));
                    line(
                        buf,
                        side,
                        1,
                        "No transcript selected / available",
                        ladder,
                        Intensity::Half,
                    );
                }
            }
            line(
                buf,
                area,
                bottom,
                "↑/↓ PgUp/PgDn scroll · g top · f follow · d full record/text · Esc choose pair · q quit",
                ladder,
                Intensity::Half,
            );
        } else {
            line(
                buf,
                area,
                0,
                &format!(
                    "Head-to-head · {} local · {} public attempts · / {}{}",
                    self.local.len(),
                    self.public.len(),
                    self.query,
                    if self.typing { "▏" } else { "" }
                ),
                ladder,
                Intensity::Full,
            );
            line(
                buf,
                area,
                1,
                "Tab/←/→ choose column · ↑/↓ choose task or attempt · Enter load · / search tasks",
                ladder,
                Intensity::Half,
            );
            let task_width = area.width / 4;
            let side_width = (area.width - task_width) / 2;
            let rects = [
                Rect::new(area.x, area.y + 2, task_width, area.height - 5),
                Rect::new(area.x + task_width, area.y + 2, side_width, area.height - 5),
                Rect::new(
                    area.x + task_width + side_width,
                    area.y + 2,
                    area.width - task_width - side_width,
                    area.height - 5,
                ),
            ];
            let tasks = self
                .visible_tasks()
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>();
            let left = self
                .choices(0)
                .iter()
                .map(|s| s.label())
                .collect::<Vec<_>>();
            let right = self
                .choices(1)
                .iter()
                .map(|s| s.label())
                .collect::<Vec<_>>();
            for (index, (values, cursor, label)) in [
                (&tasks, self.task, "Task"),
                (
                    &left,
                    self.cursors[0],
                    if self.all_agents {
                        "Local: all agents"
                    } else {
                        "Local: Coder One"
                    },
                ),
                (
                    &right,
                    self.cursors[1],
                    if self.right_local {
                        "Opponent: local"
                    } else {
                        "Opponent: Fable 5.1"
                    },
                ),
            ]
            .into_iter()
            .enumerate()
            {
                list(
                    rects[index],
                    buf,
                    ladder,
                    values,
                    cursor,
                    label,
                    self.focus == index,
                );
            }
            line(
                buf,
                area,
                bottom,
                "a Coder One/all local agents · o public/local opponent · c clear search · Esc back",
                ladder,
                Intensity::Half,
            );
        }
        if let Some(error) = self.errors.first() {
            line(
                buf,
                area,
                bottom - 1,
                &format!("{} notice(s): {error}", self.errors.len()),
                ladder,
                Intensity::Full,
            );
        }
    }
}

fn line(buf: &mut Buffer, area: Rect, row: u16, value: &str, ladder: Ladder, intensity: Intensity) {
    if row < area.height {
        buf.set_stringn(
            area.x,
            area.y + row,
            value,
            usize::from(area.width),
            ladder.style(intensity).bg(ladder.background()),
        );
    }
}
fn list(
    area: Rect,
    buf: &mut Buffer,
    ladder: Ladder,
    values: &[String],
    cursor: usize,
    label: &str,
    focused: bool,
) {
    frame(
        area,
        buf,
        ladder.style(if focused {
            Intensity::Full
        } else {
            Intensity::Quarter
        }),
    );
    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    line(
        buf,
        inner,
        0,
        &format!("{label} ({})", values.len()),
        ladder,
        Intensity::Full,
    );
    let height = usize::from(inner.height.saturating_sub(2));
    let start = cursor.saturating_sub(height.saturating_sub(1));
    if values.is_empty() {
        line(
            buf,
            inner,
            2,
            "No matching attempts",
            ladder,
            Intensity::Half,
        );
    }
    for (i, value) in values.iter().enumerate().skip(start).take(height) {
        line(
            buf,
            inner,
            (i - start + 2) as u16,
            &format!("{} {value}", if i == cursor { "›" } else { " " }),
            ladder,
            if i == cursor {
                Intensity::Full
            } else {
                Intensity::Half
            },
        );
    }
}
fn clock(ms: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::Sources;

    fn pane() -> (tempfile::TempDir, Pane) {
        let (dir, sources) = crate::runs::fixture_sources();
        let catalog = Catalog::load(sources);
        let local = catalog
            .runs
            .iter()
            .cloned()
            .map(|r| Source::Local(Box::new(r)))
            .collect();
        (dir, Pane::from_sources(local, vec![], vec![], None))
    }
    fn contents(buf: &Buffer) -> String {
        buf.content.iter().map(|c| c.symbol()).collect()
    }

    #[test]
    fn pick_local_versions_then_replay_scroll_and_return() {
        let (_dir, mut p) = pane();
        p.query = "coq".to_owned();
        p.key(Key::Char('o'));
        p.key(Key::Enter);
        assert!(p.active(), "{:?}", p.errors);
        assert!(!p.clock.playing);
        p.key(Key::Char(' '));
        for _ in 0..3 {
            p.key(Key::Char('+'));
        }
        p.advance(Duration::from_millis(100));
        assert_eq!(p.clock.speed(), 10);
        assert_eq!(p.clock.elapsed_ms, 1000.0);
        p.key(Key::Char(' '));
        p.advance(Duration::from_secs(10));
        assert_eq!(p.clock.elapsed_ms, 1000.0);
        p.key(Key::End);
        let area = Rect::new(0, 0, 150, 38);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        assert!(contents(&buf).contains("10×"));
        p.key(Key::Char('g'));
        assert!(
            !p.screens.as_ref().unwrap()[0]
                .as_ref()
                .unwrap()
                .follow
                .get()
        );
        assert!(
            p.screens.as_ref().unwrap()[1]
                .as_ref()
                .unwrap()
                .follow
                .get()
        );
        p.key(Key::Tab);
        p.key(Key::Char('g'));
        p.key(Key::Char('r'));
        assert_eq!(p.clock.elapsed_ms, 0.0);
        p.key(Key::Back);
        assert!(!p.active());
        assert_eq!(p.key(Key::Back), None);
    }

    #[test]
    fn empty_catalog_search_and_small_windows_are_safe() {
        let catalog = Catalog::load(Sources::default());
        let mut p = Pane::from_sources(vec![], vec![], vec![], None);
        for key in [
            Key::Down,
            Key::Tab,
            Key::End,
            Key::Enter,
            Key::Char('/'),
            Key::Char('x'),
            Key::Enter,
        ] {
            p.key(key);
        }
        assert!(!p.active());
        assert!(catalog.runs.is_empty());
        for (w, h) in [(0, 0), (20, 5), (60, 14), (150, 40)] {
            let area = Rect::new(0, 0, w, h);
            p.render(
                area,
                &mut Buffer::empty(area),
                crate::tui::ladder_from_environment(),
            );
        }
    }

    #[test]
    fn complete_output_can_be_scrolled_past_sixty_lines() {
        let (_dir, mut p) = pane();
        p.query = "coq".to_owned();
        p.key(Key::Enter);
        let screen = p.screens.as_mut().unwrap()[0].as_mut().unwrap();
        screen.replay.events = vec![crate::runs_replay::Event {
            elapsed_ms: 0,
            timing: "step timestamp",
            title: "tool".to_owned(),
            text: format!("{}\nFINAL SENTINEL", "full output\n".repeat(2000)),
            record: "complete record".to_owned(),
        }];
        screen.replay.duration_ms = 0;
        let area = Rect::new(0, 0, 150, 38);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        assert!(contents(&buf).contains("FINAL SENTINEL"));
        p.key(Key::Char('g'));
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        assert!(contents(&buf).contains("step timestamp"));
    }
}
