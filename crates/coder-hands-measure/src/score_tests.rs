//! The scorer over a run in the repository, so its own numbers need no
//! camera and no key.
//!
//! One of them scores the synthetic run against the report checked in
//! beside it, byte for byte. A run is data and scoring is a pure
//! function of it, so that test is what says a Mac and a CoderOS host
//! report the same numbers for the same file.
//!
//! One call on that path is not specified to the last bit: the
//! smoothing in `coder_hands::gestures` reads `f32::exp`, and a
//! platform's own library chooses that bit. The reports match on both
//! machines today, and a margin that sat exactly on a printed rounding
//! boundary could in principle round the other way somewhere else. That
//! test is where the difference would show.

use super::*;
use crate::report;
use crate::run::{Cue, Header, Row};
use coder_hands::wire::Line;
use std::path::PathBuf;

/// The recorded runs the repository keeps.
fn runs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/golden/hands/runs")
}

/// The synthetic run and its answers, scored.
fn scored() -> (Run, Replay, Report) {
    let path = runs().join("synthetic-run.jsonl");
    let run = Run::read(&path).unwrap_or_else(|error| panic!("{error}"));
    let replay = Replay::of(&run);
    let answers = crate::answers::read(&crate::answers::beside(&path))
        .unwrap_or_else(|error| panic!("{error}"));
    let report = score("synthetic-run.jsonl", &run, &replay, &answers);
    (run, replay, report)
}

/// A row of the report, by label.
fn row<'a>(report: &'a Report, label: &str) -> &'a GestureRow {
    report
        .gestures
        .iter()
        .find(|row| row.label == label)
        .unwrap_or_else(|| panic!("{label} has a row"))
}

#[test]
fn the_synthetic_run_acts_on_every_gesture_it_asks_for() {
    let (_, _, report) = scored();
    assert_eq!(report.gestures.len(), 4);
    for label in ["point", "pinch", "swipe_left", "fist"] {
        let row = row(&report, label);
        assert_eq!(row.cues, 1, "{label} cues");
        assert_eq!(row.with_hand, 1, "{label} frames with a hand");
        assert_eq!(row.acted, 1, "{label} acted");
        assert_eq!(row.stray, 0, "{label} stray acts");
        assert!(row.first_act.is_some(), "{label} first act");
    }
    assert!(row(&report, "point").moves > 0);
    assert!(row(&report, "pinch").asks > 0, "the pinch cue asks");
}

#[test]
fn the_synthetic_run_scores_to_the_report_beside_it() {
    // The name in the first line is the one the README's command gives,
    // so the file is what that command prints from the repository root.
    let name = "bench/golden/hands/runs/synthetic-run.jsonl";
    let (run, replay, _) = scored();
    let answers =
        crate::answers::read(&crate::answers::beside(&runs().join("synthetic-run.jsonl")))
            .unwrap_or_else(|error| panic!("{error}"));
    let report = score(name, &run, &replay, &answers);
    let path = runs().join("synthetic-run.report.txt");
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let got = report::render(&report);
    assert_eq!(
        got,
        expected,
        "scoring {} no longer prints what {} holds. The run is data and the \
         score is a function of it, so either a constant moved, a floor \
         moved, or the report changed. Write the new text with \
         `coder-hands-measure score bench/golden/hands/runs/synthetic-run.jsonl` \
         in the commit that earns it.",
        name,
        path.display()
    );
}

#[test]
fn a_run_says_which_camera_it_came_from() {
    let (_, _, report) = scored();
    // The runs in the repository predate the field, and the report says
    // so rather than guessing at a camera.
    assert_eq!(report.source, None);
    assert!(
        report::render(&report).contains("Recorded from: a camera the run does not name."),
        "the report names the camera"
    );
}

#[test]
fn the_rate_says_what_a_window_of_frames_covers() {
    let (_, _, report) = scored();
    let rate = &report.rate;
    assert!(rate.frames > 200, "{} frames", rate.frames);
    assert!(rate.hand_frames < rate.frames, "the count-in has no hand");
    assert!((rate.fps - 30.0).abs() < 1.0, "{} a second", rate.fps);
    assert_eq!(rate.window_frames, 30, "one second at 30 a second");
    assert!(
        (rate.window_seconds - 1.0).abs() < 0.1,
        "{} seconds",
        rate.window_seconds
    );
    assert!(rate.windows > 0);
    assert!(rate.label_run_seconds.is_some());
    assert!(rate.labels_per_window.is_some());
}

#[test]
fn a_run_with_no_hand_decides_nothing_and_asks_nothing() {
    let cue = Cue {
        label: "point".into(),
        prompt: "Point at the screen.".into(),
        ready: 1.0,
        seconds: 5.0,
    };
    let rows = (0..90)
        .map(|frame| Row {
            cue: 0,
            label: "point".into(),
            phase: Phase::Record,
            line: Line {
                timestamp: 1_789_000_000.0 + f64::from(frame) / 15.0,
                hands: Vec::new(),
                status: "No hands detected".into(),
                dropped: 0,
            },
        })
        .collect();
    let run = Run {
        header: Header::new(1_789_000_000.0, None, None, 16.0 / 9.0, vec![cue]),
        rows,
    };
    let replay = Replay::of(&run);
    let report = score("empty-room", &run, &replay, &[]);
    assert_eq!(report.rate.hand_frames, 0);
    assert!((report.rate.fps - 15.0).abs() < 0.5, "{}", report.rate.fps);
    assert_eq!(report.rate.window_frames, 15, "one second at 15 a second");
    assert!(
        (report.rate.window_seconds - 1.0).abs() < 0.1,
        "{} seconds a window at 15 frames a second",
        report.rate.window_seconds
    );
    assert_eq!(report.rate.windows, 0);
    let row = row(&report, "point");
    assert_eq!(row.cues, 1);
    assert_eq!(row.with_hand, 0);
    assert_eq!(row.acted, 0);
    assert_eq!(row.moves, 0);
    assert_eq!(row.asks, 0);
    assert!(row.margins.is_none());
    assert!(report.seam.is_none());
    assert!(report::render(&report).contains("no answers beside this run"));
}

