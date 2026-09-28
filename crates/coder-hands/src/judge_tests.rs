use super::*;

use crate::{HandPose, INDEX_PIP, INDEX_TIP, Landmark, MIDDLE_PIP};
use crate::{PINKY_PIP, RING_PIP, THUMB_IP};

/// A right hand facing the camera: wrist low, knuckles above it, index
/// finger extended and pointing up, the other fingers curled, and the
/// thumb tip held `pinch` image units off the index tip. The margins this
/// layout carries are wide everywhere except the pinch rule, whose margin
/// `pinch` moves.
fn pointing_hand(pinch: f32) -> [Landmark; 21] {
    let mut landmarks = [Landmark::default(); 21];
    let put = |landmarks: &mut [Landmark; 21], joint: usize, x: f32, y: f32| {
        landmarks[joint] = Landmark { x, y, z: 0.0 };
    };
    put(&mut landmarks, WRIST, 0.50, 0.85);
    put(&mut landmarks, MIDDLE_MCP, 0.50, 0.60);
    put(&mut landmarks, INDEX_MCP, 0.46, 0.60);
    put(&mut landmarks, INDEX_PIP, 0.46, 0.50);
    put(&mut landmarks, INDEX_TIP, 0.46, 0.38);
    for (at, (mcp, pip, tip)) in [
        (0.50, (MIDDLE_MCP, MIDDLE_PIP, MIDDLE_TIP)),
        (0.54, (RING_MCP, RING_PIP, RING_TIP)),
        (0.57, (PINKY_MCP, PINKY_PIP, PINKY_TIP)),
    ] {
        put(&mut landmarks, mcp, at, 0.60);
        put(&mut landmarks, pip, at, 0.62);
        put(&mut landmarks, tip, at, 0.72);
    }
    put(&mut landmarks, THUMB_MCP, 0.44, 0.68);
    put(&mut landmarks, THUMB_IP, 0.42, 0.64);
    put(&mut landmarks, THUMB_TIP, 0.46 + pinch, 0.38);
    landmarks
}

/// A frame holding one [`pointing_hand`] labeled `pose`.
fn frame(landmarks: [Landmark; 21], pose: HandPose) -> Frame {
    Frame {
        hands: vec![Hand { landmarks, pose }],
        status: String::new(),
    }
}

/// The seam's fixture window: eight frames of a hand that points and
/// then hovers on the pinch bound, the shape the seam exists for.
fn seam_window() -> Window {
    let mut window = Window::new();
    for at in 0..8 {
        let mut landmarks = pointing_hand(0.20);
        landmarks[INDEX_TIP].x = 0.42 + at as f32 * 0.01;
        window.push(&frame(landmarks, HandPose::FlatHand));
    }
    for _ in 0..3 {
        window.push(&frame(pointing_hand(0.09), HandPose::PinchClosed));
    }
    window.push(&frame(pointing_hand(0.09), HandPose::None));
    window
}

#[test]
fn a_frame_becomes_palm_relative_features() {
    let mut window = Window::new();
    window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    let state = window.state();
    let frames = state["frames"].as_array().expect("frames is an array");
    assert_eq!(frames.len(), 1);
    let first = &frames[0];
    assert_eq!(first["pose"], "flat hand");
    // The palm is the one the pinch rule divides by, the wrist to the
    // index knuckle, which this hand holds 0.253 image units apart. A
    // gap of 0.30 is 1.185 of it, and `pose::pinch_ratio` answers the
    // same number, which is what makes the state's `pinch` and
    // `PINCH_PALM` comparable.
    let pinch = first["pinch"].as_f64().expect("pinch") as f32;
    assert!((pinch - 1.185).abs() < 0.01, "pinch {pinch}");
    let ratio = pose::pinch_ratio(&pointing_hand(0.30)).expect("21 joints have a palm");
    assert!((pinch - ratio).abs() < 1e-6, "pinch {pinch}, rule {ratio}");
    let extension = first["extension"].as_array().expect("extension");
    assert_eq!(extension.len(), 5);
    assert_eq!(first["tip_velocity"], json!([0.0, 0.0]));
    assert!((first["x"].as_f64().expect("x") - 0.46).abs() < 1e-6);
    assert_eq!(state["second_hand"], false);
    assert_eq!(state["previous"], Value::Null);
}

