//! The results panel's view model, shared by every app that shows the
//! published leaderboard (the Grid's Gym on iOS and Android, and desktop).
//!
//! A [`Nav`] is where the player is in the panel: the boards list, one
//! board with a filter, one attempt, or one attempt's trace with a tab, a
//! page of steps, and a playhead. [`render`] turns it, the loaded
//! [`Leaderboard`], and an opened [`TraceBundle`] into one screen's
//! [`Page`]: the rows that screen draws, with every figure already
//! formatted and labeled. Native code draws what it returns and sends the
//! player's choices back; it never computes, sums, or relabels a number.
//!
//! The presentation rules in `docs/verse/gym-leaderboard.md` are enforced
//! here and tested below:
//!
//! 1. An unknown cost reads "unknown", with its estimated bound labeled a
//!    bound, never zero.
//! 2. Rates keep their denominators ("4 of 28").
//! 3. Every attempt row carries the board's labels and its own.
//! 4. A beat with a cost or time margin under 5% is labeled thin margin.
//! 5. Splits (each pass) are shown separately, never pooled.
//! 6. The board's headline is shown verbatim.
//! 7. The caveat count and the first caveat are always on the board
//!    screen, and a beat's row carries the in-sample and thin-margin
//!    caveats.
//! 8. The boards list keeps the publication's order and sums nothing
//!    across boards.
//! 9. A bar is shown with the reference's name and conditions.
//! 10. The first dollar figure on each screen is labeled list price.
//!
//! A page is one screen's slice, far under the 1 MiB native packet cap
//! ([`MAX_PAGE_BYTES`]); a whole leaderboard or bundle never crosses the
//! boundary.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::contract::{
    Attempt, Board, Cost, Label, Leaderboard, Miss, StepKind, Tally, TaskKnowledge, TaskRow,
    TaskStatus, Text, TraceBundle, TraceRef,
};
use crate::{THIN_MARGIN, pct, usd};

/// Trace steps shown per page.
pub const STEPS_PER_PAGE: usize = 50;

/// The bound on one serialized page. The native packet cap is 1 MiB; a
/// page stays far under it.
pub const MAX_PAGE_BYTES: usize = 256 * 1024;

/// Playback runs the trace's clock this many times faster than real time.
pub const PLAY_SPEED: f64 = 10.0;

/// The longest one-line preview of a step's text.
const PREVIEW_CHARS: usize = 160;

/// The task table's filters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    #[default]
    All,
    Beats,
    NeverPassed,
    OwnKnowledge,
    NoOwnKnowledge,
}

impl Filter {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Beats,
        Self::NeverPassed,
        Self::OwnKnowledge,
        Self::NoOwnKnowledge,
    ];

    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::All => "All tasks",
            Self::Beats => "Beats",
            Self::NeverPassed => "Never passed",
            Self::OwnKnowledge => "Own knowledge",
            Self::NoOwnKnowledge => "No own knowledge",
        }
    }

    fn admits(self, task: &TaskRow) -> bool {
        match self {
            Self::All => true,
            Self::Beats => task.beats > 0,
            Self::NeverPassed => task.status == TaskStatus::NeverPassed,
            Self::OwnKnowledge => matches!(task.knowledge, TaskKnowledge::Own { .. }),
            Self::NoOwnKnowledge => !matches!(task.knowledge, TaskKnowledge::Own { .. }),
        }
    }
}

/// The trace viewer's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tab {
    #[default]
    Jev,
    Briefing,
    Agent,
    Verifier,
}

impl Tab {
    pub const ALL: [Self; 4] = [Self::Jev, Self::Briefing, Self::Agent, Self::Verifier];

    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Jev => "Jev",
            Self::Briefing => "Briefing",
            Self::Agent => "Agent",
            Self::Verifier => "Verifier",
        }
    }
}

/// Where the player is in the panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nav {
    place: Place,
    filter: Filter,
    caveats_open: bool,
    trace: TraceNav,
}

#[derive(Clone, Debug, Default, PartialEq)]
enum Place {
    #[default]
    Boards,
    Board(String),
    Attempt(String, String),
    Trace(String, String),
}

/// The trace viewer's state. `step` indexes the trace's rows (every step
/// but usage, which only moves the token counter).
#[derive(Clone, Debug, Default, PartialEq)]
struct TraceNav {
    tab: Tab,
    page: usize,
    step: usize,
    playhead_ms: f64,
    playing: bool,
    expanded: Option<usize>,
}

impl Nav {
    /// The board on screen, or the board of the attempt or trace on screen.
    #[must_use]
    pub fn board(&self) -> Option<&str> {
        match &self.place {
            Place::Boards => None,
            Place::Board(b) | Place::Attempt(b, _) | Place::Trace(b, _) => Some(b),
        }
    }

    /// The attempt on screen, as (board, attempt).
    #[must_use]
    pub fn attempt(&self) -> Option<(&str, &str)> {
        match &self.place {
            Place::Attempt(b, a) | Place::Trace(b, a) => Some((b, a)),
            _ => None,
        }
    }

    /// The trace on screen, as (board, attempt).
    #[must_use]
    pub fn trace(&self) -> Option<(&str, &str)> {
        match &self.place {
            Place::Trace(b, a) => Some((b, a)),
            _ => None,
        }
    }

    #[must_use]
    pub fn filter(&self) -> Filter {
        self.filter
    }

    #[must_use]
    pub fn tab(&self) -> Tab {
        self.trace.tab
    }

    #[must_use]
    pub fn playing(&self) -> bool {
        self.trace.playing
    }

    /// Opens a board from the list. Its filter and caveats start closed.
    pub fn select_board(&mut self, leaderboard: &Leaderboard, id: &str) -> Result<(), String> {
        find_board(leaderboard, id)?;
        self.place = Place::Board(id.to_owned());
        self.filter = Filter::All;
        self.caveats_open = false;
        Ok(())
    }

