//! What a recorded run says about the rules and the seam.
//!
//! The scorer reads a [`Replay`] and counts. For each gesture the run
//! asked for: how often the rules did what the gesture owes, how long
//! they took, how often they did nothing, how often their label flipped,
//! and how the deciding margins are spread. For the seam: what it
//! answered, at what probabilities, and whether acting on its answers
//! would have matched the prompt more often than the rules did.
//!
//! Nothing here tunes a constant. The numbers say where a constant is
//! wrong; moving one is a separate change, scored by replaying the same
//! runs.
//!
//! Every number is a function of the run file alone, so the same file
//! scores the same on every platform and the report says which camera
//! the run came from rather than leaving two cameras to be read as one.

use std::collections::BTreeMap;

use coder_hands::judge::{ASK_SPACING, DEADLINE, FLICKER, THIN_MARGIN};
use coder_hands::judge::{Floors, ReportView, Trigger, window_frames};

use crate::answers::Answer;
use crate::replay::{Beat, Replay};
use crate::run::{self, Phase, Run};
use coder_hands::watch;

/// The floors the sweep reads, from lowest to highest, so the report
/// says what a floor would cost before anybody moves one.
pub const SWEEP: &[f64] = &[0.50, 0.60, 0.70, 0.80, 0.90];

/// A spread of numbers: the tenth percentile, the median, and the
/// ninetieth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    pub low: f64,
    pub mid: f64,
    pub high: f64,
}

/// The frame rate the run arrived at, and what it means for a window.
#[derive(Clone, Debug, PartialEq)]
pub struct Rate {
    /// Rows in the run.
    pub frames: usize,
    /// Rows whose frame carried a hand the rules could read.
    pub hand_frames: usize,
    /// Wall time from the first row to the last.
    pub seconds: f64,
    /// Frames a second, from the median gap between rows.
    pub fps: f64,
    /// How many frames the seam's window holds at this rate, which
    /// [`window_frames`] sizes from it.
    pub window_frames: usize,
    /// What that window is worth in
    /// wall time at that rate.
    pub window_seconds: f64,
    /// The median seconds one run of a single rule label lasts.
    pub label_run_seconds: Option<f64>,
    /// Windows of [`Rate::window_frames`] hand frames the run holds.
    pub windows: usize,
    /// How many of them cover more than one cue, which is a window that
    /// describes more than one gesture.
    pub windows_over_one_cue: usize,
    /// The median count of distinct rule labels in one window.
    pub labels_per_window: Option<f64>,
}

/// The triggers the pace sweep reads. The first row is the unspaced
/// trigger, which asked on every ambiguous transition; the second is
/// the one the desk runs. The rows after them move one number at a time,
/// so the report says what a spacing, a thin margin, and a flicker count
/// each cost before anybody moves one.
pub const PACE_SWEEP: &[Trigger] = &[
    Trigger {
        thin: THIN_MARGIN,
        flicker: FLICKER,
        spacing: 0,
    },
    Trigger {
        thin: THIN_MARGIN,
        flicker: FLICKER,
        spacing: ASK_SPACING,
    },
    Trigger {
        thin: THIN_MARGIN,
        flicker: FLICKER,
        spacing: 2,
    },
    Trigger {
        thin: 0.05,
        flicker: FLICKER,
        spacing: ASK_SPACING,
    },
    Trigger {
        thin: 0.25,
        flicker: FLICKER,
        spacing: ASK_SPACING,
    },
    Trigger {
        thin: THIN_MARGIN,
        flicker: 3,
        spacing: ASK_SPACING,
    },
    Trigger {
        thin: THIN_MARGIN,
        flicker: 8,
        spacing: ASK_SPACING,
    },
];

/// How often the seam is asked, and how much of that one request in
/// flight can carry.
#[derive(Clone, Debug, PartialEq)]
pub struct Pace {
    /// Windows the trigger asked about.
    pub asks: usize,
    /// Those asks over the run's wall time.
    pub a_second: f64,
    /// Ambiguous windows the trigger held back, which the desk records
    /// as `paced` rather than dropping.
    pub paced: usize,
    /// How many of the asks would find the seam free if every round trip
    /// took the whole second the deadline allows. It needs no round trip
    /// measured against a live API. A round trip longer than the
    /// deadline holds the seam longer than this assumes, and the `in
    /// flight` lines of a session's transcript are what say how often
    /// that happened at the desk.
    pub served: usize,
    /// One row a trigger, so the before and after of a trigger that
    /// moves are read from the same run.
    pub sweep: Vec<PaceRow>,
}