#[test]
fn velocity_is_the_tip_delta_between_beats() {
    let mut window = Window::new();
    window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    let mut second = pointing_hand(0.30);
    second[INDEX_TIP].x += 0.05;
    window.push(&frame(second, HandPose::FlatHand));
    let state = window.state();
    let velocity = &state["frames"][1]["tip_velocity"];
    assert!((velocity[0].as_f64().expect("dx") - 0.05).abs() < 1e-6);
}

#[test]
fn acted_marks_the_previous_gesture_and_push_ages_it() {
    let mut window = Window::new();
    window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    window.acted("press");
    window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    let state = window.state();
    assert_eq!(state["previous"]["gesture"], "press");
    assert_eq!(state["previous"]["ended_frames"], 2);
}

#[test]
fn a_frame_with_no_hands_ages_but_appends_nothing() {
    let mut window = Window::new();
    window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    window.acted("point");
    window.push(&Frame::default());
    assert_eq!(
        window.state()["frames"].as_array().expect("frames").len(),
        1
    );
    assert_eq!(window.state()["previous"]["ended_frames"], 1);
}

#[test]
fn a_wide_margin_transition_is_clear() {
    let mut window = Window::new();
    window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    window.push(&frame(pointing_hand(0.03), HandPose::PinchClosed));
    assert!(window.transitioned());
    assert!(!window.ambiguous());
}

#[test]
fn a_thin_margin_transition_is_ambiguous() {
    let mut window = Window::new();
    window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    window.push(&frame(pointing_hand(0.09), HandPose::PinchClosed));
    assert!(window.ambiguous());
}

#[test]
fn a_flickering_window_is_ambiguous() {
    let mut window = Window::new();
    for pose in [
        HandPose::FlatHand,
        HandPose::PinchClosed,
        HandPose::FlatHand,
        HandPose::PinchClosed,
    ] {
        window.push(&frame(pointing_hand(0.30), pose));
    }
    assert_eq!(window.flips(), 3);
    assert!(window.ambiguous());
}

/// A flickering hand makes every transition ambiguous, and the trigger
/// asks about one window of it rather than all of them. Over the owner's
/// recordings of 2026-09-18 that is the difference between 1.1 to 3.4
/// asks a second and 0.2 to 0.6.
#[test]
fn a_flickering_window_asks_once_and_paces_the_rest() {
    let mut window = Window::new();
    let held = window.capacity();
    let mut asks = 0;
    let mut paced = 0;
    for at in 0..held * 3 {
        let pose = if at % 2 == 0 {
            HandPose::FlatHand
        } else {
            HandPose::PinchClosed
        };
        window.push(&frame(pointing_hand(0.30), pose));
        match window.ask(Trigger::default()) {
            Ask::Now => asks += 1,
            Ask::Paced => paced += 1,
            Ask::Settled => {}
        }
    }
    assert_eq!(asks, 3, "one ask a window of hand");
    // A window needs three frames before its labels have flipped twice,
    // so the first two carry no ambiguous transition.
    assert_eq!(asks + paced, held * 3 - 2, "every other transition is held");
}

/// A trigger with no spacing is what the seam ran before the trigger was spaced: every
/// ambiguous transition asks, and nothing is held back.
#[test]
fn no_spacing_asks_on_every_ambiguous_transition() {
    let trigger = Trigger {
        spacing: 0,
        ..Trigger::default()
    };
    let mut window = Window::new();
    let mut asks = 0;
    for at in 0..10 {
        let pose = if at % 2 == 0 {
            HandPose::FlatHand
        } else {
            HandPose::PinchClosed
        };
        window.push(&frame(pointing_hand(0.30), pose));
        if window.ask(trigger) == Ask::Now {
            asks += 1;
        }
    }
    assert_eq!(asks, 8, "every ambiguous transition asks");
}

/// A window the rules settle asks nothing and records nothing, whatever
/// the spacing says.
#[test]
fn a_settled_transition_is_neither_asked_nor_recorded() {
    let mut window = Window::new();
    window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    window.push(&frame(pointing_hand(0.03), HandPose::PinchClosed));
    assert_eq!(window.ask(Trigger::default()), Ask::Settled);
}

/// A skipped window carries the same fields a [`Record`] carries, and a
/// reader tells the two apart by `skipped`.
#[test]
fn a_skipped_window_names_the_window_and_the_reason() {
    let missed = Missed {
        window: window_frames(NOMINAL_RATE),
        pose: "pinch".to_string(),
        margin: Some(0.04),
        rules: vec!["point".to_string()],
        skipped: Skip::Paced.word(),
    };
    let value = serde_json::to_value(&missed).expect("the record serializes");
    assert_eq!(value["skipped"], "paced");
    assert_eq!(value["pose"], "pinch");
    assert_eq!(value["rules"], json!(["point"]));
    assert_eq!(Skip::InFlight.word(), "in flight");
    assert_eq!(Skip::Closed.word(), "closed");
}

