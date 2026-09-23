//! The Runs pane: recent Terminal-Bench runs in plain words, each run's
//! story, and its transcript drawn the way the Coder terminal draws a
//! conversation.
//!
//! It is the first thing `gym-terminal --terminal-bench` shows. The list
//! reads newest first, one run a line: when it started, how it came out,
//! the task and what it asks, the agent, the tests the verifier passed,
//! what it cost, and how long it took. Enter opens a run's story; `t`
//! switches to its transcript.
//!
//! The drawing is `coder-terminal`'s: the amber [`Ladder`], the hairline
//! [`frame`] with its [`rail`]s, the [`Scrollback`] that wraps each
//! transcript block once per width, the reply's Markdown through
//! [`markdown::render`], and a Jev judgment's full record through
//! [`DecisionView`]. Tone carries every distinction; hue never varies.
//!
//! The pane holds no terminal. It takes [`Key`]s and draws into a ratatui
//! [`Buffer`], so the same code runs in `gym-terminal` and in the tests.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

use coder_terminal::decision::{DecisionView, Origin, Outcome as Answer};
use coder_terminal::{
    Intensity, Ladder, Marks, Scrollback, frame, frame_for, markdown, rail, wrap_rows,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::runs::{self, Agent, Catalog, Filter, Outcome, Run, duration, when};
use crate::runs_story::{self, Detail, clock, margin_note};
use crate::runs_transcript::{Block, Kind, first_line};
use crate::tui::ladder_from_environment;

/// A key the pane understands, whatever terminal it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    /// Escape: go back, or cancel a search.
    Back,
    Backspace,
    Char(char),
}

/// What the pane asks of the terminal around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    /// The pane handled the key.
    Handled,
    /// The reader asked to leave.
    Quit,
    /// The reader pressed an expert view's key.
    Open(char),
}

/// Which half of a run is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Summary,
    Transcript,
}

/// One transcript block, ready to wrap.
#[derive(Clone)]
struct Entry {
    block: Block,
    index: usize,
    expanded: bool,
    /// The time the margin shows, or empty when it repeats the last one.
    time: String,
    ladder: Ladder,
}

/// Where a boxed block's frame goes.
#[derive(Clone, Debug, PartialEq)]
enum Boxed {
    /// The top rule, with the box's title and verdict on it.
    Top {
        title: String,
        verdict: String,
        loud: bool,
    },
    Middle,
    Bottom,
}

/// One drawn row: styled runs of text, which block it belongs to, and its
/// place in a box.
#[derive(Clone)]
struct Row {
    block: usize,
    spans: Vec<(String, Style)>,
    boxed: Option<Boxed>,
}

type Rows = Scrollback<Entry, Row, fn(&Entry, usize) -> Vec<Row>>;

/// An open run.
struct Open {
    id: String,
    detail: Detail,
    tab: Tab,
    details: bool,
    scroll: Cell<usize>,
    selected: usize,
    expanded: BTreeSet<usize>,
    all: bool,
    rows: RefCell<Option<Rows>>,
}

impl Open {
    fn new(detail: Detail, tab: Tab) -> Self {
        Open {
            id: detail.run.id(),
            detail,
            tab,
            details: false,
            scroll: Cell::new(0),
            selected: 0,
            expanded: BTreeSet::new(),
            all: false,
            rows: RefCell::new(None),
        }
    }

    fn blocks(&self) -> &[Block] {
        &self.detail.transcript.blocks
    }

    fn is_expanded(&self, index: usize) -> bool {
        self.all != self.expanded.contains(&index)
    }

    /// Throws the wrapped rows away, so the next frame wraps afresh.
    fn invalidate(&self) {
        self.rows.replace(None);
    }
}

/// The Runs pane.
pub struct Pane {
    catalog: Catalog,
    filter: Filter,
    cursor: usize,
    list_scroll: Cell<usize>,
    open: Option<Open>,
    /// The search being typed, when the reader pressed `/`, and the search
    /// it replaces.
    typing: Option<String>,
    before: String,
    ladder: Ladder,
    now: i64,
    read_at: i64,
    tick: u64,
}

impl Pane {
    /// A pane over `catalog`, drawn with the environment's ladder.
    #[must_use]
    pub fn new(catalog: Catalog) -> Self {
        let now = runs::now_ms();
        Pane {
            catalog,
            filter: Filter::default(),
            cursor: 0,
            list_scroll: Cell::new(0),
            open: None,
            typing: None,
            before: String::new(),
            ladder: ladder_from_environment(),
            now,
            read_at: now,
            tick: 0,
        }
    }

    /// The same pane with a fixed clock, for tests and printing.
    #[must_use]
    pub fn at(mut self, now: i64) -> Self {
        self.now = now;
        self.read_at = now;
        self
    }

    /// The same pane with another ladder.
    #[must_use]
    pub fn with_ladder(mut self, ladder: Ladder) -> Self {
        self.ladder = ladder;
        self
    }

    /// The runs the filter admits, as indices into the catalog.
    fn visible(&self) -> Vec<usize> {
        self.catalog
            .runs
            .iter()
            .enumerate()
            .filter(|(_, run)| self.filter.admits(run))
            .map(|(index, _)| index)
            .collect()
    }

    fn selected_run(&self) -> Option<&Run> {
        let visible = self.visible();
        visible
            .get(self.cursor.min(visible.len().saturating_sub(1)))
            .map(|&index| &self.catalog.runs[index])
    }

    /// Reads the runs again, and the open run's records when it may have
    /// moved on. The list keeps its place by run, not by row.
    pub fn refresh(&mut self, now: i64) {
        let kept = self.selected_run().map(Run::id);
        self.catalog.refresh(now);
        self.now = now;
        self.read_at = now;
        self.tick = self.tick.wrapping_add(1);
        if let Some(id) = kept
            && let Some(position) = self
                .visible()
                .iter()
                .position(|&index| self.catalog.runs[index].id() == id)
        {
            self.cursor = position;
        }
        if let Some(open) = &mut self.open
            && let Some(run) = self.catalog.runs.iter().find(|run| run.id() == open.id)
            && (open.detail.run.outcome == Outcome::Running || run.outcome == Outcome::Running)
        {
            let following = open.selected + 1 >= open.detail.transcript.blocks.len();
            open.detail = Detail::load(run);
            if following {
                open.selected = open.detail.transcript.blocks.len().saturating_sub(1);
            }
            open.invalidate();
        }
    }

