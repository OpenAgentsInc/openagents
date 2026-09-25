//! A task/attempt picker and two full transcripts sharing one replay clock.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::time::Duration;

use coder_terminal::{Intensity, Ladder, frame, markdown, wrap_rows};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
};

use crate::runs::{Agent, Catalog};
use crate::runs_replay::{Playback, Replay, Source};
use crate::runs_replay_learning::Learning;
use crate::runs_tui::{Key, Reply};

struct Wrapped {
    width: usize,
    ladder: Ladder,
    rows: Vec<(u64, Line<'static>)>,
}
struct Screen {
    source: Source,
    replay: Replay,
    rows: RefCell<Wrapped>,
    scroll: Cell<usize>,
    follow: Cell<bool>,
    details: bool,
    /// Each event's phases, from `runs_phases`: the rules, and Jev's stored
    /// answers for the steps the rules leave.
    phases: Vec<String>,
    /// The side as the Runs pane's transcript reads it, drawn up to the
    /// playback clock; `d` switches to the raw event record.
    blocks: Vec<crate::runs_transcript::Block>,
    /// The step with the selection bar; `None` follows the latest step.
    selected: Cell<Option<usize>>,
    /// Steps opened with Enter; `all` flips every step at once.
    expanded: std::cell::RefCell<std::collections::BTreeSet<usize>>,
    all: Cell<bool>,
    /// How many steps the last frame showed.
    shown: Cell<usize>,
    /// The wheel moved the view, so it stays put until a key moves the
    /// selection.
    free: Cell<bool>,
    /// The screen row and step of every row drawn last frame, for clicks.
    drawn: std::cell::RefCell<Vec<(u16, usize)>>,
    file_hits: RefCell<Vec<crate::runs_files::Hit>>,
}
impl Screen {
    fn new(source: Source) -> Result<Self, String> {
        let replay = Replay::load(&source)?;
        let fallback = match &source {
            Source::Local(run) => run
                .task_path
                .as_ref()
                .and_then(|path| std::fs::read_to_string(path.join("instruction.md")).ok()),
            Source::Public { .. } => None,
        };
        let phases = crate::runs_phases::event_labels(
            &replay,
            &crate::runs_phases::task_of(&replay, fallback),
            &crate::runs_phases::StepStore::open(crate::runs_phases::default_dir()),
        );
        let blocks = match &source {
            Source::Local(run) => crate::runs_story::load_transcript(run).blocks,
            Source::Public { trial, cache } => {
                crate::runs_transcript::trajectory(&cache.join(&trial.file), Vec::new()).blocks
            }
        };
        Ok(Self {
            source,
            replay,
            phases,
            blocks,
            selected: Cell::new(None),
            expanded: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            all: Cell::new(false),
            shown: Cell::new(0),
            free: Cell::new(false),
            drawn: std::cell::RefCell::new(Vec::new()),
            file_hits: RefCell::new(Vec::new()),
            rows: RefCell::new(Wrapped {
                width: 0,
                ladder: Ladder::default(),
                rows: Vec::new(),
            }),
            scroll: Cell::new(0),
            follow: Cell::new(true),
            details: false,
        })
    }
    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        ladder: Ladder,
        playback: &Playback,
        focused: bool,
    ) {
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
        let elapsed = playback.elapsed_ms as u64;
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
                "{} of {} events · {} with recorded times · {} with estimated times · {}",
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
        if !self.details && !self.blocks.is_empty() {
            let was_free = self.free.get();
            let expanded = self.expanded.borrow();
            let all = self.all.get();
            let open = |index: usize| all != expanded.contains(&index);
            let selected = if self.follow.get() {
                None
            } else {
                self.selected.get()
            };
            self.shown.set(crate::runs_tui::draw_transcript(
                &self.blocks,
                elapsed,
                selected,
                &open,
                &self.scroll,
                &self.free,
                &self.drawn,
                text,
                buf,
                ladder,
            ));
            *self.file_hits.borrow_mut() =
                crate::runs_files::decorate(text, &self.drawn.borrow(), &self.blocks, buf);
            // The wheel reached the bottom: follow the latest step again.
            if was_free && !self.free.get() {
                self.follow.set(true);
                self.selected.set(None);
            }
            return;
        }
        self.file_hits.borrow_mut().clear();
        let mut wrapped = self.rows.borrow_mut();
        if wrapped.width != usize::from(text.width) || wrapped.ladder != ladder {
            wrapped.width = usize::from(text.width);
            wrapped.ladder = ladder;
            wrapped.rows.clear();
            for (index, event) in self.replay.events.iter().enumerate() {
                let phase = self
                    .phases
                    .get(index)
                    .filter(|label| !label.is_empty())
                    .map_or(String::new(), |label| format!("  · {label}"));
                wrapped.rows.push((
                    event.elapsed_ms,
                    Line::styled(
                        format!(
                            "{}  {}  [{}]{phase}",
                            clock(event.elapsed_ms),
                            event.title,
                            event.timing
                        ),
                        ladder.style(Intensity::Full).bg(ladder.background()),
                    ),
                ));
                for row in event_rows(event, self.details, usize::from(text.width), ladder) {
                    wrapped.rows.push((event.elapsed_ms, row));
                }
                wrapped.rows.push((event.elapsed_ms, Line::default()));
            }
        }
        // Before the clock reaches the first event, show that event anyway.
        let first = wrapped.rows.first().map_or(0, |(time, _)| *time);
        let available = wrapped
            .rows
            .partition_point(|(time, _)| *time <= elapsed.max(first));
        let max = available.saturating_sub(usize::from(text.height));
        let scroll = if self.follow.get() {
            max
        } else {
            self.scroll.get().min(max)
        };
        self.scroll.set(scroll);
        for (i, (_, value)) in wrapped
            .rows
            .iter()
            .take(available)
            .skip(scroll)
            .take(usize::from(text.height))
            .enumerate()
        {
            buf.set_line(text.x, text.y + i as u16, value, text.width);
        }
    }
}

