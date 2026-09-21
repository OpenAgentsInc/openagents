//! A program run's progress: the runstate's own marks, rendered.
//!
//! A program run crosses process boundaries — steps dispatch, tasks
//! delegate into worktrees of their own, and the host can die anywhere in
//! between. What survives is the record: the mark the host left on each
//! step, the state it left on the run, and the times it measured.
//! [`RunView`] is the caller's half of that record, with two
//! presentations: [`collapsed`][RunView::collapsed] is the one line a
//! rail or a board can carry while the run is in flight, and
//! [`expanded`][RunView::expanded] is the per-step detail behind it.
//! [`BoardView`] lists runs under one ordering, so a supervised fan-out
//! reads as a board rather than a burst of lines.
//!
//! Progress is a claim the runstate makes, and three rules keep the view
//! honest about whose claim it is:
//!
//! - **The view renders marks; it does not infer them.** Every field is
//!   supplied: the caller maps runstate and trace data into plain values,
//!   and a step the host never marked shows `unknown`, not a guess at
//!   where it got to.
//! - **Every mark keeps its own word.** `cancelled` is the end someone
//!   asked for, `refused` is the work's decline, `running` is a step
//!   still in flight and never reads as `done`, and `unknown` is
//!   recovery's mark — six marks, six words, never one smudge.
//! - **A gap says `unknown`; a simulation says `simulated`.** A duration
//!   nobody recorded and a cost nobody metered render as `unknown`
//!   rather than passing for a zero, and [`Origin`] rides both
//!   presentations so a demonstrated view can never pass for metered
//!   work.
//!
//! Like [`crate::decision`], the view is pure: it reads nothing, writes
//! nothing, and holds no style. Elapsed and durations are what the
//! caller recorded — the module has no clock to measure against. A
//! [`Line`] is spans of text and the [`Intensity`] step each draws at;
//! which amber a step burns is the caller's [`Ladder`][crate::Ladder]'s
//! to say.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::decision::{Line, Origin, Span};
use crate::{Intensity, wrap_rows};

/// The mark a cut leaves, so a narrowed view shows that it narrowed.
const ELLIPSIS: &str = "...";

/// The cells a field's name takes in the expanded view.
const LABEL: usize = 13;

/// Below this width an expanded field stacks its name over its value
/// rather than sharing a line neither fits on.
const STACKED: usize = LABEL + 4;

/// The widest column a step's name takes in the expanded view, so one
/// long name cannot push every mark off the row.
const NAME: usize = 20;

/// The column a step's mark takes in the expanded view — the width of
/// the longest mark word, so the six marks line up under themselves.
const MARK: usize = 9;

/// What the host marked on one step.
///
/// Six marks, six words. `Cancelled` is the end someone asked for and
/// `Refused` is the work's or the host's decline — the record keeps them
/// apart and so does the view. `Unknown` is not a transition: it is what
/// a step shows when the host never marked it at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mark {
    /// The step's claim is on disk; it has not started.
    Claimed,
    /// The step is in flight.
    Running,
    /// The step finished its work.
    Done,
    /// The step was declined.
    Refused,
    /// The step was stopped at the caller's request.
    Cancelled,
    /// The host never marked the step.
    #[default]
    Unknown,
}

impl Mark {
    /// The mark's own word.
    pub const fn word(self) -> &'static str {
        match self {
            Mark::Claimed => "claimed",
            Mark::Running => "running",
            Mark::Done => "done",
            Mark::Refused => "refused",
            Mark::Cancelled => "cancelled",
            Mark::Unknown => "unknown",
        }
    }

    /// The step the word draws at. A decline is loud, a step in flight
    /// stays present, a finished step recedes, and what nobody marked
    /// fades to the faintest tone.
    pub const fn tone(self) -> Intensity {
        match self {
            Mark::Claimed => Intensity::Quarter,
            Mark::Running => Intensity::ThreeQuarters,
            Mark::Done => Intensity::Half,
            Mark::Refused => Intensity::Full,
            Mark::Cancelled => Intensity::Half,
            Mark::Unknown => Intensity::Quarter,
        }
    }
}