    /// Whether the pane wants to be read again on a timer: always, since
    /// new runs start and running ones move.
    #[must_use]
    pub fn follows(&self) -> bool {
        true
    }

    /// Whether a run is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The open run's tab, when a run is open.
    #[must_use]
    pub fn tab(&self) -> Option<Tab> {
        self.open.as_ref().map(|open| open.tab)
    }

    /// Opens the selected run on `tab`.
    pub fn open_selected(&mut self, tab: Tab) {
        if let Some(run) = self.selected_run() {
            let detail = Detail::load(run);
            self.open = Some(Open::new(detail, tab));
        }
    }

    /// Handles one key.
    pub fn key(&mut self, key: Key) -> Reply {
        if let Some(draft) = &mut self.typing {
            match key {
                Key::Char(c) => draft.push(c),
                Key::Backspace => {
                    draft.pop();
                }
                Key::Enter => self.typing = None,
                Key::Back => {
                    self.filter.search = std::mem::take(&mut self.before);
                    self.typing = None;
                    return Reply::Handled;
                }
                _ => return Reply::Handled,
            }
            if let Some(draft) = &self.typing {
                self.filter.search = draft.clone();
            }
            self.cursor = 0;
            return Reply::Handled;
        }
        if self.open.is_some() {
            return self.run_key(key);
        }
        let length = self.visible().len();
        match key {
            Key::Up | Key::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            Key::Down | Key::Char('j') => {
                self.cursor = (self.cursor + 1).min(length.saturating_sub(1));
            }
            Key::PageUp => self.cursor = self.cursor.saturating_sub(10),
            Key::PageDown => self.cursor = (self.cursor + 10).min(length.saturating_sub(1)),
            Key::Home | Key::Char('g') => self.cursor = 0,
            Key::End | Key::Char('G') => self.cursor = length.saturating_sub(1),
            Key::Enter => self.open_selected(Tab::Summary),
            Key::Char('t') => self.open_selected(Tab::Transcript),
            Key::Char('/') => {
                self.before = self.filter.search.clone();
                self.typing = Some(self.filter.search.clone());
            }
            Key::Char('a') => {
                self.filter.agent = self.next_agent();
                self.cursor = 0;
            }
            Key::Char('o') => {
                let kinds = Outcome::kinds();
                self.filter.outcome = match self.filter.outcome {
                    None => Some(kinds[0]),
                    Some(word) => kinds
                        .iter()
                        .position(|kind| *kind == word)
                        .and_then(|index| kinds.get(index + 1))
                        .copied(),
                };
                self.cursor = 0;
            }
            Key::Char('c') | Key::Back => {
                self.filter = Filter::default();
                self.cursor = 0;
            }
            Key::Char('q') => return Reply::Quit,
            Key::Char(c) if c.is_ascii_digit() || "bmrfs".contains(c) => return Reply::Open(c),
            _ => {}
        }
        Reply::Handled
    }

    /// The next agent the filter shows, among those the runs have.
    fn next_agent(&self) -> Option<Agent> {
        let present: Vec<Agent> = Agent::ALL
            .into_iter()
            .filter(|agent| self.catalog.runs.iter().any(|run| run.agent == *agent))
            .collect();
        match self.filter.agent {
            None => present.first().copied(),
            Some(agent) => present
                .iter()
                .position(|a| *a == agent)
                .and_then(|index| present.get(index + 1))
                .copied(),
        }
    }

    fn run_key(&mut self, key: Key) -> Reply {
        let Some(open) = &mut self.open else {
            return Reply::Handled;
        };
        match key {
            Key::Back => {
                self.open = None;
                return Reply::Handled;
            }
            Key::Char('q') => return Reply::Quit,
            Key::Char('t') => {
                open.tab = match open.tab {
                    Tab::Summary => Tab::Transcript,
                    Tab::Transcript => Tab::Summary,
                };
                open.scroll.set(0);
                return Reply::Handled;
            }
            Key::Char(c) if c.is_ascii_digit() || "bmrfs".contains(c) => return Reply::Open(c),
            _ => {}
        }
        match open.tab {
            Tab::Summary => {
                let scroll = open.scroll.get();
                match key {
                    Key::Up | Key::Char('k') => open.scroll.set(scroll.saturating_sub(1)),
                    Key::Down | Key::Char('j') => open.scroll.set(scroll + 1),
                    Key::PageUp => open.scroll.set(scroll.saturating_sub(10)),
                    Key::PageDown => open.scroll.set(scroll + 10),
                    Key::Home | Key::Char('g') => open.scroll.set(0),
                    Key::End | Key::Char('G') => open.scroll.set(usize::MAX / 2),
                    Key::Char('d') => open.details = !open.details,
                    _ => {}
                }
            }
            Tab::Transcript => {
                let last = open.blocks().len().saturating_sub(1);
                match key {
                    Key::Up | Key::Char('k') => open.selected = open.selected.saturating_sub(1),
                    Key::Down | Key::Char('j') => open.selected = (open.selected + 1).min(last),
                    Key::PageUp => open.selected = open.selected.saturating_sub(10),
                    Key::PageDown => open.selected = (open.selected + 10).min(last),
                    Key::Home | Key::Char('g') => open.selected = 0,
                    Key::End | Key::Char('G') => open.selected = last,
                    Key::Enter | Key::Char(' ') => {
                        let index = open.selected;
                        if open.blocks().get(index).is_some_and(Block::expandable) {
                            if !open.expanded.insert(index) {
                                open.expanded.remove(&index);
                            }
                            open.invalidate();
                        }
                    }
                    Key::Char('e') => {
                        open.all = !open.all;
                        open.expanded.clear();
                        open.invalidate();
                    }
                    _ => {}
                }
            }
        }
        Reply::Handled
    }

