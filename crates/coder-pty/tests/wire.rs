//! The NIP-TERM fixtures parse, validate, and round-trip exactly; the
//! invalid ones refuse with the stated reason. Runs without the host.

use coder_pty::wire::{
    Attach, Close, Detach, Frame, Input, Open, Refusal, Resize, Signal, TerminalResult,
};
use coder_pty::{Applied, Body, TerminalState};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn fixtures() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/nip-term.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Parses `json` as `T`, checks it, and confirms it serializes back to the
/// same JSON value.
fn round_trip<T: DeserializeOwned + Serialize>(
    json: &Value,
    check: impl Fn(&T) -> Result<(), Refusal>,
) -> T {
    let body: T = serde_json::from_value(json.clone()).unwrap();
    check(&body).unwrap();
    assert_eq!(&serde_json::to_value(&body).unwrap(), json);
    body
}

#[test]
fn every_valid_fixture_checks_and_round_trips() {
    let all = fixtures();
    let valid = all["valid"].as_object().unwrap();
    for (name, json) in valid {
        match name.as_str() {
            "open" | "open_shell" => {
                round_trip::<Open>(json, Open::check);
            }
            "attach" => {
                round_trip::<Attach>(json, Attach::check);
            }
            "detach" => {
                round_trip::<Detach>(json, Detach::check);
            }
            "input" => {
                let input = round_trip::<Input>(json, Input::check);
                assert_eq!(input.data, b"ls -la\n");
            }
            "resize" => {
                round_trip::<Resize>(json, Resize::check);
            }
            "signal" => {
                round_trip::<Signal>(json, Signal::check);
            }
            "close" => {
                round_trip::<Close>(json, Close::check);
            }
            "result_attached" | "result_lost" => {
                round_trip::<TerminalResult>(json, |_| Ok(()));
            }
            name if name.starts_with("frame_") => {
                round_trip::<Frame>(json, Frame::check);
            }
            other => panic!("no check for fixture {other}"),
        }
    }
}

#[test]
fn every_invalid_fixture_refuses_with_its_reason() {
    let all = fixtures();
    for case in all["invalid"].as_array().unwrap() {
        let body = case["body"].clone();
        let refusal = match case["schema"].as_str().unwrap() {
            "open" => serde_json::from_value::<Open>(body).unwrap().check(),
            "attach" => serde_json::from_value::<Attach>(body).unwrap().check(),
            "frame" => serde_json::from_value::<Frame>(body).unwrap().check(),
            other => panic!("no schema {other}"),
        }
        .unwrap_err();
        assert_eq!(
            refusal.reason.code(),
            case["reason"].as_str().unwrap(),
            "{}",
            case["why"]
        );
    }
}

#[test]
fn the_fixture_frames_apply_as_a_client_would_see_them() {
    let all = fixtures();
    let valid = &all["valid"];
    let frames: Vec<Frame> = ["frame_gap", "frame_output", "frame_exit", "frame_detached"]
        .iter()
        .map(|name| serde_json::from_value(valid[*name].clone()).unwrap())
        .collect();
    let mut state = TerminalState::new(frames[0].terminal.clone(), 100, 80).starting_after(41);
    let applied: Vec<Applied> = frames.iter().map(|frame| state.apply(frame)).collect();
    assert!(matches!(
        applied[0],
        Applied::Gap {
            from: 42,
            to: 49,
            ..
        }
    ));
    assert!(matches!(applied[1], Applied::Output { seq: 50, .. }));
    assert!(matches!(applied[2], Applied::Exit(_)));
    assert!(matches!(applied[3], Applied::Detached(_)));
    assert_eq!(state.resume_after(), 51);
    assert_eq!(state.missed_bytes(), (81920, false));
    assert!(state.screen().text().contains("total 12"));
    assert!(matches!(frames[1].body, Body::Output { .. }));
}