/// Where the run's own record stands.
///
/// The words are the runstate's lifecycle: `pending` → `dispatched` →
/// `answered` | `refused` | `unverifiable` | `cancelled` → `settled`,
/// with `unknown` standing apart as recovery's mark rather than a
/// transition. Only `settled` is terminal; every other state still has
/// work the record is waiting on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// The claim is on disk; nothing has dispatched.
    Pending,
    /// Handed out and not yet back.
    Dispatched,
    /// The work produced an answer; not yet settled.
    Answered,
    /// The work declined, or the host declined it.
    Refused,
    /// The work came back with nothing anyone could check.
    Unverifiable,
    /// The caller chose to end the record.
    Cancelled,
    /// Terminal: `outcome` says what the run came to.
    Settled,
    /// Recovery's mark on a record nobody could account for.
    #[default]
    Unknown,
}

impl State {
    /// The state's own word.
    pub const fn word(self) -> &'static str {
        match self {
            State::Pending => "pending",
            State::Dispatched => "dispatched",
            State::Answered => "answered",
            State::Refused => "refused",
            State::Unverifiable => "unverifiable",
            State::Cancelled => "cancelled",
            State::Settled => "settled",
            State::Unknown => "unknown",
        }
    }

    /// The step the word draws at.
    pub const fn tone(self) -> Intensity {
        match self {
            State::Pending => Intensity::Quarter,
            State::Dispatched => Intensity::Half,
            State::Answered => Intensity::ThreeQuarters,
            State::Refused => Intensity::Full,
            State::Unverifiable => Intensity::Half,
            State::Cancelled => Intensity::Half,
            State::Settled => Intensity::Half,
            State::Unknown => Intensity::Quarter,
        }
    }

    /// Whether the record is terminal — the mark a board sorts last.
    const fn settled(self) -> bool {
        matches!(self, State::Settled)
    }
}

/// What a settled run came to — the terminal half of its state.
///
/// `Cancelled` is the end the caller asked for, never a refusal the work
/// gave and never the `unknown` a crash leaves: the distinction is the
/// whole point of durable state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The run produced its answer.
    Answered,
    /// The run was refused.
    Refused,
    /// The run finished without an answer anyone could check.
    Unverifiable,
    /// The run was stopped at the caller's request.
    Cancelled,
}

impl Outcome {
    /// The outcome's own word.
    pub const fn word(self) -> &'static str {
        match self {
            Outcome::Answered => "answered",
            Outcome::Refused => "refused",
            Outcome::Unverifiable => "unverifiable",
            Outcome::Cancelled => "cancelled",
        }
    }

    /// The step the word draws at.
    pub const fn tone(self) -> Intensity {
        match self {
            Outcome::Answered => Intensity::ThreeQuarters,
            Outcome::Refused => Intensity::Full,
            Outcome::Unverifiable => Intensity::Half,
            Outcome::Cancelled => Intensity::Half,
        }
    }
}

/// One step's claim: its name, the mark the host left, and the duration
/// it recorded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepView {
    /// The step's name in the program.
    pub name: String,
    /// The mark the host left on the step — `Unknown` when it left none.
    pub mark: Mark,
    /// How long the step took, as the caller recorded it — `None` when
    /// nobody recorded one, which renders as `unknown`, never a guess.
    pub duration_ms: Option<u64>,
}

/// What one program run's record claims, filled in by the caller.
///
/// Every field is supplied: the caller maps runstate and trace data
/// into plain values, and this module does no I/O and holds no clock.
/// The [`collapsed`][RunView::collapsed] line keeps the origin, the run
/// id, the program, the count of steps done, the step in flight, and
/// the run's state word; [`expanded`][RunView::expanded] shows
/// everything the view holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunView {
    /// Simulated or metered — carried on both presentations.
    pub origin: Origin,
    /// The run's identifier.
    pub run: String,
    /// The program's slug.
    pub program: String,
    /// The steps in program order, each with the mark the host left.
    pub steps: Vec<StepView>,
    /// The run's own state.
    pub state: State,
    /// What the run came to — drawn when `state` is [`State::Settled`],
    /// and `unknown` when a settled run carries none.
    pub outcome: Option<Outcome>,
    /// How long the run took or has taken, as the caller recorded it —
    /// `None` renders `unknown`. The view never measures.
    pub elapsed_ms: Option<u64>,
    /// What the run cost, when it was metered — `None` renders
    /// `unknown`, never a fabricated zero.
    pub cost: Option<f64>,
}