/// The counts a session reports add up: every ambiguous window is either
/// asked about or named as skipped.
#[test]
fn the_counts_hold_every_ambiguous_window() {
    let mut counts = Counts {
        asked: 4,
        ..Counts::default()
    };
    counts.skip(Skip::Paced);
    counts.skip(Skip::Paced);
    counts.skip(Skip::InFlight);
    counts.skip(Skip::Closed);
    assert_eq!(counts.paced, 2);
    assert_eq!(counts.in_flight, 1);
    assert_eq!(counts.closed, 1);
    assert_eq!(counts.skipped(), 4);
    assert_eq!(counts.ambiguous(), 8);
}

#[test]
fn a_steady_window_never_asks() {
    let mut window = Window::new();
    for _ in 0..5 {
        window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    }
    assert!(!window.transitioned());
    assert!(!window.ambiguous());
}

#[test]
fn the_window_holds_one_second() {
    let mut window = Window::new();
    let held = window.capacity();
    for _ in 0..held + 10 {
        window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    }
    assert_eq!(
        window.state()["frames"].as_array().expect("frames").len(),
        held
    );
}

/// The window is one second of hand at whatever rate the camera
/// answers, which is why it is sized in time rather than in frames.
#[test]
fn the_window_follows_the_camera_rate() {
    assert_eq!(WINDOW_FRAMES_MIN, MIN_COUNTED as usize, "one minimum");
    assert_eq!(window_frames(15.0), 15);
    assert_eq!(window_frames(30.0), 30);
    assert_eq!(window_frames(4.0), WINDOW_FRAMES_MIN);
    assert_eq!(window_frames(120.0), WINDOW_FRAMES_MAX);
    assert_eq!(window_frames(f64::NAN), window_frames(NOMINAL_RATE));
    assert_eq!(window_frames(0.0), window_frames(NOMINAL_RATE));
    let mut window = Window::at_rate(30.0);
    for _ in 0..40 {
        window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    }
    assert_eq!(window.len(), 30);
    window.set_rate(15.0);
    assert_eq!(window.len(), 15);
}

#[test]
fn questions_carry_the_three_judgments() {
    let questions = questions();
    assert_eq!(questions.len(), 3);
    let Some(jev::Question::Choice(choice)) = questions.get(INTENT) else {
        panic!("intent is a Choice");
    };
    for option in [
        "point",
        "press",
        "release",
        "swipe_left",
        "swipe_right",
        "escape",
        "rest",
        "none",
    ] {
        assert!(choice.criteria.contains_key(option), "missing {option}");
    }
    assert!(matches!(
        questions.get(ADDRESSED),
        Some(jev::Question::Noul(_))
    ));
    assert!(matches!(
        questions.get(CONTINUING),
        Some(jev::Question::Noul(_))
    ));
    questions.validate().expect("the question set validates");
}

fn report(intent: &str, confidence: f64, addressed: f64, continuing: f64) -> Report {
    Report {
        intent: intent.to_string(),
        confidence,
        probabilities: Vec::new(),
        addressed,
        continuing,
        usage: jev::Usage::default(),
        request_id: None,
    }
}

#[test]
fn addressed_below_its_floor_drops_whatever_intent_says() {
    assert_eq!(verdict(&report("press", 0.95, 0.30, 0.0)), Action::Drop);
}

#[test]
fn continuing_above_its_floor_holds_the_running_gesture() {
    assert_eq!(verdict(&report("press", 0.90, 0.90, 0.80)), Action::Hold);
}

#[test]
fn press_and_escape_carry_higher_floors() {
    assert_eq!(verdict(&report("press", 0.70, 0.90, 0.10)), Action::Drop);
    assert_eq!(verdict(&report("press", 0.80, 0.90, 0.10)), Action::Press);
    assert_eq!(verdict(&report("escape", 0.70, 0.90, 0.10)), Action::Drop);
    assert_eq!(verdict(&report("escape", 0.80, 0.90, 0.10)), Action::Escape);
}

#[test]
fn point_acts_at_the_base_floor() {
    assert_eq!(verdict(&report("point", 0.65, 0.90, 0.10)), Action::Point);
    assert_eq!(verdict(&report("point", 0.55, 0.90, 0.10)), Action::Drop);
}