    /// Draws the pane into `area`.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let field = Style::new().bg(self.ladder.background());
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].reset();
                buf[(x, y)].set_style(field);
            }
        }
        if area.width < 40 || area.height < 8 {
            buf.set_string(
                area.left(),
                area.top(),
                "window too small",
                self.style(Intensity::Half),
            );
            return;
        }
        match &self.open {
            None => self.render_list(area, buf),
            Some(open) => self.render_run(open, area, buf),
        }
    }

    fn style(&self, intensity: Intensity) -> Style {
        self.ladder.style(intensity).bg(self.ladder.background())
    }

    /// The header line: a title on the left, facts on the right.
    fn header(&self, area: Rect, buf: &mut Buffer, left: &[(String, Intensity)], right: &str) {
        let mut x = area.left() + 1;
        for (text, intensity) in left {
            let room = area.right().saturating_sub(x + 1);
            let (next, _) = buf.set_stringn(
                x,
                area.top(),
                text,
                usize::from(room),
                self.style(*intensity),
            );
            x = next;
        }
        let width = right.chars().count() as u16;
        if x + 2 + width < area.right() {
            buf.set_string(
                area.right() - 1 - width,
                area.top(),
                right,
                self.style(Intensity::Half),
            );
        }
    }

    /// The frame under the header, its rails, and the room inside.
    fn framed(
        &self,
        area: Rect,
        buf: &mut Buffer,
        top: (&str, &str),
        bottom: (&str, &str),
    ) -> Rect {
        let boxed = Rect::new(area.left(), area.top() + 1, area.width, area.height - 1);
        frame(boxed, buf, self.style(Intensity::Half));
        rail(
            boxed,
            buf,
            0,
            Some((top.0, self.style(Intensity::Full))),
            (!top.1.is_empty()).then_some((top.1, self.style(Intensity::Half))),
        );
        let keys = if bottom.0.chars().count() + 6 > usize::from(boxed.width) {
            bottom.1
        } else {
            bottom.0
        };
        rail(
            boxed,
            buf,
            boxed.height - 1,
            Some((keys, self.style(Intensity::ThreeQuarters))),
            None,
        );
        Rect::new(
            boxed.left() + 2,
            boxed.top() + 1,
            boxed.width.saturating_sub(4),
            boxed.height.saturating_sub(2),
        )
    }

    fn render_list(&self, area: Rect, buf: &mut Buffer) {
        let visible = self.visible();
        let running = self.catalog.running();
        let facts = format!(
            "{} runs{} · read {}",
            self.catalog.runs.len(),
            if running > 0 {
                format!(" · {running} running")
            } else {
                String::new()
            },
            match (self.now - self.read_at) / 1000 {
                0 => "just now".to_owned(),
                seconds => format!("{seconds}s ago"),
            }
        );
        self.header(
            area,
            buf,
            &[("Terminal-Bench runs".to_owned(), Intensity::Full)],
            &facts,
        );
        let title = match self.filter.describe() {
            Some(filter) => format!("Runs · showing {filter} · {}", visible.len()),
            None => "Runs, newest first".to_owned(),
        };
        let search;
        let bottom = match &self.typing {
            Some(draft) => {
                search = format!("search: {draft}▏  enter keeps it · esc cancels");
                (search.as_str(), search.as_str())
            }
            None => (
                "↑↓ move · enter open · t transcript · / search · a agent · o outcome · c clear · 1-9 expert views · q quit",
                "↑↓ enter t / a o c q",
            ),
        };
        let inner = self.framed(area, buf, (&title, ""), bottom);
        if inner.height < 4 {
            return;
        }
        let width = usize::from(inner.width);
        // The list, a rule, and a preview of the selected run.
        let preview_rows: u16 = 4;
        let list_rows = usize::from(inner.height.saturating_sub(preview_rows + 1));
        let widths = ListColumns::fit(width);
        buf.set_stringn(
            inner.left(),
            inner.top(),
            widths.row([
                "started", "outcome", "task", "agent", "tests", "cost", "time",
            ]),
            width,
            self.style(Intensity::Half),
        );
        if visible.is_empty() {
            buf.set_stringn(
                inner.left(),
                inner.top() + 1,
                if self.catalog.runs.is_empty() {
                    "No runs found. Start a Terminal-Bench job, or check the jobs directory."
                } else {
                    "No run matches. Press c to clear the filter."
                },
                width,
                self.style(Intensity::Half),
            );
            return;
        }
        let cursor = self.cursor.min(visible.len() - 1);
        let mut scroll = self.list_scroll.get();
        if cursor < scroll {
            scroll = cursor;
        } else if cursor >= scroll + list_rows {
            scroll = cursor + 1 - list_rows;
        }
        self.list_scroll.set(scroll);
        for (offset, &index) in visible.iter().skip(scroll).take(list_rows).enumerate() {
            let run = &self.catalog.runs[index];
            let y = inner.top() + 1 + offset as u16;
            let selected = scroll + offset == cursor;
            let columns = runs::columns(run, self.now);
            let tone = match run.outcome {
                Outcome::Failed => Intensity::Full,
                Outcome::Passed | Outcome::Running => Intensity::ThreeQuarters,
                Outcome::NotGraded(_) => Intensity::Half,
            };
            let mut style = self.style(if selected { Intensity::Full } else { tone });
            if selected {
                style = style.bg(self.ladder.selection());
                for x in inner.left() - 1..inner.right() {
                    buf[(x, y)].set_style(Style::new().bg(self.ladder.selection()));
                }
                buf[(inner.left() - 1, y)].set_char('▸').set_style(
                    self.ladder
                        .style(Intensity::Full)
                        .bg(self.ladder.selection()),
                );
            }
            let mut columns = columns;
            if run.outcome == Outcome::Running {
                columns[1] = columns[1].replacen('●', &frame_for(self.tick).to_string(), 1);
            }
            buf.set_stringn(inner.left(), y, widths.row(columns), width, style);
        }
        // The preview: what the selected run asked, and how it ended.
        let rule_y = inner.bottom() - preview_rows - 1 + 1;
        for x in inner.left()..inner.right() {
            buf[(x, rule_y - 1)]
                .set_char('─')
                .set_style(self.style(Intensity::Quarter));
        }
        if let Some(run) = self.selected_run() {
            let mut lines: Vec<(String, Intensity)> = Vec::new();
            let ask = run.ask.clone().unwrap_or_else(|| run.task.clone());
            for line in runs_story::wrap(&format!("{}: {ask}", run.task), width)
                .into_iter()
                .take(2)
            {
                lines.push((line, Intensity::ThreeQuarters));
            }
            let status = match &run.outcome {
                Outcome::NotGraded(why) => format!("Not graded: {why}."),
                Outcome::Running => match run.active_ms {
                    Some(active) => format!(
                        "Running for {}; last wrote something {}.",
                        run.elapsed_ms(self.now).map_or_else(String::new, duration),
                        when(active, self.now)
                    ),
                    None => "Running.".to_owned(),
                },
                Outcome::Passed | Outcome::Failed => format!(
                    "{} {} · {}.  Enter reads its story.",
                    run.agent_label(),
                    run.outcome.word(),
                    match run.tests {
                        Some(tests) => format!("{} of {} tests passed", tests.passed, tests.total),
                        None => "no test counts".to_owned(),
                    }
                ),
            };
            lines.push((status, Intensity::Half));
            lines.push((run.job.clone(), Intensity::Quarter));
            for (offset, (line, intensity)) in lines.iter().take(4).enumerate() {
                buf.set_stringn(
                    inner.left(),
                    rule_y + offset as u16,
                    line,
                    width,
                    self.style(*intensity),
                );
            }
        }
    }

    fn render_run(&self, open: &Open, area: Rect, buf: &mut Buffer) {
        let run = &open.detail.run;
        let outcome_tone = match run.outcome {
            Outcome::Failed => Intensity::Full,
            Outcome::NotGraded(_) => Intensity::Half,
            _ => Intensity::ThreeQuarters,
        };
        let word = if run.outcome == Outcome::Running {
            format!("{} running", frame_for(self.tick))
        } else {
            format!("{} {}", run.outcome.mark(), run.outcome.word())
        };
        self.header(
            area,
            buf,
            &[
                (format!("{}  ", run.task), Intensity::Full),
                (word, outcome_tone),
            ],
            &runs_story::byline(run, self.now),
        );
        let (title, hint, keys, short) = match open.tab {
            Tab::Summary => (
                "Summary",
                "t shows the transcript",
                "↑↓ scroll · t transcript · d details · esc back · q quit",
                "↑↓ t d esc q",
            ),
            Tab::Transcript => (
                "Transcript",
                "t shows the summary",
                "↑↓ move · enter open or close · e open all · t summary · esc back · q quit",
                "↑↓ enter e t esc q",
            ),
        };
        let inner = self.framed(area, buf, (title, hint), (keys, short));
        match open.tab {
            Tab::Summary => self.render_summary(open, inner, buf),
            Tab::Transcript => self.render_transcript(open, inner, buf),
        }
    }

    fn summary_lines(&self, open: &Open, width: usize) -> Vec<(String, Intensity)> {
        let mut lines = Vec::new();
        for paragraph in runs_story::summary(&open.detail, self.now) {
            lines.push((paragraph.heading.clone(), Intensity::Full));
            for line in runs_story::wrap(&paragraph.text, width) {
                lines.push((line, Intensity::ThreeQuarters));
            }
            lines.push((String::new(), Intensity::Half));
        }
        lines.push((
            format!(
                "The transcript has {}. Press t to read it.",
                crate::runs_transcript::plural(open.blocks().len(), "step")
            ),
            Intensity::Half,
        ));
        lines.push((String::new(), Intensity::Half));
        if open.details {
            lines.push(("Details".to_owned(), Intensity::Full));
            for (name, value) in runs_story::details(&open.detail) {
                for (index, line) in runs_story::wrap(&value, width.saturating_sub(12))
                    .into_iter()
                    .enumerate()
                {
                    let label = if index == 0 { name.as_str() } else { "" };
                    lines.push((format!("{label:<11} {line}"), Intensity::Half));
                }
            }
        } else {
            lines.push((
                "Press d for the details experts use: job, files, and versions.".to_owned(),
                Intensity::Quarter,
            ));
        }
        lines
    }

    fn render_summary(&self, open: &Open, inner: Rect, buf: &mut Buffer) {
        let width = usize::from(inner.width);
        let lines = self.summary_lines(open, width);
        let room = usize::from(inner.height);
        let scroll = open.scroll.get().min(lines.len().saturating_sub(room));
        open.scroll.set(scroll);
        for (offset, (line, intensity)) in lines.iter().skip(scroll).take(room).enumerate() {
            let mut style = self.style(*intensity);
            if *intensity == Intensity::Full {
                style = style.add_modifier(Modifier::BOLD);
            }
            buf.set_stringn(
                inner.left(),
                inner.top() + offset as u16,
                line,
                width,
                style,
            );
        }
    }

    fn render_transcript(&self, open: &Open, inner: Rect, buf: &mut Buffer) {
        let width = usize::from(inner.width) + 2;
        if open.blocks().is_empty() {
            buf.set_stringn(
                inner.left(),
                inner.top(),
                "No transcript was kept for this run.",
                width,
                self.style(Intensity::Half),
            );
            return;
        }
        let mut cache = open.rows.borrow_mut();
        let rows = cache.get_or_insert_with(|| self.scrollback(open));
        let rows: Vec<&Row> = rows.rows(width, |_| true);
        let room = usize::from(inner.height);
        let selected = open.selected.min(open.blocks().len() - 1);
        let first = rows
            .iter()
            .position(|row| row.block == selected)
            .unwrap_or(0);
        let last = rows
            .iter()
            .rposition(|row| row.block == selected)
            .unwrap_or(first);
        let mut scroll = open.scroll.get();
        if first < scroll {
            scroll = first;
        } else if last >= scroll + room {
            scroll = if last - first < room {
                last + 1 - room
            } else {
                first
            };
        }
        scroll = scroll.min(rows.len().saturating_sub(1));
        open.scroll.set(scroll);
        let left = inner.left() - 2;
        let shown: Vec<&Row> = rows.iter().skip(scroll).take(room).copied().collect();
        for (offset, row) in shown.iter().enumerate() {
            let y = inner.top() + offset as u16;
            let mine = row.block == selected;
            let mut x = left + 1;
            for (text, style) in &row.spans {
                let style = if mine {
                    style.bg(self.ladder.selection())
                } else {
                    *style
                };
                let room = (inner.right() + 1).saturating_sub(x);
                let (next, _) = buf.set_stringn(x, y, text, usize::from(room), style);
                x = next;
            }
            if mine {
                for cx in left + 2..inner.right() + 1 {
                    let cell = &mut buf[(cx, y)];
                    cell.set_bg(self.ladder.selection());
                }
                buf[(left + 1, y)].set_char('▌').set_style(
                    self.ladder
                        .style(Intensity::Full)
                        .bg(self.ladder.background()),
                );
            }
        }
        // Boxes: each boxed block's visible rows get the hairline frame,
        // and its title rides the top rule when the top is on screen.
        let mut offset = 0;
        while offset < shown.len() {
            if shown[offset].boxed.is_none() {
                offset += 1;
                continue;
            }
            let block = shown[offset].block;
            let start = offset;
            while offset < shown.len()
                && shown[offset].block == block
                && shown[offset].boxed.is_some()
            {
                offset += 1;
            }
            let x = left + MARGIN as u16;
            let area = Rect::new(
                x,
                inner.top() + start as u16,
                inner.right().saturating_sub(x),
                (offset - start) as u16,
            );
            let tone = if block == selected {
                Intensity::ThreeQuarters
            } else {
                Intensity::Half
            };
            frame(area, buf, self.style(tone));
            if let Some(Boxed::Top {
                title,
                verdict,
                loud,
            }) = &shown[start].boxed
            {
                rail(
                    area,
                    buf,
                    0,
                    Some((title, self.style(Intensity::Full))),
                    Some((
                        verdict,
                        self.style(if *loud {
                            Intensity::Full
                        } else {
                            Intensity::Half
                        }),
                    )),
                );
            }
        }
    }

    /// The transcript's blocks in a fresh scrollback.
    fn scrollback(&self, open: &Open) -> Rows {
        let mut rows: Rows = Scrollback::new(block_rows as fn(&Entry, usize) -> Vec<Row>);
        let start = open.blocks().iter().find_map(|block| block.at);
        let mut last_time = String::new();
        for (index, block) in open.blocks().iter().enumerate() {
            let time = clock(block.at, start);
            let shown = if time == last_time {
                String::new()
            } else {
                time.clone()
            };
            if !time.is_empty() {
                last_time = time;
            }
            rows.push(Entry {
                block: block.clone(),
                index,
                expanded: open.is_expanded(index),
                time: shown,
                ladder: self.ladder,
            });
        }
        rows
    }

    /// The pane as plain text, `width` wide and `height` tall.
    #[must_use]
    pub fn to_text(&self, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        self.render(area, &mut buf);
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol().to_owned())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The list as plain lines, for `--print`.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let widths = ListColumns::fit(146);
        let mut lines = vec![widths.row([
            "started", "outcome", "task", "agent", "tests", "cost", "time",
        ])];
        for index in self.visible() {
            lines.push(widths.row(runs::columns(&self.catalog.runs[index], self.now)));
        }
        lines
    }
}