/// What one trigger would ask of one run.
#[derive(Clone, Debug, PartialEq)]
pub struct PaceRow {
    /// The margin under which the trigger calls a transition ambiguous.
    pub thin: f32,
    /// Label flips inside one window past which the trigger calls the
    /// window ambiguous whatever the margin says.
    pub flicker: usize,
    /// Windows of hand the trigger puts between one ask and the next.
    pub spacing: usize,
    /// Windows it asks about.
    pub asks: usize,
    /// Those asks over the run's wall time.
    pub a_second: f64,
    /// Ambiguous windows it holds back.
    pub paced: usize,
    /// Asks the seam would carry with every round trip taking the whole
    /// second the deadline allows.
    pub served: usize,
}

/// What the rules did with one gesture.
#[derive(Clone, Debug, PartialEq)]
pub struct GestureRow {
    /// The gesture's label.
    pub label: String,
    /// How many times the run asked for it.
    pub cues: usize,
    /// How many of those the rules acted on, which means every act the
    /// gesture owes appeared.
    pub acted: usize,
    /// How many carried a hand the rules read at all.
    pub with_hand: usize,
    /// Acts that command the desk and the gesture does not owe.
    pub stray: usize,
    /// Pointer moves, which cost nothing and are not errors.
    pub moves: usize,
    /// The median seconds from the prompt to the first act the gesture
    /// owes, over the cues that acted.
    pub first_act: Option<f64>,
    /// The mean count of rule-label changes inside one cue.
    pub flips: f64,
    /// The spread of deciding margins over the frames with a hand.
    pub margins: Option<Spread>,
    /// The share of those frames whose margin is inside the seam's
    /// [`THIN_MARGIN`], which is what makes a transition ambiguous.
    pub thin: f64,
    /// Windows inside these cues the seam would be asked about.
    pub asks: usize,
}

/// What the rules did while you were counted in, which is the closest a
/// run comes to a hand that is not addressing the desk.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RestRow {
    pub cues: usize,
    pub frames: usize,
    pub stray: usize,
    pub moves: usize,
    pub asks: usize,
}

/// One floor, and what acting at it would have done.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepRow {
    /// The floor every action carries in this row. The two Noul floors
    /// stay where they are.
    pub floor: f64,
    /// Windows the table would act on.
    pub acts: usize,
    /// Of those, the ones naming an act the cue owes.
    pub right: usize,
    /// Of those, the ones naming another command.
    pub wrong: usize,
}

/// What the seam answered about one gesture.
#[derive(Clone, Debug, PartialEq)]
pub struct SeamRow {
    pub label: String,
    /// Answered windows inside this gesture's cues.
    pub windows: usize,
    /// Windows whose answer names an act the gesture owes.
    pub seam_right: usize,
    /// Windows whose answer names another command.
    pub seam_wrong: usize,
    /// Windows whose answer drops, holds, or rests.
    pub seam_quiet: usize,
    /// Windows where the rules did an act the gesture owes on the same
    /// frame.
    pub rules_right: usize,
}

/// What the seam did over the whole run.
#[derive(Clone, Debug, PartialEq)]
pub struct SeamReport {
    /// Windows the replay asked the seam about, which is what the
    /// trigger let through rather than every ambiguous window it saw.
    /// [`Pace`] holds the ones it held back.
    pub asked: usize,
    /// Windows an answer was kept for.
    pub answered: usize,
    /// Requests that failed.
    pub failed: usize,
    /// Ambiguous windows with no answer beside them, which a fresh ask
    /// would cover.
    pub uncovered: usize,
    /// Answers that arrived inside the seam's deadline.
    pub met_deadline: usize,
    /// The spread of round trips, in milliseconds.
    pub round_trip_ms: Option<Spread>,
    /// The spread of the top intent's confidence.
    pub confidence: Option<Spread>,
    /// The spread of the `addressed` answer.
    pub addressed: Option<Spread>,
    /// The spread of the `continuing` answer.
    pub continuing: Option<Spread>,
    /// One row a gesture.
    pub rows: Vec<SeamRow>,
    /// What each floor would act on.
    pub sweep: Vec<SweepRow>,
}

