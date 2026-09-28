//! One scored run as text.
//!
//! The report is numbers in columns: what the rules did with each
//! gesture, what a window is worth at the rate the frames arrived, and
//! what the seam answered. It draws no conclusion, because the counts
//! are what the two issues wait on.

use std::fmt::Write as _;

use crate::answers;
use crate::replay::Replay;
use crate::score::{Pace, Report, SeamReport, Spread};

/// The whole report.
#[must_use]
pub fn render(report: &Report) -> String {
    let mut out = String::new();
    let rate = &report.rate;
    let _ = writeln!(out, "Run: {}", report.run);
    let _ = writeln!(
        out,
        "Recorded from: {}.",
        match report.source {
            Some(source) => source.word(),
            None => "a camera the run does not name",
        }
    );
    let _ = writeln!(
        out,
        "Frames: {} over {:.1} s, {:.1} a second. {} carried a hand.",
        rate.frames, rate.seconds, rate.fps, rate.hand_frames
    );
    let _ = writeln!(
        out,
        "The seam's window holds {} frame(s) at that rate, which is {:.2} s.",
        rate.window_frames, rate.window_seconds
    );
    if report
        .gestures
        .iter()
        .all(|row| row.label == crate::run::UNLABELLED)
    {
        let _ = writeln!(
            out,
            "Nobody was prompted while this was recorded, so no line of it \
             says what the hand meant. Every act is counted under stray \
             because the run owes none, and the windows below cover one \
             label rather than one gesture."
        );
    }
    let _ = writeln!(
        out,
        "Label runs: median {}. Windows: {}, of which {} cover more than one \
         gesture; median {} label(s) a window.",
        seconds(rate.label_run_seconds),
        rate.windows,
        rate.windows_over_one_cue,
        number(rate.labels_per_window)
    );
    let _ = writeln!(out);
    render_pace(&mut out, &report.pace);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{:<12}{:>6}{:>7}{:>7}{:>7}{:>7}{:>11}{:>7}  {:<22}{:>6}{:>6}",
        "Gesture",
        "cues",
        "acted",
        "hand",
        "stray",
        "moves",
        "first act",
        "flips",
        "margin p10/p50/p90",
        "thin",
        "asks"
    );
    for row in &report.gestures {
        let _ = writeln!(
            out,
            "{:<12}{:>6}{:>7}{:>7}{:>7}{:>7}{:>11}{:>7.1}  {:<22}{:>5.0}%{:>6}",
            row.label,
            row.cues,
            row.acted,
            row.with_hand,
            row.stray,
            row.moves,
            seconds(row.first_act),
            row.flips,
            margins(row.margins),
            row.thin * 100.0,
            row.asks
        );
    }
    let _ = writeln!(out);
    for line in [
        "acted counts the cues where every act the gesture owes appeared;",
        "hand counts the cues the rules read a hand in; stray counts",
        "commands the cue never asked for; thin is the share of frames",
        "whose margin the seam reads as ambiguous.",
    ] {
        let _ = writeln!(out, "{line}");
    }
    let rest = &report.rest;
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Count-in: {} stretch(es), {} frame(s), {} command(s) nobody asked for, \
         {} pointer move(s), {} ask(s).",
        rest.cues, rest.frames, rest.stray, rest.moves, rest.asks
    );
    let _ = writeln!(out);
    match &report.seam {
        Some(seam) => render_seam(&mut out, seam),
        None => {
            let _ = writeln!(
                out,
                "Seam: no answers beside this run. Ask once, then score again:"
            );
            let _ = writeln!(
                out,
                "  coder-hands-measure ask {}\n  the answers land in {}",
                report.run,
                answers::beside(std::path::Path::new(&report.run)).display()
            );
        }
    }
    out
}

/// How often the trigger asks, what it holds back, and what one request
/// in flight covers of it.
fn render_pace(out: &mut String, pace: &Pace) {
    let _ = writeln!(
        out,
        "Asks: {} over the run, {:.2} a second. The trigger held {} more ambiguous \
         window(s) back, which the desk records as skipped rather than dropping.",
        pace.asks, pace.a_second, pace.paced
    );
    let _ = writeln!(
        out,
        "If every round trip took the whole second the deadline allows, one request \
         in flight would carry {} of those {} ask(s).",
        pace.served, pace.asks
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{:<7}{:>9}{:>9}{:>7}{:>11}{:>8}{:>8}   spacing 0 is the unspaced trigger",
        "thin", "flicker", "spacing", "asks", "a second", "paced", "served"
    );
    for row in &pace.sweep {
        let _ = writeln!(
            out,
            "{:<7.2}{:>9}{:>9}{:>7}{:>11.2}{:>8}{:>8}",
            row.thin, row.flicker, row.spacing, row.asks, row.a_second, row.paced, row.served
        );
    }
}