/// The list's column widths at a width.
struct ListColumns {
    widths: [usize; 7],
}

impl ListColumns {
    fn fit(width: usize) -> Self {
        // started, outcome, task, agent, tests, cost, time
        let mut widths = [12, 14, 0, 24, 6, 7, 8];
        if width < 100 {
            widths[3] = 13;
            widths[4] = 0;
            widths[5] = 0;
        }
        let fixed: usize =
            widths.iter().sum::<usize>() + 2 * widths.iter().filter(|w| **w > 0).count();
        widths[2] = width.saturating_sub(fixed).max(12);
        ListColumns { widths }
    }

    fn row<S: AsRef<str>>(&self, columns: [S; 7]) -> String {
        let mut out = String::new();
        for (index, (text, width)) in columns.iter().zip(self.widths).enumerate() {
            if width == 0 {
                continue;
            }
            let text = text.as_ref();
            let clipped = clip(text, width);
            let pad = width.saturating_sub(clipped.chars().count());
            if matches!(index, 4..=6) {
                out.push_str(&" ".repeat(pad));
                out.push_str(&clipped);
            } else {
                out.push_str(&clipped);
                out.push_str(&" ".repeat(pad));
            }
            out.push_str("  ");
        }
        out.trim_end().to_owned()
    }
}