fn event_rows(
    event: &crate::runs_replay::Event,
    details: bool,
    width: usize,
    ladder: Ladder,
) -> Vec<Line<'static>> {
    let literal = |text: &str| {
        wrap_rows(text, width)
            .into_iter()
            .map(|range| {
                Line::styled(
                    text[range].to_owned(),
                    ladder.style(Intensity::Half).bg(ladder.background()),
                )
            })
            .collect::<Vec<_>>()
    };
    if details {
        return literal(&event.record);
    }
    if event.parts.is_empty() {
        return literal(&event.text);
    }
    let mut rows = Vec::new();
    for (index, part) in event.parts.iter().enumerate() {
        if index > 0 {
            rows.push(Line::default());
        }
        if part.markdown {
            for rendered in markdown::wrapped(&part.text, width) {
                let base = ladder.style(rendered.intensity).bg(ladder.background());
                let spans = rendered
                    .marked
                    .runs_in(0..rendered.marked.text.len())
                    .into_iter()
                    .map(|(text, marks)| Span::styled(text, marks.style(base, ladder)))
                    .collect::<Vec<_>>();
                rows.push(Line::from(spans));
            }
        } else {
            rows.extend(literal(&part.text));
        }
    }
    rows
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
    file_viewer: Option<crate::runs_files::Viewer>,
    side_errors: [Option<String>; 2],
    learning: Learning,
    analysis: bool,
    analysis_scroll: [Cell<usize>; 2],
    pub clock: Playback,
    pub errors: Vec<String>,
    /// Where each side was drawn last frame, for the mouse.
    sides: Cell<[Rect; 2]>,
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
            file_viewer: None,
            side_errors: [None, None],
            learning: Learning::default(),
            sides: Cell::new([Rect::default(); 2]),
            analysis: false,
            analysis_scroll: [Cell::new(0), Cell::new(0)],
            clock: Playback::new(0),
            errors,
        };
        if let Some(run) = selected {
            pane.task = pane
                .visible_tasks()
                .iter()
                .position(|task| *task == run.task)
                .unwrap_or(0);
            pane.cursors[0] = pane
                .choices(0)
                .iter()
                .position(|s| s.id() == run.id())
                .unwrap_or(0);
        }
        pane
    }
    /// Selects `task`, then the attempt on each side whose identity or
    /// description contains the given text, and starts the pair. On the
    /// right, `pass` picks the newest public attempt that passed and is on
    /// this computer; `best` picks the cheapest passing public attempt, the
    /// winner to beat. Returns what couldn't be found.
    pub fn preselect(
        &mut self,
        task: &str,
        left: Option<&str>,
        right: Option<&str>,
    ) -> Result<(), String> {
        self.query.clear();
        self.all_agents = true;
        self.task = self
            .visible_tasks()
            .iter()
            .position(|t| *t == task)
            .ok_or_else(|| format!("no replayable attempt for task {task}"))?;
        for (side, wanted) in [left, right].into_iter().enumerate() {
            let Some(wanted) = wanted else { continue };
            let choices = self.choices(side);
            let found = if wanted == "best" {
                // The winner to beat: the cheapest passing public attempt on
                // this computer.
                choices
                    .iter()
                    .enumerate()
                    .filter_map(|(index, s)| match s {
                        Source::Public { trial, cache }
                            if trial.reward == Some(1.0) && cache.join(&trial.file).is_file() =>
                        {
                            Some((index, trial.cost_usd.unwrap_or(f64::MAX), trial.id.clone()))
                        }
                        _ => None,
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.cmp(&b.2)))
                    .map(|(index, _, _)| index)
            } else {
                choices.iter().position(|s| match (wanted, s) {
                    ("pass", Source::Public { trial, cache }) => {
                        trial.reward == Some(1.0) && cache.join(&trial.file).is_file()
                    }
                    _ => s.id().contains(wanted) || s.description().contains(wanted),
                })
            };
            self.cursors[side] = found.ok_or_else(|| {
                format!(
                    "no {} attempt on {task} matches {wanted}",
                    if side == 0 { "left" } else { "right" }
                )
            })?;
        }
        self.start();
        Ok(())
    }
    pub fn with_learning(
        mut self,
        store: crate::runs_learning::Store,
        judge: crate::runs_learning::Judge,
        context: crate::runs_learning::Context,
    ) -> Self {
        self.learning = Learning::new(
            self.local.iter().chain(&self.public).cloned(),
            store,
            judge,
            context,
        );
        self
    }
    fn visible_tasks(&self) -> Vec<&str> {
        let mut tasks: Vec<_> = self
            .tasks
            .iter()
            .filter(|task| task.contains(&self.query.to_lowercase()))
            .map(String::as_str)
            .collect();
        // Task order uses the best visible attempt; attempts without a score
        // remain after ranked attempts, with newest first as the tie-breaker.
        let mut values = std::collections::HashMap::new();
        for source in self
            .local
            .iter()
            .chain(&self.public)
            .filter(|s| self.admitted(s))
        {
            let value = values
                .entry(source.task())
                .or_insert((None::<f64>, None::<i64>));
            if let Some(score) = self.learning.score(source) {
                value.0 = Some(value.0.map_or(score, |old| old.max(score)));
            }
            value.1 = value.1.max(source.started_ms());
        }
        tasks.sort_by(|a, b| {
            let a_value = values.get(a).copied().unwrap_or_default();
            let b_value = values.get(b).copied().unwrap_or_default();
            let rank = if self.learning.enabled {
                score_order(a_value.0, b_value.0)
            } else {
                std::cmp::Ordering::Equal
            };
            rank.then(b_value.1.cmp(&a_value.1)).then(a.cmp(b))
        });
        tasks
    }
    fn admitted(&self, source: &Source) -> bool {
        match source {
            Source::Local(run) => {
                self.right_local || self.all_agents || run.agent == Agent::CoderOne
            }
            Source::Public { .. } => !self.right_local,
        }
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
        let task = self.task_name();
        let mut choices: Vec<_> = sources
            .iter()
            .filter(|s| Some(s.task()) == task)
            .filter(|s| {
                side == 1
                    || self.all_agents
                    || matches!(s, Source::Local(run) if run.agent == Agent::CoderOne)
            })
            .collect();
        choices.sort_by(|a, b| {
            let rank = if self.learning.enabled {
                score_order(self.learning.score(a), self.learning.score(b))
            } else {
                std::cmp::Ordering::Equal
            };
            rank.then(b.started_ms().cmp(&a.started_ms()))
                .then(a.id().cmp(&b.id()))
        });
        choices
    }
    pub fn active(&self) -> bool {
        self.screens.is_some()
    }
    pub fn advance(&mut self, elapsed: Duration) {
        if self.learning.ranking() {
            let kept = self.selection();
            if self.learning.poll() {
                self.restore(kept);
            }
        }
        if self.active() && !self.analysis {
            self.clock.advance(elapsed);
        }
    }
    fn selection(&self) -> (Option<String>, [Option<String>; 2]) {
        (
            self.task_name().map(str::to_owned),
            std::array::from_fn(|side| self.choices(side).get(self.cursors[side]).map(|s| s.id())),
        )
    }
    fn restore(&mut self, (task, attempts): (Option<String>, [Option<String>; 2])) {
        self.task = self
            .visible_tasks()
            .iter()
            .position(|t| Some(*t) == task.as_deref())
            .unwrap_or(0);
        for (side, id) in attempts.iter().enumerate() {
            self.cursors[side] = self
                .choices(side)
                .iter()
                .position(|s| Some(s.id()).as_ref() == id.as_ref())
                .unwrap_or(0);
        }
    }
    fn toggle_learning(&mut self) {
        let kept = self.selection();
        if self.active() {
            self.analysis = !self.analysis;
            if self.analysis {
                self.clock.playing = false;
                self.learning.enabled = true;
            }
        } else {
            self.learning.enabled = !self.learning.enabled;
        }
        if self.learning.enabled {
            self.learning.start(kept.0.as_deref());
        }
        self.restore(kept);
    }
    fn start(&mut self) {
        let selected: Vec<Option<Source>> = (0..2)
            .map(|side| {
                self.choices(side)
                    .get(self.cursors[side])
                    .map(|s| (*s).clone())
            })
            .collect();
        if selected.iter().all(Option::is_none) {
            return;
        }
        let mut screens = [None, None];
        self.side_errors = [None, None];
        self.errors.clear();
        for (side, source) in selected.into_iter().enumerate() {
            if let Some(source) = source {
                match Screen::new(source) {
                    Ok(screen) => {
                        self.errors.extend(screen.replay.warnings.clone());
                        screens[side] = Some(screen);
                    }
                    Err(error) => {
                        self.errors.push(error.clone());
                        self.side_errors[side] = Some(error);
                    }
                }
            }
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
        self.analysis = false;
        self.analysis_scroll = [Cell::new(0), Cell::new(0)];
        self.focus = 0;
    }
    /// Returns `None` when Escape leaves head-to-head mode.
    pub fn key(&mut self, key: Key) -> Option<Reply> {
        if let Some(viewer) = &mut self.file_viewer {
            if viewer.key(key) {
                self.file_viewer = None;
            }
            return Some(Reply::Handled);
        }
        if !self.analysis
            && let Key::Click { column, row } = key
            && let Some(screens) = &self.screens
        {
            for (side, screen) in screens.iter().enumerate() {
                let Some(screen) = screen else {
                    continue;
                };
                if let Some(hit) =
                    crate::runs_files::clicked(&screen.file_hits.borrow(), column, row)
                {
                    let run = match &screen.source {
                        Source::Local(run) => Some(run.as_ref()),
                        Source::Public { .. } => None,
                    };
                    self.file_viewer = Some(crate::runs_files::Viewer::open(
                        run,
                        &screen.blocks,
                        hit.step,
                        &hit.path,
                    ));
                    self.clock.playing = false;
                    self.focus = side;
                    return Some(Reply::Handled);
                }
            }
        }
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
        if key == Key::Char('l') {
            self.toggle_learning();
            return Some(Reply::Handled);
        }
        if self.active() && self.analysis {
            let scroll = &self.analysis_scroll[self.focus];
            match key {
                Key::Back => {
                    self.screens = None;
                    self.analysis = false;
                    self.focus = 0;
                }
                Key::Char('q') => return Some(Reply::Quit),
                Key::Tab => self.focus = 1 - self.focus,
                Key::Up | Key::Char('k') => scroll.set(scroll.get().saturating_sub(1)),
                Key::Down | Key::Char('j') => scroll.set(scroll.get().saturating_add(1)),
                Key::PageUp => scroll.set(scroll.get().saturating_sub(20)),
                Key::PageDown => scroll.set(scroll.get().saturating_add(20)),
                Key::Home | Key::Char('g') => scroll.set(0),
                Key::End | Key::Char('G') => scroll.set(usize::MAX),
                _ => {}
            }
            return Some(Reply::Handled);
        }
        if let Some(screens) = &mut self.screens
            && let Key::WheelUp { column, row }
            | Key::WheelDown { column, row }
            | Key::Click { column, row } = key
        {
            // The mouse acts on the side under the pointer.
            let side = self
                .sides
                .get()
                .iter()
                .position(|rect| column >= rect.left() && column < rect.right());
            if let Some(side) = side
                && let Some(screen) = &screens[side]
                && !screen.details
                && !screen.blocks.is_empty()
            {
                self.focus = side;
                let last = screen.shown.get().saturating_sub(1);
                if screen.follow.get() || screen.selected.get().is_none() {
                    screen.selected.set(Some(last));
                }
                screen.follow.set(false);
                screen.free.set(true);
                match key {
                    Key::WheelUp { .. } => screen.scroll.set(screen.scroll.get().saturating_sub(3)),
                    Key::WheelDown { .. } => screen.scroll.set(screen.scroll.get() + 3),
                    _ => {
                        let hit = screen
                            .drawn
                            .borrow()
                            .iter()
                            .find(|(y, _)| *y == row)
                            .map(|(_, block)| *block);
                        if let Some(index) = hit {
                            screen.selected.set(Some(index));
                            let mut open = screen.expanded.borrow_mut();
                            if !open.insert(index) {
                                open.remove(&index);
                            }
                        }
                    }
                }
            }
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
                    if let Some(screen) = &screens[self.focus]
                        && !screen.details
                        && !screen.blocks.is_empty()
                    {
                        // The transcript view: move the selection, open steps.
                        let last = screen.shown.get().saturating_sub(1);
                        let here = if screen.follow.get() {
                            last
                        } else {
                            screen.selected.get().unwrap_or(last).min(last)
                        };
                        screen.free.set(false);
                        let pick = |to: usize| {
                            screen.follow.set(false);
                            screen.selected.set(Some(to.min(last)));
                        };
                        match key {
                            Key::Up | Key::Char('k') => pick(here.saturating_sub(1)),
                            Key::Down | Key::Char('j') => pick(here + 1),
                            Key::PageUp => pick(here.saturating_sub(10)),
                            Key::PageDown => pick(here + 10),
                            Key::Char('g') => pick(0),
                            Key::Char('G' | 'f') => {
                                screen.follow.set(true);
                                screen.selected.set(None);
                            }
                            Key::Enter => {
                                let mut open = screen.expanded.borrow_mut();
                                if !open.insert(here) {
                                    open.remove(&here);
                                }
                                pick(here);
                            }
                            Key::Char('e') => {
                                screen.all.set(!screen.all.get());
                                screen.expanded.borrow_mut().clear();
                            }
                            _ => {}
                        }
                    } else if let Some(screen) = &screens[self.focus] {
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
                let kept = self.selection();
                self.all_agents = !self.all_agents;
                self.restore(kept);
            }
            Key::Char('o') => {
                let kept = self.selection();
                self.right_local = !self.right_local;
                self.restore(kept);
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
        if let Some(viewer) = &self.file_viewer {
            viewer.render(area, buf, ladder);
            return;
        }
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
                    if self.analysis {
                        "Jev analysis"
                    } else if self.clock.playing {
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
                if self.analysis {
                    "l back to replay · Tab choose side · ↑/↓ PgUp/PgDn scroll the assessment"
                } else {
                    "l Jev analysis · Space play or pause · +/- speed · ←/→ seek 30s · n/b next or previous event · r restart · End jump to end"
                },
                ladder,
                Intensity::Half,
            );
            let left = Rect::new(area.x, area.y + 2, area.width / 2, area.height - 5);
            let right = Rect::new(left.right(), left.y, area.width - left.width, left.height);
            self.sides.set([left, right]);
            for (index, side) in [left, right].into_iter().enumerate() {
                if let Some(screen) = &screens[index] {
                    if self.analysis {
                        self.render_assessment(screen, index, side, buf, ladder);
                    } else {
                        screen.render(side, buf, ladder, &self.clock, self.focus == index);
                    }
                } else {
                    frame(side, buf, ladder.style(Intensity::Quarter));
                    let inner = Rect::new(
                        side.x + 1,
                        side.y + 1,
                        side.width.saturating_sub(2),
                        side.height.saturating_sub(2),
                    );
                    paragraph(
                        buf,
                        inner,
                        self.side_errors[index].as_deref().unwrap_or(
                            "No attempt selected for this side.\n\nPress Esc to choose a task and an attempt for each side.",
                        ),
                        ladder,
                    );
                }
            }
            line(
                buf,
                area,
                bottom,
                if self.analysis {
                    "Judgments cover the whole run, including events not replayed yet · l transcripts · Esc choose pair · q quit"
                } else {
                    "click path opens file · ↑/↓ select · enter open or close · e open all · g top · G latest and follow · Tab other side · d raw record · Esc choose pair · q quit"
                },
                ladder,
                Intensity::Half,
            );
        } else {
            line(
                buf,
                area,
                0,
                &format!(
                    "Head-to-head · {} · {} local · {} public attempts · / {}{}",
                    if self.learning.enabled {
                        "Jev learning order"
                    } else {
                        "newest first"
                    },
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
                "l newest first or Jev order · Tab ←/→ column · ↑/↓ choose · Enter load · / search tasks",
                ladder,
                Intensity::Half,
            );
            let task_width = area.width / 4;
            let side_width = (area.width - task_width) / 2;
            let hints = if self.learning.enabled && area.height >= 22 {
                4
            } else {
                0
            };
            let height = area.height - 5 - hints;
            let rects = [
                Rect::new(area.x, area.y + 2, task_width, height),
                Rect::new(area.x + task_width, area.y + 2, side_width, height),
                Rect::new(
                    area.x + task_width + side_width,
                    area.y + 2,
                    area.width - task_width - side_width,
                    height,
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
                .map(|s| self.learning.label(s))
                .collect::<Vec<_>>();
            let right = self
                .choices(1)
                .iter()
                .map(|s| self.learning.label(s))
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
            if hints > 0 {
                for side in 0..2 {
                    if let Some(source) = self.choices(side).get(self.cursors[side]) {
                        let rect = rects[side + 1];
                        let lines = self.learning.assessment(source);
                        let text = if self.learning.score(source).is_some() {
                            lines
                                .iter()
                                .skip(1)
                                .take(3)
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("\n")
                        } else {
                            lines.join("\n")
                        };
                        paragraph(
                            buf,
                            Rect::new(
                                rect.x + 1,
                                rect.bottom(),
                                rect.width.saturating_sub(2),
                                hints,
                            ),
                            &text,
                            ladder,
                        );
                    }
                }
            }
            line(
                buf,
                area,
                bottom,
                "a Coder One or all local agents · o public or local opponent · c clear search · Esc back",
                ladder,
                Intensity::Half,
            );
        }
        if self.analysis || self.learning.enabled {
            line(
                buf,
                area,
                bottom - 1,
                &self.learning.status(),
                ladder,
                Intensity::Full,
            );
        } else if let Some(error) = self.errors.first() {
            line(
                buf,
                area,
                bottom - 1,
                &format!(
                    "{} notices: {}",
                    self.errors.len(),
                    error.lines().next().unwrap_or(error)
                ),
                ladder,
                Intensity::Full,
            );
        }
    }
    fn render_assessment(
        &self,
        screen: &Screen,
        index: usize,
        area: Rect,
        buf: &mut Buffer,
        ladder: Ladder,
    ) {
        frame(
            area,
            buf,
            ladder.style(if self.focus == index {
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
            &screen.source.label(),
            ladder,
            Intensity::Full,
        );
        let text = self.learning.assessment(&screen.source).join("\n\n");
        let rows = wrap_rows(&text, usize::from(inner.width));
        let height = usize::from(inner.height.saturating_sub(2));
        let start = self.analysis_scroll[index]
            .get()
            .min(rows.len().saturating_sub(height));
        self.analysis_scroll[index].set(start);
        for (i, span) in rows.into_iter().skip(start).take(height).enumerate() {
            line(
                buf,
                inner,
                i as u16 + 2,
                &text[span],
                ladder,
                Intensity::Half,
            );
        }
    }
}

fn score_order(a: Option<f64>, b: Option<f64>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(a), Some(b)) => b.total_cmp(&a),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn paragraph(buf: &mut Buffer, area: Rect, text: &str, ladder: Ladder) {
    for (row, span) in wrap_rows(text, usize::from(area.width))
        .into_iter()
        .take(usize::from(area.height))
        .enumerate()
    {
        line(buf, area, row as u16, &text[span], ladder, Intensity::Full);
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

    #[test]
    fn replay_markdown_styles_prose_and_keeps_tool_evidence_literal() {
        use crate::runs_replay::{Event, Part};
        use ratatui::style::Modifier;
        let prose = "# Heading\n\n**Bold** *italic* `inline` ~~gone~~ [link](https://example.com)\n\n- one\n- two\n\n> quote\n\n```rust\n    let value = 2 * 3;\n```\n\n| Name | State |\n| --- | --- |\n| Parser | Ready |";
        let literal = "# literal output\n**still literal**\n- [ ] shell output";
        let event = Event {
            elapsed_ms: 1000,
            timing: "step timestamp",
            title: "agent".to_owned(),
            text: format!("{prose}\n\n{literal}"),
            record: prose.to_owned(),
            parts: vec![
                Part {
                    text: prose.to_owned(),
                    markdown: true,
                },
                Part {
                    text: literal.to_owned(),
                    markdown: false,
                },
            ],
        };
        let rows = event_rows(&event, false, 80, Ladder::default());
        let text = rows
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.starts_with("Heading\n"), "{text}");
        assert!(text.contains("• one") && text.contains("│ quote"), "{text}");
        assert!(
            text.contains("    let value = 2 * 3;") && !text.contains("```rust"),
            "{text}"
        );
        assert!(
            text.contains("Name │ State") && text.contains(literal),
            "{text}"
        );
        for (word, modifier) in [
            ("Heading", Modifier::BOLD),
            ("Bold", Modifier::BOLD),
            ("italic", Modifier::ITALIC),
            ("gone", Modifier::CROSSED_OUT),
            ("link", Modifier::UNDERLINED),
        ] {
            assert!(
                rows.iter()
                    .flat_map(|row| &row.spans)
                    .any(|span| span.content == word && span.style.add_modifier.contains(modifier)),
                "{word}: {rows:?}"
            );
        }
        let raw = event_rows(&event, true, 80, Ladder::default());
        assert!(
            raw.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
                .contains("**Bold** *italic* `inline`")
        );
        assert!(
            raw.iter()
                .flat_map(|row| &row.spans)
                .all(|span| !span.style.add_modifier.contains(Modifier::BOLD))
        );
    }

    #[test]
    fn markdown_keeps_full_long_code_and_reflows_when_the_pane_resizes() {
        use crate::runs_replay::{Event, Part};
        let source = format!(
            "```\n{}\nLAST CODE LINE\n```",
            "    untouched **code**\n".repeat(2000)
        );
        let event = Event {
            elapsed_ms: 0,
            timing: "step timestamp",
            title: "agent".to_owned(),
            text: source.clone(),
            record: source.clone(),
            parts: vec![Part {
                text: source,
                markdown: true,
            }],
        };
        let rows = event_rows(&event, false, 40, Ladder::default());
        assert!(rows.len() > 2000);
        assert!(rows.iter().any(|row| row.to_string() == "LAST CODE LINE"));
        assert_eq!(rows[0].to_string(), "    untouched **code**");
        let (_dir, mut pane) = pane();
        pane.query = "coq".to_owned();
        pane.key(Key::Enter);
        let screen = pane.screens.as_mut().unwrap()[0].as_mut().unwrap();
        screen.replay.events = vec![
            event,
            Event {
                elapsed_ms: 5000,
                timing: "step timestamp",
                title: "agent".to_owned(),
                text: "FUTURE".to_owned(),
                record: "**FUTURE**".to_owned(),
                parts: vec![Part {
                    text: "**FUTURE**".to_owned(),
                    markdown: true,
                }],
            },
        ];
        // These cover the raw event record; the transcript view is
        // tested through the Runs pane.
        screen.blocks.clear();
        screen.replay.duration_ms = 5000;
        pane.clock = Playback::new(5000);
        for width in [150, 80, 180] {
            let area = Rect::new(0, 0, width, 38);
            let mut buf = Buffer::empty(area);
            pane.render(area, &mut buf, Ladder::default());
            assert!(contents(&buf).contains("LAST CODE LINE"));
            assert!(!contents(&buf).contains("FUTURE"));
        }
        pane.key(Key::Char('d'));
        let area = Rect::new(0, 0, 150, 38);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf, Ladder::default());
        assert!(contents(&buf).contains("```"));
        pane.key(Key::Char('d'));
        pane.key(Key::End);
        pane.key(Key::Char('f'));
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf, Ladder::default());
        assert!(contents(&buf).contains("FUTURE"));
        assert!(!contents(&buf).contains("**FUTURE**"));
    }

    #[test]
    fn learning_order_preserves_pair_and_analysis_preserves_replay_position() {
        use crate::runs_learning::{Context, Judge, Store};
        use crate::runs_replay_learning::tests::{public, seed};
        let (dir, sources) = crate::runs::fixture_sources();
        let catalog = Catalog::load(sources);
        let mut older = catalog
            .runs
            .iter()
            .find(|r| r.agent == Agent::CoderOne && crate::runs_learning::rankable(r))
            .unwrap()
            .clone();
        older.task = "parser".to_owned();
        older.started_ms = Some(1000);
        let mut newer = older.clone();
        newer.task = "recent-task".to_owned();
        newer.trial = "newer".to_owned();
        newer.variant = Some("newer".to_owned());
        newer.started_ms = Some(i64::MAX);
        let local = vec![
            Source::Local(Box::new(older)),
            Source::Local(Box::new(newer)),
        ];
        let opponent = public(dir.path(), "aa");
        let mut store = Store::default();
        let context = Context::default();
        seed(&mut store, &local[0], &context, 4.0);
        seed(&mut store, &local[1], &context, 0.0);
        seed(&mut store, &opponent, &context, 3.0);
        let mut pane = Pane::from_sources(local, vec![opponent], vec![], None).with_learning(
            store,
            Judge::Off("offline".to_owned()),
            context,
        );
        assert_eq!(pane.visible_tasks()[0], "recent-task");
        let kept = pane.selection();
        pane.key(Key::Char('l'));
        assert!(pane.learning.enabled);
        assert_eq!(pane.visible_tasks()[0], "parser");
        assert_eq!(pane.selection(), kept);
        pane.key(Key::Home);
        pane.key(Key::Enter);
        assert!(
            pane.screens.as_ref().unwrap().iter().all(Option::is_some),
            "{:?}",
            pane.errors
        );
        pane.key(Key::Char('n'));
        pane.key(Key::Char(' '));
        let elapsed = pane.clock.elapsed_ms;
        pane.key(Key::Char('l'));
        assert!(pane.analysis);
        assert!(!pane.clock.playing);
        pane.advance(Duration::from_secs(30));
        assert_eq!(pane.clock.elapsed_ms, elapsed);
        let area = Rect::new(0, 0, 160, 48);
        let mut buffer = Buffer::empty(area);
        pane.render(area, &mut buffer, Ladder::default());
        let text = contents(&buffer);
        assert!(text.contains("Jev analysis"), "{text}");
        assert!(text.contains("Fable 5.1"), "{text}");
        assert_eq!(text.matches("Learning value").count(), 2, "{text}");
        pane.key(Key::Char('l'));
        assert!(!pane.analysis);
        assert_eq!(pane.clock.elapsed_ms, elapsed);
        pane.key(Key::Back);
        pane.key(Key::Char('l'));
        assert!(!pane.learning.enabled);
        assert_eq!(pane.visible_tasks()[0], "recent-task");
    }

    #[test]
    fn analysis_updates_while_replaying_and_search_l_remains_text() {
        use crate::runs_learning::{Context, Judge, Store};
        use crate::runs_replay_learning::tests::{public, recorded};
        let dir = tempfile::tempdir().unwrap();
        let sources = vec![public(dir.path(), "aa"), public(dir.path(), "bb")];
        let context = Context::default();
        let judge = Judge::Recorded(recorded(&sources, &context));
        let mut pane = Pane::from_sources(vec![], sources, vec![], None).with_learning(
            Store::default(),
            judge,
            context,
        );
        pane.key(Key::Char('/'));
        pane.key(Key::Char('l'));
        assert_eq!(pane.query, "l");
        assert!(!pane.learning.enabled);
        pane.key(Key::Back);
        let kept = pane.selection();
        pane.key(Key::Enter);
        pane.key(Key::Char('l'));
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while pane.learning.ranking() {
            assert!(std::time::Instant::now() < deadline);
            pane.advance(Duration::from_millis(10));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(pane.selection(), kept);
        assert!(
            pane.choices(1)
                .iter()
                .all(|s| pane.learning.score(s).is_some())
        );
        for area in [Rect::new(0, 0, 60, 14), Rect::new(2, 3, 120, 28)] {
            let mut buffer = Buffer::empty(area);
            pane.render(area, &mut buffer, Ladder::default());
            assert!(contents(&buffer).contains("Jev analysis"));
        }
    }

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
            parts: vec![],
            record: "complete record".to_owned(),
        }];
        // These cover the raw event record; the transcript view is
        // tested through the Runs pane.
        screen.blocks.clear();
        screen.replay.duration_ms = 0;
        let area = Rect::new(0, 0, 150, 38);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        assert!(contents(&buf).contains("FINAL SENTINEL"));
        p.key(Key::Char('g'));
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        assert!(contents(&buf).contains("step timestamp"));
    }

    #[test]
    fn file_viewer_in_replay_pauses_both_sides_and_preserves_the_clock() {
        use crate::runs_transcript::{Block, Kind};
        let (_dir, mut pane) = pane();
        pane.query = "coq".to_owned();
        pane.key(Key::Char('o'));
        pane.key(Key::Enter);
        for (side, screen) in pane.screens.as_mut().unwrap().iter_mut().enumerate() {
            let screen = screen.as_mut().unwrap();
            screen.blocks = vec![Block {
                at: Some(0),
                kind: Kind::Look {
                    what: "Read tests/T5.sh".to_owned(),
                    output: format!("side {side} contents"),
                },
            }];
        }
        pane.clock.elapsed_ms = 500.0;
        pane.clock.playing = true;
        let area = Rect::new(0, 0, 150, 38);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf, Ladder::default());
        let hit = pane.screens.as_ref().unwrap()[1]
            .as_ref()
            .unwrap()
            .file_hits
            .borrow()[0]
            .clone();
        pane.key(Key::Click {
            column: hit.cells.x,
            row: hit.cells.y,
        });
        assert!(!pane.clock.playing);
        assert_eq!(pane.focus, 1);
        pane.advance(Duration::from_secs(20));
        assert_eq!(pane.clock.elapsed_ms, 500.0);
        pane.render(area, &mut buf, Ladder::default());
        assert!(contents(&buf).contains("side 1 contents"));
        pane.key(Key::Back);
        assert!(pane.file_viewer.is_none());
        assert!(pane.active());
        assert_eq!(pane.clock.elapsed_ms, 500.0);
    }

    #[test]
    fn enter_opens_the_selected_step_in_a_replay_pane() {
        use crate::runs_transcript::{Block, Kind};
        let (_dir, mut p) = pane();
        p.query = "coq".to_owned();
        p.key(Key::Enter);
        let screen = p.screens.as_mut().unwrap()[0].as_mut().unwrap();
        screen.blocks = vec![
            Block {
                at: Some(0),
                kind: Kind::Tool {
                    name: "first tool".to_owned(),
                    input: String::new(),
                    output: "HIDDEN DETAIL".to_owned(),
                },
            },
            Block {
                at: Some(0),
                kind: Kind::Note("latest".to_owned()),
            },
        ];
        let area = Rect::new(0, 0, 150, 38);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        assert!(!contents(&buf).contains("HIDDEN DETAIL"));
        p.key(Key::Up);
        p.key(Key::Enter);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        assert!(contents(&buf).contains("HIDDEN DETAIL"));
        p.key(Key::Enter);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        assert!(!contents(&buf).contains("HIDDEN DETAIL"));
    }

    #[test]
    fn a_click_opens_a_step_and_the_wheel_scrolls_rows() {
        use crate::runs_transcript::{Block, Kind};
        let (_dir, mut p) = pane();
        p.query = "coq".to_owned();
        p.key(Key::Enter);
        let screen = p.screens.as_mut().unwrap()[0].as_mut().unwrap();
        let long = (0..200)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        screen.blocks = vec![
            Block {
                at: Some(0),
                kind: Kind::Tool {
                    name: "first tool".to_owned(),
                    input: String::new(),
                    output: long,
                },
            },
            Block {
                at: Some(0),
                kind: Kind::Note("latest".to_owned()),
            },
        ];
        let area = Rect::new(0, 0, 150, 38);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        let screen = p.screens.as_ref().unwrap()[0].as_ref().unwrap();
        let (row, _) = *screen
            .drawn
            .borrow()
            .iter()
            .find(|(_, block)| *block == 0)
            .unwrap();
        p.key(Key::Click { column: 10, row });
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        assert!(contents(&buf).contains("line 3"));
        let before = p.screens.as_ref().unwrap()[0]
            .as_ref()
            .unwrap()
            .scroll
            .get();
        p.key(Key::WheelDown { column: 10, row });
        p.key(Key::WheelDown { column: 10, row });
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        let screen = p.screens.as_ref().unwrap()[0].as_ref().unwrap();
        assert_eq!(screen.scroll.get(), before + 6);
        assert_eq!(screen.selected.get(), Some(0));
    }

    #[test]
    fn a_loaded_trace_opens_on_its_first_event_without_waiting() {
        let (_dir, mut p) = pane();
        p.query = "coq".to_owned();
        p.key(Key::Enter);
        let screen = p.screens.as_mut().unwrap()[0].as_mut().unwrap();
        // These cover the raw event record; the transcript view is
        // tested through the Runs pane.
        screen.blocks.clear();
        screen.replay.events = vec![crate::runs_replay::Event {
            elapsed_ms: 5000,
            timing: "host timestamp",
            title: "agent".to_owned(),
            text: "FIRST MESSAGE".to_owned(),
            parts: vec![],
            record: "FIRST RECORD".to_owned(),
        }];
        screen.replay.duration_ms = 5000;
        let area = Rect::new(0, 0, 150, 38);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        let text = contents(&buf);
        assert!(!text.contains("Transcript loaded."));
        assert!(text.contains("FIRST MESSAGE"));
    }

    #[test]
    fn a_new_computer_shows_the_missing_download_and_reloads_after_acquisition() {
        use crate::runs_replay::{PublicTrial, SCHEMA, public_sources};
        use sha2::{Digest, Sha256};

        let cache = tempfile::tempdir().unwrap();
        let bytes = br#"{"steps":[{"source":"agent","message":"PUBLIC TRANSCRIPT"}]}"#;
        let trial: PublicTrial = serde_json::from_value(serde_json::json!({
            "id":"aa", "file":"aa.json", "task":"coq-block-bound",
            "model":"Fable 5.1", "agent":"Claude Code", "effort":"max",
            "source_url":"https://example.com", "available":true,
            "sha256":format!("{:x}",Sha256::digest(bytes))
        }))
        .unwrap();
        let manifest = cache.path().join("manifest.json");
        std::fs::write(
            &manifest,
            serde_json::json!({"schema":SCHEMA,"trials":[trial]}).to_string(),
        )
        .unwrap();
        let sources = public_sources(cache.path(), &manifest).unwrap();
        assert!(sources[0].label().starts_with("[not on this computer]"));
        // Even with neither side loaded, show the cause inside the failed side.
        let mut p = Pane::from_sources(vec![], sources, vec![], None);
        p.key(Key::Enter);
        assert!(p.active());
        let area = Rect::new(0, 0, 150, 38);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        let right = buf
            .content
            .chunks(150)
            .flat_map(|row| &row[75..])
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(right.contains("Fable transcript is not on this computer."));
        assert!(right.contains("uv run python -m tbench.public_replays"));
        assert!(right.contains("Then press Esc and Enter"));

        std::fs::write(cache.path().join("aa.json"), bytes).unwrap();
        p.key(Key::Back);
        p.key(Key::Enter);
        assert!(p.errors.is_empty());
        p.render(area, &mut buf, crate::tui::ladder_from_environment());
        assert!(contents(&buf).contains("PUBLIC TRANSCRIPT"));
    }
}