impl RunView {
    /// The collapsed summary: one line of run id, program, the count of
    /// steps done, the step in flight when one is, and the run's state
    /// word behind the origin's word — clipped to `width` and marked
    /// with `...` where it had to cut.
    pub fn collapsed(&self, width: usize) -> Line {
        let mut spans = Vec::new();
        for (index, (_, part)) in self.summary_parts().into_iter().enumerate() {
            if index > 0 {
                spans.push(Span::new("  ", Intensity::Half));
            }
            spans.extend(part);
        }
        clip(spans, width)
    }

    /// The full detail: every field the view holds, then one row per
    /// step with its mark word and the duration it recorded, and the
    /// run's settled outcome when the run is terminal.
    ///
    /// Whatever [`collapsed`][Self::collapsed] had to drop at `width` is
    /// stated twice — once because every field draws in full, and once
    /// in the `elided` row that names what the summary hid.
    pub fn expanded(&self, width: usize) -> Vec<Line> {
        let mut lines = vec![clip(
            vec![
                Span::new("run", Intensity::Full),
                Span::new("  ", Intensity::Half),
                Span::new(self.origin.word(), self.origin.tone()),
            ],
            width,
        )];

        let (run, tone) = shown(&self.run, Intensity::ThreeQuarters);
        field(&mut lines, width, "run", &run, tone);
        let (program, tone) = shown(&self.program, Intensity::Half);
        field(&mut lines, width, "program", &program, tone);

        let mut state = String::from(self.state.word());
        if self.state == State::Settled
            && let Some(outcome) = self.outcome
        {
            state.push(' ');
            state.push_str(outcome.word());
        }
        field(&mut lines, width, "state", &state, self.state.tone());

        if self.state == State::Settled {
            let (word, tone) = match self.outcome {
                Some(outcome) => (outcome.word(), outcome.tone()),
                None => ("unknown", Intensity::Quarter),
            };
            field(&mut lines, width, "outcome", word, tone);
        }

        let (done, total) = self.done();
        let count = if total == 0 {
            "no steps".to_owned()
        } else {
            format!("{done}/{total} done")
        };
        field(&mut lines, width, "steps", &count, Intensity::Half);
        let name_width = self
            .steps
            .iter()
            .map(|step| step.name.width())
            .max()
            .unwrap_or(0)
            .min(NAME);
        for step in &self.steps {
            lines.push(step_line(step, name_width, width));
        }

        let (elapsed, tone) = match self.elapsed_ms {
            Some(ms) => (format!("{ms} ms"), Intensity::ThreeQuarters),
            None => ("unknown".to_owned(), Intensity::Quarter),
        };
        field(&mut lines, width, "elapsed", &elapsed, tone);

        let (cost, tone) = match self.cost {
            Some(cost) => (format!("${cost:.4}"), Intensity::ThreeQuarters),
            None => ("unknown".to_owned(), Intensity::Quarter),
        };
        field(&mut lines, width, "cost", &cost, tone);

        let elided = self.elided(width);
        if !elided.is_empty() {
            field(
                &mut lines,
                width,
                "elided",
                &elided.join(", "),
                Intensity::Quarter,
            );
        }
        lines
    }

    /// The steps the host marked `done`, over the steps the program
    /// has — the count the collapsed line carries.
    fn done(&self) -> (usize, usize) {
        (
            self.steps
                .iter()
                .filter(|step| step.mark == Mark::Done)
                .count(),
            self.steps.len(),
        )
    }

    /// The step the run is on: the first the host marked `running`, else
    /// the first still `claimed`. The marks say which step is current —
    /// the view does not guess, so a run between steps shows none.
    fn current(&self) -> Option<&StepView> {
        self.steps
            .iter()
            .find(|step| step.mark == Mark::Running)
            .or_else(|| self.steps.iter().find(|step| step.mark == Mark::Claimed))
    }

