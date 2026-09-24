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
//! - **The host's own record says what it says.** The authority a run
//!   ran under, the bounds it ran within, the worktrees its record
//!   retains, and what verification came to are the record's claims: a
//!   bound nobody set is `unbounded`, a worktree nobody claims is not
//!   shown, and a run the host left mid-flight is `interrupted`, never
//!   a failure the work reported.
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

/// The widest column a retained worktree's path takes in the expanded
/// view — a longer path clips with `...` rather than pushing the
/// owning task's mark off the row.
const PATH: usize = 32;

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

/// The caller's bound on a run, as the record states it.
///
/// Every half is optional because the caller's bound is optional: a
/// bound nobody set renders `unbounded`, and the view never invents a
/// number the host did not state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    /// The most tasks the run may have in flight at once.
    pub max_concurrency: Option<usize>,
    /// The run's deadline in milliseconds, measured from when it
    /// started.
    pub deadline_ms: Option<u64>,
    /// The most the run may spend.
    pub spend_limit: Option<f64>,
}

/// A worktree the run's record retains: the checkout's path, the task
/// that owns it, and the mark on that task's record.
///
/// A retained worktree outlives the process — it is where a reconciler
/// finds the checkout a crash left. What the view shows is the
/// record's claim: the mark is the owning task's, never a guess at
/// what the disk holds, and a worktree no record claims is not shown
/// at all.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorktreeRef {
    /// The checkout's path, as the record kept it.
    pub path: String,
    /// The task whose record retains the worktree.
    pub task: String,
    /// The mark on the owning task's record.
    pub mark: Mark,
}

/// What the run's verification came to.
///
/// Four states, four words — kept apart the way the record keeps them.
/// `Passed` is the plan's own verdict. `Refused` is a decline with the
/// refusal code the record kept beside it — a failed review is its own
/// word next to the run's outcome, never a smudged pass. `Unavailable`
/// is a service the host could not reach: no answer, which is not a
/// refusal. `Unknown` is what is left when verification was expected
/// and nobody recorded what it came to.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum VerificationView {
    /// The verification plan ran and passed.
    Passed,
    /// Verification declined the work, with the record's refusal code.
    Refused {
        /// The code the record kept — `verification_unverifiable`, a
        /// review's own code, whatever the host wrote down.
        code: String,
    },
    /// The verification service could not be reached.
    Unavailable,
    /// Verification was expected and nothing recorded what it came to.
    #[default]
    Unknown,
}

impl VerificationView {
    /// The outcome's text and the step it draws at — a refusal keeps
    /// its code beside its word.
    fn shown(&self) -> (String, Intensity) {
        match self {
            VerificationView::Passed => ("passed".to_owned(), Intensity::ThreeQuarters),
            VerificationView::Refused { code } => {
                let (code, _) = shown(code, Intensity::Quarter);
                (format!("refused {code}"), Intensity::Full)
            }
            VerificationView::Unavailable => ("unavailable".to_owned(), Intensity::Half),
            VerificationView::Unknown => ("unknown".to_owned(), Intensity::Quarter),
        }
    }
}