    pub fn set_filter(&mut self, filter: Filter) -> Result<(), String> {
        match self.place {
            Place::Board(_) => {
                self.filter = filter;
                Ok(())
            }
            _ => Err("Open a board to filter its tasks".into()),
        }
    }

    pub fn set_caveats_open(&mut self, open: bool) -> Result<(), String> {
        match self.place {
            Place::Board(_) => {
                self.caveats_open = open;
                Ok(())
            }
            _ => Err("Open a board to read its caveats".into()),
        }
    }

    /// Opens an attempt on the open board.
    pub fn select_attempt(&mut self, leaderboard: &Leaderboard, id: &str) -> Result<(), String> {
        let Place::Board(board) = &self.place else {
            return Err("Open a board to choose one of its attempts".into());
        };
        let board = board.clone();
        find_attempt(find_board(leaderboard, &board)?, id)?;
        self.place = Place::Attempt(board, id.to_owned());
        Ok(())
    }

    /// Opens the trace of the attempt on screen, and returns the bundle
    /// the caller must load for it. The viewer starts on Jev at the clock's
    /// start, paused.
    pub fn open_trace(&mut self, leaderboard: &Leaderboard) -> Result<TraceRef, String> {
        let Place::Attempt(board, attempt) = &self.place else {
            return Err("Open an attempt to open its trace".into());
        };
        let found = find_attempt(find_board(leaderboard, board)?, attempt)?;
        let trace = found
            .trace
            .clone()
            .ok_or_else(|| "This attempt has no published trace".to_owned())?;
        self.place = Place::Trace(board.clone(), attempt.clone());
        self.trace = TraceNav::default();
        Ok(trace)
    }

    /// One screen back: trace to attempt, attempt to board, board to list.
    pub fn back(&mut self) {
        self.place = match std::mem::take(&mut self.place) {
            Place::Boards | Place::Board(_) => Place::Boards,
            Place::Attempt(b, _) => Place::Board(b),
            Place::Trace(b, a) => Place::Attempt(b, a),
        };
        self.trace.playing = false;
    }

    /// Back to the boards list, as a newly loaded publication requires.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_tab(&mut self, tab: Tab) -> Result<(), String> {
        self.require_trace()?;
        self.trace.tab = tab;
        Ok(())
    }

    pub fn set_page(&mut self, bundle: &TraceBundle, page: usize) -> Result<(), String> {
        self.require_trace()?;
        if page >= pages(rows(bundle).len()) {
            return Err("That page isn't in the trace".into());
        }
        self.trace.page = page;
        Ok(())
    }

    /// Moves the playhead to `fraction` of the trace's clock and pauses.
    pub fn seek(&mut self, bundle: &TraceBundle, fraction: f64) -> Result<(), String> {
        self.require_trace()?;
        if !fraction.is_finite() {
            return Err("Seek to a point on the timeline".into());
        }
        let clock = Clock::of(bundle);
        self.trace.playing = false;
        self.trace.playhead_ms = fraction.clamp(0.0, 1.0) * clock.duration_ms;
        self.trace.step = clock.step_at(self.trace.playhead_ms);
        self.follow();
        Ok(())
    }

    /// Steps the playhead to the next or previous row and pauses.
    pub fn step(&mut self, bundle: &TraceBundle, forward: bool) -> Result<(), String> {
        self.require_trace()?;
        let clock = Clock::of(bundle);
        self.trace.playing = false;
        if clock.rows.is_empty() {
            return Ok(());
        }
        self.trace.step = if forward {
            (self.trace.step + 1).min(clock.rows.len() - 1)
        } else {
            self.trace.step.saturating_sub(1)
        };
        self.trace.playhead_ms = clock.rows[self.trace.step];
        self.follow();
        Ok(())
    }

    /// Plays or pauses. Playing from the end starts over.
    pub fn set_playing(&mut self, bundle: &TraceBundle, playing: bool) -> Result<(), String> {
        self.require_trace()?;
        let clock = Clock::of(bundle);
        if playing && self.trace.playhead_ms >= clock.duration_ms {
            self.trace.playhead_ms = 0.0;
            self.trace.step = 0;
        }
        self.trace.playing = playing && !clock.rows.is_empty();
        self.follow();
        Ok(())
    }

    /// Advances playback by `dt` seconds of real time. Returns whether the
    /// page changed (the current row, or playback stopping at the end), so
    /// a host re-reads the page only then.
    pub fn tick(&mut self, bundle: &TraceBundle, dt: f64) -> bool {
        if !self.trace.playing || self.trace().is_none() || !dt.is_finite() || dt <= 0.0 {
            return false;
        }
        let clock = Clock::of(bundle);
        self.trace.playhead_ms =
            (self.trace.playhead_ms + dt * 1000.0 * PLAY_SPEED).min(clock.duration_ms);
        let step = clock.step_at(self.trace.playhead_ms);
        let ended = self.trace.playhead_ms >= clock.duration_ms;
        let changed = step != self.trace.step || ended;
        self.trace.step = step;
        if ended {
            self.trace.playing = false;
        }
        if changed {
            self.follow();
        }
        changed
    }

    /// Expands one row's output, or collapses it with `None`. Only one
    /// output is expanded at a time, so a page stays small.
    pub fn expand(&mut self, bundle: &TraceBundle, row: Option<usize>) -> Result<(), String> {
        self.require_trace()?;
        if row.is_some_and(|r| r >= rows(bundle).len()) {
            return Err("That step isn't in the trace".into());
        }
        self.trace.expanded = row;
        Ok(())
    }

    fn require_trace(&self) -> Result<(), String> {
        self.trace()
            .map(|_| ())
            .ok_or_else(|| "Open a trace first".to_owned())
    }

    /// Keeps the current row's page on screen.
    fn follow(&mut self) {
        self.trace.page = self.trace.step / STEPS_PER_PAGE;
    }
}

