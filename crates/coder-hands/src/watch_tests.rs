use super::*;

use crate::Landmark;
use crate::judge::Report;
use crate::wire::{Point, WireHand};

use crate::gestures::Label;

/// One answer over a window the rules read as a pinch on a thin margin,
/// with the hand aimed at the middle of the screen.
fn answer(report: Result<Report, String>) -> Answer {
    Answer {
        meta: Meta {
            window: 30,
            pose: "pinch".to_string(),
            margin: Some(0.04),
            rules: vec!["point".to_string()],
            at: Some((0.25, 0.75)),
        },
        report,
        met_deadline: true,
        elapsed: std::time::Duration::from_millis(120),
    }
}

/// A press answered above every floor.
fn press() -> Report {
    Report {
        intent: "press".to_string(),
        confidence: 0.86,
        probabilities: vec![("press".to_string(), 0.86), ("point".to_string(), 0.14)],
        addressed: 0.91,
        continuing: 0.12,
        usage: Default::default(),
        request_id: Some("req-test".to_string()),
    }
}

#[test]
fn the_seam_off_asks_nothing_and_applies_nothing() {
    let mut watch = Watch::off();
    assert!(!watch.asks());
    let line = Line {
        timestamp: 1.0,
        hands: Vec::new(),
        status: "No hands detected".to_string(),
        dropped: 0,
    };
    watch.feed(&line, &Step::default(), Some((0.5, 0.5)));
    watch.acted(&[Act::Release]);
    assert!(watch.take().is_empty());
    assert_eq!(watch.counts(), Counts::default());
}

#[test]
fn a_record_carries_the_answers_the_rules_and_the_deadline() {
    let (record, act) = judged(&Judge::new(Mode::Shadow), &answer(Ok(press())));
    assert!(act.is_none(), "shadow applies nothing");
    assert_eq!(record.action, "press");
    assert_eq!(record.rules, vec!["point".to_string()]);
    assert_eq!(record.window, 30);
    assert!(record.met_deadline);
    assert_eq!(record.deadline_ms, 1_000);
    assert_eq!(record.failed, None);
    let line = transcript(&record);
    let value: serde_json::Value =
        serde_json::from_str(&line).expect("the record is one JSON line");
    assert_eq!(value["report"]["intent"], "press");
    assert_eq!(value["report"]["probabilities"][0][0], "press");
    assert_eq!(value["report"]["addressed"], 0.91);
    assert_eq!(value["rules"][0], "point");
    assert!(!line.contains('\n'), "one line: {line}");
}

#[test]
fn only_the_act_mode_applies_and_it_acts_where_the_window_pointed() {
    let (_, act) = judged(&Judge::new(Mode::Act), &answer(Ok(press())));
    assert_eq!(act, Some(Act::Press(0.25, 0.75)));
    let (_, suggested) = judged(&Judge::new(Mode::Suggest), &answer(Ok(press())));
    assert!(suggested.is_none());
    let mut blind = answer(Ok(press()));
    blind.meta.at = None;
    let (_, nowhere) = judged(&Judge::new(Mode::Act), &blind);
    assert!(nowhere.is_none(), "a press with no point presses nothing");
}

#[test]
fn an_answer_under_its_floor_is_recorded_as_a_drop() {
    let mut unsure = press();
    unsure.confidence = 0.62;
    let (record, act) = judged(&Judge::new(Mode::Act), &answer(Ok(unsure)));
    assert_eq!(record.action, "drop");
    assert!(act.is_none());
}

#[test]
fn a_request_that_failed_is_recorded_with_its_sentence() {
    let (record, act) = judged(
        &Judge::new(Mode::Shadow),
        &answer(Err("the API answered 503".to_string())),
    );
    assert!(record.report.is_none());
    assert_eq!(record.action, "drop");
    assert_eq!(record.failed.as_deref(), Some("the API answered 503"));
    assert!(act.is_none());
    assert!(transcript(&record).contains("503"));
}