fn clip(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// The cells before a block's text: the selection bar, the time, a space,
/// the open-or-closed mark, and a space.
const MARGIN: usize = 9;

/// The rows one transcript block draws at `width`.
fn block_rows(entry: &Entry, width: usize) -> Vec<Row> {
    let mut out = Builder::new(entry, width);
    let block = &entry.block;
    let inner = out.inner;
    match &block.kind {
        Kind::Task(text) => {
            out.push(vec![(
                "› The task".to_owned(),
                out.tone(Intensity::Full).add_modifier(Modifier::BOLD),
            )]);
            out.markdown(&out.limited(text, 8), "  ");
        }
        Kind::Say(text) => out.markdown(&out.limited(text, 12), ""),
        Kind::Report(text) => {
            out.push(vec![(
                "■ Final report".to_owned(),
                out.tone(Intensity::Full).add_modifier(Modifier::BOLD),
            )]);
            out.markdown(&out.limited(text, 10), "  ");
        }
        Kind::Think(text) => {
            let room = inner.saturating_sub(10).max(8);
            let shown = if entry.expanded {
                text.clone()
            } else {
                clip(&first_line(text), room)
            };
            let style = out.tone(Intensity::Quarter).add_modifier(Modifier::ITALIC);
            for (n, range) in wrap_rows(&shown, room).into_iter().enumerate() {
                let lead = if n == 0 { "thinking  " } else { "          " };
                out.push(vec![
                    (lead.to_owned(), out.tone(Intensity::Quarter)),
                    (shown[range].to_owned(), style),
                ]);
            }
        }
        Kind::Section { title, note, .. } => {
            let right = margin_note(block).unwrap_or_default();
            let tail = if right.is_empty() {
                "──".to_owned()
            } else {
                format!(" {right} ──")
            };
            let fill = inner.saturating_sub(title.chars().count() + 4 + tail.chars().count());
            out.push(vec![
                ("── ".to_owned(), out.tone(Intensity::Half)),
                (
                    format!("{title} "),
                    out.tone(Intensity::Full).add_modifier(Modifier::BOLD),
                ),
                ("─".repeat(fill), out.tone(Intensity::Half)),
                (tail, out.tone(Intensity::Half)),
            ]);
            if let Some(note) = note {
                out.wrapped("   ", note, Intensity::Half);
            }
        }
        Kind::Command { exit, failed, .. } => {
            let headline = block.headline();
            let (command, status) = match headline.rsplit_once("   (") {
                Some((command, status)) => (command.to_owned(), format!("   ({status}")),
                None => (headline, String::new()),
            };
            let room = inner.saturating_sub(status.chars().count()).max(10);
            let loud = *failed || exit.is_some_and(|code| code != 0);
            out.push(vec![
                (clip(&command, room), out.tone(Intensity::ThreeQuarters)),
                (
                    status,
                    out.tone(if loud {
                        Intensity::Full
                    } else {
                        Intensity::Quarter
                    }),
                ),
            ]);
            out.body(block.body(entry.expanded), Intensity::Half);
        }
        Kind::Edit { .. } => {
            out.push(vec![
                ("± ".to_owned(), out.tone(Intensity::Half)),
                (
                    clip(&block.headline(), inner.saturating_sub(2)),
                    out.tone(Intensity::ThreeQuarters),
                ),
            ]);
            out.body(block.body(entry.expanded), Intensity::Half);
        }
        Kind::Look { .. } | Kind::Tool { .. } => {
            out.push(vec![
                ("· ".to_owned(), out.tone(Intensity::Quarter)),
                (
                    clip(&block.headline(), inner.saturating_sub(2)),
                    out.tone(Intensity::Half),
                ),
            ]);
            out.body(block.body(entry.expanded), Intensity::Quarter);
        }
        Kind::Decision {
            question,
            answer,
            detail,
            probability,
            milliseconds,
            cost_usd,
        } => {
            let room = inner.saturating_sub(2);
            let answer = clip(
                answer,
                room.saturating_sub(question.chars().count() + 1).max(8),
            );
            out.push(vec![
                ("◆ ".to_owned(), out.tone(Intensity::Half)),
                (format!("{question} "), out.tone(Intensity::Half)),
                (answer.clone(), out.tone(Intensity::ThreeQuarters)),
            ]);
            if entry.expanded {
                if let Some(probability) = probability {
                    // The Coder terminal's own record of a judgment.
                    let view = DecisionView {
                        origin: Origin::Metered,
                        profile: "Jev".to_owned(),
                        destination: "TypeSafe System One".to_owned(),
                        selection: question.clone(),
                        outcome: Answer::Answered {
                            answer,
                            probability: *probability,
                        },
                        review: None,
                        model: None,
                        latency_ms: *milliseconds,
                        cost: *cost_usd,
                    };
                    for line in view.expanded(room) {
                        let mut spans = vec![("│ ".to_owned(), out.tone(Intensity::Quarter))];
                        spans.extend(
                            line.spans
                                .into_iter()
                                .map(|span| (span.text, out.tone(span.intensity))),
                        );
                        out.push(spans);
                    }
                }
                out.body(
                    detail
                        .iter()
                        .map(|(name, value)| format!("{name}: {value}"))
                        .collect(),
                    Intensity::Half,
                );
            }
        }
        Kind::Check {
            title,
            verdict,
            lines,
            good,
        } => out.boxed(
            title,
            verdict,
            lines.len(),
            *good,
            block.body(entry.expanded),
        ),
        Kind::Note(text) => {
            let room = inner.saturating_sub(2);
            out.push(vec![
                ("· ".to_owned(), out.tone(Intensity::Quarter)),
                (clip(text, room), out.tone(Intensity::Quarter)),
            ]);
        }
    }
    if out.rows.is_empty() {
        out.push(Vec::new());
    }
    // A quiet line after the task and each report.
    if matches!(block.kind, Kind::Report(_) | Kind::Task(_)) {
        out.rows.push(Row {
            block: entry.index,
            spans: Vec::new(),
            boxed: None,
        });
    }
    out.rows
}

/// Builds one block's rows: the margin on the first row, the hang on the
/// rest.
struct Builder<'a> {
    entry: &'a Entry,
    /// The cells the block's text may take.
    inner: usize,
    rows: Vec<Row>,
}