/// What one program run's record claims, filled in by the caller.
///
/// Every field is supplied: the caller maps runstate and trace data
/// into plain values, and this module does no I/O and holds no clock.
/// The [`collapsed`][RunView::collapsed] line keeps the origin, the run
/// id, the program, the count of steps done, the step in flight, and
/// the run's state word; [`expanded`][RunView::expanded] shows
/// everything the view holds — the authority and the bounds the run
/// ran under and what verification came to, beside the steps, the
/// retained worktrees, and the marks.
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
    /// The caller's bound on the run — `None` when the run carried no
    /// budget, which renders `unbounded`, never a fabricated number.
    pub bounds: Option<Bounds>,
    /// The permit or program-authority digest the run ran under —
    /// `None` renders `unknown`, an authority the record did not keep.
    pub authorization: Option<String>,
    /// The worktrees the run's record retains, each under its owning
    /// task's mark. A worktree nobody claimed is not shown.
    pub worktrees: Vec<WorktreeRef>,
    /// What verification came to — `None` when no verification ran,
    /// which is not the `unknown` a missed record leaves.
    pub verification: Option<VerificationView>,
    /// The caller ended the run: `true` renders `cancelled`, whatever
    /// the folded state last said — the end someone asked for is never
    /// a refusal the work gave.
    pub cancelled: bool,
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
        self.detail(width, self.steps.len())
    }

    /// The full detail with the step list held to `height` rows: a
    /// program with more steps than the bound shows the first `height`
    /// and then a `N more` row that says what it hid — a cut is marked,
    /// never silent.
    pub fn expanded_within(&self, width: usize, height: usize) -> Vec<Line> {
        self.detail(width, self.steps.len().min(height))
    }

    /// The expanded render behind both presentations, with at most
    /// `step_rows` step rows before the `N more` summary.
    fn detail(&self, width: usize, step_rows: usize) -> Vec<Line> {
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

        let (authority, tone) = match &self.authorization {
            Some(authority) => shown(authority, Intensity::Half),
            None => ("unknown".to_owned(), Intensity::Quarter),
        };
        field(&mut lines, width, "authority", &authority, tone);
        field(
            &mut lines,
            width,
            "bounds",
            &bounds_text(self.bounds.unwrap_or_default()),
            Intensity::Half,
        );

        let (word, tone) = self.state_view();
        let mut state = String::from(word);
        if self.state == State::Settled && (self.outcome.is_some() || self.cancelled) {
            state.push(' ');
            state.push_str(self.outcome_view().0);
        }
        field(&mut lines, width, "state", &state, tone);

        if self.state == State::Settled {
            let (word, tone) = self.outcome_view();
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
        for step in self.steps.iter().take(step_rows) {
            lines.push(step_line(step, name_width, width));
        }
        let hidden = self.steps.len() - step_rows;
        if hidden > 0 {
            let noun = if hidden == 1 { "step" } else { "steps" };
            lines.push(clip(
                vec![
                    Span::new("  ", Intensity::Half),
                    Span::new(format!("{hidden} more {noun}"), Intensity::Quarter),
                ],
                width,
            ));
        }

        if !self.worktrees.is_empty() {
            let count = self.worktrees.len();
            let noun = if count == 1 { "worktree" } else { "worktrees" };
            field(
                &mut lines,
                width,
                "worktrees",
                &format!("{count} {noun} retained"),
                Intensity::Half,
            );
            let path_width = self
                .worktrees
                .iter()
                .map(|worktree| worktree.path.width())
                .max()
                .unwrap_or(0)
                .min(PATH);
            let task_width = self
                .worktrees
                .iter()
                .map(|worktree| worktree.task.width())
                .max()
                .unwrap_or(0)
                .min(NAME);
            for worktree in &self.worktrees {
                lines.push(worktree_line(worktree, path_width, task_width, width));
            }
        }

        if let Some(verification) = &self.verification {
            let (word, tone) = verification.shown();
            field(&mut lines, width, "verification", &word, tone);
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
                "not shown",
                &elided.join(", "),
                Intensity::Quarter,
            );
        }
        lines
    }

    /// The run's state word and tone: `cancelled` when the caller
    /// ended the record — whatever the folded state last said — and
    /// `interrupted` when recovery marked the run `unknown`, a run the
    /// host left mid-flight, which is no failure the work reported and
    /// no decline anyone gave.
    fn state_view(&self) -> (&'static str, Intensity) {
        if self.cancelled {
            ("cancelled", Intensity::Half)
        } else if self.state == State::Unknown {
            ("interrupted", Intensity::Half)
        } else {
            (self.state.word(), self.state.tone())
        }
    }

    /// The settled run's outcome word and tone: `cancelled` when the
    /// caller ended the record, else the outcome's own — `unknown`
    /// when a settled run carries none.
    fn outcome_view(&self) -> (&'static str, Intensity) {
        if self.cancelled {
            ("cancelled", Intensity::Half)
        } else {
            match self.outcome {
                Some(outcome) => (outcome.word(), outcome.tone()),
                None => ("unknown", Intensity::Quarter),
            }
        }
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
        let (word, tone) = self.state_view();
        let mut state = vec![Span::new(word, tone)];
        if self.state == State::Settled && (self.outcome.is_some() || self.cancelled) {
            let (word, tone) = self.outcome_view();
            state.push(Span::new(" ", Intensity::Half));
            state.push(Span::new(word, tone));
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

/// One retained worktree's row in the expanded view: the path the
/// record kept, the task that owns it, and the mark on that task's
/// record.
fn worktree_line(
    worktree: &WorktreeRef,
    path_width: usize,
    task_width: usize,
    width: usize,
) -> Line {
    let (path, path_tone) = shown(&worktree.path, Intensity::ThreeQuarters);
    let (task, task_tone) = shown(&worktree.task, Intensity::Half);
    clip(
        vec![
            Span::new("  ", Intensity::Half),
            Span::new(left(&path, path_width), path_tone),
            Span::new("  ", Intensity::Half),
            Span::new(left(&task, task_width), task_tone),
            Span::new("  ", Intensity::Half),
            Span::new(worktree.mark.word(), worktree.mark.tone()),
        ],
        width,
    )
}

/// The bounds line: each bound the caller stated beside each it did
/// not — `unbounded` is the honest word, never a fabricated number.
fn bounds_text(bounds: Bounds) -> String {
    let concurrency = bounds
        .max_concurrency
        .map_or_else(|| "unbounded".to_owned(), |count| count.to_string());
    let deadline = bounds
        .deadline_ms
        .map_or_else(|| "unbounded".to_owned(), |ms| format!("{ms} ms"));
    let spend = bounds
        .spend_limit
        .map_or_else(|| "unbounded".to_owned(), |limit| format!("${limit:.4}"));
    format!("concurrency {concurrency}, deadline {deadline}, spend {spend}")
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
            ..RunView::default()
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

    /// One labelled field's row out of the expanded view, by the
    /// field's name.
    fn field_row(view: &RunView, name: &str) -> String {
        expanded_text(view, 120)
            .lines()
            .find(|line| line.starts_with(name))
            .unwrap_or_else(|| panic!("a row for {name}"))
            .to_owned()
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
        assert!(!wide.contains("not shown"), "{wide}");

        // At 24 cells the summary keeps the origin and the run, and the
        // detail names what it dropped.
        let text = expanded_text(&running_view(), 24);
        let at = text.find("not shown").expect("an elided row");
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

    #[test]
    fn a_twenty_column_terminal_keeps_ordered_lines_with_marked_cuts() {
        let view = running_view();
        // Every line fits the terminal, and the fields keep their order
        // even as every value wraps.
        let lines = view.expanded(20);
        for line in &lines {
            assert!(
                line.width() <= 20,
                "{} is {} cells",
                line.text(),
                line.width()
            );
        }
        let text: Vec<String> = lines.iter().map(Line::text).collect();
        let at = |name: &str| {
            text.iter()
                .position(|line| line.starts_with(name))
                .unwrap_or_else(|| panic!("a line for {name}"))
        };
        let program = at("program");
        let authority = at("authority");
        let bounds = at("bounds");
        let state = at("state");
        let steps = at("steps");
        let elapsed = at("elapsed");
        let cost = at("cost");
        assert!(
            program < authority
                && authority < bounds
                && bounds < state
                && state < steps
                && steps < elapsed
                && elapsed < cost,
            "{}",
            text.join("\n")
        );

        // The summary still ends in a marked cut, and the detail names
        // what it dropped.
        let line = view.collapsed(20);
        assert!(line.width() <= 20, "{} cells", line.width());
        assert!(line.text().ends_with("..."), "{}", line.text());
        assert!(at("not shown") > cost, "{}", text.join("\n"));
    }

    #[test]
    fn a_step_list_past_the_height_names_what_it_hid() {
        let mut view = running_view();
        view.steps = (0..6)
            .map(|index| StepView {
                name: format!("step-{index}"),
                mark: Mark::Done,
                duration_ms: Some(10),
            })
            .collect();
        let text = view
            .expanded_within(120, 2)
            .iter()
            .map(Line::text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("step-0"), "{text}");
        assert!(text.contains("step-1"), "{text}");
        for index in 2..6 {
            assert!(!text.contains(&format!("step-{index}")), "{text}");
        }
        assert!(text.contains("4 more steps"), "{text}");

        // A list that fits the bound shows no summary at all.
        let text = view
            .expanded_within(120, 6)
            .iter()
            .map(Line::text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("step-5"), "{text}");
        assert!(!text.contains("more step"), "{text}");
    }

    #[test]
    fn an_unavailable_service_is_not_a_refusal() {
        let mut view = running_view();
        view.verification = Some(VerificationView::Unavailable);
        let row = field_row(&view, "verification");
        assert!(row.contains("unavailable"), "{row}");
        assert!(!row.contains("refused"), "{row}");
        assert!(!row.contains("unknown"), "{row}");

        view.verification = Some(VerificationView::Refused {
            code: "verification_unverifiable".to_owned(),
        });
        let row = field_row(&view, "verification");
        assert!(row.contains("refused"), "{row}");
        assert!(row.contains("verification_unverifiable"), "{row}");
        assert!(!row.contains("unavailable"), "{row}");

        // And a run with no verification shows no row at all.
        let text = expanded_text(&running_view(), 120);
        assert!(!text.contains("verification"), "{text}");
    }

    #[test]
    fn a_failed_review_keeps_the_outcome_and_its_own_word() {
        let mut view = running_view();
        view.state = State::Settled;
        view.outcome = Some(Outcome::Answered);
        view.verification = Some(VerificationView::Refused {
            code: "review_failed".to_owned(),
        });
        let outcome = field_row(&view, "outcome");
        assert!(outcome.contains("answered"), "{outcome}");
        let row = field_row(&view, "verification");
        assert!(row.contains("refused"), "{row}");
        assert!(row.contains("review_failed"), "{row}");
        assert!(!row.contains("passed"), "{row}");
        assert!(!row.contains("answered"), "{row}");
    }

    #[test]
    fn unmetered_spend_is_unknown_and_an_unset_bound_is_unbounded() {
        let mut view = running_view();
        view.cost = None;
        let cost = field_row(&view, "cost");
        assert!(cost.contains("unknown"), "{cost}");
        assert!(!cost.contains("$0"), "{cost}");
        let bounds = field_row(&view, "bounds");
        assert!(bounds.contains("unbounded"), "{bounds}");
        assert!(!bounds.contains("$0"), "{bounds}");

        // A bound the caller stated shows its number; the halves it
        // left unstated still say unbounded.
        view.bounds = Some(Bounds {
            max_concurrency: Some(4),
            ..Bounds::default()
        });
        let bounds = field_row(&view, "bounds");
        assert!(bounds.contains("concurrency 4"), "{bounds}");
        assert!(bounds.contains("deadline unbounded"), "{bounds}");
        assert!(bounds.contains("spend unbounded"), "{bounds}");
    }

    #[test]
    fn an_interrupted_run_is_not_a_failure() {
        let mut view = running_view();
        view.state = State::Unknown;
        view.steps[1].mark = Mark::Unknown;
        let state = field_row(&view, "state");
        assert!(state.contains("interrupted"), "{state}");
        assert!(!state.contains("failed"), "{state}");
        assert!(!state.contains("refused"), "{state}");
        // The collapsed line carries the same word.
        let line = view.collapsed(120);
        assert!(line.text().contains("interrupted"), "{}", line.text());
        assert!(!line.text().contains("refused"), "{}", line.text());
        // The step the host never marked still says unknown — the
        // record's own gap, not a failure either.
        let row = step_row(&view, "rank");
        assert!(row.contains("unknown"), "{row}");
        assert!(!row.contains("failed"), "{row}");
    }

    #[test]
    fn a_cancelled_run_is_the_end_someone_asked_for() {
        let mut view = running_view();
        view.cancelled = true;
        let state = field_row(&view, "state");
        assert!(state.contains("cancelled"), "{state}");
        assert!(!state.contains("refused"), "{state}");
        let line = view.collapsed(120);
        assert!(line.text().contains("cancelled"), "{}", line.text());
        assert!(!line.text().contains("refused"), "{}", line.text());

        // Even beside a folded state that says otherwise, the recorded
        // end keeps its own word.
        view.state = State::Refused;
        let state = field_row(&view, "state");
        assert!(state.contains("cancelled"), "{state}");
        assert!(!state.contains("refused"), "{state}");
    }

    #[test]
    fn a_retained_worktree_shows_its_owner_and_mark() {
        let mut view = running_view();
        view.worktrees = vec![
            WorktreeRef {
                path: "/tmp/run-7f3a/fix-tests".to_owned(),
                task: "fix-tests".to_owned(),
                mark: Mark::Running,
            },
            WorktreeRef {
                path: "/tmp/run-7f3a/add-docs".to_owned(),
                task: "add-docs".to_owned(),
                mark: Mark::Unknown,
            },
        ];
        let text = expanded_text(&view, 120);
        let row = field_row(&view, "worktrees");
        assert!(row.contains("2 worktrees retained"), "{row}");
        assert!(text.contains("/tmp/run-7f3a/fix-tests"), "{text}");
        assert!(text.contains("add-docs"), "{text}");
        let unclaimed = text
            .lines()
            .find(|line| line.contains("add-docs"))
            .expect("a row for the second worktree");
        assert!(unclaimed.contains("unknown"), "{unclaimed}");

        // A worktree nobody claims is not shown at all.
        let text = expanded_text(&running_view(), 120);
        assert!(!text.contains("worktree"), "{text}");
    }

    #[test]
    fn the_authority_a_run_ran_under_is_the_records() {
        let mut view = running_view();
        view.authorization = Some("sha256:9f4c".to_owned());
        let row = field_row(&view, "authority");
        assert!(row.contains("sha256:9f4c"), "{row}");

        // An authority the record did not keep says unknown.
        let row = field_row(&running_view(), "authority");
        assert!(row.contains("unknown"), "{row}");
    }
}