/// Everything one run says.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// The run's file.
    pub run: String,
    /// The camera the run was recorded from, and none for a run written
    /// before the header carried one.
    pub source: Option<run::Source>,
    /// The frame rate and what it means for a window.
    pub rate: Rate,
    /// How often the seam is asked, and what one request in flight
    /// covers of it.
    pub pace: Pace,
    /// One row a gesture, in the order the script asks for them.
    pub gestures: Vec<GestureRow>,
    /// The count-in frames.
    pub rest: RestRow,
    /// What the seam answered, when a run has answers beside it.
    pub seam: Option<SeamReport>,
}

/// One stretch of rows with the same cue and phase.
struct Segment<'a> {
    label: &'a str,
    phase: Phase,
    beats: &'a [Beat],
}

/// Scores one run.
#[must_use]
pub fn score(name: &str, run: &Run, replay: &Replay, answers: &[Answer]) -> Report {
    let segments = segments(replay);
    let mut gestures = Vec::new();
    for cue in ordered_labels(run) {
        gestures.push(gesture_row(&cue, &segments));
    }
    Report {
        run: name.to_string(),
        source: run.header.source,
        rate: rate(replay),
        pace: pace(run, replay),
        gestures,
        rest: rest_row(&segments),
        seam: seam(replay, answers),
    }
}

/// How often the trigger asks, and how much of that one request in
/// flight covers, at the trigger the replay ran and at each of
/// [`PACE_SWEEP`].
fn pace(run: &Run, replay: &Replay) -> Pace {
    let seconds = replay.seconds();
    let deadline = DEADLINE.as_secs_f64();
    let sweep = PACE_SWEEP
        .iter()
        .map(|trigger| {
            let swept = Replay::of_with(run, *trigger);
            PaceRow {
                thin: trigger.thin,
                flicker: trigger.flicker,
                spacing: trigger.spacing,
                asks: swept.asks(),
                a_second: a_second(swept.asks(), seconds),
                paced: swept.paced(),
                served: served(&swept.ask_times(), deadline),
            }
        })
        .collect();
    Pace {
        asks: replay.asks(),
        a_second: a_second(replay.asks(), seconds),
        paced: replay.paced(),
        served: served(&replay.ask_times(), deadline),
        sweep,
    }
}

/// A count over a run's wall time, and zero for a run with no span.
fn a_second(count: usize, seconds: f64) -> f64 {
    if seconds > 0.0 {
        count as f64 / seconds
    } else {
        0.0
    }
}

/// How many of `times` find the seam free when every request holds it
/// for `round_trip` seconds, which is what one request in flight means.
fn served(times: &[f64], round_trip: f64) -> usize {
    let mut free_at = f64::NEG_INFINITY;
    let mut served = 0;
    for at in times {
        if *at >= free_at {
            served += 1;
            free_at = at + round_trip;
        }
    }
    served
}

/// The labels the script asks for, in script order and without repeats.
fn ordered_labels(run: &Run) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for cue in &run.header.script {
        if !seen.iter().any(|label| label == &cue.label) {
            seen.push(cue.label.clone());
        }
    }
    seen
}

/// The run cut into stretches of one cue and one phase.
fn segments(replay: &Replay) -> Vec<Segment<'_>> {
    let mut out: Vec<Segment<'_>> = Vec::new();
    let mut start = 0;
    while start < replay.beats.len() {
        let head = &replay.beats[start];
        let mut end = start + 1;
        while end < replay.beats.len()
            && replay.beats[end].cue == head.cue
            && replay.beats[end].phase == head.phase
        {
            end += 1;
        }
        out.push(Segment {
            label: &head.label,
            phase: head.phase,
            beats: &replay.beats[start..end],
        });
        start = end;
    }
    out
}

/// The frame rate, the window [`window_frames`] sizes from it, and how
/// much of the run one such window covers.
fn rate(replay: &Replay) -> Rate {
    let beats = &replay.beats;
    let gaps: Vec<f64> = beats
        .windows(2)
        .map(|pair| pair[1].t - pair[0].t)
        .filter(|gap| *gap > 0.0 && *gap < 5.0)
        .collect();
    let gap = percentile(&gaps, 0.5).unwrap_or(0.0);
    let fps = if gap > 0.0 { 1.0 / gap } else { 0.0 };
    let seconds = match (beats.first(), beats.last()) {
        (Some(first), Some(last)) => (last.t - first.t).max(0.0),
        _ => 0.0,
    };
    let hands: Vec<&Beat> = beats.iter().filter(|beat| beat.hand).collect();
    let held = window_frames(fps);
    let mut windows = 0;
    let mut over_one_cue = 0;
    let mut labels: Vec<f64> = Vec::new();
    for start in 0..hands.len().saturating_sub(held - 1) {
        let window = &hands[start..start + held];
        windows += 1;
        let cues: Vec<usize> = distinct(window.iter().map(|beat| beat.cue));
        if cues.len() > 1 {
            over_one_cue += 1;
        }
        labels.push(distinct(window.iter().map(|beat| beat.rule)).len() as f64);
    }
    Rate {
        frames: beats.len(),
        hand_frames: hands.len(),
        seconds,
        fps,
        window_frames: held,
        window_seconds: if fps > 0.0 { held as f64 / fps } else { 0.0 },
        label_run_seconds: percentile(&label_runs(&hands), 0.5),
        windows,
        windows_over_one_cue: over_one_cue,
        labels_per_window: percentile(&labels, 0.5),
    }
}