/// The pace section says how often the trigger asks, what it held back,
/// and what the same run would have asked at the unspaced trigger.
#[test]
fn the_pace_says_what_the_trigger_asked_and_what_it_held_back() {
    let (_, replay, report) = scored();
    let pace = &report.pace;
    assert_eq!(pace.asks, replay.asks());
    assert_eq!(pace.paced, replay.paced());
    assert!(pace.paced > 0, "this run has a window the trigger holds");
    assert!(pace.served <= pace.asks);
    assert_eq!(pace.sweep.len(), PACE_SWEEP.len());
    let before = &pace.sweep[0];
    let now = &pace.sweep[1];
    assert_eq!(before.spacing, 0, "the first row is the unspaced trigger");
    assert_eq!(before.paced, 0, "no spacing holds nothing back");
    assert!(
        before.asks > now.asks,
        "spacing asks less: {} against {}",
        before.asks,
        now.asks
    );
    assert_eq!(now.asks + now.paced, before.asks, "the same windows, paced");
    let text = report::render(&report);
    assert!(text.contains("The trigger held"), "the report says it");
    assert!(text.contains("spacing 0 is the unspaced trigger"));
}

/// The replay sizes its window from the run's own rate, the way the desk
/// sizes it from the camera, so a replayed window holds the span of hand
/// the desk would have held.
#[test]
fn the_replay_window_follows_the_runs_rate() {
    let (_, replay, report) = scored();
    assert_eq!(report.rate.window_frames, 30);
    let full = replay
        .beats
        .iter()
        .map(|beat| beat.window)
        .max()
        .expect("the run has beats");
    assert_eq!(
        full, 30,
        "the replay fills a 30-frame window at 30 a second"
    );
}

#[test]
fn scoring_a_run_twice_says_the_same_thing() {
    let (run, _, first) = scored();
    let again = score("synthetic-run.jsonl", &run, &Replay::of(&run), &{
        let path = runs().join("synthetic-run.jsonl");
        crate::answers::read(&crate::answers::beside(&path))
            .unwrap_or_else(|error| panic!("{error}"))
    });
    assert_eq!(first, again);
    assert_eq!(report::render(&first), report::render(&again));
}

#[test]
fn the_answers_beside_the_run_give_the_seam_its_numbers() {
    let (_, replay, report) = scored();
    let seam = report.seam.as_ref().expect("the run has answers");
    assert_eq!(seam.asked, replay.asks());
    assert!(seam.answered > 0, "{} answered", seam.answered);
    assert_eq!(seam.answered + seam.failed, seam.asked - seam.uncovered);
    assert!(seam.round_trip_ms.is_some());
    assert!(seam.confidence.is_some());
    let right: usize = seam.rows.iter().map(|row| row.seam_right).sum();
    let wrong: usize = seam.rows.iter().map(|row| row.seam_wrong).sum();
    assert!(right > 0, "an answer names an act its cue owes");
    assert!(wrong > 0, "an answer names another command");
    assert_eq!(seam.sweep.len(), SWEEP.len());
    for pair in seam.sweep.windows(2) {
        assert!(
            pair[0].acts >= pair[1].acts,
            "a higher floor acts no more often: {:?}",
            seam.sweep
        );
    }
    let text = report::render(&report);
    assert!(text.contains("Seam: "), "{text}");
    assert!(text.contains("rules right"), "{text}");
    assert!(text.contains("0.60"), "{text}");
}

#[test]
fn every_recorded_run_replays_and_scores() {
    let dir = runs();
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .filter(|path| !path.to_string_lossy().ends_with(".answers.jsonl"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no run under {}", dir.display());
    for path in &names {
        let run = Run::read(path).unwrap_or_else(|error| panic!("{error}"));
        let replay = Replay::of(&run);
        let answers = crate::answers::read(&crate::answers::beside(path))
            .unwrap_or_else(|error| panic!("{error}"));
        let name = path.display().to_string();
        let report = score(&name, &run, &replay, &answers);
        assert_eq!(report.rate.frames, run.rows.len(), "{name}");
        assert!(!report.gestures.is_empty(), "{name}");
        assert!(!report::render(&report).is_empty(), "{name}");
    }
}

#[test]
fn the_recorded_empty_room_asks_for_nothing() {
    let path = runs().join("empty-room-recorded.jsonl");
    let run = Run::read(&path).unwrap_or_else(|error| panic!("{error}"));
    let replay = Replay::of(&run);
    let report = score("empty-room-recorded.jsonl", &run, &replay, &[]);
    assert_eq!(report.rate.hand_frames, 0, "nobody was in the room");
    assert!(report.rate.frames > 300, "{} frames", report.rate.frames);
    assert!(
        (report.rate.fps - 15.0).abs() < 1.5,
        "{} frames a second",
        report.rate.fps
    );
    assert!(
        (report.rate.window_seconds - 1.0).abs() < 0.15,
        "the window is one second at any rate, and this one is {:.2} s",
        report.rate.window_seconds
    );
    assert_eq!(replay.asks(), 0);
    for row in &report.gestures {
        assert_eq!(row.acted, 0, "{}", row.label);
        assert_eq!(row.moves, 0, "{}", row.label);
        assert_eq!(row.stray, 0, "{}", row.label);
    }
    assert_eq!(report.rest.stray, 0);
}
