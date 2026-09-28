//! One plain-language sentence per board, and the one that tops the boards
//! list, built from the board's tallies and rows by code.
//!
//! A board's `headline` is the study's sentence, shown verbatim. The
//! summary is the reader's one-line answer to "what does this board show":
//! who beats whom, on what, in how many of how many, with the qualifier
//! that limits it in the same sentence (`in-sample`, `held-out`, `not
//! pre-registered`, the thin margins, the reliability gap). No number in it
//! is typed by hand, and none is summed or averaged across boards.
//!
//! The list's top sentence is one board's summary, chosen by the evidence
//! behind it rather than by a score: a board with at least one beat, then
//! held-out over not, pre-registered over not, and not in-sample over
//! in-sample; ties keep the publication's order. It names the board it
//! came from. See "Presentation rules" in `docs/verse/gym-leaderboard.md`.

use std::fmt::Write as _;

use serde::Serialize;

use crate::contract::{Board, BoardKind, Label, Leaderboard};
use crate::{median, pct};

/// The boards list's top sentence and the board it came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TopSummary {
    /// The board the sentence summarizes; tapping it opens that board.
    pub board: String,
    pub text: String,
    /// "From one board: <title>. Boards aren't pooled."
    pub source: String,
}

/// The board's one-sentence summary.
#[must_use]
pub fn board_summary(board: &Board) -> String {
    let has = |label: Label| board.labels.contains(&label);
    let agent = &board.subject.agent;
    let reference = &board.reference.name;
    let bench = format!("{} {}", board.benchmark.name, board.benchmark.version);
    let held_out = if has(Label::OutOfSample) {
        "held-out "
    } else {
        ""
    };

    if board.kind == BoardKind::Reference {
        let host = board
            .snapshot
            .as_ref()
            .map_or(reference.as_str(), |s| s.host.as_str());
        let date = board
            .snapshot
            .as_ref()
            .map(|s| s.fetched_at.get(..10).unwrap_or(&s.fetched_at));
        let mut text = format!("{host}'s public {bench} leaderboard");
        if let Some(date) = date {
            let _ = write!(text, " as fetched on {date}");
        }
        text.push_str(
            ": context for the other boards, not a result of ours and not ranked against them.",
        );
        return text;
    }

    let totals = &board.totals;
    if totals.beats == 0 {
        return format!(
            "{agent} hasn't beaten {reference} on {held_out}{bench} tasks yet: {} of {} attempts passed and {} of {} beat the bar.",
            totals.passes, totals.attempts, totals.beats, totals.attempts
        );
    }

    if board.kind == BoardKind::CostBelowReferencePerTrial
        && let Some(text) = per_trial_summary(board, agent, reference, &bench, held_out)
    {
        return text;
    }

    let what = match board.kind {
        BoardKind::BeatCheapestAndFastestWin => {
            format!("{reference}'s cheapest and fastest win on both cost and time")
        }
        BoardKind::CostBelowCheapestWin => format!("{reference}'s cheapest winning run on cost"),
        BoardKind::CostBelowReferencePerTrial => format!("{reference}'s cost per trial"),
        BoardKind::Reference | BoardKind::Other => format!("{reference}'s bar"),
    };
    let mut text = format!(
        "{agent} beats {what} in {} of {} attempts on {held_out}{bench} tasks",
        totals.beats, totals.attempts
    );
    let beats: Vec<_> = board.attempts.iter().filter(|a| a.beat).collect();
    let in_sample = beats
        .iter()
        .filter(|a| a.labels.contains(&Label::InSample))
        .count();
    let beat_count = beats.len();
    if in_sample == beat_count || (in_sample == 0 && has(Label::InSample)) {
        text.push_str(
            ", all in-sample: using knowledge written from earlier runs of the same task",
        );
    } else if in_sample > 0 {
        let _ = write!(
            text,
            ", {in_sample} of those {beat_count} in-sample: using knowledge written from earlier runs of the same task"
        );
    }
    let thin = beats
        .iter()
        .filter(|a| a.labels.contains(&Label::ThinMargin))
        .count();
    if thin > 0 {
        let _ = write!(
            text,
            "; {thin} of the {beat_count} by a margin under {}",
            pct(crate::THIN_MARGIN)
        );
    }
    if !has(Label::PreRegistered) {
        text.push_str("; not pre-registered");
    }
    text.push('.');
    text
}

/// The per-trial board's sentence: the median pass's share of the
/// reference's cost, and the reliability gap beside it.
fn per_trial_summary(
    board: &Board,
    agent: &str,
    reference: &str,
    bench: &str,
    held_out: &str,
) -> Option<String> {
    let ratios: Vec<f64> = board
        .attempts
        .iter()
        .filter(|a| a.passed && !a.cost_ratio_is_bound)
        .filter_map(|a| a.cost_ratio)
        .collect();
    let share = median(&ratios)?;
    let mut text = format!(
        "{agent}'s median pass cost {} of {reference}'s cost per trial on {} {held_out}{bench} tasks ({} passes with a known cost)",
        pct(share),
        board.tasks.len(),
        ratios.len()
    );
    if let Some(first) = board.splits.first() {
        let _ = write!(
            text,
            ", but {} passed only {} of {}",
            first.name, first.tally.passes, first.tally.attempts
        );
        let (passes, trials) = board.tasks.iter().fold((0, 0), |(p, t), row| {
            (
                p + row.bar.reference_passes.unwrap_or(0),
                t + row.bar.reference_trials.unwrap_or(0),
            )
        });
        if trials > 0 {
            let _ = write!(text, ", against {reference}'s {passes} of {trials} trials");
        }
    }
    if !board.labels.contains(&Label::PreRegistered) {
        text.push_str("; not pre-registered");
    }
    text.push('.');
    Some(text)
}

/// Where a board's evidence stands, strongest first when sorted
/// descending. Not a score: no rate or dollar figure is compared.
fn evidence(board: &Board) -> (bool, bool, bool, bool) {
    let has = |label: Label| board.labels.contains(&label);
    (
        board.kind != BoardKind::Reference && board.totals.beats > 0,
        has(Label::OutOfSample),
        has(Label::PreRegistered),
        !has(Label::InSample),
    )
}

/// The boards list's top sentence: the summary of the board with the
/// strongest evidence behind a beat, or `None` when no board has a beat.
#[must_use]
pub fn top_summary(leaderboard: &Leaderboard) -> Option<TopSummary> {
    let mut best: Option<&Board> = None;
    for board in &leaderboard.boards {
        if best.is_none_or(|b| evidence(board) > evidence(b)) {
            best = Some(board);
        }
    }
    let board = best.filter(|b| evidence(b).0)?;
    Some(TopSummary {
        board: board.id.clone(),
        text: board_summary(board),
        source: format!(
            "From one board: {}. Each board is its own study; none is pooled with another.",
            board.title
        ),
    })
}