impl<'a> Builder<'a> {
    fn new(entry: &'a Entry, width: usize) -> Self {
        Builder {
            entry,
            inner: width.saturating_sub(MARGIN + 1).max(10),
            rows: Vec::new(),
        }
    }

    fn tone(&self, intensity: Intensity) -> Style {
        let ladder = self.entry.ladder;
        ladder.style(intensity).bg(ladder.background())
    }

    /// The cells before the text: the time and the open-or-closed mark on
    /// the first row, blank after it.
    fn margin(&self) -> Vec<(String, Style)> {
        if self.rows.is_empty() {
            let chevron = match (self.entry.block.expandable(), self.entry.expanded) {
                (false, _) => " ",
                (true, false) => "▸",
                (true, true) => "▾",
            };
            vec![
                (
                    format!("{:>6} ", self.entry.time),
                    self.tone(Intensity::Quarter),
                ),
                (format!("{chevron} "), self.tone(Intensity::Half)),
            ]
        } else {
            vec![(" ".repeat(MARGIN - 1), self.tone(Intensity::Quarter))]
        }
    }

    fn push(&mut self, spans: Vec<(String, Style)>) {
        self.push_boxed(spans, None);
    }

    fn push_boxed(&mut self, spans: Vec<(String, Style)>, boxed: Option<Boxed>) {
        let mut all = self.margin();
        all.extend(spans);
        self.rows.push(Row {
            block: self.entry.index,
            spans: all,
            boxed,
        });
    }

    /// `text` cut to `limit` lines while the block is closed, with a line
    /// that says how to read the rest.
    fn limited(&self, text: &str, limit: usize) -> String {
        let lines: Vec<&str> = text.lines().collect();
        if self.entry.expanded || lines.len() <= limit {
            return text.to_owned();
        }
        format!(
            "{}\n\n… {} more lines; press enter to read them",
            lines[..limit].join("\n"),
            lines.len() - limit
        )
    }