fn find_board<'a>(leaderboard: &'a Leaderboard, id: &str) -> Result<&'a Board, String> {
    leaderboard
        .boards
        .iter()
        .find(|b| b.id == id)
        .ok_or_else(|| "That board isn't in this publication".to_owned())
}

fn find_attempt<'a>(board: &'a Board, id: &str) -> Result<&'a Attempt, String> {
    board
        .attempts
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| "That attempt isn't on this board".to_owned())
}

/// Where the publication on screen came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// The leaderboard's content digest.
    pub digest: String,
    pub commit: Option<String>,
    pub freshness: Freshness,
    /// Seconds since the publication was fetched, for a cached copy.
    pub age_seconds: Option<u64>,
}

/// Whether the publication on screen is the index's latest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Fetched and verified against the index on this visit.
    Current,
    /// A verified copy from an earlier visit, shown while the index is
    /// checked.
    Cached,
    /// A verified copy from an earlier visit; the index couldn't be read.
    Offline,
}

impl Source {
    /// The boards list's footer: the digest's first 8 characters, the
    /// commit, and whether the copy is current, cached, or offline.
    #[must_use]
    pub fn footer(&self) -> String {
        let mut text = format!(
            "Publication {}",
            self.digest.get(..8).unwrap_or(&self.digest)
        );
        if let Some(commit) = &self.commit {
            let _ = write!(text, " · commit {}", commit.get(..7).unwrap_or(commit));
        }
        let age = self.age_seconds.map(age_text);
        match (self.freshness, age) {
            (Freshness::Current, _) => text.push_str(" · current"),
            (Freshness::Cached, Some(age)) => {
                let _ = write!(text, " · cached {age} ago, checking for a newer one");
            }
            (Freshness::Cached, None) => text.push_str(" · cached, checking for a newer one"),
            (Freshness::Offline, Some(age)) => {
                let _ = write!(text, " · offline, cached {age} ago");
            }
            (Freshness::Offline, None) => text.push_str(" · offline, cached"),
        }
        text
    }
}

fn age_text(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds} s"),
        60..3600 => format!("{} min", seconds / 60),
        3600..86_400 => format!("{} h", seconds / 3600),
        _ => format!("{} d", seconds / 86_400),
    }
}

/// A label as a chip: its code, for styling, and the words shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Chip {
    pub code: Label,
    pub text: &'static str,
}

fn chips(labels: impl IntoIterator<Item = Label>) -> Vec<Chip> {
    let mut labels: Vec<Label> = labels.into_iter().collect();
    labels.sort_unstable();
    labels.dedup();
    labels
        .into_iter()
        .map(|code| Chip {
            code,
            text: code.text(),
        })
        .collect()
}

fn chip_text(chips: &[Chip]) -> String {
    chips.iter().map(|c| c.text).collect::<Vec<_>>().join(", ")
}

/// A caveat as shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CaveatRow {
    pub code: String,
    pub text: String,
}

/// One screen of the results panel.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "screen", rename_all = "snake_case")]
pub enum Page {
    Boards(BoardsPage),
    Board(Box<BoardPage>),
    Attempt(Box<AttemptPage>),
    Trace(Box<TracePage>),
}

/// Screen 1: every board, in the publication's order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BoardsPage {
    pub rows: Vec<BoardsRow>,
    /// The publication's digest, commit, and freshness.
    pub footer: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BoardsRow {
    pub id: String,
    pub title: String,
    pub benchmark: String,
    /// The board's own headline, verbatim.
    pub headline: String,
    /// Shown under the headline when it carries a dollar figure, so the
    /// first dollar figure on the screen is labeled list price.
    pub headline_note: Option<&'static str>,
    pub labels: Vec<Chip>,
    pub accessibility: String,
}

/// Screen 2: one board.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BoardPage {
    pub id: String,
    pub title: String,
    pub benchmark: String,
    pub question: String,
    /// The board's own headline, verbatim.
    pub headline: String,
    pub headline_note: Option<&'static str>,
    pub labels: Vec<Chip>,
    /// Every attempt first, then each split on its own.
    pub tallies: Vec<TallyRow>,
    pub spend: Vec<String>,
    pub reference: ReferenceView,
    pub caveat_count: usize,
    /// The first caveat always; all of them when open.
    pub caveats: Vec<CaveatRow>,
    pub caveats_open: bool,
    pub filter: Filter,
    pub filters: Vec<FilterChip>,
    pub tasks: Vec<TaskView>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TallyRow {
    pub name: String,
    /// "13 of 28", with the whole denominator.
    pub passed: String,
    pub beat: String,
    pub cost_unknown: String,
    pub faults: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ReferenceView {
    pub name: String,
    pub rule: String,
    pub conditions: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FilterChip {
    pub filter: Filter,
    pub text: &'static str,
    /// Tasks it shows.
    pub count: usize,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TaskView {
    pub task: String,
    /// The bar, with the reference that set it.
    pub bar: String,
    pub knowledge: &'static str,
    pub status: &'static str,
    pub attempts: Vec<AttemptCell>,
    pub accessibility: String,
}

/// An attempt as a compact cell in the task table.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AttemptCell {
    pub id: String,
    pub series: String,
    pub passed: bool,
    pub beat: bool,
    /// "pass · beat · $2.0976 · 420 s".
    pub text: String,
    /// The board's labels and the attempt's own.
    pub labels: Vec<Chip>,
    /// On a beat: the board's in-sample and thin-margin caveats that apply.
    pub caveats: Vec<CaveatRow>,
    pub accessibility: String,
}

/// Screen 3: one attempt.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AttemptPage {
    pub id: String,
    pub board_title: String,
    pub header: Header,
    pub series: String,
    pub trial: String,
    /// Reward, cost, and time against the bar, with ratios.
    pub numbers: Vec<String>,
    /// Why it isn't a beat, in words, or that it is one.
    pub misses: String,
    pub reference: ReferenceView,
    pub phases: Vec<String>,
    pub how_it_ended: Option<String>,
    pub jev: Option<String>,
    pub verifier: Option<String>,
    pub failed_tests: Vec<String>,
    pub caveats: Vec<CaveatRow>,
    /// "Open trace" when a bundle exists, with its size.
    pub trace: Option<String>,
    pub accessibility: String,
}

/// What every attempt and trace screen shows at the top.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Header {
    pub task: String,
    pub result: &'static str,
    pub beat: &'static str,
    pub cost: String,
    pub time: String,
    pub labels: Vec<Chip>,
    pub accessibility: String,
}

