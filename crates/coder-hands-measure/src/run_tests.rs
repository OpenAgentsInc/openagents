//! The run format reads back what it wrote, and the gestures it asks for
//! owe acts the rules can produce.

use super::*;
use coder_hands::gestures::Act;
use coder_hands::wire::Line;
use coder_wm::Dir;

fn line(t: f64) -> Line {
    Line {
        timestamp: t,
        hands: Vec::new(),
        status: "No hands detected".into(),
        dropped: 0,
    }
}

#[test]
fn a_header_and_its_rows_read_back() {
    let cues = script(1);
    let header = Header::new(
        1_789_000_000.0,
        Some(Source::Daemon),
        Some("1280x720".into()),
        16.0 / 9.0,
        cues,
    );
    let rows = vec![
        Row {
            cue: 0,
            label: "point".into(),
            phase: Phase::Ready,
            line: line(1.0),
        },
        Row {
            cue: 0,
            label: "point".into(),
            phase: Phase::Record,
            line: line(2.0),
        },
    ];
    let written = Run {
        header: header.clone(),
        rows: rows.clone(),
    };
    let read = Run::parse(&written.render()).expect("the run reads back");
    assert_eq!(read.header, header);
    assert_eq!(read.rows, rows);
    assert_eq!(read.rows[0].phase.word(), "ready");
    assert_eq!(read.rows[1].phase.word(), "record");
}

#[test]
fn an_empty_run_and_a_broken_row_are_refused_by_name() {
    assert_eq!(Run::parse("  \n"), Err("the run is empty".to_string()));
    let header = Header::new(0.0, None, None, 1.0, script(1)).render();
    let error = Run::parse(&format!("{header}{{\n")).unwrap_err();
    assert!(error.starts_with("line 2: "), "{error}");
}

#[test]
fn a_file_of_another_kind_or_version_says_which() {
    let mut header = Header::new(0.0, None, None, 1.0, Vec::new());
    header.kind = "trajectory".into();
    let error = Run::parse(&header.render()).unwrap_err();
    assert!(error.contains("not a hands-run file"), "{error}");
    let mut header = Header::new(0.0, None, None, 1.0, Vec::new());
    header.version = VERSION + 7;
    let error = Run::parse(&header.render()).unwrap_err();
    assert!(error.contains("knows version"), "{error}");
}

#[test]
fn the_camera_a_run_was_recorded_from_survives_the_file() {
    for source in [Source::Daemon, Source::Vision] {
        let header = Header::new(0.0, Some(source), None, 1.0, script(1));
        let read = Run::parse(&header.render()).expect("the header reads back");
        assert_eq!(read.header.source, Some(source));
    }
    let daemon = Header::new(0.0, Some(Source::Daemon), None, 1.0, Vec::new()).render();
    assert!(daemon.contains("\"source\":\"daemon\""), "{daemon}");
    let vision = Header::new(0.0, Some(Source::Vision), None, 1.0, Vec::new()).render();
    assert!(vision.contains("\"source\":\"vision\""), "{vision}");
    // A run written before the field existed carries none, and reads.
    let older = Header::new(0.0, None, None, 1.0, Vec::new()).render();
    assert!(!older.contains("source"), "{older}");
    assert_eq!(
        Run::parse(&older)
            .expect("the older header reads")
            .header
            .source,
        None
    );
}

#[test]
fn a_pass_asks_for_every_gesture_once() {
    let one = script(1);
    assert_eq!(one.len(), GESTURES.len());
    let two = script(2);
    assert_eq!(two.len(), GESTURES.len() * 2);
    assert_eq!(two[0].label, GESTURES[0].label);
    assert_eq!(two[GESTURES.len()].label, GESTURES[0].label);
    let seconds = script_seconds(&two);
    assert!(seconds > 30.0 && seconds < 120.0, "{seconds} seconds");
}

#[test]
fn every_gesture_owes_acts_the_rules_can_produce() {
    let words = [
        Act::Point(0.0, 0.0).word(),
        Act::Press(0.0, 0.0).word(),
        Act::Drag(0.0, 0.0).word(),
        Act::Release.word(),
        Act::Swipe(Dir::Left).word(),
        Act::Swipe(Dir::Right).word(),
        Act::Escape.word(),
    ];
    for gesture in GESTURES {
        assert!(!gesture.owed.is_empty(), "{} owes nothing", gesture.label);
        for owed in gesture.owed {
            assert!(words.contains(owed), "{} owes {owed}", gesture.label);
        }
        assert!(
            gesture.prompt.ends_with('.'),
            "{} reads as a sentence",
            gesture.label
        );
    }
    for command in COMMANDS {
        assert!(words.contains(command), "{command} is no act");
    }
    assert!(gesture("point").is_some());
    assert!(gesture("shrug").is_none());
}

/// A file of raw landmark lines, the shape the camera daemon publishes,
/// reads as one unlabelled run rather than being refused for want of a
/// header.
#[test]
fn a_file_of_landmark_lines_reads_as_one_unlabelled_run() {
    let text = (0..5)
        .map(|frame| {
            let mut row = line(1_789_000_000.0 + f64::from(frame) / 15.0);
            row.status = "1 hand(s) detected".into();
            row.hands = vec![coder_hands::wire::WireHand {
                landmarks: vec![
                    coder_hands::wire::Point {
                        x: 0.4 + frame as f32 * 0.01,
                        y: 0.5,
                        z: 1.0,
                    };
                    21
                ],
                pose: "none".into(),
            }];
            serde_json::to_string(&row).expect("a line renders")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let run = Run::parse(&text).expect("landmark lines read");
    assert_eq!(run.header.script.len(), 1);
    assert_eq!(run.header.script[0].label, UNLABELLED);
    assert_eq!(run.rows.len(), 5);
    assert!(
        run.rows
            .iter()
            .all(|row| row.label == UNLABELLED && row.phase == Phase::Record)
    );
    assert!(
        gesture(UNLABELLED).is_none(),
        "an unlabelled run owes no act"
    );
}

/// A recorder that polls faster than the camera answers writes the same
/// reading many times over. Reading the rate off those timestamps gives
/// the poll loop's rate, so the repeats are dropped.
#[test]
fn repeated_readings_are_dropped_from_a_raw_file() {
    let mut text = String::new();
    for frame in 0..30 {
        let mut row = line(1_789_000_000.0 + f64::from(frame) / 1_000.0);
        row.status = "Tracking active".into();
        text.push_str(&serde_json::to_string(&row).expect("a line renders"));
        text.push('\n');
    }
    let run = Run::parse(&text).expect("landmark lines read");
    assert_eq!(
        run.rows.len(),
        1,
        "30 copies of one reading are one reading"
    );
}

/// A file that is neither a run header nor landmark lines says so rather
/// than reading as an empty run.
#[test]
fn a_file_that_is_neither_says_so() {
    let error = Run::parse("{\"hello\":\"world\"}\n").expect_err("refused");
    assert!(
        error.contains("neither a hands-run header nor a landmark line"),
        "{error}"
    );
}