#[test]
fn none_and_an_unknown_intent_drop() {
    assert_eq!(verdict(&report("none", 0.99, 0.90, 0.10)), Action::Drop);
    assert_eq!(verdict(&report("shrug", 0.99, 0.90, 0.10)), Action::Drop);
}

#[test]
fn shadow_records_and_never_applies() {
    let judge = Judge::new(Mode::Shadow);
    let decision = judge.decide(&report("press", 0.90, 0.90, 0.10), true);
    assert_eq!(decision.action, Action::Press);
    assert!(!decision.apply);
    assert!(decision.hint.is_none());
    assert!(judge.asks());
}

#[test]
fn suggest_hints_and_never_applies() {
    let judge = Judge::new(Mode::Suggest);
    let decision = judge.decide(&report("press", 0.90, 0.90, 0.10), true);
    assert!(!decision.apply);
    assert_eq!(decision.hint.as_deref(), Some("press"));
}

/// An answer that missed the deadline is recorded and never acted on:
/// the desk has moved on by the time it lands, while the measurement
/// still counts it against what the hand meant.
#[test]
fn a_late_answer_records_and_never_applies() {
    let judge = Judge::new(Mode::Act);
    let decision = judge.decide(&report("press", 0.90, 0.90, 0.10), false);
    assert_eq!(decision.action, Action::Press);
    assert!(!decision.apply);
}

#[test]
fn act_applies_above_the_floor_and_only_there() {
    let judge = Judge::new(Mode::Act);
    assert!(judge.decide(&report("press", 0.80, 0.90, 0.10), true).apply);
    assert!(!judge.decide(&report("press", 0.70, 0.90, 0.10), true).apply);
    assert!(!judge.decide(&report("point", 0.90, 0.90, 0.80), true).apply);
    assert!(!judge.decide(&report("rest", 0.90, 0.90, 0.10), true).apply);
}

#[test]
fn off_asks_nothing_and_says_rules_only() {
    let judge = Judge::new(Mode::Off);
    assert!(!judge.asks());
    assert_eq!(judge.status(), RULES_ONLY);
    assert_eq!(judge.status(), "hands: rules only");
}

#[test]
fn the_record_holds_answers_deadline_and_action() {
    let judge = Judge::new(Mode::Shadow);
    let report = report("press", 0.80, 0.92, 0.15);
    let decision = judge.decide(&report, true);
    let record = Record {
        window: window_frames(NOMINAL_RATE),
        pose: "pinch".to_string(),
        margin: Some(0.1),
        deadline_ms: DEADLINE.as_millis() as u64,
        met_deadline: true,
        report: Some(report.view()),
        rules: vec!["point".to_string()],
        action: decision.action.label().to_string(),
        failed: None,
    };
    let value = serde_json::to_value(&record).expect("the record serializes");
    assert_eq!(value["report"]["intent"], "press");
    assert_eq!(value["report"]["addressed"], 0.92);
    assert_eq!(value["met_deadline"], true);
    assert_eq!(value["action"], "press");
    assert_eq!(value["rules"], json!(["point"]));
    assert_eq!(value.get("failed"), None);
}

#[test]
fn a_word_names_a_rung_of_the_ladder_and_an_unread_one_names_none() {
    assert_eq!(Mode::named("shadow"), Some(Mode::Shadow));
    assert_eq!(Mode::named(" ON "), Some(Mode::Shadow));
    assert_eq!(Mode::named("off"), Some(Mode::Off));
    assert_eq!(Mode::named(""), Some(Mode::Off));
    assert_eq!(Mode::named("suggest"), Some(Mode::Suggest));
    assert_eq!(Mode::named("act"), Some(Mode::Act));
    assert_eq!(Mode::named("maybe"), None);
    assert_eq!(Mode::Shadow.word(), "shadow");
    assert_eq!(Mode::named(Mode::Act.word()), Some(Mode::Act));
}

#[test]
fn a_cleared_window_holds_no_frame_and_no_previous_gesture() {
    let mut window = Window::new();
    window.push(&frame(pointing_hand(0.30), HandPose::FlatHand));
    window.acted("press");
    assert_eq!(window.len(), 1);
    assert!(!window.is_empty());
    window.clear();
    assert!(window.is_empty());
    assert_eq!(window.state()["previous"], Value::Null);
}

/// The recorded case for the `hands` seam: the request a borderline-pinch
/// window sends, and fixed answers to it.
fn case_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/jev-hands-seam.json")
}