/// Screen 4: one attempt's trace.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TracePage {
    pub attempt: String,
    pub header: Header,
    pub clock: ClockView,
    pub tab: Tab,
    pub tabs: Vec<TabChip>,
    pub jev: Option<JevTab>,
    pub briefing: Option<TextView>,
    pub agent: Option<AgentTab>,
    pub verifier: Option<VerifierTab>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TabChip {
    pub tab: Tab,
    pub text: &'static str,
    pub selected: bool,
    pub available: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClockView {
    pub duration_ms: u64,
    pub playhead_ms: u64,
    /// The playhead as a fraction of the clock, for the scrubber.
    pub fraction: f64,
    pub step: usize,
    pub steps: usize,
    pub playing: bool,
    /// "1:07 / 7:00".
    pub text: String,
    /// The trace's clock of each row, as fractions, for scrubber ticks.
    pub marks: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TextView {
    pub text: String,
    /// "cut from N bytes" when the text was bounded.
    pub cut: Option<String>,
}

impl From<&Text> for TextView {
    fn from(value: &Text) -> Self {
        Self {
            text: value.text.clone(),
            cut: cut_text(value),
        }
    }
}

fn cut_text(value: &Text) -> Option<String> {
    value.original_bytes.map(|n| format!("cut from {n} bytes"))
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct JevTab {
    pub summary: String,
    pub question_set: String,
    pub questions: Vec<String>,
    pub keep_threshold: f64,
    pub flag_threshold: f64,
    pub candidates: Vec<CandidateRow>,
    pub requirements: Vec<RequirementRow>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CandidateRow {
    pub rank: u32,
    pub id: String,
    pub title: Option<String>,
    /// Jev's probability, 0 to 1, drawn as a bar against the threshold.
    pub p: f64,
    pub kept: bool,
    /// Written from earlier runs on this task.
    pub own: bool,
    pub fate: String,
    pub accessibility: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RequirementRow {
    pub text: String,
    pub p: f64,
    pub flagged: bool,
    pub accessibility: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentTab {
    pub page: usize,
    pub pages: usize,
    /// The running token counter at the playhead.
    pub tokens: String,
    pub rows: Vec<StepRow>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StepRow {
    /// The row's index in the trace, for expanding and seeking.
    pub index: usize,
    /// "0:07" on the trace's clock.
    pub at: String,
    pub kind: &'static str,
    /// The step's text, or its first line for a command's output.
    pub text: String,
    pub exit_code: Option<i64>,
    /// A command result's output is shown only when expanded.
    pub expandable: bool,
    pub output: Option<TextView>,
    pub cut: Option<String>,
    /// At or before the playhead.
    pub reached: bool,
    pub current: bool,
    pub accessibility: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VerifierTab {
    pub summary: String,
    pub tests: Vec<TestRow>,
    pub output_tail: TextView,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TestRow {
    pub name: String,
    pub status: String,
    pub passed: bool,
}

/// Formats dollars, labeling the first figure on a screen list price.
struct Money {
    labeled: bool,
}

impl Money {
    fn new() -> Self {
        Self { labeled: false }
    }

    /// A screen whose headline already showed a dollar figure, with the
    /// list-price note under it.
    fn after_note(note: Option<&'static str>) -> Self {
        Self {
            labeled: note.is_some(),
        }
    }

    fn usd(&mut self, amount: f64) -> String {
        let text = usd(amount);
        if self.labeled {
            text
        } else {
            self.labeled = true;
            format!("{text} (list price)")
        }
    }

    /// An attempt's cost. Unknown is "unknown", with any estimated bound
    /// labeled a bound; it is never zero.
    fn cost(&mut self, cost: &Cost) -> String {
        match *cost {
            Cost::Reported { usd } => self.usd(usd),
            Cost::Unknown {
                lower_bound_usd,
                upper_bound_usd,
            } => match (lower_bound_usd, upper_bound_usd) {
                (Some(low), _) => format!("unknown (bound: at least {})", self.usd(low)),
                (None, Some(high)) => format!("unknown (bound: at most {})", self.usd(high)),
                (None, None) => "unknown".into(),
            },
        }
    }
}

const LIST_PRICE_NOTE: &str = "Dollar figures are list price, not a bill.";

fn headline_note(headline: &str) -> Option<&'static str> {
    headline.contains('$').then_some(LIST_PRICE_NOTE)
}

fn seconds(value: f64) -> String {
    format!("{value:.0} s")
}

fn of(part: u32, whole: u32) -> String {
    format!("{part} of {whole}")
}

fn benchmark(board: &Board) -> String {
    format!("{} {}", board.benchmark.name, board.benchmark.version)
}

fn reference(board: &Board) -> ReferenceView {
    ReferenceView {
        name: board.reference.name.clone(),
        rule: board.reference.rule.clone(),
        conditions: board.reference.conditions.clone(),
    }
}

/// Renders the screen `nav` is on. `bundle` is the opened trace's bundle,
/// once loaded; without it the trace screen isn't ready yet.
pub fn render(
    nav: &Nav,
    leaderboard: &Leaderboard,
    source: Option<&Source>,
    bundle: Option<&TraceBundle>,
) -> Result<Page, String> {
    Ok(match &nav.place {
        Place::Boards => Page::Boards(boards_page(leaderboard, source)),
        Place::Board(id) => Page::Board(Box::new(board_page(
            find_board(leaderboard, id)?,
            nav.filter,
            nav.caveats_open,
        ))),
        Place::Attempt(b, a) => {
            let board = find_board(leaderboard, b)?;
            Page::Attempt(Box::new(attempt_page(board, find_attempt(board, a)?)))
        }
        Place::Trace(b, a) => {
            let board = find_board(leaderboard, b)?;
            let attempt = find_attempt(board, a)?;
            let bundle = bundle
                .filter(|t| t.board == board.id && t.attempt == attempt.id)
                .ok_or_else(|| "The trace is loading".to_owned())?;
            Page::Trace(Box::new(trace_page(board, attempt, bundle, &nav.trace)))
        }
    })
}

fn boards_page(leaderboard: &Leaderboard, source: Option<&Source>) -> BoardsPage {
    let mut money = Money::new();
    let rows = leaderboard
        .boards
        .iter()
        .map(|board| {
            let labels = chips(board.labels.iter().copied());
            let note = headline_note(&board.headline);
            if note.is_some() {
                money.labeled = true;
            }
            BoardsRow {
                accessibility: format!(
                    "{}. {}. {}. Labels: {}.",
                    board.title,
                    benchmark(board),
                    board.headline,
                    chip_text(&labels)
                ),
                id: board.id.clone(),
                title: board.title.clone(),
                benchmark: benchmark(board),
                headline: board.headline.clone(),
                headline_note: note,
                labels,
            }
        })
        .collect();
    BoardsPage {
        rows,
        footer: source.map(Source::footer),
    }
}

fn tally_row(name: &str, tally: &Tally) -> TallyRow {
    let passed = of(tally.passes, tally.attempts);
    let beat = of(tally.beats, tally.attempts);
    let cost_unknown = of(tally.cost_unknown, tally.attempts);
    let faults = format!(
        "{} fault{} (not counted as attempts)",
        tally.faults,
        if tally.faults == 1 { "" } else { "s" }
    );
    TallyRow {
        text: format!(
            "{name}: passed {passed}, beat {beat}, cost unknown on {cost_unknown}, {faults}"
        ),
        name: name.to_owned(),
        passed,
        beat,
        cost_unknown,
        faults,
    }
}

fn board_page(board: &Board, filter: Filter, caveats_open: bool) -> BoardPage {
    let note = headline_note(&board.headline);
    let mut money = Money::after_note(note);
    let mut tallies = vec![tally_row("All attempts", &board.totals)];
    tallies.extend(board.splits.iter().map(|s| tally_row(&s.name, &s.tally)));
    let mut spend = vec![format!("Reported: {}", money.usd(board.spend.reported_usd))];
    if let Some(low) = board.spend.estimated_lower_bound_usd {
        spend.push(format!(
            "Unknown costs, estimated lower bound: {} (a bound, not a report)",
            money.usd(low)
        ));
    }
    if let Some(high) = board.spend.estimated_upper_bound_usd {
        spend.push(format!(
            "With unknown costs, estimated upper bound: {} (a bound, not a report)",
            money.usd(high)
        ));
    }
    let caveats: Vec<CaveatRow> = board
        .caveats
        .iter()
        .take(if caveats_open { usize::MAX } else { 1 })
        .map(|c| CaveatRow {
            code: c.code.clone(),
            text: c.text.clone(),
        })
        .collect();
    let filters = Filter::ALL
        .iter()
        .map(|&f| FilterChip {
            filter: f,
            text: f.text(),
            count: board.tasks.iter().filter(|t| f.admits(t)).count(),
            selected: f == filter,
        })
        .collect();
    let tasks = board
        .tasks
        .iter()
        .filter(|t| filter.admits(t))
        .map(|t| task_view(board, t, &mut money))
        .collect();
    BoardPage {
        id: board.id.clone(),
        title: board.title.clone(),
        benchmark: benchmark(board),
        question: board.question.clone(),
        headline: board.headline.clone(),
        headline_note: note,
        labels: chips(board.labels.iter().copied()),
        tallies,
        spend,
        reference: reference(board),
        caveat_count: board.caveats.len(),
        caveats,
        caveats_open,
        filter,
        filters,
        tasks,
    }
}

fn bar_text(board: &Board, bar: &crate::contract::Bar, money: &mut Money) -> String {
    let mut parts = Vec::new();
    if let Some(cost) = bar.cost_usd {
        parts.push(money.usd(cost));
    }
    if let Some(s) = bar.seconds {
        parts.push(seconds(s));
    }
    if parts.is_empty() {
        parts.push("no bar".into());
    }
    let mut text = format!("{}'s bar: {}", board.reference.name, parts.join(", "));
    if let (Some(p), Some(n)) = (bar.reference_passes, bar.reference_trials) {
        let _ = write!(text, " ({} passed {})", board.reference.name, of(p, n));
    }
    text
}

fn knowledge_text(knowledge: TaskKnowledge) -> &'static str {
    match knowledge {
        TaskKnowledge::Own { .. } => "own knowledge",
        TaskKnowledge::OtherTasksOnly => "other tasks' knowledge only",
        TaskKnowledge::Off => "knowledge off",
    }
}

fn status_text(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Beat => "beat",
        TaskStatus::Confirmed => "confirmed",
        TaskStatus::NotConfirmed => "not confirmed",
        TaskStatus::PassedWithoutBeat => "passed without a beat",
        TaskStatus::NeverPassed => "never passed",
    }
}

fn task_view(board: &Board, task: &TaskRow, money: &mut Money) -> TaskView {
    let bar = bar_text(board, &task.bar, money);
    let attempts: Vec<AttemptCell> = task
        .attempts
        .iter()
        .filter_map(|id| board.attempts.iter().find(|a| &a.id == id))
        .map(|a| attempt_cell(board, a, money))
        .collect();
    let status = status_text(task.status);
    let knowledge = knowledge_text(task.knowledge);
    TaskView {
        accessibility: format!(
            "{}. {status}. Passed {}, beat {}. {bar}. {knowledge}.",
            task.task,
            of(task.passes, attempts.len() as u32),
            of(task.beats, attempts.len() as u32),
        ),
        task: task.task.clone(),
        bar,
        knowledge,
        status,
        attempts,
    }
}

/// The margin by which a beat came in under its bar: the smaller of its
/// cost and time margins, as a fraction.
fn margin(attempt: &Attempt) -> Option<f64> {
    [attempt.cost_ratio, attempt.time_ratio]
        .into_iter()
        .flatten()
        .map(|ratio| 1.0 - ratio)
        .min_by(f64::total_cmp)
}

fn thin(attempt: &Attempt) -> bool {
    attempt.beat && margin(attempt).is_some_and(|m| m < THIN_MARGIN)
}

/// The board's labels and the attempt's own, with thin margin on a beat
/// that came in under its bar by less than 5%.
fn attempt_labels(board: &Board, attempt: &Attempt) -> Vec<Chip> {
    chips(
        board
            .labels
            .iter()
            .chain(&attempt.labels)
            .copied()
            .chain(thin(attempt).then_some(Label::ThinMargin)),
    )
}

/// The board's caveats a beat's row must carry: in-sample when it kept its
/// own knowledge, and thin margin when its margin is thin.
fn beat_caveats(board: &Board, attempt: &Attempt) -> Vec<CaveatRow> {
    if !attempt.beat {
        return Vec::new();
    }
    let in_sample = attempt.labels.contains(&Label::InSample);
    board
        .caveats
        .iter()
        .filter(|c| {
            (c.code == "in_sample" && in_sample) || (c.code == "thin_margin" && thin(attempt))
        })
        .map(|c| CaveatRow {
            code: c.code.clone(),
            text: c.text.clone(),
        })
        .collect()
}

fn attempt_cell(board: &Board, attempt: &Attempt, money: &mut Money) -> AttemptCell {
    let labels = attempt_labels(board, attempt);
    let mut parts = vec![
        if attempt.passed { "pass" } else { "fail" }.to_owned(),
        if attempt.beat { "beat" } else { "no beat" }.to_owned(),
        money.cost(&attempt.cost),
    ];
    parts.push(
        attempt
            .seconds
            .map_or_else(|| "time unknown".into(), seconds),
    );
    let text = parts.join(" · ");
    AttemptCell {
        accessibility: format!(
            "{}, {}: {text}. Labels: {}.",
            attempt.task,
            attempt.series,
            chip_text(&labels)
        ),
        id: attempt.id.clone(),
        series: attempt.series.clone(),
        passed: attempt.passed,
        beat: attempt.beat,
        text,
        caveats: beat_caveats(board, attempt),
        labels,
    }
}

fn header(board: &Board, attempt: &Attempt, bar: &crate::contract::Bar) -> Header {
    let mut money = Money::new();
    let cost = money.cost(&attempt.cost);
    let time = match (attempt.seconds, bar.seconds) {
        (Some(s), Some(b)) => format!(
            "{} against {}'s {}",
            seconds(s),
            board.reference.name,
            seconds(b)
        ),
        (Some(s), None) => format!("{} (time isn't part of the bar)", seconds(s)),
        (None, _) => "time unknown".into(),
    };
    let labels = attempt_labels(board, attempt);
    let result = if attempt.passed { "passed" } else { "failed" };
    let beat = if attempt.beat {
        "beat the bar"
    } else {
        "no beat"
    };
    Header {
        accessibility: format!(
            "{}: {result}, {beat}. Cost {cost}. Time {time}. Labels: {}.",
            attempt.task,
            chip_text(&labels)
        ),
        task: attempt.task.clone(),
        result,
        beat,
        cost,
        time,
        labels,
    }
}

fn miss_text(miss: Miss, attempt: &Attempt) -> String {
    match miss {
        Miss::Failed => format!(
            "reward {}",
            attempt
                .reward
                .map_or_else(|| "unknown".into(), |r| format!("{r}"))
        ),
        Miss::CostUnknown => "cost unknown".into(),
        Miss::Cost => "cost".into(),
        Miss::Time => "time".into(),
    }
}

fn task_bar<'a>(board: &'a Board, attempt: &Attempt) -> Option<&'a crate::contract::Bar> {
    board
        .tasks
        .iter()
        .find(|t| t.task == attempt.task)
        .map(|t| &t.bar)
}

fn attempt_page(board: &Board, attempt: &Attempt) -> AttemptPage {
    let empty = crate::contract::Bar {
        cost_usd: None,
        seconds: None,
        cost_trial: None,
        time_trial: None,
        deadline_seconds: None,
        reference_passes: None,
        reference_trials: None,
    };
    let bar = task_bar(board, attempt).unwrap_or(&empty);
    let header = header(board, attempt, bar);
    // The header labeled the first figure.
    let mut money = Money { labeled: true };
    let mut numbers = vec![format!(
        "Reward {}",
        attempt
            .reward
            .map_or_else(|| "unknown".into(), |r| format!("{r}"))
    )];
    let cost = money.cost(&attempt.cost);
    numbers.push(match (attempt.cost_ratio, bar.cost_usd) {
        (Some(ratio), Some(b)) => format!(
            "Cost {cost}: {} of {}'s bar {}{}",
            pct(ratio),
            board.reference.name,
            money.usd(b),
            if attempt.cost_ratio_is_bound {
                " (comparing the bound)"
            } else {
                ""
            }
        ),
        (_, Some(b)) => format!(
            "Cost {cost}; {}'s bar {}",
            board.reference.name,
            money.usd(b)
        ),
        _ => format!("Cost {cost}"),
    });
    numbers.push(match (attempt.seconds, attempt.time_ratio, bar.seconds) {
        (Some(s), Some(ratio), Some(b)) => format!(
            "Time {}: {} of {}'s bar {}",
            seconds(s),
            pct(ratio),
            board.reference.name,
            seconds(b)
        ),
        (Some(s), _, _) => format!("Time {} (not part of the bar)", seconds(s)),
        (None, _, _) => "Time unknown".into(),
    });
    if let Some(deadline) = bar.deadline_seconds {
        numbers.push(format!("Deadline {deadline} s"));
    }
    let misses = if attempt.beat {
        "Beat the bar".into()
    } else if attempt.misses.is_empty() {
        "Not a beat".into()
    } else {
        format!(
            "Not a beat: {}",
            attempt
                .misses
                .iter()
                .map(|&m| miss_text(m, attempt))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let phases = attempt
        .phases
        .map(|p| {
            [
                ("Environment setup", p.environment_setup),
                ("Agent setup", p.agent_setup),
                ("Agent", p.agent_execution),
                ("Verifier", p.verifier),
            ]
            .into_iter()
            .map(|(name, s)| {
                format!(
                    "{name}: {}",
                    s.map_or_else(|| "unknown".into(), |s| format!("{s:.1} s"))
                )
            })
            .collect()
        })
        .unwrap_or_default();
    let jev = attempt.jev.as_ref().map(|j| {
        format!(
            "Jev ({}, {}) kept {} candidates{}, flagged {} requirements",
            j.question_set,
            j.outcome,
            of(j.kept, j.candidates),
            if j.kept_own > 0 {
                format!(" ({} written from this task)", j.kept_own)
            } else {
                String::new()
            },
            of(j.flagged, j.requirements),
        )
    });
    let verifier = attempt.verifier.as_ref().and_then(|v| v.summary.clone());
    let failed_tests = attempt
        .verifier
        .as_ref()
        .map(|v| v.failed_tests.clone())
        .unwrap_or_default();
    let trace = attempt
        .trace
        .as_ref()
        .map(|t| format!("Open trace ({} KB)", t.bytes.div_ceil(1024)));
    AttemptPage {
        accessibility: format!(
            "{} {}. {}.",
            header.accessibility,
            numbers.join(". "),
            misses
        ),
        id: attempt.id.clone(),
        board_title: board.title.clone(),
        series: attempt.series.clone(),
        trial: attempt.trial.clone(),
        numbers,
        misses,
        reference: reference(board),
        phases,
        how_it_ended: attempt.how_it_ended.clone(),
        jev,
        verifier,
        failed_tests,
        caveats: beat_caveats(board, attempt),
        trace,
        header,
    }
}

/// The trace's rows (every step but usage) on its clock.
struct Clock {
    /// Each row's time, in milliseconds; a step without a time takes the
    /// time before it.
    rows: Vec<f64>,
    duration_ms: f64,
}

impl Clock {
    fn of(bundle: &TraceBundle) -> Self {
        let mut at = 0.0_f64;
        let mut rows = Vec::new();
        let mut duration_ms = 0.0_f64;
        for step in &bundle.steps {
            if let Some(ms) = step.at_ms {
                at = at.max(ms as f64);
            }
            duration_ms = duration_ms.max(at);
            if !matches!(step.kind, StepKind::Usage { .. }) {
                rows.push(at);
            }
        }
        Self { rows, duration_ms }
    }

    /// The last row at or before `ms`.
    fn step_at(&self, ms: f64) -> usize {
        self.rows.partition_point(|&at| at <= ms).saturating_sub(1)
    }
}

/// The indices in `bundle.steps` of the trace's rows.
fn rows(bundle: &TraceBundle) -> Vec<usize> {
    bundle
        .steps
        .iter()
        .enumerate()
        .filter(|(_, s)| !matches!(s.kind, StepKind::Usage { .. }))
        .map(|(i, _)| i)
        .collect()
}

fn pages(rows: usize) -> usize {
    rows.div_ceil(STEPS_PER_PAGE).max(1)
}

fn clock_text(ms: f64) -> String {
    let s = (ms / 1000.0).floor() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn preview(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut out: String = line.chars().take(PREVIEW_CHARS).collect();
    if line.chars().count() > PREVIEW_CHARS
        || text.lines().filter(|l| !l.trim().is_empty()).count() > 1
    {
        out.push('…');
    }
    out
}

fn trace_page(board: &Board, attempt: &Attempt, bundle: &TraceBundle, nav: &TraceNav) -> TracePage {
    let bar = &bundle.outcome.bar;
    let header = header(board, attempt, bar);
    let clock = Clock::of(bundle);
    let step = nav.step.min(clock.rows.len().saturating_sub(1));
    let duration = clock.duration_ms.max(1.0);
    let clock_view = ClockView {
        duration_ms: clock.duration_ms as u64,
        playhead_ms: nav.playhead_ms as u64,
        fraction: (nav.playhead_ms / duration).clamp(0.0, 1.0),
        step,
        steps: clock.rows.len(),
        playing: nav.playing,
        text: format!(
            "{} / {}",
            clock_text(nav.playhead_ms),
            clock_text(clock.duration_ms)
        ),
        marks: clock
            .rows
            .iter()
            .map(|&at| (at / duration) as f32)
            .collect(),
    };
    let available = |tab: Tab| match tab {
        Tab::Jev => bundle.jev.is_some(),
        Tab::Briefing => bundle.briefing.is_some(),
        Tab::Agent => true,
        Tab::Verifier => bundle.verifier.is_some(),
    };
    let tabs = Tab::ALL
        .iter()
        .map(|&t| TabChip {
            tab: t,
            text: t.text(),
            selected: t == nav.tab,
            available: available(t),
        })
        .collect();
    let mut page = TracePage {
        attempt: attempt.id.clone(),
        header,
        clock: clock_view,
        tab: nav.tab,
        tabs,
        jev: None,
        briefing: None,
        agent: None,
        verifier: None,
    };
    match nav.tab {
        Tab::Jev => page.jev = bundle.jev.as_ref().map(jev_tab),
        Tab::Briefing => page.briefing = bundle.briefing.as_ref().map(TextView::from),
        Tab::Agent => page.agent = Some(agent_tab(bundle, nav, step)),
        Tab::Verifier => {
            page.verifier = bundle.verifier.as_ref().map(|v| VerifierTab {
                summary: format!(
                    "Reward {}: {} passed, {} failed",
                    v.reward
                        .map_or_else(|| "unknown".into(), |r| format!("{r}")),
                    v.passed,
                    v.failed
                ),
                tests: v
                    .tests
                    .iter()
                    .map(|t| TestRow {
                        name: t.name.clone(),
                        status: t.status.clone(),
                        passed: t.status == "passed",
                    })
                    .collect(),
                output_tail: TextView::from(&v.output_tail),
            });
        }
    }
    page
}

fn jev_tab(jev: &crate::contract::JevDecision) -> JevTab {
    let kept = jev.candidates.iter().filter(|c| c.kept).count();
    let own = jev
        .candidates
        .iter()
        .filter(|c| c.kept && c.written_from_this_task == Some(true))
        .count();
    let flagged = jev.requirements.iter().filter(|r| r.flagged).count();
    JevTab {
        summary: format!(
            "Kept {kept} of {} candidates ({own} written from this task) at p ≥ {}; flagged {flagged} of {} requirements at p ≥ {}",
            jev.candidates.len(),
            jev.keep_threshold,
            jev.requirements.len(),
            jev.flag_threshold
        ),
        question_set: jev.question_set.clone(),
        questions: jev.questions.clone(),
        keep_threshold: jev.keep_threshold,
        flag_threshold: jev.flag_threshold,
        candidates: jev
            .candidates
            .iter()
            .map(|c| {
                let own = c.written_from_this_task == Some(true);
                CandidateRow {
                    accessibility: format!(
                        "Rank {}: {}. Probability {:.2}, {} (threshold {}){}.",
                        c.rank,
                        c.title.as_deref().unwrap_or(&c.id),
                        c.p,
                        if c.kept { "kept" } else { "not kept" },
                        jev.keep_threshold,
                        if own { ", written from this task" } else { "" }
                    ),
                    rank: c.rank,
                    id: c.id.clone(),
                    title: c.title.clone(),
                    p: c.p,
                    kept: c.kept,
                    own,
                    fate: c.fate.clone(),
                }
            })
            .collect(),
        requirements: jev
            .requirements
            .iter()
            .map(|r| RequirementRow {
                accessibility: format!(
                    "{}. Probability {:.2}, {} (threshold {}).",
                    r.text,
                    r.p,
                    if r.flagged { "flagged" } else { "not flagged" },
                    jev.flag_threshold
                ),
                text: r.text.clone(),
                p: r.p,
                flagged: r.flagged,
            })
            .collect(),
    }
}

fn agent_tab(bundle: &TraceBundle, nav: &TraceNav, step: usize) -> AgentTab {
    let rows = rows(bundle);
    let pages = pages(rows.len());
    let page = nav.page.min(pages - 1);
    let clock = Clock::of(bundle);
    // The token counter at the playhead: the last usage at or before it.
    let tokens = bundle
        .steps
        .iter()
        .filter(|s| s.at_ms.is_none_or(|at| at as f64 <= nav.playhead_ms))
        .filter_map(|s| match s.kind {
            StepKind::Usage {
                input_tokens,
                cache_write_tokens,
                cache_read_tokens,
                output_tokens,
            } => Some((
                input_tokens,
                cache_write_tokens,
                cache_read_tokens,
                output_tokens,
            )),
            _ => None,
        })
        .next_back()
        .map_or_else(
            || "No tokens yet".to_owned(),
            |(i, w, r, o)| format!("{i} in · {w} cache write · {r} cache read · {o} out"),
        );
    let rows = rows
        .iter()
        .enumerate()
        .skip(page * STEPS_PER_PAGE)
        .take(STEPS_PER_PAGE)
        .map(|(index, &at)| {
            let s = &bundle.steps[at];
            let at_ms = clock.rows[index];
            let (kind, text, exit_code, output, cut) = match &s.kind {
                StepKind::Host { text } => ("host", text.text.clone(), None, None, cut_text(text)),
                StepKind::Decision { name, duration_ms } => (
                    "decision",
                    match duration_ms {
                        Some(ms) => format!("{name} decided in {ms} ms"),
                        None => name.clone(),
                    },
                    None,
                    None,
                    None,
                ),
                StepKind::DelegateStarted { agent, model } => (
                    "delegate_started",
                    match model {
                        Some(m) => format!("{agent} started on {m}"),
                        None => format!("{agent} started"),
                    },
                    None,
                    None,
                    None,
                ),
                StepKind::Say { text } => ("say", text.text.clone(), None, None, cut_text(text)),
                StepKind::Command { command } => (
                    "command",
                    command.text.clone(),
                    None,
                    None,
                    cut_text(command),
                ),
                StepKind::CommandResult { exit_code, output } => (
                    "command_result",
                    preview(&output.text),
                    *exit_code,
                    Some(output),
                    cut_text(output),
                ),
                StepKind::DelegateEnded { error, result } => (
                    "delegate_ended",
                    format!(
                        "{}{}",
                        if *error {
                            "Ended with an error: "
                        } else {
                            "Ended: "
                        },
                        result.text
                    ),
                    None,
                    None,
                    cut_text(result),
                ),
                StepKind::Usage { .. } => unreachable!("usage steps aren't rows"),
            };
            let expanded = nav.expanded == Some(index);
            StepRow {
                accessibility: format!(
                    "{} at {}: {}{}",
                    kind.replace('_', " "),
                    clock_text(at_ms),
                    exit_code.map_or_else(String::new, |c| format!("exit code {c}. ")),
                    text
                ),
                index,
                at: clock_text(at_ms),
                kind,
                text,
                exit_code,
                expandable: output.is_some(),
                output: output.filter(|_| expanded).map(TextView::from),
                cut,
                reached: index <= step && at_ms <= nav.playhead_ms,
                current: index == step,
            }
        })
        .collect();
    AgentTab {
        page,
        pages,
        tokens,
        rows,
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