    /// Plain text, wrapped to hang under `prefix`.
    fn wrapped(&mut self, prefix: &str, text: &str, intensity: Intensity) {
        let hang = " ".repeat(prefix.chars().count());
        let room = self.inner.saturating_sub(hang.len()).max(8);
        for (n, range) in wrap_rows(text, room).into_iter().enumerate() {
            let lead = if n == 0 {
                prefix.to_owned()
            } else {
                hang.clone()
            };
            let spans = vec![
                (lead, self.tone(Intensity::Half)),
                (text[range].to_owned(), self.tone(intensity)),
            ];
            self.push(spans);
        }
    }

    /// A reply's Markdown, laid out and styled the way the Coder terminal
    /// draws one.
    fn markdown(&mut self, text: &str, lead: &str) {
        let ladder = self.entry.ladder;
        for rendered in markdown::render(text) {
            let hang = " ".repeat(lead.chars().count() + rendered.hang);
            let room = self.inner.saturating_sub(hang.len()).max(8);
            let base = self.tone(rendered.intensity);
            for (n, range) in wrap_rows(&rendered.marked.text, room)
                .into_iter()
                .enumerate()
            {
                let first = if n == 0 {
                    lead.to_owned()
                } else {
                    hang.clone()
                };
                let mut spans = vec![(first, self.tone(Intensity::Half))];
                for (text, marks) in rendered.marked.runs_in(range) {
                    spans.push((text, marked(base, ladder, &marks)));
                }
                self.push(spans);
            }
        }
    }

    /// Output or detail lines under a quiet rule, each cut to the row.
    fn body(&mut self, lines: Vec<String>, intensity: Intensity) {
        let room = self.inner.saturating_sub(2).max(8);
        for line in lines {
            let line = clip(&line, room);
            let style = if line.starts_with("… ") || line.starts_with("- ") {
                self.tone(Intensity::Quarter)
            } else if line.starts_with("+ ") {
                self.tone(Intensity::ThreeQuarters)
            } else {
                self.tone(intensity)
            };
            let spans = vec![
                ("│ ".to_owned(), self.tone(Intensity::Quarter)),
                (line, style),
            ];
            self.push(spans);
        }
    }

    /// A check, a repair, or a round as its own box: the frame is drawn
    /// over these rows when they are on screen.
    fn boxed(
        &mut self,
        title: &str,
        verdict: &str,
        lines: usize,
        good: Option<bool>,
        body: Vec<String>,
    ) {
        let loud = good == Some(false);
        let mark = match good {
            Some(true) => "✓",
            Some(false) => "✗",
            None => "·",
        };
        self.push_boxed(
            Vec::new(),
            Some(Boxed::Top {
                title: title.to_owned(),
                verdict: mark.to_owned(),
                loud,
            }),
        );
        let room = self.inner.saturating_sub(4).max(8);
        let mut content = vec![verdict.to_owned()];
        content.extend(body);
        if !self.entry.expanded && lines > 3 {
            content.push(format!("… {} more; press enter", lines - 3));
        }
        let tone = self.tone(if loud {
            Intensity::Full
        } else {
            Intensity::ThreeQuarters
        });
        for line in content {
            for range in wrap_rows(&line, room) {
                self.push_boxed(
                    vec![(format!("  {}", &line[range]), tone)],
                    Some(Boxed::Middle),
                );
            }
        }
        self.push_boxed(Vec::new(), Some(Boxed::Bottom));
    }
}