/// The seam's half of the report.
fn render_seam(out: &mut String, seam: &SeamReport) {
    let _ = writeln!(
        out,
        "Seam: {} window(s) asked about, {} answered, {} failed, {} with no answer beside them.",
        seam.asked, seam.answered, seam.failed, seam.uncovered
    );
    let _ = writeln!(
        out,
        "Deadline met by {} of {}. Round trip p10/p50/p90: {}.",
        seam.met_deadline,
        seam.answered,
        millis(seam.round_trip_ms)
    );
    let _ = writeln!(
        out,
        "confidence {}; addressed {}; continuing {}.",
        probability(seam.confidence),
        probability(seam.addressed),
        probability(seam.continuing)
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{:<12}{:>9}{:>12}{:>12}{:>12}{:>13}",
        "Gesture", "windows", "seam right", "seam wrong", "seam quiet", "rules right"
    );
    for row in &seam.rows {
        let _ = writeln!(
            out,
            "{:<12}{:>9}{:>12}{:>12}{:>12}{:>13}",
            row.label, row.windows, row.seam_right, row.seam_wrong, row.seam_quiet, row.rules_right
        );
    }
    let right: usize = seam.rows.iter().map(|row| row.seam_right).sum();
    let wrong: usize = seam.rows.iter().map(|row| row.seam_wrong).sum();
    let rules: usize = seam.rows.iter().map(|row| row.rules_right).sum();
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "On the windows asked about: the seam named an owed act {right} time(s) and another \
         command {wrong} time(s); the rules did an owed act {rules} time(s)."
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{:<8}{:>6}{:>7}{:>7}   one floor for every action, the two other floors unchanged",
        "Floor", "acts", "right", "wrong"
    );
    for row in &seam.sweep {
        let _ = writeln!(
            out,
            "{:<8.2}{:>6}{:>7}{:>7}",
            row.floor, row.acts, row.right, row.wrong
        );
    }
}

/// A spread of margins, such as `+0.05/+0.21/+0.44`.
fn margins(spread: Option<Spread>) -> String {
    match spread {
        Some(spread) => format!("{:+.2}/{:+.2}/{:+.2}", spread.low, spread.mid, spread.high),
        None => "-".to_string(),
    }
}

/// A spread of probabilities, such as `0.62/0.78/0.93`.
fn probability(spread: Option<Spread>) -> String {
    match spread {
        Some(spread) => format!("{:.2}/{:.2}/{:.2}", spread.low, spread.mid, spread.high),
        None => "-".to_string(),
    }
}

/// A spread of round trips, such as `410/520/980 ms`.
fn millis(spread: Option<Spread>) -> String {
    match spread {
        Some(spread) => format!("{:.0}/{:.0}/{:.0} ms", spread.low, spread.mid, spread.high),
        None => "-".to_string(),
    }
}

/// A count of seconds, or a dash when nothing was measured.
fn seconds(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{value:.2} s"),
        None => "-".to_string(),
    }
}

/// A plain number, or a dash when nothing was measured.
fn number(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{value:.1}"),
        None => "-".to_string(),
    }
}

/// Every window the rules could not settle: where it is in the run, what
/// the rules called it, on what margin, what the desk did on that frame,
/// and whether the trigger asked about it or held it back. The listing
/// holds both, because a listing of the asks alone is the record the
/// seam's numbers were untrustworthy from.
#[must_use]
pub fn render_windows(replay: &Replay) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:>7}{:>6}{:<14}{:>9}{:>9}{:>9}{:>9}  rules",
        "row", "cue", " gesture", "phase", "pose", "margin", "sent"
    );
    let unsettled = replay
        .beats
        .iter()
        .filter(|beat| beat.ask.is_some() || beat.skipped.is_some());
    for beat in unsettled {
        let _ = writeln!(
            out,
            "{:>7}{:>6} {:<13}{:>9}{:>9}{:>9.2}{:>9}  {}",
            beat.row,
            beat.cue,
            beat.label,
            beat.phase.word(),
            beat.rule,
            beat.margin,
            match beat.skipped {
                Some(skip) => skip.word(),
                None => "asked",
            },
            if beat.acts.is_empty() {
                "-".to_string()
            } else {
                beat.acts.join(", ")
            }
        );
    }
    out
}