#[test]
fn the_window_reads_the_hand_the_rules_read_and_counts_the_other() {
    let wire = |x: f32| WireHand {
        landmarks: (0..21)
            .map(|joint| Point {
                x: x + joint as f32 * 0.001,
                y: 0.5,
                z: 0.5,
            })
            .collect(),
        pose: "none".to_string(),
    };
    let line = Line {
        timestamp: 2.0,
        hands: vec![wire(0.2), wire(0.7)],
        status: "2 hand(s) detected".to_string(),
        dropped: 0,
    };
    let mut step = Step::default();
    let mut smoothed = [Landmark::default(); 21];
    smoothed[0] = Landmark {
        x: 0.42,
        y: 0.43,
        z: 0.44,
    };
    step.hand = Some(smoothed);
    step.label = Label::Pinch;
    let frame = frame_of(&line, &step);
    assert_eq!(frame.hands.len(), 2, "the second hand is still counted");
    assert_eq!(
        frame.hands[0].landmarks[0].x, 0.42,
        "the rules' hand is first"
    );
}

/// A skipped window reaches the transcript as its own line, so the
/// record says what the seam did not ask rather than counting it.
#[test]
fn a_skipped_window_is_one_line_of_the_transcript() {
    let meta = Meta {
        window: 30,
        pose: "pinch".to_string(),
        margin: Some(0.04),
        rules: vec!["point".to_string()],
        at: Some((0.25, 0.75)),
    };
    let line = missed(&meta, Skip::Paced);
    let value: serde_json::Value = serde_json::from_str(&line).expect("one JSON line");
    assert_eq!(value["skipped"], "paced");
    assert_eq!(value["pose"], "pinch");
    assert_eq!(value["window"], 30);
    assert_eq!(value["rules"][0], "point");
    assert!(!line.contains('\n'), "one line: {line}");
    assert!(missed(&meta, Skip::InFlight).contains("in flight"));
}

#[test]
fn every_act_the_desk_takes_names_a_gesture_the_window_keeps() {
    assert_eq!(gesture_of(&Act::Point(0.1, 0.2)), Some("point"));
    assert_eq!(gesture_of(&Act::Press(0.1, 0.2)), Some("press"));
    assert_eq!(gesture_of(&Act::Drag(0.1, 0.2)), Some("press"));
    assert_eq!(gesture_of(&Act::Release), Some("release"));
    assert_eq!(gesture_of(&Act::Swipe(Dir::Left)), Some("swipe_left"));
    assert_eq!(gesture_of(&Act::Swipe(Dir::Right)), Some("swipe_right"));
    assert_eq!(gesture_of(&Act::Escape), Some("escape"));
    assert_eq!(gesture_of(&Act::Swipe(Dir::Up)), None);
}

#[test]
fn a_mode_the_variable_does_not_name_leaves_the_rules_alone() {
    assert_eq!(MODE_VAR, "CODEROS_HANDS_JUDGE");
    assert_eq!(Mode::named("shadow"), Some(Mode::Shadow));
    assert_eq!(Mode::named("sometimes"), None);
    let watch = Watch::start(Mode::Off);
    assert!(!watch.asks());
    assert_eq!(watch.counts(), Counts::default());
}

/// The rate the camera answers at sizes the window, because the window
/// is a second of hand rather than a count of frames and the rate is not
/// one number across machines.
#[test]
fn the_window_follows_the_rate_the_frames_arrive_at() {
    let mut rate = Rate::default();
    let mut at = 1_789_000_000.0;
    let mut last = None;
    for _ in 0..40 {
        at += 1.0 / 30.0;
        last = rate.push(at).or(last);
    }
    assert_eq!(last, Some(30), "one second at 30 a second");

    let mut rate = Rate::default();
    let mut at = 1_789_000_000.0;
    let mut last = None;
    for _ in 0..40 {
        at += 1.0 / 15.0;
        last = rate.push(at).or(last);
    }
    assert_eq!(last, Some(15), "one second at 15 a second");
}

/// A dropout is not a frame, so the gap across it does not drag the
/// rate down. The recordings of 2026-09-18 lost the hand 21 times in
/// half a minute, three of them for over a second.
#[test]
fn a_dropout_does_not_set_the_rate() {
    let mut rate = Rate::default();
    let mut at = 1_789_000_000.0;
    let mut last = None;
    for frame in 0..40 {
        at += if frame == 12 { 9.0 } else { 1.0 / 30.0 };
        last = rate.push(at).or(last);
    }
    assert_eq!(last, Some(30), "the nine-second gap is not a frame");
}