/// Serves one System One request on loopback: answers the case's
/// response when the request body matches the case's request, and a 404
/// otherwise. Answers the port and the thread, whose value is whether the
/// body matched.
fn replay(case: Value) -> (u16, std::thread::JoinHandle<bool>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("the address").port();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the client connects");
        let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
        let mut length = 0usize;
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).expect("a header line");
            let header = line.trim_end();
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().expect("a length");
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).expect("the body");
        let sent: Value = serde_json::from_slice(&body).expect("a JSON body");
        let matched = sent == case["request"];
        let (status, answer) = if matched {
            ("200 OK", case["response"].to_string())
        } else {
            (
                "404 Not Found",
                serde_json::json!({"error": "no case"}).to_string(),
            )
        };
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\nx-request-id: {}\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{answer}",
            case["request_id"].as_str().unwrap_or("req"),
            answer.len()
        )
        .expect("the answer");
        matched
    });
    (port, server)
}

/// The `hands` seam reads a request's answers back into a [`Report`].
#[tokio::test]
async fn ask_replays_the_hands_seam() {
    let case: Value =
        serde_json::from_str(&std::fs::read_to_string(case_path()).expect("the case"))
            .expect("the case parses");
    let (port, server) = replay(case);
    let config = jev::Config::new()
        .api_key("test-no-key")
        .base_url(format!("http://127.0.0.1:{port}"))
        .retry(jev::RetryPolicy {
            max_retries: 0,
            ..jev::RetryPolicy::default()
        });
    let client = jev::Client::new(config).expect("client");
    let (state, findings) = sendable(seam_window().state());
    assert_eq!(findings, 0);
    let report = ask(&client, "jev-latest", state, questions()).await;
    assert!(
        server.join().expect("the server"),
        "the request matched the recorded case"
    );
    let report = report.expect("the seam answers");
    assert_eq!(report.intent, "press");
    assert!(report.addressed > 0.5);
    assert!(report.continuing < 0.5);
    assert_eq!(verdict(&report), Action::Press);
}

/// The scan withholds a string that is not a short word, and leaves the
/// numbers and the labels alone.
#[test]
fn the_scan_withholds_anything_but_a_word() {
    let (state, findings) = sendable(serde_json::json!({
        "frames": [{ "pose": "pinch", "pinch": 0.3 }],
        "previous": "swipe_left",
        "note": "TYPESAFE_API_KEY=ts-0123456789abcdef",
    }));
    assert_eq!(findings, 1);
    assert_eq!(state["note"], "[withheld]");
    assert_eq!(state["frames"][0]["pose"], "pinch");
    assert_eq!(state["previous"], "swipe_left");
}

/// Regenerate the `hands` seam's case when the question set or the
/// window changes:
///
/// ```sh
/// cargo test -p coder-hands write_the_hands_seam_case -- --ignored
/// ```
///
/// The case's answers are fixed, not recorded, because the test's promise
/// is the decode and the verdict, not what the live API thinks of this
/// window.
#[test]
#[ignore = "writes the hands seam case; run it to regenerate the fixture"]
fn write_the_hands_seam_case() {
    let (state, findings) = sendable(seam_window().state());
    assert_eq!(findings, 0);
    let request = serde_json::json!({
        "model": "jev-latest",
        "questions": serde_json::to_value(questions()).expect("questions"),
        "state": state,
    });
    let response = serde_json::json!({
        "model": "jev-1.13.0",
        "answers": {
            "intent": {
                "type": "choice",
                "choice": "press",
                "confidence": 0.80,
                "probabilities": {
                    "point": 0.03,
                    "press": 0.80,
                    "release": 0.08,
                    "swipe_left": 0.02,
                    "swipe_right": 0.02,
                    "escape": 0.02,
                    "rest": 0.04,
                    "none": 0.01
                }
            },
            "addressed": { "type": "noul", "noul": 0.92 },
            "continuing": { "type": "noul", "noul": 0.15 }
        },
        "usage": { "input_tokens": 640, "output_tokens": 41 }
    });
    let case = serde_json::json!({
        "why": "the hands seam's read of a borderline-pinch window: fixed answers over the recorded request shape",
        "path": "/v1/systemone",
        "request": request,
        "response": response,
        "request_id": "req-hands-demo",
        "status": 200,
    });
    let text = serde_json::to_string_pretty(&case).expect("the case renders");
    std::fs::write(case_path(), text + "\n").expect("the case is written");
    eprintln!("wrote {}", case_path().display());
}