/// How long each run of one rule label lasted, in seconds.
fn label_runs(hands: &[&Beat]) -> Vec<f64> {
    let mut runs = Vec::new();
    let mut start = 0;
    while start < hands.len() {
        let mut end = start + 1;
        while end < hands.len() && hands[end].rule == hands[start].rule {
            end += 1;
        }
        runs.push(hands[end - 1].t - hands[start].t);
        start = end;
    }
    runs
}

/// The values of an iterator, without repeats, in the order they came.
fn distinct<T: PartialEq>(values: impl Iterator<Item = T>) -> Vec<T> {
    let mut out: Vec<T> = Vec::new();
    for value in values {
        if !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

/// One gesture's row, over every cue that asked for it.
fn gesture_row(label: &str, segments: &[Segment<'_>]) -> GestureRow {
    let owed: &[&str] = run::gesture(label).map(|g| g.owed).unwrap_or(&[]);
    let mut row = GestureRow {
        label: label.to_string(),
        cues: 0,
        acted: 0,
        with_hand: 0,
        stray: 0,
        moves: 0,
        first_act: None,
        flips: 0.0,
        margins: None,
        thin: 0.0,
        asks: 0,
    };
    let mut firsts = Vec::new();
    let mut flips = Vec::new();
    let mut margins = Vec::new();
    let mut thin = 0;
    for segment in segments
        .iter()
        .filter(|s| s.phase == Phase::Record && s.label == label)
    {
        row.cues += 1;
        let hands: Vec<&Beat> = segment.beats.iter().filter(|beat| beat.hand).collect();
        if !hands.is_empty() {
            row.with_hand += 1;
        }
        let start = segment.beats.first().map(|beat| beat.t).unwrap_or_default();
        let mut wanted = owed.iter();
        let mut next = wanted.next();
        let mut first = None;
        for beat in segment.beats {
            row.asks += usize::from(beat.ask.is_some());
            for act in &beat.acts {
                let moved = act == "point" || act == "drag";
                row.moves += usize::from(moved);
                if next.is_some_and(|owed| owed == act) {
                    first.get_or_insert(beat.t - start);
                    next = wanted.next();
                } else if !moved
                    && run::COMMANDS.contains(&act.as_str())
                    && !owed.contains(&act.as_str())
                {
                    row.stray += 1;
                }
            }
        }
        if next.is_none() {
            row.acted += 1;
        }
        if let Some(seconds) = first {
            firsts.push(seconds);
        }
        flips.push(label_flips(&hands) as f64);
        for beat in &hands {
            margins.push(f64::from(beat.margin));
            thin += usize::from(beat.margin.abs() < THIN_MARGIN);
        }
    }
    row.first_act = percentile(&firsts, 0.5);
    row.flips = mean(&flips);
    row.margins = spread(&margins);
    row.thin = if margins.is_empty() {
        0.0
    } else {
        thin as f64 / margins.len() as f64
    };
    row
}

/// How many times the rules' label changed inside one cue.
fn label_flips(hands: &[&Beat]) -> usize {
    hands
        .windows(2)
        .filter(|pair| pair[0].rule != pair[1].rule)
        .count()
}

/// The count-in frames of every cue, taken together.
fn rest_row(segments: &[Segment<'_>]) -> RestRow {
    let mut row = RestRow::default();
    for segment in segments.iter().filter(|s| s.phase == Phase::Ready) {
        row.cues += 1;
        row.frames += segment.beats.len();
        for beat in segment.beats {
            row.asks += usize::from(beat.ask.is_some());
            for act in &beat.acts {
                if act == "point" || act == "drag" {
                    row.moves += 1;
                } else if run::COMMANDS.contains(&act.as_str()) {
                    row.stray += 1;
                }
            }
        }
    }
    row
}

/// What the seam answered, when a run has answers beside it.
fn seam(replay: &Replay, answers: &[Answer]) -> Option<SeamReport> {
    if answers.is_empty() {
        return None;
    }
    let asked = replay.asks();
    let held: Vec<&Answer> = answers.iter().filter(|a| a.report.is_some()).collect();
    let mut rows: BTreeMap<String, SeamRow> = BTreeMap::new();
    let mut confidence = Vec::new();
    let mut addressed = Vec::new();
    let mut continuing = Vec::new();
    let mut trips = Vec::new();
    for answer in &held {
        let Some(report) = &answer.report else {
            continue;
        };
        trips.push(answer.elapsed_ms as f64);
        confidence.push(report.confidence);
        addressed.push(report.addressed);
        continuing.push(report.continuing);
        let owed = owed_of(answer);
        let row = rows.entry(answer.label.clone()).or_insert_with(|| SeamRow {
            label: answer.label.clone(),
            windows: 0,
            seam_right: 0,
            seam_wrong: 0,
            seam_quiet: 0,
            rules_right: 0,
        });
        row.windows += 1;
        match acted_word(report, Floors::default()) {
            Some(word) if owed.contains(&word) => row.seam_right += 1,
            Some(_) => row.seam_wrong += 1,
            None => row.seam_quiet += 1,
        }
        if answer.rules.iter().any(|act| owed.contains(&act.as_str())) {
            row.rules_right += 1;
        }
    }
    let covered: Vec<usize> = answers.iter().map(|answer| answer.row).collect();
    let uncovered = replay
        .beats
        .iter()
        .filter(|beat| beat.ask.is_some() && !covered.contains(&beat.row))
        .count();
    Some(SeamReport {
        asked,
        answered: held.len(),
        failed: answers.iter().filter(|a| a.failed.is_some()).count(),
        uncovered,
        met_deadline: held.iter().filter(|a| a.met_deadline).count(),
        round_trip_ms: spread(&trips),
        confidence: spread(&confidence),
        addressed: spread(&addressed),
        continuing: spread(&continuing),
        rows: rows.into_values().collect(),
        sweep: sweep(&held),
    })
}

/// What each floor would act on, over the answers a run holds.
fn sweep(held: &[&Answer]) -> Vec<SweepRow> {
    SWEEP
        .iter()
        .map(|floor| {
            let mut row = SweepRow {
                floor: *floor,
                acts: 0,
                right: 0,
                wrong: 0,
            };
            for answer in held {
                let Some(report) = &answer.report else {
                    continue;
                };
                let floors = Floors {
                    act: *floor,
                    press: *floor,
                    escape: *floor,
                    ..Floors::default()
                };
                if let Some(word) = acted_word(report, floors) {
                    row.acts += 1;
                    if owed_of(answer).contains(&word) {
                        row.right += 1;
                    } else {
                        row.wrong += 1;
                    }
                }
            }
            row
        })
        .collect()
}

/// The acts the cue an answer belongs to owes. A count-in owes nothing,
/// so every command on it is one the desk would have taken uninvited.
fn owed_of(answer: &Answer) -> &'static [&'static str] {
    match answer.phase {
        Phase::Ready => &[],
        Phase::Record => run::gesture(&answer.label).map(|g| g.owed).unwrap_or(&[]),
    }
}

/// The act word one answer would move the desk with at these floors, and
/// nothing when the table drops, holds, or rests.
fn acted_word(report: &ReportView, floors: Floors) -> Option<&'static str> {
    // The act carries a point, which this scorer does not read: it
    // counts which act the answer names, not where it would land.
    watch::act_of(report.verdict_with(floors), Some((0.0, 0.0))).map(|act| act.word())
}

/// The mean of a list, and zero for an empty one.
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// The value at `fraction` of a sorted copy of `values`.
fn percentile(values: &[f64], fraction: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted.get(at).copied()
}

/// The tenth, fiftieth, and ninetieth percentiles of a list.
fn spread(values: &[f64]) -> Option<Spread> {
    Some(Spread {
        low: percentile(values, 0.1)?,
        mid: percentile(values, 0.5)?,
        high: percentile(values, 0.9)?,
    })
}

#[cfg(test)]
#[path = "score_tests.rs"]
mod tests;