    /// The collapsed line's fields, in order: origin first, then run,
    /// program, the count of steps done, the step in flight when one
    /// is, and the run's state word.
    fn summary_parts(&self) -> Vec<(&'static str, Vec<Span>)> {
        let single = |value: &str, tone: Intensity| {
            let (text, tone) = shown(value, tone);
            vec![Span::new(text, tone)]
        };
        let (done, total) = self.done();
        let count = if total == 0 {
            "no steps".to_owned()
        } else {
            format!("{done}/{total} steps done")
        };
        let mut state = vec![Span::new(self.state.word(), self.state.tone())];
        if self.state == State::Settled
            && let Some(outcome) = self.outcome
        {
            state.push(Span::new(" ", Intensity::Half));
            state.push(Span::new(outcome.word(), outcome.tone()));
        }
        let mut parts = vec![
            (
                "origin",
                vec![Span::new(self.origin.word(), self.origin.tone())],
            ),
            ("run", single(&self.run, Intensity::ThreeQuarters)),
            ("program", single(&self.program, Intensity::Half)),
            ("steps", vec![Span::new(count, Intensity::Half)]),
        ];
        if let Some(step) = self.current() {
            let (text, tone) = shown(&step.name, Intensity::ThreeQuarters);
            parts.push(("current", vec![Span::new(text, tone)]));
        }
        parts.push(("state", state));
        parts
    }

    /// The fields the collapsed line drops at `width`, by name. An empty
    /// list means the summary fit whole.
    fn elided(&self, width: usize) -> Vec<&'static str> {
        let parts = self.summary_parts();
        let text: usize = parts
            .iter()
            .flat_map(|(_, spans)| spans.iter())
            .map(|span| span.text.width())
            .sum();
        let separators = 2 * parts.len().saturating_sub(1);
        if text + separators <= width {
            return Vec::new();
        }
        // A field is dropped when none of its text survives the budget
        // the `...` marker leaves.
        let budget = width.saturating_sub(ELLIPSIS.len());
        let mut used = 0;
        let mut elided = Vec::new();
        for (index, (name, spans)) in parts.iter().enumerate() {
            let before = if index == 0 { 0 } else { 2 };
            let room = budget.saturating_sub(used + before);
            let part: usize = spans.iter().map(|span| span.text.width()).sum();
            if part.min(room) == 0 {
                elided.push(*name);
            }
            used += before + part;
        }
        elided
    }
}

/// A board of runs: one collapsed line each, under one ordering.
///
/// The board is how a supervised fan-out reads: every run still in
/// flight stands ahead of every run that settled, and runs within a
/// group order by run id, so a glance answers which runs are still
/// moving before it says what any of them came to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardView {
    /// The runs the board lists.
    pub runs: Vec<RunView>,
}

impl BoardView {
    /// One collapsed line per run — in-progress runs before settled
    /// ones, then by run id. The sort is stable, so two runs the
    /// ordering cannot separate keep the order the caller gave them.
    pub fn lines(&self, width: usize) -> Vec<Line> {
        let mut order: Vec<&RunView> = self.runs.iter().collect();
        order.sort_by(|a, b| {
            a.state
                .settled()
                .cmp(&b.state.settled())
                .then_with(|| a.run.cmp(&b.run))
        });
        order.iter().map(|run| run.collapsed(width)).collect()
    }
}

/// One step's row in the expanded view: its name, its mark word, and
/// the duration the host recorded — `unknown` when it recorded none.
fn step_line(step: &StepView, name_width: usize, width: usize) -> Line {
    let (name, name_tone) = shown(&step.name, Intensity::ThreeQuarters);
    let (duration, duration_tone) = match step.duration_ms {
        Some(ms) => (format!("{ms} ms"), Intensity::Half),
        None => ("unknown".to_owned(), Intensity::Quarter),
    };
    clip(
        vec![
            Span::new("  ", Intensity::Half),
            Span::new(left(&name, name_width), name_tone),
            Span::new("  ", Intensity::Half),
            Span::new(left(step.mark.word(), MARK), step.mark.tone()),
            Span::new("  ", Intensity::Half),
            Span::new(duration, duration_tone),
        ],
        width,
    )
}

/// One field in the expanded view: `name` quiet on the left, `value`
/// beside it or under it, wrapped when the width runs short so nothing
/// is cut.
fn field(lines: &mut Vec<Line>, width: usize, name: &str, value: &str, tone: Intensity) {
    if width >= STACKED {
        let inner = width - LABEL;
        for (index, range) in wrap_rows(value, inner).into_iter().enumerate() {
            let lead = if index == 0 {
                left(name, LABEL)
            } else {
                " ".repeat(LABEL)
            };
            lines.push(Line {
                spans: vec![
                    Span::new(lead, Intensity::Half),
                    Span::new(&value[range], tone),
                ],
            });
        }
    } else {
        lines.push(Line {
            spans: vec![Span::new(clip_text(name, width), Intensity::Half)],
        });
        let inner = width.saturating_sub(2).max(1);
        for range in wrap_rows(value, inner) {
            lines.push(Line {
                spans: vec![
                    Span::new("  ", Intensity::Half),
                    Span::new(&value[range], tone),
                ],
            });
        }
    }
}