/// The style one marked run draws at, the way the Coder terminal draws a
/// reply: code at full amber, and bold, italic, and links as modifiers.
fn marked(base: Style, ladder: Ladder, marks: &Marks) -> Style {
    let mut style = if marks.code {
        ladder.style(Intensity::Full).bg(ladder.background())
    } else {
        base
    };
    if marks.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if marks.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if marks.strike {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    if marks.link.is_some() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_terminal::Colors;

    fn pane() -> (tempfile::TempDir, Pane) {
        let (dir, sources) = crate::runs::fixture_sources();
        let catalog = Catalog::load(sources);
        let pane = Pane::new(catalog)
            .at(runs::now_ms())
            .with_ladder(crate::tui::ladder(Colors::None));
        (dir, pane)
    }

    /// Moves the list's cursor to the run whose task is `task`.
    fn select(pane: &mut Pane, task: &str) {
        pane.key(Key::Home);
        for _ in 0..pane.catalog.runs.len() {
            if pane.selected_run().is_some_and(|run| run.task == task) {
                return;
            }
            pane.key(Key::Down);
        }
        panic!("no {task} run in the list");
    }

    #[test]
    fn the_list_says_what_each_run_was_and_how_it_came_out() {
        let (_dir, pane) = pane();
        let text = pane.to_text(140, 30);
        assert!(text.contains("Terminal-Bench runs"), "{text}");
        assert!(text.contains("5 runs · 1 running"), "{text}");
        assert!(text.contains("✓ passed"), "{text}");
        assert!(text.contains("✗ failed"), "{text}");
        assert!(text.contains("○ not graded"), "{text}");
        assert!(
            text.contains("coq-block-bound — Prove `target_theorem`"),
            "{text}"
        );
        assert!(text.contains("Claude Code · Opus 5.5"), "{text}");
        assert!(text.contains("95/97"), "{text}");
        assert!(text.contains("$2.92"), "{text}");
        // The footer names the keys a newcomer needs.
        for key in [
            "↑↓ move",
            "enter open",
            "t transcript",
            "/ search",
            "q quit",
        ] {
            assert!(text.contains(key), "{key}: {text}");
        }
        // No digests or invocation ids in the default view.
        assert!(!text.contains("sha256"), "{text}");
        assert!(!text.contains("inv-"), "{text}");
    }

    #[test]
    fn a_narrow_window_still_reads() {
        let (_dir, pane) = pane();
        let text = pane.to_text(80, 20);
        assert!(text.contains("coq-block-bound"), "{text}");
        assert!(text.contains("↑↓ enter t / a o c q"), "{text}");
    }

    #[test]
    fn enter_opens_the_story_and_t_the_transcript() {
        let (_dir, mut pane) = pane();
        select(&mut pane, "coq-block-bound");
        assert_eq!(pane.key(Key::Enter), Reply::Handled);
        assert_eq!(pane.tab(), Some(Tab::Summary));
        let text = pane.to_text(140, 45);
        for phrase in [
            "Summary",
            "What the task asked",
            "What happened",
            "Why it passed",
            "Cost and time",
            "esc back",
        ] {
            assert!(text.contains(phrase), "{phrase}: {text}");
        }
        assert!(
            !text.contains("7a02024adccc"),
            "the policy digest hides behind d: {text}"
        );
        pane.key(Key::Char('d'));
        let text = pane.to_text(140, 45);
        pane.key(Key::End);
        let end = pane.to_text(140, 45);
        assert!(
            text.contains("Details") || end.contains("Details"),
            "{text}\n{end}"
        );
        assert!(end.contains("7a02024adccc"), "{end}");

        pane.key(Key::Char('t'));
        assert_eq!(pane.tab(), Some(Tab::Transcript));
        let text = pane.to_text(140, 45);
        assert!(text.contains("Transcript"), "{text}");
        assert!(text.contains("› The task"), "{text}");
        assert!(text.contains("Which executor starts?"), "{text}");
        assert!(
            text.contains("Claude Code on Opus 5.5 takes over"),
            "{text}"
        );

        assert_eq!(pane.key(Key::Back), Reply::Handled);
        assert!(!pane.is_open());
        assert_eq!(pane.key(Key::Char('q')), Reply::Quit);
    }

    #[test]
    fn a_command_opens_to_show_its_output_and_closes_again() {
        let (_dir, mut pane) = pane();
        select(&mut pane, "wal-recovery-ordering");
        pane.key(Key::Char('t'));
        // The first command is two steps in: the task, then the command.
        pane.key(Key::Down);
        let closed = pane.to_text(120, 40);
        assert!(closed.contains("$ cd /app; find . -type f"), "{closed}");
        assert!(closed.contains("▸ $ cd /app; find"), "{closed}");
        pane.key(Key::Enter);
        let open = pane.to_text(120, 40);
        assert!(open.contains("▾ $ cd /app; find"), "{open}");
        assert!(open.contains("│ "), "the output shows under a rule: {open}");
        pane.key(Key::Enter);
        let again = pane.to_text(120, 40);
        assert!(again.contains("▸ $ cd /app; find"), "{again}");
        assert!(!again.contains("▾ $ cd /app; find"), "{again}");
    }

    #[test]
    fn checks_draw_in_a_hairline_box() {
        let (_dir, mut pane) = pane();
        select(&mut pane, "coq-block-bound");
        pane.key(Key::Char('t'));
        let index = pane
            .open
            .as_ref()
            .unwrap()
            .blocks()
            .iter()
            .position(|block| matches!(&block.kind, Kind::Check { title, .. } if title == "Coder One's checks"))
            .unwrap();
        pane.open.as_mut().unwrap().selected = index;
        let text = pane.to_text(120, 40);
        assert!(text.contains("┌─ Coder One's checks"), "{text}");
        assert!(text.contains("1 inconclusive, 1 passed"), "{text}");
        assert!(text.contains("└──"), "{text}");
    }

    #[test]
    fn search_and_filters_narrow_the_list() {
        let (_dir, mut pane) = pane();
        pane.key(Key::Char('/'));
        for c in "wal".chars() {
            pane.key(Key::Char(c));
        }
        let typing = pane.to_text(140, 30);
        assert!(typing.contains("search: wal"), "{typing}");
        pane.key(Key::Enter);
        let text = pane.to_text(140, 30);
        assert!(text.contains("wal-recovery-ordering"), "{text}");
        assert!(!text.contains("coq-block-bound"), "{text}");
        pane.key(Key::Char('c'));
        assert_eq!(pane.visible().len(), 5);

        // The agent filter cycles through the agents the runs have.
        pane.key(Key::Char('a'));
        assert_eq!(pane.filter.agent, Some(Agent::CoderOne));
        assert_eq!(pane.visible().len(), 3);
        pane.key(Key::Char('a'));
        assert_eq!(pane.filter.agent, Some(Agent::ClaudeCode));
        pane.key(Key::Char('c'));

        pane.key(Key::Char('o'));
        assert_eq!(pane.filter.outcome, Some("passed"));
        assert_eq!(pane.visible().len(), 1);
        pane.key(Key::Char('o'));
        assert_eq!(pane.filter.outcome, Some("failed"));
        assert_eq!(pane.visible().len(), 2);
        // Escape in the list clears the filters.
        pane.key(Key::Back);
        assert_eq!(pane.visible().len(), 5);

        // Esc while typing gives the old search back.
        pane.key(Key::Char('/'));
        pane.key(Key::Char('z'));
        pane.key(Key::Back);
        assert_eq!(pane.filter.search, "");
    }

    #[test]
    fn the_expert_views_stay_behind_their_keys() {
        let (_dir, mut pane) = pane();
        assert_eq!(pane.key(Key::Char('1')), Reply::Open('1'));
        assert_eq!(pane.key(Key::Char('f')), Reply::Open('f'));
    }

    #[test]
    fn a_running_trial_follows_its_live_log() {
        let (dir, mut pane) = pane();
        select(&mut pane, "fin-saccr-rwa");
        pane.key(Key::Char('t'));
        pane.key(Key::End);
        let before = pane.open.as_ref().unwrap().blocks().len();
        let log = dir.path().join(
            "jobs/tb4--coder-one-tunable-v6--fin-saccr-rwa/fin-saccr-rwa__LYRuJ2C/agent/live/episode.atif.jsonl",
        );
        let session = std::fs::read_to_string(&log)
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find_map(|value| {
                value
                    .pointer("/step/extensions/executor_event/session_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap();
        let now = runs::now_ms();
        let line = serde_json::json!({"record": "step", "step": {"at": now, "source": "System",
            "message": "executor event 99: command_started",
            "extensions": {"executor_event": {"session_id": session, "event": {"line": 999, "kind": "command_started", "command": "python3 /app/calc.py"}}}}});
        let mut text = std::fs::read_to_string(&log).unwrap();
        text.push_str(&format!("{line}\n"));
        std::fs::write(&log, text).unwrap();
        pane.refresh(now);
        let open = pane.open.as_ref().unwrap();
        assert_eq!(open.blocks().len(), before + 1);
        assert_eq!(open.selected, before, "the view follows the newest step");
        let screen = pane.to_text(120, 30);
        assert!(screen.contains("$ python3 /app/calc.py"), "{screen}");
        assert!(screen.contains("running"), "{screen}");
    }
}