/// A name the caller filled, or `unknown` at the faintest step when the
/// field was left blank — a blank field is a field nobody filled, and
/// the view says so.
fn shown(value: &str, tone: Intensity) -> (String, Intensity) {
    if value.trim().is_empty() {
        ("unknown".to_owned(), Intensity::Quarter)
    } else {
        (value.to_owned(), tone)
    }
}

/// Cuts spans at `width` cells. Whatever is dropped ends with `...`, so
/// a cut is always marked, never silent.
fn clip(spans: Vec<Span>, width: usize) -> Line {
    let total: usize = spans.iter().map(|span| span.text.width()).sum();
    if total <= width {
        return Line { spans };
    }
    let budget = width.saturating_sub(ELLIPSIS.len());
    let mut kept = Vec::new();
    let mut used = 0;
    for span in spans {
        let room = budget.saturating_sub(used);
        if room == 0 {
            break;
        }
        if span.text.width() <= room {
            used += span.text.width();
            kept.push(span);
            continue;
        }
        let mut taken = String::new();
        let mut cells = 0;
        for grapheme in span.text.graphemes(true) {
            cells += grapheme.width();
            if cells > room {
                break;
            }
            taken.push_str(grapheme);
        }
        if !taken.is_empty() {
            kept.push(Span::new(taken, span.intensity));
        }
        break;
    }
    let marker = &ELLIPSIS[..width.min(ELLIPSIS.len())];
    if !marker.is_empty() {
        kept.push(Span::new(marker, Intensity::Quarter));
    }
    Line { spans: kept }
}

/// Cuts a bare string at `width` cells, with the same `...` mark.
fn clip_text(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width <= ELLIPSIS.len() {
        return ELLIPSIS[..width].to_owned();
    }
    let room = width - ELLIPSIS.len();
    let mut taken = String::new();
    let mut cells = 0;
    for grapheme in text.graphemes(true) {
        cells += grapheme.width();
        if cells > room {
            break;
        }
        taken.push_str(grapheme);
    }
    taken.push_str(ELLIPSIS);
    taken
}

/// Pads `text` to `width` cells on the right, so a column of values
/// lines up under itself.
fn left(text: &str, width: usize) -> String {
    let taken = text.width();
    if taken >= width {
        clip_text(text, width)
    } else {
        format!("{text}{}", " ".repeat(width - taken))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A metered run mid-flight: one step done, one in flight, one
    /// claimed.
    fn running_view() -> RunView {
        RunView {
            origin: Origin::Metered,
            run: "run-7f3a".to_owned(),
            program: "site-survey".to_owned(),
            steps: vec![
                StepView {
                    name: "collect".to_owned(),
                    mark: Mark::Done,
                    duration_ms: Some(411),
                },
                StepView {
                    name: "rank".to_owned(),
                    mark: Mark::Running,
                    duration_ms: Some(96),
                },
                StepView {
                    name: "report".to_owned(),
                    mark: Mark::Claimed,
                    duration_ms: None,
                },
            ],
            state: State::Dispatched,
            outcome: None,
            elapsed_ms: Some(812),
            cost: Some(0.0021),
        }
    }

    /// The expanded view as one string, for substring checks.
    fn expanded_text(view: &RunView, width: usize) -> String {
        view.expanded(width)
            .iter()
            .map(Line::text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// One step's row out of the expanded view, by the step's name.
    fn step_row(view: &RunView, name: &str) -> String {
        expanded_text(view, 120)
            .lines()
            .find(|line| line.trim_start().starts_with(name))
            .unwrap_or_else(|| panic!("a row for {name}"))
            .to_owned()
    }

    #[test]
    fn the_collapsed_line_is_one_line_with_the_headline() {
        let line = running_view().collapsed(120);
        let text = line.text();
        for part in [
            "metered",
            "run-7f3a",
            "site-survey",
            "1/3 steps done",
            "rank",
            "dispatched",
        ] {
            assert!(text.contains(part), "{part} missing from {text}");
        }
    }

    #[test]
    fn the_collapsed_line_marks_a_cut() {
        let line = running_view().collapsed(24);
        assert!(line.width() <= 24, "{} cells", line.width());
        assert!(line.text().ends_with("..."), "{}", line.text());
        // Even an empty width still yields one line.
        assert_eq!(running_view().collapsed(0).width(), 0);
    }

    #[test]
    fn a_step_less_run_says_so() {
        let mut view = running_view();
        view.steps = Vec::new();
        let text = view.collapsed(120).text();
        assert!(text.contains("no steps"), "{text}");
    }

    #[test]
    fn the_expanded_view_has_one_row_per_step() {
        let view = running_view();
        let text = expanded_text(&view, 120);
        for (name, mark) in [
            ("collect", "done"),
            ("rank", "running"),
            ("report", "claimed"),
        ] {
            let row = step_row(&view, name);
            assert!(row.contains(mark), "{mark} missing from {row}");
            assert!(text.contains(name), "{name} missing from\n{text}");
        }
    }

    #[test]
    fn every_mark_keeps_its_own_word() {
        let mut view = running_view();
        view.steps = [
            ("step-a", Mark::Claimed, "claimed"),
            ("step-b", Mark::Running, "running"),
            ("step-c", Mark::Done, "done"),
            ("step-d", Mark::Refused, "refused"),
            ("step-e", Mark::Cancelled, "cancelled"),
            ("step-f", Mark::Unknown, "unknown"),
        ]
        .into_iter()
        .map(|(name, mark, _)| StepView {
            name: name.to_owned(),
            mark,
            duration_ms: Some(100),
        })
        .collect();
        for (name, _, word) in [
            ("step-a", Mark::Claimed, "claimed"),
            ("step-b", Mark::Running, "running"),
            ("step-c", Mark::Done, "done"),
            ("step-d", Mark::Refused, "refused"),
            ("step-e", Mark::Cancelled, "cancelled"),
            ("step-f", Mark::Unknown, "unknown"),
        ] {
            let row = step_row(&view, name);
            assert!(row.contains(word), "{word} missing from {row}");
            for other in [
                "claimed",
                "running",
                "done",
                "refused",
                "cancelled",
                "unknown",
            ] {
                if other != word {
                    assert!(!row.contains(other), "{other} in {row}");
                }
            }
        }
    }

    #[test]
    fn an_unmarked_step_is_unknown_not_a_guess() {
        let mut view = running_view();
        view.steps[2].mark = Mark::Unknown;
        let row = step_row(&view, "report");
        assert!(row.contains("unknown"), "{row}");
        for word in ["claimed", "running", "done", "refused", "cancelled"] {
            assert!(!row.contains(word), "{word} in {row}");
        }
    }

    #[test]
    fn a_cancelled_step_is_not_a_refusal() {
        let mut view = running_view();
        view.steps[1].mark = Mark::Cancelled;
        let row = step_row(&view, "rank");
        assert!(row.contains("cancelled"), "{row}");
        assert!(!row.contains("refused"), "{row}");
    }

    #[test]
    fn a_running_step_is_never_done() {
        let row = step_row(&running_view(), "rank");
        assert!(row.contains("running"), "{row}");
        assert!(!row.contains("done"), "{row}");
    }

    #[test]
    fn an_unrecorded_duration_is_unknown() {
        let row = step_row(&running_view(), "report");
        assert!(row.contains("unknown"), "{row}");
        assert!(!row.contains("0 ms"), "{row}");
    }

    #[test]
    fn unrecorded_time_and_unmetered_cost_are_unknown() {
        let mut view = running_view();
        view.elapsed_ms = None;
        view.cost = None;
        let text = expanded_text(&view, 80);
        let elapsed = text
            .lines()
            .find(|line| line.starts_with("elapsed"))
            .expect("an elapsed row");
        assert!(elapsed.contains("unknown"), "{elapsed}");
        assert!(!elapsed.contains("0 ms"), "{elapsed}");
        let cost = text
            .lines()
            .find(|line| line.starts_with("cost"))
            .expect("a cost row");
        assert!(cost.contains("unknown"), "{cost}");
        assert!(!cost.contains("$0"), "{cost}");
        assert!(!cost.contains("0.0"), "{cost}");
    }

    #[test]
    fn a_settled_run_states_its_outcome() {
        let mut view = running_view();
        view.state = State::Settled;
        view.outcome = Some(Outcome::Cancelled);
        let text = expanded_text(&view, 120);
        let outcome = text
            .lines()
            .find(|line| line.starts_with("outcome"))
            .expect("an outcome row");
        assert!(outcome.contains("cancelled"), "{outcome}");
        assert!(!outcome.contains("refused"), "{outcome}");

        // A run still in flight has no outcome row at all — the view
        // does not infer one.
        let text = expanded_text(&running_view(), 120);
        assert!(!text.contains("outcome"), "{text}");

        // And a settled run whose outcome nobody recorded says so.
        let mut view = running_view();
        view.state = State::Settled;
        let text = expanded_text(&view, 120);
        let outcome = text
            .lines()
            .find(|line| line.starts_with("outcome"))
            .expect("an outcome row");
        assert!(outcome.contains("unknown"), "{outcome}");
    }

    #[test]
    fn a_simulated_view_cannot_pass_for_metered() {
        let mut view = running_view();
        view.origin = Origin::Simulated;

        let line = view.collapsed(120);
        assert_eq!(line.spans[0].text, "simulated");
        assert_eq!(line.spans[0].intensity, Intensity::Quarter);

        let text = expanded_text(&view, 120);
        assert!(text.contains("simulated"), "{text}");
        assert!(!text.contains("metered"), "{text}");

        // A measurement names itself too, at a different step.
        let metered = running_view().collapsed(120);
        assert_eq!(metered.spans[0].text, "metered");
        assert_eq!(metered.spans[0].intensity, Intensity::Half);
    }

    #[test]
    fn the_view_stays_readable_at_forty_columns() {
        let line = running_view().collapsed(40);
        assert!(line.width() <= 40, "{} cells", line.width());
        assert!(line.text().ends_with("..."), "{}", line.text());
        for line in running_view().expanded(40) {
            assert!(
                line.width() <= 40,
                "{} is {} cells",
                line.text(),
                line.width()
            );
        }
    }

    #[test]
    fn the_detail_names_what_the_summary_dropped() {
        // A wide summary fits whole, and no elided row appears.
        let wide = expanded_text(&running_view(), 120);
        assert!(!wide.contains("elided"), "{wide}");

        // At 24 cells the summary keeps the origin and the run, and the
        // detail names what it dropped.
        let text = expanded_text(&running_view(), 24);
        let at = text.find("elided").expect("an elided row");
        let after = &text[at..];
        assert!(after.contains("current"), "{after}");
        assert!(after.contains("state"), "{after}");
    }

    #[test]
    fn the_board_orders_in_progress_before_settled_then_by_run_id() {
        let mut settled = running_view();
        settled.run = "run-b".to_owned();
        settled.state = State::Settled;
        settled.outcome = Some(Outcome::Answered);
        let mut later = running_view();
        later.run = "run-c".to_owned();
        let mut earlier = running_view();
        earlier.run = "run-a".to_owned();
        let board = BoardView {
            runs: vec![settled, later, earlier],
        };
        let lines = board.lines(120);
        assert_eq!(lines.len(), 3);
        let text: Vec<String> = lines.iter().map(Line::text).collect();
        assert!(text[0].contains("run-a"), "{}", text[0]);
        assert!(text[1].contains("run-c"), "{}", text[1]);
        assert!(text[2].contains("run-b"), "{}", text[2]);
    }

    #[test]
    fn the_board_keeps_an_unaccounted_run_in_flight() {
        // An `unknown` run never settled — it sorts with the runs still
        // moving, not behind them.
        let mut settled = running_view();
        settled.run = "run-a".to_owned();
        settled.state = State::Settled;
        settled.outcome = Some(Outcome::Answered);
        let mut unaccounted = running_view();
        unaccounted.run = "run-z".to_owned();
        unaccounted.state = State::Unknown;
        let board = BoardView {
            runs: vec![settled, unaccounted],
        };
        let lines = board.lines(120);
        assert!(lines[0].text().contains("run-z"), "{}", lines[0].text());
        assert!(lines[1].text().contains("run-a"), "{}", lines[1].text());
    }

    #[test]
    fn the_view_is_deterministic() {
        let view = running_view();
        assert_eq!(view.collapsed(60), view.collapsed(60));
        assert_eq!(view.expanded(60), view.expanded(60));
        let board = BoardView {
            runs: vec![running_view(), running_view()],
        };
        assert_eq!(board.lines(60), board.lines(60));
    }
}
